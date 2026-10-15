//! The walker sizes a folder tree the way `du -sk` does (allocated blocks,
//! each hard link once, symlinks never followed, one volume only), but
//! reports as it goes, so the storage detail view can grow its rows live.

use alpheus_lib::scan;
use alpheus_lib::walker::{self, Shared, Sink};
use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

/// Records every tracked folder and the bytes that landed in it directly.
struct Recorder {
    nodes: Mutex<Vec<(PathBuf, usize, u64)>>,
    max_depth: usize,
}

impl Recorder {
    fn new(root: &Path, max_depth: usize) -> Self {
        Recorder {
            nodes: Mutex::new(vec![(root.to_path_buf(), 0, 0)]),
            max_depth,
        }
    }
    /// KB recorded at or under `path`.
    fn total(&self, path: &Path) -> u64 {
        let n = self.nodes.lock().unwrap();
        n.iter()
            .filter(|(p, _, _)| p.starts_with(path))
            .map(|(_, _, bytes)| bytes)
            .sum::<u64>()
            .div_ceil(1024)
    }
    fn tracked(&self, path: &Path) -> bool {
        self.nodes.lock().unwrap().iter().any(|(p, _, _)| p == path)
    }
}

impl Sink for Recorder {
    type Node = usize;
    fn child(&self, parent: usize, name: &OsStr) -> Option<usize> {
        let mut n = self.nodes.lock().unwrap();
        let (path, depth, _) = n[parent].clone();
        if depth >= self.max_depth {
            return None;
        }
        n.push((path.join(name), depth + 1, 0));
        Some(n.len() - 1)
    }
    fn grow(&self, node: usize, bytes: u64) {
        self.nodes.lock().unwrap()[node].2 += bytes;
    }
}

