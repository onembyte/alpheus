//! Disk scanner: turns the raw filesystem into curated reclaim cards.
//!
//! Scans developer artifacts (node_modules, .next, Cargo target/, Python caches),
//! package manager stores (Pacman, Yay, npm, pnpm, NuGet, Go, Pip, UV, Poetry),
//! system logs/coredumps/caches, Docker/Flatpak, and Trash on Linux & macOS.
//! Scanners run in parallel threads; sizes are `du -sk` actuals, not estimates.
//! Nothing in this module deletes anything — see `exec` for that.

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

#[derive(Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    Safe,
    WithCare,
    Manual,
}

#[derive(Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ActionKind {
    Delete,
    Command,
    Explain,
}

#[derive(Serialize, Clone)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tier: Tier,
    pub size_kb: u64,
    pub paths: Vec<String>,
    pub proof: Option<String>,
    pub action: ActionKind,
    pub command_display: Option<String>,
}

pub fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()))
}

/// Hard denylist: nothing outside $HOME is ever deletable directly,
/// and these subtrees are untouchable even inside it.
pub fn is_denied(p: &Path) -> bool {
    let h = home();
    if !p.starts_with(&h) {
        let system_allowed = [
            Path::new("/var/cache/pacman/pkg"),
            Path::new("/var/log/journal"),
            Path::new("/var/lib/systemd/coredump"),
            Path::new("/System/Volumes/Update"),
        ];
        if system_allowed.iter().any(|allowed| p.starts_with(allowed)) {
            return false;
        }
        return true;
    }
    [
        h.join("Documents/prod"),
        h.join(".ssh"),
        h.join(".gnupg"),
        h.join(".claude"),
        h.join("Library/Keychains"),
    ]
    .iter()
    .any(|d| p.starts_with(d))
}

/// One `du -sk` invocation for a batch of paths → actual KB per path.
pub fn du_many_kb(paths: &[PathBuf]) -> HashMap<PathBuf, u64> {
    let mut map = HashMap::new();
    if paths.is_empty() {
        return map;
    }
    let mut cmd = Command::new("du");
    cmd.arg("-sk");
    for p in paths {
        cmd.arg(p);
    }
    if let Ok(out) = cmd.output() {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let mut it = line.splitn(2, '\t');
            if let (Some(kb), Some(p)) = (it.next(), it.next()) {
                if let Ok(kb) = kb.trim().parse::<u64>() {
                    map.insert(PathBuf::from(p), kb);
                }
            }
        }
    }
    map
}

pub fn du_kb(path: &Path) -> u64 {
    du_many_kb(&[path.to_path_buf()])
        .values()
        .next()
        .copied()
        .unwrap_or(0)
}

pub fn scan_all() -> Vec<Card> {
    let (a, b, c, d, e, f) = std::thread::scope(|s| {
        let h1 = s.spawn(scan_projects);
        let h2 = s.spawn(scan_platform_dev);
        let h3 = s.spawn(scan_caches);
        let h4 = s.spawn(scan_simple);
        let h5 = s.spawn(scan_commands);
        let h6 = s.spawn(scan_stale_downloads);
        (
            h1.join().unwrap_or_default(),
            h2.join().unwrap_or_default(),
            h3.join().unwrap_or_default(),
            h4.join().unwrap_or_default(),
            h5.join().unwrap_or_default(),
            h6.join().unwrap_or_default(),
        )
    });
    let mut cards: Vec<Card> = [a, b, c, d, e, f].into_iter().flatten().collect();
    cards.sort_by_key(|c| std::cmp::Reverse(c.size_kb));
    cards
}

// ---------------------------------------------------------------- projects

