//! Categories like System Settings → Storage. Every entry of the Data volume
//! lands in exactly one category, so nothing is walked or counted twice.

use alpheus_lib::apfs::{Container, Volume};
use alpheus_lib::categories::{self, Category, Job};
use alpheus_lib::scan;
use alpheus_lib::tree::{State, Tree, View};
use alpheus_lib::walker::{self, Shared};
use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    let root = scan::home().join(format!(
        ".cache/alpheus_test_categories_{name}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn mk(root: &Path, rel: &str) {
    let p = root.join(rel);
    if rel.ends_with('/') {
        fs::create_dir_all(p).unwrap();
    } else {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, vec![1u8; 8192]).unwrap();
    }
}

/// A fake Data volume with one user, "me".
fn data_volume(name: &str) -> PathBuf {
    let d = fixture(name);
    for rel in [
        "Applications/Safari.app/Contents/x",
        "Applications/Utilities/Terminal.app/x",
        "Users/me/Applications/Local.app/x",
        "Users/me/Documents/report.pdf",
        "Users/me/Desktop/shot.png",
        "Users/me/Downloads/setup.dmg",
        "Users/me/Music/song.m4a",
        "Users/me/Pictures/Photos Library.photoslibrary/db",
        "Users/me/Library/Developer/Xcode/x",
        "Users/me/Library/Mail/V10/x",
        "Users/me/Library/Messages/chat.db",
        "Users/me/Library/Mobile Documents/com~apple~CloudDocs/x",
        "Users/me/Library/Application Support/App/x",
        "Users/me/Library/Caches/App/x",
        "Users/me/.cargo/registry/x",
        "Users/me/.Trash/old.zip",
        "Users/me/.zsh_history",
        "Users/me/dev/project/main.rs",
        "Users/me/notes.txt",
        "Users/Shared/x",
        "Users/other/x",
        "Library/Caches/x",
        "private/var/vm/swapfile0",
        "opt/homebrew/bin/x",
        "usr/local/bin/x",
        "Volumes/NAS/remote.bin",
        ".Spotlight-V100/store",
    ] {
        mk(&d, rel);
    }
    d
}

fn category_of(jobs: &[Job], d: &Path, rel: &str) -> Category {
    jobs.iter()
        .find(|j| j.path == d.join(rel))
        .unwrap_or_else(|| panic!("no job for {rel}"))
        .category
}

#[test]
fn test_every_entry_gets_the_settings_category() {
    let d = data_volume("classify");
    let jobs = categories::plan(&d, Path::new("/Users/me"));
    use Category::*;
    for (rel, want) in [
        ("Applications/Safari.app", Applications),
        ("Applications/Utilities", Applications),
        ("Users/me/Applications/Local.app", Applications),
        ("Users/me/Documents", Documents),
        ("Users/me/Desktop", Documents),
        ("Users/me/Downloads", Documents),
        ("Users/me/notes.txt", Documents),
        ("Users/me/Music", Music),
        ("Users/me/Pictures", Photos),
        ("Users/me/Library/Developer", Developer),
        ("Users/me/Library/Mail", Mail),
        ("Users/me/Library/Messages", Messages),
        ("Users/me/Library/Mobile Documents", ICloudDrive),
        ("Users/me/Library/Application Support", SystemData),
        ("Users/me/Library/Caches", SystemData),
        ("Users/me/.cargo", Developer),
        ("Users/me/.zsh_history", Developer),
        ("Users/me/dev", Developer),
        ("Users/me/.Trash", Trash),
        ("Users/Shared", OtherUsers),
        ("Users/other", OtherUsers),
        ("Library", SystemData),
        ("private", SystemData),
        (".Spotlight-V100", SystemData),
        ("opt", Developer),
        ("usr", Developer),
    ] {
        assert_eq!(category_of(&jobs, &d, rel), want, "{rel}");
    }
    let _ = fs::remove_dir_all(d);
}

/// Mounts live under /Volumes and are other disks; the folders that were
/// split up (/Users, the home folder, ~/Library, /Applications) must not
/// also be walked whole, or their contents would count twice.
#[test]
fn test_nothing_is_walked_twice() {
    let d = data_volume("twice");
    let jobs = categories::plan(&d, Path::new("/Users/me"));
    for split in [
        "Volumes",
        "Users",
        "Users/me",
        "Users/me/Library",
        "Users/me/Applications",
        "Applications",
    ] {
        assert!(
            !jobs.iter().any(|j| j.path == d.join(split)),
            "{split} is split up and must not be a job itself"
        );
    }
    for a in &jobs {
        for b in &jobs {
            assert!(
                a.path == b.path || !b.path.starts_with(&a.path),
                "{} is inside {}",
                b.path.display(),
                a.path.display()
            );
        }
    }
    let _ = fs::remove_dir_all(d);
}