fn fixture(name: &str) -> PathBuf {
    let root = scan::home().join(format!(
        ".cache/alpheus_test_walker_{name}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn write_kb(p: &Path, kb: usize) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, vec![7u8; kb * 1024]).unwrap();
}

fn du_kb(p: &Path) -> u64 {
    let out = Command::new("du").arg("-sk").arg(p).output().unwrap();
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn test_totals_match_du_at_every_tracked_level() {
    let root = fixture("du");
    write_kb(&root.join("top.bin"), 300);
    write_kb(&root.join("a/one.bin"), 1024);
    write_kb(&root.join("a/b/two.bin"), 512);
    write_kb(&root.join("a/b/c/three.bin"), 64);
    write_kb(&root.join("d/four.bin"), 7);

    let rec = Recorder::new(&root, 1);
    let shared = Shared::default();
    let out = walker::walk(&root, 0, &rec, &shared);

    assert!(!out.cancelled);
    assert_eq!(walker::kb(out.bytes), du_kb(&root), "walk total vs du");
    assert_eq!(
        rec.total(&root),
        walker::kb(out.bytes),
        "every byte lands in some node"
    );
    assert!(rec.tracked(&root.join("a")));
    assert!(
        !rec.tracked(&root.join("a/b")),
        "depth 2 is beyond the sink's limit"
    );
    assert_eq!(
        rec.total(&root.join("a")),
        du_kb(&root.join("a")),
        "untracked depths roll up into their tracked ancestor"
    );
    assert_eq!(shared.files.load(Ordering::Relaxed), 5);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_hard_links_count_once() {
    let root = fixture("hardlink");
    write_kb(&root.join("x/data.bin"), 2048);
    fs::create_dir_all(root.join("y")).unwrap();
    fs::hard_link(root.join("x/data.bin"), root.join("y/same.bin")).unwrap();

    let out = walker::walk(&root, 0, &Recorder::new(&root, 2), &Shared::default());
    assert_eq!(walker::kb(out.bytes), du_kb(&root));
    assert!(
        walker::kb(out.bytes) < 2 * 2048,
        "the second name must not double the size"
    );
    let _ = fs::remove_dir_all(root);
}

/// A link to a big file elsewhere, and a link back up the tree (a loop),
/// count as links: the walk neither grows by the target nor runs forever.
#[test]
fn test_symlinks_are_not_followed() {
    let root = fixture("symlink");
    let outside = fixture("symlink_target");
    write_kb(&outside.join("big.bin"), 4096);
    write_kb(&root.join("small.bin"), 8);
    symlink(outside.join("big.bin"), root.join("to-big")).unwrap();
    symlink(&root, root.join("loop")).unwrap();

    let out = walker::walk(&root, 0, &Recorder::new(&root, 3), &Shared::default());
    let kb = walker::kb(out.bytes);
    assert!(kb < 1024, "followed a symlink: {kb} KB");
    assert_eq!(kb, du_kb(&root));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

/// A folder macOS won't let us read is counted, not silently skipped.
#[test]
fn test_unreadable_folders_are_counted() {
    let root = fixture("unreadable");
    write_kb(&root.join("ok/file.bin"), 16);
    write_kb(&root.join("locked/secret.bin"), 16);
    fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();

    let shared = Shared::default();
    let out = walker::walk(&root, 0, &Recorder::new(&root, 1), &shared);
    fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(shared.unreadable.load(Ordering::Relaxed), 1);
    assert!(
        walker::kb(out.bytes) >= 16,
        "the readable part is still measured"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_cancel_stops_the_walk() {
    let root = fixture("cancel");
    for i in 0..20 {
        write_kb(&root.join(format!("d{i}/f.bin")), 4);
    }
    let shared = Shared::default();
    shared.cancel.store(true, Ordering::Relaxed);
    let rec = Recorder::new(&root, 1);
    let out = walker::walk(&root, 0, &rec, &shared);
    assert!(out.cancelled);
    assert!(
        rec.nodes.lock().unwrap().len() < 21,
        "a cancelled walk must not keep descending"
    );
    let _ = fs::remove_dir_all(root);
}

/// A volume mounted inside the tree (like the NAS share under /Volumes) is
/// another disk and must not be counted, let alone walked.
#[cfg(target_os = "macos")]
#[test]
fn test_stays_on_one_volume() {
    let root = fixture("volume");
    write_kb(&root.join("local.bin"), 64);
    let image = root.with_extension("dmg");
    let mnt = root.join("mnt");
    fs::create_dir_all(&mnt).unwrap();
    let made = Command::new("hdiutil")
        .args([
            "create", "-size", "4m", "-fs", "HFS+", "-volname", "t", "-quiet",
        ])
        .arg(&image)
        .status()
        .is_ok_and(|s| s.success())
        && Command::new("hdiutil")
            .args(["attach", "-nobrowse", "-quiet", "-mountpoint"])
            .arg(&mnt)
            .arg(&image)
            .status()
            .is_ok_and(|s| s.success());
    if !made {
        eprintln!("hdiutil unavailable; skipping");
        let _ = fs::remove_dir_all(&root);
        return;
    }
    write_kb(&mnt.join("remote.bin"), 2048);

    let out = walker::walk(&root, 0, &Recorder::new(&root, 2), &Shared::default());

    let _ = Command::new("hdiutil")
        .args(["detach", "-quiet"])
        .arg(&mnt)
        .status();
    let _ = fs::remove_file(&image);
    let _ = fs::remove_dir_all(&root);
    let kb = walker::kb(out.bytes);
    assert!(kb < 1024, "walked into the mounted volume: {kb} KB");
}

/// Manual check on a real tree: `ALPHEUS_BENCH_DIR=~/dev cargo test --release
/// --test walker_test real_tree -- --ignored --nocapture`. Same total as `du`
/// (within 0.1%, for files changing mid-walk) and the time each takes.
#[test]
#[ignore]
fn real_tree_matches_du() {
    let Ok(dir) = std::env::var("ALPHEUS_BENCH_DIR") else {
        eprintln!("set ALPHEUS_BENCH_DIR");
        return;
    };
    let dir = PathBuf::from(dir);
    let t = std::time::Instant::now();
    let shared = Shared::default();
    let out = walker::walk(&dir, 0, &Recorder::new(&dir, 2), &shared);
    let walk_time = t.elapsed();
    let t = std::time::Instant::now();
    let du = du_kb(&dir);
    let du_time = t.elapsed();
    let kb = walker::kb(out.bytes);
    eprintln!(
        "walker {kb} KB in {walk_time:?} ({} files, {} unreadable) · du {du} KB in {du_time:?}",
        shared.files.load(Ordering::Relaxed),
        shared.unreadable.load(Ordering::Relaxed)
    );
    assert!(kb.abs_diff(du) as f64 / (du.max(1) as f64) < 0.001);
}
