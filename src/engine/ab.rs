//! Alpha-beta engine: iterative-deepening PVS over complete actions.
//!
//! Legal actions are produced lazily by the hypercuboid search under a heuristic per-axis
//! ordering, so the first actions generated combine the best semimove on each board. The TT
//! action is tried first; at non-PV nodes only a depth-dependent number of actions is searched.

use super::eval::{evaluate, piece_value, EvalParams};
use super::{Engine, Limits, OptionValue};
use crate::core::action::{Action, FullMove, MoveSeq};
use crate::core::hypercuboid::{Entry, HcCursor, HcInfo, SearchMode};
use crate::core::piece::*;
use crate::core::state::State;
use crate::misc::integer_set::Index;
use crate::misc::ordering::HcOrdering;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const MATE: i32 = 30000;
pub const INF: i32 = 32000;
const MAX_PLY: usize = 128;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bound {
    Exact,
    Lower,
    Upper,
}

#[derive(Clone)]
struct TtEntry {
    key: u64,
    depth: i16,
    bound: Bound,
    score: i32,
    action: MoveSeq,
}

#[derive(Clone, Debug)]
pub struct AbParams {
    /// Actions searched at non-PV nodes by remaining depth (index = depth, last repeats).
    pub lmp: Vec<usize>,
    /// Multiplier for the PV-node action cap.
    pub pv_cap_mult: usize,
    pub root_cap: usize,
    pub batch: usize,
    pub qdepth: i32,
    pub qcap: usize,
    pub lmr_min_depth: i32,
    pub lmr_min_index: usize,
    pub tt_mb: usize,
}

impl Default for AbParams {
    fn default() -> Self {
        AbParams {
            lmp: vec![0, 16, 24, 32, 40],
            pv_cap_mult: 3,
            root_cap: 4000,
            batch: 12,
            qdepth: 3,
            qcap: 10,
            lmr_min_depth: 3,
            lmr_min_index: 4,
            tt_mb: 64,
        }
    }
}

pub struct AlphaBeta {
    pub params: AbParams,
    pub eval: EvalParams,
    tt: Vec<Option<TtEntry>>,
    history: Vec<i32>,
    killers: Vec<[Option<MoveSeq>; 2]>,
    nodes: u64,
    deadline: Option<Instant>,
    node_limit: Option<u64>,
    aborted: bool,
    stop_ptr: *const AtomicBool,
}

// The raw stop pointer is only dereferenced during `find_best_move`, while the borrow it
// came from is alive.
unsafe impl Send for AlphaBeta {}

#[inline]
fn mate_to_tt(score: i32, ply: usize) -> i32 {
    if score > MATE - 1000 {
        score + ply as i32
    } else if score < -MATE + 1000 {
        score - ply as i32
    } else {
        score
    }
}
#[inline]
fn mate_from_tt(score: i32, ply: usize) -> i32 {
    if score > MATE - 1000 {
        score - ply as i32
    } else if score < -MATE + 1000 {
        score + ply as i32
    } else {
        score
    }
}

fn history_index(s: &State, m: &FullMove) -> usize {
    let pc = to_white(piece_name(s.get_piece(m.from, s.get_present().1))) as usize;
    let d = m.to - m.from;
    let k = (pc * 64 + m.to.xy() as usize) * 16 + (((d.t() + 2).clamp(0, 3)) * 4 + (d.l().signum() + 1)) as usize;
    k & 0xffff
}

impl AlphaBeta {
    pub fn new(params: AbParams, eval: EvalParams) -> Self {
        let entries = (params.tt_mb << 20) / 64;
        AlphaBeta {
            params,
            eval,
            tt: vec![None; entries.next_power_of_two()],
            history: vec![0; 1 << 16],
            killers: vec![[None, None]; MAX_PLY + 1],
            nodes: 0,
            deadline: None,
            node_limit: None,
            aborted: false,
            stop_ptr: std::ptr::null(),
        }
    }

