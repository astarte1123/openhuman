//! Renders a meeting transcript (+ summary, if any) to PDF or DOCX bytes.
//!
//! Pure rendering functions only — given a [`Meeting`] and its
//! [`TranscriptChunk`]s, produce raw file bytes. Deliberately does not touch
//! the filesystem (no `std::fs` calls here), so both functions are easy to
//! unit test without a workspace dir; see `ops::meetings_export` for the
//! RPC-facing function that decides *where* those bytes get written.
//!
//! Both formats are rendered with pure-Rust crates chosen specifically to
//! avoid adding a C-toolchain build requirement to the core crate — this
//! project has been repeatedly blocked by exactly that class of native-dep
//! build breakage (see the `whisper-rs-sys` CRT-mismatch patch and the
//! `onnxruntime` history in git log):
//!
//! - **PDF** via `printpdf`, with `default-features = false` in the
//!   workspace `Cargo.toml`. That drops printpdf's `html`/`text_layout*`
//!   feature set (built on the `azul-layout` HTML/CSS engine, which pulls in
//!   `rust-fontconfig` and friends) — this domain's export is plain
//!   headings + paragraphs, so none of that is needed. Text is placed
//!   line-by-line with `Op::ShowText` using one of the 14 standard PDF fonts
//!   (`BuiltinFont`), which printpdf embeds internally, so no font file
//!   needs to ship with this binary.
//! - **DOCX** via `docx-rs`, which is pure Rust with no native dependencies
//!   at all.
//!
//! Neither crate does text layout for us here (no `text_layout` feature for
//! printpdf; DOCX paragraphs wrap at Word's render time, not ours), so the
//! PDF path does its own crude character-count word wrap ([`wrap_text`]) and
//! manual pagination ([`PdfLayout`]). This is not glyph-accurate but is
//! good enough for a plain-text transcript dump — see the module doc on
//! `mod.rs` note about export scope being "simple, no fancy styling".

use crate::openhuman::meetings::types::{Meeting, TranscriptChunk};
use anyhow::{Context, Result};
use docx_rs::{Docx, Paragraph, Run};
use printpdf::{
    BuiltinFont, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point, Pt, TextItem,
};
use std::io::Cursor;

const PAGE_WIDTH_MM: f32 = 210.0;
const PAGE_HEIGHT_MM: f32 = 297.0;
const MARGIN_MM: f32 = 20.0;

const TITLE_FONT_SIZE_PT: f32 = 18.0;
const TITLE_LINE_HEIGHT_MM: f32 = 10.0;
const HEADING_FONT_SIZE_PT: f32 = 13.0;
const HEADING_LINE_HEIGHT_MM: f32 = 8.0;
const META_FONT_SIZE_PT: f32 = 10.0;
const META_LINE_HEIGHT_MM: f32 = 5.5;
const BODY_FONT_SIZE_PT: f32 = 11.0;
const BODY_LINE_HEIGHT_MM: f32 = 6.0;

/// Rough characters-per-line budget for manual word wrap in the PDF export.
/// printpdf's non-`text_layout` build has no text-measurement API, so
/// wrapping is by character count rather than actual glyph width — good
/// enough for a plain transcript dump on an A4 page with 20mm margins at
/// ~11pt Helvetica.
const WRAP_CHARS: usize = 92;

