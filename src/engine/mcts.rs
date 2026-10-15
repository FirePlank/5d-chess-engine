//! Monte Carlo tree search engines: UCT / progressive widening over the semimove tree,
//! random or weighted rollouts, flat UCB and a random mover.

use super::features::{self, MoveEvaluator, MoveWeights, Weights};
use super::finetree::{nearby_ceiling, FineTree, NodeId};
use super::{Engine, Limits, OptionValue};
use crate::core::action::{Action, MoveSeq};
use crate::core::hypercuboid::{HcInfo, SearchMode};
use crate::core::state::{MateType, State};
use crate::misc::ordering::HcOrdering;
use crate::misc::rng::Mt19937;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

pub const EXPLORATION: f32 = 1.4142135623730951;
pub const DEFAULT_ROLLOUT_MAX_ACTIONS: usize = 200;
pub const DEFAULT_TEMPERATURE: f32 = 1200.0;

pub fn uct(sum: f32, visits: u64, parent_visits: u64, maximizing: bool) -> f32 {
    if visits == 0 {
        return if maximizing { f32::INFINITY } else { f32::NEG_INFINITY };
    }
    let avg = sum / visits as f32;
    let explore = EXPLORATION * ((parent_visits as f32 + 1.0).ln() / visits as f32).sqrt();
    if maximizing { avg + explore } else { avg - explore }
}

#[derive(Default)]
pub struct NodeInfo {
    pub sum_reward: f32,
    pub visits: u64,
    pub registered: bool,
    all_children_included: bool,
    fully_expanded: bool,
    registered_children: usize,
    ordering: Option<HcOrdering>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OrderKind {
    Random,
    Scored(MoveWeights),
}
impl PartialEq for MoveWeights {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}
impl std::fmt::Debug for MoveWeights {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TreeKind {
    Uct,
    Pw { constant: f64, alpha: f64 },
}

#[derive(Clone, Copy, Debug)]
pub enum Selection {
    Random,
    Weighted,
}

#[derive(Clone, Copy, Debug)]
pub enum Cutoff {
    /// Draw score at the rollout limit.
    Zero,
    /// Linear evaluation at the rollout limit.
    Linear(&'static str),
}

#[derive(Clone, Debug)]
pub enum DefaultKind {
    Rollout { selection: Selection, cutoff: Cutoff },
    /// No simulation; terminal positions keep their outcome.
    ZeroPosition,
}

pub struct Rollout {
    pub kind: DefaultKind,
    pub max_actions: usize,
    pub temperature: f32,
    pub weights: Weights,
    rng: Option<Mt19937>,
}

fn fallback_rng() -> &'static mut Mt19937 {
    thread_local! {
        static RNG: std::cell::UnsafeCell<Mt19937> = std::cell::UnsafeCell::new(Mt19937::from_entropy());
    }
    // SAFETY: thread-local and never borrowed re-entrantly.
    RNG.with(|r| unsafe { &mut *r.get() })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Terminal {
    WhiteWins,
    BlackWins,
    Stalemate,
    ActionLimit,
}

pub struct Reward {
    pub score: f32,
    pub end: Terminal,
    pub num_actions: usize,
}

fn mate_reward(winner: Option<bool>, n: usize) -> Reward {
    match winner {
        Some(true) => Reward { score: -1.0, end: Terminal::BlackWins, num_actions: n },
        Some(false) => Reward { score: 1.0, end: Terminal::WhiteWins, num_actions: n },
        None => Reward { score: 0.0, end: Terminal::Stalemate, num_actions: n },
    }
}

fn mate_winner(s: &State) -> Option<bool> {
    if s.get_mate_type() == MateType::Checkmate { Some(!s.get_present().1) } else { None }
}

impl Rollout {
    pub fn new(kind: DefaultKind, max_actions: usize, temperature: f32, weights: Weights, seed: Option<u32>) -> Self {
        Rollout { kind, max_actions, temperature, weights, rng: seed.map(Mt19937::new) }
    }

