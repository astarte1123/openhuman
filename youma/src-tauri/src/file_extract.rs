// file_extract.rs — Multi-format text extraction for knowledge base ingestion.
//
// Single entry point: `extract_text(path)` dispatches by file extension.
// Supported:
//   • PDF        (.pdf)                  — pdf-extract crate
//   • Word       (.docx)                 — ZIP + XML text stripping
//   • Excel      (.xlsx .xlsm .ods)      — ZIP + manual XML parsing (shared strings)
//   • HTML/XHTML (.html .htm .xhtml)     — tag stripping + entity decode
//   • EPUB       (.epub)                 — ZIP + HTML text stripping
//   • RTF        (.rtf)                  — control-word stripping
//   • Everything else                    — UTF-8 read (.txt .md .csv .json .yaml source…)

use std::io::Read;

/// Extract plain text from any supported file type.
pub fn extract_text(path: &str) -> Result<String, String> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    eprintln!("[file_extract] path={path} ext={ext}");

    match ext.as_str() {
        "pdf"                        => extract_pdf(path),
        "docx"                       => extract_docx(path),
        "xlsx" | "xlsm"              => extract_xlsx(path),
        "ods"                        => extract_ods(path),
        "html" | "htm" | "xhtml"     => extract_html(path),
        "epub"                       => extract_epub(path),
        "rtf"                        => extract_rtf(path),
        _ => {
            // UTF-8 text: .txt .md .csv .json .yaml .toml .rs .py .js .ts …
            std::fs::read_to_string(path)
                .map_err(|e| format!("[file_extract] read_to_string {path}: {e}"))
        }
    }
}

// ---------------------------------------------------------------------------
// PDF
// ---------------------------------------------------------------------------

fn extract_pdf(path: &str) -> Result<String, String> {
    pdf_extract::extract_text(path)
        .map_err(|e| format!("[file_extract] PDF {path}: {e}"))
}

// ---------------------------------------------------------------------------
// DOCX  (ZIP → word/document.xml → strip XML tags)
// ---------------------------------------------------------------------------

fn extract_docx(path: &str) -> Result<String, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("[file_extract] docx open {path}: {e}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("[file_extract] docx zip {path}: {e}"))?;

    let mut xml_buf = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|e| format!("[file_extract] docx no document.xml in {path}: {e}"))?
        .read_to_string(&mut xml_buf)
        .map_err(|e| format!("[file_extract] docx read xml {path}: {e}"))?;

    Ok(strip_xml_tags(&xml_buf))
}

// ---------------------------------------------------------------------------
// XLSX / XLSM  (ZIP → xl/sharedStrings.xml + xl/worksheets/sheetN.xml)
//
// Properly resolves shared-string references (t="s" cells) so string cell
// values appear as text rather than numeric indices.
// ---------------------------------------------------------------------------

fn extract_xlsx(path: &str) -> Result<String, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("[file_extract] xlsx open {path}: {e}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("[file_extract] xlsx zip {path}: {e}"))?;

    // 1. Shared strings table (may not exist if workbook has only numbers)
    let shared: Vec<String> = {
        let mut buf = String::new();
        if archive.by_name("xl/sharedStrings.xml").ok()
            .and_then(|mut e| e.read_to_string(&mut buf).ok()).is_some()
        {
            parse_shared_strings(&buf)
        } else {
            Vec::new()
        }
    };

    // 2. Collect sheet file names
    let sheet_names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            let name = archive.by_index(i).ok()?.name().to_string();
            if name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml") {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    // 3. Extract each sheet
    let mut out = String::new();
    for sheet_path in &sheet_names {
        let mut xml = String::new();
        if archive.by_name(sheet_path).ok()
            .and_then(|mut e| e.read_to_string(&mut xml).ok()).is_some()
        {
            let sheet_text = parse_xlsx_sheet(&xml, &shared);
            if !sheet_text.is_empty() {
                let label = sheet_path
                    .trim_start_matches("xl/worksheets/")
                    .trim_end_matches(".xml");
                out.push_str(&format!("# {label}\n{sheet_text}\n"));
            }
        }
    }

    if out.is_empty() {
        Err(format!("[file_extract] xlsx {path}: no sheet content found"))
    } else {
        Ok(out)
    }
}

/// Parse `xl/sharedStrings.xml` → ordered list of string values.
/// Each `<si>` element contains one or more `<t>` children; we join their text.
fn parse_shared_strings(xml: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut in_si = false;
    let mut in_t = false;
    let mut current_si = String::new();
    let mut current_t = String::new();
    let mut chars = xml.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            let tag: String = chars.by_ref().take_while(|c| *c != '>').collect();
            let tag_name = tag.split_whitespace().next().unwrap_or("").trim_start_matches('/');
            match tag_name {
                "si" if !tag.starts_with('/') => {
                    in_si = true;
                    current_si.clear();
                }
                "si" => {
                    result.push(current_si.trim().to_string());
                    in_si = false;
                }
                "t" if !tag.starts_with('/') && in_si => {
                    in_t = true;
                    current_t.clear();
                }
                "t" if in_si => {
                    current_si.push_str(&current_t);
                    in_t = false;
                }
                _ => {}
            }
        } else if in_t {
            current_t.push(ch);
        }
    }
    result
}