/// Renders a meeting (title, source/created/duration metadata, full
/// transcript, and summary if present) as PDF file bytes.
pub fn render_pdf(meeting: &Meeting, chunks: &[TranscriptChunk]) -> Result<Vec<u8>> {
    log::debug!(
        "[meetings] export::render_pdf meeting_id={} chunk_count={}",
        meeting.id,
        chunks.len()
    );

    let mut layout = PdfLayout::new();

    layout.line(&meeting.title, BuiltinFont::HelveticaBold, TITLE_FONT_SIZE_PT, TITLE_LINE_HEIGHT_MM);
    for meta_line in wrap_text(&format_meta_line(meeting), WRAP_CHARS) {
        layout.line(&meta_line, BuiltinFont::Helvetica, META_FONT_SIZE_PT, META_LINE_HEIGHT_MM);
    }
    layout.blank_line(BODY_LINE_HEIGHT_MM);

    layout.line("Transcript", BuiltinFont::HelveticaBold, HEADING_FONT_SIZE_PT, HEADING_LINE_HEIGHT_MM);
    if chunks.is_empty() {
        layout.line(
            "(no transcript recorded)",
            BuiltinFont::Helvetica,
            BODY_FONT_SIZE_PT,
            BODY_LINE_HEIGHT_MM,
        );
    } else {
        for chunk in chunks {
            for wrapped in wrap_text(&transcript_line(chunk), WRAP_CHARS) {
                layout.line(&wrapped, BuiltinFont::Helvetica, BODY_FONT_SIZE_PT, BODY_LINE_HEIGHT_MM);
            }
        }
    }

    if let Some(summary) = non_empty_summary(meeting) {
        layout.blank_line(BODY_LINE_HEIGHT_MM);
        layout.line("Summary", BuiltinFont::HelveticaBold, HEADING_FONT_SIZE_PT, HEADING_LINE_HEIGHT_MM);
        for summary_line in summary.lines() {
            if summary_line.trim().is_empty() {
                layout.blank_line(BODY_LINE_HEIGHT_MM);
                continue;
            }
            for wrapped in wrap_text(summary_line, WRAP_CHARS) {
                layout.line(&wrapped, BuiltinFont::Helvetica, BODY_FONT_SIZE_PT, BODY_LINE_HEIGHT_MM);
            }
        }
    }

    let pages = layout.finish();
    let mut doc = PdfDocument::new(&meeting.title);
    let mut warnings = Vec::new();
    let bytes = doc.with_pages(pages).save(&PdfSaveOptions::default(), &mut warnings);

    if !warnings.is_empty() {
        log::debug!(
            "[meetings] export::render_pdf meeting_id={} produced {} save warning(s): {warnings:?}",
            meeting.id,
            warnings.len()
        );
    }

    Ok(bytes)
}

/// Renders a meeting (title, source/created/duration metadata, full
/// transcript, and summary if present) as DOCX file bytes.
pub fn render_docx(meeting: &Meeting, chunks: &[TranscriptChunk]) -> Result<Vec<u8>> {
    log::debug!(
        "[meetings] export::render_docx meeting_id={} chunk_count={}",
        meeting.id,
        chunks.len()
    );

    let mut docx = Docx::new()
        .add_paragraph(heading_paragraph(&meeting.title, TITLE_HEADING_SIZE))
        .add_paragraph(text_paragraph(&format_meta_line(meeting)))
        .add_paragraph(heading_paragraph("Transcript", SECTION_HEADING_SIZE));

    if chunks.is_empty() {
        docx = docx.add_paragraph(text_paragraph("(no transcript recorded)"));
    } else {
        for chunk in chunks {
            docx = docx.add_paragraph(text_paragraph(&transcript_line(chunk)));
        }
    }

    if let Some(summary) = non_empty_summary(meeting) {
        docx = docx.add_paragraph(heading_paragraph("Summary", SECTION_HEADING_SIZE));
        for summary_line in summary.lines() {
            if summary_line.trim().is_empty() {
                continue;
            }
            docx = docx.add_paragraph(text_paragraph(summary_line));
        }
    }

    let mut bytes: Vec<u8> = Vec::new();
    docx.pack(Cursor::new(&mut bytes))
        .context("Failed to pack meeting export DOCX archive")?;

    Ok(bytes)
}

/// docx-rs `Run::size` is in half-points (Word convention), so 36 == 18pt.
const TITLE_HEADING_SIZE: usize = 36;
/// 26 half-points == 13pt.
const SECTION_HEADING_SIZE: usize = 26;

fn heading_paragraph(text: &str, half_point_size: usize) -> Paragraph {
    Paragraph::new().add_run(Run::new().add_text(text.to_string()).bold().size(half_point_size))
}

fn text_paragraph(text: &str) -> Paragraph {
    Paragraph::new().add_run(Run::new().add_text(text.to_string()))
}