    fn rng(&mut self) -> &mut Mt19937 {
        match &mut self.rng {
            Some(r) => r,
            None => fallback_rng(),
        }
    }

    pub fn select_action(&mut self, s: &State, selection: Selection, stop: &AtomicBool) -> Option<MoveSeq> {
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        let (info, space) = HcInfo::build_hc(s);
        let order = match selection {
            Selection::Random => HcOrdering::random(info.get_universe(), self.rng()),
            Selection::Weighted => {
                let t = self.temperature;
                let u = info.get_universe();
                let mut ev = MoveEvaluator::new(&info, s, MoveWeights::default_rollout());
                let mut w = Vec::with_capacity(u.dimension());
                for axis in 0..u.dimension() {
                    w.push(u[axis].iter().map(|c| (ev.score(axis, c) / t).exp()).collect::<Vec<f32>>());
                }
                HcOrdering::weighted(u, w, self.rng())
            }
        };
        info.search_with(s, space, order, SearchMode::Mixed).next()
    }

    pub fn evaluate(&mut self, mut s: State, stop: &AtomicBool) -> Option<Reward> {
        let (selection, cutoff) = match self.kind {
            DefaultKind::ZeroPosition => {
                if stop.load(Ordering::Relaxed) {
                    return None;
                }
                let (info, space) = HcInfo::build_hc(&s);
                let any = info.search_with(&s, space, HcOrdering::Natural, SearchMode::Mixed).next();
                return Some(match any {
                    None => mate_reward(mate_winner(&s), 0),
                    Some(_) => Reward { score: 0.0, end: Terminal::ActionLimit, num_actions: 0 },
                });
            }
            DefaultKind::Rollout { selection, cutoff } => (selection, cutoff),
        };
        let mut n = 0;
        while n < self.max_actions {
            if stop.load(Ordering::Relaxed) {
                return None;
            }
            let Some(action) = self.select_action(&s, selection, stop) else {
                if stop.load(Ordering::Relaxed) {
                    return None;
                }
                return Some(mate_reward(mate_winner(&s), n));
            };
            let ap = crate::misc::prof::scope(8);
            for m in action {
                s.apply_move::<true>(m);
            }
            s.submit::<true>();
            drop(ap);
            n += 1;
        }
        let score = match cutoff {
            Cutoff::Zero => 0.0,
            Cutoff::Linear(_) => features::linear_evaluate(&s, &self.weights),
        };
        Some(Reward { score, end: Terminal::ActionLimit, num_actions: n })
    }
}

pub struct Mcts {
    pub name: String,
    tree_kind: TreeKind,
    order_kind: OrderKind,
    rng: Option<Mt19937>,
    rollout: Rollout,
}

struct Stats {
    conclusive: u64,
    inconclusive: u64,
    terminal: u64,
}

impl Mcts {
    pub fn new(name: &str, tree_kind: TreeKind, order_kind: OrderKind, rollout: Rollout, seed: Option<u32>) -> Self {
        Mcts { name: name.into(), tree_kind, order_kind, rng: seed.map(Mt19937::new), rollout }
    }

    fn rng(&mut self) -> &mut Mt19937 {
        match &mut self.rng {
            Some(r) => r,
            None => fallback_rng(),
        }
    }

    fn action_root(t: &FineTree<NodeInfo>, mut id: NodeId) -> NodeId {
        while let Some(p) = t.parent(id) {
            if t.is_nodal(id) {
                break;
            }
            id = p;
        }
        id
    }

    fn with_order(&mut self, t: &mut FineTree<NodeInfo>, id: NodeId) -> HcOrdering {
        match self.order_kind {
            OrderKind::Random => {
                let u = t.hc_info(id).get_universe().clone();
                HcOrdering::random(&u, self.rng())
            }
            OrderKind::Scored(w) => {
                let root = Self::action_root(t, id);
                if let Some(o) = &t.info(root).ordering {
                    return o.clone();
                }
                let info = t.hc_info(root);
                let scores = MoveEvaluator::new(info, t.state(root), w).score_table();
                let u = info.get_universe().clone();
                let o = HcOrdering::scored(&u, &scores, self.rng());
                t.info_mut(root).ordering = Some(o.clone());
                o
            }
        }
    }