/// Depth-limited walk collecting node_modules, .next, target, and python caches.
fn find_artifacts(
    root: &Path,
    depth: usize,
    nm: &mut Vec<PathBuf>,
    nx: &mut Vec<PathBuf>,
    targets: &mut Vec<PathBuf>,
    pycaches: &mut Vec<PathBuf>,
) {
    if depth == 0 {
        return;
    }
    let Ok(rd) = fs::read_dir(root) else { return };
    for entry in rd.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "node_modules" {
            if !is_denied(&path) {
                nm.push(path);
            }
            continue;
        }
        if name == ".next" {
            if !is_denied(&path) {
                nx.push(path);
            }
            continue;
        }
        if name == "target" {
            if let Some(parent) = path.parent() {
                if parent.join("Cargo.toml").exists() && !is_denied(&path) {
                    targets.push(path);
                    continue;
                }
            }
        }
        if name == "__pycache__" || name == ".pytest_cache" || name == ".ruff_cache" || name == ".mypy_cache" {
            if !is_denied(&path) {
                pycaches.push(path);
            }
            continue;
        }
        if name.starts_with('.') || name == "Library" || name == ".local" || name == ".cache" {
            continue;
        }
        find_artifacts(&path, depth - 1, nm, nx, targets, pycaches);
    }
}

struct GitProof {
    ok: bool,
    summary: String,
}

/// Verifies whether the enclosing Git repository is clean, pushed, and has a remote.
fn git_verify(project: &Path) -> Option<GitProof> {
    let toplevel = Command::new("git")
        .arg("-C")
        .arg(project)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()));

    let repo_root = match toplevel {
        Some(root) if root.exists() => root,
        _ => {
            if project.join(".git").exists() {
                project.to_path_buf()
            } else {
                return None;
            }
        }
    };

    let run = |args: &[&str]| -> Option<String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(&repo_root)
            .args(args)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let dirty = run(&["status", "--porcelain"])
        .map(|s| !s.is_empty())
        .unwrap_or(true);
    let has_remote = run(&["remote"]).map(|s| !s.is_empty()).unwrap_or(false);
    let unpushed = run(&["rev-list", "--count", "@{u}..HEAD"]).and_then(|s| s.parse::<u64>().ok());
    let ok = !dirty && has_remote && unpushed == Some(0);
    let summary = format!(
        "{}, {}, {}",
        if dirty { "DIRTY working tree" } else { "clean" },
        if has_remote {
            "has remote"
        } else {
            "NO REMOTE"
        },
        match unpushed {
            Some(0) => "nothing unpushed".to_string(),
            Some(n) => format!("{n} unpushed commits"),
            None => "no upstream".to_string(),
        }
    );
    Some(GitProof { ok, summary })
}

/// Auto-discovers common developer roots across Linux and macOS.
fn project_roots() -> Vec<(PathBuf, usize)> {
    let h = home();
    let candidates = [
        "Projects",
        "Developer",
        "code",
        "Code",
        "src",
        "workspace",
        "Workspace",
        "dev",
        "Dev",
        "repos",
        "Documents",
        ".hermes",
    ];
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    for name in candidates {
        let dir = h.join(name);
        if dir.is_dir() && !is_denied(&dir) {
            let canonical = dir.canonicalize().unwrap_or_else(|_| dir.clone());
            if seen.insert(canonical) {
                let depth = if name == ".hermes" { 3 } else { 5 };
                roots.push((dir, depth));
            }
        }
    }
    roots
}

