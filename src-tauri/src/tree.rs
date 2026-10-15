//! The live tree behind the storage detail view.
//!
//! Nodes are appended, never removed, and a parent always comes before its
//! children, so subtree totals are one reverse pass over the list. Each
//! walked folder keeps a fixed number of levels below it; deeper folders fold
//! into their last kept ancestor, which bounds memory on a whole-disk walk.
//! Sizes only ever grow, so successive snapshots never show a total shrink.
//!
//! Opening a folder past its kept depth measures it again into a staging
//! node with no parent, which no total counts; its rows show live, and when
//! the walk ends they are swapped in, so the totals above never jump.

use crate::walker::Sink;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub type NodeId = usize;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    Pending,
    Measuring,
    Done,
}

struct Node {
    name: String,
    parent: Option<NodeId>,
    depth: usize,
    /// Deepest level at which folders below this one still get nodes.
    track_to: usize,
    children: Vec<NodeId>,
    /// Bytes directly in this folder, plus folders folded into it.
    own: u64,
    state: State,
    /// Subfolders were folded in at the kept depth: there is more to open.
    folded: bool,
    /// Where this node's insides are being measured again, until `swap`.
    staging: Option<NodeId>,
}

impl Node {
    fn deepenable(&self) -> bool {
        self.folded
            && self.children.is_empty()
            && self.staging.is_none()
            && self.state == State::Done
    }

    fn new(
        name: String,
        parent: Option<NodeId>,
        depth: usize,
        track_to: usize,
        state: State,
    ) -> Node {
        Node {
            name,
            parent,
            depth,
            track_to,
            children: vec![],
            own: 0,
            state,
            folded: false,
            staging: None,
        }
    }
}

/// One node as the view sees it, with its biggest children.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct View {
    pub id: NodeId,
    pub name: String,
    /// The whole subtree.
    pub bytes: u64,
    pub own_bytes: u64,
    pub state: State,
    pub children: Vec<View>,
    /// Children left out past the cap, and what they hold together.
    pub other_count: usize,
    pub other_bytes: u64,
    /// All of its children, shown or not.
    pub child_count: usize,
    /// Opening it shows rows: it has children, or folded folders to measure.
    pub openable: bool,
}

pub struct Tree {
    nodes: Mutex<Vec<Node>>,
    /// The remainder node and the total the whole tree must add up to.
    budget: Mutex<Option<(NodeId, u64)>>,
}

impl Tree {
    pub const ROOT: NodeId = 0;

    pub fn new(name: &str) -> Self {
        Tree {
            nodes: Mutex::new(vec![Node::new(name.into(), None, 0, 0, State::Pending)]),
            budget: Mutex::new(None),
        }
    }

    /// A node the scan places itself (a category, a folder to walk), which
    /// keeps `levels` levels of folders below it once walked.
    pub fn add(&self, parent: NodeId, name: &str, levels: usize) -> NodeId {
        let mut n = self.nodes.lock().unwrap();
        let depth = n[parent].depth + 1;
        n.push(Node::new(
            name.into(),
            Some(parent),
            depth,
            depth + levels,
            State::Pending,
        ));
        let id = n.len() - 1;
        n[parent].children.push(id);
        id
    }

    /// A node whose size is already known (an APFS volume): done, with
    /// nothing to walk.
    pub fn add_measured(&self, parent: NodeId, name: &str, bytes: u64) -> NodeId {
        let id = self.add(parent, name, 0);
        let mut n = self.nodes.lock().unwrap();
        n[id].own = bytes;
        n[id].state = State::Done;
        id
    }

    /// A node holding whatever `budget` leaves once everything else in the
    /// tree is counted, so the root always adds up to `budget`. It shrinks as
    /// walks measure more, and stops at zero.
    pub fn add_remainder(&self, parent: NodeId, name: &str, budget: u64) -> NodeId {
        let id = self.add(parent, name, 0);
        self.nodes.lock().unwrap()[id].state = State::Done;
        *self.budget.lock().unwrap() = Some((id, budget));
        id
    }

