//! Alpha-beta engine: iterative-deepening PVS over complete actions.
//!
//! Legal actions are produced lazily by the hypercuboid search under a heuristic per-axis
//! ordering, so the first actions generated combine the best semimove on each board. The TT
//! action is tried first; at non-PV nodes only a depth-dependent number of actions is searched.

use super::eval::{evaluate, piece_value, EvalParams};
use super::features::{MoveEvaluator, MoveWeights};
use super::{Engine, Limits, OptionValue};
use crate::core::action::{Action, FullMove, MoveSeq};
use crate::core::board::Board;
use crate::core::hypercuboid::{BestFirstCursor, Entry, HcCursor, HcInfo, SearchMode};
use crate::core::bitboard::*;
use crate::core::piece::*;
use crate::core::state::{Mark, State};
use crate::misc::integer_set::{Index, IntegerSet};
use crate::misc::geometry::SearchSpace;
use crate::misc::ordering::HcOrdering;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Legal actions of a node in heuristic order. With `merge`, each branch-count subspace gets
/// its own cursor and the best-scoring next action across them is returned, so time-travel
/// actions compete with the non-branching ones instead of always coming after all of them.
struct ActionGen {
    cursors: Vec<HcCursor>,
    heads: Vec<Option<(i32, MoveSeq)>>,
    filled: Vec<bool>,
    tail: HcCursor,
    best_first: Option<BestFirstCursor>,
}

impl ActionGen {
    fn new(ss: SearchSpace, order: HcOrdering, merge: bool) -> Self {
        Self::with_scores(ss, order, None, merge)
    }

    fn with_scores(ss: SearchSpace, order: HcOrdering, scores: Option<Vec<Vec<i32>>>, merge: bool) -> Self {
        let order = std::rc::Rc::new(order);
        if let Some(sc) = scores {
            let bf = BestFirstCursor::new(ss, order.clone(), std::rc::Rc::new(sc));
            let empty = HcCursor::with_shared(SearchSpace::default(), order, SearchMode::Balanced);
            return ActionGen { cursors: Vec::new(), heads: Vec::new(), filled: Vec::new(), tail: empty, best_first: Some(bf) };
        }
        // Subspaces are ordered [k branches, ..., 1 branch, 0 branches]. Only the 0- and
        // 1-branch ones are merged; finding a first legal action with several simultaneous
        // branches can be very expensive, so those come last, in one tail cursor.
        let mut tail = ss.hcs;
        let mut merged: Vec<HcCursor> = Vec::new();
        if merge && tail.len() >= 2 {
            let h0 = tail.pop().unwrap();
            let h1 = tail.pop().unwrap();
            merged.push(HcCursor::with_shared(SearchSpace::from_vec(vec![h1]), order.clone(), SearchMode::Balanced));
            merged.push(HcCursor::with_shared(SearchSpace::from_vec(vec![h0]), order.clone(), SearchMode::Balanced));
        }
        let tail_cursor = HcCursor::with_shared(SearchSpace::from_vec(tail), order, SearchMode::Balanced);
        let n = merged.len();
        ActionGen { cursors: merged, heads: vec![None; n], filled: vec![false; n], tail: tail_cursor, best_first: None }
    }