    #[inline]
    fn tt_slot(&self, key: u64) -> usize {
        (key as usize) & (self.tt.len() - 1)
    }

    fn tt_probe(&self, key: u64) -> Option<&TtEntry> {
        self.tt[self.tt_slot(key)].as_ref().filter(|e| e.key == key)
    }

    fn tt_store(&mut self, key: u64, depth: i32, bound: Bound, score: i32, action: &MoveSeq) {
        let slot = self.tt_slot(key);
        let replace = match &self.tt[slot] {
            None => true,
            Some(e) => e.key != key || depth as i16 >= e.depth - 1 || bound == Bound::Exact,
        };
        if replace {
            self.tt[slot] = Some(TtEntry { key, depth: depth as i16, bound, score, action: action.clone() });
        }
    }

    fn check_abort(&mut self) -> bool {
        if self.aborted {
            return true;
        }
        // Nodes cost tens of microseconds, so reading the clock every node is free.
        {
            let stop = unsafe { &*self.stop_ptr };
            if stop.load(Ordering::Relaxed)
                || self.deadline.map(|d| Instant::now() >= d).unwrap_or(false)
                || self.node_limit.map(|n| self.nodes >= n).unwrap_or(false)
            {
                self.aborted = true;
            }
        }
        self.aborted
    }

    /// Ordering score of one HC coordinate.
    fn coord_score(&self, s: &State, info: &HcInfo, n: usize, i: Index) -> i32 {
        let player = s.get_present().1;
        match info.entry(n, i) {
            Entry::Null => -5,
            Entry::Departing { .. } => 0,
            Entry::Physical { m, .. } | Entry::Arriving { m, .. } => {
                let mover = s.get_piece(m.from, player);
                let victim = m.captured_piece(s);
                let mut sc = 0;
                if victim != NO_PIECE {
                    sc += 10_000 + 10 * piece_value(victim, &self.eval) - piece_value(mover, &self.eval) / 10;
                } else {
                    sc += self.history[history_index(s, m)].clamp(-5000, 5000);
                }
                let wm = to_white(piece_name(mover));
                if (wm == PAWN_W || wm == BRAWN_W) && (m.to.y() == 0 || m.to.y() == s.get_board_size().1 - 1) {
                    sc += 9000;
                }
                if m.from.tl() != m.to.tl() && victim == NO_PIECE {
                    sc -= 300;
                }
                if wm == KING_W || wm == COMMON_KING_W {
                    sc -= 200;
                }
                sc
            }
        }
    }

    fn ordering(&self, s: &State, info: &HcInfo) -> HcOrdering {
        let u = info.get_universe();
        let mut orders = Vec::with_capacity(u.dimension());
        // Branch axes share entries; score them once.
        let mut branch_cache: Option<Vec<Index>> = None;
        for n in 0..u.dimension() {
            if n >= info.first_branch_axis() {
                if let Some(c) = &branch_cache {
                    orders.push(c.clone());
                    continue;
                }
            }
            let mut v: Vec<(i32, Index)> = u[n].iter().map(|i| (self.coord_score(s, info, n, i), i)).collect();
            v.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            let o: Vec<Index> = v.into_iter().map(|x| x.1).collect();
            if n >= info.first_branch_axis() {
                branch_cache = Some(o.clone());
            }
            orders.push(o);
        }
        HcOrdering::Precomputed(orders)
    }

    fn action_score(&self, s: &State, act: &MoveSeq, ply: usize) -> i32 {
        let mut sc = 0;
        let player = s.get_present().1;
        for m in act {
            let victim = m.captured_piece(s);
            if victim != NO_PIECE {
                sc += 10_000 + 10 * piece_value(victim, &self.eval) - piece_value(s.get_piece(m.from, player), &self.eval) / 10;
            } else {
                sc += self.history[history_index(s, m)].clamp(-5000, 5000) / 4;
            }
            if m.from.tl() != m.to.tl() && victim == NO_PIECE {
                sc -= 300;
            }
        }
        if ply <= MAX_PLY {
            if self.killers[ply][0].as_ref() == Some(act) {
                sc += 8000;
            } else if self.killers[ply][1].as_ref() == Some(act) {
                sc += 7000;
            }
        }
        sc
    }