fn scan_projects() -> Vec<Card> {
    let mut nm: Vec<PathBuf> = vec![];
    let mut nx: Vec<PathBuf> = vec![];
    let mut targets: Vec<PathBuf> = vec![];
    let mut pycaches: Vec<PathBuf> = vec![];

    for (root, depth) in project_roots() {
        find_artifacts(&root, depth, &mut nm, &mut nx, &mut targets, &mut pycaches);
    }

    let all: Vec<PathBuf> = nm
        .iter()
        .chain(nx.iter())
        .chain(targets.iter())
        .chain(pycaches.iter())
        .cloned()
        .collect();
    let sizes = du_many_kb(&all);

    let mut verified: (Vec<String>, u64, Vec<String>) = (vec![], 0, vec![]);
    let mut unverified: (Vec<String>, u64, Vec<String>) = (vec![], 0, vec![]);

    for p in &nm {
        let kb = *sizes.get(p).unwrap_or(&0);
        let project = p.parent().unwrap_or(p);
        let name = project
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let (bucket, proof) = match git_verify(project) {
            Some(g) if g.ok => (&mut verified, format!("{name}: {}", g.summary)),
            Some(g) => (&mut unverified, format!("{name}: {}", g.summary)),
            None => (&mut unverified, format!("{name}: not a git repo")),
        };
        bucket.0.push(p.to_string_lossy().to_string());
        bucket.1 += kb;
        bucket.2.push(proof);
    }

    let mut out = vec![];
    if !verified.0.is_empty() {
        out.push(Card {
            id: "node-modules-verified".into(),
            title: "node_modules — verified repos".into(),
            description: "Dependency folders of projects whose git repo is clean, pushed, and has a remote. npm/pnpm/yarn install restores them from the lockfile.".into(),
            tier: Tier::Safe,
            size_kb: verified.1,
            paths: verified.0,
            proof: Some(verified.2.join("\n")),
            action: ActionKind::Delete,
            command_display: None,
        });
    }
    if !unverified.0.is_empty() {
        out.push(Card {
            id: "node-modules-unverified".into(),
            title: "node_modules — unverified projects".into(),
            description: "Projects with no git remote, uncommitted or unpushed work, or no repo at all. These projects may have no remote backup — push them first.".into(),
            tier: Tier::WithCare,
            size_kb: unverified.1,
            paths: unverified.0,
            proof: Some(unverified.2.join("\n")),
            action: ActionKind::Delete,
            command_display: None,
        });
    }
    if !nx.is_empty() {
        let total: u64 = nx.iter().map(|p| sizes.get(p).copied().unwrap_or(0)).sum();
        out.push(Card {
            id: "next-builds".into(),
            title: ".next build outputs".into(),
            description:
                "Next.js build directories — regenerated on demand by `next build` or `next dev`."
                    .into(),
            tier: Tier::WithCare,
            size_kb: total,
            paths: nx.iter().map(|p| p.to_string_lossy().to_string()).collect(),
            proof: None,
            action: ActionKind::Delete,
            command_display: None,
        });
    }
    if !targets.is_empty() {
        let total: u64 = targets
            .iter()
            .map(|p| sizes.get(p).copied().unwrap_or(0))
            .sum();
        if total >= 1024 {
            out.push(Card {
                id: "cargo-target".into(),
                title: "Rust Cargo build directories (target/)".into(),
                description:
                    "Compiled Rust binaries and intermediate object files. Regenerated on next `cargo build`."
                        .into(),
                tier: Tier::Safe,
                size_kb: total,
                paths: targets
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect(),
                proof: None,
                action: ActionKind::Delete,
                command_display: None,
            });
        }
    }
    if !pycaches.is_empty() {
        let total: u64 = pycaches
            .iter()
            .map(|p| sizes.get(p).copied().unwrap_or(0))
            .sum();
        if total >= 1024 {
            out.push(Card {
                id: "py-cache".into(),
                title: "Python bytecode & test caches (__pycache__, .pytest_cache)".into(),
                description:
                    "Compiled Python bytecode (.pyc) and test/linter caches. Automatically regenerated by Python on run."
                        .into(),
                tier: Tier::Safe,
                size_kb: total,
                paths: pycaches
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect(),
                proof: None,
                action: ActionKind::Delete,
                command_display: None,
            });
        }
    }
    out
}

// ---------------------------------------------------------------- platform dev (Xcode / Linux)

