//! Semimove-level search tree used by MCTS (arena-allocated).
//!
//! A *nodal* node owns a hypercuboid context (a submitted position); its descendants pick one
//! coordinate per axis of that context. The node on the last axis is a *ceiling*: its path is a
//! complete legal action, and it becomes nodal itself when ignited.

use crate::core::action::MoveSeq;
use crate::core::hypercuboid::HcInfo;
use crate::core::state::State;
use crate::misc::geometry::{Hc, Point, SearchSpace, Slice};
use crate::misc::integer_set::Index;
use crate::misc::ordering::HcOrdering;

pub type NodeId = u32;
type CellId = u32;
type CtxId = u32;

pub struct Context {
    pub state: State,
    pub hc_info: HcInfo,
    pub verified_terminal: bool,
}

struct Cell {
    parent: Option<CellId>,
    space: Hc,
    subspace: SearchSpace,
}

pub struct Node<T> {
    parent: Option<NodeId>,
    own_ctx: Option<CtxId>,
    ctx: Option<CtxId>,
    pub n: Index,
    pub i: Index,
    cells: Vec<CellId>,
    next_cell: usize,
    children: Vec<NodeId>,
    pub info: T,
}

pub struct FineTree<T> {
    nodes: Vec<Node<T>>,
    cells: Vec<Cell>,
    ctxs: Vec<Context>,
}

impl<T: Default> FineTree<T> {
    pub fn new_root(s: &State) -> Self {
        let mut t = FineTree { nodes: Vec::new(), cells: Vec::new(), ctxs: Vec::new() };
        t.nodes.push(Node {
            parent: None,
            own_ctx: None,
            ctx: None,
            n: Index::MAX - 6,
            i: Index::MAX - 6,
            cells: Vec::new(),
            next_cell: 0,
            children: Vec::new(),
            info: T::default(),
        });
        t.ignite_impl(0, s);
        t
    }

    pub const ROOT: NodeId = 0;

