//! Runs the walks that fill the storage detail tree.
//!
//! Jobs (one folder each, already placed in the tree) are taken in order by a
//! few worker threads at lowered QoS, so the first rows fill in first, big
//! folders don't hold up small ones, and the app keeps its speed. Meanwhile
//! the calling thread hands a snapshot to `on_snapshot` every `every`, and
//! once more when the last walk ends.
//!
//! A [`Session`] is one such walk of the whole disk, as the app runs it: it
//! sends small payloads while running, lets the user move the focus, can be
//! cancelled, and leaves a result worth caching for the next launch.

use crate::apfs::Container;
use crate::categories;
use crate::qos;
use crate::tree::{NodeId, State, Tree, View};
use crate::walker::{self, Shared};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

pub fn run<F: Fn(&Tree)>(
    jobs: Vec<(PathBuf, NodeId)>,
    tree: Arc<Tree>,
    shared: Arc<Shared>,
    workers: usize,
    every: Duration,
    on_snapshot: F,
) {
    let queue = Mutex::new(VecDeque::from(jobs));
    let running = AtomicUsize::new(workers.max(1));
    let main = std::thread::current();

    std::thread::scope(|s| {
        for _ in 0..workers.max(1) {
            s.spawn(|| {
                qos::lower();
                while !shared.cancel.load(Relaxed) {
                    let Some((path, id)) = queue.lock().unwrap().pop_front() else {
                        break;
                    };
                    tree.set_state(id, State::Measuring);
                    walker::walk(&path, id, &*tree, &shared);
                    if !shared.cancel.load(Relaxed) {
                        tree.set_state(id, State::Done);
                    }
                }
                running.fetch_sub(1, Relaxed);
                main.unpark();
            });
        }
        while running.load(Relaxed) > 0 {
            std::thread::park_timeout(every);
            if running.load(Relaxed) > 0 {
                on_snapshot(&tree);
            }
        }
    });
    on_snapshot(&tree);
}

// ---------------------------------------------------------------- session

/// Parallel walks per session: enough to overlap disk waits, few enough to
/// leave cores for everything else.
const WORKERS: usize = 3;

/// What the view receives: the category rows, and the children of the row
/// the user opened. Kept small because it is sent several times a second.
#[derive(Serialize, Deserialize, Clone)]
pub struct Payload {
    pub root: View,
    pub focus: Option<View>,
    pub files: u64,
    pub unreadable: u64,
    pub running: bool,
    /// Unix seconds.
    pub started: u64,
    pub finished: Option<u64>,
}

/// Levels kept below a folder whose insides are measured on demand.
const DEEPEN_LEVELS: usize = 3;

type Emit = dyn Fn(Payload) + Send + Sync;

/// What every thread of a session shares.
struct Core {
    tree: Arc<Tree>,
    shared: Arc<Shared>,
    /// Folder each job node was walked from, to find a node's path on disk.
    paths: HashMap<NodeId, PathBuf>,
    /// Where the Data volume is mounted (`/System/Volumes/Data` on macOS).
    data: PathBuf,
    focus: Mutex<Option<NodeId>>,
    finished: Mutex<Option<u64>>,
    started: u64,
    every: Duration,
    emit: Box<Emit>,
}

impl Core {
    fn payload(&self) -> Payload {
        let finished = *self.finished.lock().unwrap();
        Payload {
            root: self.tree.snapshot(Tree::ROOT, 1, 50),
            focus: self
                .focus
                .lock()
                .unwrap()
                .map(|id| self.tree.snapshot(id, 1, 200)),
            files: self.shared.files.load(Relaxed),
            unreadable: self.shared.unreadable.load(Relaxed),
            running: finished.is_none(),
            started: self.started,
            finished,
        }
    }

    fn emit(&self) {
        (self.emit)(self.payload());
    }
}

pub struct Session {
    core: Arc<Core>,
    /// The main walk, then one thread per folder opened past the kept depth.
    threads: Mutex<Vec<JoinHandle<()>>>,
    /// Each on-demand walk's own state: a fresh hard-link set, so files the
    /// main walk already counted aren't read as zero inside the folder.
    deepens: Mutex<Vec<Arc<Shared>>>,
}

impl Session {
    /// Plans the walk of the Data volume at `data` for the user at `home`,
    /// and starts it on a background thread. `emit` gets a payload every
    /// `every` while it runs, and a last one, not running, when it ends.
    pub fn start<F>(
        data: PathBuf,
        home: PathBuf,
        container: Option<Container>,
        every: Duration,
        emit: F,
    ) -> Session
    where
        F: Fn(Payload) + Send + Sync + 'static,
    {
        let name = container
            .as_ref()
            .and_then(|c| c.role("System"))
            .map_or("Disk".to_string(), |v| v.name.clone());
        let tree = Arc::new(Tree::new(&name));
        let jobs = categories::build(&tree, container.as_ref(), categories::plan(&data, &home));
        let core = Arc::new(Core {
            tree,
            shared: Arc::new(Shared::default()),
            paths: jobs.iter().map(|(p, id)| (*id, p.clone())).collect(),
            data,
            focus: Mutex::new(None),
            finished: Mutex::new(None),
            started: now(),
            every,
            emit: Box::new(emit),
        });
        let c = core.clone();
        let main = std::thread::spawn(move || {
            qos::lower();
            run(
                jobs,
                c.tree.clone(),
                c.shared.clone(),
                WORKERS,
                every,
                |_| c.emit(),
            );
            *c.finished.lock().unwrap() = Some(now());
            c.emit();
        });
        Session {
            core,
            threads: Mutex::new(vec![main]),
            deepens: Mutex::new(vec![]),
        }
    }

