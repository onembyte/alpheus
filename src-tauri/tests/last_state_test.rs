//! The last known state, kept on disk so Alpheus opens on it at once, days
//! later or after a restart, and corrects it in the background. One small
//! JSON file: the last reclaim cards and the storage detail.

use alpheus_lib::breakdown::{Payload, Session};
use alpheus_lib::last_state;
use alpheus_lib::scan::{self, ActionKind, Card, Tier};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

fn fixture(name: &str) -> PathBuf {
    let root = scan::home().join(format!(
        ".cache/alpheus_test_last_state_{name}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn card(id: &str, kb: u64) -> Card {
    Card {
        id: id.into(),
        title: id.into(),
        description: "test".into(),
        tier: Tier::Safe,
        size_kb: kb,
        paths: vec![format!("/tmp/{id}")],
        proof: None,
        action: ActionKind::Delete,
        command_display: None,
    }
}

/// A finished walk of a tiny Data volume, as the app would cache it.
fn walked(d: &Path) -> Payload {
    for (rel, kb) in [
        ("Users/me/Documents/a/b/c.bin", 64),
        ("Users/me/.cargo/registry/x", 32),
    ] {
        let p = d.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, vec![1u8; kb * 1024]).unwrap();
    }
    let s = Session::start(
        d.to_path_buf(),
        PathBuf::from("/Users/me"),
        None,
        Duration::from_millis(5),
        |_| {},
    );
    s.wait();
    s.cached()
}

#[test]
fn test_cards_and_walk_are_saved_apart_and_read_together() {
    let dir = fixture("both");
    let data = fixture("both_data");
    assert!(last_state::load(&dir).cards.is_none(), "nothing saved yet");

    last_state::save_cards(
        &dir,
        &[card("trash", 2048), card("pkg-caches", 512)],
        1_700_000_000,
    );
    let walk = walked(&data);
    last_state::save_breakdown(&dir, &walk);

    let back = last_state::load(&dir);
    let cards = back.cards.expect("cards kept when the walk was saved");
    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].id, "trash");
    assert_eq!(back.cards_at, Some(1_700_000_000));
    assert_eq!(
        back.breakdown.expect("walk kept").root.bytes,
        walk.root.bytes
    );

    // A later scan replaces the cards and leaves the walk alone.
    last_state::save_cards(&dir, &[card("trash", 10)], 1_700_000_100);
    let back = last_state::load(&dir);
    assert_eq!(back.cards.unwrap().len(), 1);
    assert!(back.breakdown.is_some());
    let _ = fs::remove_dir_all(dir);
    let _ = fs::remove_dir_all(data);
}

/// The storage detail is kept three levels deep: category, folder, and the
/// folders inside it, so the snapshot can stand in for most of the view.
#[test]
fn test_the_walk_is_kept_three_levels_deep() {
    let data = fixture("depth_data");
    let walk = walked(&data);
    let docs = walk
        .root
        .children
        .iter()
        .find(|c| c.name == "Documents")
        .expect("Documents category");
    let folder = docs
        .children
        .iter()
        .find(|c| c.name == "Documents")
        .unwrap();
    assert!(
        folder.children.iter().any(|c| c.name == "a"),
        "third level kept"
    );
    let _ = fs::remove_dir_all(data);
}

/// Damaged, foreign or future files are ignored, never shown; the next save
/// replaces them.
#[test]
fn test_a_file_it_cannot_trust_reads_as_empty() {
    let dir = fixture("damaged");
    let file = dir.join("last-state.json");
    for bad in [
        "{ broken",
        r#"{"version": 999, "cards": [], "cards_at": 1}"#,
        r#"[1, 2, 3]"#,
    ] {
        fs::write(&file, bad).unwrap();
        let s = last_state::load(&dir);
        assert!(s.cards.is_none() && s.breakdown.is_none(), "trusted: {bad}");
    }
    last_state::save_cards(&dir, &[card("trash", 1)], 5);
    assert!(last_state::load(&dir).cards.is_some(), "a save replaces it");
    assert!(last_state::load(&dir.join("missing")).cards.is_none());
    let _ = fs::remove_dir_all(dir);
}

/// The reclaim scan and the walk finish on different threads, sometimes at
/// once: neither save may lose the other's half, and no temp file is left.
#[test]
fn test_saves_from_two_threads_both_land() {
    let dir = Arc::new(fixture("race"));
    let data = fixture("race_data");
    let walk = Arc::new(walked(&data));
    let (d1, d2, w) = (dir.clone(), dir.clone(), walk.clone());
    let a = std::thread::spawn(move || {
        for i in 0..40 {
            last_state::save_cards(&d1, &[card("trash", i)], i);
        }
    });
    let b = std::thread::spawn(move || {
        for _ in 0..40 {
            last_state::save_breakdown(&d2, &w);
        }
    });
    a.join().unwrap();
    b.join().unwrap();
    let back = last_state::load(&dir);
    assert!(back.cards.is_some() && back.breakdown.is_some());
    let leftovers: Vec<_> = fs::read_dir(&*dir)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name() != "last-state.json")
        .collect();
    assert!(leftovers.is_empty(), "temp files left behind");
    let _ = fs::remove_dir_all(&*dir);
    let _ = fs::remove_dir_all(data);
}
