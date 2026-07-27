// office.rs — Office Open XML document generation (DOCX, XLSX) via the zip crate.
//
// Both formats are ZIP archives containing XML files:
//   DOCX: [Content_Types].xml, word/document.xml, _rels/.rels, word/_rels/document.xml.rels
//   XLSX: [Content_Types].xml, xl/workbook.xml, xl/worksheets/sheet1.xml,
//         xl/sharedStrings.xml, xl/_rels/workbook.xml.rels, _rels/.rels
//
// Markdown→DOCX body handles: # ## ### headings, **bold**, *italic*, - bullet lists, plain text.

use std::io::Write;
use zip::{write::FileOptions, CompressionMethod, ZipWriter};

// ---------------------------------------------------------------------------
// Downloads directory
// ---------------------------------------------------------------------------

pub fn downloads_dir() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            return format!("{}\\Downloads", profile);
        }
    }
    // macOS / Linux
    if let Ok(home) = std::env::var("HOME") {
        return format!("{}/Downloads", home);
    }
    ".".to_string()
}

fn unique_path(dir: &str, stem: &str, ext: &str) -> String {
    let sep = if dir.contains('\\') { "\\" } else { "/" };
    let base = format!("{}{}{}.{}", dir, sep, stem, ext);
    if !std::path::Path::new(&base).exists() {
        return base;
    }
    for n in 2..=999 {
        let candidate = format!("{}{}{} ({}).{}", dir, sep, stem, n, ext);
        if !std::path::Path::new(&candidate).exists() {
            return candidate;
        }
    }
    base
}

// ---------------------------------------------------------------------------
// XML helpers
// ---------------------------------------------------------------------------

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ---------------------------------------------------------------------------
// DOCX generation
// ---------------------------------------------------------------------------

/// Convert a line of Markdown text into OOXML run XML.
/// Handles **bold**, *italic*, and plain text (inline only).
fn md_runs(line: &str) -> String {
    let mut result = String::new();
    let mut rest = line;

    while !rest.is_empty() {
        // Bold: **text**
        if let Some(start) = rest.find("**") {
            let before = &rest[..start];
            if !before.is_empty() {
                result.push_str(&format!(
                    "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                    xml_escape(before)
                ));
            }
            rest = &rest[start + 2..];
            if let Some(end) = rest.find("**") {
                let bold_text = &rest[..end];
                result.push_str(&format!(
                    "<w:r><w:rPr><w:b/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                    xml_escape(bold_text)
                ));
                rest = &rest[end + 2..];
                continue;
            } else {
                // Unmatched ** — treat as literal
                result.push_str("<w:r><w:t>**</w:t></w:r>");
            }
        } else if let Some(start) = rest.find('*') {
            let before = &rest[..start];
            if !before.is_empty() {
                result.push_str(&format!(
                    "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                    xml_escape(before)
                ));
            }
            rest = &rest[start + 1..];
            if let Some(end) = rest.find('*') {
                let italic_text = &rest[..end];
                result.push_str(&format!(
                    "<w:r><w:rPr><w:i/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                    xml_escape(italic_text)
                ));
                rest = &rest[end + 1..];
                continue;
            } else {
                result.push_str("<w:r><w:t>*</w:t></w:r>");
            }
        } else {
            result.push_str(&format!(
                "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                xml_escape(rest)
            ));
            break;
        }
    }
    result
}

/// Convert Markdown content to a sequence of OOXML paragraph elements.
fn md_to_ooxml_paragraphs(md: &str) -> String {
    let mut out = String::new();
    for line in md.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            // Empty line = blank paragraph spacer
            out.push_str("<w:p><w:pPr><w:jc w:val=\"left\"/></w:pPr></w:p>");
            continue;
        }
        // Headings
        if let Some(rest) = trimmed.strip_prefix("### ") {
            out.push_str(&format!(
                "<w:p><w:pPr><w:pStyle w:val=\"Heading3\"/></w:pPr><w:r><w:t>{}</w:t></w:r></w:p>",
                xml_escape(rest)
            ));
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            out.push_str(&format!(
                "<w:p><w:pPr><w:pStyle w:val=\"Heading2\"/></w:pPr><w:r><w:t>{}</w:t></w:r></w:p>",
                xml_escape(rest)
            ));
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            out.push_str(&format!(
                "<w:p><w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr><w:r><w:t>{}</w:t></w:r></w:p>",
                xml_escape(rest)
            ));
        } else if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            // Bullet list item
            out.push_str(&format!(
                "<w:p><w:pPr><w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"1\"/></w:numPr></w:pPr>{}</w:p>",
                md_runs(rest)
            ));
        } else {
            out.push_str(&format!("<w:p>{}</w:p>", md_runs(trimmed)));
        }
    }
    out
}

