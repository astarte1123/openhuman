//! Renders a meeting (title, metadata, participants, summary) to a minimal
//! but valid PowerPoint (`.pptx`) deck.
//!
//! No pptx-writing crate exists in the pure-Rust ecosystem the way
//! `docx-rs` does for `.docx` (see `export.rs`'s module doc for why this
//! project avoids native-dependency crates), so this hand-rolls the OOXML
//! package: a `.pptx` is just a zip archive of XML parts following the
//! ECMA-376 PresentationML schema. This writes the minimum part set a
//! real viewer (PowerPoint, LibreOffice Impress, Google Slides import)
//! requires — one shared slide master/layout, and one `slideN.xml` per
//! slide with a title + body text placeholder. No images, transitions, or
//! per-slide styling; this is a plain-text deck, matching the PDF/DOCX
//! export's "simple, no fancy styling" scope (see `mod.rs`).
//!
//! Deck contents: a title slide (meeting title + source/date/duration),
//! then one or more summary slides (the prose summary word-wrapped and
//! paginated across slides so no single slide overflows), each carrying a
//! "Participants" line built from the distinct `speaker` labels seen in
//! the transcript. Renders an explicit "(no summary yet)" slide rather
//! than silently omitting content when `meeting.summary` is empty, so a
//! user who exports before summarizing gets a clear signal instead of a
//! confusing near-empty deck.

use crate::aura::meetings::types::{Meeting, TranscriptChunk};
use anyhow::{Context, Result};
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Rough characters-per-line budget for the body placeholder, and maximum
/// lines before starting a new slide. Matches `export.rs`'s
/// character-count word-wrap approach (no text-measurement API available
/// without pulling in a layout engine) — good enough for a plain-text
/// summary dump, not glyph-accurate.
const WRAP_CHARS: usize = 70;
const MAX_LINES_PER_SLIDE: usize = 14;

/// Renders a meeting to `.pptx` file bytes.
pub fn render_pptx(meeting: &Meeting, chunks: &[TranscriptChunk]) -> Result<Vec<u8>> {
    log::debug!(
        "[meetings] export_pptx::render_pptx meeting_id={} chunk_count={}",
        meeting.id,
        chunks.len()
    );

    let mut slide_bodies: Vec<Vec<String>> = Vec::new();

    // Title slide body: metadata + participants, no summary text. Prefer
    // the planned attendee roster the user entered at meeting creation;
    // fall back to the distinct speaker labels actually seen in the
    // transcript (e.g. "You"/"Others" from per-channel tagging) when no
    // roster was entered.
    let mut title_body = vec![format_meta_line(meeting)];
    if !meeting.participants.is_empty() {
        title_body.push(format!("Participants: {}", meeting.participants.join(", ")));
    } else {
        let participants = distinct_speakers(chunks);
        if !participants.is_empty() {
            title_body.push(format!("Participants: {}", participants.join(", ")));
        }
    }
    slide_bodies.push(title_body);

    // Summary slides: paginate the wrapped summary lines across slides so
    // none overflows MAX_LINES_PER_SLIDE.
    let summary_lines = summary_lines_for_deck(meeting);
    for page in summary_lines.chunks(MAX_LINES_PER_SLIDE) {
        slide_bodies.push(page.to_vec());
    }

    let slide_titles: Vec<&str> = std::iter::once(meeting.title.as_str())
        .chain(std::iter::repeat("Summary").take(slide_bodies.len() - 1))
        .collect();

    write_pptx_zip(&slide_titles, &slide_bodies)
}

/// Distinct, order-preserving list of non-empty `speaker` labels seen
/// across the transcript (e.g. `["You", "Others"]` for the per-channel
/// tagging the meeting recorder now applies — see
/// `app/src-tauri/src/meeting_recorder/recorder.rs`).
fn distinct_speakers(chunks: &[TranscriptChunk]) -> Vec<String> {
    let mut seen = Vec::new();
    for chunk in chunks {
        if let Some(speaker) = chunk.speaker.as_deref().map(str::trim) {
            if !speaker.is_empty() && !seen.iter().any(|s: &String| s == speaker) {
                seen.push(speaker.to_string());
            }
        }
    }
    seen
}

