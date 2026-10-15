//! Alpha-beta engine: iterative-deepening PVS over complete actions.
//!
//! Legal actions are produced lazily by the hypercuboid search, best-first by the summed
//! ordering scores of their semimoves, with checking semimoves (physical, and across time
//! and timelines) ordered first. The TT action and the killers are tried before the action
//! space is built; at non-PV nodes only a depth-dependent number of actions is searched.
//! Quiescence searches captures and promotions only, one quiet filler move per board.

use super::hce;
use super::{Limits, OptionValue};
use crate::core::action::{Action, FullMove, MoveSeq};
use crate::core::bitboard::pmask;
use crate::core::board::Board;
use crate::core::check_position::{BoardOverlay, CheckKind, CheckPosition};
use crate::core::hypercuboid::{has_physical_check, BestFirstCursor, Entry, HcCursor, HcInfo};
use crate::core::multiverse::next_turn;
use crate::core::piece::*;
use crate::core::state::{Mark, State};
use crate::misc::geometry::SearchSpace;
use crate::misc::integer_set::Index;
use crate::misc::ordering::HcOrdering;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const MATE: i32 = 30000;
pub const INF: i32 = 32000;
const MAX_PLY: usize = 128;

/// Actions searched at non-PV nodes by remaining depth (index = depth, last repeats).
const LMP: [usize; 5] = [0, 16, 24, 32, 40];
/// Multiplier for the PV-node action cap.
const PV_CAP_MULT: usize = 3;
/// Root actions considered.
const ROOT_CAP: usize = 4000;
/// Actions generated (and sorted by action score) at a time at interior nodes.
const BATCH: usize = 12;
const QDEPTH: i32 = 3;
const QCAP: usize = 10;
const LMR_MIN_DEPTH: i32 = 3;
const LMR_MIN_INDEX: usize = 4;
/// Ordering bonus of a checking semimove (above every capture).
const CHECK_BONUS: i32 = 30_000;
/// Reverse futility pruning: at non-PV nodes up to this depth, return the static eval when
/// it beats beta by RFP_MARGIN per ply.
const RFP_DEPTH: i32 = 2;
const RFP_MARGIN: i32 = 200;
const DEFAULT_HASH_MB: usize = 64;

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

pub struct AlphaBeta {
    hce: hce::Weights,
    /// Exchange values for the per-board capture search at quiescent leaves.
    local_vals: super::local::Values,
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

impl Default for AlphaBeta {
    fn default() -> Self {
        Self::new()
    }
}

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

fn tt_table(mb: usize) -> Vec<Option<TtEntry>> {
    vec![None; ((mb << 20) / 64).next_power_of_two()]
}

/// Static piece values for capture ordering (MVV-LVA).
#[inline]
fn piece_value(pc: Piece) -> i32 {
    match pc {
        PAWN_W => 100,
        BRAWN_W => 150,
        KNIGHT_W => 400,
        BISHOP_W => 450,
        ROOK_W => 330,
        QUEEN_W => 1250,
        PRINCESS_W => 900,
        UNICORN_W => 250,
        DRAGON_W => 150,
        COMMON_KING_W => 300,
        KING_W | ROYAL_QUEEN_W => 2000,
        _ => 0,
    }
}

/// White-normalized piece on `sq` (same as `to_white(piece_name(b.get_piece(sq)))`), decoded
/// straight from the bitboards.
#[inline(always)]
pub(crate) fn kind_at(b: &Board, sq: i32) -> Piece {
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

/// Whether a semimove gives check: anything on its result board attacking an enemy royal
/// physically, or the moved piece itself attacking one across time or timelines (discovered
/// superphysical checks are not looked for).
struct CheckDetector<'a> {
    info: &'a HcInfo,
    s: &'a State,
    checks: Option<CheckPosition<'a>>,
}

impl<'a> CheckDetector<'a> {
    fn new(info: &'a HcInfo, s: &'a State) -> Self {
        CheckDetector { info, s, checks: None }
    }

