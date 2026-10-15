//! Superseded builds: old versions an updater left beside the one in use.
//!
//! The danger in these cards is offering the build that is running. Every test
//! here is about the markers that name the current build, and about refusing
//! to offer anything when no marker can be read.

use alpheus_lib::exec;
use alpheus_lib::scan::{self, ActionKind, Tier};
use alpheus_lib::superseded::{self, Builds};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

const HASH_A: &str = "c139e2731e42a0340acc3d34de2cbdb01237a27c";
const HASH_B: &str = "5b508d94493ed3344e38945ffca1b03b713ee401";

fn fixture(name: &str) -> PathBuf {
    let root = scan::home().join(format!(
        ".cache/alpheus_test_superseded_{name}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn launcher(build: &Path) -> String {
    format!(
        "#!/bin/sh\nrealApp=\"Autodesk Fusion\"\ndestfolder=\"{}\"\n\nSTREAMERPATH=\"$destfolder/$realApp.app\"\n",
        build.display()
    )
}

/// A Fusion `production` folder: two builds, an unrelated folder, and the
/// `Autodesk Fusion.app` link the updater points at the current build.
fn fusion_fixture(name: &str, linked: &str) -> PathBuf {
    let production = fixture(name).join("production");
    for d in [HASH_A, HASH_B, "not-a-build"] {
        fs::create_dir_all(production.join(d).join("Autodesk Fusion.app")).unwrap();
    }
    symlink(
        production.join(linked).join("Autodesk Fusion.app"),
        production.join("Autodesk Fusion.app"),
    )
    .unwrap();
    production
}

fn cleanup(production_or_root: &Path) {
    let root = production_or_root
        .ancestors()
        .find(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("alpheus_test_superseded_"))
        })
        .unwrap();
    let _ = fs::remove_dir_all(root);
}

// ---------------------------------------------------------------- policy

/// With no marker naming the current build there is no way to know which one
/// is safe, so nothing may be offered — not "everything but the newest".
#[test]
fn test_nothing_offered_without_a_current_marker() {
    let builds = Builds {
        all: vec![PathBuf::from("/x/a"), PathBuf::from("/x/b")],
        current: vec![],
    };
    assert!(superseded::superseded(&builds, "").is_empty());
}

/// A marker that resolves to no build on disk was misread (say, a link that
/// moved to a new layout). Then even the build it meant could be offered.
#[test]
fn test_marker_matching_no_build_offers_nothing() {
    let builds = Builds {
        all: vec![PathBuf::from("/x/a"), PathBuf::from("/x/b")],
        current: vec![PathBuf::from("/x")],
    };
    assert!(superseded::superseded(&builds, "").is_empty());
}

#[test]
fn test_current_and_running_builds_are_kept() {
    let builds = Builds {
        all: vec![
            PathBuf::from("/x/old"),
            PathBuf::from("/x/older"),
            PathBuf::from("/x/new"),
        ],
        current: vec![PathBuf::from("/x/new")],
    };
    let running = "/usr/sbin/cron\n/x/older/bin/daemon --serve\n";
    assert_eq!(
        superseded::superseded(&builds, running),
        vec![PathBuf::from("/x/old")]
    );
}

/// A running command line that merely shares a prefix (`/x/old-2`) must not
/// protect `/x/old`.
#[test]
fn test_running_match_is_a_whole_path_component() {
    let builds = Builds {
        all: vec![PathBuf::from("/x/old"), PathBuf::from("/x/new")],
        current: vec![PathBuf::from("/x/new")],
    };
    let running = "/x/old-2/bin/daemon\n";
    assert_eq!(
        superseded::superseded(&builds, running),
        vec![PathBuf::from("/x/old")]
    );
}

// ---------------------------------------------------------------- Fusion

#[test]
fn test_launcher_build_reads_destfolder() {
    let build = PathBuf::from(format!(
        "/Users/me/Library/Application Support/Autodesk/webdeploy/production/{HASH_A}"
    ));
    assert_eq!(superseded::launcher_build(&launcher(&build)), Some(build));
    assert_eq!(
        superseded::launcher_build("#!/bin/sh\nopen -a Fusion\n"),
        None
    );
    assert_eq!(superseded::launcher_build("destfolder=\"\"\n"), None);
}

#[test]
fn test_fusion_offers_only_the_unlinked_build() {
    let production = fusion_fixture("fusion", HASH_A);
    let script = launcher(&production.join(HASH_A));

    let builds = superseded::fusion_builds(&production, Some(&script));
    let names: Vec<String> = builds
        .all
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert_eq!(
        builds.all.len(),
        2,
        "only hash-named build folders: {names:?}"
    );
    assert!(!names.contains(&"not-a-build".to_string()));
    assert!(!names.contains(&"Autodesk Fusion.app".to_string()));

    let offered = superseded::superseded(&builds, "");
    assert_eq!(offered.len(), 1);
    assert!(offered[0].ends_with(HASH_B), "offered {offered:?}");
    cleanup(&production);
}