    fn first_ordered_child(t: &FineTree<NodeInfo>, id: NodeId, order: &HcOrdering, unregistered_only: bool) -> Option<NodeId> {
        let ch = t.children(id);
        let first = *ch.first()?;
        let mut cand = crate::misc::integer_set::IntegerSet::new();
        for &c in ch {
            if !unregistered_only || !t.info(c).registered {
                cand.insert(t.node(c).i);
            }
        }
        let axis = t.node(first).n as usize;
        let coord = order.first(axis, &cand)?;
        t.get_child(id, coord)
    }

    fn best_uct_child(t: &FineTree<NodeInfo>, id: NodeId) -> Option<NodeId> {
        let maximizing = !t.player(id);
        let pv = t.info(id).visits;
        let mut best = None;
        let mut best_score = if maximizing { f32::NEG_INFINITY } else { f32::INFINITY };
        for &c in t.children(id) {
            let i = t.info(c);
            if !i.registered || i.visits == 0 {
                continue;
            }
            let s = uct(i.sum_reward, i.visits, pv, maximizing);
            if best.is_none() || if maximizing { s > best_score } else { s < best_score } {
                best = Some(c);
                best_score = s;
            }
        }
        best
    }

    fn widening_limit(&self, visits: u64) -> usize {
        match self.tree_kind {
            TreeKind::Pw { constant, alpha } => {
                let raw = (constant * (visits as f64).powf(alpha)).ceil();
                if raw >= usize::MAX as f64 { usize::MAX } else { (raw as usize).max(1) }
            }
            TreeKind::Uct => usize::MAX,
        }
    }

    fn register(t: &mut FineTree<NodeInfo>, parent: NodeId, child: NodeId) {
        if !t.info(child).registered {
            t.info_mut(child).registered = true;
            t.info_mut(parent).registered_children += 1;
        }
    }

    fn expand(&mut self, t: &mut FineTree<NodeInfo>, id: NodeId, stop: &AtomicBool) -> Option<NodeId> {
        if t.info(id).fully_expanded || stop.load(Ordering::Relaxed) {
            return None;
        }
        if !t.info(id).all_children_included {
            if !t.children(id).is_empty() || matches!(self.tree_kind, TreeKind::Uct) {
                let order = self.with_order(t, id);
                if let Some(c) = Self::first_ordered_child(t, id, &order, true) {
                    Self::register(t, id, c);
                    return Some(c);
                }
            }
            t.info_mut(id).all_children_included = true;
        }
        if t.is_ceiling(id) && !t.is_nodal(id) {
            t.ignite(id);
        }
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        let order = self.with_order(t, id);
        if let Some(c) = t.expand(id, &order) {
            let i = t.node(c).i;
            let c = t.get_child(id, i).unwrap();
            Self::register(t, id, c);
            t.info_mut(id).all_children_included = false;
            return Some(c);
        }
        t.info_mut(id).fully_expanded = true;
        None
    }

    fn select(&mut self, t: &mut FineTree<NodeInfo>, stop: &AtomicBool) -> Option<NodeId> {
        let mut id = FineTree::<NodeInfo>::ROOT;
        t.info_mut(id).registered = true;
        while !stop.load(Ordering::Relaxed) {
            let widen = match self.tree_kind {
                TreeKind::Uct => true,
                TreeKind::Pw { .. } => {
                    let i = t.info(id);
                    !i.fully_expanded && i.registered_children < self.widening_limit(i.visits)
                }
            };
            if widen {
                if let Some(c) = self.expand(t, id, stop) {
                    return Some(c);
                }
            }
            match Self::best_uct_child(t, id) {
                None => return Some(id),
                Some(c) => id = c,
            }
        }
        None
    }

