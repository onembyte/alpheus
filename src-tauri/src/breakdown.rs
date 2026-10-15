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
use std::collections::VecDeque;
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

pub struct Session {
    tree: Arc<Tree>,
    shared: Arc<Shared>,
    focus: Arc<Mutex<Option<NodeId>>>,
    finished: Arc<Mutex<Option<u64>>>,
    started: u64,
    handle: Mutex<Option<JoinHandle<()>>>,
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
        F: Fn(Payload) + Send + 'static,
    {
        let name = container
            .as_ref()
            .and_then(|c| c.role("System"))
            .map_or("Disk".to_string(), |v| v.name.clone());
        let tree = Arc::new(Tree::new(&name));
        let jobs = categories::build(&tree, container.as_ref(), categories::plan(&data, &home));
        let s = Session {
            tree,
            shared: Arc::new(Shared::default()),
            focus: Arc::default(),
            finished: Arc::default(),
            started: now(),
            handle: Mutex::new(None),
        };
        let (tree, shared, focus, finished, started) = (
            s.tree.clone(),
            s.shared.clone(),
            s.focus.clone(),
            s.finished.clone(),
            s.started,
        );
        let handle = std::thread::spawn(move || {
            qos::lower();
            let live = |t: &Tree| emit(payload(t, &shared, *focus.lock().unwrap(), started, None));
            run(jobs, tree.clone(), shared.clone(), WORKERS, every, live);
            let end = Some(now());
            *finished.lock().unwrap() = end;
            emit(payload(
                &tree,
                &shared,
                *focus.lock().unwrap(),
                started,
                end,
            ));
        });
        *s.handle.lock().unwrap() = Some(handle);
        s
    }

    pub fn cancel(&self) {
        self.shared.cancel.store(true, Relaxed);
    }

    pub fn running(&self) -> bool {
        self.finished.lock().unwrap().is_none()
    }

    /// Blocks until the walk has ended (finished or cancelled).
    pub fn wait(&self) {
        if let Some(h) = self.handle.lock().unwrap().take() {
            let _ = h.join();
        }
    }

    /// The row whose children travel with each payload; an id the tree
    /// doesn't have clears it.
    pub fn set_focus(&self, id: Option<NodeId>) {
        *self.focus.lock().unwrap() = id.filter(|i| self.tree.has(*i));
    }

    pub fn payload(&self) -> Payload {
        payload(
            &self.tree,
            &self.shared,
            *self.focus.lock().unwrap(),
            self.started,
            *self.finished.lock().unwrap(),
        )
    }

    /// The result as cached for the next launch: two levels, so the preview
    /// can open a category before the new walk reaches it.
    pub fn cached(&self) -> Payload {
        let mut p = self.payload();
        p.root = self.tree.snapshot(Tree::ROOT, 2, 50);
        p.focus = None;
        p
    }
}

fn payload(
    tree: &Tree,
    shared: &Shared,
    focus: Option<NodeId>,
    started: u64,
    finished: Option<u64>,
) -> Payload {
    Payload {
        root: tree.snapshot(Tree::ROOT, 1, 50),
        focus: focus.map(|id| tree.snapshot(id, 1, 200)),
        files: shared.files.load(Relaxed),
        unreadable: shared.unreadable.load(Relaxed),
        running: finished.is_none(),
        started,
        finished,
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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
