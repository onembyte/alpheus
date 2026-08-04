//! Disk scanner: turns the raw filesystem into curated reclaim cards.
//!
//! Every category here corresponds to a real macOS space hog (Docker VM disks,
//! per-project `node_modules`, Xcode leftovers, app caches, iPhone backups…).
//! Scanners run in parallel threads; sizes are `du -sk` actuals, not estimates.
//! Nothing in this module deletes anything — see `exec` for that.

use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    PathBuf::from(std::env::var("HOME").expect("HOME not set"))
}

/// Hard denylist from HANDOFF.md: nothing outside $HOME is ever deletable,
/// and these subtrees are untouchable even inside it.
pub fn is_denied(p: &Path) -> bool {
    let h = home();
    if !p.starts_with(&h) {
        return true;
    }
    [
        h.join("Documents/prod"),
        h.join(".ssh"),
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
    let mut cmd = Command::new("/usr/bin/du");
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
    let (a, b, c, d, e) = std::thread::scope(|s| {
        let h1 = s.spawn(scan_projects);
        let h2 = s.spawn(scan_xcode);
        let h3 = s.spawn(scan_caches);
        let h4 = s.spawn(scan_simple);
        let h5 = s.spawn(scan_commands);
        (
            h1.join().unwrap_or_default(),
            h2.join().unwrap_or_default(),
            h3.join().unwrap_or_default(),
            h4.join().unwrap_or_default(),
            h5.join().unwrap_or_default(),
        )
    });
    let mut cards: Vec<Card> = [a, b, c, d, e].into_iter().flatten().collect();
    cards.sort_by_key(|c| std::cmp::Reverse(c.size_kb));
    cards
}

// ---------------------------------------------------------------- projects

/// Depth-limited walk collecting node_modules/.next dirs. Never follows
/// symlinks, never descends into the artifacts themselves.
fn find_artifacts(root: &Path, depth: usize, nm: &mut Vec<PathBuf>, nx: &mut Vec<PathBuf>) {
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
        if name.starts_with('.') || name == "Library" {
            continue;
        }
        find_artifacts(&path, depth - 1, nm, nx);
    }
}

struct GitProof {
    ok: bool,
    summary: String,
}

/// The "show proof" check from HANDOFF.md: a repo counts as verified only if
/// it is clean, has a remote, and has nothing unpushed.
fn git_verify(project: &Path) -> Option<GitProof> {
    if !project.join(".git").exists() {
        return None;
    }
    let run = |args: &[&str]| -> Option<String> {
        let out = Command::new("/usr/bin/git")
            .arg("-C")
            .arg(project)
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

/// Roots walked for per-project artifacts, with their max depth. `~/Documents`
/// is where projects live by convention; extra roots cover tools that vendor
/// their own node_modules outside it.
fn project_roots() -> Vec<(PathBuf, usize)> {
    let h = home();
    vec![(h.join("Documents"), 5), (h.join(".hermes"), 3)]
}

fn scan_projects() -> Vec<Card> {
    let mut nm: Vec<PathBuf> = vec![];
    let mut nx: Vec<PathBuf> = vec![];
    for (root, depth) in project_roots() {
        find_artifacts(&root, depth, &mut nm, &mut nx);
    }

    let all: Vec<PathBuf> = nm.iter().chain(nx.iter()).cloned().collect();
    let sizes = du_many_kb(&all);

    // (paths, total_kb, proof lines)
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
            description: "Dependency folders of projects whose git repo is clean, pushed, and has a remote (proof below). npm/pnpm install restores them from the lockfile.".into(),
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
            description: "Projects with no git remote, uncommitted or unpushed work, or no repo at all. The node_modules folders themselves are regenerable, but these projects may have no backup anywhere — push them to a remote first.".into(),
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
                "Next.js build directories — regenerated by the next `next build` or `next dev`."
                    .into(),
            tier: Tier::WithCare,
            size_kb: total,
            paths: nx.iter().map(|p| p.to_string_lossy().to_string()).collect(),
            proof: None,
            action: ActionKind::Delete,
            command_display: None,
        });
    }
    out
}

// ---------------------------------------------------------------- xcode

fn scan_xcode() -> Vec<Card> {
    let mut out = vec![];
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
                description: "Simulator devices, each carrying a full OS image. This action only deletes simulators marked unavailable or broken — erasing a healthy simulator stays a manual call.".into(),
                tier: Tier::WithCare,
                size_kb: kb,
                paths: vec![sims.to_string_lossy().to_string()],
                proof: None,
                action: ActionKind::Command,
                command_display: Some("xcrun simctl delete unavailable".into()),
            });
        }
    }
    out
}

// ---------------------------------------------------------------- caches