/// Returns `meeting.summary` trimmed, or `None` if absent/blank.
fn non_empty_summary(meeting: &Meeting) -> Option<&str> {
    meeting
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Formats one transcript chunk exactly like `ops::meetings_summarize`'s
/// transcript-building step, so the exported document's transcript reads
/// identically to what the summarization LLM saw.
fn transcript_line(chunk: &TranscriptChunk) -> String {
    format!("[{}]: {}", chunk.speaker.as_deref().unwrap_or("unknown"), chunk.text)
}

/// One line of "Source: ... | Created: ... | Duration: ..." metadata shown
/// under the title in both export formats.
fn format_meta_line(meeting: &Meeting) -> String {
    let mut parts = vec![
        format!("Source: {}", meeting.source),
        format!("Created: {}", meeting.created_at),
    ];
    if let Some(duration_ms) = meeting.duration_ms {
        parts.push(format!("Duration: {}", format_duration_ms(duration_ms)));
    }
    parts.join("   |   ")
}

/// Formats a millisecond duration as `H:MM:SS` (or `M:SS` under an hour).
/// Negative input (shouldn't happen — `duration_ms` is set once from an
/// elapsed recording) is clamped to zero rather than underflowing.
fn format_duration_ms(ms: i64) -> String {
    let total_secs = ms.max(0) / 1000;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// Greedy word-wrap by character count (see [`WRAP_CHARS`] doc comment for
/// why this isn't glyph-width-based). A single word longer than `max_chars`
/// is left unsplit on its own line rather than being hyphenated.
fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        // Preserve blank lines (e.g. an empty transcript chunk's text)
        // rather than collapsing them away.
        lines.push(String::new());
    }
    lines
}

/// Manual line-by-line PDF layout: tracks a vertical cursor in mm from the
/// top margin and starts a new page whenever the next line would cross the
/// bottom margin. printpdf's non-`text_layout` build has no pagination of
/// its own, so this is the whole "layout engine" for this export.
struct PdfLayout {
    pages: Vec<PdfPage>,
    ops: Vec<Op>,
    y_mm: f32,
}

impl PdfLayout {
    fn new() -> Self {
        Self {
            pages: Vec::new(),
            ops: vec![Op::StartTextSection],
            y_mm: PAGE_HEIGHT_MM - MARGIN_MM,
        }
    }

    /// Writes one line of text at the current vertical cursor, positioning
    /// it explicitly with `Op::SetTextCursor` (rather than relying on PDF's
    /// leading + line-break operators) so page breaks are just "start a new
    /// `Vec<Op>` and reset `y_mm`" with no operator-state to carry over.
    fn line(&mut self, text: &str, font: BuiltinFont, font_size_pt: f32, line_height_mm: f32) {
        self.ensure_space(line_height_mm);
        self.ops.push(Op::SetFont {
            font: PdfFontHandle::Builtin(font),
            size: Pt(font_size_pt),
        });
        self.ops.push(Op::SetTextCursor {
            pos: Point::new(Mm(MARGIN_MM), Mm(self.y_mm)),
        });
        if !text.is_empty() {
            self.ops.push(Op::ShowText {
                items: vec![TextItem::Text(text.to_string())],
            });
        }
        self.y_mm -= line_height_mm;
    }

    /// Advances the cursor by `line_height_mm` without drawing anything —
    /// used for the blank line between sections.
    fn blank_line(&mut self, line_height_mm: f32) {
        self.ensure_space(line_height_mm);
        self.y_mm -= line_height_mm;
    }

    fn ensure_space(&mut self, line_height_mm: f32) {
        if self.y_mm - line_height_mm < MARGIN_MM {
            self.flush_page();
        }
    }

    fn flush_page(&mut self) {
        self.ops.push(Op::EndTextSection);
        let ops = std::mem::replace(&mut self.ops, vec![Op::StartTextSection]);
        self.pages
            .push(PdfPage::new(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), ops));
        self.y_mm = PAGE_HEIGHT_MM - MARGIN_MM;
    }

    fn finish(mut self) -> Vec<PdfPage> {
        self.ops.push(Op::EndTextSection);
        self.pages
            .push(PdfPage::new(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), self.ops));
        self.pages
    }
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
