use alpheus_lib::analyze;
use alpheus_lib::dupes;
use alpheus_lib::exec;
use alpheus_lib::scan::{self, ActionKind, Card, Tier};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

#[test]
fn test_denylist_boundaries() {
    let h = scan::home();

    // Sensitive directories MUST be denied
    assert!(scan::is_denied(&h.join(".ssh")));
    assert!(scan::is_denied(&h.join(".ssh/id_rsa")));
    assert!(scan::is_denied(&h.join(".claude")));
    assert!(scan::is_denied(&h.join(".gnupg")));
    assert!(scan::is_denied(&h.join("Documents/prod")));
    assert!(scan::is_denied(&h.join("Documents/prod/subfolder")));

    // External root system paths MUST be denied (except allowlisted)
    assert!(scan::is_denied(Path::new("/etc")));
    assert!(scan::is_denied(Path::new("/usr")));
    assert!(scan::is_denied(Path::new("/bin")));
    assert!(scan::is_denied(Path::new("/home")));

    // Allowlisted system measurement directories MUST be allowed for inspection
    assert!(!scan::is_denied(Path::new("/var/cache/pacman/pkg")));
    assert!(!scan::is_denied(Path::new("/var/lib/systemd/coredump")));
}

#[test]
fn test_exec_dry_run_rules() {
    // 1. Dry run on an explain-only card must error
    let explain_card = Card {
        id: "os-update".into(),
        title: "Test Explain".into(),
        description: "Test".into(),
        tier: Tier::Manual,
        size_kb: 5000,
        paths: vec![],
        proof: None,
        action: ActionKind::Explain,
        command_display: None,
    };
    assert!(exec::dry_run(&explain_card).is_err());

    // 2. Dry run on a command card returns method: "command"
    let cmd_card = Card {
        id: "pacman-cache".into(),
        title: "Test Cmd".into(),
        description: "Test".into(),
        tier: Tier::Safe,
        size_kb: 5000,
        paths: vec![],
        proof: None,
        action: ActionKind::Command,
        command_display: Some("sudo paccache -rk2".into()),
    };
    let dr_cmd = exec::dry_run(&cmd_card).expect("command dry-run failed");
    assert_eq!(dr_cmd.method, "command");
    assert_eq!(dr_cmd.command, Some("sudo paccache -rk2".into()));
}

#[test]
fn test_directory_analyzer_computation() {
    let test_root = scan::home().join(format!(
        ".cache/alpheus_test_analyze_{}",
        std::process::id()
    ));
    let _ = fs::create_dir_all(&test_root);

    let sub_a = test_root.join("sub_a");
    let sub_b = test_root.join("sub_b");
    fs::create_dir_all(&sub_a).unwrap();
    fs::create_dir_all(&sub_b).unwrap();

    // Write 2 MB into sub_a
    let mut file_a = File::create(sub_a.join("data.bin")).unwrap();
    file_a.write_all(&vec![0u8; 2 * 1024 * 1024]).unwrap();

    // Write 1 MB into sub_b
    let mut file_b = File::create(sub_b.join("data.bin")).unwrap();
    file_b.write_all(&vec![0u8; 1 * 1024 * 1024]).unwrap();

    let analysis = analyze::analyze_directory(&test_root, 10);
    assert_eq!(analysis.entries.len(), 2);
    assert_eq!(analysis.entries[0].name, "sub_a");
    assert_eq!(analysis.entries[1].name, "sub_b");
    assert!(analysis.entries[0].size_kb > analysis.entries[1].size_kb);

    let _ = fs::remove_dir_all(test_root);
}

#[test]
fn test_duplicate_scanner_pipeline() {
    let test_root = scan::home().join(format!(".cache/alpheus_test_dupes_{}", std::process::id()));
    let _ = fs::create_dir_all(&test_root);

    // Create 3 identical 1.5 MB files
    let payload = vec![0x42u8; 1536 * 1024];
    fs::write(test_root.join("file1.dat"), &payload).unwrap();
    fs::write(test_root.join("file2.dat"), &payload).unwrap();
    fs::write(test_root.join("file3.dat"), &payload).unwrap();

    // Create 1 distinct 1.5 MB file
    let mut distinct = vec![0x42u8; 1536 * 1024];
    distinct[100] = 0x99;
    fs::write(test_root.join("file_distinct.dat"), &distinct).unwrap();

    let res = dupes::scan_duplicates(&test_root, 1024);
    assert_eq!(res.groups.len(), 1);
    assert_eq!(res.groups[0].duplicates.len(), 2); // 2 duplicates of 1 original
    assert_eq!(res.total_duplicate_files, 2);

    let _ = fs::remove_dir_all(test_root);
}