    fn gives_check(&mut self, axis: usize, coord: Index) -> bool {
        let info: &'a HcInfo = self.info;
        let Some(mb) = info.get_move_boards(axis, coord) else { return false };
        let s: &'a State = self.s;
        let defender = !s.get_present().1;
        if has_physical_check(mb.result, defender) || mb.departure.map(|d| has_physical_check(d, defender)).unwrap_or(false)
        {
            return true;
        }
        let checks = self.checks.get_or_insert_with(|| CheckPosition::for_move_candidates(s));
        let player = s.get_present().1;
        let mv = mb.mv;
        let at = next_turn((mv.to.t(), player));
        let arrival = BoardOverlay { l: mv.new_position(s).l(), t: at.0, c: at.1, value: mb.result };
        let mask = pmask(mv.to.xy());
        let departure = mb.departure.map(|d| {
            let dt = next_turn((mv.from.t(), player));
            BoardOverlay { l: mv.from.l(), t: dt.0, c: dt.1, value: d }
        });
        checks.gives_check_masked(CheckKind::Superphysical, arrival, player, departure, mask)
    }
}

/// Per-axis ordering of a node: axis orders (best first) and each coordinate's score
/// (indexed by coordinate).
struct NodeOrder {
    order: HcOrdering,
    scores: Vec<Vec<i32>>,
}

impl AlphaBeta {
    pub fn new() -> Self {
        let hce = hce::tuned_weights();
        AlphaBeta {
            local_vals: super::local::values_from(&hce[hce::fi::MAT_A..hce::fi::MAT_A + 10]),
            hce,
            tt: tt_table(DEFAULT_HASH_MB),
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
        let _p = crate::misc::prof::scope(9);
        let sd = hce::sides(s);
        let f = hce::features_from(&sd, s, hce::needs_checks(&self.hce));
        let base = hce::score(&f, &self.hce, s.get_present().1);
        match super::evalnet::embedded() {
            Some(net) => base + net.residual(&super::evalnet::inputs_from(&sd, s, base)),
            None => base,
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
        let stop = unsafe { &*self.stop_ptr };
        if stop.load(Ordering::Relaxed)
            || self.deadline.map(|d| Instant::now() >= d).unwrap_or(false)
            || self.node_limit.map(|n| self.nodes >= n).unwrap_or(false)
        {
            self.aborted = true;
        }
        self.aborted
    }

    /// Ordering score of one HC coordinate.
    fn coord_score(&self, s: &State, info: &HcInfo, n: usize, i: Index, checks: &mut Option<CheckDetector>) -> i32 {
        match info.entry(n, i) {
            Entry::Null => -5,
            Entry::Departing { .. } => 0,
            Entry::Physical { m, .. } | Entry::Arriving { m, .. } => {
                let (mover, victim) = mover_victim(s, m);
                let mut sc = 0;
                if victim != NO_PIECE {
                    sc += 10_000 + 10 * piece_value(victim) - piece_value(mover) / 10;
                } else {
                    sc += self.history[history_index_of(mover, m)].clamp(-5000, 5000);
                }
                if (mover == PAWN_W || mover == BRAWN_W) && (m.to.y() == 0 || m.to.y() == s.get_board_size().1 - 1) {
                    sc += 9000;
                }
                if m.from.tl() != m.to.tl() && victim == NO_PIECE {
                    sc -= 300;
                }
                if mover == KING_W || mover == COMMON_KING_W {
                    sc -= 200;
                }
                if checks.as_mut().map(|c| c.gives_check(n, i)).unwrap_or(false) {
                    sc += CHECK_BONUS;
                }
                sc
            }
        }
    }

    /// Node ordering; checking semimoves are detected (and go first) when `checks`.
    fn node_ordering(&self, s: &State, info: &HcInfo, checks: bool) -> NodeOrder {
        let _p = crate::misc::prof::scope(6);
        let u = info.get_universe();
        let mut orders = Vec::with_capacity(u.dimension());
        let mut scores: Vec<Vec<i32>> = Vec::with_capacity(u.dimension());
        // Branch axes share entries; score them once.
        let mut branch_cache: Option<(Vec<Index>, Vec<i32>)> = None;
        let mut detector = if checks { Some(CheckDetector::new(info, s)) } else { None };
        for n in 0..u.dimension() {
            if n >= info.first_branch_axis() {
                if let Some((o, sc)) = &branch_cache {
                    orders.push(o.clone());
                    scores.push(sc.clone());
                    continue;
                }
            }
            let mut v: Vec<(i32, Index)> = u[n].iter().map(|i| (self.coord_score(s, info, n, i, &mut detector), i)).collect();
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
        NodeOrder { order: HcOrdering::Precomputed(orders), scores }
    }

    /// Per-axis order of a node's coordinates (diagnostics).
    pub fn ordering(&self, s: &State, info: &HcInfo) -> HcOrdering {
        self.node_ordering(s, info, true).order
    }

    /// Action generator for an interior node or the root.
    fn action_gen(&self, s: &State, info: &HcInfo, ss: SearchSpace) -> BestFirstCursor {
        let no = self.node_ordering(s, info, true);
        BestFirstCursor::new(ss, no.order, no.scores)
    }

    fn action_score(&self, s: &State, act: &MoveSeq, ply: usize) -> i32 {
        let mut sc = 0;
        for m in act {
            let (mover, victim) = mover_victim(s, m);
            if victim != NO_PIECE {
                sc += 10_000 + 10 * piece_value(victim) - piece_value(mover) / 10;
            } else {
                sc += self.history[history_index_of(mover, m)].clamp(-5000, 5000) / 4;
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

    /// The first `cap` actions of `s` in the order an interior node would try them (diagnostics).
    pub fn node_order(&self, s: &State, cap: usize) -> Vec<MoveSeq> {
        let (info, ss) = HcInfo::build_hc(s);
        let mut agen = self.action_gen(s, &info, ss);
        let mut out = Vec::new();
        let mut exhausted = false;
        while out.len() < cap && !exhausted {
            let mut batch: Vec<(i32, MoveSeq)> = Vec::new();
            while batch.len() < BATCH {
                match agen.next(&info, s) {
                    None => {
                        exhausted = true;
                        break;
                    }
                    Some(a) => batch.push((self.action_score(s, &a, 1), a)),
                }
            }
            batch.sort_by(|a, b| b.0.cmp(&a.0));
            out.extend(batch.into_iter().map(|x| x.1));
        }
        out.truncate(cap);
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
        if !in_check {
            // Moves on different boards don't interact (barring jumps between boards), so
            // the pending captures resolve board by board and their gains add up.
            let stm = s.get_present().1;
            let height = s.get_board_size().1;
            let (l_min, l_max) = s.get_lines_range();
            let mut gain = 0;
            for l in l_min..=l_max {
                let (t, c) = s.get_timeline_end(l);
                if c == stm {
                    gain += super::local::capture_gain(s.board(l, t, c), stm, height, &self.local_vals);
                }
            }
            return stand + gain;
        }
        let (info, ss) = HcInfo::build_hc(s);
        // In check: search the evasions.
        let mut agen = HcCursor::new(ss, self.node_ordering(s, &info, false).order);
        let mut best = -INF;
        let mut any = false;
        let mut tried = 0;
        while tried < QCAP {
            if self.check_abort() {
                return 0;
            }
            let Some(act) = agen.next(&info, s) else { break };
            any = true;
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

    /// Search one remembered action (TT move or killer) with PVS; returns its score.
    fn search_child(&mut self, s: &mut State, mark: Mark, first: bool, depth: i32, ply: usize, alpha: i32, beta: i32, pv: bool) -> i32 {
        let v = if first {
            -self.negamax(s, depth - 1, ply + 1, -beta, -alpha, pv)
        } else {
            let mut v = -self.negamax(s, depth - 1, ply + 1, -alpha - 1, -alpha, false);
            if !self.aborted && v > alpha && v < beta && pv {
                v = -self.negamax(s, depth - 1, ply + 1, -beta, -alpha, true);
            }
            v
        };
        s.unmake(mark);
        v
    }

    fn negamax(&mut self, s: &mut State, depth: i32, ply: usize, mut alpha: i32, beta: i32, pv: bool) -> i32 {
        if depth <= 0 {
            return self.qsearch(s, ply, alpha, beta, QDEPTH);
        }
        self.nodes += 1;
        if self.check_abort() {
            return 0;
        }
        if ply >= MAX_PLY {
            return self.static_eval(s);
        }
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

        if !pv && depth <= RFP_DEPTH && beta.abs() < MATE - 1000 && !Self::in_check(s) {
            let e = self.static_eval(s);
            if e - RFP_MARGIN * depth >= beta {
                return e;
            }
        }

        let mut best = -INF;
        let mut best_action: MoveSeq = Vec::new();
        let mut searched = 0usize;
        let cap = {
            let base = LMP[(depth as usize).min(LMP.len() - 1)];
            if pv { base * PV_CAP_MULT } else { base }
        };

        // Remembered actions first, before paying for the action space: the TT action,
        // then the killers.
        let mut pre_tried: Vec<MoveSeq> = Vec::new();
        let mut remembered: Vec<MoveSeq> = tt_action.iter().cloned().collect();
        if ply <= MAX_PLY {
            remembered.extend(self.killers[ply].iter().flatten().filter(|k| tt_action.as_ref() != Some(*k)).cloned());
        }
        for (k, act) in remembered.into_iter().enumerate() {
            let Some(mark) = Self::make_checked(s, &act) else { continue };
            // The TT action keeps the node's PV status; killers after it are null-window.
            let v = if k == 0 && tt_action.is_some() {
                let v = -self.negamax(s, depth - 1, ply + 1, -beta, -alpha, pv);
                s.unmake(mark);
                v
            } else {
                self.search_child(s, mark, searched == 0, depth, ply, alpha, beta, pv)
            };
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
            pre_tried.push(act);
        }

        let (info, ss) = HcInfo::build_hc(s);
        let mut agen = self.action_gen(s, &info, ss);
        let mut generated = 0usize;
        let mut exhausted = false;
        while searched < cap && !exhausted {
            let mut batch: Vec<(i32, MoveSeq)> = Vec::with_capacity(BATCH);
            while batch.len() < BATCH {
                if self.check_abort() {
                    return 0;
                }
                match agen.next(&info, s) {
                    None => {
                        exhausted = true;
                        break;
                    }
                    Some(act) => {
                        generated += 1;
                        if pre_tried.contains(&act) || tt_action.as_ref() == Some(&act) {
                            continue;
                        }
                        batch.push((self.action_score(s, &act, ply), act));
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
                if depth >= LMR_MIN_DEPTH && searched >= LMR_MIN_INDEX && quiet {
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

impl AlphaBeta {
    pub fn new_game(&mut self) {
        for e in self.tt.iter_mut() {
            *e = None;
        }
        for h in self.history.iter_mut() {
            *h = 0;
        }
    }

    /// Returns true when the option was recognized (only `Hash`, in MB).
    pub fn set_option(&mut self, key: &str, value: &OptionValue) -> bool {
        match (key, value.as_f64()) {
            ("Hash", Some(mb)) => {
                self.tt = tt_table((mb as usize).max(1));
                true
            }
            _ => false,
        }
    }

    pub fn find_best_move(&mut self, root: &State, limits: &Limits, stop: &AtomicBool, info: &mut dyn FnMut(String)) -> Option<Action> {
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
        let mut agen = self.action_gen(root, &hc, ss);
        let mut roots: Vec<(i32, MoveSeq)> = Vec::new();
        // Enumerating every root action can take seconds with many timelines: spend at most
        // a quarter of the budget on it (always keeping at least one action).
        let gen_deadline = budget.map(|ms| start + Duration::from_millis(ms / 4));
        while roots.len() < ROOT_CAP {
            if !roots.is_empty() && gen_deadline.map(|d| Instant::now() >= d).unwrap_or(false) {
                break;
            }
            match agen.next(&hc, root) {
                Some(a) => roots.push((self.action_score(root, &a, 0), a)),
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