fn scan_platform_dev() -> Vec<Card> {
    let mut out = vec![];

    // macOS Xcode
    #[cfg(target_os = "macos")]
    {
        let xcode = home().join("Library/Developer/Xcode");
        let mut derived: Vec<PathBuf> = vec![];
        if let Ok(rd) = fs::read_dir(&xcode) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().starts_with("DerivedData") {
                    derived.push(e.path());
                }
            }
        }
        let sizes = du_many_kb(&derived);
        let total: u64 = sizes.values().sum();
        if total >= 1024 {
            out.push(Card {
                id: "xcode-derived".into(),
                title: "Xcode DerivedData".into(),
                description: "Build caches and indexes. Xcode regenerates them — the first build after cleanup is slower, nothing is lost.".into(),
                tier: Tier::Safe,
                size_kb: total,
                paths: derived.iter().map(|p| p.to_string_lossy().to_string()).collect(),
                proof: None,
                action: ActionKind::Delete,
                command_display: None,
            });
        }

        let device_support = xcode.join("iOS DeviceSupport");
        if device_support.exists() {
            let kb = du_kb(&device_support);
            if kb >= 1024 {
                out.push(Card {
                    id: "xcode-devicesupport".into(),
                    title: "iOS device debug symbols".into(),
                    description: "Symbol files copied from iPhones for on-device debugging, one set per iOS version. Re-copied automatically the next time you debug on the device.".into(),
                    tier: Tier::Safe,
                    size_kb: kb,
                    paths: vec![device_support.to_string_lossy().to_string()],
                    proof: None,
                    action: ActionKind::Delete,
                    command_display: None,
                });
            }
        }

        let sims = home().join("Library/Developer/CoreSimulator/Devices");
        if sims.exists() {
            let kb = du_kb(&sims);
            if kb >= 1024 {
                out.push(Card {
                    id: "xcode-simulators".into(),
                    title: "iOS Simulators".into(),
                    description: "Simulator devices, each carrying a full OS image. This action only deletes simulators marked unavailable or broken.".into(),
                    tier: Tier::WithCare,
                    size_kb: kb,
                    paths: vec![sims.to_string_lossy().to_string()],
                    proof: None,
                    action: ActionKind::Command,
                    command_display: Some("xcrun simctl delete unavailable".into()),
                });
            }
        }
    }

    // Linux Yay / AUR Cache
    let yay_cache = home().join(".cache/yay");
    if yay_cache.exists() && !is_denied(&yay_cache) {
        let kb = du_kb(&yay_cache);
        if kb >= 1024 {
            out.push(Card {
                id: "yay-cache".into(),
                title: "Yay / AUR package build cache".into(),
                description: "Cloned AUR git repositories and built package artifacts in ~/.cache/yay. Safely removed; packages are already installed.".into(),
                tier: Tier::Safe,
                size_kb: kb,
                paths: vec![yay_cache.to_string_lossy().to_string()],
                proof: None,
                action: ActionKind::Delete,
                command_display: None,
            });
        }
    }

    out
}

// ---------------------------------------------------------------- caches