    fn is_tactical(s: &State, act: &MoveSeq) -> bool {
        let h = s.get_board_size().1;
        act.iter().any(|m| {
            m.captured_piece(s) != NO_PIECE || {
                let pc = to_white(piece_name(s.get_piece(m.from, s.get_present().1)));
                (pc == PAWN_W || pc == BRAWN_W) && (m.to.y() == 0 || m.to.y() == h - 1) && m.from.tl() == m.to.tl()
            }
        })
    }

    /// Apply an action that was generated as legal for `s`.
    fn child(s: &State, act: &MoveSeq) -> State {
        let mut c = s.clone();
        for &m in act {
            c.apply_move::<true>(m);
        }
        c.submit::<true>();
        c
    }

    /// Validate a remembered action (TT/killer) in `s`.
    fn legal_child(s: &State, act: &MoveSeq) -> Option<State> {
        let player = s.get_present().1;
        let mut c = s.clone();
        for &m in act {
            if c.get_board_ptr(m.from.l(), m.from.t(), player).is_none() || c.get_timeline_end(m.from.l()) != (m.from.t(), player) {
                return None;
            }
            if !c.apply_move::<false>(m) {
                return None;
            }
        }
        if !c.submit::<false>() || c.first_check(c.get_present().1).is_some() {
            return None;
        }
        Some(c)
    }

    fn in_check(s: &State) -> bool {
        s.has_phantom_check()
    }

    fn terminal_score(s: &State, ply: usize) -> i32 {
        if Self::in_check(s) { -(MATE - ply as i32) } else { 0 }
    }

    fn qsearch(&mut self, s: &State, ply: usize, mut alpha: i32, beta: i32, qd: i32) -> i32 {
        self.nodes += 1;
        if self.check_abort() {
            return 0;
        }
        let stand = evaluate(s, &self.eval);
        let in_check = Self::in_check(s);
        // Royal captures are threatened: standing pat is unsound, so search evasions.
        if !in_check {
            if stand >= beta {
                return stand;
            }
            alpha = alpha.max(stand);
        }
        if qd <= 0 || ply >= MAX_PLY {
            return if in_check { stand } else { alpha };
        }
        let (info, ss) = HcInfo::build_hc(s);
        let order = self.ordering(s, &info);
        let mut cursor = HcCursor::new(ss, order, SearchMode::Balanced);
        let mut best = if in_check { -INF } else { stand };
        let mut tried = 0;
        let mut any = false;
        while tried < self.params.qcap {
            if self.check_abort() {
                return 0;
            }
            let Some(act) = cursor.next(&info, s) else { break };
            any = true;
            if !in_check && !Self::is_tactical(s, &act) {
                break;
            }
            tried += 1;
            let c = Self::child(s, &act);
            let v = -self.qsearch(&c, ply + 1, -beta, -alpha, qd - 1);
            if self.aborted {
                return 0;
            }
            if v > best {
                best = v;
                if v > alpha {
                    alpha = v;
                    if v >= beta {
                        break;
                    }
                }
            }
        }
        if !any {
            return Self::terminal_score(s, ply);
        }
        best
    }

