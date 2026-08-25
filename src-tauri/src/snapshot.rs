//! Disk growth tracking and snapshot diff engine.

use crate::scan::{du_many_kb, is_denied};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Snapshot {
    pub timestamp: u64,
    pub target: String,
    pub entries: HashMap<String, u64>,
}

#[derive(Serialize, Clone, Debug)]
pub struct DiffEntry {
    pub path: String,
    pub old_kb: u64,
    pub new_kb: u64,
    pub delta_kb: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct SnapshotDiff {
    pub target: String,
    pub old_timestamp: u64,
    pub new_timestamp: u64,
    pub net_growth_kb: i64,
    pub changes: Vec<DiffEntry>,
}

fn snapshots_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/share/alpheus/snapshots")
}

fn collect_snapshot_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = vec![];
    if let Ok(rd) = fs::read_dir(root) {
        for entry in rd.flatten() {
            let p = entry.path();
            if is_denied(&p) {
                continue;
            }
            if p.is_dir() {
                paths.push(p.clone());
                // also collect depth 2 for common subdirs like .cache/* and Documents/*
                if let Ok(sub_rd) = fs::read_dir(&p) {
                    for sub in sub_rd.flatten() {
                        let sp = sub.path();
                        if sp.is_dir() && !is_denied(&sp) {
                            paths.push(sp);
                        }
                    }
                }
            }
        }
    }
    paths
}

pub fn take_snapshot(target: &Path) -> Result<Snapshot, String> {
    let paths = collect_snapshot_paths(target);
    let sizes = du_many_kb(&paths);

    let mut entries = HashMap::new();
    for (p, kb) in sizes {
        if kb >= 1024 {
            // only track folders >= 1MB
            entries.insert(p.to_string_lossy().to_string(), kb);
        }
    }

    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let snap = Snapshot {
        timestamp,
        target: target.to_string_lossy().to_string(),
        entries,
    };

    let dir = snapshots_dir();
    let _ = fs::create_dir_all(&dir);
    let file = dir.join(format!("snapshot-{timestamp}.json"));
    let json = serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?;
    fs::write(file, json).map_err(|e| e.to_string())?;

    Ok(snap)
}

pub fn list_snapshots() -> Vec<Snapshot> {
    let dir = snapshots_dir();
    let mut list = vec![];
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&p) {
                    if let Ok(snap) = serde_json::from_str::<Snapshot>(&content) {
                        list.push(snap);
                    }
                }
            }
        }
    }
    list.sort_by_key(|s| s.timestamp);
    list
}

pub fn diff_latest_with_live(target: &Path) -> Result<SnapshotDiff, String> {
    let snapshots = list_snapshots();
    let old_snap = snapshots
        .iter()
        .filter(|s| s.target == target.to_string_lossy())
        .last()
        .cloned();

    let old_snap = match old_snap {
        Some(s) => s,
        None => {
            // If no previous snapshot exists, take one now as baseline
            let initial = take_snapshot(target)?;
            return Ok(SnapshotDiff {
                target: target.to_string_lossy().to_string(),
                old_timestamp: initial.timestamp,
                new_timestamp: initial.timestamp,
                net_growth_kb: 0,
                changes: vec![],
            });
        }
    };

    // Measure live current paths
    let live_paths = collect_snapshot_paths(target);
    let live_sizes = du_many_kb(&live_paths);

    let mut all_keys: HashMap<String, (u64, u64)> = HashMap::new();

    for (k, &old_kb) in &old_snap.entries {
        all_keys.insert(k.clone(), (old_kb, 0));
    }

    for (p, &new_kb) in &live_sizes {
        let key = p.to_string_lossy().to_string();
        if let Some(entry) = all_keys.get_mut(&key) {
            entry.1 = new_kb;
        } else if new_kb >= 1024 {
            all_keys.insert(key, (0, new_kb));
        }
    }

    let mut changes: Vec<DiffEntry> = vec![];
    let mut net_growth_kb = 0i64;

    for (path, (old_kb, new_kb)) in all_keys {
        let delta_kb = new_kb as i64 - old_kb as i64;
        if delta_kb.abs() >= 10 * 1024 {
            // >= 10MB change
            net_growth_kb += delta_kb;
            changes.push(DiffEntry {
                path,
                old_kb,
                new_kb,
                delta_kb,
            });
        }
    }

    changes.sort_by_key(|c| std::cmp::Reverse(c.delta_kb));

    let now_ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(SnapshotDiff {
        target: target.to_string_lossy().to_string(),
        old_timestamp: old_snap.timestamp,
        new_timestamp: now_ts,
        net_growth_kb,
        changes,
    })
}