fn scan_caches() -> Vec<Card> {
    let mut out = vec![];
    let h = home();

    // macOS ~/Library/Caches
    let mac_caches = h.join("Library/Caches");
    if mac_caches.exists() {
        let excluded = [
            "Homebrew",
            "CocoaPods",
            "ms-playwright",
            "com.spotify.client",
            "colima",
        ];
        let mut subs: Vec<PathBuf> = vec![];
        if let Ok(rd) = fs::read_dir(&mac_caches) {
            for e in rd.flatten() {
                let Ok(ft) = e.file_type() else { continue };
                if !ft.is_dir() {
                    continue;
                }
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("com.apple.") || excluded.contains(&name.as_str()) {
                    continue;
                }
                subs.push(e.path());
            }
        }
        let sizes = du_many_kb(&subs);
        let mut sized: Vec<(PathBuf, u64)> = subs
            .into_iter()
            .map(|p| {
                let kb = sizes.get(&p).copied().unwrap_or(0);
                (p, kb)
            })
            .filter(|(_, kb)| *kb >= 1024)
            .collect();
        sized.sort_by_key(|s| std::cmp::Reverse(s.1));
        let total: u64 = sized.iter().map(|(_, kb)| kb).sum();
        if total >= 1024 {
            out.push(Card {
                id: "library-caches".into(),
                title: "App caches (~/Library/Caches)".into(),
                description: "Per-app scratch data — apps rebuild it as needed. Apple system caches (com.apple.*) are untouched. Folders under 1 MB are skipped.".into(),
                tier: Tier::Safe,
                size_kb: total,
                paths: sized.iter().map(|(p, _)| p.to_string_lossy().to_string()).collect(),
                proof: None,
                action: ActionKind::Delete,
                command_display: None,
            });
        }
    }

    // Linux XDG ~/.cache
    let linux_caches = h.join(".cache");
    if linux_caches.exists() && !mac_caches.exists() {
        let excluded = ["yay", "ms-playwright", "spotify", "colima", "pnpm", "go-build", "pip", "uv", "pypoetry", "huggingface", "torch"];
        let mut subs: Vec<PathBuf> = vec![];
        if let Ok(rd) = fs::read_dir(&linux_caches) {
            for e in rd.flatten() {
                let Ok(ft) = e.file_type() else { continue };
                if !ft.is_dir() {
                    continue;
                }
                let name = e.file_name().to_string_lossy().to_string();
                if excluded.contains(&name.as_str()) || is_denied(&e.path()) {
                    continue;
                }
                subs.push(e.path());
            }
        }
        let sizes = du_many_kb(&subs);
        let mut sized: Vec<(PathBuf, u64)> = subs
            .into_iter()
            .map(|p| {
                let kb = sizes.get(&p).copied().unwrap_or(0);
                (p, kb)
            })
            .filter(|(_, kb)| *kb >= 25 * 1024) // 25 MB+
            .collect();
        sized.sort_by_key(|s| std::cmp::Reverse(s.1));
        let total: u64 = sized.iter().map(|(_, kb)| kb).sum();
        if total >= 1024 {
            out.push(Card {
                id: "xdg-cache".into(),
                title: "User App Caches (~/.cache)".into(),
                description: "Browser, thumbnail, editor, and application scratch files. Apps rebuild them automatically.".into(),
                tier: Tier::Safe,
                size_kb: total,
                paths: sized.iter().map(|(p, _)| p.to_string_lossy().to_string()).collect(),
                proof: None,
                action: ActionKind::Delete,
                command_display: None,
            });
        }
    }

    out
}

// ---------------------------------------------------------------- fixed-path cards

struct Spec {
    id: &'static str,
    title: &'static str,
    description: &'static str,
    tier: Tier,
    candidates: Vec<PathBuf>,
}

