//! The live tree behind the storage detail view.
//!
//! Nodes are appended, never removed, and a parent always comes before its
//! children, so subtree totals are one reverse pass over the list. Each
//! walked folder keeps a fixed number of levels below it; deeper folders fold
//! into their last kept ancestor, which bounds memory on a whole-disk walk.
//! Sizes only ever grow, so successive snapshots never show a total shrink.

use crate::walker::Sink;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
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
            nodes: Mutex::new(vec![Node {
                name: name.into(),
                parent: None,
                depth: 0,
                track_to: 0,
                children: vec![],
                own: 0,
                state: State::Pending,
            }]),
            budget: Mutex::new(None),
        }
    }

    /// A node the scan places itself (a category, a folder to walk), which
    /// keeps `levels` levels of folders below it once walked.
    pub fn add(&self, parent: NodeId, name: &str, levels: usize) -> NodeId {
        let mut n = self.nodes.lock().unwrap();
        let depth = n[parent].depth + 1;
        n.push(Node {
            name: name.into(),
            parent: Some(parent),
            depth,
            track_to: depth + levels,
            children: vec![],
            own: 0,
            state: State::Pending,
        });
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

    /// Whether `id` names a node. Ids arrive from the view, so check first.
    pub fn has(&self, id: NodeId) -> bool {
        id < self.nodes.lock().unwrap().len()
    }

    pub fn set_state(&self, id: NodeId, state: State) {
        self.nodes.lock().unwrap()[id].state = state;
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

fn view(n: &[Node], own: &[u64], total: &[u64], id: NodeId, levels: usize, top: usize) -> View {
    let node = &n[id];
    let mut kids = node.children.clone();
    kids.sort_by(|a, b| total[*b].cmp(&total[*a]).then(n[*a].name.cmp(&n[*b].name)));
    let cut = if levels == 0 { 0 } else { top.min(kids.len()) };
    let (shown, rest) = kids.split_at(cut);
    View {
        id,
        name: node.name.clone(),
        bytes: total[id],
        own_bytes: own[id],
        state: state(n, id),
        children: shown
            .iter()
            .map(|c| view(n, own, total, *c, levels - 1, top))
            .collect(),
        other_count: rest.len(),
        other_bytes: rest.iter().map(|c| total[*c]).sum(),
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
            return None;
        }
        let (depth, track_to) = (p.depth + 1, p.track_to);
        n.push(Node {
            name: name.to_string_lossy().into(),
            parent: Some(parent),
            depth,
            track_to,
            children: vec![],
            own: 0,
            state: State::Measuring,
        });
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
