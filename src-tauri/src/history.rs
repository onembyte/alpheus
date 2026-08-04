//! Append-only log of every reclaim action ("freed 23.4 GB on Aug 3"),
//! stored as JSON in the app-data directory.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone)]
pub struct HistoryEntry {
    pub timestamp: u64,
    pub card_id: String,
    pub title: String,
    pub freed_kb: u64,
    pub method: String,
}

pub fn load(dir: &Path) -> Vec<HistoryEntry> {
    std::fs::read_to_string(dir.join("history.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn append(dir: &Path, entry: HistoryEntry) {
    let _ = std::fs::create_dir_all(dir);
    let mut entries = load(dir);
    entries.push(entry);
    if let Ok(json) = serde_json::to_string_pretty(&entries) {
        let _ = std::fs::write(dir.join("history.json"), json);
    }
}