fn scan_simple() -> Vec<Card> {
    let h = home();
    let specs = vec![
        Spec {
            id: "colima",
            title: "colima / Docker VM disk",
            tier: Tier::WithCare,
            description: "The Docker VM's virtual disk (~/.colima) or local container store — the next `colima start` provisions a fresh VM and images re-pull.",
            candidates: vec![h.join(".colima"), h.join(".local/share/containers")],
        },
        Spec {
            id: "spotify-cache",
            title: "Spotify cache & downloads",
            tier: Tier::Safe,
            description: "Streaming cache plus downloaded tracks. It re-caches as you listen; downloads must be re-downloaded in the app.",
            candidates: vec![
                h.join("Library/Application Support/Spotify/PersistentCache"),
                h.join("Library/Caches/com.spotify.client"),
                h.join(".cache/spotify"),
            ],
        },
        Spec {
            id: "claude-vm",
            title: "Claude desktop VM bundle",
            tier: Tier::WithCare,
            description: "The desktop app's local VM image (vm_bundles). Only used by Claude's local-VM agent mode; it re-provisions itself on next use.",
            candidates: vec![
                h.join("Library/Application Support/Claude/vm_bundles"),
                h.join(".config/Claude/vm_bundles"),
            ],
        },
        Spec {
            id: "iphone-backups",
            title: "iPhone backups (MobileSync)",
            tier: Tier::WithCare,
            description: "Local device backups. Anything not also in iCloud is gone for good — always goes to the Trash, never direct delete.",
            candidates: vec![h.join("Library/Application Support/MobileSync/Backup")],
        },
        Spec {
            id: "leftovers",
            title: "One-off leftovers",
            tier: Tier::Safe,
            description: "Dead weight from one-off tools: app-updater caches, VS Code's cached extension installers.",
            candidates: vec![
                h.join("Library/Application Support/OnVUE"),
                h.join("Library/Application Support/Caches/binance-updater"),
                h.join("Library/Application Support/Caches/fing-updater"),
                h.join("Library/Application Support/Code/CachedExtensionVSIXs"),
                h.join(".config/Code/CachedExtensionVSIXs"),
            ],
        },
        Spec {
            id: "pkg-caches",
            title: "Package-manager caches (npm, pnpm, pip, uv, go, nuget)",
            tier: Tier::Safe,
            description: "npm, pnpm, NuGet, Pip, UV, Poetry, HuggingFace, Go module and Playwright caches. Everything re-downloads on demand when building.",
            candidates: vec![
                h.join(".npm/_cacache"),
                h.join("Library/pnpm/store"),
                h.join(".local/share/pnpm/store"),
                h.join(".local/share/NuGet"),
                h.join("Library/Caches/ms-playwright"),
                h.join(".cache/ms-playwright"),
                h.join("Library/Caches/CocoaPods"),
                h.join("Library/Caches/colima"),
                h.join("go/pkg/mod"),
                h.join(".cache/go-build"),
                h.join(".cache/pip"),
                h.join(".cache/uv"),
                h.join(".cache/pypoetry/cache"),
                h.join(".cache/pypoetry/artifacts"),
                h.join(".cache/huggingface/hub"),
                h.join(".cargo/registry/cache"),
            ],
        },
        Spec {
            id: "trash",
            title: "Trash",
            tier: Tier::Safe,
            description: "Files already sitting in the Trash — including anything this app moved there. Emptying frees disk space permanently.",
            candidates: vec![h.join(".Trash"), h.join(".local/share/Trash")],
        },
    ];

    let mut out = vec![];
    for spec in specs {
        let paths: Vec<PathBuf> = spec
            .candidates
            .iter()
            .filter(|p| p.exists() && !is_denied(p))
            .cloned()
            .collect();
        if paths.is_empty() {
            continue;
        }
        let sizes = du_many_kb(&paths);
        let total: u64 = sizes.values().sum();
        if total < 1024 {
            continue;
        }
        let proof = if spec.id == "iphone-backups" {
            Some(list_backups(&paths[0]))
        } else {
            None
        };
        out.push(Card {
            id: spec.id.into(),
            title: spec.title.into(),
            description: spec.description.into(),
            tier: spec.tier,
            size_kb: total,
            paths: paths
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            proof,
            action: ActionKind::Delete,
            command_display: None,
        });
    }
    out
}

fn list_backups(dir: &Path) -> String {
    let now = SystemTime::now();
    let mut lines = vec![];
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let age_days = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|m| now.duration_since(m).ok())
                .map(|d| d.as_secs() / 86400);
            match age_days {
                Some(days) => lines.push(format!(
                    "{} — last modified {days} days ago",
                    e.file_name().to_string_lossy()
                )),
                None => lines.push(e.file_name().to_string_lossy().to_string()),
            }
        }
    }
    if lines.is_empty() {
        "no backup folders found".to_string()
    } else {
        lines.join("\n")
    }
}

// ---------------------------------------------------------------- stale downloads (>30 days)