    fn next(&mut self, info: &HcInfo, s: &State, score: &mut dyn FnMut(&MoveSeq) -> i32) -> Option<(i32, MoveSeq)> {
        if let Some(bf) = &mut self.best_first {
            return bf.next(info, s).map(|a| (score(&a), a));
        }
        let mut best: Option<usize> = None;
        // Later cursors hold fewer branches: they win ties.
        for k in (0..self.cursors.len()).rev() {
            if !self.filled[k] {
                self.heads[k] = self.cursors[k].next(info, s).map(|a| (score(&a), a));
                self.filled[k] = true;
            }
            if let Some((sc, _)) = &self.heads[k] {
                if best.map(|b| *sc > self.heads[b].as_ref().unwrap().0).unwrap_or(true) {
                    best = Some(k);
                }
            }
        }
        match best {
            Some(k) => {
                self.filled[k] = false;
                self.heads[k].take()
            }
            None => self.tail.next(info, s).map(|a| (score(&a), a)),
        }
    }
}

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
    pub q_fastpath: bool,
    /// Interleave time-travel actions with non-branching ones by score.
    pub merge_branches: bool,
    /// Extend the search one ply when the side to move is in check.
    pub check_ext: bool,
    /// Order time travels that land with check first and other travels last.
    pub travel_checks: bool,
    /// Semimoves that give check go first on their axis: 0 off, 1 physical checks,
    /// 2 physical and superphysical checks.
    pub check_order: u8,
    /// Ordering bonus for a checking semimove.
    pub check_bonus: i32,
    /// Interior nodes with at most this remaining depth order checks first.
    pub check_order_depth: i32,
    /// Enumerate actions best-first by summed semimove scores (interior nodes and root).
    pub best_first: bool,
    /// Reverse futility pruning: at non-PV nodes with depth <= rfp_depth, not in check,
    /// return the static eval when it beats beta by rfp_margin per ply (0 = off).
    pub rfp_depth: i32,
    pub rfp_margin: i32,
    /// Evaluate with the feature HCE (`hce::tuned_weights`) instead of `eval::evaluate`.
    pub use_hce: bool,
    /// Order root actions by the static eval after them (plus the ordering score / 16)
    /// before the first iteration.
    pub root_static: bool,
    /// Quiescence enumerates only captures/promotions plus one quiet filler per board,
    /// best-first, so different captures are tried instead of one capture with many fillers.
    pub q_restrict: bool,
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
            q_fastpath: true,
            merge_branches: false,
            check_ext: false,
            travel_checks: false,
            check_order: 3,
            check_bonus: 30_000,
            check_order_depth: 64,
            best_first: true,
            rfp_depth: 0,
            rfp_margin: 200,
            use_hce: false,
            root_static: false,
            q_restrict: false,
        }
    }
}

