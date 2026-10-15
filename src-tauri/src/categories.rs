//! Categories like System Settings → Storage, and the walk plan behind them.
//!
//! The Data volume's top level is split into jobs, each a folder (or file)
//! with exactly one category. Folders that hold several categories (/Users,
//! the home folder, ~/Library, /Applications) are split one level down and
//! never walked whole, so nothing is counted twice. Mounts under /Volumes
//! are other disks and are left out.
//!
//! Two deliberate differences from Settings: Developer also holds toolchains
//! and project folders (hidden folders in the home folder, Homebrew, ~/dev),
//! and System Data is broken down rather than shown as one number.

use crate::apfs::Container;
use crate::tree::{NodeId, Tree};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    Applications,
    Developer,
    Documents,
    ICloudDrive,
    Mail,
    Messages,
    Music,
    Photos,
    Trash,
    OtherUsers,
    MacOS,
    SystemData,
}

/// The order System Settings lists them in.
pub const ORDER: [Category; 12] = [
    Category::Applications,
    Category::Developer,
    Category::Documents,
    Category::ICloudDrive,
    Category::Mail,
    Category::Messages,
    Category::Music,
    Category::Photos,
    Category::Trash,
    Category::OtherUsers,
    Category::MacOS,
    Category::SystemData,
];

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::Applications => "Applications",
            Category::Developer => "Developer",
            Category::Documents => "Documents",
            Category::ICloudDrive => "iCloud Drive",
            Category::Mail => "Mail",
            Category::Messages => "Messages",
            Category::Music => "Music",
            Category::Photos => "Photos",
            Category::Trash => "Trash",
            Category::OtherUsers => "Other Users",
            Category::MacOS => "macOS",
            Category::SystemData => "System Data",
        }
    }
}

/// Home-folder names that hold code rather than documents.
const PROJECT_ROOTS: &[&str] = &[
    "dev",
    "Dev",
    "Developer",
    "Projects",
    "code",
    "Code",
    "src",
    "repos",
    "workspace",
    "Workspace",
];

/// Folders kept below each walked folder; app bundles keep none.
const LEVELS: usize = 3;

/// One folder or file to walk, and where its row goes.
pub struct Job {
    pub category: Category,
    /// A row grouping several jobs inside the category ("Your Library").
    pub group: Option<&'static str>,
    pub path: PathBuf,
    pub label: String,
    pub levels: usize,
}

/// Every job for the Data volume mounted at `data`, for the user whose home
/// is `home` (an absolute path as the system sees it, e.g. `/Users/me`).
/// Top-level entries on another device (mounts, Linux's /proc) are skipped.
pub fn plan(data: &Path, home: &Path) -> Vec<Job> {
    let home = data.join(home.strip_prefix("/").unwrap_or(home));
    let device = fs::symlink_metadata(data).map(|m| m.dev()).ok();
    let mut jobs = vec![];
    for (name, path) in entries(data) {
        let elsewhere = fs::symlink_metadata(&path).is_ok_and(|m| Some(m.dev()) != device);
        if elsewhere {
            continue;
        }
        match name.as_str() {
            "Volumes" => {}
            "Applications" => apps(&path, &mut jobs),
            "Users" | "home" => {
                for (user, p) in entries(&path) {
                    if p == home {
                        home_jobs(&p, &mut jobs);
                    } else {
                        jobs.push(job(Category::OtherUsers, None, p, user));
                    }
                }
            }
            "opt" | "usr" => jobs.push(job(Category::Developer, None, path, format!("/{name}"))),
            _ => jobs.push(job(Category::SystemData, None, path, format!("/{name}"))),
        }
    }
    jobs
}

fn home_jobs(home: &Path, jobs: &mut Vec<Job>) {
    for (name, path) in entries(home) {
        let category = match name.as_str() {
            "Library" => {
                library_jobs(&path, jobs);
                continue;
            }
            "Applications" => {
                apps(&path, jobs);
                continue;
            }
            ".Trash" => Category::Trash,
            "Music" => Category::Music,
            "Pictures" => Category::Photos,
            n if n.starts_with('.') || PROJECT_ROOTS.contains(&n) => Category::Developer,
            _ => Category::Documents,
        };
        jobs.push(job(category, None, path, name));
    }
}

fn library_jobs(library: &Path, jobs: &mut Vec<Job>) {
    for (name, path) in entries(library) {
        let (category, group) = match name.as_str() {
            "Developer" => (Category::Developer, None),
            "Mail" => (Category::Mail, None),
            "Messages" => (Category::Messages, None),
            "Mobile Documents" => (Category::ICloudDrive, None),
            _ => (Category::SystemData, Some("Your Library")),
        };
        let label = if group.is_some() {
            name
        } else {
            format!("Library/{name}")
        };
        jobs.push(job(category, group, path, label));
    }
}

/// Each app is its own row; a bundle's insides are not worth rows.
fn apps(dir: &Path, jobs: &mut Vec<Job>) {
    for (name, path) in entries(dir) {
        let (label, levels) = match name.strip_suffix(".app") {
            Some(app) => (app.to_string(), 0),
            None => (name, 1),
        };
        jobs.push(Job {
            category: Category::Applications,
            group: None,
            path,
            label,
            levels,
        });
    }
}

fn job(category: Category, group: Option<&'static str>, path: PathBuf, label: String) -> Job {
    Job {
        category,
        group,
        path,
        label,
        levels: LEVELS,
    }
}

/// Entries of `dir` by name, sorted so plans are stable.
fn entries(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| (e.file_name().to_string_lossy().to_string(), e.path()))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Places the categories, the APFS volumes and the jobs in `tree`, and
/// returns each job's path with its node, ready for `breakdown::run`.
///
/// With APFS figures, macOS and the support volumes come straight from APFS,
/// and an Unattributed row under System Data makes the whole tree add up to
/// the disk's used space: snapshots, purgeable space, unreadable folders, and
/// while the walk runs, everything not measured yet.
pub fn build(tree: &Tree, container: Option<&Container>, jobs: Vec<Job>) -> Vec<(PathBuf, NodeId)> {
    let mut nodes: HashMap<Category, NodeId> = HashMap::new();
    for c in ORDER {
        let volumes = container.is_some() && c == Category::SystemData;
        if c == Category::MacOS {
            if let Some(system) = container.and_then(|k| k.role("System")) {
                tree.add_measured(Tree::ROOT, c.label(), system.used_kb * 1024);
            }
        } else if volumes || jobs.iter().any(|j| j.category == c) {
            nodes.insert(c, tree.add(Tree::ROOT, c.label(), 0));
        }
    }

    if let Some(k) = container {
        let sys = nodes[&Category::SystemData];
        for v in &k.volumes {
            let walked_or_macos = v.roles.iter().any(|r| r == "System" || r == "Data");
            if !walked_or_macos {
                tree.add_measured(sys, &v.name, v.used_kb * 1024);
            }
        }
        let used: u64 = k.volumes.iter().map(|v| v.used_kb * 1024).sum();
        tree.add_remainder(sys, "Unattributed", used);
    }

    let mut groups: HashMap<(Category, &str), NodeId> = HashMap::new();
    jobs.into_iter()
        .map(|j| {
            let mut parent = nodes[&j.category];
            if let Some(g) = j.group {
                parent = *groups
                    .entry((j.category, g))
                    .or_insert_with(|| tree.add(parent, g, 0));
            }
            let id = tree.add(parent, &j.label, j.levels);
            (j.path, id)
        })
        .collect()
}