fn scan_stale_downloads() -> Vec<Card> {
    let downloads = home().join("Downloads");
    if !downloads.is_dir() || is_denied(&downloads) {
        return vec![];
    }

    let now = SystemTime::now();
    let mut stale_paths = vec![];

    if let Ok(rd) = fs::read_dir(&downloads) {
        for entry in rd.flatten() {
            let path = entry.path();
            if is_denied(&path) {
                continue;
            }
            if let Ok(meta) = entry.metadata() {
                if let Ok(modified) = meta.modified() {
                    if let Ok(duration) = now.duration_since(modified) {
                        let age_days = duration.as_secs() / 86400;
                        if age_days >= 30 {
                            stale_paths.push(path);
                        }
                    }
                }
            }
        }
    }

    if stale_paths.is_empty() {
        return vec![];
    }

    let sizes = du_many_kb(&stale_paths);
    let mut sized: Vec<(PathBuf, u64)> = stale_paths
        .into_iter()
        .map(|p| {
            let kb = sizes.get(&p).copied().unwrap_or(0);
            (p, kb)
        })
        .filter(|(_, kb)| *kb >= 25 * 1024) // 25 MB+
        .collect();

    sized.sort_by_key(|s| std::cmp::Reverse(s.1));
    let total: u64 = sized.iter().map(|(_, kb)| kb).sum();
    if total < 25 * 1024 {
        return vec![];
    }

    vec![Card {
        id: "stale-downloads".into(),
        title: "Stale large downloads (>30 days untouched in ~/Downloads)".into(),
        description: "Old installers, iso images, and downloaded archives that have not been modified in over 30 days. Review before removing.".into(),
        tier: Tier::WithCare,
        size_kb: total,
        paths: sized.iter().map(|(p, _)| p.to_string_lossy().to_string()).collect(),
        proof: None,
        action: ActionKind::Delete,
        command_display: None,
    }]
}

// ---------------------------------------------------------------- command & info cards