    fn complete_to_ceiling(&mut self, t: &mut FineTree<NodeInfo>, mut id: NodeId, stop: &AtomicBool) -> Option<NodeId> {
        while !stop.load(Ordering::Relaxed) {
            if t.is_ceiling(id) {
                return Some(id);
            }
            if t.children(id).is_empty() {
                let order = self.with_order(t, id);
                t.expand(id, &order)?;
                t.info_mut(id).all_children_included = false;
            }
            let order = self.with_order(t, id);
            id = Self::first_ordered_child(t, id, &order, false)?;
        }
        None
    }

    fn most_visited(t: &FineTree<NodeInfo>) -> Option<NodeId> {
        let mut id = FineTree::<NodeInfo>::ROOT;
        while !t.is_ceiling(id) {
            let mut best: Option<NodeId> = None;
            for &c in t.children(id) {
                let v = t.info(c).visits;
                if v > 0 && best.map(|b| v > t.info(b).visits).unwrap_or(true) {
                    best = Some(c);
                }
            }
            match best {
                None => return nearby_ceiling(t, id),
                Some(b) => id = b,
            }
        }
        Some(id)
    }
}

impl Engine for Mcts {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn set_option(&mut self, key: &str, value: &OptionValue) -> bool {
        match (key, value) {
            ("rollout-max-actions", OptionValue::Int(v)) => {
                self.rollout.max_actions = (*v).max(0) as usize;
                true
            }
            ("weight-temperature", OptionValue::Float(v)) if *v > 0.0 && v.is_finite() => {
                self.rollout.temperature = *v as f32;
                true
            }
            _ => false,
        }
    }