    fn negamax(&mut self, s: &State, depth: i32, ply: usize, mut alpha: i32, beta: i32, pv: bool) -> i32 {
        if depth <= 0 {
            return self.qsearch(s, ply, alpha, beta, self.params.qdepth);
        }
        self.nodes += 1;
        if self.check_abort() {
            return 0;
        }
        if ply >= MAX_PLY {
            return evaluate(s, &self.eval);
        }
        // Mate distance pruning.
        let alpha0 = alpha;
        let key = s.hash();
        let mut tt_action: Option<MoveSeq> = None;
        if let Some(e) = self.tt_probe(key) {
            let sc = mate_from_tt(e.score, ply);
            if !pv && e.depth as i32 >= depth {
                match e.bound {
                    Bound::Exact => return sc,
                    Bound::Lower if sc >= beta => return sc,
                    Bound::Upper if sc <= alpha => return sc,
                    _ => {}
                }
            }
            if !e.action.is_empty() {
                tt_action = Some(e.action.clone());
            }
        }

        let mut best = -INF;
        let mut best_action: MoveSeq = Vec::new();
        let mut searched = 0usize;
        let cap = {
            let lmp = &self.params.lmp;
            let base = lmp[(depth as usize).min(lmp.len() - 1)];
            if pv { base * self.params.pv_cap_mult } else { base }
        };

        // Remembered action first.
        if let Some(act) = &tt_action {
            if let Some(c) = Self::legal_child(s, act) {
                let v = -self.negamax(&c, depth - 1, ply + 1, -beta, -alpha, pv);
                if self.aborted {
                    return 0;
                }
                searched += 1;
                best = v;
                best_action = act.clone();
                if v > alpha {
                    alpha = v;
                }
                if alpha >= beta {
                    self.on_cutoff(s, act, depth, ply);
                    self.tt_store(key, depth, Bound::Lower, mate_to_tt(best, ply), &best_action);
                    return best;
                }
            }
        }

        let (info, ss) = HcInfo::build_hc(s);
        let order = self.ordering(s, &info);
        let mut cursor = HcCursor::new(ss, order, SearchMode::Balanced);
        let mut generated = 0usize;
        let mut exhausted = false;
        while searched < cap && !exhausted {
            let mut batch: Vec<(i32, MoveSeq)> = Vec::with_capacity(self.params.batch);
            while batch.len() < self.params.batch {
                if self.check_abort() {
                    return 0;
                }
                match cursor.next(&info, s) {
                    None => {
                        exhausted = true;
                        break;
                    }
                    Some(act) => {
                        generated += 1;
                        if tt_action.as_ref() == Some(&act) {
                            continue;
                        }
                        let sc = self.action_score(s, &act, ply);
                        batch.push((sc, act));
                    }
                }
            }
            batch.sort_by(|a, b| b.0.cmp(&a.0));
            for (sc, act) in batch {
                if searched >= cap {
                    break;
                }
                let c = Self::child(s, &act);
                let quiet = sc < 10_000;
                let mut r = 0;
                if depth >= self.params.lmr_min_depth && searched >= self.params.lmr_min_index && quiet {
                    r = 1 + (searched >= 12) as i32;
                    if pv {
                        r -= 1;
                    }
                }
                let mut v;
                if searched == 0 {
                    v = -self.negamax(&c, depth - 1, ply + 1, -beta, -alpha, pv);
                } else {
                    v = -self.negamax(&c, depth - 1 - r, ply + 1, -alpha - 1, -alpha, false);
                    if !self.aborted && v > alpha && r > 0 {
                        v = -self.negamax(&c, depth - 1, ply + 1, -alpha - 1, -alpha, false);
                    }
                    if !self.aborted && v > alpha && v < beta && pv {
                        v = -self.negamax(&c, depth - 1, ply + 1, -beta, -alpha, true);
                    }
                }
                if self.aborted {
                    return 0;
                }
                searched += 1;
                if v > best {
                    best = v;
                    best_action = act.clone();
                    if v > alpha {
                        alpha = v;
                    }
                }
                if alpha >= beta {
                    self.on_cutoff(s, &act, depth, ply);
                    self.tt_store(key, depth, Bound::Lower, mate_to_tt(best, ply), &best_action);
                    return best;
                }
            }
        }
        if searched == 0 && generated == 0 && tt_action.is_none() {
            return Self::terminal_score(s, ply);
        }
        if searched == 0 {
            // Only the TT action existed but failed validation; fall back to static eval.
            return evaluate(s, &self.eval);
        }
        let bound = if best > alpha0 { Bound::Exact } else { Bound::Upper };
        self.tt_store(key, depth, bound, mate_to_tt(best, ply), &best_action);
        best
    }

