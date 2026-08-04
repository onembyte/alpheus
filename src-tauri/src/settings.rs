//! Persisted app settings (app-data dir, settings.json).

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct AppSettings {
    /// Seconds between automatic background scans; 0 disables them.
    pub auto_scan_secs: u64,
    /// Notify (and warn in the tray) when free space drops below this many GB.
    pub notify_below_gb: f64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_scan_secs: 0,
            notify_below_gb: 15.0,
        }
    }
}

pub fn load(dir: &Path) -> AppSettings {
    std::fs::read_to_string(dir.join("settings.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(dir: &Path, settings: &AppSettings) {
    let _ = std::fs::create_dir_all(dir);
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(dir.join("settings.json"), json);
    }
}
