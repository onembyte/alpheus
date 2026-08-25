//! High-speed directory analyzer and space hog detector.

use crate::scan::{self, du_many_kb};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Debug)]
pub struct HogEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size_kb: u64,
    pub percent: f64,
}

#[derive(Serialize, Clone, Debug)]
pub struct TopAnalysis {
    pub target: String,
    pub total_scanned_kb: u64,
    pub entries: Vec<HogEntry>,
}

/// Analyzes the immediate children of `target_dir` and lists the largest space consumers.
pub fn analyze_directory(target_dir: &Path, limit: usize) -> TopAnalysis {
    let mut dirs_to_measure: Vec<PathBuf> = vec![];
    let mut files_measured: Vec<(PathBuf, u64)> = vec![];

    if let Ok(rd) = fs::read_dir(target_dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if scan::is_denied(&path) {
                continue;
            }

            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    dirs_to_measure.push(path);
                } else if ft.is_file() {
                    if let Ok(meta) = entry.metadata() {
                        let kb = meta.len() / 1024;
                        if kb > 0 {
                            files_measured.push((path, kb));
                        }
                    }
                }
            }
        }
    }

    let dir_sizes = du_many_kb(&dirs_to_measure);

    let mut all_entries: Vec<(PathBuf, bool, u64)> = vec![];

    for d in dirs_to_measure {
        let size_kb = dir_sizes.get(&d).copied().unwrap_or(0);
        if size_kb > 0 {
            all_entries.push((d, true, size_kb));
        }
    }

    for (f, size_kb) in files_measured {
        all_entries.push((f, false, size_kb));
    }

    all_entries.sort_by_key(|e| std::cmp::Reverse(e.2));

    let total_scanned_kb: u64 = all_entries.iter().map(|e| e.2).sum();

    let entries: Vec<HogEntry> = all_entries
        .into_iter()
        .take(limit)
        .map(|(path, is_dir, size_kb)| {
            let percent = if total_scanned_kb > 0 {
                (size_kb as f64 / total_scanned_kb as f64) * 100.0
            } else {
                0.0
            };
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());

            HogEntry {
                path: path.to_string_lossy().to_string(),
                name,
                is_dir,
                size_kb,
                percent,
            }
        })
        .collect();

    TopAnalysis {
        target: target_dir.to_string_lossy().to_string(),
        total_scanned_kb,
        entries,
    }
}
