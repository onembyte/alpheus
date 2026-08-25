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
    let test_root = scan::home().join(format!(".cache/alpheus_test_analyze_{}", std::process::id()));
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
