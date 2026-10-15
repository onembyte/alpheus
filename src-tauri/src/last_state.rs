//! The last known state, kept on disk so Alpheus opens on it at once, days
//! later or after a restart, and corrects it in the background.
//!
//! One small JSON file in the app's data folder: the last reclaim cards, and
//! the last finished storage detail walk. They finish at different moments,
//! so each save replaces its own half and keeps the other. Writes go to a
//! temp file renamed into place, so a crash or power cut mid-write leaves the
//! previous state whole. A file that is damaged, or from another version, is
//! ignored rather than shown.

use crate::breakdown::Payload;
use crate::scan::Card;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const FILE: &str = "last-state.json";
/// Bumped whenever the file's shape changes, so an old file is ignored.
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct LastState {
    pub version: u32,
    pub cards: Option<Vec<Card>>,
    /// When the cards were scanned (Unix seconds).
    pub cards_at: Option<u64>,
    pub breakdown: Option<Payload>,
}

/// The reclaim scan and the walk save from different threads; one at a time,
/// so neither loses the other's half.
static WRITING: Mutex<()> = Mutex::new(());

/// The last known state; empty when there is none or it can't be trusted.
pub fn load(dir: &Path) -> LastState {
    fs::read_to_string(dir.join(FILE))
        .ok()
        .and_then(|json| serde_json::from_str::<LastState>(&json).ok())
        .filter(|s| s.version == VERSION)
        .unwrap_or_default()
}

pub fn save_cards(dir: &Path, cards: &[Card], at: u64) {
    update(dir, |s| {
        s.cards = Some(cards.to_vec());
        s.cards_at = Some(at);
    });
}

pub fn save_breakdown(dir: &Path, walk: &Payload) {
    update(dir, |s| s.breakdown = Some(walk.clone()));
}

fn update(dir: &Path, change: impl FnOnce(&mut LastState)) {
    let _one_at_a_time = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut state = load(dir);
    state.version = VERSION;
    change(&mut state);
    let Ok(json) = serde_json::to_string(&state) else {
        return;
    };
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    let tmp = temp_path(dir);
    if fs::write(&tmp, json).is_ok() && fs::rename(&tmp, dir.join(FILE)).is_err() {
        let _ = fs::remove_file(&tmp);
    }
}

/// Per process, so a second copy of the app can't write into this one's file.
fn temp_path(dir: &Path) -> PathBuf {
    dir.join(format!(".{FILE}.{}.tmp", std::process::id()))
}