/// When the link and the launcher disagree, both builds are treated as current.
#[test]
fn test_fusion_disagreeing_markers_keep_both() {
    let production = fusion_fixture("fusion_disagree", HASH_A);
    let script = launcher(&production.join(HASH_B));
    let builds = superseded::fusion_builds(&production, Some(&script));
    assert!(superseded::superseded(&builds, "").is_empty());
    cleanup(&production);
}

/// No link and no launcher: Fusion's current build is unknowable.
#[test]
fn test_fusion_without_markers_offers_nothing() {
    let production = fusion_fixture("fusion_nomarker", HASH_A);
    fs::remove_file(production.join("Autodesk Fusion.app")).unwrap();
    let builds = superseded::fusion_builds(&production, None);
    assert!(superseded::superseded(&builds, "").is_empty());
    cleanup(&production);
}

// ---------------------------------------------------------------- Codex

fn codex_fixture(name: &str) -> PathBuf {
    let daemon = fixture(name).join("app-server-daemon");
    for v in ["0.160.0", "0.161.0", "0.162.1"] {
        fs::create_dir_all(daemon.join("releases").join(v).join("bin")).unwrap();
    }
    daemon
}

#[test]
fn test_codex_keeps_current_and_pending_update() {
    let daemon = codex_fixture("codex");
    symlink(daemon.join("releases/0.161.0"), daemon.join("current")).unwrap();
    // A newer release already downloaded and waiting to become current.
    fs::write(daemon.join("auto-update-version"), "0.162.1\n").unwrap();

    let builds = superseded::codex_releases(&daemon);
    assert_eq!(builds.all.len(), 3);
    let offered = superseded::superseded(&builds, "");
    assert_eq!(offered.len(), 1);
    assert!(offered[0].ends_with("0.160.0"), "offered {offered:?}");
    cleanup(&daemon);
}

#[test]
fn test_codex_without_current_link_offers_nothing() {
    let daemon = codex_fixture("codex_nolink");
    let builds = superseded::codex_releases(&daemon);
    assert!(superseded::superseded(&builds, "").is_empty());
    cleanup(&daemon);
}

// ---------------------------------------------------------------- cards

/// These cards wait for the user's review: never the safe tier (which
/// auto-clean may act on), and never on the direct-rm list, so even the 11 GB
/// Fusion build goes to the Trash where it can be put back.
#[test]
fn test_cards_need_review_and_go_to_trash() {
    for id in [
        "fusion-old-builds",
        "codex-old-releases",
        "diagnostic-reports",
    ] {
        assert!(
            !exec::RM_ALLOWED.contains(&id),
            "{id} must never skip the Trash"
        );
    }
    for card in superseded::scan() {
        assert!(card.tier == Tier::WithCare, "{} must need review", card.id);
        assert!(card.action == ActionKind::Delete, "{}", card.id);
        assert!(
            card.proof.is_some(),
            "{} must list what it removes",
            card.id
        );
        for p in &card.paths {
            assert!(!scan::is_denied(Path::new(p)), "denied path offered: {p}");
        }
    }
}

/// On this machine: whatever the live markers name as current must never be
/// among the paths a card offers.
#[test]
fn test_live_scan_never_offers_a_current_build() {
    let h = scan::home();
    let mut current: Vec<PathBuf> = vec![];
    let fusion = h.join("Library/Application Support/Autodesk/webdeploy/production");
    if let Ok(t) = fs::canonicalize(fusion.join("Autodesk Fusion.app")) {
        current.push(t.parent().unwrap().to_path_buf());
    }
    if let Ok(t) = fs::canonicalize(h.join(".codex/packages/app-server-daemon/current")) {
        current.push(t);
    }
    for card in superseded::scan() {
        for p in &card.paths {
            let p = fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p));
            assert!(
                !current.contains(&p),
                "{} offered the current build {}",
                card.id,
                p.display()
            );
        }
    }
}

// ---------------------------------------------------------------- diagnostics

/// A report still being written must not be offered. `sysdiagnosed` is a
/// daemon that never exits, so "is it running" says nothing; the files do.
#[test]
fn test_diagnostics_offer_only_settled_reports() {
    use std::time::{Duration, SystemTime};
    let root = fixture("diag");
    let now = SystemTime::now();

    let old = root.join("fba-fr-1-ded:OLD");
    fs::create_dir_all(old.join("sysdiagnose_x")).unwrap();
    let f = fs::File::create(old.join("sysdiagnose_x/log.txt")).unwrap();
    f.set_modified(now - Duration::from_secs(2 * 86400))
        .unwrap();

    // Old folder, but a file inside was written a minute ago.
    let busy = root.join("fba-fr-2-ded:BUSY");
    fs::create_dir_all(busy.join("sysdiagnose_y")).unwrap();
    let f = fs::File::create(busy.join("sysdiagnose_y/log.txt")).unwrap();
    f.set_modified(now - Duration::from_secs(60)).unwrap();

    let offered = superseded::diagnostic_reports(&root, now);
    assert_eq!(offered, vec![old]);
    cleanup(&root);
}