fn container() -> Container {
    let v = |name: &str, role: &str, gb: u64| Volume {
        id: name.into(),
        name: name.into(),
        roles: vec![role.into()],
        used_kb: gb * 1024 * 1024,
    };
    Container {
        reference: "disk3".into(),
        capacity_kb: 245 * 1024 * 1024,
        free_kb: 30 * 1024 * 1024,
        volumes: vec![
            v("Macintosh HD", "System", 18),
            v("Preboot", "Preboot", 22),
            v("Recovery", "Recovery", 3),
            v("Data", "Data", 170),
        ],
    }
}

fn child<'a>(v: &'a View, name: &str) -> &'a View {
    v.children
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no {name} under {}", v.name))
}

/// Before anything is walked the view already adds up to the disk's used
/// space: macOS and the support volumes come from APFS, and everything not
/// measured yet sits in Unattributed, which shrinks as the walk proceeds.
#[test]
fn test_view_always_adds_up_to_used_space() {
    let d = data_volume("build");
    let tree = Tree::new("Macintosh HD");
    let c = container();
    let used: u64 = c.volumes.iter().map(|v| v.used_kb * 1024).sum();
    let jobs = categories::build(
        &tree,
        Some(&c),
        categories::plan(&d, Path::new("/Users/me")),
    );

    let before = tree.snapshot(Tree::ROOT, 2, 50);
    assert_eq!(before.bytes, used);
    let macos = child(&before, "macOS");
    assert_eq!(macos.bytes, 18 * 1024 * 1024 * 1024);
    assert!(
        macos.state == State::Done,
        "known from APFS, nothing to walk"
    );
    let sys = child(&before, "System Data");
    assert_eq!(child(sys, "Preboot").bytes, 22 * 1024 * 1024 * 1024);
    let unattributed_before = child(sys, "Unattributed").bytes;

    let shared = Shared::default();
    for (path, id) in &jobs {
        walker::walk(path, *id, &tree, &shared);
    }
    let after = tree.snapshot(Tree::ROOT, 2, 50);
    assert_eq!(after.bytes, used, "the total is the disk, measured or not");
    let unattributed_after = child(child(&after, "System Data"), "Unattributed").bytes;
    assert!(unattributed_after < unattributed_before);
    assert!(child(&after, "Developer").bytes > 0);

    // Categories come in the Settings order, whatever their sizes.
    let order: Vec<&str> = categories::ORDER.iter().map(|c| c.label()).collect();
    assert_eq!(order[0], "Applications");
    assert_eq!(*order.last().unwrap(), "System Data");
    let _ = fs::remove_dir_all(d);
}

/// Without APFS figures (Linux, or diskutil failing) there is no budget:
/// no macOS row, no Unattributed, just what was walked.
#[test]
fn test_without_apfs_only_walked_space_is_shown() {
    let d = data_volume("noapfs");
    let tree = Tree::new("Disk");
    let jobs = categories::build(&tree, None, categories::plan(&d, Path::new("/Users/me")));
    let shared = Shared::default();
    for (path, id) in &jobs {
        walker::walk(path, *id, &tree, &shared);
    }
    let v = tree.snapshot(Tree::ROOT, 2, 50);
    assert!(v.children.iter().all(|c| c.name != "macOS"));
    let walked: u64 = jobs
        .iter()
        .map(|(p, _)| walker::kb(walker::walk(p, Tree::ROOT, &NullSink, &Shared::default()).bytes))
        .sum();
    assert!(walker::kb(v.bytes).abs_diff(walked) <= jobs.len() as u64);
    let _ = fs::remove_dir_all(d);
}

struct NullSink;
impl walker::Sink for NullSink {
    type Node = usize;
    fn child(&self, _: usize, _: &std::ffi::OsStr) -> Option<usize> {
        None
    }
    fn grow(&self, _: usize, _: u64) {}
}

