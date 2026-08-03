// meetings/store.rs — path management and meeting CRUD on the filesystem.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingMeta {
    pub id: String,
    pub title: String,
    pub date: String,         // ISO 8601 datetime
    pub duration_secs: u64,
    pub attendees: Vec<String>,
    pub tags: Vec<String>,
    pub file_name: String,
    #[serde(default)]
    pub template: String,     // "general" | "standup" | "one_on_one" | "retrospective" | "planning"
    #[serde(default)]
    pub lang: String,         // BCP-47 language tag e.g. "it-IT", "en-US"
}

pub fn meetings_dir() -> PathBuf {
    let dir = crate::data_dir();
    let path = std::path::Path::new(dir).join("meetings");
    let _ = std::fs::create_dir_all(&path);
    path
}

pub fn list_meetings() -> Result<Vec<MeetingMeta>, String> {
    let dir = meetings_dir();
    let mut metas: Vec<MeetingMeta> = Vec::new();

    let rd = std::fs::read_dir(&dir).map_err(|e| format!("meetings read_dir: {e}"))?;
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        match super::markdown::parse_frontmatter(&content) {
            Ok(meta) => metas.push(meta),
            Err(e) => eprintln!("[meetings] skip {}: {e}", path.display()),
        }
    }
    metas.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(metas)
}

pub fn get_meeting(id: &str) -> Result<(MeetingMeta, String), String> {
    let dir = meetings_dir();
    let rd = std::fs::read_dir(&dir).map_err(|e| format!("meetings read_dir: {e}"))?;
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        if let Ok(meta) = super::markdown::parse_frontmatter(&content) {
            if meta.id == id {
                let body = super::markdown::extract_body(&content);
                return Ok((meta, body));
            }
        }
    }
    Err(format!("[meetings] not found: {id}"))
}

pub fn delete_meeting(id: &str) -> Result<(), String> {
    let dir = meetings_dir();
    let rd = std::fs::read_dir(&dir).map_err(|e| format!("meetings read_dir: {e}"))?;
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        if let Ok(meta) = super::markdown::parse_frontmatter(&content) {
            if meta.id == id {
                std::fs::remove_file(&path)
                    .map_err(|e| format!("[meetings] delete: {e}"))?;
                eprintln!("[meetings] deleted id={id}");
                return Ok(());
            }
        }
    }
    Err(format!("[meetings] not found: {id}"))
}

/// Update title, tags, and/or attendees of an existing meeting.
pub fn update_meeting(
    id: &str,
    title: Option<&str>,
    tags: Option<Vec<String>>,
    attendees: Option<Vec<String>>,
) -> Result<MeetingMeta, String> {
    let dir = meetings_dir();
    let rd = std::fs::read_dir(&dir).map_err(|e| format!("meetings read_dir: {e}"))?;
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") { continue; }
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        if let Ok(mut meta) = super::markdown::parse_frontmatter(&content) {
            if meta.id == id {
                if let Some(t) = title { meta.title = t.to_string(); }
                if let Some(t) = tags { meta.tags = t; }
                if let Some(a) = attendees { meta.attendees = a; }
                let body = super::markdown::extract_body(&content);
                let new_content = super::markdown::format_meeting(&meta, &body);
                std::fs::write(&path, &new_content)
                    .map_err(|e| format!("[meetings] update write: {e}"))?;
                eprintln!("[meetings] updated id={id}");
                return Ok(meta);
            }
        }
    }
    Err(format!("[meetings] update: not found: {id}"))
}

/// Return (title, date, body_snippet) for the N most recent meetings.
/// Used to inject meeting context into the AI system prompt.
pub fn get_recent_summaries(limit: usize) -> Vec<(String, String, String)> {
    let dir = meetings_dir();
    let mut items: Vec<(String, String, String, String)> = Vec::new(); // (date, title, date_display, snippet)
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") { continue; }
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            if let Ok(meta) = super::markdown::parse_frontmatter(&content) {
                let body = super::markdown::extract_body(&content);
                let snippet: String = body.chars().take(300).collect();
                items.push((meta.date.clone(), meta.title, meta.date, snippet));
            }
        }
    }
    items.sort_by(|a, b| b.0.cmp(&a.0));
    items.truncate(limit);
    items.into_iter().map(|(_, title, date, snippet)| (title, date, snippet)).collect()
}

/// Update only the body of a meeting, preserving all metadata.
pub fn update_body(id: &str, new_body: &str) -> Result<MeetingMeta, String> {
    let dir = meetings_dir();
    let rd = std::fs::read_dir(&dir).map_err(|e| format!("meetings read_dir: {e}"))?;
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") { continue; }
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        if let Ok(meta) = super::markdown::parse_frontmatter(&content) {
            if meta.id == id {
                let new_content = super::markdown::format_meeting(&meta, new_body);
                std::fs::write(&path, &new_content)
                    .map_err(|e| format!("[meetings] update_body write: {e}"))?;
                eprintln!("[meetings] update_body id={id}");
                return Ok(meta);
            }
        }
    }
    Err(format!("[meetings] update_body: not found: {id}"))
}

pub fn search_meetings(query: &str) -> Result<Vec<MeetingMeta>, String> {
    let dir = meetings_dir();
    let q = query.to_lowercase();
    let mut results: Vec<MeetingMeta> = Vec::new();

    let rd = std::fs::read_dir(&dir).map_err(|e| format!("meetings read_dir: {e}"))?;
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        if content.to_lowercase().contains(&q) {
            if let Ok(meta) = super::markdown::parse_frontmatter(&content) {
                results.push(meta);
            }
        }
    }
    results.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(results)
}