fn scan_caches() -> Vec<Card> {
    let caches = home().join("Library/Caches");
    // Counted in their own dedicated cards instead:
    let excluded = [
        "Homebrew",
        "CocoaPods",
        "ms-playwright",
        "com.spotify.client",
        "colima",
    ];
    let mut subs: Vec<PathBuf> = vec![];
    if let Ok(rd) = fs::read_dir(&caches) {
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
    if total < 1024 {
        return vec![];
    }
    vec![Card {
        id: "library-caches".into(),
        title: "App caches (~/Library/Caches)".into(),
        description: "Per-app scratch data — apps rebuild it as needed. Apple system caches (com.apple.*) are deliberately left alone; Homebrew, CocoaPods, Playwright and Spotify are counted in their own cards. Folders under 1 MB are skipped.".into(),
        tier: Tier::Safe,
        size_kb: total,
        paths: sized.iter().map(|(p, _)| p.to_string_lossy().to_string()).collect(),
        proof: None,
        action: ActionKind::Delete,
        command_display: None,
    }]
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
            description: "The Docker VM's virtual disk (~/.colima) — often the single biggest hidden item on a dev Mac. Deleting equals `colima delete`: the next `colima start` provisions a fresh VM, and images/containers come back from your Dockerfiles.",
            candidates: vec![h.join(".colima")],
        },
        Spec {
            id: "spotify-cache",
            title: "Spotify cache & downloads",
            tier: Tier::Safe,
            description: "Streaming cache plus downloaded tracks. Quit Spotify first. It re-caches as you listen; downloads must be re-downloaded in the app.",
            candidates: vec![
                h.join("Library/Application Support/Spotify/PersistentCache"),
                h.join("Library/Caches/com.spotify.client"),
            ],
        },
        Spec {
            id: "claude-vm",
            title: "Claude desktop VM bundle",
            tier: Tier::WithCare,
            description: "The desktop app's local VM image (vm_bundles). Only used by Claude's local-VM agent mode; it re-provisions itself on next use. Quit Claude before removing.",
            candidates: vec![h.join("Library/Application Support/Claude/vm_bundles")],
        },
        Spec {
            id: "iphone-backups",
            title: "iPhone backups (MobileSync)",
            tier: Tier::WithCare,
            description: "Local device backups. Anything not also in iCloud is gone for good — check Finder → iPhone → Manage Backups first. Always goes to the Trash, never direct delete.",
            candidates: vec![h.join("Library/Application Support/MobileSync/Backup")],
        },
        Spec {
            id: "leftovers",
            title: "One-off leftovers",
            tier: Tier::Safe,
            description: "Dead weight from one-off tools: exam-proctoring runtimes, app-updater caches, VS Code's cached extension installers.",
            candidates: vec![
                h.join("Library/Application Support/OnVUE"),
                h.join("Library/Application Support/Caches/binance-updater"),
                h.join("Library/Application Support/Caches/fing-updater"),
                h.join("Library/Application Support/Code/CachedExtensionVSIXs"),
            ],
        },
        Spec {
            id: "pkg-caches",
            title: "Package-manager caches",
            tier: Tier::Safe,
            description: "npm, pnpm, NuGet, CocoaPods, Go module and Playwright browser caches. Everything re-downloads on demand the next time you build.",
            candidates: vec![
                h.join(".npm/_cacache"),
                h.join("Library/pnpm/store"),
                h.join(".local/share/pnpm/store"),
                h.join(".local/share/NuGet"),
                h.join("Library/Caches/ms-playwright"),
                h.join("Library/Caches/CocoaPods"),
                h.join("Library/Caches/colima"),
                h.join("go/pkg/mod"),
            ],
        },
        Spec {
            id: "trash",
            title: "Finder Trash",
            tier: Tier::Safe,
            description: "Files already sitting in the Trash — including anything this app moved there. Emptying is the one delete that can't go to the Trash again.",
            candidates: vec![h.join(".Trash")],
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
    let now = std::time::SystemTime::now();
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

// ---------------------------------------------------------------- command & info cards

fn scan_commands() -> Vec<Card> {
    let mut out = vec![];

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

    if let Ok(out_snap) = Command::new("/usr/bin/tmutil")
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

    let update_vol = Path::new("/System/Volumes/Update");
    if update_vol.exists() {
        let kb = du_kb(update_vol); // best-effort; undercounts without root
        out.push(Card {
            id: "os-update".into(),
            title: "Staged macOS update".into(),
            description: "Staged system updates and Preboot bloat can pin 20+ GB that Settings files under \"System Data\". The only fix is finishing the update: System Settings → General → Software Update, then restart. Nothing here to click.".into(),
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