fn summary_lines_for_deck(meeting: &Meeting) -> Vec<String> {
    let summary = meeting
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let Some(summary) = summary else {
        return vec![
            "(no summary yet — generate one before exporting for full content)".to_string(),
        ];
    };
    let mut lines = Vec::new();
    for raw_line in summary.lines() {
        if raw_line.trim().is_empty() {
            continue;
        }
        lines.extend(wrap_text(raw_line, WRAP_CHARS));
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn format_meta_line(meeting: &Meeting) -> String {
    let mut parts = vec![
        format!("Source: {}", meeting.source),
        format!("Created: {}", meeting.created_at),
    ];
    if let Some(duration_ms) = meeting.duration_ms {
        let total_secs = duration_ms.max(0) / 1000;
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;
        parts.push(if hours > 0 {
            format!("Duration: {hours}:{minutes:02}:{seconds:02}")
        } else {
            format!("Duration: {minutes}:{seconds:02}")
        });
    }
    parts.join("   |   ")
}

/// Greedy word-wrap by character count — see `export.rs::wrap_text` (same
/// approach, duplicated rather than shared across a pdf/docx-specific
/// module boundary to keep `export_pptx.rs` self-contained).
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
        lines.push(String::new());
    }
    lines
}

/// XML-escapes the five predefined XML entities. PowerPoint text runs are
/// plain XML text nodes, so unescaped `&`/`<` in a transcript or summary
/// (e.g. "R&D", "if x < y") would otherwise produce a corrupt, unopenable
/// `.pptx`.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Assembles the full OOXML package and returns its zip bytes.
/// `titles`/`bodies` must be the same length (one entry per slide).
fn write_pptx_zip(titles: &[&str], bodies: &[Vec<String>]) -> Result<Vec<u8>> {
    let slide_count = titles.len();
    let mut buf = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));
        let opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        write_part(
            &mut zip,
            opts,
            "[Content_Types].xml",
            &content_types_xml(slide_count),
        )?;
        write_part(&mut zip, opts, "_rels/.rels", ROOT_RELS_XML)?;
        write_part(
            &mut zip,
            opts,
            "ppt/presentation.xml",
            &presentation_xml(slide_count),
        )?;
        write_part(
            &mut zip,
            opts,
            "ppt/_rels/presentation.xml.rels",
            &presentation_rels_xml(slide_count),
        )?;
        write_part(&mut zip, opts, "ppt/theme/theme1.xml", THEME_XML)?;
        write_part(
            &mut zip,
            opts,
            "ppt/slideMasters/slideMaster1.xml",
            SLIDE_MASTER_XML,
        )?;
        write_part(
            &mut zip,
            opts,
            "ppt/slideMasters/_rels/slideMaster1.xml.rels",
            SLIDE_MASTER_RELS_XML,
        )?;
        write_part(
            &mut zip,
            opts,
            "ppt/slideLayouts/slideLayout1.xml",
            SLIDE_LAYOUT_XML,
        )?;
        write_part(
            &mut zip,
            opts,
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
            SLIDE_LAYOUT_RELS_XML,
        )?;

        for (i, (title, body)) in titles.iter().zip(bodies.iter()).enumerate() {
            let n = i + 1;
            write_part(
                &mut zip,
                opts,
                &format!("ppt/slides/slide{n}.xml"),
                &slide_xml(title, body),
            )?;
            write_part(
                &mut zip,
                opts,
                &format!("ppt/slides/_rels/slide{n}.xml.rels"),
                SLIDE_RELS_XML,
            )?;
        }

        zip.finish()
            .context("failed to finalize pptx zip archive")?;
    }
    Ok(buf)
}

fn write_part<W: std::io::Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    opts: SimpleFileOptions,
    path: &str,
    contents: &str,
) -> Result<()> {
    zip.start_file(path, opts)
        .with_context(|| format!("failed to start pptx zip entry {path}"))?;
    zip.write_all(contents.as_bytes())
        .with_context(|| format!("failed to write pptx zip entry {path}"))?;
    Ok(())
}

const ROOT_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#;

fn content_types_xml(slide_count: usize) -> String {
    let overrides: String = (1..=slide_count)
        .map(|n| {
            format!(
                r#"<Override PartName="/ppt/slides/slide{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
            )
        })
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
<Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
<Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
<Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
{overrides}
</Types>"#
    )
}

fn presentation_xml(slide_count: usize) -> String {
    let sld_id_lst: String = (1..=slide_count)
        .map(|n| format!(r#"<p:sldId id="{}" r:id="rId{}"/>"#, 255 + n, n + 1))
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>
<p:sldIdLst>{sld_id_lst}</p:sldIdLst>
<p:sldSz cx="12192000" cy="6858000" type="screen16x9"/>
<p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>"#
    )
}

fn presentation_rels_xml(slide_count: usize) -> String {
    let mut rels = String::from(
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>"#,
    );
    for n in 1..=slide_count {
        rels.push_str(&format!(
            r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{}.xml"/>"#,
            n + 1,
            n
        ));
    }
    let theme_rid = slide_count + 2;
    rels.push_str(&format!(
        r#"<Relationship Id="rId{theme_rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>"#
    ));
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#
    )
}

