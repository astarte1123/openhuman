// meetings/markdown.rs — YAML frontmatter read/write for meeting .md files.
//
// Format (Minutes-compatible):
//
//   ---
//   id: "uuid"
//   title: "Meeting title"
//   date: "2026-05-22T14:30:00Z"
//   duration_secs: 3600
//   attendees: ["Alice", "Bob"]
//   tags: ["sales", "q2"]
//   file_name: "2026-05-22-abcd1234.md"
//   ---
//
//   ## Trascrizione
//   ...
//
//   ## Riassunto AI
//   ...

use super::store::MeetingMeta;
use std::path::PathBuf;

pub fn parse_frontmatter(content: &str) -> Result<MeetingMeta, String> {
    let content = content.trim();
    if !content.starts_with("---") {
        return Err("no frontmatter".into());
    }
    let rest = &content[3..];
    let end = rest.find("\n---").ok_or("frontmatter not closed")?;
    let yaml = &rest[..end];

    let mut id = String::new();
    let mut title = String::new();
    let mut date = String::new();
    let mut duration_secs: u64 = 0;
    let mut attendees: Vec<String> = Vec::new();
    let mut tags: Vec<String> = Vec::new();
    let mut file_name = String::new();
    let mut template = String::new();
    let mut lang = String::new();

    for line in yaml.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("id:") {
            id = unquote(v);
        } else if let Some(v) = line.strip_prefix("title:") {
            title = unquote(v);
        } else if let Some(v) = line.strip_prefix("date:") {
            date = unquote(v);
        } else if let Some(v) = line.strip_prefix("duration_secs:") {
            duration_secs = v.trim().parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("attendees:") {
            attendees = parse_inline_list(v);
        } else if let Some(v) = line.strip_prefix("tags:") {
            tags = parse_inline_list(v);
        } else if let Some(v) = line.strip_prefix("file_name:") {
            file_name = unquote(v);
        } else if let Some(v) = line.strip_prefix("template:") {
            template = unquote(v);
        } else if let Some(v) = line.strip_prefix("lang:") {
            lang = unquote(v);
        }
    }

    if id.is_empty() {
        return Err("frontmatter missing id".into());
    }
    Ok(MeetingMeta { id, title, date, duration_secs, attendees, tags, file_name, template, lang })
}

pub fn extract_body(content: &str) -> String {
    let content = content.trim();
    if !content.starts_with("---") {
        return content.to_string();
    }
    let rest = &content[3..];
    if let Some(end) = rest.find("\n---") {
        rest[end + 4..].trim().to_string()
    } else {
        content.to_string()
    }
}

pub fn format_meeting(meta: &super::store::MeetingMeta, body: &str) -> String {
    let attendees_yaml = inline_list(&meta.attendees);
    let tags_yaml = inline_list(&meta.tags);
    // All string fields that appear inside YAML double-quoted scalars must be escaped,
    // including template and lang which come from user-editable settings.
    format!(
        "---\nid: \"{}\"\ntitle: \"{}\"\ndate: \"{}\"\nduration_secs: {}\nattendees: [{}]\ntags: [{}]\nfile_name: \"{}\"\ntemplate: \"{}\"\nlang: \"{}\"\n---\n\n{}",
        meta.id,
        escape_yaml(&meta.title),
        meta.date,            // internal RFC3339 — safe
        meta.duration_secs,   // u64 — safe
        attendees_yaml,
        tags_yaml,
        meta.file_name,       // derived from date+uuid prefix — safe
        escape_yaml(&meta.template),
        escape_yaml(&meta.lang),
        body
    )
}

pub fn write_meeting(
    dir: &std::path::Path,
    meta: &MeetingMeta,
    body: &str,
) -> Result<PathBuf, String> {
    // Derive file name from date + id prefix (handle_save already set meta.file_name,
    // but we recompute here as the authoritative path so both are always in sync).
    let date_prefix = meta.date.get(..10).unwrap_or("0000-00-00");
    let id_prefix = meta.id.get(..8).unwrap_or(&meta.id);
    let file_name = format!("{}-{}.md", date_prefix, id_prefix);

    // Use format_meeting so template and lang are always written to disk (fixes data-loss bug).
    let mut meta_with_name = meta.clone();
    meta_with_name.file_name = file_name.clone();
    let content = format_meeting(&meta_with_name, body);

    let path = dir.join(&file_name);
    std::fs::write(&path, &content).map_err(|e| format!("[meetings] write: {e}"))?;
    eprintln!("[meetings] wrote {}", path.display());
    Ok(path)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn unquote(s: &str) -> String {
    s.trim().trim_matches('"').trim_matches('\'').to_string()
}

fn parse_inline_list(s: &str) -> Vec<String> {
    let s = s.trim().trim_start_matches('[').trim_end_matches(']');
    s.split(',')
        .map(|p| p.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn inline_list(items: &[String]) -> String {
    items
        .iter()
        .map(|s| format!("\"{}\"", escape_yaml(s)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn escape_yaml(s: &str) -> String {
    // Inside YAML double-quoted scalars: \, ", and control chars must be escaped.
    s.replace('\\', "\\\\")
     .replace('"', "\\\"")
     .replace('\n', "\\n")
     .replace('\r', "\\r")
     .replace('\t', "\\t")
}