    #[inline]
    pub fn node(&self, id: NodeId) -> &Node<T> {
        &self.nodes[id as usize]
    }
    #[inline]
    pub fn info(&self, id: NodeId) -> &T {
        &self.nodes[id as usize].info
    }
    #[inline]
    pub fn info_mut(&mut self, id: NodeId) -> &mut T {
        &mut self.nodes[id as usize].info
    }
    #[inline]
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id as usize].parent
    }
    #[inline]
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.nodes[id as usize].children
    }
    #[inline]
    pub fn is_nodal(&self, id: NodeId) -> bool {
        self.nodes[id as usize].own_ctx.is_some()
    }
    #[inline]
    fn context_of(&self, id: NodeId) -> CtxId {
        let n = &self.nodes[id as usize];
        n.own_ctx.or(n.ctx).unwrap()
    }
    /// The context this node's search refers to (its own when nodal).
    pub fn hc_info(&self, id: NodeId) -> &HcInfo {
        &self.ctxs[self.context_of(id) as usize].hc_info
    }
    /// The state this node's context was built from.
    pub fn state(&self, id: NodeId) -> &State {
        &self.ctxs[self.context_of(id) as usize].state
    }
    pub fn player(&self, id: NodeId) -> bool {
        self.hc_info(id).player()
    }
    pub fn is_ceiling(&self, id: NodeId) -> bool {
        let n = &self.nodes[id as usize];
        match n.ctx {
            None => false,
            Some(c) => n.n as usize + 1 == self.ctxs[c as usize].hc_info.dimension(),
        }
    }
    pub fn get_child(&self, id: NodeId, i: Index) -> Option<NodeId> {
        self.nodes[id as usize].children.iter().copied().find(|&c| self.nodes[c as usize].i == i)
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    /// Children sharing a semimove with an earlier sibling (should be zero).
    pub fn duplicate_children(&self) -> usize {
        let mut d = 0;
        for n in &self.nodes {
            for (k, &c) in n.children.iter().enumerate() {
                let i = self.nodes[c as usize].i;
                if n.children[..k].iter().any(|&o| self.nodes[o as usize].i == i) {
                    d += 1;
                }
            }
        }
        d
    }

    fn ignite_impl(&mut self, id: NodeId, s: &State) {
        let (hc_info, ss) = HcInfo::build_hc(s);
        let universe = hc_info.get_universe().clone();
        let ctx = self.ctxs.len() as CtxId;
        self.ctxs.push(Context { state: s.clone(), hc_info, verified_terminal: false });
        let cell = self.cells.len() as CellId;
        self.cells.push(Cell { parent: None, space: universe, subspace: ss });
        let n = &mut self.nodes[id as usize];
        n.own_ctx = Some(ctx);
        n.cells.clear();
        n.cells.push(cell);
        n.next_cell = 0;
    }

    /// Make a ceiling node nodal by applying its action.
    pub fn ignite(&mut self, id: NodeId) {
        let s = self.state_at_ceiling(id);
        self.ignite_impl(id, &s);
    }

    /// State after the action of a ceiling node (the nodal state if already ignited).
    pub fn state_at_ceiling(&self, id: NodeId) -> State {
        let _p = crate::misc::prof::scope(7);
        if self.is_nodal(id) {
            return self.state(id).clone();
        }
        let ctx = self.nodes[id as usize].ctx.unwrap();
        let mut s = self.ctxs[ctx as usize].state.clone();
        for m in self.to_action(id) {
            s.apply_move::<true>(m);
        }
        s.submit::<true>();
        s
    }

    pub fn to_action(&self, id: NodeId) -> MoveSeq {
        let ctx = self.nodes[id as usize].ctx.expect("to_action on root");
        let info = &self.ctxs[ctx as usize].hc_info;
        let dim = info.dimension();
        let mut pt = vec![0 as Index; dim];
        let mut cur = id;
        for _ in 0..dim {
            let n = &self.nodes[cur as usize];
            pt[n.n as usize] = n.i;
            cur = n.parent.unwrap();
        }
        info.to_action(&pt)
    }

    fn add_cell(&mut self, node: NodeId, parent: Option<CellId>, space: Hc) -> CellId {
        let id = self.cells.len() as CellId;
        let subspace = SearchSpace::from_vec(vec![space.clone()]);
        self.cells.push(Cell { parent, space, subspace });
        self.nodes[node as usize].cells.push(id);
        id
    }

    fn add_child(&mut self, node: NodeId, n: Index, i: Index) -> NodeId {
        let ctx = Some(self.context_of(node));
        let id = self.nodes.len() as NodeId;
        self.nodes.push(Node {
            parent: Some(node),
            own_ctx: None,
            ctx,
            n,
            i,
            cells: Vec::new(),
            next_cell: 0,
            children: Vec::new(),
            info: T::default(),
        });
        self.nodes[node as usize].children.push(id);
        id
    }

    /// Whether no legal action exists from this nodal node (may add children while checking).
    pub fn is_terminal(&mut self, id: NodeId, order: &HcOrdering) -> bool {
        if !self.is_nodal(id) {
            return false;
        }
        let ctx = self.nodes[id as usize].own_ctx.unwrap();
        if self.ctxs[ctx as usize].verified_terminal {
            return true;
        }
        if !self.nodes[id as usize].children.is_empty() {
            return false;
        }
        if self.expand(id, order).is_some() {
            return false;
        }
        self.ctxs[ctx as usize].verified_terminal = true;
        true
    }

    /// Find the next unexplored semimove below `id`; returns the new child.
    pub fn expand(&mut self, id: NodeId, order: &HcOrdering) -> Option<NodeId> {
        let (pt, cell) = self.explore(id, order)?;
        let final_node = self.isolate(id, &pt, cell);
        Some(self.normalize(id, &pt, cell, final_node))
    }

    fn explore(&mut self, id: NodeId, order: &HcOrdering) -> Option<(Point, CellId)> {
        let ctx = self.context_of(id) as usize;
        let mut consecutive_empty = 0;
        loop {
            let ncells = self.nodes[id as usize].cells.len();
            if ncells == 0 || consecutive_empty >= ncells {
                return None;
            }
            let node = &mut self.nodes[id as usize];
            if node.next_cell >= ncells {
                node.next_cell = 0;
            }
            let cell = node.cells[node.next_cell];
            node.next_cell += 1;
            let c = &mut self.cells[cell as usize];
            if c.subspace.is_empty() {
                consecutive_empty += 1;
                continue;
            }
            consecutive_empty = 0;
            let info = &self.ctxs[ctx].hc_info;
            let st = &self.ctxs[ctx].state;
            let hc = c.subspace.back();
            let pt = match info.take_point(hc, order) {
                Some(p) => p,
                None => {
                    c.subspace.pop_back();
                    continue;
                }
            };
            match info.find_problem(st, &pt, hc) {
                Some(problem) => self.remove_problem(id, &problem, cell),
                None => return Some((pt, cell)),
            }
        }
    }

    fn remove_from_cell(&mut self, s: &Slice, cell: CellId, force: bool) {
        let c = &mut self.cells[cell as usize];
        if c.space.intersects(s) {
            c.subspace.remove_slice_backwards(s, force, 2, 1);
        }
    }

    fn remove_from_node(&mut self, s: &Slice, node: NodeId, critical: CellId, force: bool) {
        self.remove_from_cell(s, critical, force);
        for k in 0..self.nodes[node as usize].cells.len() {
            let c = self.nodes[node as usize].cells[k];
            if c != critical {
                self.remove_from_cell(s, c, false);
            }
        }
    }

    fn remove_problem(&mut self, id: NodeId, s: &Slice, origin: CellId) {
        let problem_ctx = self.context_of(id);
        self.remove_from_node(s, id, origin, true);
        if self.is_nodal(id) {
            return;
        }
        if !self.cells[origin as usize].space.is_slice_good(s, 1) {
            return;
        }
        let mut ancestor = self.nodes[id as usize].parent;
        let mut path_cell = self.cells[origin as usize].parent;
        while let (Some(a), Some(pc)) = (ancestor, path_cell) {
            if self.context_of(a) != problem_ctx {
                break;
            }
            if !self.cells[pc as usize].space.is_slice_good(s, 1) {
                break;
            }
            self.remove_from_node(s, a, pc, false);
            if self.is_nodal(a) {
                break;
            }
            ancestor = self.nodes[a as usize].parent;
            path_cell = self.cells[pc as usize].parent;
        }
    }

    fn first_axis(&self, id: NodeId) -> usize {
        if self.is_nodal(id) { 0 } else { self.nodes[id as usize].n as usize + 1 }
    }

    fn isolate(&mut self, id: NodeId, p: &Point, target_cell: CellId) -> NodeId {
        let dim = self.ctxs[self.context_of(id) as usize].hc_info.dimension();
        let mut current_node = id;
        let mut current_cell = target_cell;
        let mut current_hc = self.cells[target_cell as usize].subspace.len() - 1;
        let mut next_n = self.first_axis(id);
        while next_n < dim {
            let next_i = p[next_n];
            let next_node = self.add_child(current_node, next_n as Index, next_i);
            let (with_i, without_i) = self.cells[current_cell as usize].subspace.hcs[current_hc].split(next_n, next_i);
            let next_cell = self.add_cell(next_node, Some(current_cell), with_i);
            let cc = &mut self.cells[current_cell as usize];
            cc.subspace.hcs[current_hc] = without_i;
            cc.subspace.prune_empty();
            current_node = next_node;
            current_cell = next_cell;
            current_hc = self.cells[current_cell as usize].subspace.len() - 1;
            next_n += 1;
        }
        current_node
    }

    /// Move points sharing the new path's prefix from sibling HCs of the target cell into the
    /// matching descendants' cells. Each piece goes to the node of its own axis; attaching
    /// it one level higher would create duplicate children.
    fn normalize(&mut self, id: NodeId, p: &Point, target_cell: CellId, final_node: NodeId) -> NodeId {
        let dim = self.ctxs[self.context_of(id) as usize].hc_info.dimension();
        let mut path_nodes: Vec<Option<NodeId>> = vec![None; dim + 1];
        let mut cur = final_node;
        while cur != id {
            let n = &self.nodes[cur as usize];
            path_nodes[n.n as usize + 1] = Some(cur);
            cur = n.parent.unwrap();
        }
        let start = self.first_axis(id);
        path_nodes[start] = Some(id);
        let count = self.cells[target_cell as usize].subspace.len();
        for h in 0..count {
            let mut current_cell = target_cell;
            let mut current_hc = h;
            let mut next_n = start;
            while next_n < dim {
                let next_i = p[next_n];
                if !self.cells[current_cell as usize].subspace.hcs[current_hc][next_n].contains(next_i) {
                    break;
                }
                let owner = path_nodes[next_n + 1].unwrap();
                let (with_i, without_i) = self.cells[current_cell as usize].subspace.hcs[current_hc].split(next_n, next_i);
                let next_cell = self.add_cell(owner, Some(current_cell), with_i);
                self.cells[current_cell as usize].subspace.hcs[current_hc] = without_i;
                current_cell = next_cell;
                current_hc = self.cells[current_cell as usize].subspace.len() - 1;
                next_n += 1;
            }
        }
        path_nodes[start + 1].unwrap()
    }
}

/// Follow first children down to a ceiling node.
pub fn nearby_ceiling<T: Default>(t: &FineTree<T>, mut id: NodeId) -> Option<NodeId> {
    while !t.is_ceiling(id) {
        id = *t.children(id).first()?;
    }
    Some(id)
}