/// Manual check on this Mac: `cargo test --release --test categories_test
/// real_disk -- --ignored --nocapture`. Walks the whole Data volume with the
/// real plan and prints every category, the time taken, unreadable folders
/// and how many rows the tree holds.
#[cfg(target_os = "macos")]
#[test]
#[ignore]
fn real_disk_breakdown() {
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    let t = std::time::Instant::now();
    let container = alpheus_lib::apfs::read_root();
    let tree = Arc::new(Tree::new("Macintosh HD"));
    let jobs = categories::build(
        &tree,
        container.as_ref(),
        categories::plan(Path::new("/System/Volumes/Data"), &scan::home()),
    );
    let n_jobs = jobs.len();
    let shared = Arc::new(Shared::default());
    let snaps = std::sync::atomic::AtomicUsize::new(0);
    alpheus_lib::breakdown::run(
        jobs,
        tree.clone(),
        shared.clone(),
        3,
        std::time::Duration::from_millis(200),
        |_| {
            snaps.fetch_add(1, Ordering::Relaxed);
        },
    );
    let t_snap = std::time::Instant::now();
    let v = tree.snapshot(Tree::ROOT, 1, 50);
    eprintln!(
        "{n_jobs} jobs, {:?}, {} snapshots, snapshot takes {:?}, {} files, {} unreadable folders",
        t.elapsed(),
        snaps.load(Ordering::Relaxed),
        t_snap.elapsed(),
        shared.files.load(Ordering::Relaxed),
        shared.unreadable.load(Ordering::Relaxed)
    );
    for c in &v.children {
        eprintln!("{:>8.1} GB  {}", c.bytes as f64 / 1e9, c.name);
    }
    eprintln!("{:>8.1} GB  total", v.bytes as f64 / 1e9);
    let sys = v.children.iter().find(|c| c.name == "System Data").unwrap();
    for c in &tree.snapshot(sys.id, 1, 12).children {
        eprintln!(
            "   {:>8.1} GB  System Data / {}",
            c.bytes as f64 / 1e9,
            c.name
        );
    }
}

/// Linux keeps homes in /home, not /Users.
#[test]
fn test_linux_home_is_split_like_users() {
    let d = fixture("linux");
    for rel in [
        "home/me/Documents/a.txt",
        "home/me/.cargo/x",
        "home/other/x",
    ] {
        mk(&d, rel);
    }
    let jobs = categories::plan(&d, Path::new("/home/me"));
    assert_eq!(
        category_of(&jobs, &d, "home/me/Documents"),
        Category::Documents
    );
    assert_eq!(
        category_of(&jobs, &d, "home/me/.cargo"),
        Category::Developer
    );
    assert_eq!(category_of(&jobs, &d, "home/other"), Category::OtherUsers);
    let _ = fs::remove_dir_all(d);
}

/// A top-level entry on another device (a mount, Linux's /proc) is never a
/// job: the walker stays on the volume it starts on, so a job rooted there
/// would walk the other disk.
#[cfg(target_os = "macos")]
#[test]
fn test_top_level_mounts_are_skipped() {
    use std::process::Command;
    let d = fixture("mount");
    mk(&d, "Library/x");
    let image = d.with_extension("dmg");
    let mnt = d.join("mnt");
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
        let _ = fs::remove_dir_all(&d);
        return;
    }
    let jobs = categories::plan(&d, Path::new("/Users/me"));
    let _ = Command::new("hdiutil")
        .args(["detach", "-quiet"])
        .arg(&mnt)
        .status();
    let _ = fs::remove_file(&image);
    let _ = fs::remove_dir_all(&d);
    assert!(jobs.iter().any(|j| j.path == d.join("Library")));
    assert!(
        !jobs.iter().any(|j| j.path == mnt),
        "mounted volume became a job"
    );
}

/// Between walks the APFS figures move (a reclaim, an update download): the
/// rows that come from APFS, and the total, follow them without a walk.
#[test]
fn test_refresh_moves_the_apfs_rows_and_the_total() {
    let d = data_volume("refresh");
    let tree = Tree::new("Macintosh HD");
    let before = container();
    let jobs = categories::build(
        &tree,
        Some(&before),
        categories::plan(&d, Path::new("/Users/me")),
    );
    let shared = Shared::default();
    for (path, id) in &jobs {
        walker::walk(path, *id, &tree, &shared);
    }

    let mut after = container();
    after.volumes[0].used_kb = 19 * 1024 * 1024; // macOS grew
    after.volumes[1].used_kb = 8 * 1024 * 1024; // Preboot shrank after an update
    after.volumes[3].used_kb = 160 * 1024 * 1024; // Data lost 10 GB to a reclaim
    categories::refresh(&tree, &after);

    let v = tree.snapshot(Tree::ROOT, 2, 50);
    let used: u64 = after.volumes.iter().map(|v| v.used_kb * 1024).sum();
    assert_eq!(v.bytes, used);
    assert_eq!(child(&v, "macOS").bytes, 19 * 1024 * 1024 * 1024);
    assert_eq!(
        child(child(&v, "System Data"), "Preboot").bytes,
        8 * 1024 * 1024 * 1024
    );
    let _ = fs::remove_dir_all(d);
}
