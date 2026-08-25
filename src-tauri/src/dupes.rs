//! Multi-tier high-speed duplicate file scanner.

use crate::scan::is_denied;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Debug)]
pub struct DuplicateGroup {
    pub hash_prefix: String,
    pub file_size_kb: u64,
    pub wasted_kb: u64,
    pub original: String,
    pub duplicates: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct DuplicateScanResult {
    pub target: String,
    pub total_scanned_files: usize,
    pub total_duplicate_files: usize,
    pub total_wasted_kb: u64,
    pub groups: Vec<DuplicateGroup>,
}

fn file_header_hash(path: &Path) -> Option<[u8; 32]> {
    let mut f = File::open(path).ok()?;
    let mut buf = [0u8; 4096];
    let n = f.read(&mut buf).ok()?;
    if n == 0 {
        return None;
    }
    let mut hasher = Sha256::new();
    hasher.update(&buf[..n]);
    Some(hasher.finalize().into())
}

fn file_full_hash(path: &Path) -> Option<String> {
    let mut f = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

fn collect_files(dir: &Path, min_size_bytes: u64, list: &mut Vec<(PathBuf, u64)>) {
    if is_denied(dir) {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let path = entry.path();
        if is_denied(&path) {
            continue;
        }
        if let Ok(ft) = entry.file_type() {
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.') || name_str == "node_modules" || name_str == "target" || name_str == ".git" {
                    continue;
                }
                collect_files(&path, min_size_bytes, list);
            } else if ft.is_file() {
                if let Ok(meta) = entry.metadata() {
                    let len = meta.len();
                    if len >= min_size_bytes {
                        list.push((path, len));
                    }
                }
            }
        }
    }
}

pub fn scan_duplicates(root: &Path, min_size_kb: u64) -> DuplicateScanResult {
    let min_bytes = min_size_kb * 1024;
    let mut candidates: Vec<(PathBuf, u64)> = vec![];
    collect_files(root, min_bytes, &mut candidates);

    let total_scanned_files = candidates.len();

    // Stage 1: Group by size
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for (path, len) in candidates {
        by_size.entry(len).or_default().push(path);
    }

    let mut size_groups: Vec<(u64, Vec<PathBuf>)> = by_size
        .into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .collect();

    // Stage 2: Group by 4KB header hash
    let mut by_header: HashMap<([u8; 32], u64), Vec<PathBuf>> = HashMap::new();
    for (size, paths) in size_groups.drain(..) {
        for p in paths {
            if let Some(h) = file_header_hash(&p) {
                by_header.entry((h, size)).or_default().push(p);
            }
        }
    }

    let mut header_groups: Vec<(u64, Vec<PathBuf>)> = by_header
        .into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .map(|((_, size), paths)| (size, paths))
        .collect();

    // Stage 3: Full SHA-256 for exact match
    let mut by_full_hash: HashMap<(String, u64), Vec<PathBuf>> = HashMap::new();
    for (size, paths) in header_groups.drain(..) {
        for p in paths {
            if let Some(h) = file_full_hash(&p) {
                by_full_hash.entry((h, size)).or_default().push(p);
            }
        }
    }

    let mut groups: Vec<DuplicateGroup> = vec![];
    let mut total_wasted_kb = 0u64;
    let mut total_duplicate_files = 0usize;

    for ((hash, size_bytes), mut paths) in by_full_hash {
        if paths.len() <= 1 {
            continue;
        }

        // Sort by path length and alphabetical to consistently pick canonical original
        paths.sort();

        let original = paths.remove(0);
        let duplicates: Vec<String> = paths.iter().map(|p| p.to_string_lossy().to_string()).collect();
        let file_size_kb = size_bytes / 1024;
        let wasted_kb = (duplicates.len() as u64) * file_size_kb;

        total_wasted_kb += wasted_kb;
        total_duplicate_files += duplicates.len();

        let prefix = if hash.len() >= 8 {
            hash[..8].to_string()
        } else {
            hash
        };

        groups.push(DuplicateGroup {
            hash_prefix: prefix,
            file_size_kb,
            wasted_kb,
            original: original.to_string_lossy().to_string(),
            duplicates,
        });
    }

    groups.sort_by_key(|g| std::cmp::Reverse(g.wasted_kb));

    DuplicateScanResult {
        target: root.to_string_lossy().to_string(),
        total_scanned_files,
        total_duplicate_files,
        total_wasted_kb,
        groups,
    }
}