/// Generate a .docx file from markdown content. Returns the saved file path.
pub fn create_docx(title: &str, content_md: &str, filename: Option<&str>) -> Result<String, String> {
    eprintln!("[office] create_docx title={title:?} content_len={}", content_md.len());

    let stem = filename
        .map(|f| f.trim_end_matches(".docx").to_string())
        .unwrap_or_else(|| sanitize_filename(title));
    let dir = downloads_dir();
    let path = unique_path(&dir, &stem, "docx");

    let file = std::fs::File::create(&path)
        .map_err(|e| format!("create docx file: {e}"))?;
    let mut zip = ZipWriter::new(file);
    let opts = FileOptions::default()
        .compression_method(CompressionMethod::Deflated);

    // [Content_Types].xml
    zip.start_file("[Content_Types].xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(DOCX_CONTENT_TYPES.as_bytes()).map_err(|e| e.to_string())?;

    // _rels/.rels
    zip.start_file("_rels/.rels", opts).map_err(|e| e.to_string())?;
    zip.write_all(DOCX_RELS.as_bytes()).map_err(|e| e.to_string())?;

    // word/_rels/document.xml.rels
    zip.start_file("word/_rels/document.xml.rels", opts).map_err(|e| e.to_string())?;
    zip.write_all(DOCX_DOCUMENT_RELS.as_bytes()).map_err(|e| e.to_string())?;

    // word/document.xml
    let body = md_to_ooxml_paragraphs(content_md);
    let title_para = format!(
        "<w:p><w:pPr><w:pStyle w:val=\"Title\"/></w:pPr><w:r><w:t>{}</w:t></w:r></w:p>",
        xml_escape(title)
    );
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:wpc="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas"
            xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<w:body>
{title_para}
{body}
<w:sectPr><w:pgSz w:w="12240" w:h="15840"/></w:sectPr>
</w:body>
</w:document>"#,
        title_para = title_para,
        body = body
    );
    zip.start_file("word/document.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(document.as_bytes()).map_err(|e| e.to_string())?;

    // word/styles.xml — minimal styles for headings and lists
    zip.start_file("word/styles.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(DOCX_STYLES.as_bytes()).map_err(|e| e.to_string())?;

    // word/numbering.xml — list numbering definition
    zip.start_file("word/numbering.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(DOCX_NUMBERING.as_bytes()).map_err(|e| e.to_string())?;

    zip.finish().map_err(|e| e.to_string())?;

    eprintln!("[office] docx saved: {path}");
    Ok(path)
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .take(80)
        .collect::<String>()
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// Shared markdown helpers
// ---------------------------------------------------------------------------

/// Remove **bold**, *italic*, and `code` inline markers, keeping the text.
fn strip_md_inline(s: &str) -> String {
    let mut out = s.to_string();
    for marker in &["**", "*", "`"] {
        loop {
            if let Some(a) = out.find(marker) {
                if let Some(b) = out[a + marker.len()..].find(marker) {
                    let content = out[a + marker.len()..a + marker.len() + b].to_string();
                    out = format!("{}{}{}", &out[..a], content, &out[a + marker.len() + b + marker.len()..]);
                } else { break; }
            } else { break; }
        }
    }
    out
}

/// Word-wrap `text` to fit `max_pts` wide at the given `font_size`.
/// Approximates Helvetica average char width as 55% of font size.
fn wrap_text_lines(text: &str, max_pts: f32, font_size: f32) -> Vec<String> {
    let avg_w = font_size * 0.55_f32;
    let max_chars = ((max_pts / avg_w).floor() as usize).max(10);
    if text.len() <= max_chars {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut rest = text.trim();
    while rest.len() > max_chars {
        // find last space before max_chars boundary
        let boundary = rest
            .char_indices()
            .nth(max_chars)
            .map(|(i, _)| i)
            .unwrap_or(rest.len());
        let break_at = rest[..boundary].rfind(' ').unwrap_or(boundary);
        lines.push(rest[..break_at].trim_end().to_string());
        rest = rest[break_at..].trim_start();
    }
    if !rest.is_empty() { lines.push(rest.to_string()); }
    if lines.is_empty() { lines.push(String::new()); }
    lines
}

// ---------------------------------------------------------------------------
// PDF generation
// ---------------------------------------------------------------------------

/// Escape a UTF-8 string into a PDF literal string using octal escapes for
/// non-ASCII bytes (WinAnsiEncoding compatible — covers all Latin-1 chars
/// used in Italian, French, German, Spanish, etc.).
fn pdf_escape(s: &str) -> String {
    let mut out = String::from("(");
    for c in s.chars() {
        let b = if (c as u32) < 256 { c as u8 } else { b'?' };
        match b {
            b'('  => out.push_str("\\("),
            b')'  => out.push_str("\\)"),
            b'\\' => out.push_str("\\\\"),
            b if b >= 32 && b < 128 => out.push(b as char),
            b if b >= 128 => { out.push_str(&format!("\\{:03o}", b)); }
            _ => {}
        }
    }
    out.push(')');
    out
}

/// Generate a .pdf file from markdown content. Returns the saved file path.
pub fn create_pdf(title: &str, content_md: &str, filename: Option<&str>) -> Result<String, String> {
    eprintln!("[office] create_pdf title={title:?} content_len={}", content_md.len());

    let stem = filename
        .map(|f| f.trim_end_matches(".pdf").to_string())
        .unwrap_or_else(|| sanitize_filename(title));
    let dir = downloads_dir();
    let path = unique_path(&dir, &stem, "pdf");

    // A4 page, points
    const W: f32 = 595.0;
    const H: f32 = 842.0;
    const ML: f32 = 60.0;
    const MR: f32 = 60.0;
    const MT: f32 = 60.0;
    const MB: f32 = 60.0;
    let text_w = W - ML - MR;

    // Render items: (y, x_indent, font_name, font_size, text)
    #[derive(Clone)]
    struct RItem {
        text:   String,
        size:   f32,
        bold:   bool,
        indent: f32,
        gap:    f32,  // extra vertical gap before this item
    }

    let mut items: Vec<RItem> = Vec::new();
    let push = |items: &mut Vec<RItem>, text: &str, size: f32, bold: bool, indent: f32, gap: f32| {
        items.push(RItem { text: text.to_string(), size, bold, indent, gap });
    };

    // Document title
    for line in wrap_text_lines(title, text_w, 20.0) {
        push(&mut items, &line, 20.0, true, 0.0, 0.0);
    }
    push(&mut items, "", 11.0, false, 0.0, 8.0); // spacer

    for raw in content_md.lines() {
        let t = raw.trim();
        if let Some(r) = t.strip_prefix("### ") {
            let txt = strip_md_inline(r);
            for l in wrap_text_lines(&txt, text_w, 12.0) { push(&mut items, &l, 12.0, true, 0.0, 8.0); }
        } else if let Some(r) = t.strip_prefix("## ") {
            let txt = strip_md_inline(r);
            for l in wrap_text_lines(&txt, text_w, 14.0) { push(&mut items, &l, 14.0, true, 0.0, 10.0); }
        } else if let Some(r) = t.strip_prefix("# ") {
            let txt = strip_md_inline(r);
            for l in wrap_text_lines(&txt, text_w, 17.0) { push(&mut items, &l, 17.0, true, 0.0, 12.0); }
        } else if let Some(r) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            let txt = format!("-  {}", strip_md_inline(r));
            for l in wrap_text_lines(&txt, text_w - 16.0, 11.0) { push(&mut items, &l, 11.0, false, 16.0, 0.0); }
        } else if t.is_empty() {
            push(&mut items, "", 11.0, false, 0.0, 5.0);
        } else {
            let txt = strip_md_inline(t);
            for l in wrap_text_lines(&txt, text_w, 11.0) { push(&mut items, &l, 11.0, false, 0.0, 0.0); }
        }
    }

    // Paginate
    struct Page { ops: String }
    let mut pages: Vec<Page> = Vec::new();

    let mut ops = String::new();
    let mut y: f32 = H - MT;
    let mut cur_font = ("", 0f32);

    let flush_page = |ops: &mut String, pages: &mut Vec<Page>| {
        if !ops.is_empty() { ops.push_str("ET\n"); }
        pages.push(Page { ops: ops.clone() });
        ops.clear();
    };

    let mut first_on_page = true;

    for item in &items {
        let line_h = item.size * 1.4;

        if item.text.is_empty() {
            y -= item.gap.max(line_h * 0.4);
            if y < MB { flush_page(&mut ops, &mut pages); y = H - MT; first_on_page = true; }
            continue;
        }

        let needed = line_h + if first_on_page { 0.0 } else { item.gap };
        if y - needed < MB {
            flush_page(&mut ops, &mut pages);
            y = H - MT;
            first_on_page = true;
        }

        if ops.is_empty() { ops.push_str("BT\n"); }

        y -= if first_on_page { 0.0 } else { item.gap };
        y -= line_h;

        let fname = if item.bold { "F2" } else { "F1" };
        if cur_font != (fname, item.size) {
            ops.push_str(&format!("/{fname} {:.1} Tf\n", item.size));
            cur_font = (fname, item.size);
        }

        let x = ML + item.indent;
        ops.push_str(&format!("1 0 0 1 {:.2} {:.2} Tm\n", x, y));
        ops.push_str(&format!("{} Tj\n", pdf_escape(&item.text)));
        first_on_page = false;
    }
    if !ops.is_empty() {
        ops.push_str("ET\n");
        pages.push(Page { ops });
    }
    if pages.is_empty() { pages.push(Page { ops: "BT\nET\n".into() }); }

    // Build PDF byte stream
    let n = pages.len();
    // Object numbering:
    //   1 = Catalog, 2 = Pages, 3 = Font F1, 4 = Font F2
    //   5+2i   = Page i dict
    //   5+2i+1 = Content stream i
    let total_objs = 4 + n * 2;
    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
    let mut xref: Vec<usize> = vec![0; total_objs + 1]; // 1-indexed

    macro_rules! write_obj {
        ($num:expr, $body:expr) => {{
            xref[$num] = buf.len();
            let s = format!("{} 0 obj\n{}\nendobj\n", $num, $body);
            buf.extend_from_slice(s.as_bytes());
        }};
    }

    buf.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");

    // Obj 1: Catalog
    write_obj!(1, "<<\n/Type /Catalog\n/Pages 2 0 R\n>>");

    // Obj 2: Pages
    let kids: String = (0..n).map(|i| format!("{} 0 R", 5 + i * 2)).collect::<Vec<_>>().join(" ");
    write_obj!(2, format!("<<\n/Type /Pages\n/Kids [{kids}]\n/Count {n}\n>>"));

    // Obj 3: Helvetica
    write_obj!(3, "<<\n/Type /Font\n/Subtype /Type1\n/BaseFont /Helvetica\n/Encoding /WinAnsiEncoding\n>>");

    // Obj 4: Helvetica-Bold
    write_obj!(4, "<<\n/Type /Font\n/Subtype /Type1\n/BaseFont /Helvetica-Bold\n/Encoding /WinAnsiEncoding\n>>");

    let resources = "/Resources <<\n/Font << /F1 3 0 R /F2 4 0 R >>\n>>";

    for (i, page) in pages.iter().enumerate() {
        let page_obj    = 5 + i * 2;
        let content_obj = page_obj + 1;

        // Page dict
        write_obj!(page_obj, format!(
            "<<\n/Type /Page\n/Parent 2 0 R\n/MediaBox [0 0 {W:.0} {H:.0}]\n{resources}\n/Contents {content_obj} 0 R\n>>"
        ));

        // Content stream
        let stream = page.ops.as_bytes();
        let len = stream.len();
        let header = format!("{content_obj} 0 obj\n<<\n/Length {len}\n>>\nstream\n");
        xref[content_obj] = buf.len();
        buf.extend_from_slice(header.as_bytes());
        buf.extend_from_slice(stream);
        buf.extend_from_slice(b"\nendstream\nendobj\n");
    }

    // Cross-reference table
    let xref_offset = buf.len();
    buf.extend_from_slice(format!("xref\n0 {}\n", total_objs + 1).as_bytes());
    buf.extend_from_slice(b"0000000000 65535 f \n");
    for i in 1..=total_objs {
        buf.extend_from_slice(format!("{:010} 00000 n \n", xref[i]).as_bytes());
    }
    buf.extend_from_slice(
        format!("trailer\n<<\n/Size {}\n/Root 1 0 R\n>>\nstartxref\n{xref_offset}\n%%EOF\n",
            total_objs + 1).as_bytes()
    );

    std::fs::write(&path, &buf).map_err(|e| format!("write pdf: {e}"))?;
    eprintln!("[office] pdf saved: {path} ({} bytes)", buf.len());
    Ok(path)
}

// ---------------------------------------------------------------------------
// PPTX generation
// ---------------------------------------------------------------------------

/// Generate a .pptx file from markdown content.
/// Each `# Heading` starts a new slide. Content between headings goes on that slide.
/// Returns the saved file path.
pub fn create_pptx(title: &str, content_md: &str, filename: Option<&str>) -> Result<String, String> {
    eprintln!("[office] create_pptx title={title:?}");

    let stem = filename
        .map(|f| f.trim_end_matches(".pptx").to_string())
        .unwrap_or_else(|| sanitize_filename(title));
    let dir = downloads_dir();
    let path = unique_path(&dir, &stem, "pptx");

    // Parse markdown into slides: [(slide_title, Vec<content_line>)]
    let mut slides: Vec<(String, Vec<String>)> = Vec::new();
    // Title slide
    slides.push((title.to_string(), Vec::new()));

    for line in content_md.lines() {
        let t = line.trim();
        if let Some(r) = t.strip_prefix("# ") {
            slides.push((strip_md_inline(r), Vec::new()));
        } else if let Some(r) = t.strip_prefix("## ") {
            // Sub-heading on same slide
            if let Some(last) = slides.last_mut() {
                last.1.push(format!("  {}", strip_md_inline(r)));
            }
        } else if let Some(r) = t.strip_prefix("### ") {
            if let Some(last) = slides.last_mut() {
                last.1.push(format!("    {}", strip_md_inline(r)));
            }
        } else if let Some(r) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            if let Some(last) = slides.last_mut() {
                last.1.push(format!("\u{2022} {}", strip_md_inline(r)));
            }
        } else if !t.is_empty() {
            if let Some(last) = slides.last_mut() {
                last.1.push(strip_md_inline(t));
            }
        }
    }

    // Build PPTX ZIP
    let file = std::fs::File::create(&path).map_err(|e| format!("create pptx: {e}"))?;
    let mut zip = ZipWriter::new(file);
    let opts = FileOptions::default().compression_method(CompressionMethod::Deflated);

    // [Content_Types].xml
    let slide_ct: String = slides.iter().enumerate()
        .map(|(i, _)| format!(
            r#"<Override PartName="/ppt/slides/slide{}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#,
            i + 1
        ))
        .collect::<Vec<_>>()
        .join("\n");
    let ct = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
<Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
<Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
<Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
{slide_ct}
</Types>"#);
    zip.start_file("[Content_Types].xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(ct.as_bytes()).map_err(|e| e.to_string())?;

    // _rels/.rels
    zip.start_file("_rels/.rels", opts).map_err(|e| e.to_string())?;
    zip.write_all(PPTX_ROOT_RELS.as_bytes()).map_err(|e| e.to_string())?;

    // ppt/theme/theme1.xml
    zip.start_file("ppt/theme/theme1.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(PPTX_THEME.as_bytes()).map_err(|e| e.to_string())?;

    // ppt/slideMasters/slideMaster1.xml
    zip.start_file("ppt/slideMasters/slideMaster1.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(PPTX_SLIDE_MASTER.as_bytes()).map_err(|e| e.to_string())?;

    // ppt/slideMasters/_rels/slideMaster1.xml.rels
    zip.start_file("ppt/slideMasters/_rels/slideMaster1.xml.rels", opts).map_err(|e| e.to_string())?;
    zip.write_all(PPTX_SLIDE_MASTER_RELS.as_bytes()).map_err(|e| e.to_string())?;

    // ppt/slideLayouts/slideLayout1.xml
    zip.start_file("ppt/slideLayouts/slideLayout1.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(PPTX_SLIDE_LAYOUT.as_bytes()).map_err(|e| e.to_string())?;

    // ppt/slideLayouts/_rels/slideLayout1.xml.rels
    zip.start_file("ppt/slideLayouts/_rels/slideLayout1.xml.rels", opts).map_err(|e| e.to_string())?;
    zip.write_all(PPTX_SLIDE_LAYOUT_RELS.as_bytes()).map_err(|e| e.to_string())?;

    // ppt/presentation.xml
    let slide_id_list: String = slides.iter().enumerate()
        .map(|(i, _)| format!(r#"<p:sldId id="{}" r:id="rId{}"/>"#, 256 + i, i + 1))
        .collect::<Vec<_>>()
        .join("\n");
    let pres = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
  xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId100"/></p:sldMasterIdLst>
<p:sldSz cx="9144000" cy="5143500"/>
<p:notesSz cx="6858000" cy="9144000"/>
<p:sldIdLst>
{slide_id_list}
</p:sldIdLst>
</p:presentation>"#);
    zip.start_file("ppt/presentation.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(pres.as_bytes()).map_err(|e| e.to_string())?;

    // ppt/_rels/presentation.xml.rels
    let pres_rels_slides: String = slides.iter().enumerate()
        .map(|(i, _)| format!(
            r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{}.xml"/>"#,
            i + 1, i + 1
        ))
        .collect::<Vec<_>>()
        .join("\n");
    let pres_rels = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
{pres_rels_slides}
<Relationship Id="rId100" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
</Relationships>"#);
    zip.start_file("ppt/_rels/presentation.xml.rels", opts).map_err(|e| e.to_string())?;
    zip.write_all(pres_rels.as_bytes()).map_err(|e| e.to_string())?;

    // Generate each slide
    for (i, (slide_title, content_lines)) in slides.iter().enumerate() {
        let slide_num = i + 1;

        // Build content body paragraphs
        let body_paras: String = content_lines.iter().map(|line| {
            format!(
                r#"<a:p><a:r><a:t>{}</a:t></a:r></a:p>"#,
                xml_escape(line)
            )
        }).collect::<Vec<_>>().join("\n");

        let slide_xml = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<p:cSld>
<p:spTree>
  <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
  <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></a:xfrm></p:grpSpPr>
  <!-- Title -->
  <p:sp>
    <p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr>
    <p:spPr><a:xfrm><a:off x="457200" y="274638"/><a:ext cx="8229600" cy="1143000"/></a:xfrm></p:spPr>
    <p:txBody><a:bodyPr/><a:lstStyle/>
      <a:p><a:r><a:rPr lang="it-IT" dirty="0"/><a:t>{title_text}</a:t></a:r></a:p>
    </p:txBody>
  </p:sp>
  <!-- Content -->
  <p:sp>
    <p:nvSpPr><p:cNvPr id="3" name="Content"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph idx="1"/></p:nvPr></p:nvSpPr>
    <p:spPr><a:xfrm><a:off x="457200" y="1600200"/><a:ext cx="8229600" cy="3497262"/></a:xfrm></p:spPr>
    <p:txBody><a:bodyPr/><a:lstStyle/>
      {body_paras}
    </p:txBody>
  </p:sp>
</p:spTree>
</p:cSld>
</p:sld>"#,
            title_text = xml_escape(slide_title),
        );

        zip.start_file(format!("ppt/slides/slide{slide_num}.xml"), opts).map_err(|e| e.to_string())?;
        zip.write_all(slide_xml.as_bytes()).map_err(|e| e.to_string())?;

        // slide rels
        let slide_rels = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>"#);
        zip.start_file(format!("ppt/slides/_rels/slide{slide_num}.xml.rels"), opts).map_err(|e| e.to_string())?;
        zip.write_all(slide_rels.as_bytes()).map_err(|e| e.to_string())?;
    }

    zip.finish().map_err(|e| e.to_string())?;
    eprintln!("[office] pptx saved: {path}");
    Ok(path)
}

// ---------------------------------------------------------------------------
// PPTX static boilerplate
// ---------------------------------------------------------------------------

const PPTX_ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#;

const PPTX_THEME: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office Theme">
<a:themeElements>
<a:clrScheme name="Office">
  <a:dk1><a:sysClr lastClr="000000" val="windowText"/></a:dk1>
  <a:lt1><a:sysClr lastClr="ffffff" val="window"/></a:lt1>
  <a:dk2><a:srgbClr val="44546A"/></a:dk2>
  <a:lt2><a:srgbClr val="E7E6E6"/></a:lt2>
  <a:accent1><a:srgbClr val="4472C4"/></a:accent1>
  <a:accent2><a:srgbClr val="ED7D31"/></a:accent2>
  <a:accent3><a:srgbClr val="A9D18E"/></a:accent3>
  <a:accent4><a:srgbClr val="FF0000"/></a:accent4>
  <a:accent5><a:srgbClr val="FFC000"/></a:accent5>
  <a:accent6><a:srgbClr val="70AD47"/></a:accent6>
  <a:hlink><a:srgbClr val="0563C1"/></a:hlink>
  <a:folHlink><a:srgbClr val="954F72"/></a:folHlink>
</a:clrScheme>
<a:fontScheme name="Office">
  <a:majorFont><a:latin typeface="Calibri Light"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont>
  <a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont>
</a:fontScheme>
<a:fmtScheme name="Office"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst>
<a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst>
<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>
<a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst>
</a:fmtScheme>
</a:themeElements>
</a:theme>"#;

const PPTX_SLIDE_MASTER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
  xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg>
<p:spTree>
  <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
  <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></a:xfrm></p:grpSpPr>
</p:spTree></p:cSld>
<p:txStyles>
  <p:titleStyle><a:lstStyle><a:lvl1pPr><a:defRPr lang="it-IT" sz="4400" b="1"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mj-lt"/></a:defRPr></a:lvl1pPr></a:lstStyle></p:titleStyle>
  <p:bodyStyle><a:lstStyle><a:lvl1pPr><a:defRPr lang="it-IT" sz="2800"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/></a:defRPr></a:lvl1pPr></a:lstStyle></p:bodyStyle>
  <p:otherStyle><a:lstStyle/></p:otherStyle>
</p:txStyles>
<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst>
<p:hf sldNum="0" hdr="0" ftr="0" dt="0"/>
<p:txStyles><p:titleStyle><a:lstStyle/></p:titleStyle><p:bodyStyle><a:lstStyle/></p:bodyStyle><p:otherStyle><a:lstStyle/></p:otherStyle></p:txStyles>
</p:sldMaster>"#;

const PPTX_SLIDE_MASTER_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>"#;

const PPTX_SLIDE_LAYOUT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
  xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
  type="titleContent" preserve="1">
<p:cSld name="Title and Content">
<p:spTree>
  <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
  <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></a:xfrm></p:grpSpPr>
  <p:sp>
    <p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr>
    <p:spPr><a:xfrm><a:off x="457200" y="274638"/><a:ext cx="8229600" cy="1143000"/></a:xfrm></p:spPr>
    <p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody>
  </p:sp>
  <p:sp>
    <p:nvSpPr><p:cNvPr id="3" name="Content"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph idx="1"/></p:nvPr></p:nvSpPr>
    <p:spPr><a:xfrm><a:off x="457200" y="1600200"/><a:ext cx="8229600" cy="3497262"/></a:xfrm></p:spPr>
    <p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody>
  </p:sp>
</p:spTree>
</p:cSld>
</p:sldLayout>"#;

const PPTX_SLIDE_LAYOUT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>"#;

// ---------------------------------------------------------------------------
// XLSX generation
// ---------------------------------------------------------------------------

/// Convert column index (0-based) to Excel letter(s): 0→A, 25→Z, 26→AA
fn col_letter(mut idx: usize) -> String {
    let mut result = String::new();
    loop {
        result.insert(0, (b'A' + (idx % 26) as u8) as char);
        if idx < 26 {
            break;
        }
        idx = idx / 26 - 1;
    }
    result
}

/// Build a shared strings table and return (sharedStrings.xml content, index map).
fn build_shared_strings(headers: &[String], rows: &[Vec<String>]) -> (String, std::collections::HashMap<String, usize>) {
    let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut strings: Vec<String> = Vec::new();

    let mut register = |s: &str| {
        if !index.contains_key(s) {
            index.insert(s.to_string(), strings.len());
            strings.push(s.to_string());
        }
    };

    for h in headers {
        register(h);
    }
    for row in rows {
        for cell in row {
            // Only register as shared string if not numeric
            if cell.parse::<f64>().is_err() {
                register(cell);
            }
        }
    }

    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="{}" uniqueCount="{}">"#,
        strings.len(), strings.len()
    );
    for s in &strings {
        xml.push_str(&format!("<si><t>{}</t></si>", xml_escape(s)));
    }
    xml.push_str("</sst>");
    (xml, index)
}

/// Generate a .xlsx file. Returns the saved file path.
pub fn create_xlsx(
    title: &str,
    headers: &[String],
    rows: &[Vec<String>],
    filename: Option<&str>,
) -> Result<String, String> {
    eprintln!("[office] create_xlsx title={title:?} cols={} rows={}", headers.len(), rows.len());

    let stem = filename
        .map(|f| f.trim_end_matches(".xlsx").to_string())
        .unwrap_or_else(|| sanitize_filename(title));
    let dir = downloads_dir();
    let path = unique_path(&dir, &stem, "xlsx");

    let (shared_strings_xml, ss_index) = build_shared_strings(headers, rows);

    // Build sheet XML
    let mut sheet_xml = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData>"#);

    // Header row (row 1) with bold style (styleIndex=1)
    sheet_xml.push_str("<row r=\"1\">");
    for (ci, h) in headers.iter().enumerate() {
        let cell_ref = format!("{}1", col_letter(ci));
        let si = ss_index.get(h.as_str()).copied().unwrap_or(0);
        sheet_xml.push_str(&format!(
            "<c r=\"{cell_ref}\" t=\"s\" s=\"1\"><v>{si}</v></c>"
        ));
    }
    sheet_xml.push_str("</row>");

    // Data rows
    for (ri, row) in rows.iter().enumerate() {
        let row_num = ri + 2;
        sheet_xml.push_str(&format!("<row r=\"{row_num}\">"));
        for (ci, cell) in row.iter().enumerate() {
            let cell_ref = format!("{}{}", col_letter(ci), row_num);
            if let Ok(n) = cell.parse::<f64>() {
                // Numeric cell — no shared string
                sheet_xml.push_str(&format!("<c r=\"{cell_ref}\"><v>{n}</v></c>"));
            } else if cell.is_empty() {
                sheet_xml.push_str(&format!("<c r=\"{cell_ref}\"/>"));
            } else {
                let si = ss_index.get(cell.as_str()).copied().unwrap_or(0);
                sheet_xml.push_str(&format!("<c r=\"{cell_ref}\" t=\"s\"><v>{si}</v></c>"));
            }
        }
        sheet_xml.push_str("</row>");
    }

    sheet_xml.push_str("</sheetData></worksheet>");

    // workbook.xml
    let workbook_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<sheets>
<sheet name="{name}" sheetId="1" r:id="rId1"/>
</sheets>
</workbook>"#,
        name = xml_escape(title)
    );

    // styles.xml — minimal: style 0 = normal, style 1 = bold header
    let styles_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<fonts>
<font><sz val="11"/><name val="Calibri"/></font>
<font><b/><sz val="11"/><name val="Calibri"/></font>
</fonts>
<fills>
<fill><patternFill patternType="none"/></fill>
<fill><patternFill patternType="gray125"/></fill>
</fills>
<borders><border><left/><right/><top/><bottom/><diagonal/></border></borders>
<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>
<cellXfs>
<xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>
<xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0"><alignment horizontal="center"/></xf>
</cellXfs>
</styleSheet>"#;

    let file = std::fs::File::create(&path)
        .map_err(|e| format!("create xlsx file: {e}"))?;
    let mut zip = ZipWriter::new(file);
    let opts = FileOptions::default()
        .compression_method(CompressionMethod::Deflated);

    zip.start_file("[Content_Types].xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(XLSX_CONTENT_TYPES.as_bytes()).map_err(|e| e.to_string())?;

    zip.start_file("_rels/.rels", opts).map_err(|e| e.to_string())?;
    zip.write_all(XLSX_RELS.as_bytes()).map_err(|e| e.to_string())?;

    zip.start_file("xl/_rels/workbook.xml.rels", opts).map_err(|e| e.to_string())?;
    zip.write_all(XLSX_WORKBOOK_RELS.as_bytes()).map_err(|e| e.to_string())?;

    zip.start_file("xl/workbook.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(workbook_xml.as_bytes()).map_err(|e| e.to_string())?;

    zip.start_file("xl/styles.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(styles_xml.as_bytes()).map_err(|e| e.to_string())?;

    zip.start_file("xl/sharedStrings.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(shared_strings_xml.as_bytes()).map_err(|e| e.to_string())?;

    zip.start_file("xl/worksheets/sheet1.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(sheet_xml.as_bytes()).map_err(|e| e.to_string())?;

    zip.finish().map_err(|e| e.to_string())?;

    eprintln!("[office] xlsx saved: {path}");
    Ok(path)
}

// ---------------------------------------------------------------------------
// Static XML templates
// ---------------------------------------------------------------------------

const DOCX_CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>
</Types>"#;

const DOCX_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

const DOCX_DOCUMENT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>
</Relationships>"#;

const DOCX_STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:style w:type="paragraph" w:styleId="Normal"><w:name w:val="Normal"/>
<w:rPr><w:sz w:val="24"/><w:szCs w:val="24"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/>
<w:rPr><w:b/><w:sz w:val="52"/><w:szCs w:val="52"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/>
<w:rPr><w:b/><w:sz w:val="40"/><w:szCs w:val="40"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/>
<w:rPr><w:b/><w:sz w:val="32"/><w:szCs w:val="32"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/>
<w:rPr><w:b/><w:sz w:val="28"/><w:szCs w:val="28"/></w:rPr></w:style>
</w:styles>"#;

const DOCX_NUMBERING: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:abstractNum w:abstractNumId="0">
<w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/>
<w:lvlText w:val="•"/><w:lvlJc w:val="left"/>
<w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl>
</w:abstractNum>
<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
</w:numbering>"#;

const XLSX_CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
<Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>
<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
</Types>"#;

const XLSX_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#;

const XLSX_WORKBOOK_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/>
<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#;
