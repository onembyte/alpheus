//! The only module allowed to remove anything — and it refuses to be creative.
//!
//! Hard rules, enforced here regardless of what the UI/CLI asks for:
//! - every path is re-checked against the denylist at execute time
//! - totals under 5 GB go to the Trash (reversible); direct `rm` is
//!   reserved for card ids on the known-regenerable allowlist
//! - command cards run a fixed argv keyed by card id — no frontend input ever
//!   reaches a shell

use crate::scan::{self, ActionKind, Card, Tier};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize)]
pub struct DryRunEntry {
    pub path: String,
    pub size_kb: u64,
}

#[derive(Serialize)]
pub struct DryRun {
    pub entries: Vec<DryRunEntry>,
    pub total_kb: u64,
    pub method: String, // "trash" | "delete" | "command"
    pub command: Option<String>,
    pub warning: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct ExecResult {
    pub freed_kb: u64,
    pub method: String,
    pub message: String,
}

const FIVE_GB_KB: u64 = 5 * 1024 * 1024;

/// Cards whose targets are known-regenerable and may be rm'd outright when the
/// total is ≥ 5 GB (HANDOFF rule). iphone-backups is deliberately absent —
/// backups always go through the Trash no matter the size.
const RM_ALLOWED: &[&str] = &[
    "node-modules-verified",
    "node-modules-unverified",
    "next-builds",
    "cargo-target",
    "py-cache",
    "yay-cache",
    "xdg-cache",
    "xcode-derived",
    "xcode-devicesupport",
    "library-caches",
    "pkg-caches",
    "colima",
    "spotify-cache",
    "claude-vm",
    "leftovers",
    "stale-downloads",
    "trash",
];

fn method_for(card: &Card, total_kb: u64) -> String {
    if card.id == "trash" {
        return "delete".into(); // emptying the Trash can't go to the Trash
    }
    if total_kb >= FIVE_GB_KB && RM_ALLOWED.contains(&card.id.as_str()) {
        "delete".into()
    } else {
        "trash".into()
    }
}

fn warning_for(card: &Card) -> Option<String> {
    match card.tier {
        Tier::WithCare => Some(
            "Needs a decision — read the list. Exactly what is shown here is removed, nothing else."
                .into(),
        ),
        _ => None,
    }
}

fn verified_paths(card: &Card) -> Result<Vec<PathBuf>, String> {
    let paths: Vec<PathBuf> = card
        .paths
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect();
    for p in &paths {
        if scan::is_denied(p) {
            return Err(format!("refusing: {} is on the denylist", p.display()));
        }
    }
    Ok(paths)
}

pub fn dry_run(card: &Card) -> Result<DryRun, String> {
    match card.action {
        ActionKind::Explain => Err("This card is informational — nothing to execute.".into()),
        ActionKind::Command => Ok(DryRun {
            entries: vec![],
            total_kb: card.size_kb,
            method: "command".into(),
            command: card.command_display.clone(),
            warning: warning_for(card),
        }),
        ActionKind::Delete => {
            let paths = verified_paths(card)?;
            let sizes = scan::du_many_kb(&paths);
            let mut entries: Vec<DryRunEntry> = paths
                .iter()
                .map(|p| DryRunEntry {
                    path: p.to_string_lossy().to_string(),
                    size_kb: sizes.get(p).copied().unwrap_or(0),
                })
                .collect();
            entries.sort_by_key(|e| std::cmp::Reverse(e.size_kb));
            let total_kb: u64 = entries.iter().map(|e| e.size_kb).sum();
            Ok(DryRun {
                method: method_for(card, total_kb),
                entries,
                total_kb,
                command: None,
                warning: warning_for(card),
            })
        }
    }
}

pub fn execute(card: &Card) -> Result<ExecResult, String> {
    match card.action {
        ActionKind::Explain => Err("This card is informational — nothing to execute.".into()),
        ActionKind::Command => run_command_card(card),
        ActionKind::Delete => {
            let paths = verified_paths(card)?;
            if paths.is_empty() {
                return Err("nothing left to remove — rescan first".into());
            }
            let sizes = scan::du_many_kb(&paths);
            let total_kb: u64 = sizes.values().sum();
            let method = method_for(card, total_kb);

            if card.id == "trash" {
                empty_trash_dirs(&paths)?;
                return Ok(ExecResult {
                    freed_kb: total_kb,
                    method,
                    message: format!("Emptied the Trash — {} freed.", fmt(total_kb)),
                });
            }

            if method == "delete" {
                for p in &paths {
                    // Make read-only files (e.g. Go modules) writable before removing
                    let _ = Command::new("chmod")
                        .args(["-R", "u+w"])
                        .arg(p)
                        .output();
                    remove_path(p)?;
                }
                Ok(ExecResult {
                    freed_kb: total_kb,
                    method,
                    message: format!(
                        "Deleted {} across {} locations.",
                        fmt(total_kb),
                        paths.len()
                    ),
                })
            } else {
                trash::delete_all(&paths).map_err(|e| e.to_string())?;
                Ok(ExecResult {
                    freed_kb: total_kb,
                    method,
                    message: format!(
                        "Moved {} to the Trash — empty it to actually free the space.",
                        fmt(total_kb)
                    ),
                })
            }
        }
    }
}

fn remove_path(p: &Path) -> Result<(), String> {
    let res = if p.is_dir() {
        std::fs::remove_dir_all(p)
    } else {
        std::fs::remove_file(p)
    };
    res.map_err(|e| format!("{}: {}", p.display(), e))
}

fn empty_trash_dirs(trash_dirs: &[PathBuf]) -> Result<(), String> {
    for dir in trash_dirs {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let _ = remove_path(&e.path());
            }
        }
        // Linux XDG trash subdirectories
        for sub in &["files", "info", "expunged"] {
            let sub_dir = dir.join(sub);
            if let Ok(rd) = std::fs::read_dir(&sub_dir) {
                for e in rd.flatten() {
                    let _ = remove_path(&e.path());
                }
            }
        }
    }
    Ok(())
}