    /// The child of `parent` called `name`.
    pub fn find_child(&self, parent: NodeId, name: &str) -> Option<NodeId> {
        let n = self.nodes.lock().unwrap();
        n.get(parent)?
            .children
            .iter()
            .copied()
            .find(|c| n[*c].name == name)
    }

    /// Replaces the size of a node whose size is known from outside (an APFS
    /// volume), unlike walked nodes, which only grow.
    pub fn set_own(&self, id: NodeId, bytes: u64) {
        if let Some(x) = self.nodes.lock().unwrap().get_mut(id) {
            x.own = bytes;
        }
    }

    /// Moves the total the tree adds up to (see `add_remainder`).
    pub fn set_budget(&self, bytes: u64) {
        if let Some((_, budget)) = self.budget.lock().unwrap().as_mut() {
            *budget = bytes;
        }
    }

    /// Whether `id` names a node. Ids arrive from the view, so check first.
    pub fn has(&self, id: NodeId) -> bool {
        id < self.nodes.lock().unwrap().len()
    }

    pub fn set_state(&self, id: NodeId, state: State) {
        self.nodes.lock().unwrap()[id].state = state;
    }

    /// Whether opening `id` needs its insides measured: a finished folder
    /// with folded folders and no rows of its own yet.
    pub fn deepen_target(&self, id: NodeId) -> bool {
        self.nodes
            .lock()
            .unwrap()
            .get(id)
            .is_some_and(Node::deepenable)
    }

    /// Starts measuring `id`'s insides afresh, `levels` deep, into a new
    /// staging node that no total counts. Walk the folder into the returned
    /// node, then `swap`.
    pub fn stage(&self, id: NodeId, levels: usize) -> NodeId {
        stage(&mut self.nodes.lock().unwrap(), id, levels)
    }

    /// `stage` if `id` is a `deepen_target`, as one step, so two requests
    /// for the same folder can't start two walks.
    pub fn try_stage(&self, id: NodeId, levels: usize) -> Option<NodeId> {
        let mut n = self.nodes.lock().unwrap();
        n.get(id)
            .is_some_and(Node::deepenable)
            .then(|| stage(&mut n, id, levels))
    }

    /// Moves the staged rows under `id`, which then holds what the new walk
    /// measured. Children always follow their parent in the list, so totals
    /// stay one reverse pass.
    pub fn swap(&self, id: NodeId) {
        let mut n = self.nodes.lock().unwrap();
        let Some(s) = n[id].staging.take() else {
            return;
        };
        let kids = std::mem::take(&mut n[s].children);
        for k in &kids {
            n[*k].parent = Some(id);
        }
        n[id].children = kids;
        n[id].own = std::mem::take(&mut n[s].own);
        n[id].track_to = n[s].track_to;
        n[id].folded = n[s].folded;
        n[id].state = State::Done;
    }

    /// The rows holding `path`, from the root down: the job whose folder
    /// contains it (the deepest, if jobs nest), then its kept folders by name
    /// as far as the tree goes. Empty when no job holds it.
    pub fn locate(&self, path: &Path, jobs: &HashMap<NodeId, PathBuf>) -> Vec<NodeId> {
        let Some((job, root)) = jobs
            .iter()
            .filter(|(_, p)| path.starts_with(p))
            .max_by_key(|(_, p)| p.components().count())
        else {
            return vec![];
        };
        let n = self.nodes.lock().unwrap();
        let mut chain = vec![];
        let mut at = Some(*job);
        while let Some(i) = at {
            chain.push(i);
            at = n[i].parent;
        }
        chain.reverse();
        let mut here = *job;
        for part in path
            .strip_prefix(root)
            .into_iter()
            .flat_map(Path::components)
        {
            let name = part.as_os_str().to_string_lossy();
            let Some(next) = n[here].children.iter().find(|c| n[**c].name == name) else {
                break;
            };
            chain.push(*next);
            here = *next;
        }
        chain
    }