    pub fn cancel(&self) {
        self.core.shared.cancel.store(true, Relaxed);
        for d in self.deepens.lock().unwrap().iter() {
            d.cancel.store(true, Relaxed);
        }
    }

    /// Whether the main walk is still running.
    pub fn running(&self) -> bool {
        self.core.finished.lock().unwrap().is_none()
    }

    /// Blocks until every walk of this session has ended.
    pub fn wait(&self) {
        loop {
            let Some(h) = self.threads.lock().unwrap().pop() else {
                break;
            };
            let _ = h.join();
        }
    }

    /// The row whose children travel with each payload; an id the tree
    /// doesn't have clears it. Opening a row past the kept depth measures
    /// its insides in the background.
    pub fn set_focus(&self, id: Option<NodeId>) {
        let id = id.filter(|i| self.core.tree.has(*i));
        *self.core.focus.lock().unwrap() = id;
        if let Some(id) = id {
            self.deepen(id);
        }
    }

    fn deepen(&self, id: NodeId) {
        let Some(path) = self.core.tree.path_from(id, &self.core.paths) else {
            return;
        };
        if !path.is_dir() || self.core.shared.cancel.load(Relaxed) {
            return;
        }
        let Some(staging) = self.core.tree.try_stage(id, DEEPEN_LEVELS) else {
            return;
        };
        let shared = Arc::new(Shared::default());
        self.deepens.lock().unwrap().push(shared.clone());
        let c = self.core.clone();
        let h = std::thread::spawn(move || {
            qos::lower();
            let every = c.every;
            run(
                vec![(path, staging)],
                c.tree.clone(),
                shared.clone(),
                1,
                every,
                |_| c.emit(),
            );
            if !shared.cancel.load(Relaxed) {
                c.tree.swap(id);
            }
            c.emit();
        });
        self.threads.lock().unwrap().push(h);
    }

    pub fn payload(&self) -> Payload {
        self.core.payload()
    }

    /// Applies fresh APFS figures to the rows that come from APFS and to the
    /// total, then tells the window. Cheap: nothing is walked.
    pub fn refresh_volumes(&self, container: &Container) {
        categories::refresh(&self.core.tree, container);
        self.core.emit();
    }

    /// For each path, the rows holding it from the root down (empty when the
    /// walk doesn't cover it). Paths may be written as the user sees them
    /// (`/Users/…`) or on the Data volume itself.
    pub fn locate(&self, paths: &[PathBuf]) -> Vec<Vec<NodeId>> {
        let c = &self.core;
        paths
            .iter()
            .map(|p| {
                let direct = c.tree.locate(p, &c.paths);
                if !direct.is_empty() {
                    return direct;
                }
                let on_data = c.data.join(p.strip_prefix("/").unwrap_or(p));
                c.tree.locate(&on_data, &c.paths)
            })
            .collect()
    }

    /// The result as cached for the next launch: two levels, so the preview
    /// can open a category before the new walk reaches it.
    pub fn cached(&self) -> Payload {
        let mut p = self.payload();
        p.root = self.core.tree.snapshot(Tree::ROOT, 2, 50);
        p.focus = None;
        p
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The app's one walk. The app starts it at launch and the window asks for
/// it when it opens; both get the same walk, and only a finished one is
/// replaced. The lock is held while a new walk is planned, so two callers
/// racing can't start two.
#[derive(Default)]
pub struct Slot(Mutex<Option<Arc<Session>>>);

impl Slot {
    /// The running walk, or a new one from `start`.
    pub fn ensure(&self, start: impl FnOnce() -> Session) -> Arc<Session> {
        let mut slot = self.0.lock().unwrap();
        if let Some(s) = slot.as_ref().filter(|s| s.running()) {
            return s.clone();
        }
        let s = Arc::new(start());
        *slot = Some(s.clone());
        s
    }

    /// The current walk, running or finished.
    pub fn current(&self) -> Option<Arc<Session>> {
        self.0.lock().unwrap().clone()
    }
}

// ---------------------------------------------------------------- cache

fn cache_file(dir: &Path) -> PathBuf {
    dir.join("breakdown.json")
}

pub fn save_cache(dir: &Path, p: &Payload) {
    if fs::create_dir_all(dir).is_ok() {
        if let Ok(json) = serde_json::to_string(p) {
            let _ = fs::write(cache_file(dir), json);
        }
    }
}

/// The last finished result, or `None` when there is none or it is damaged.
pub fn load_cache(dir: &Path) -> Option<Payload> {
    serde_json::from_str(&fs::read_to_string(cache_file(dir)).ok()?).ok()
}
