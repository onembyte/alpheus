//! iOS Simulator accounting — the one category where the obvious command lies.
//!
//! `xcrun simctl delete unavailable` removes only devices whose *runtime* is
//! missing. It is not a way to reclaim the Devices tree, and it does not exist
//! at all unless full Xcode is installed: `simctl` ships inside `Xcode.app`,
//! never in the Command Line Tools. Offering it against the measured size of
//! the whole tree promised space it could never deliver, and on a
//! CommandLineTools-only Mac it failed outright.
//!
//! This module measures what is actually on disk and classifies each device, so
//! the scanner can offer a remedy that matches the machine it is running on.

#![cfg(target_os = "macos")]

use crate::scan::{du_many_kb, home};
use std::path::{Path, PathBuf};
use std::process::Command;

/// One simulator device directory under `CoreSimulator/Devices/<UDID>`.
pub struct Device {
    pub udid: String,
    pub name: String,
    pub runtime: String,
    pub size_kb: u64,
    pub path: PathBuf,
    /// Its runtime is gone (or unknowable because Xcode is not installed), so
    /// the directory is dead weight that nothing can boot.
    pub stranded: bool,
    /// Currently booted — never a deletion candidate.
    pub booted: bool,
}

pub fn devices_dir() -> PathBuf {
    home().join("Library/Developer/CoreSimulator/Devices")
}