const SLIDE_MASTER_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>"#;

const SLIDE_LAYOUT_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>"#;

const SLIDE_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>"#;

const SLIDE_MASTER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld>
<p:spTree>
<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
<p:grpSpPr/>
</p:spTree>
</p:cSld>
<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst>
</p:sldMaster>"#;

const SLIDE_LAYOUT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="title">
<p:cSld>
<p:spTree>
<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
<p:grpSpPr/>
</p:spTree>
</p:cSld>
</p:sldLayout>"#;

const THEME_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Youma">
<a:themeElements>
<a:clrScheme name="Youma">
<a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>
<a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
<a:dk2><a:srgbClr val="1F2933"/></a:dk2>
<a:lt2><a:srgbClr val="F5F5F4"/></a:lt2>
<a:accent1><a:srgbClr val="4A83DD"/></a:accent1>
<a:accent2><a:srgbClr val="4A83DD"/></a:accent2>
<a:accent3><a:srgbClr val="4A83DD"/></a:accent3>
<a:accent4><a:srgbClr val="4A83DD"/></a:accent4>
<a:accent5><a:srgbClr val="4A83DD"/></a:accent5>
<a:accent6><a:srgbClr val="4A83DD"/></a:accent6>
<a:hlink><a:srgbClr val="4A83DD"/></a:hlink>
<a:folHlink><a:srgbClr val="4A83DD"/></a:folHlink>
</a:clrScheme>
<a:fontScheme name="Youma">
<a:majorFont><a:latin typeface="Calibri"/></a:majorFont>
<a:minorFont><a:latin typeface="Calibri"/></a:minorFont>
</a:fontScheme>
<a:fmtScheme name="Youma">
<a:fillStyleLst><a:solidFill><a:schemeClr val="accent1"/></a:solidFill><a:solidFill><a:schemeClr val="accent1"/></a:solidFill><a:solidFill><a:schemeClr val="accent1"/></a:solidFill></a:fillStyleLst>
<a:lnStyleLst><a:ln><a:solidFill><a:schemeClr val="accent1"/></a:solidFill></a:ln><a:ln><a:solidFill><a:schemeClr val="accent1"/></a:solidFill></a:ln><a:ln><a:solidFill><a:schemeClr val="accent1"/></a:solidFill></a:ln></a:lnStyleLst>
<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>
<a:bgFillStyleLst><a:solidFill><a:schemeClr val="lt1"/></a:solidFill><a:solidFill><a:schemeClr val="lt1"/></a:solidFill><a:solidFill><a:schemeClr val="lt1"/></a:solidFill></a:bgFillStyleLst>
</a:fmtScheme>
</a:themeElements>
</a:theme>"#;

fn slide_xml(title: &str, body_lines: &[String]) -> String {
    let title_escaped = xml_escape(title);
    let body_paragraphs: String = if body_lines.is_empty() {
        r#"<a:p><a:endParaRPr lang="en-US"/></a:p>"#.to_string()
    } else {
        body_lines
            .iter()
            .map(|line| {
                format!(
                    r#"<a:p><a:r><a:rPr lang="en-US" sz="1800" dirty="0"/><a:t>{}</a:t></a:r></a:p>"#,
                    xml_escape(line)
                )
            })
            .collect()
    };

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld>
<p:spTree>
<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
<p:grpSpPr/>
<p:sp>
<p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr>
<p:spPr>
<a:xfrm><a:off x="457200" y="274638"/><a:ext cx="11277600" cy="1143000"/></a:xfrm>
<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
</p:spPr>
<p:txBody>
<a:bodyPr/><a:lstStyle/>
<a:p><a:r><a:rPr lang="en-US" sz="3200" b="1" dirty="0"/><a:t>{title_escaped}</a:t></a:r></a:p>
</p:txBody>
</p:sp>
<p:sp>
<p:nvSpPr><p:cNvPr id="3" name="Body"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr>
<p:spPr>
<a:xfrm><a:off x="457200" y="1600200"/><a:ext cx="11277600" cy="4800600"/></a:xfrm>
<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
</p:spPr>
<p:txBody>
<a:bodyPr/><a:lstStyle/>
{body_paragraphs}
</p:txBody>
</p:sp>
</p:spTree>
</p:cSld>
</p:sld>"#
    )
}

#[cfg(test)]
#[path = "export_pptx_tests.rs"]
mod tests;
