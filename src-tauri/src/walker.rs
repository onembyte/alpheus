//! Sizes a folder tree like `du -sk`, reporting as it goes.
//!
//! It counts allocated blocks (what the disk really spends), each hard-linked
//! file once, never follows a symlink, and never leaves the volume it started
//! on: a mount point inside the tree (the NAS share under /Volumes) is another
//! disk. Sizes go into a [`Sink`] folder by folder, so a view can show totals
//! growing while the walk runs. Walks that share one [`Shared`] (hard-link
//! set, counters, cancel flag) may run in parallel.
//!
//! Sizes are bytes of allocated blocks; [`kb`] rounds up the way `du` does.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{self, Metadata};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::Mutex;

/// Where a walk reports. The sink owns the tree; the walker only measures.
pub trait Sink: Sync {
    type Node: Copy;
    /// A tracked node for the folder `name` inside `parent`, or `None` to
    /// fold that folder and everything below it into `parent`.
    fn child(&self, parent: Self::Node, name: &OsStr) -> Option<Self::Node>;
    /// `bytes` more found directly in `node`, or in folders folded into it.
    fn grow(&self, node: Self::Node, bytes: u64);
}

/// State shared by every walk of one scan.
#[derive(Default)]
pub struct Shared {
    pub cancel: AtomicBool,
    /// Folders that could not be listed (permissions, Full Disk Access).
    pub unreadable: AtomicU64,
    /// Everything that isn't a folder: files, symlinks, sockets.
    pub files: AtomicU64,
    /// (device, inode) of every multiply-linked file already counted.
    seen: Mutex<HashSet<(u64, u64)>>,
}

pub struct Outcome {
    pub bytes: u64,
    pub cancelled: bool,
}

/// KB as `du -k` prints it: rounded up.
pub fn kb(bytes: u64) -> u64 {
    bytes.div_ceil(1024)
}

/// Walks `root`, reporting into `node` and the tracked folders below it.
pub fn walk<S: Sink>(root: &Path, node: S::Node, sink: &S, shared: &Shared) -> Outcome {
    let bytes = match fs::symlink_metadata(root) {
        Ok(meta) if meta.is_dir() => folder(root, meta.dev(), node, true, &meta, sink, shared),
        Ok(meta) => {
            let b = other(&meta, shared);
            sink.grow(node, b);
            b
        }
        Err(_) => {
            shared.unreadable.fetch_add(1, Relaxed);
            0
        }
    };
    Outcome {
        bytes,
        cancelled: shared.cancel.load(Relaxed),
    }
}

/// Measures the folder at `path` and returns its total. Its own bytes go to
/// `node`; when `tracked`, `node` is this folder and subfolders may get
/// nodes of their own, otherwise everything below folds into `node`.
fn folder<S: Sink>(
    path: &Path,
    device: u64,
    node: S::Node,
    tracked: bool,
    meta: &Metadata,
    sink: &S,
    shared: &Shared,
) -> u64 {
    let mut here = allocated(meta);
    let mut below = 0;
    match fs::read_dir(path) {
        Err(_) => {
            shared.unreadable.fetch_add(1, Relaxed);
        }
        Ok(entries) => {
            for entry in entries.flatten() {
                if shared.cancel.load(Relaxed) {
                    break;
                }
                // Does not follow symlinks: a link is measured as a link.
                let Ok(m) = entry.metadata() else { continue };
                if !m.is_dir() {
                    here += other(&m, shared);
                } else if m.dev() == device {
                    let p = entry.path();
                    let child = if tracked {
                        sink.child(node, &entry.file_name())
                    } else {
                        None
                    };
                    below += match child {
                        Some(c) => folder(&p, device, c, true, &m, sink, shared),
                        None => folder(&p, device, node, false, &m, sink, shared),
                    };
                }
            }
        }
    }
    sink.grow(node, here);
    here + below
}

/// Bytes a non-folder entry adds: zero for a second name of a file already
/// counted.
fn other(meta: &Metadata, shared: &Shared) -> u64 {
    shared.files.fetch_add(1, Relaxed);
    if meta.nlink() > 1 && !shared.seen.lock().unwrap().insert((meta.dev(), meta.ino())) {
        return 0;
    }
    allocated(meta)
}

fn allocated(meta: &Metadata) -> u64 {
    meta.blocks() * 512
}
