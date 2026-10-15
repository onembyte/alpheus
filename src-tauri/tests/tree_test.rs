//! The live tree behind the storage detail view, and the scheduler that fills
//! it: several walks at once, snapshots at a limited rate, totals that only grow.

use alpheus_lib::breakdown;
use alpheus_lib::scan;
use alpheus_lib::tree::{State, Tree, View};
use alpheus_lib::walker::{self, Shared};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn fixture(name: &str) -> PathBuf {
    let root = scan::home().join(format!(
        ".cache/alpheus_test_tree_{name}_{}",
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

fn find<'a>(v: &'a View, name: &str) -> &'a View {
    v.children
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no child {name} under {}", v.name))
}

/// apps: a 3 MB folder, b 2 MB, c 1 MB, plus five tiny folders.
fn sample(name: &str) -> PathBuf {
    let root = fixture(name);
    write_kb(&root.join("apps/a/big.bin"), 3072);
    write_kb(&root.join("apps/b/x/mid.bin"), 2048);
    write_kb(&root.join("apps/c/small.bin"), 1024);
    for i in 0..5 {
        write_kb(&root.join(format!("apps/tiny{i}/t.bin")), 4);
    }
    root
}

#[test]
fn test_walking_into_the_tree_gives_du_totals_at_every_level() {
    let root = sample("levels");
    let tree = Tree::new("Disk");
    let job = tree.add(Tree::ROOT, "Applications", 2);
    walker::walk(&root.join("apps"), job, &tree, &Shared::default());

    let view = tree.snapshot(Tree::ROOT, 3, 50);
    assert_eq!(walker::kb(view.bytes), du_kb(&root.join("apps")));
    let apps = find(&view, "Applications");
    assert_eq!(
        walker::kb(find(apps, "b").bytes),
        du_kb(&root.join("apps/b"))
    );
    assert_eq!(
        walker::kb(find(find(apps, "b"), "x").bytes),
        du_kb(&root.join("apps/b/x"))
    );
    // Each folder's row flips to done as its own walk ends, not the job's.
    assert!(find(apps, "b").state == State::Done);
    assert!(find(find(apps, "b"), "x").state == State::Done);
    let _ = fs::remove_dir_all(root);
}

/// Biggest first; past `top`, the rest folds into one "other" figure, so a
/// folder with 10,000 children doesn't send 10,000 rows to the screen.
#[test]
fn test_snapshot_is_sorted_and_capped() {
    let root = sample("capped");
    let tree = Tree::new("Disk");
    let job = tree.add(Tree::ROOT, "Applications", 2);
    walker::walk(&root.join("apps"), job, &tree, &Shared::default());

    let apps = tree.snapshot(job, 1, 3);
    let names: Vec<&str> = apps.children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "c"]);
    assert_eq!(apps.other_count, 5);
    let shown: u64 = apps.children.iter().map(|c| c.bytes).sum();
    assert!(apps.other_bytes > 0);
    assert!(apps.own_bytes + shown + apps.other_bytes == apps.bytes);
    let _ = fs::remove_dir_all(root);
}

/// Folders deeper than a job's tracking depth fold into their last tracked
/// ancestor, which keeps memory bounded on a whole-disk walk.
#[test]
fn test_tracking_depth_is_per_job() {
    let root = sample("depth");
    let tree = Tree::new("Disk");
    let job = tree.add(Tree::ROOT, "Applications", 1);
    walker::walk(&root.join("apps"), job, &tree, &Shared::default());

    let b = find(&tree.snapshot(job, 3, 50), "b").clone();
    assert!(
        b.children.is_empty(),
        "x is beyond depth 1 and must fold into b"
    );
    assert_eq!(walker::kb(b.bytes), du_kb(&root.join("apps/b")));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_scheduler_fills_every_job_and_snapshots_only_grow() {
    let root = sample("sched");
    write_kb(&root.join("docs/report.bin"), 512);
    write_kb(&root.join("dev/project/build.bin"), 1536);

    let tree = Arc::new(Tree::new("Disk"));
    let jobs: Vec<(PathBuf, usize)> = ["apps", "docs", "dev"]
        .iter()
        .map(|d| (root.join(d), tree.add(Tree::ROOT, d, 2)))
        .collect();
    for (_, id) in &jobs {
        assert!(tree.snapshot(*id, 0, 0).state == State::Pending);
    }

    let seen: Arc<Mutex<Vec<u64>>> = Arc::default();
    let log = seen.clone();
    let shared = Arc::new(Shared::default());
    breakdown::run(
        jobs,
        tree.clone(),
        shared,
        2,
        Duration::from_millis(1),
        move |t: &Tree| log.lock().unwrap().push(t.snapshot(Tree::ROOT, 0, 0).bytes),
    );

    let totals = seen.lock().unwrap().clone();
    assert!(!totals.is_empty(), "at least the final snapshot is sent");
    assert!(
        totals.windows(2).all(|w| w[0] <= w[1]),
        "a total shrank: {totals:?}"
    );
    let view = tree.snapshot(Tree::ROOT, 1, 50);
    assert!(view.children.iter().all(|c| c.state == State::Done));
    assert_eq!(
        walker::kb(view.bytes),
        du_kb(&root.join("apps")) + du_kb(&root.join("docs")) + du_kb(&root.join("dev"))
    );
    assert_eq!(
        *totals.last().unwrap(),
        view.bytes,
        "last snapshot is the final one"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_cancel_leaves_unstarted_jobs_pending() {
    let root = sample("cancelled");
    let tree = Arc::new(Tree::new("Disk"));
    let jobs: Vec<(PathBuf, usize)> = (0..4)
        .map(|i| (root.join("apps"), tree.add(Tree::ROOT, &format!("j{i}"), 2)))
        .collect();
    let shared = Arc::new(Shared::default());
    shared.cancel.store(true, Ordering::Relaxed);
    breakdown::run(
        jobs,
        tree.clone(),
        shared,
        2,
        Duration::from_millis(1),
        |_| {},
    );
    let view = tree.snapshot(Tree::ROOT, 1, 50);
    assert!(
        view.children.iter().all(|c| c.state == State::Pending),
        "nothing ran, so nothing may claim to be measured"
    );
    let _ = fs::remove_dir_all(root);
}