    fn on_cutoff(&mut self, s: &State, act: &MoveSeq, depth: i32, ply: usize) {
        if Self::is_tactical(s, act) {
            return;
        }
        for m in act {
            let h = &mut self.history[history_index(s, m)];
            *h += depth * depth;
            if *h > 20000 {
                for x in self.history.iter_mut() {
                    *x /= 2;
                }
            }
        }
        if ply <= MAX_PLY && self.killers[ply][0].as_ref() != Some(act) {
            self.killers[ply][1] = self.killers[ply][0].take();
            self.killers[ply][0] = Some(act.clone());
        }
    }

    fn pv_line(&self, root: &State, first: &MoveSeq, max: usize) -> Vec<MoveSeq> {
        let mut line = vec![first.clone()];
        let mut s = Self::child(root, first);
        while line.len() < max {
            let Some(e) = self.tt_probe(s.hash()) else { break };
            let act = e.action.clone();
            if act.is_empty() {
                break;
            }
            let Some(c) = Self::legal_child(&s, &act) else { break };
            line.push(act);
            s = c;
        }
        line
    }
}

fn action_text(s: &State, act: &MoveSeq) -> String {
    let a = Action::from_moveseq(act, s);
    let mut os = s.clone();
    let mut parts = Vec::new();
    for em in a.moves() {
        parts.push(em.lan(&os));
        if let Some(n) = os.normalize_promotion(*em) {
            os.apply_ext::<true>(n);
        }
    }
    parts.join(" ")
}

impl Engine for AlphaBeta {
    fn name(&self) -> String {
        "ab".into()
    }

    fn new_game(&mut self) {
        for e in self.tt.iter_mut() {
            *e = None;
        }
        for h in self.history.iter_mut() {
            *h = 0;
        }
    }

    fn set_option(&mut self, key: &str, value: &OptionValue) -> bool {
        let v = value.as_f64();
        match (key, v) {
            ("qdepth", Some(x)) => self.params.qdepth = x as i32,
            ("qcap", Some(x)) => self.params.qcap = x as usize,
            ("batch", Some(x)) => self.params.batch = (x as usize).max(1),
            ("pv_cap_mult", Some(x)) => self.params.pv_cap_mult = x as usize,
            ("Hash", Some(x)) => {
                self.params.tt_mb = x as usize;
                let entries = ((x as usize) << 20) / 64;
                self.tt = vec![None; entries.next_power_of_two()];
            }
            ("inactive_weight", Some(x)) => self.eval.inactive_weight = x as i32,
            ("timeline_slot", Some(x)) => self.eval.timeline_slot = x as i32,
            ("tempo", Some(x)) => self.eval.tempo = x as i32,
            _ => return false,
        }
        true
    }