pub struct AlphaBeta {
    pub params: AbParams,
    pub eval: EvalParams,
    pub hce: super::hce::Weights,
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

/// White-normalized piece on `sq` (same as `to_white(piece_name(b.get_piece(sq)))`), decoded
/// straight from the bitboards.
#[inline(always)]
fn kind_at(b: &Board, sq: i32) -> Piece {
    let z = 1u64 << sq;
    if b.occupied() & z == 0 {
        return NO_PIECE;
    }
    if b.lpawn() & z != 0 {
        return if b.lrawn() & z != 0 { BRAWN_W } else { PAWN_W };
    }
    if b.lknight() & z != 0 {
        return KNIGHT_W;
    }
    if b.lking() & z != 0 {
        return if b.royal() & z != 0 { KING_W } else { COMMON_KING_W };
    }
    let (r, bi, d) = (b.lrook() & z != 0, b.lbishop() & z != 0, b.ldragon() & z != 0);
    if r && d {
        return if b.royal() & z != 0 { ROYAL_QUEEN_W } else { QUEEN_W };
    }
    if r && bi {
        return PRINCESS_W;
    }
    if r {
        return ROOK_W;
    }
    if bi {
        return BISHOP_W;
    }
    if b.lunicorn() & z != 0 {
        return UNICORN_W;
    }
    if d {
        return DRAGON_W;
    }
    WALL_PIECE
}

/// (moving piece, captured piece), white-normalized; matches `moved_piece`/`captured_piece`.
#[inline]
fn mover_victim(s: &State, m: &FullMove) -> (Piece, Piece) {
    let player = s.get_present().1;
    let src = s.board(m.from.l(), m.from.t(), player);
    let mover = kind_at(src, m.from.xy());
    let victim = if m.from.tl() == m.to.tl() {
        if mover == KING_W && (m.from.x() - m.to.x()).abs() > 1 {
            NO_PIECE
        } else {
            let d = kind_at(src, m.to.xy());
            if d == NO_PIECE && (mover == PAWN_W || mover == BRAWN_W) && m.from.x() != m.to.x() {
                kind_at(src, ppos(m.to.x(), m.from.y()))
            } else {
                d
            }
        }
    } else {
        kind_at(s.board(m.to.l(), m.to.t(), player), m.to.xy())
    };
    (mover, victim)
}

/// Whether a superphysical move lands with the arriving piece attacking an enemy royal on its
/// destination board (physical attack only).
fn landing_check(s: &State, m: &FullMove) -> bool {
    let player = s.get_present().1;
    let src = s.board(m.from.l(), m.from.t(), player);
    let dst = s.board(m.to.l(), m.to.t(), player);
    let royals = dst.royal() & dst.hostile(player);
    if royals == 0 {
        return false;
    }
    let sq = m.to.xy();
    let occ = dst.occupied() | pmask(sq);
    let att = match kind_at(src, m.from.xy()) {
        PAWN_W | BRAWN_W => if player { black_pawn_attack(sq) } else { white_pawn_attack(sq) },
        KNIGHT_W => knight_attack(sq),
        KING_W | COMMON_KING_W => king_attack(sq),
        ROOK_W => rook_attack(sq, occ),
        BISHOP_W => bishop_attack(sq, occ),
        QUEEN_W | PRINCESS_W | ROYAL_QUEEN_W => queen_attack(sq, occ),
        _ => 0,
    };
    att & royals != 0
}

#[inline]
fn history_index_of(mover: Piece, m: &FullMove) -> usize {
    let pc = mover as usize;
    let d = m.to - m.from;
    let k = (pc * 64 + m.to.xy() as usize) * 16 + (((d.t() + 2).clamp(0, 3)) * 4 + (d.l().signum() + 1)) as usize;
    k & 0xffff
}

fn history_index(s: &State, m: &FullMove) -> usize {
    history_index_of(mover_victim(s, m).0, m)
}

impl AlphaBeta {
    pub fn new(params: AbParams, eval: EvalParams) -> Self {
        let entries = (params.tt_mb << 20) / 64;
        AlphaBeta {
            params,
            eval,
            hce: super::hce::tuned_weights(),
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
    fn static_eval(&self, s: &State) -> i32 {
        if self.params.use_hce {
            let _p = crate::misc::prof::scope(9);
            super::hce::evaluate(s, &self.hce)
        } else {
            evaluate(s, &self.eval)
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
    fn coord_score(&self, s: &State, info: &HcInfo, n: usize, i: Index, checks: &mut Option<MoveEvaluator>) -> i32 {
        let check = |checks: &mut Option<MoveEvaluator>| match checks {
            Some(ev) => {
                match self.params.check_order {
                    1 => ev.is_physical_check(n, i),
                    2 => ev.is_check(n, i),
                    _ => ev.is_check_fast(n, i),
                }
            }
            None => false,
        };
        match info.entry(n, i) {
            Entry::Null => -5,
            Entry::Departing { .. } => 0,
            Entry::Physical { m, .. } | Entry::Arriving { m, .. } => {
                let (mover, victim) = mover_victim(s, m);
                let mut sc = 0;
                if victim != NO_PIECE {
                    sc += 10_000 + 10 * piece_value(victim, &self.eval) - piece_value(mover, &self.eval) / 10;
                } else {
                    sc += self.history[history_index_of(mover, m)].clamp(-5000, 5000);
                }
                let wm = mover;
                if (wm == PAWN_W || wm == BRAWN_W) && (m.to.y() == 0 || m.to.y() == s.get_board_size().1 - 1) {
                    sc += 9000;
                }
                if m.from.tl() != m.to.tl() && victim == NO_PIECE {
                    sc -= 300;
                }
                if self.params.travel_checks && m.from.tl() != m.to.tl() {
                    sc += if landing_check(s, m) { 12_000 } else { -2_000 };
                }
                if wm == KING_W || wm == COMMON_KING_W {
                    sc -= 200;
                }
                if check(checks) {
                    sc += self.params.check_bonus;
                }
                sc
            }
        }
    }

    pub fn ordering(&self, s: &State, info: &HcInfo, checks: bool) -> HcOrdering {
        self.ordering_scores(s, info, checks).0
    }

    /// Per-axis order (best first) and each coordinate's score, indexed by coordinate.
    fn ordering_scores(&self, s: &State, info: &HcInfo, checks: bool) -> (HcOrdering, Vec<Vec<i32>>) {
        let _p = crate::misc::prof::scope(6);
        let u = info.get_universe();
        let mut orders = Vec::with_capacity(u.dimension());
        let mut scores: Vec<Vec<i32>> = Vec::with_capacity(u.dimension());
        // Branch axes share entries; score them once.
        let mut branch_cache: Option<(Vec<Index>, Vec<i32>)> = None;
        let mut checks = if checks && self.params.check_order > 0 { Some(MoveEvaluator::new(info, s, MoveWeights([0.0; 12]))) } else { None };
        for n in 0..u.dimension() {
            if n >= info.first_branch_axis() {
                if let Some((o, sc)) = &branch_cache {
                    orders.push(o.clone());
                    scores.push(sc.clone());
                    continue;
                }
            }
            let mut v: Vec<(i32, Index)> = u[n].iter().map(|i| (self.coord_score(s, info, n, i, &mut checks), i)).collect();
            let mut sc = vec![0i32; v.iter().map(|x| x.1 as usize + 1).max().unwrap_or(0)];
            for &(score, i) in &v {
                sc[i as usize] = score;
            }
            v.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            let o: Vec<Index> = v.into_iter().map(|x| x.1).collect();
            if n >= info.first_branch_axis() {
                branch_cache = Some((o.clone(), sc.clone()));
            }
            orders.push(o);
            scores.push(sc);
        }
        (HcOrdering::Precomputed(orders), scores)
    }

    /// Quiescence search space: per axis the captures and promotions, departures (for
    /// capturing travels), nulls, and the best quiet coordinate as filler.
    fn q_space(&self, s: &State, info: &HcInfo, ss: SearchSpace, order: &HcOrdering) -> SearchSpace {
        let u = info.get_universe();
        let h = s.get_board_size().1;
        let mut allowed: Vec<IntegerSet> = Vec::with_capacity(u.dimension());
        for n in 0..u.dimension() {
            if n > info.first_branch_axis() {
                let first = allowed[info.first_branch_axis()].clone();
                allowed.push(first);
                continue;
            }
            let mut set = IntegerSet::new();
            let mut filler = false;
            order.for_each(n, &u[n], |i| match info.entry(n, i) {
                Entry::Null | Entry::Departing { .. } => set.insert(i),
                Entry::Physical { m, .. } | Entry::Arriving { m, .. } => {
                    let (pc, victim) = mover_victim(s, m);
                    let promo = (pc == PAWN_W || pc == BRAWN_W) && (m.to.y() == 0 || m.to.y() == h - 1);
                    if victim != NO_PIECE || promo || !filler {
                        filler |= victim == NO_PIECE && !promo;
                        set.insert(i);
                    }
                }
            });
            allowed.push(set);
        }
        let mut out = SearchSpace::default();
        'hc: for mut hc in ss.hcs {
            for n in 0..hc.dimension() {
                hc[n].and_assign(&allowed[n]);
                if hc[n].is_empty() {
                    continue 'hc;
                }
            }
            out.hcs.push(hc);
        }
        out
    }

    /// Action generator for an interior node or the root.
    fn action_gen(&self, s: &State, info: &HcInfo, ss: SearchSpace, checks: bool) -> ActionGen {
        if self.params.best_first {
            let (order, scores) = self.ordering_scores(s, info, checks);
            ActionGen::with_scores(ss, order, Some(scores), self.params.merge_branches)
        } else {
            ActionGen::new(ss, self.ordering(s, info, checks), self.params.merge_branches)
        }
    }

    fn action_score(&self, s: &State, act: &MoveSeq, ply: usize) -> i32 {
        let mut sc = 0;
        for m in act {
            let (mover, victim) = mover_victim(s, m);
            if victim != NO_PIECE {
                sc += 10_000 + 10 * piece_value(victim, &self.eval) - piece_value(mover, &self.eval) / 10;
            } else {
                sc += self.history[history_index_of(mover, m)].clamp(-5000, 5000) / 4;
            }
            if m.from.tl() != m.to.tl() && victim == NO_PIECE {
                sc -= 300;
            }
            if self.params.travel_checks && m.from.tl() != m.to.tl() {
                sc += if landing_check(s, m) { 12_000 } else { -2_000 };
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

    /// The first `cap` actions of `s` in the order an interior node would try them (diagnostics).
    pub fn node_order(&self, s: &State, cap: usize) -> Vec<MoveSeq> {
        let (info, ss) = HcInfo::build_hc(s);
        let mut agen = self.action_gen(s, &info, ss, true);
        let mut out = Vec::new();
        let mut exhausted = false;
        while out.len() < cap && !exhausted {
            let mut batch: Vec<(i32, MoveSeq)> = Vec::new();
            while batch.len() < self.params.batch {
                match agen.next(&info, s, &mut |a| self.action_score(s, a, 1)) {
                    None => {
                        exhausted = true;
                        break;
                    }
                    Some(x) => batch.push(x),
                }
            }
            batch.sort_by(|a, b| b.0.cmp(&a.0));
            out.extend(batch.into_iter().map(|x| x.1));
        }
        out.truncate(cap);
        if let Ok(n) = std::env::var("CHECK_FIRST").map(|v| v.parse::<usize>().unwrap_or(0)) {
            // Experiment: stable-partition the first n actions into checking ones first.
            let n = n.min(out.len());
            let mut c = s.clone();
            c.enable_undo();
            let gives: Vec<bool> = out[..n]
                .iter()
                .map(|a| {
                    let m = Self::make(&mut c, a);
                    let g = Self::in_check(&c);
                    c.unmake(m);
                    g
                })
                .collect();
            let (mut chk, mut rest): (Vec<MoveSeq>, Vec<MoveSeq>) = (Vec::new(), Vec::new());
            for (a, g) in out.drain(..n).zip(gives) {
                if g { chk.push(a) } else { rest.push(a) }
            }
            chk.extend(rest);
            chk.extend(out.drain(..));
            out = chk;
        }
        out
    }

    fn is_tactical(s: &State, act: &MoveSeq) -> bool {
        let h = s.get_board_size().1;
        act.iter().any(|m| {
            let (pc, victim) = mover_victim(s, m);
            victim != NO_PIECE
                || ((pc == PAWN_W || pc == BRAWN_W) && (m.to.y() == 0 || m.to.y() == h - 1) && m.from.tl() == m.to.tl())
        })
    }

    /// Play an action generated as legal for `s`; undo with `s.unmake(mark)`.
    fn make(s: &mut State, act: &MoveSeq) -> Mark {
        let _p = crate::misc::prof::scope(7);
        let mark = s.mark();
        for &m in act {
            s.apply_move::<true>(m);
        }
        s.submit::<true>();
        mark
    }

    /// Play a remembered action (TT/killer) only if it is legal in `s`.
    fn make_checked(s: &mut State, act: &MoveSeq) -> Option<Mark> {
        let _p = crate::misc::prof::scope(11);
        let player = s.get_present().1;
        let mark = s.mark();
        let mut ok = true;
        for &m in act {
            if s.get_board_ptr(m.from.l(), m.from.t(), player).is_none()
                || s.get_timeline_end(m.from.l()) != (m.from.t(), player)
                || !s.apply_move::<false>(m)
            {
                ok = false;
                break;
            }
        }
        if ok && s.submit::<false>() && s.first_check(s.get_present().1).is_none() {
            return Some(mark);
        }
        s.unmake(mark);
        None
    }

    /// Owned copy of the position after `act` (for PV reporting).
    fn child(s: &State, act: &MoveSeq) -> State {
        let mut c = s.clone();
        for &m in act {
            c.apply_move::<true>(m);
        }
        c.submit::<true>();
        c
    }

    fn any_tactical(s: &State) -> bool {
        let player = s.get_present().1;
        let mut scratch = crate::core::multiverse::MoveList::new();
        let (l_min, l_max) = s.get_lines_range();
        for l in l_min..=l_max {
            let (t, c) = s.get_timeline_end(l);
            if c == player && s.multiverse().has_tactical_move(l, t, player, &mut scratch) {
                return true;
            }
        }
        false
    }

    fn in_check(s: &State) -> bool {
        let _p = crate::misc::prof::scope(10);
        s.has_phantom_check()
    }

    fn terminal_score(s: &State, ply: usize) -> i32 {
        if Self::in_check(s) { -(MATE - ply as i32) } else { 0 }
    }

    fn qsearch(&mut self, s: &mut State, ply: usize, mut alpha: i32, beta: i32, qd: i32) -> i32 {
        self.nodes += 1;
        if self.check_abort() {
            return 0;
        }
        let stand = self.static_eval(s);
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
        // Nothing to capture or promote anywhere: skip building the action space.
        if !in_check && self.params.q_fastpath && !Self::any_tactical(s) {
            return stand;
        }
        let (info, ss) = HcInfo::build_hc(s);
        let mut agen = if self.params.q_restrict && !in_check {
            let (order, scores) = self.ordering_scores(s, &info, false);
            let qs = self.q_space(s, &info, ss, &order);
            ActionGen::with_scores(qs, order, Some(scores), false)
        } else {
            ActionGen::new(ss, self.ordering(s, &info, false), self.params.merge_branches)
        };
        let mut best = if in_check { -INF } else { stand };
        let mut tried = 0;
        let mut any = false;
        while tried < self.params.qcap {
            if self.check_abort() {
                return 0;
            }
            let Some((_, act)) = agen.next(&info, s, &mut |a| self.action_score(s, a, ply)) else { break };
            any = true;
            if !in_check && !Self::is_tactical(s, &act) {
                break;
            }
            tried += 1;
            let mark = Self::make(s, &act);
            let v = -self.qsearch(s, ply + 1, -beta, -alpha, qd - 1);
            s.unmake(mark);
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

    fn negamax(&mut self, s: &mut State, mut depth: i32, ply: usize, mut alpha: i32, beta: i32, pv: bool) -> i32 {
        if self.params.check_ext && ply < MAX_PLY / 2 && Self::in_check(s) {
            depth += 1;
        }
        if depth <= 0 {
            return self.qsearch(s, ply, alpha, beta, self.params.qdepth);
        }
        self.nodes += 1;
        if self.check_abort() {
            return 0;
        }
        if ply >= MAX_PLY {
            return self.static_eval(s);
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

        if !pv && depth <= self.params.rfp_depth && beta.abs() < MATE - 1000 && !Self::in_check(s) {
            let e = self.static_eval(s);
            if e - self.params.rfp_margin * depth >= beta {
                return e;
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
            if let Some(mark) = Self::make_checked(s, act) {
                let v = -self.negamax(s, depth - 1, ply + 1, -beta, -alpha, pv);
                s.unmake(mark);
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
        let mut agen = self.action_gen(s, &info, ss, depth <= self.params.check_order_depth);
        let mut generated = 0usize;
        let mut exhausted = false;
        while searched < cap && !exhausted {
            let mut batch: Vec<(i32, MoveSeq)> = Vec::with_capacity(self.params.batch);
            while batch.len() < self.params.batch {
                if self.check_abort() {
                    return 0;
                }
                match agen.next(&info, s, &mut |a| self.action_score(s, a, ply)) {
                    None => {
                        exhausted = true;
                        break;
                    }
                    Some((sc, act)) => {
                        generated += 1;
                        if tt_action.as_ref() == Some(&act) {
                            continue;
                        }
                        batch.push((sc, act));
                    }
                }
            }
            batch.sort_by(|a, b| b.0.cmp(&a.0));
            for (sc, act) in batch {
                if searched >= cap {
                    break;
                }
                let mark = Self::make(s, &act);
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
                    v = -self.negamax(s, depth - 1, ply + 1, -beta, -alpha, pv);
                } else {
                    v = -self.negamax(s, depth - 1 - r, ply + 1, -alpha - 1, -alpha, false);
                    if !self.aborted && v > alpha && r > 0 {
                        v = -self.negamax(s, depth - 1, ply + 1, -alpha - 1, -alpha, false);
                    }
                    if !self.aborted && v > alpha && v < beta && pv {
                        v = -self.negamax(s, depth - 1, ply + 1, -beta, -alpha, true);
                    }
                }
                s.unmake(mark);
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
            return self.static_eval(s);
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
            if Self::make_checked(&mut s, &act).is_none() {
                break;
            }
            line.push(act);
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
            ("q_fastpath", _) => self.params.q_fastpath = !matches!(value, OptionValue::Bool(false)),
            ("merge_branches", _) => self.params.merge_branches = !matches!(value, OptionValue::Bool(false)),
            ("check_ext", _) => self.params.check_ext = !matches!(value, OptionValue::Bool(false)),
            ("travel_checks", _) => self.params.travel_checks = !matches!(value, OptionValue::Bool(false)),
            ("batch", Some(x)) => self.params.batch = (x as usize).max(1),
            ("pv_cap_mult", Some(x)) => self.params.pv_cap_mult = x as usize,
            ("check_order", Some(x)) => self.params.check_order = x as u8,
            ("check_bonus", Some(x)) => self.params.check_bonus = x as i32,
            ("check_order_depth", Some(x)) => self.params.check_order_depth = x as i32,
            ("rfp_depth", Some(x)) => self.params.rfp_depth = x as i32,
            ("rfp_margin", Some(x)) => self.params.rfp_margin = x as i32,
            ("q_restrict", _) => self.params.q_restrict = !matches!(value, OptionValue::Bool(false)),
            ("root_static", _) => self.params.root_static = !matches!(value, OptionValue::Bool(false)),
            ("use_hce", _) => self.params.use_hce = !matches!(value, OptionValue::Bool(false)),
            ("best_first", _) => self.params.best_first = !matches!(value, OptionValue::Bool(false)),
            ("root_cap", Some(x)) => self.params.root_cap = (x as usize).max(1),
            ("lmr_min_depth", Some(x)) => self.params.lmr_min_depth = x as i32,
            ("lmr_min_index", Some(x)) => self.params.lmr_min_index = x as usize,
            (k, Some(x)) if k.starts_with("lmp") && k[3..].parse::<usize>().is_ok() => {
                let i = k[3..].parse::<usize>().unwrap();
                if i >= self.params.lmp.len() {
                    let last = *self.params.lmp.last().unwrap();
                    self.params.lmp.resize(i + 1, last);
                }
                self.params.lmp[i] = x as usize;
            }
            ("Hash", Some(x)) => {
                self.params.tt_mb = x as usize;
                let entries = ((x as usize) << 20) / 64;
                self.tt = vec![None; entries.next_power_of_two()];
            }
            ("inactive_weight", Some(x)) => self.eval.inactive_weight = x as i32,
            ("timeline_slot", Some(x)) => self.eval.timeline_slot = x as i32,
            ("timeline_slot_scaled", Some(x)) => self.eval.timeline_slot_scaled = x as i32,
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
        let mut work = root.clone();
        work.enable_undo();

        // Root actions (all of them, up to the root cap), ordered heuristically.
        let (hc, ss) = HcInfo::build_hc(root);
        let mut agen = self.action_gen(root, &hc, ss, true);
        let mut roots: Vec<(i32, MoveSeq)> = Vec::new();
        // Enumerating every root action can take seconds with many timelines: spend at most
        // a quarter of the budget on it (always keeping at least one action).
        let gen_deadline = budget.map(|ms| start + Duration::from_millis(ms / 4));
        while roots.len() < self.params.root_cap {
            if !roots.is_empty() && gen_deadline.map(|d| Instant::now() >= d).unwrap_or(false) {
                break;
            }
            match agen.next(&hc, root, &mut |a| self.action_score(root, a, 0)) {
                Some(entry) => roots.push(entry),
                None => break,
            }
        }
        if roots.is_empty() {
            return None;
        }
        if self.params.root_static {
            for (sc, act) in roots.iter_mut() {
                let mark = Self::make(&mut work, act);
                *sc = -self.static_eval(&work) + *sc / 16;
                work.unmake(mark);
            }
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
                let mark = Self::make(&mut work, act);
                let v = if k == 0 {
                    -self.negamax(&mut work, depth - 1, 1, -beta, -alpha, true)
                } else {
                    let mut v = -self.negamax(&mut work, depth - 1, 1, -alpha - 1, -alpha, false);
                    if !self.aborted && v > alpha {
                        v = -self.negamax(&mut work, depth - 1, 1, -beta, -alpha, true);
                    }
                    v
                };
                work.unmake(mark);
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
