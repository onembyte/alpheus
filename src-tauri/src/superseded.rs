//! Superseded builds: old versions an updater left beside the one in use.
//!
//! Side-by-side updaters (Autodesk Fusion's webdeploy, the Codex app-server
//! daemon) install every version in its own folder and point a marker at the
//! one in use. The previous folders stay behind, Fusion's at 11 GB apiece.
//!
//! Which build is current is read from the updater's own markers, never
//! guessed from dates or version order. When no marker matches a build on
//! disk, nothing is offered; a build any running process was started from is
//! kept as well. Every card here needs review and goes through the Trash.

use crate::exec::fmt;
use crate::scan::{du_many_kb, home, is_denied, modified_day, ActionKind, Card, Tier};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

/// Every build of one app on disk, and the ones its markers say are in use.
/// Both lists hold canonical paths, so they compare by value.
pub struct Builds {
    pub all: Vec<PathBuf>,
    pub current: Vec<PathBuf>,
}

/// The builds that may be offered: all of them, minus the current ones, minus
/// any that appear in a running command line (`running` is `ps` output).
///
/// Empty unless a marker matches a build on disk: a marker that resolves to
/// nothing means it was misread, and then no build is known to be safe.
pub fn superseded(builds: &Builds, running: &str) -> Vec<PathBuf> {
    if !builds.all.iter().any(|b| builds.current.contains(b)) {
        return vec![];
    }
    builds
        .all
        .iter()
        .filter(|b| !builds.current.contains(b))
        .filter(|b| !in_use(b, running))
        .cloned()
        .collect()
}

fn in_use(build: &Path, running: &str) -> bool {
    let prefix = format!("{}/", build.display());
    running.lines().any(|l| l.contains(&prefix))
}

// ---------------------------------------------------------------- Fusion

/// Fusion's launcher script names the current build on its `destfolder="…"` line.
pub fn launcher_build(script: &str) -> Option<PathBuf> {
    script.lines().find_map(|l| {
        let v = l.trim().strip_prefix("destfolder=\"")?.strip_suffix('"')?;
        (!v.is_empty()).then(|| PathBuf::from(v))
    })
}

/// Fusion's builds under webdeploy's `production` folder. Two markers name the
/// current one: the updater's `Autodesk Fusion.app` link and the launcher
/// script. Both are kept when they disagree.
pub fn fusion_builds(production: &Path, launcher_script: Option<&str>) -> Builds {
    let all = subdirs(production, |name| {
        name.len() == 40 && name.bytes().all(|b| b.is_ascii_hexdigit())
    });
    let mut current = vec![];
    if let Ok(target) = fs::read_link(production.join("Autodesk Fusion.app")) {
        let target = production.join(target); // no-op when already absolute
        let build = if target.extension().is_some_and(|e| e == "app") {
            target.parent().map(Path::to_path_buf)
        } else {
            Some(target)
        };
        current.extend(build.map(|b| canonical(&b)));
    }
    if let Some(b) = launcher_script.and_then(launcher_build) {
        current.push(canonical(&b));
    }
    Builds { all, current }
}

// ---------------------------------------------------------------- Codex

/// Codex app-server releases. The `current` link names the one in use; a
/// release named in `auto-update-version` is downloaded and about to become
/// current, so it is kept too — but only alongside a readable `current` link.
pub fn codex_releases(daemon: &Path) -> Builds {
    let releases = daemon.join("releases");
    let all = subdirs(&releases, |_| true);
    let mut current = vec![];
    if let Ok(t) = fs::canonicalize(daemon.join("current")) {
        current.push(t);
        if let Ok(v) = fs::read_to_string(daemon.join("auto-update-version")) {
            let v = v.trim();
            let pending = releases.join(v);
            if !v.is_empty() && !v.contains('/') && pending.is_dir() {
                current.push(canonical(&pending));
            }
        }
    }
    Builds { all, current }
}

// ---------------------------------------------------------------- helpers

/// Real directories directly inside `dir` whose name `accept`s, canonicalized.
/// Symlinks are skipped: `file_type` does not follow them.
fn subdirs(dir: &Path, accept: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut out: Vec<PathBuf> = rd
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| accept(&e.file_name().to_string_lossy()))
        .map(|e| canonical(&e.path()))
        .collect();
    out.sort();
    out
}

fn canonical(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Every running command line, or `None` when `ps` cannot be read — and then
/// no build is offered, since none can be shown to be idle.
fn running_commands() -> Option<String> {
    let out = Command::new("/bin/ps")
        .args(["-A", "-ww", "-o", "command="])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

// ---------------------------------------------------------------- diagnostics

/// How long a report must sit untouched before it counts as finished.
const SETTLE: Duration = Duration::from_secs(30 * 60);

/// Report folders under `dir` whose newest file is at least 30 minutes old.
/// A report still being collected keeps gaining files, and `sysdiagnosed`
/// never exits, so the files are the only honest signal. A report with any
/// unreadable part is left out.
pub fn diagnostic_reports(dir: &Path, now: SystemTime) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut out: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            matches!(newest_file(p), Ok(Some(t))
                if now.duration_since(t).is_ok_and(|age| age >= SETTLE))
        })
        .collect();
    out.sort();
    out
}

