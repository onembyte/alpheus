//! Runs the walks that fill the storage detail tree.
//!
//! Jobs (one folder each, already placed in the tree) are taken in order by a
//! few worker threads, so the first rows fill in first and big folders don't
//! hold up small ones. Meanwhile the calling thread hands a snapshot to
//! `on_snapshot` every `every`, and once more when the last walk ends.

use crate::tree::{NodeId, State, Tree};
use crate::walker::{self, Shared};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
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