    fn find_best_move(&mut self, root: &State, limits: &Limits, stop: &AtomicBool, info: &mut dyn FnMut(String)) -> Option<Action> {
        let start = Instant::now();
        self.stop_ptr = stop as *const AtomicBool;
        self.nodes = 0;
        self.aborted = false;
        self.node_limit = limits.nodes;
        let budget = limits.movetime.or_else(|| {
            limits.time_left.map(|t| (t / 25 + limits.increment.unwrap_or(0) * 3 / 4).min(t / 3))
        });
        self.deadline = budget.map(|ms| start + Duration::from_millis(ms.saturating_sub(5).max(1)));
        for k in self.killers.iter_mut() {
            *k = [None, None];
        }
        let max_depth = limits.depth.unwrap_or(64) as i32;

        // Root actions (all of them, up to the root cap), ordered heuristically.
        let (hc, ss) = HcInfo::build_hc(root);
        let order = self.ordering(root, &hc);
        let mut cursor = HcCursor::new(ss, order, SearchMode::Balanced);
        let mut roots: Vec<(i32, MoveSeq)> = Vec::new();
        // Enumerating every root action can take seconds with many timelines: spend at most
        // a quarter of the budget on it (always keeping at least one action).
        let gen_deadline = budget.map(|ms| start + Duration::from_millis(ms / 4));
        while roots.len() < self.params.root_cap {
            if !roots.is_empty() && gen_deadline.map(|d| Instant::now() >= d).unwrap_or(false) {
                break;
            }
            match cursor.next(&hc, root) {
                Some(a) => {
                    let sc = self.action_score(root, &a, 0);
                    roots.push((sc, a));
                }
                None => break,
            }
        }
        if roots.is_empty() {
            return None;
        }
        roots.sort_by(|a, b| b.0.cmp(&a.0));
        let mut best: MoveSeq = roots[0].1.clone();
        let mut best_score = 0;
        for depth in 1..=max_depth {
            let mut alpha = -INF;
            let beta = INF;
            let mut iter_best: Option<(i32, MoveSeq)> = None;
            let mut scores: Vec<i32> = Vec::with_capacity(roots.len());
            for (k, (_, act)) in roots.iter().enumerate() {
                let c = Self::child(root, act);
                let v = if k == 0 {
                    -self.negamax(&c, depth - 1, 1, -beta, -alpha, true)
                } else {
                    let mut v = -self.negamax(&c, depth - 1, 1, -alpha - 1, -alpha, false);
                    if !self.aborted && v > alpha {
                        v = -self.negamax(&c, depth - 1, 1, -beta, -alpha, true);
                    }
                    v
                };
                if self.aborted {
                    break;
                }
                scores.push(v);
                if v > alpha {
                    alpha = v;
                    iter_best = Some((v, act.clone()));
                }
            }
            if let Some((v, act)) = iter_best {
                // A partially searched iteration still improves on the previous best.
                best = act;
                best_score = v;
            }
            let elapsed = start.elapsed().as_secs_f64();
            if !self.aborted {
                // Reorder roots: best first, rest by this iteration's scores.
                let mut idx: Vec<usize> = (0..roots.len()).collect();
                idx.sort_by(|&a, &b| scores.get(b).copied().unwrap_or(-INF).cmp(&scores.get(a).copied().unwrap_or(-INF)));
                roots = idx.into_iter().map(|i| roots[i].clone()).collect();
                let pv = self.pv_line(root, &best, depth as usize);
                let score_txt = if best_score > MATE - 1000 {
                    format!("mate {}", (MATE - best_score + 1) / 2)
                } else if best_score < -MATE + 1000 {
                    format!("mate -{}", (MATE + best_score + 1) / 2)
                } else {
                    format!("cp {}", best_score)
                };
                let mut st = root.clone();
                let mut pv_txt = Vec::new();
                for a in &pv {
                    pv_txt.push(format!("{} submit", action_text(&st, a)));
                    st = Self::child(&st, a);
                }
                info(format!(
                    "depth {} score {} nodes {} nps {:.0} time {} roots {} pv {}",
                    depth,
                    score_txt,
                    self.nodes,
                    self.nodes as f64 / elapsed.max(1e-6),
                    (elapsed * 1000.0) as u64,
                    roots.len(),
                    pv_txt.join(" ")
                ));
            }
            if self.aborted || best_score.abs() > MATE - 1000 {
                break;
            }
            if let Some(d) = self.deadline {
                // Do not start an iteration that cannot finish.
                let spent = start.elapsed();
                let left = d.saturating_duration_since(Instant::now());
                if left < spent {
                    break;
                }
            }
        }
        Some(Action::from_moveseq(&best, root))
    }
}