/// Newest file modification time under `p`, symlinks not followed. Folders'
/// own times are ignored: adding an entry touches them. `Ok(None)` when there
/// are no files, `Err` when any part can't be read.
fn newest_file(p: &Path) -> Result<Option<SystemTime>, ()> {
    let meta = fs::symlink_metadata(p).map_err(drop)?;
    if !meta.is_dir() {
        return meta.modified().map(Some).map_err(drop);
    }
    let mut newest = None;
    for e in fs::read_dir(p).map_err(drop)?.flatten() {
        newest = newest.max(newest_file(&e.path())?);
    }
    Ok(newest)
}

fn name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// A review card for `offered`, one proof line per path plus `kept` lines, or
/// `None` when there is under 1 MB to show.
fn card(
    id: &str,
    title: &str,
    description: &str,
    offered: Vec<PathBuf>,
    kept: Vec<String>,
    label: impl Fn(&Path) -> String,
) -> Option<Card> {
    let offered: Vec<PathBuf> = offered.into_iter().filter(|p| !is_denied(p)).collect();
    let sizes = du_many_kb(&offered);
    let total: u64 = offered.iter().filter_map(|p| sizes.get(p)).sum();
    if total < 1024 {
        return None;
    }
    let mut proof: Vec<String> = offered
        .iter()
        .map(|p| {
            let kb = sizes.get(p).copied().unwrap_or(0);
            format!("{}  ·  {}  ·  {}", fmt(kb), label(p), modified_day(p))
        })
        .collect();
    proof.extend(kept);
    Some(Card {
        id: id.into(),
        title: format!("{title} ({})", offered.len()),
        description: description.into(),
        tier: Tier::WithCare,
        size_kb: total,
        paths: offered
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        proof: Some(proof.join("\n")),
        action: ActionKind::Delete,
        command_display: None,
    })
}

fn builds_card(
    id: &str,
    title: &str,
    description: &str,
    builds: &Builds,
    running: &str,
    label: impl Fn(&Path) -> String,
) -> Option<Card> {
    let kept = builds
        .all
        .iter()
        .filter(|b| builds.current.contains(b))
        .map(|b| format!("kept  ·  {}  ·  current build", label(b)))
        .collect();
    card(
        id,
        title,
        description,
        superseded(builds, running),
        kept,
        label,
    )
}

// ---------------------------------------------------------------- scan

pub fn scan() -> Vec<Card> {
    let h = home();
    let mut out = vec![];

    if let Some(running) = running_commands() {
        let production = h.join("Library/Application Support/Autodesk/webdeploy/production");
        if production.is_dir() {
            let script = [h.join("Applications"), PathBuf::from("/Applications")]
                .iter()
                .find_map(|apps| {
                    fs::read_to_string(
                        apps.join("Autodesk Fusion.app/Contents/MacOS/Autodesk Fusion"),
                    )
                    .ok()
                });
            out.extend(builds_card(
                "fusion-old-builds",
                "Old Autodesk Fusion builds",
                "Earlier Fusion versions the updater left beside the current one. Fusion only runs the build its launcher points to; that one, and any build a running process uses, is kept. Moved to the Trash, so a build can be put back.",
                &fusion_builds(&production, script.as_deref()),
                &running,
                |p| {
                    let n = name(p);
                    format!("build {}", n.get(..7).unwrap_or(&n))
                },
            ));
        }

        let daemon = h.join(".codex/packages/app-server-daemon");
        if daemon.is_dir() {
            out.extend(builds_card(
                "codex-old-releases",
                "Old Codex app-server releases",
                "Earlier versions of the Codex app-server daemon, one folder per update. Codex runs the release its `current` link points to; that one, a downloaded update waiting to take over, and any release a running process uses are kept.",
                &codex_releases(&daemon),
                &running,
                name,
            ));
        }
    }

    let diag = h.join("Library/Logs/com.apple.diagnosticextensionsd");
    if diag.is_dir() {
        out.extend(card(
            "diagnostic-reports",
            "Feedback Assistant diagnostics",
            "System diagnostics (sysdiagnose) macOS collected for a Feedback Assistant report. Once that report is sent, this is only a local copy. Check it went through before removing. A report still being written is not listed.",
            diagnostic_reports(&diag, SystemTime::now()),
            vec![],
            name,
        ));
    }

    out
}