/// Fixed, allowlisted commands only — the card id picks the argv, nothing from
/// the frontend reaches a shell.
fn run_command_card(card: &Card) -> Result<ExecResult, String> {
    let before = crate::disk::usage().free_kb;
    match card.id.as_str() {
        "pacman-cache" => {
            if Path::new("/usr/bin/paccache").exists() {
                run_ok(&["paccache", "-rk2"])?;
            } else {
                run_ok(&["pacman", "-Sc", "--noconfirm"])?;
            }
        }
        "journal-logs" => {
            run_ok(&["journalctl", "--vacuum-size=200M"])?;
        }
        "coredump-logs" => {
            if Path::new("/usr/bin/coredumpctl").exists() {
                run_ok(&["coredumpctl", "vacuum", "--size=50M"])?;
            }
        }
        "flatpak-unused" => {
            run_ok(&["flatpak", "uninstall", "--unused", "-y"])?;
        }
        "docker-prune" => {
            run_ok(&["docker", "system", "prune", "-f"])?;
        }
        "brew-cleanup" => {
            let brew = [
                "/opt/homebrew/bin/brew",
                "/usr/local/bin/brew",
                "/home/linuxbrew/.linuxbrew/bin/brew",
            ]
            .iter()
            .find(|p| Path::new(p).exists())
            .copied()
            .ok_or("brew not found")?;
            run_ok(&[brew, "cleanup", "--prune=all"])?;
        }
        "xcode-simulators" => {
            run_ok(&["/usr/bin/xcrun", "simctl", "delete", "unavailable"])?;
        }
        "tm-snapshots" => {
            let out = Command::new("tmutil")
                .args(["listlocalsnapshots", "/"])
                .output()
                .map_err(|e| e.to_string())?;
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                if let Some(date) = line
                    .trim()
                    .strip_prefix("com.apple.TimeMachine.")
                    .and_then(|s| s.strip_suffix(".local"))
                {
                    let _ = Command::new("tmutil")
                        .args(["deletelocalsnapshots", date])
                        .output();
                }
            }
        }
        other => return Err(format!("no allowlisted command for card {other}")),
    }
    let freed = crate::disk::usage().free_kb.saturating_sub(before);
    Ok(ExecResult {
        freed_kb: freed,
        method: "command".into(),
        message: format!("Done — {} freed.", fmt(freed)),
    })
}

fn run_ok(argv: &[&str]) -> Result<(), String> {
    let out = Command::new(argv[0])
        .args(&argv[1..])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn fmt(kb: u64) -> String {
    let gb = kb as f64 / (1024.0 * 1024.0);
    if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        format!("{:.0} MB", kb as f64 / 1024.0)
    }
}