// ---------------------------------------------------------------------------
// iOS simulators
//
// Regression cover for the shipped bug: the card measured the whole
// CoreSimulator/Devices tree and offered `xcrun simctl delete unavailable` to
// reclaim it. The two never matched, and on a Mac with only the Command Line
// Tools `simctl` does not exist at all, so the action failed outright.
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod simulators_tests {
    use super::*;
    use alpheus_lib::simulators::{self, Remedy};

    /// The bug itself: without Xcode there is no `simctl`, so a simctl-based
    /// remedy must never be chosen. This is the assertion that would have
    /// caught the original failure.
    #[test]
    fn test_never_offers_simctl_without_xcode() {
        assert_eq!(simulators::remedy(false, false), Remedy::DeleteDirs);
        assert_eq!(simulators::remedy(false, true), Remedy::Blocked);

        // With Xcode present simctl owns the device set, booted or not:
        // an unavailable device is by definition not running.
        assert_eq!(simulators::remedy(true, false), Remedy::Simctl);
        assert_eq!(simulators::remedy(true, true), Remedy::Simctl);
    }

    /// `simctl()` must answer from the filesystem, never from an assumption.
    #[test]
    fn test_simctl_resolution_is_real() {
        match simulators::simctl() {
            Some(p) => assert!(p.is_file(), "simctl() returned a non-existent path"),
            None => {
                // Claiming absence is only allowed when it is genuinely absent.
                let probe = std::process::Command::new("/usr/bin/xcrun")
                    .args(["--find", "simctl"])
                    .output();
                if let Ok(o) = probe {
                    assert!(
                        !o.status.success(),
                        "simctl() reported None while xcrun can find it"
                    );
                }
            }
        }
    }

    /// Every device the scanner would delete must be a device directory inside
    /// the user's own Devices folder — never the tree root, never /Library.
    #[test]
    fn test_stranded_devices_are_scoped_and_deletable() {
        let devices_dir = simulators::devices_dir();
        for d in simulators::devices() {
            assert!(
                d.path.starts_with(&devices_dir),
                "device escaped the Devices folder: {}",
                d.path.display()
            );
            assert_ne!(d.path, devices_dir, "a device must not be the tree root");
            assert!(
                !scan::is_denied(&d.path),
                "device path is on the denylist: {}",
                d.path.display()
            );
            assert!(d.path.join("device.plist").is_file());
        }
    }

    /// When Xcode is gone the simctl card must fail with an instruction the
    /// user can act on — not the raw `xcrun: error: unable to find utility`
    /// text that the original version leaked into the UI.
    #[test]
    fn test_simctl_card_fails_readably_without_xcode() {
        if simulators::simctl().is_some() {
            return; // Xcode present: running the real command here is not a test.
        }
        let card = Card {
            id: "xcode-simulators".into(),
            title: "Stranded iOS simulators".into(),
            description: "Test".into(),
            tier: Tier::WithCare,
            size_kb: 1024,
            paths: vec![],
            proof: None,
            action: ActionKind::Command,
            command_display: Some("xcrun simctl delete unavailable".into()),
        };
        let err = exec::execute(&card).expect_err("must refuse without simctl");
        assert!(
            err.contains("Xcode is not installed"),
            "error must explain itself, got: {err}"
        );
        assert!(
            !err.contains("xcrun:"),
            "raw tool noise must not reach the user: {err}"
        );
    }

    /// Runtime images live under /Library and are root-owned. Alpheus measures
    /// them and must never be able to delete them.
    #[test]
    fn test_runtime_volumes_are_never_deletable() {
        for (path, kb) in simulators::runtime_volumes() {
            assert!(
                scan::is_denied(&path),
                "runtime image must stay undeletable: {}",
                path.display()
            );
            assert!(kb > 0, "a reported runtime image must have a measured size");
        }
    }
}

/// A command card's total must come from measured paths or not be quoted at
/// all. The old code echoed `card.size_kb` back as "space reclaimed", turning
/// an unrelated directory size into a promise about a command's yield.
#[test]
fn test_command_dry_run_never_invents_a_total() {
    let pathless = Card {
        id: "tm-snapshots".into(),
        title: "Snapshots".into(),
        description: "Test".into(),
        tier: Tier::WithCare,
        size_kb: 9_999_999, // a number the command has no claim to
        paths: vec![],
        proof: None,
        action: ActionKind::Command,
        command_display: Some("tmutil deletelocalsnapshots".into()),
    };
    let dr = exec::dry_run(&pathless).expect("dry-run failed");
    assert_eq!(
        dr.total_kb, 0,
        "a command with nothing to measure must report 0 (renders as 'unknown')"
    );
    assert!(dr.entries.is_empty());

    // With real paths, the total must equal what is actually on disk.
    let root = scan::home().join(format!(".cache/alpheus_test_cmd_{}", std::process::id()));
    let _ = fs::create_dir_all(&root);
    fs::write(root.join("blob.bin"), vec![0u8; 2 * 1024 * 1024]).unwrap();

    let with_paths = Card {
        id: "brew-cleanup".into(),
        title: "Measured".into(),
        description: "Test".into(),
        tier: Tier::Safe,
        size_kb: 9_999_999,
        paths: vec![root.to_string_lossy().to_string()],
        proof: None,
        action: ActionKind::Command,
        command_display: Some("brew cleanup".into()),
    };
    let dr = exec::dry_run(&with_paths).expect("dry-run failed");
    assert_ne!(dr.total_kb, 9_999_999, "total must be measured, not echoed");
    assert!(
        dr.total_kb >= 2000 && dr.total_kb < 9_999_999,
        "measured total was {} KB",
        dr.total_kb
    );
    assert_eq!(dr.entries.len(), 1);

    let _ = fs::remove_dir_all(root);
}

/// An unknown card id must be refused rather than shelled out to.
#[test]
fn test_command_allowlist_rejects_unknown_ids() {
    let rogue = Card {
        id: "rm -rf /".into(),
        title: "Rogue".into(),
        description: "Test".into(),
        tier: Tier::Safe,
        size_kb: 0,
        paths: vec![],
        proof: None,
        action: ActionKind::Command,
        command_display: Some("anything".into()),
    };
    let err = exec::execute(&rogue).expect_err("unknown command id must be refused");
    assert!(err.contains("no allowlisted command"), "got: {err}");
}
