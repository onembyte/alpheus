//! A storage-detail session: one walk of the whole disk in the background,
//! with live payloads for the view, a focus the user can move while it runs,
//! cancel, and a cached copy of the last finished result for instant preview.

use alpheus_lib::breakdown::{self, Payload, Session};
use alpheus_lib::scan;
use alpheus_lib::tree::State;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn fixture(name: &str) -> PathBuf {
    let root = scan::home().join(format!(
        ".cache/alpheus_test_session_{name}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn mk(root: &Path, rel: &str, kb: usize) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, vec![1u8; kb * 1024]).unwrap();
}

fn data_volume(name: &str) -> PathBuf {
    let d = fixture(name);
    mk(&d, "Applications/Big.app/Contents/bin", 2048);
    mk(&d, "Users/me/Documents/a.pdf", 512);
    mk(&d, "Users/me/.cargo/registry/crate", 1024);
    mk(&d, "Library/Caches/x", 256);
    for i in 0..30 {
        mk(&d, &format!("Users/me/dev/p{i}/src/main.rs"), 16);
    }
    d
}

fn start(d: &Path, log: Arc<Mutex<Vec<Payload>>>) -> Session {
    Session::start(
        d.to_path_buf(),
        PathBuf::from("/Users/me"),
        None,
        Duration::from_millis(5),
        move |p| log.lock().unwrap().push(p),
    )
}

#[test]
fn test_session_walks_everything_and_reports_the_end() {
    let d = data_volume("full");
    let log: Arc<Mutex<Vec<Payload>>> = Arc::default();
    let s = start(&d, log.clone());
    s.wait();

    let last = log
        .lock()
        .unwrap()
        .last()
        .cloned()
        .expect("a final payload");
    assert!(!last.running, "the last payload says the walk ended");
    assert!(last.finished.is_some());
    assert!(last.root.state == State::Done);
    let names: Vec<&str> = last.root.children.iter().map(|c| c.name.as_str()).collect();
    for want in ["Applications", "Developer", "Documents", "System Data"] {
        assert!(names.contains(&want), "missing {want} in {names:?}");
    }
    assert!(last.files >= 34);
    let _ = fs::remove_dir_all(d);
}

/// The regular payload carries only the category rows; the rows under the
/// one the user opened travel in `focus`, so each update stays small.
#[test]
fn test_focus_brings_that_rows_children() {
    let d = data_volume("focus");
    let log: Arc<Mutex<Vec<Payload>>> = Arc::default();
    let s = start(&d, log.clone());
    s.wait();

    let p = s.payload();
    assert!(p.focus.is_none());
    assert!(
        p.root.children.iter().all(|c| c.children.is_empty()),
        "the root payload is one level deep"
    );
    let dev = p
        .root
        .children
        .iter()
        .find(|c| c.name == "Developer")
        .unwrap();
    s.set_focus(Some(dev.id));
    let focused = s.payload().focus.expect("focus payload");
    assert_eq!(focused.id, dev.id);
    assert!(focused.children.iter().any(|c| c.name == "dev"));
    s.set_focus(None);
    assert!(s.payload().focus.is_none());
    let _ = fs::remove_dir_all(d);
}

#[test]
fn test_cancel_ends_the_session() {
    let d = data_volume("cancel");
    let log: Arc<Mutex<Vec<Payload>>> = Arc::default();
    let s = start(&d, log.clone());
    s.cancel();
    s.wait();
    assert!(!s.payload().running);
    let _ = fs::remove_dir_all(d);
}

/// The finished result is saved, so the next launch can show it at once.
/// A damaged cache is ignored, never shown.
#[test]
fn test_cache_round_trip_and_damaged_cache() {
    let d = data_volume("cache");
    let dir = fixture("cache_dir");
    let log: Arc<Mutex<Vec<Payload>>> = Arc::default();
    let s = start(&d, log.clone());
    s.wait();

    breakdown::save_cache(&dir, &s.cached());
    let back = breakdown::load_cache(&dir).expect("cache reads back");
    assert_eq!(back.root.bytes, s.payload().root.bytes);
    assert!(
        back.root.children.iter().any(|c| !c.children.is_empty()),
        "the cached copy keeps a second level for the preview"
    );

    fs::write(dir.join("breakdown.json"), "{ broken").unwrap();
    assert!(breakdown::load_cache(&dir).is_none());
    assert!(breakdown::load_cache(&dir.join("missing")).is_none());
    let _ = fs::remove_dir_all(d);
    let _ = fs::remove_dir_all(dir);
}

/// The walk must yield to the app: its threads run below default priority.
#[cfg(target_os = "macos")]
#[test]
fn test_workers_run_at_lowered_qos() {
    use alpheus_lib::qos;
    std::thread::spawn(|| {
        assert!(!qos::is_lowered(), "a fresh thread starts at default QoS");
        qos::lower();
        assert!(qos::is_lowered());
    })
    .join()
    .unwrap();
}

/// Focus ids come from the webview; one the tree doesn't have must clear the
/// focus, never index past the end of the tree.
#[test]
fn test_unknown_focus_id_is_ignored() {
    let d = data_volume("badfocus");
    let s = start(&d, Arc::default());
    s.wait();
    s.set_focus(Some(usize::MAX));
    assert!(s.payload().focus.is_none());
    let _ = fs::remove_dir_all(d);
}

/// Gate A, run by hand: `cargo test --release --test session_test gate_a --
/// --ignored --nocapture`. Times the reclaim scan alone and while a whole-disk
/// walk runs (medians of three), and how long that walk takes at utility QoS.
#[cfg(target_os = "macos")]
#[test]
#[ignore]
fn gate_a() {
    use std::time::Instant;
    let time_scan = || {
        let t = Instant::now();
        let n = scan::scan_all().len();
        (t.elapsed(), n)
    };
    let median = |mut v: Vec<Duration>| {
        v.sort();
        v[v.len() / 2]
    };
    time_scan(); // warm the caches the way a second scan in the app would

    let alone: Vec<Duration> = (0..3).map(|_| time_scan().0).collect();

    let t = Instant::now();
    let s = Session::start(
        PathBuf::from("/System/Volumes/Data"),
        scan::home(),
        alpheus_lib::apfs::read_root(),
        Duration::from_millis(200),
        |_| {},
    );
    let mut during = vec![];
    while s.running() && during.len() < 3 {
        during.push(time_scan().0);
    }
    s.wait();
    let walk = t.elapsed();
    let p = s.payload();

    eprintln!(
        "reclaim scan alone:        {:?}  {alone:?}",
        median(alone.clone())
    );
    if during.is_empty() {
        eprintln!("walk ended before a scan could overlap it");
    } else {
        eprintln!(
            "reclaim scan during walk:  {:?}  {during:?}",
            median(during.clone())
        );
    }
    eprintln!(
        "whole-disk walk at utility QoS: {walk:?}, {} files, {} unreadable",
        p.files, p.unreadable
    );
}

/// The app starts a walk at launch and the window asks for one when it opens:
/// both must get the same walk, and only a finished walk is replaced.
#[test]
fn test_slot_keeps_one_walk_at_a_time() {
    use alpheus_lib::breakdown::Slot;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let d = data_volume("slot");
    let slot = Slot::default();
    let made = AtomicUsize::new(0);
    let make = || {
        made.fetch_add(1, Ordering::SeqCst);
        start(&d, Arc::default())
    };

    assert!(slot.current().is_none());
    let first = slot.ensure(make);
    let again = slot.ensure(make);
    assert!(
        Arc::ptr_eq(&first, &again) || !first.running(),
        "a running walk is reused"
    );
    first.wait();
    let before = made.load(Ordering::SeqCst);
    let next = slot.ensure(make);
    assert_eq!(
        made.load(Ordering::SeqCst),
        before + 1,
        "a finished walk is replaced"
    );
    assert!(!Arc::ptr_eq(&first, &next));
    assert!(Arc::ptr_eq(&slot.current().unwrap(), &next));
    next.wait();
    let _ = fs::remove_dir_all(d);
}

/// Opening a row past the kept depth measures its insides in the background,
/// and the rows arrive in later payloads.
#[test]
fn test_opening_past_the_kept_depth_measures_it() {
    let d = data_volume("deepen");
    mk(&d, "Users/me/Documents/a/b/c/d/deep.bin", 64);
    let s = start(&d, Arc::default());
    s.wait();

    let child = |p: &Payload, name: &str| {
        let parent = p.focus.as_ref().unwrap_or(&p.root);
        parent
            .children
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no {name} under {}", parent.name))
            .clone()
    };
    // Documents (category) → Documents (folder) → a → b → c, kept 3 deep.
    let mut p = s.payload();
    for name in ["Documents", "Documents", "a", "b"] {
        s.set_focus(Some(child(&p, name).id));
        p = s.payload();
    }
    let c = child(&p, "c");
    assert!(c.openable && c.child_count == 0, "d was folded into c");

    s.set_focus(Some(c.id));
    s.wait();
    let opened = s.payload().focus.expect("focus on c");
    assert!(opened.state == alpheus_lib::tree::State::Done);
    assert!(
        opened.children.iter().any(|x| x.name == "d"),
        "c's insides were measured"
    );
    assert_eq!(opened.bytes, c.bytes, "the total didn't move");
    let _ = fs::remove_dir_all(d);
}

/// Card paths are written the way the user sees them (/Users/…), while the
/// walk runs on the Data volume's own paths; locating has to match both.
#[test]
fn test_locate_maps_user_paths_onto_the_data_volume() {
    let d = data_volume("locate");
    let s = start(&d, Arc::default());
    s.wait();
    let chains = s.locate(&[
        PathBuf::from("/Users/me/.cargo/registry"),
        d.join("Users/me/Documents/a.pdf"),
        PathBuf::from("/nowhere/at/all"),
    ]);
    assert_eq!(chains.len(), 3);
    let p = s.payload();
    let developer = p
        .root
        .children
        .iter()
        .find(|c| c.name == "Developer")
        .unwrap();
    assert!(
        chains[0].contains(&developer.id),
        "the cargo path sits under Developer"
    );
    assert!(chains[0].len() >= 4, "root, category, .cargo, registry");
    assert!(!chains[1].is_empty());
    assert!(chains[2].is_empty());
    let _ = fs::remove_dir_all(d);
}