/// Parse a worksheet XML and produce tab-separated rows.
/// Cells with type `s` (shared string) are resolved; others use the `<v>` value.
fn parse_xlsx_sheet(xml: &str, shared: &[String]) -> String {
    let mut out = String::new();
    let mut in_row = false;
    let mut in_cell = false;
    let mut in_v = false;
    let mut cell_type = String::new();
    let mut cell_value = String::new();
    let mut row_cells: Vec<String> = Vec::new();
    let mut chars = xml.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            let tag: String = chars.by_ref().take_while(|c| *c != '>').collect();
            let is_close = tag.starts_with('/');
            let tag_body = tag.trim_start_matches('/');
            let tag_name = tag_body.split_whitespace().next().unwrap_or("");

            match tag_name {
                "row" if !is_close => { in_row = true; row_cells.clear(); }
                "row" if is_close => {
                    if !row_cells.is_empty() {
                        out.push_str(&row_cells.join("\t"));
                        out.push('\n');
                    }
                    in_row = false;
                }
                "c" if !is_close && in_row => {
                    in_cell = true;
                    cell_value.clear();
                    // Extract t="s" attribute from the tag
                    cell_type = if tag_body.contains("t=\"s\"") || tag_body.contains("t='s'") {
                        "s".to_string()
                    } else {
                        String::new()
                    };
                }
                "c" if is_close && in_row => {
                    let display = if cell_type == "s" {
                        cell_value.trim().parse::<usize>()
                            .ok()
                            .and_then(|i| shared.get(i))
                            .cloned()
                            .unwrap_or_else(|| cell_value.trim().to_string())
                    } else {
                        cell_value.trim().to_string()
                    };
                    row_cells.push(display);
                    in_cell = false;
                }
                "v" if !is_close && in_cell => { in_v = true; cell_value.clear(); }
                "v" if is_close && in_cell  => { in_v = false; }
                _ => {}
            }
        } else if in_v {
            cell_value.push(ch);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// ODS  (OpenDocument Spreadsheet — ZIP → content.xml → strip tags)
//
// ODS XML is more complex than XLSX; for RAG we just strip all tags and
// concatenate the visible text, which covers cell values and labels well.
// ---------------------------------------------------------------------------

fn extract_ods(path: &str) -> Result<String, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("[file_extract] ods open {path}: {e}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("[file_extract] ods zip {path}: {e}"))?;

    let mut xml = String::new();
    archive
        .by_name("content.xml")
        .map_err(|e| format!("[file_extract] ods no content.xml in {path}: {e}"))?
        .read_to_string(&mut xml)
        .map_err(|e| format!("[file_extract] ods read {path}: {e}"))?;

    Ok(strip_xml_tags(&xml))
}

// ---------------------------------------------------------------------------
// HTML / XHTML
// ---------------------------------------------------------------------------

fn extract_html(path: &str) -> Result<String, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("[file_extract] html read {path}: {e}"))?;
    Ok(strip_html_tags(&raw))
}

// ---------------------------------------------------------------------------
// EPUB  (ZIP → iterate *.html / *.xhtml entries → strip tags)
// ---------------------------------------------------------------------------

fn extract_epub(path: &str) -> Result<String, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("[file_extract] epub open {path}: {e}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("[file_extract] epub zip {path}: {e}"))?;

    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|e| e.name().to_string()))
        .filter(|n| n.ends_with(".html") || n.ends_with(".htm") || n.ends_with(".xhtml"))
        .collect();

    if names.is_empty() {
        return Err(format!("[file_extract] epub {path}: no HTML entries found"));
    }

    let mut text = String::new();
    for name in names {
        let mut buf = String::new();
        if archive.by_name(&name).ok()
            .and_then(|mut e| e.read_to_string(&mut buf).ok()).is_some()
        {
            text.push_str(&strip_html_tags(&buf));
            text.push('\n');
        }
    }
    Ok(text)
}

// ---------------------------------------------------------------------------
// RTF  (strip control words and group delimiters)
// ---------------------------------------------------------------------------

fn extract_rtf(path: &str) -> Result<String, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("[file_extract] rtf read {path}: {e}"))?;
    Ok(strip_rtf(&raw))
}

// ---------------------------------------------------------------------------
// Internal text helpers
// ---------------------------------------------------------------------------

fn strip_xml_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len() / 2);
    let mut in_tag = false;
    let mut last_space = false;

    for ch in s.chars() {
        match ch {
            '<' => {
                in_tag = true;
                if !last_space { out.push(' '); last_space = true; }
            }
            '>' => { in_tag = false; }
            _ if !in_tag => {
                if ch.is_whitespace() {
                    if !last_space { out.push(' '); last_space = true; }
                } else {
                    out.push(ch);
                    last_space = false;
                }
            }
            _ => {}
        }
    }
    out.trim().to_string()
}

fn strip_html_tags(s: &str) -> String {
    strip_xml_tags(s)
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

fn strip_rtf(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                match chars.peek() {
                    Some(&c) if c.is_alphabetic() => {
                        while chars.peek().map(|c| c.is_alphanumeric() || *c == '-').unwrap_or(false) {
                            chars.next();
                        }
                        if chars.peek() == Some(&' ') { chars.next(); }
                    }
                    Some(&'\'') => {
                        // \' HH — hex-encoded character, skip 3 chars total
                        chars.next(); chars.next(); chars.next();
                    }
                    Some(_) => { chars.next(); }
                    None => {}
                }
            }
            '{' | '}' => {}
            '\r' | '\n' => out.push('\n'),
            _ => out.push(ch),
        }
    }

    let mut result = String::new();
    let mut prev_blank = false;
    for line in out.lines() {
        let t = line.trim();
        if t.is_empty() {
            if !prev_blank { result.push('\n'); prev_blank = true; }
        } else {
            result.push_str(t);
            result.push('\n');
            prev_blank = false;
        }
    }
    result
}