    fn find_best_move(&mut self, root: &State, limits: &Limits, stop: &AtomicBool, info: &mut dyn FnMut(String)) -> Option<Action> {
        let start = Instant::now();
        let iteration_limit = limits.depth.map(|d| d as u64 * 10).or(limits.nodes);
        let deadline = limits.movetime.map(|ms| start + std::time::Duration::from_millis(ms));
        let mut t: FineTree<NodeInfo> = FineTree::new_root(root);
        let mut it = 0u64;
        let mut st = Stats { conclusive: 0, inconclusive: 0, terminal: 0 };
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            if iteration_limit.map(|l| it >= l).unwrap_or(false) {
                break;
            }
            if deadline.map(|d| Instant::now() >= d).unwrap_or(false) {
                break;
            }
            let Some(node) = self.select(&mut t, stop) else { break };
            let Some(ceiling) = self.complete_to_ceiling(&mut t, node, stop) else { break };
            let s = t.state_at_ceiling(ceiling);
            let Some(r) = self.rollout.evaluate(s, stop) else { break };
            let mut cur = Some(ceiling);
            while let Some(c) = cur {
                let i = t.info_mut(c);
                i.sum_reward += r.score;
                i.visits += 1;
                cur = t.parent(c);
            }
            match r.end {
                Terminal::ActionLimit => st.inconclusive += 1,
                _ if r.num_actions == 0 => st.terminal += 1,
                _ => st.conclusive += 1,
            }
            it += 1;
        }
        let best = Self::most_visited(&t);
        let secs = start.elapsed().as_secs_f64();
        info(format!(
            "mcts_stats elapsed_seconds={} iterations={} ips={} conclusive_rollouts={} inconclusive_rollouts={} terminal_tree_evaluations={} tree_nodes={} duplicate_children={}",
            secs,
            it,
            if secs > 0.0 { it as f64 / secs } else { 0.0 },
            st.conclusive,
            st.inconclusive,
            st.terminal,
            t.node_count(),
            t.duplicate_children()
        ));
        let best = best?;
        let mut scores = Vec::new();
        let mut cur = best;
        while let Some(p) = t.parent(cur) {
            let i = t.info(cur);
            if i.visits > 0 {
                scores.push(i.sum_reward / i.visits as f32);
            }
            cur = p;
        }
        scores.reverse();
        let avg = if scores.is_empty() { 0.0 } else { scores.iter().map(|&x| x as f64).sum::<f64>() / scores.len() as f64 };
        info(format!(
            "mcts_score average={} detailed={}",
            avg,
            scores.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(":")
        ));
        Some(Action::from_moveseq(&t.to_action(best), root))
    }
}

/// Flat UCB over root actions with repeated rollouts.
pub struct FlatUcb {
    pub name: String,
    rollout: Rollout,
}

impl FlatUcb {
    pub fn new(name: &str, rollout: Rollout) -> Self {
        FlatUcb { name: name.into(), rollout }
    }
}

impl Engine for FlatUcb {
    fn name(&self) -> String {
        self.name.clone()
    }
    fn set_option(&mut self, key: &str, value: &OptionValue) -> bool {
        match (key, value) {
            ("rollout-max-actions", OptionValue::Int(v)) => {
                self.rollout.max_actions = (*v).max(0) as usize;
                true
            }
            ("weight-temperature", OptionValue::Float(v)) if *v > 0.0 => {
                self.rollout.temperature = *v as f32;
                true
            }
            _ => false,
        }
    }
    fn find_best_move(&mut self, root: &State, limits: &Limits, stop: &AtomicBool, info: &mut dyn FnMut(String)) -> Option<Action> {
        let start = Instant::now();
        let (hc, ss) = HcInfo::build_hc(root);
        let mut children: Vec<(MoveSeq, State, f32, u64)> = hc
            .search(root, ss)
            .map(|mv| {
                let mut p = root.clone();
                for &m in &mv {
                    p.apply_move::<true>(m);
                }
                p.submit::<false>();
                (mv, p, 0.0, 0)
            })
            .collect();
        if children.is_empty() {
            return None;
        }
        let limit = limits.depth.map(|d| d as u64 * 10).or(limits.nodes);
        let deadline = limits.movetime.map(|ms| start + std::time::Duration::from_millis(ms));
        let maximizing = !root.get_present().1;
        let mut total = 0u64;
        while !stop.load(Ordering::Relaxed)
            && limit.map(|l| total < l).unwrap_or(true)
            && deadline.map(|d| Instant::now() < d).unwrap_or(true)
        {
            let mut sel = 0;
            let mut best = if maximizing { f32::NEG_INFINITY } else { f32::INFINITY };
            for (i, c) in children.iter().enumerate() {
                let s = uct(c.2, c.3, total, maximizing);
                if (maximizing && s > best) || (!maximizing && s < best) {
                    sel = i;
                    best = s;
                }
            }
            let Some(r) = self.rollout.evaluate(children[sel].1.clone(), stop) else { break };
            children[sel].2 += r.score;
            children[sel].3 += 1;
            total += 1;
        }
        let mut bi = 0;
        for i in 0..children.len() {
            if children[i].3 > children[bi].3 {
                bi = i;
            }
        }
        let secs = start.elapsed().as_secs_f64();
        info(format!("flat_ucb_stats elapsed_seconds={} iterations={} ips={}", secs, total, total as f64 / secs.max(1e-9)));
        let c = &children[bi];
        info(format!("flat_ucb_score score={}", if c.3 > 0 { c.2 / c.3 as f32 } else { 0.0 }));
        Some(Action::from_moveseq(&c.0, root))
    }
}

/// Plays a random legal action.
pub struct Monkey {
    rng: Option<Mt19937>,
}
impl Monkey {
    pub fn new(seed: Option<u32>) -> Self {
        Monkey { rng: seed.map(Mt19937::new) }
    }
}
impl Engine for Monkey {
    fn name(&self) -> String {
        "monkey".into()
    }
    fn set_option(&mut self, _: &str, _: &OptionValue) -> bool {
        false
    }
    fn find_best_move(&mut self, s: &State, _: &Limits, stop: &AtomicBool, _: &mut dyn FnMut(String)) -> Option<Action> {
        let (w, ss) = HcInfo::build_hc(s);
        let rng = match &mut self.rng {
            Some(r) => r,
            None => fallback_rng(),
        };
        let order = HcOrdering::random(w.get_universe(), rng);
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        let mv = w.search_with(s, ss, order, SearchMode::Iterative).next()?;
        Some(Action::from_moveseq(&mv, s))
    }
}
