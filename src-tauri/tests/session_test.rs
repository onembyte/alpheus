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