fn scan_commands() -> Vec<Card> {
    let mut out = vec![];

    // Linux: Pacman Package Cache (/var/cache/pacman/pkg)
    let pacman_cache = Path::new("/var/cache/pacman/pkg");
    if pacman_cache.exists() {
        let kb = du_kb(pacman_cache);
        if kb >= 1024 {
            out.push(Card {
                id: "pacman-cache".into(),
                title: "Pacman package cache (/var/cache/pacman/pkg)".into(),
                description: "Arch Linux downloaded package archives. Cleaning removes uninstalled and older versions while retaining installed packages.".into(),
                tier: Tier::Safe,
                size_kb: kb,
                paths: vec![pacman_cache.to_string_lossy().to_string()],
                proof: None,
                action: ActionKind::Command,
                command_display: Some("sudo paccache -rk2".into()),
            });
        }
    }

    // Linux: Systemd Journal Logs
    let journal_dir = Path::new("/var/log/journal");
    if journal_dir.exists() {
        let kb = du_kb(journal_dir);
        if kb >= 1024 {
            out.push(Card {
                id: "journal-logs".into(),
                title: "Systemd journal logs (/var/log/journal)".into(),
                description: "Arch Linux system logs. Vacuuming retains the latest 200 MB of logs and frees the rest.".into(),
                tier: Tier::WithCare,
                size_kb: kb,
                paths: vec![journal_dir.to_string_lossy().to_string()],
                proof: None,
                action: ActionKind::Command,
                command_display: Some("sudo journalctl --vacuum-size=200M".into()),
            });
        }
    }

    // Linux: Systemd Crash Coredumps (/var/lib/systemd/coredump)
    let coredump_dir = Path::new("/var/lib/systemd/coredump");
    if coredump_dir.exists() {
        let kb = du_kb(coredump_dir);
        if kb >= 1024 {
            out.push(Card {
                id: "coredump-logs".into(),
                title: "Systemd crash coredumps (/var/lib/systemd/coredump)".into(),
                description: "Stored crash dumps from aborted processes. Vacuuming removes old crash dumps while keeping recent reports.".into(),
                tier: Tier::Safe,
                size_kb: kb,
                paths: vec![coredump_dir.to_string_lossy().to_string()],
                proof: None,
                action: ActionKind::Command,
                command_display: Some("sudo coredumpctl vacuum --size=50M".into()),
            });
        }
    }

    // Linux: Flatpak unused runtimes
    if let Ok(out_flatpak) = Command::new("flatpak").args(["list", "--unused"]).output() {
        if out_flatpak.status.success() && !out_flatpak.stdout.is_empty() {
            let text = String::from_utf8_lossy(&out_flatpak.stdout);
            let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
            if !lines.is_empty() {
                out.push(Card {
                    id: "flatpak-unused".into(),
                    title: format!("Unused Flatpak runtimes ({} found)", lines.len()),
                    description: "Old Flatpak runtimes no longer required by any installed application.".into(),
                    tier: Tier::Safe,
                    size_kb: 0,
                    paths: vec![],
                    proof: Some(lines.join("\n")),
                    action: ActionKind::Command,
                    command_display: Some("flatpak uninstall --unused -y".into()),
                });
            }
        }
    }

    // Docker / Podman prune
    if let Ok(out_docker) = Command::new("docker").args(["system", "df"]).output() {
        if out_docker.status.success() {
            let text = String::from_utf8_lossy(&out_docker.stdout);
            // Parse reclaimable column if any
            let has_reclaimable = text.lines().any(|l| l.contains("GB") || l.contains("MB"));
            if has_reclaimable && !text.contains("0B        0B") {
                out.push(Card {
                    id: "docker-prune".into(),
                    title: "Docker unused containers, images & build cache".into(),
                    description: "Dangling Docker images, stopped build containers, and build cache. Runs `docker system prune -f`.".into(),
                    tier: Tier::WithCare,
                    size_kb: 0,
                    paths: vec![],
                    proof: Some(text.trim().to_string()),
                    action: ActionKind::Command,
                    command_display: Some("docker system prune -f".into()),
                });
            }
        }
    }

    // macOS: Homebrew
    let brew_cache = home().join("Library/Caches/Homebrew");
    if brew_cache.exists() {
        let kb = du_kb(&brew_cache);
        if kb >= 1024 {
            out.push(Card {
                id: "brew-cleanup".into(),
                title: "Homebrew cleanup".into(),
                description: "Old downloads and outdated kegs. Runs Homebrew's own cleanup, which only removes what brew knows is stale.".into(),
                tier: Tier::Safe,
                size_kb: kb,
                paths: vec![brew_cache.to_string_lossy().to_string()],
                proof: None,
                action: ActionKind::Command,
                command_display: Some("brew cleanup --prune=all".into()),
            });
        }
    }

    // macOS: Time Machine
    #[cfg(target_os = "macos")]
    if let Ok(out_snap) = Command::new("tmutil")
        .args(["listlocalsnapshots", "/"])
        .output()
    {
        let text = String::from_utf8_lossy(&out_snap.stdout).to_string();
        let snaps: Vec<String> = text
            .lines()
            .filter(|l| l.contains("com.apple.TimeMachine"))
            .map(|s| s.trim().to_string())
            .collect();
        if !snaps.is_empty() {
            out.push(Card {
                id: "tm-snapshots".into(),
                title: format!("Time Machine local snapshots ({})", snaps.len()),
                description: "APFS snapshots pin deleted data until they expire (~24 h) or the disk hits pressure. Their size can't be attributed exactly; deleting releases whatever they pin.".into(),
                tier: Tier::WithCare,
                size_kb: 0,
                paths: vec![],
                proof: Some(snaps.join("\n")),
                action: ActionKind::Command,
                command_display: Some("tmutil deletelocalsnapshots <each snapshot date>".into()),
            });
        }
    }

    // macOS: Staged Update
    let update_vol = Path::new("/System/Volumes/Update");
    if update_vol.exists() {
        let kb = du_kb(update_vol);
        out.push(Card {
            id: "os-update".into(),
            title: "Staged macOS update".into(),
            description: "Staged system updates and Preboot bloat can pin 20+ GB. The only fix is finishing the update in System Settings → Software Update. Nothing here to click.".into(),
            tier: Tier::Manual,
            size_kb: kb,
            paths: vec!["/System/Volumes/Update".into()],
            proof: None,
            action: ActionKind::Explain,
            command_display: None,
        });
    }

    out
}