/// Absolute path to a *working* `simctl`, or `None`.
///
/// Resolved through `xcrun --find` rather than assumed, because the answer
/// depends on what `xcode-select` points at and changes when Xcode is
/// installed or removed. Re-resolved at execute time, never cached.
pub fn simctl() -> Option<PathBuf> {
    let out = Command::new("/usr/bin/xcrun")
        .args(["--find", "simctl"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let p = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

/// True while a simulator is genuinely live — Simulator.app is open, or a
/// device is booted (`launchd_sim` is spawned once per booted device).
///
/// Deliberately does NOT test for `CoreSimulatorService`. That XPC daemon lives
/// in a system framework that outlives Xcode itself and is woken lazily by
/// anything that so much as reads the CoreSimulator framework — including this
/// scanner. Gating on it suppressed the card on an idle machine.
pub fn simulator_booted() -> bool {
    ["Simulator", "launchd_sim"].iter().any(|proc_name| {
        Command::new("/usr/bin/pgrep")
            .arg("-x")
            .arg(proc_name)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// What Alpheus may legitimately offer for stranded devices on this machine.
///
/// Kept as a pure decision so the rule can be tested without a Mac in any
/// particular state — this is the exact logic that shipped broken.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Remedy {
    /// Full Xcode is present: let `simctl` own the device set.
    Simctl,
    /// No `simctl`, so no Xcode — the directories are inert and can be removed
    /// directly, like any other folder in $HOME.
    DeleteDirs,
    /// A simulator is live and there is no tool to coordinate with it. Report
    /// the space, offer nothing.
    Blocked,
}

/// `simctl delete unavailable` stays safe even while a simulator is booted —
/// an unavailable device is by definition one that cannot be running — so a
/// working `simctl` outranks the booted check. Without it, a live host means
/// hands off.
pub fn remedy(have_simctl: bool, booted: bool) -> Remedy {
    if have_simctl {
        Remedy::Simctl
    } else if booted {
        Remedy::Blocked
    } else {
        Remedy::DeleteDirs
    }
}

/// A plist (binary or XML) decoded to JSON via the system `plutil`.
fn plist_json(p: &Path) -> Option<serde_json::Value> {
    let out = Command::new("/usr/bin/plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(p)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice(&out.stdout).ok()
}

/// `com.apple.CoreSimulator.SimRuntime.iOS-27-0` → `iOS 27.0`
fn pretty_runtime(identifier: &str) -> String {
    let tail = identifier.rsplit('.').next().unwrap_or(identifier);
    let mut parts = tail.split('-');
    match parts.next() {
        Some(os) => {
            let version: Vec<&str> = parts.collect();
            if version.is_empty() {
                os.to_string()
            } else {
                format!("{os} {}", version.join("."))
            }
        }
        None => identifier.to_string(),
    }
}

/// What `simctl` knows: UDID → (is_available, is_booted).
///
/// An empty map means simctl could not be consulted; callers must treat that as
/// "unknown", never as "everything is fine".
fn simctl_state(simctl: &Path) -> std::collections::HashMap<String, (bool, bool)> {
    let mut map = std::collections::HashMap::new();
    let Ok(out) = Command::new(simctl)
        .args(["list", "devices", "--json"])
        .output()
    else {
        return map;
    };
    if !out.status.success() {
        return map;
    }
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return map;
    };
    let Some(runtimes) = v.get("devices").and_then(|d| d.as_object()) else {
        return map;
    };
    for list in runtimes.values() {
        for dev in list.as_array().into_iter().flatten() {
            let Some(udid) = dev.get("udid").and_then(|u| u.as_str()) else {
                continue;
            };
            let available = dev
                .get("isAvailable")
                .and_then(|a| a.as_bool())
                .unwrap_or(false);
            let booted = dev
                .get("state")
                .and_then(|s| s.as_str())
                .map(|s| s.eq_ignore_ascii_case("Booted"))
                .unwrap_or(false);
            map.insert(udid.to_string(), (available, booted));
        }
    }
    map
}

/// Every device directory on disk, sized and classified.
///
/// Classification is driven by what is actually installed: with no `simctl`
/// there is no Xcode, so nothing can boot any of them and all of them are
/// stranded.
pub fn devices() -> Vec<Device> {
    let dir = devices_dir();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return vec![];
    };

    let mut found: Vec<(String, PathBuf)> = vec![];
    for e in rd.flatten() {
        let path = e.path();
        if !path.is_dir() || !path.join("device.plist").is_file() {
            continue;
        }
        let udid = e.file_name().to_string_lossy().to_string();
        found.push((udid, path));
    }
    if found.is_empty() {
        return vec![];
    }

    let paths: Vec<PathBuf> = found.iter().map(|(_, p)| p.clone()).collect();
    let sizes = du_many_kb(&paths);

    let simctl_bin = simctl();
    let state = simctl_bin
        .as_ref()
        .map(|s| simctl_state(s))
        .unwrap_or_default();
    let have_simctl = simctl_bin.is_some();

    let mut out: Vec<Device> = found
        .into_iter()
        .map(|(udid, path)| {
            let plist = plist_json(&path.join("device.plist"));
            let name = plist
                .as_ref()
                .and_then(|v| v.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("Unnamed device")
                .to_string();
            let runtime = plist
                .as_ref()
                .and_then(|v| v.get("runtime"))
                .and_then(|r| r.as_str())
                .map(pretty_runtime)
                .unwrap_or_else(|| "unknown runtime".into());

            // No simctl → no Xcode → nothing can boot it. Known to simctl →
            // trust its verdict. Unknown to a working simctl → also stranded.
            let (available, booted) = match state.get(&udid) {
                Some(&(a, b)) => (a, b),
                None => (false, false),
            };
            let stranded = !have_simctl || !available;

            Device {
                size_kb: sizes.get(&path).copied().unwrap_or(0),
                udid,
                name,
                runtime,
                path,
                stranded,
                booted,
            }
        })
        .collect();

    out.sort_by_key(|d| std::cmp::Reverse(d.size_kb));
    out
}

/// Installed runtime images under `/Library` — measurable, but root-owned, so
/// Alpheus reports them and never touches them.
pub fn runtime_volumes() -> Vec<(PathBuf, u64)> {
    let mut found: Vec<PathBuf> = vec![];
    for root in [
        "/Library/Developer/CoreSimulator/Volumes",
        "/Library/Developer/CoreSimulator/Profiles/Runtimes",
    ] {
        if let Ok(rd) = std::fs::read_dir(root) {
            for e in rd.flatten() {
                found.push(e.path());
            }
        }
    }
    if found.is_empty() {
        return vec![];
    }
    let sizes = du_many_kb(&found);
    let mut out: Vec<(PathBuf, u64)> = found
        .into_iter()
        .map(|p| {
            let kb = sizes.get(&p).copied().unwrap_or(0);
            (p, kb)
        })
        .filter(|(_, kb)| *kb > 0)
        .collect();
    out.sort_by_key(|(_, kb)| std::cmp::Reverse(*kb));
    out
}