    /// The folder `id` stands for: the nearest job's path (`jobs` maps job
    /// nodes to the paths walked into them) plus the names below it.
    pub fn path_from(&self, id: NodeId, jobs: &HashMap<NodeId, PathBuf>) -> Option<PathBuf> {
        let n = self.nodes.lock().unwrap();
        let mut names = vec![];
        let mut at = Some(id);
        while let Some(i) = at {
            if let Some(path) = jobs.get(&i) {
                let mut path = path.clone();
                path.extend(names.iter().rev());
                return Some(path);
            }
            names.push(n.get(i)?.name.clone());
            at = n[i].parent;
        }
        None
    }

    /// `focus` and `levels` levels below it, each node's children biggest
    /// first and capped at `top`.
    pub fn snapshot(&self, focus: NodeId, levels: usize, top: usize) -> View {
        let n = self.nodes.lock().unwrap();
        let mut own: Vec<u64> = n.iter().map(|x| x.own).collect();
        let mut total = own.clone();
        for i in (1..n.len()).rev() {
            if let Some(p) = n[i].parent {
                total[p] += total[i];
            }
        }
        if let Some((rem, budget)) = *self.budget.lock().unwrap() {
            let fill = budget.saturating_sub(total[Self::ROOT]);
            own[rem] = fill;
            let mut up = Some(rem);
            while let Some(i) = up {
                total[i] += fill;
                up = n[i].parent;
            }
        }
        view(&n, &own, &total, focus, levels, top)
    }
}

fn stage(n: &mut Vec<Node>, id: NodeId, levels: usize) -> NodeId {
    let (name, depth) = (n[id].name.clone(), n[id].depth);
    n.push(Node::new(
        name,
        None,
        depth,
        depth + levels,
        State::Measuring,
    ));
    let staging = n.len() - 1;
    n[id].staging = Some(staging);
    n[id].state = State::Measuring;
    staging
}

fn view(n: &[Node], own: &[u64], total: &[u64], id: NodeId, levels: usize, top: usize) -> View {
    let node = &n[id];
    // While its insides are measured again, a node shows the staged rows.
    let inside = node.staging.unwrap_or(id);
    let mut kids = n[inside].children.clone();
    kids.sort_by(|a, b| total[*b].cmp(&total[*a]).then(n[*a].name.cmp(&n[*b].name)));
    let cut = if levels == 0 { 0 } else { top.min(kids.len()) };
    let (shown, rest) = kids.split_at(cut);
    View {
        id,
        name: node.name.clone(),
        bytes: total[id],
        own_bytes: own[inside],
        state: state(n, id),
        children: shown
            .iter()
            .map(|c| view(n, own, total, *c, levels - 1, top))
            .collect(),
        other_count: rest.len(),
        other_bytes: rest.iter().map(|c| total[*c]).sum(),
        child_count: kids.len(),
        openable: !kids.is_empty() || node.folded || node.staging.is_some(),
    }
}

/// A node nobody walked (the root, a category) reports its children's state:
/// done when all are, pending when none started, measuring in between.
fn state(n: &[Node], id: NodeId) -> State {
    let node = &n[id];
    if node.state != State::Pending || node.children.is_empty() {
        return node.state;
    }
    let states: Vec<State> = node.children.iter().map(|c| state(n, *c)).collect();
    if states.iter().all(|s| *s == State::Done) {
        State::Done
    } else if states.iter().all(|s| *s == State::Pending) {
        State::Pending
    } else {
        State::Measuring
    }
}

impl Sink for Tree {
    type Node = NodeId;

    fn child(&self, parent: NodeId, name: &OsStr) -> Option<NodeId> {
        let mut n = self.nodes.lock().unwrap();
        let p = &n[parent];
        if p.depth >= p.track_to {
            n[parent].folded = true;
            return None;
        }
        let (depth, track_to) = (p.depth + 1, p.track_to);
        n.push(Node::new(
            name.to_string_lossy().into(),
            Some(parent),
            depth,
            track_to,
            State::Measuring,
        ));
        let id = n.len() - 1;
        n[parent].children.push(id);
        Some(id)
    }

    fn grow(&self, node: NodeId, bytes: u64) {
        self.nodes.lock().unwrap()[node].own += bytes;
    }

    fn done(&self, node: NodeId) {
        self.nodes.lock().unwrap()[node].state = State::Done;
    }
}
