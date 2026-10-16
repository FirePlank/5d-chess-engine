//! Alpha-beta engine: iterative-deepening PVS over complete actions.
//!
//! Legal actions are produced lazily by the hypercuboid search, best-first by the summed
//! ordering scores of their semimoves, with checking semimoves (physical, and across time
//! and timelines) ordered first. The TT action and the killers are tried before the action
//! space is built; at non-PV nodes only a depth-dependent number of actions is searched.
//! Quiescence searches captures and promotions only, one quiet filler move per board.

use super::hce;
use super::time::TimePlan;
use super::{Limits, OptionValue};
use crate::core::action::{Action, FullMove, MoveSeq};
use crate::core::bitboard::{marked_pos, pmask};
use crate::core::board::Board;
use crate::core::check_position::{BoardOverlay, CheckBuf, CheckKind, CheckPosition, SpReach};
use crate::core::hypercuboid::{has_physical_check, BestFirstCursor, Entry, HcCursor, HcInfo, NeedBranch};
use crate::core::multiverse::next_turn;
use crate::core::piece::*;
use crate::core::state::{Mark, State};
use crate::misc::geometry::SearchSpace;
use crate::misc::integer_set::Index;
use crate::misc::ordering::{AxisOrders, HcOrdering};
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const MATE: i32 = 30000;
pub const INF: i32 = 32000;
const MAX_PLY: usize = 128;

/// Actions searched at non-PV nodes by remaining depth (index = depth, last repeats).
const LMP: [usize; 5] = [0, 20, 32, 44, 56];
/// Multiplier for the PV-node action cap.
const PV_CAP_MULT: usize = 3;
/// Root actions considered.
const ROOT_CAP: usize = 1000;
/// Actions generated (and sorted by action score) at a time at interior nodes: the first
/// batch holds one action and each next one twice as many, up to this.
const BATCH: usize = 12;
const QDEPTH: i32 = 3;
const QCAP: usize = 4;
const LMR_MIN_DEPTH: i32 = 3;
const LMR_MIN_INDEX: usize = 4;
/// Late root actions (by the previous iteration's order) are searched shallower from this
/// index on, and re-searched at full depth if they beat alpha.
const ROOT_LMR_MIN_INDEX: usize = 6;

/// Late move reduction for the `index`-th action at `depth`: 1 + ln(depth) ln(index) / 3.
#[inline]
fn lmr(depth: i32, index: usize) -> i32 {
    (1.0 + (depth.max(1) as f32).ln() * (index.max(1) as f32).ln() / 3.0) as i32
}
/// Ordering bonus of a checking semimove (above every capture).
const CHECK_BONUS: i32 = 30_000;
/// Reverse futility pruning: at non-PV nodes up to this depth, return the static eval when
/// it beats beta by RFP_MARGIN per ply.
const RFP_DEPTH: i32 = 2;
const RFP_MARGIN: i32 = 200;
/// Null move (a pass, i.e. a phantom turn): at non-PV nodes from this depth, when the static
/// eval is at least beta, search the pass with depth reduced by NMP_R + depth / NMP_DIV.
const NMP_MIN_DEPTH: i32 = 3;
const NMP_R: i32 = 2;
const NMP_DIV: i32 = 3;
const DEFAULT_HASH_MB: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Bound {
    /// An unused slot.
    Empty = 0,
    Exact,
    Lower,
    Upper,
}

/// All-zero bytes are an empty entry.
struct TtEntry {
    key: u64,
    depth: i16,
    bound: Bound,
    /// A mate `score` is proven (see `AlphaBeta::proven`).
    proven: bool,
    score: i32,
    action: MoveSeq,
}

/// One table slot: an entry and its lock. All-zero bytes are an empty, unlocked slot.
struct TtSlot {
    /// Held while the entry is read or written. Actions have any length (long ones live on
    /// the heap), so threads cannot share entries without one: a reader racing a writer
    /// could copy a torn action or follow a freed pointer. The lock is only ever tried: a
    /// thread finding it held treats its probe as a miss or skips its store, so no thread
    /// ever waits on another, and a single search thread never finds it held.
    lock: AtomicBool,
    entry: UnsafeCell<TtEntry>,
}

/// The transposition table, shared by every search thread: zero-allocated, so creating or
/// clearing it costs nothing until slots are used.
struct TtTable {
    slots: Vec<TtSlot>,
    /// Some stored action spilled to the heap (entries then need dropping one by one).
    spilled: AtomicBool,
}

// SAFETY: an entry is only accessed by the thread holding its slot's lock.
unsafe impl Sync for TtTable {}
unsafe impl Send for TtTable {}

impl TtTable {
    fn new(mb: usize) -> Self {
        Self::with_len(((mb << 20) / 64).next_power_of_two())
    }

    /// An empty table of `n` slots.
    fn with_len(n: usize) -> Self {
        let layout = std::alloc::Layout::array::<TtSlot>(n).expect("tt size");
        // SAFETY: an all-zero `TtSlot` is valid (unlocked, empty bound, empty inline action),
        // and the block is allocated with the layout `Vec` uses for this capacity.
        let slots = unsafe {
            let p = std::alloc::alloc_zeroed(layout) as *mut TtSlot;
            if p.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            Vec::from_raw_parts(p, n, n)
        };
        TtTable { slots, spilled: AtomicBool::new(false) }
    }

    fn len(&self) -> usize {
        self.slots.len()
    }

    /// Run `f` on the entry for `key` under its slot's lock: None when the slot holds
    /// another position or another thread holds the lock.
    #[inline]
    fn probe<R>(&self, key: u64, f: impl FnOnce(&TtEntry) -> R) -> Option<R> {
        let slot = &self.slots[(key as usize) & (self.slots.len() - 1)];
        if slot.lock.swap(true, Ordering::Acquire) {
            return None;
        }
        // SAFETY: the lock gives this thread exclusive access to the entry.
        let e = unsafe { &*slot.entry.get() };
        let r = if e.bound != Bound::Empty && e.key == key { Some(f(e)) } else { None };
        slot.lock.store(false, Ordering::Release);
        r
    }

    fn store(&self, key: u64, depth: i32, bound: Bound, score: i32, proven: bool, action: &MoveSeq) {
        let slot = &self.slots[(key as usize) & (self.slots.len() - 1)];
        if slot.lock.swap(true, Ordering::Acquire) {
            return;
        }
        // SAFETY: the lock gives this thread exclusive access to the entry.
        let e = unsafe { &mut *slot.entry.get() };
        if e.bound == Bound::Empty || e.key != key || depth as i16 >= e.depth - 1 || bound == Bound::Exact {
            e.key = key;
            e.depth = depth as i16;
            e.bound = bound;
            e.proven = proven;
            e.score = score;
            e.action.assign(action);
            if e.action.spilled() && !self.spilled.load(Ordering::Relaxed) {
                self.spilled.store(true, Ordering::Relaxed);
            }
        }
        slot.lock.store(false, Ordering::Release);
    }
}

impl Drop for TtTable {
    fn drop(&mut self) {
        if !*self.spilled.get_mut() {
            // SAFETY: no entry owns heap memory, so nothing is leaked by skipping their drops.
            unsafe { self.slots.set_len(0) };
        }
    }
}

/// Search nodes of one helper thread, published for the main thread's node count. Each
/// sits on its own cache lines, so publishing never bounces a line between threads.
#[repr(align(128))]
#[derive(Default)]
struct NodeCounter(AtomicU64);

/// One thread's search result, for the vote between threads.
#[derive(Clone, Debug)]
struct ThreadResult {
    best: MoveSeq,
    score: i32,
    /// A mate `score` is proven (see `AlphaBeta::proven`).
    proven: bool,
    /// Deepest iteration the thread finished (0: none, and then it does not vote).
    completed_depth: i32,
    pv_len: usize,
    nodes: u64,
    thread_id: usize,
}

/// The result to play, by Stockfish's thread voting (as in Apeiron): each action accrues
/// `(score - min_score + 14) * completed_depth` from the threads choosing it. Proven wins
/// win outright (the fastest mate among them; a mate score that is not proven is no more
/// than a large score), a loss is never switched to, and a thread that finished no iteration
/// does not vote.
fn select_best_thread(results: &[ThreadResult]) -> usize {
    let finished = |r: &ThreadResult| r.completed_depth > 0;
    let proven_win = |r: &ThreadResult| r.proven && r.score > MATE - 1000;
    let is_loss = |v: i32| v < -(MATE - 1000);
    let min_score = results.iter().filter(|r| finished(r)).map(|r| r.score).min().unwrap_or(0);
    let value = |r: &ThreadResult| (r.score - min_score + 14) as i64 * r.completed_depth as i64;
    let vote = |m: &MoveSeq| -> i64 { results.iter().filter(|r| finished(r) && r.best == *m).map(value).sum() };
    // Prefer threads with a non-truncated PV on exact vote ties.
    let with_pv = |r: &ThreadResult| value(r) * (r.pv_len > 2) as i64;
    // With no finished thread at all, the main thread's fallback action stands.
    let mut best_idx = results.iter().position(|r| r.thread_id == 0).unwrap_or(0);
    for (i, r) in results.iter().enumerate() {
        let best = &results[best_idx];
        if !finished(r) {
            continue;
        }
        if !finished(best) {
            best_idx = i;
            continue;
        }
        let (best_vote, new_vote) = (vote(&best.best), vote(&r.best));
        if proven_win(best) {
            if proven_win(r) && r.score > best.score {
                best_idx = i;
            }
        } else if is_loss(best.score) {
            if r.score > best.score {
                best_idx = i;
            }
        } else if proven_win(r) || (!is_loss(r.score) && (new_vote > best_vote || (new_vote == best_vote && with_pv(r) > with_pv(best)))) {
            best_idx = i;
        }
    }
    best_idx
}

const KNOWN_EVAL: u8 = 1;
const KNOWN_CHECK: u8 = 2;
const IN_CHECK: u8 = 4;
const KNOWN_BITS: u32 = 16;

#[derive(Clone, Copy, Default)]
struct KnownSlot {
    key: u64,
    eval: i32,
    flags: u8,
}

/// Static evaluation and in-check status by position hash (direct-mapped; a hit is a full
/// key match, as in the transposition table).
struct KnownCache {
    slots: Vec<KnownSlot>,
}

impl KnownCache {
    fn new() -> Self {
        KnownCache { slots: vec![KnownSlot::default(); 1 << KNOWN_BITS] }
    }

    #[inline(always)]
    fn slot(&self, key: u64) -> usize {
        (key >> (64 - KNOWN_BITS)) as usize
    }

    /// Update slot `i` for `key` (starting it afresh when it held another position).
    #[inline(always)]
    fn store(&mut self, i: usize, key: u64, f: impl FnOnce(&mut KnownSlot)) {
        let e = &mut self.slots[i];
        if e.key != key || e.flags == 0 {
            *e = KnownSlot { key, eval: 0, flags: 0 };
        }
        f(e);
    }

    fn clear(&mut self) {
        self.slots.fill(KnownSlot::default());
    }
}

pub struct AlphaBeta {
    hce: hce::Weights,
    /// Exchange values for the per-board capture search at quiescent leaves.
    local_vals: super::local::Values,
    /// Shared with the helper threads.
    tt: Arc<TtTable>,
    history: Vec<i32>,
    killers: Vec<[Option<MoveSeq>; 2]>,
    /// Per ply: the last evasion that failed high in quiescence (tried before building).
    qkillers: Vec<Option<MoveSeq>>,
    nodes: u64,
    deadline: Option<Instant>,
    node_limit: Option<u64>,
    aborted: bool,
    stop_ptr: *const AtomicBool,
    /// Whether the mate score the last `negamax`/`qsearch` call returned is proven: a mated
    /// score when every legal action was searched (no pruning, cap or cutoff left one out)
    /// and every reply's mating score is proven; a mating score when the reply giving it is
    /// proven; a checkmate on the board. A proven mate is a forced mate within the reported
    /// distance. Only meaningful for mate scores; it never steers the search.
    proven: bool,
    /// The root is won or lost by enough (or by mate) that pruning which hides long mating
    /// lines is switched off (Stockfish's seekMate).
    seek_mate: bool,
    /// Per-node buffers, reused down the recursion.
    bufs: Vec<Box<NodeBufs>>,
    /// Evaluation terms of recently seen boards.
    terms: Box<hce::TermCache>,
    /// Whether the position the next `negamax` call searches is in check, when known.
    known_check: Option<bool>,
    /// Static evaluations and in-check status of recently seen positions.
    known: KnownCache,
    /// The score (internal units, side to move) of the last `find_best_move`.
    last_score: i32,
    /// Search threads (`Threads`), this one included.
    threads: usize,
    /// Lazy SMP helper threads (started by the first search that needs them).
    helpers: Vec<Helper>,
    /// Nodes of the helpers in the last search.
    helper_nodes: u64,
    /// Helpers whose results the last search collected.
    helper_results: usize,
    /// 0 for the main thread; helpers count from 1.
    thread_id: usize,
    /// A helper publishes its node count here (null for the main thread).
    nodes_out: *const AtomicU64,
}

// The raw stop and node-count pointers are only dereferenced during a search, while what
// they point to is alive.
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

/// Whether `v` is a mating score (the side to move mates).
#[inline]
fn mating(v: i32) -> bool {
    v > MATE - 1000
}

/// Whether a node's `best` score is a proven mate: a mating score when the reply giving it is
/// (`best_proven`), a mated score when every legal action was searched and every reply's
/// mating score is proven (`all_proven`).
#[inline]
fn mate_proven(best: i32, best_proven: bool, all_proven: bool) -> bool {
    if mating(best) {
        best_proven
    } else {
        best < -MATE + 1000 && all_proven
    }
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

/// (moving piece, captured piece), white-normalized; matches `moved_piece`/`captured_piece`.
#[inline]
fn mover_victim(s: &State, m: &FullMove) -> (Piece, Piece) {
    let player = s.get_present().1;
    let src = s.board(m.from.l(), m.from.t(), player);
    let mover = src.kind(m.from.xy());
    let victim = if m.from.tl() == m.to.tl() {
        if mover == KING_W && (m.from.x() - m.to.x()).abs() > 1 {
            NO_PIECE
        } else {
            let d = src.kind(m.to.xy());
            if d == NO_PIECE && (mover == PAWN_W || mover == BRAWN_W) && m.from.x() != m.to.x() {
                src.kind(ppos(m.to.x(), m.from.y()))
            } else {
                d
            }
        }
    } else {
        s.board(m.to.l(), m.to.t(), player).kind(m.to.xy())
    };
    (mover, victim)
}

/// Material the moved piece stands to lose on its result board: all of it when attacked and
/// undefended, otherwise what the cheapest attacker gains by taking it.
#[inline]
fn exchange_loss(b: &Board, to: i32, player: bool, mover_value: i32) -> i32 {
    let att = b.attacks_to(to) & !pmask(to);
    let enemy = att & b.hostile(player);
    if enemy == 0 {
        return 0;
    }
    if att & b.hostile(!player) == 0 {
        return mover_value;
    }
    let least = marked_pos(enemy).map(|sq| piece_value(b.kind(sq))).min().unwrap_or(0);
    (mover_value - least).max(0)
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
    buf: CheckBuf,
    /// Superphysical reach from the next board of each line with physical moves.
    reach: Vec<(i32, SpReach)>,
}

impl<'a> CheckDetector<'a> {
    fn new(info: &'a HcInfo, s: &'a State, scratch: &mut OrderScratch) -> Self {
        let mut reach = std::mem::take(&mut scratch.reach);
        reach.clear();
        CheckDetector { info, s, checks: None, buf: std::mem::take(&mut scratch.checks), reach }
    }

    /// Give the buffers back to `scratch`.
    fn finish(self, scratch: &mut OrderScratch) {
        scratch.checks = match self.checks {
            Some(c) => c.into_buf(),
            None => self.buf,
        };
        scratch.reach = self.reach;
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
        if self.checks.is_none() {
            self.checks = Some(CheckPosition::for_move_candidates_in(s, std::mem::take(&mut self.buf)));
        }
        let checks = self.checks.as_mut().unwrap();
        let player = s.get_present().1;
        let mv = mb.mv;
        let at = next_turn((mv.to.t(), player));
        if mb.departure.is_none() {
            // A physical move: only the moved piece's reach from its line's next board counts,
            // and that board's surroundings are the same for every move on the line.
            let l = mv.to.l();
            let i = match self.reach.iter().position(|r| r.0 == l) {
                Some(i) => i,
                None => {
                    self.reach.push((l, checks.sp_reach(l, 2 * at.0 + at.1 as i32 + 1)));
                    self.reach.len() - 1
                }
            };
            return self.reach[i].1.attacks(mb.result, mv.to.xy());
        }
        let arrival = BoardOverlay { l: mv.new_position(s).l(), t: at.0, c: at.1, value: mb.result };
        let mask = pmask(mv.to.xy());
        let departure = mb.departure.map(|d| {
            let dt = next_turn((mv.from.t(), player));
            BoardOverlay { l: mv.from.l(), t: dt.0, c: dt.1, value: d }
        });
        checks.gives_check_masked(CheckKind::Superphysical, arrival, player, departure, mask)
    }
}

/// Scratch of `node_ordering`.
#[derive(Default)]
struct OrderScratch {
    sort: Vec<u64>,
    checks: CheckBuf,
    reach: Vec<(i32, SpReach)>,
    /// History values of the branch coordinates, by coordinate, as they were when the
    /// node was ordered (the branch axes may be scored later).
    branch_history: Vec<i32>,
}

/// How `node_ordering` treats the branch axes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BranchOrder {
    /// Score and order them now.
    Now,
    /// Only score the null coordinate and record the history values the rest need.
    Defer,
    /// Score and order them with the recorded history values.
    Recorded,
}

/// Buffers of one search node, reused by later nodes at the same recursion depth.
#[derive(Default)]
struct NodeBufs {
    info: HcInfo,
    cursor: BestFirstCursor,
    evasions: HcCursor,
    order: OrderScratch,
    /// Coordinate scores of the evasion ordering (unused by its cursor).
    evasion_scores: Vec<Vec<i32>>,
}

impl AlphaBeta {
    pub fn new() -> Self {
        Self::with_tt(Arc::new(TtTable::new(DEFAULT_HASH_MB)))
    }

    fn with_tt(tt: Arc<TtTable>) -> Self {
        let hce = hce::tuned_weights();
        AlphaBeta {
            local_vals: super::local::values_from(&hce[hce::fi::MAT_A..hce::fi::MAT_A + 10]),
            hce,
            tt,
            history: vec![0; 1 << 16],
            killers: vec![[None, None]; MAX_PLY + 1],
            qkillers: vec![None; MAX_PLY + 1],
            nodes: 0,
            deadline: None,
            node_limit: None,
            aborted: false,
            stop_ptr: std::ptr::null(),
            proven: false,
            seek_mate: false,
            bufs: Vec::new(),
            terms: Box::default(),
            known_check: None,
            known: KnownCache::new(),
            last_score: 0,
            threads: 1,
            helpers: Vec::new(),
            helper_nodes: 0,
            helper_results: 0,
            thread_id: 0,
            nodes_out: std::ptr::null(),
        }
    }

    #[inline]
    fn static_eval(&mut self, s: &State) -> i32 {
        let _p = crate::misc::prof::scope(9);
        let key = s.hash();
        let i = self.known.slot(key);
        let e = self.known.slots[i];
        if e.key == key && e.flags & KNOWN_EVAL != 0 {
            return e.eval;
        }
        let sd = hce::sides_with(s, &mut *self.terms);
        let f = hce::features_from(&sd, s, hce::needs_checks(&self.hce));
        let base = hce::score(&f, &self.hce, s.get_present().1);
        let v = match super::evalnet::embedded() {
            Some(net) => base + net.residual(&super::evalnet::inputs_from(&sd, s, base)),
            None => base,
        };
        self.known.store(i, key, |e| {
            e.eval = v;
            e.flags |= KNOWN_EVAL;
        });
        v
    }

    /// Whether the side to move is in check (royal captures threatened if it passed).
    fn in_check(&mut self, s: &State) -> bool {
        let _p = crate::misc::prof::scope(10);
        let key = s.hash();
        let i = self.known.slot(key);
        let e = self.known.slots[i];
        if e.key == key && e.flags & KNOWN_CHECK != 0 {
            return e.flags & IN_CHECK != 0;
        }
        let c = s.has_phantom_check();
        self.known.store(i, key, |e| e.flags |= KNOWN_CHECK | if c { IN_CHECK } else { 0 });
        c
    }

    #[inline]
    fn tt_store(&self, key: u64, depth: i32, bound: Bound, score: i32, proven: bool, action: &MoveSeq) {
        self.tt.store(key, depth, bound, score, proven, action);
    }

    fn check_abort(&mut self) -> bool {
        if self.aborted {
            return true;
        }
        if !self.nodes_out.is_null() {
            // SAFETY: a helper's counter outlives its search (see `Helper::spawn`).
            unsafe { (*self.nodes_out).store(self.nodes, Ordering::Relaxed) };
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
    /// Ordering score of one HC coordinate; `history` overrides the history table's value.
    fn coord_score(&self, s: &State, info: &HcInfo, n: usize, i: Index, checks: &mut Option<CheckDetector>, history: Option<i32>) -> i32 {
        match info.entry(n, i) {
            Entry::Null => -5,
            Entry::Departing { .. } => 0,
            Entry::Physical { m, b, mover, victim } | Entry::Arriving { m, b, mover, victim, .. } => {
                let (mover, victim) = (*mover, *victim);
                // Per-board exchange check on the result board: moves that hang the moved
                // piece sink on their board, so best-first combinations avoid them.
                let mut sc = -10 * exchange_loss(b, m.to.xy(), s.get_present().1, piece_value(mover));
                if victim != NO_PIECE {
                    sc += 10_000 + 10 * piece_value(victim) - piece_value(mover) / 10;
                } else {
                    sc += history.unwrap_or_else(|| self.history[history_index_of(mover, m)]).clamp(-5000, 5000);
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

    /// Node ordering into `orders` (per axis, coordinates best first) and `scores` (per axis,
    /// each coordinate's score) of the axes in `axes`; checking semimoves are detected (and go
    /// first) when `checks`.
    #[allow(clippy::too_many_arguments)]
    fn node_ordering(
        &self,
        s: &State,
        info: &HcInfo,
        checks: bool,
        scratch: &mut OrderScratch,
        orders: &mut AxisOrders,
        scores: &mut Vec<Vec<i32>>,
        axes: std::ops::Range<usize>,
        branch: BranchOrder,
    ) {
        let _p = crate::misc::prof::scope(6);
        let u = info.get_universe();
        // Branch axes share entries: they are scored once and share the first one's lists.
        let fba = info.first_branch_axis();
        let dim = u.dimension().min(fba + 1);
        orders.shared = fba;
        let orders = &mut orders.lists;
        if orders.len() < dim {
            orders.resize_with(dim, Vec::new);
        }
        if scores.len() < dim {
            scores.resize_with(dim, Vec::new);
        }
        let mut detector = if checks { Some(CheckDetector::new(info, s, scratch)) } else { None };
        for n in axes.start..axes.end.min(dim) {
            let sc = &mut scores[n];
            sc.clear();
            if n == fba && branch == BranchOrder::Defer {
                // Only the null coordinate is scored (all a branch axis restricted to it
                // needs); the history values the others will need are recorded as of now.
                let hist = &mut scratch.branch_history;
                hist.clear();
                hist.resize(u[n].max().map(|i| i as usize + 1).unwrap_or(0), 0);
                for i in u[n].iter() {
                    if let Some((m, mover)) = info.branch_arrival(i) {
                        hist[i as usize] = self.history[history_index_of(mover, &m)];
                    }
                }
                sc.push(self.coord_score(s, info, n, 0, &mut detector, None));
                continue;
            }
            sc.resize(u[n].max().map(|i| i as usize + 1).unwrap_or(0), 0);
            // Sort keys: score descending, then coordinate ascending.
            let v = &mut scratch.sort;
            v.clear();
            for i in u[n].iter() {
                let recorded = if n == fba && branch == BranchOrder::Recorded { Some(scratch.branch_history[i as usize]) } else { None };
                let score = self.coord_score(s, info, n, i, &mut detector, recorded);
                sc[i as usize] = score;
                v.push(((!((score as u32) ^ 0x8000_0000) as u64) << 32) | i as u64);
            }
            v.sort_unstable();
            let o = &mut orders[n];
            o.clear();
            o.extend(v.iter().map(|&k| k as u32 as Index));
        }
        if let Some(d) = detector {
            d.finish(scratch);
        }
    }

    /// Per-axis order of a node's coordinates (diagnostics).
    pub fn ordering(&self, s: &State, info: &HcInfo) -> HcOrdering {
        let mut orders = AxisOrders::default();
        self.node_ordering(s, info, true, &mut OrderScratch::default(), &mut orders, &mut Vec::new(), 0..usize::MAX, BranchOrder::Now);
        HcOrdering::Precomputed(orders)
    }

    /// Moves on boards the side to move doesn't have to play (ahead of the present, or on
    /// inactive timelines) are left out: only the null coordinate stay on those axes.
    fn without_future(s: &State, info: &HcInfo, ss: &SearchSpace) -> SearchSpace {
        let present = s.get_present();
        let (amin, amax) = s.get_active_range();
        let (lo, hi) = s.get_lines_range();
        let optional = |l: i32| -> Option<usize> {
            let n = info.line_to_axis(l)?;
            if l >= amin && l <= amax && s.get_timeline_end(l) == present { None } else { Some(n) }
        };
        if !(lo..=hi).any(|l| optional(l).is_some()) {
            return ss.clone();
        }
        let mut out = SearchSpace::default();
        'hc: for hc in &ss.hcs {
            for l in lo..=hi {
                if let Some(n) = optional(l) {
                    if !hc[n].contains(0) {
                        continue 'hc;
                    }
                }
            }
            let mut h = hc.clone();
            for l in lo..=hi {
                if let Some(n) = optional(l) {
                    h[n].assign_singleton(0);
                }
            }
            out.hcs.push(h);
        }
        out
    }

    /// Order the node's coordinates into `cursor` and start it on `ss`.
    /// Order the node's coordinates into `cursor` and start it on `ss`; with `defer`, the
    /// branch axes are ordered once the cursor needs them (`next_with` and `branch_order`).
    fn start_actions(&self, s: &State, info: &HcInfo, cursor: &mut BestFirstCursor, scratch: &mut OrderScratch, ss: SearchSpace, defer: bool) {
        let (orders, scores) = cursor.ordering_mut();
        let branch = if defer { BranchOrder::Defer } else { BranchOrder::Now };
        self.node_ordering(s, info, true, scratch, orders, scores, 0..usize::MAX, branch);
        cursor.set_branch_pending(defer && info.first_branch_axis() < info.dimension());
        cursor.restart(info, ss);
    }

    /// The deferred branch-axis ordering of `start_actions`.
    fn branch_order(&self, s: &State, info: &HcInfo, scratch: &mut OrderScratch, orders: &mut AxisOrders, scores: &mut Vec<Vec<i32>>) {
        let fba = info.first_branch_axis();
        self.node_ordering(s, info, true, scratch, orders, scores, fba..fba + 1, BranchOrder::Recorded);
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
        let mut agen = BestFirstCursor::default();
        self.start_actions(s, &info, &mut agen, &mut OrderScratch::default(), ss, false);
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
        if ok && s.submit::<false>() && !s.royal_capturable(s.get_present().1) {
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

    /// Passing is only trusted when some active board holds at least two non-pawn pieces
    /// (besides royals) of the side to move. Counted per board: every timeline holds its own
    /// copy of a piece, so a lone knight on three timelines is still a lone knight. With less,
    /// zugzwang and mating nets the reduced search can't see are common.
    fn has_null_material(s: &State) -> bool {
        let stm = s.get_present().1;
        let (amin, amax) = s.get_active_range();
        (amin..=amax).any(|l| {
            let (t, c) = s.get_timeline_end(l);
            let b = s.board(l, t, c);
            (b.hostile(!stm) & !b.lpawn() & !b.royal()).count_ones() >= 2
        })
    }

    /// Boards the side to move must play this action.
    fn mandatory_boards(s: &State) -> usize {
        let present = s.get_present();
        let (amin, amax) = s.get_active_range();
        (amin..=amax).filter(|&l| s.get_timeline_end(l) == present).count()
    }

    /// Whether `without_future` leaves out any point of `ss` (a move on an optional board).
    fn trims_future(s: &State, info: &HcInfo, ss: &SearchSpace) -> bool {
        let present = s.get_present();
        let (amin, amax) = s.get_active_range();
        let (lo, hi) = s.get_lines_range();
        let optional: Vec<usize> = (lo..=hi)
            .filter(|&l| !(l >= amin && l <= amax && s.get_timeline_end(l) == present))
            .filter_map(|l| info.line_to_axis(l))
            .collect();
        !optional.is_empty() && ss.hcs.iter().any(|hc| optional.iter().any(|&n| hc[n].has_other_than(0)))
    }

    fn terminal_score(&mut self, s: &State, ply: usize) -> i32 {
        self.proven = true;
        if self.in_check(s) {
            return -(MATE - ply as i32);
        }
        // Stalemate: a draw, unless the exiled-kings rule decides it.
        match s.exiled_kings_loser() {
            Some(loser) if loser == s.get_present().1 => -(MATE - ply as i32),
            Some(_) => MATE - ply as i32,
            None => 0,
        }
    }

    /// `known_check`: whether `s` is in check, when the caller already knows.
    fn qsearch(&mut self, s: &mut State, ply: usize, mut alpha: i32, beta: i32, qd: i32, known_check: Option<bool>) -> i32 {
        self.nodes += 1;
        // Only the in-check evasion search below can return a mate score.
        self.proven = false;
        if self.check_abort() {
            return 0;
        }
        let in_check = known_check.unwrap_or_else(|| self.in_check(s));
        let leaf = qd <= 0 || ply >= MAX_PLY;
        // Royal captures are threatened: standing pat is unsound, so search evasions (the
        // static evaluation only matters at the depth limit).
        if in_check && leaf {
            return self.static_eval(s);
        }
        if !in_check {
            let stand = self.static_eval(s);
            if stand >= beta {
                return stand;
            }
            alpha = alpha.max(stand);
            if leaf {
                return alpha;
            }
            // Moves on different boards don't interact (barring jumps between boards), so
            // the pending captures resolve board by board and their gains add up.
            let present = s.get_present();
            let height = s.get_board_size().1;
            let (amin, amax) = s.get_active_range();
            let mut gain = 0;
            for l in amin..=amax {
                let (t, c) = s.get_timeline_end(l);
                if (t, c) == present {
                    gain += super::local::capture_gain(s.board(l, t, c), c, height, &self.local_vals);
                }
            }
            return stand + gain;
        }
        // In check: the evasion that failed high here last first, before building the rest.
        let mut best = -INF;
        let mut any = false;
        let mut tried = 0;
        // Mate proof: the best reply's, and every reply's with all evasions searched.
        let (mut best_proven, mut all_proven) = (false, true);
        let mut qkiller = if ply <= MAX_PLY { self.qkillers[ply].clone() } else { None };
        if let Some(k) = qkiller.take() {
            if let Some(mark) = Self::make_checked(s, &k) {
                let v = -self.qsearch(s, ply + 1, -beta, -alpha, qd - 1, None);
                s.unmake(mark);
                if self.aborted {
                    return 0;
                }
                // Searched: skipped when the evasions come round to it.
                qkiller = Some(k);
                any = true;
                tried = 1;
                best = v;
                best_proven = self.proven;
                all_proven &= self.proven;
                if v > alpha {
                    alpha = v;
                    if v >= beta {
                        return v;
                    }
                }
            }
        }
        // Search the evasions.
        let mut nb = self.bufs.pop().unwrap_or_default();
        {
            let NodeBufs { info, evasions, order, evasion_scores, .. } = &mut *nb;
            evasions.ss = info.rebuild(s, true);
            // The branch axes are only ordered once some hypercuboid needs their order.
            let fba = info.first_branch_axis();
            self.node_ordering(s, info, false, order, evasions.order.axes_mut(), evasion_scores, 0..fba, BranchOrder::Now);
            evasions.branch_pending = fba < info.dimension();
        }
        let aborted = 'search: {
            while tried < QCAP {
                if self.check_abort() {
                    break 'search true;
                }
                let NodeBufs { info, evasions, order, evasion_scores, .. } = &mut *nb;
                let fba = info.first_branch_axis();
                // (Quiescence leaves the history alone, so ordering later gives the same order.)
                let next = evasions.next_with(info, s, &mut |o| self.node_ordering(s, info, false, order, o, evasion_scores, fba..fba + 1, BranchOrder::Now));
                let Some(act) = (match next {
                    Ok(a) => a,
                    Err(NeedBranch) => {
                        info.materialize_branch(s);
                        continue;
                    }
                }) else {
                    // Every evasion was searched.
                    break 'search false;
                };
                if qkiller.as_ref() == Some(&act) {
                    qkiller = None;
                    continue;
                }
                any = true;
                tried += 1;
                let mark = Self::make(s, &act);
                let v = -self.qsearch(s, ply + 1, -beta, -alpha, qd - 1, None);
                s.unmake(mark);
                if self.aborted {
                    break 'search true;
                }
                all_proven &= self.proven;
                if v > best {
                    best = v;
                    best_proven = self.proven;
                    if v > alpha {
                        alpha = v;
                        if v >= beta {
                            if ply <= MAX_PLY {
                                self.qkillers[ply] = Some(act.clone());
                            }
                            break;
                        }
                    }
                }
            }
            // The evasion cap or a cutoff left evasions unsearched.
            all_proven = false;
            false
        };
        self.bufs.push(nb);
        if aborted {
            return 0;
        }
        if !any {
            // Only reached in check: mated.
            self.proven = true;
            return -(MATE - ply as i32);
        }
        self.proven = mate_proven(best, best_proven, all_proven);
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
        let known_check = self.known_check.take();
        if depth <= 0 {
            return self.qsearch(s, ply, alpha, beta, QDEPTH, known_check);
        }
        self.nodes += 1;
        // Every return below that can be a mate score sets this.
        self.proven = false;
        if self.check_abort() {
            return 0;
        }
        if ply >= MAX_PLY {
            return self.static_eval(s);
        }
        let alpha0 = alpha;
        let key = s.hash();
        let mut tt_action: Option<MoveSeq> = None;
        // Ok((score, proven)): a cutoff; Err(action): the remembered action, if any. (Read
        // under the slot's lock, so the score, its proof and the action are one entry's.)
        let probe = self.tt.probe(key, |e| {
            let sc = mate_from_tt(e.score, ply);
            if !pv && e.depth as i32 >= depth {
                let cut = match e.bound {
                    Bound::Exact => true,
                    Bound::Lower => sc >= beta,
                    Bound::Upper => sc <= alpha,
                    Bound::Empty => false,
                };
                if cut {
                    return Ok((sc, e.proven));
                }
            }
            Err(if e.action.is_empty() { None } else { Some(e.action.clone()) })
        });
        match probe {
            Some(Ok((sc, proven))) => {
                self.proven = proven;
                return sc;
            }
            Some(Err(a)) => tt_action = a,
            None => {}
        }

        if !pv && beta.abs() < MATE - 1000 && (depth <= RFP_DEPTH || depth >= NMP_MIN_DEPTH) && !known_check.unwrap_or_else(|| self.in_check(s)) {
            if depth <= RFP_DEPTH {
                let e = self.static_eval(s);
                if e - RFP_MARGIN * depth >= beta {
                    return e;
                }
            }
            if depth >= NMP_MIN_DEPTH && !self.seek_mate && Self::has_null_material(s) && self.static_eval(s) >= beta {
                let r = NMP_R + depth / NMP_DIV;
                let mark = s.make_null();
                let v = -self.negamax(s, depth - 1 - r, ply + 1, -beta, -beta + 1, false);
                s.unmake(mark);
                if self.aborted {
                    return 0;
                }
                if v >= beta {
                    // Never a mate score (beta is not one): nothing to prove.
                    self.proven = false;
                    return if v > MATE - 1000 { beta } else { v };
                }
            }
        }

        let mut best = -INF;
        let mut best_action = MoveSeq::new();
        let mut searched = 0usize;
        // Mate proof: the best reply's, and every searched reply's (see `proven`).
        let (mut best_proven, mut all_proven) = (false, true);
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
            all_proven &= self.proven;
            if v > best {
                best = v;
                best_proven = self.proven;
                best_action = act.clone();
                if v > alpha {
                    alpha = v;
                }
            }
            if alpha >= beta {
                self.on_cutoff(s, &act, depth, ply);
                let proven = mate_proven(best, best_proven, false);
                self.tt_store(key, depth, Bound::Lower, mate_to_tt(best, ply), proven, &best_action);
                self.proven = proven;
                return best;
            }
            pre_tried.push(act);
        }

        let mut nb = self.bufs.pop().unwrap_or_default();
        let ss = nb.info.rebuild(s, true);
        let first = Self::without_future(s, &nb.info, &ss);
        let mut full = Some(ss);
        {
            let NodeBufs { info, cursor, order, .. } = &mut *nb;
            self.start_actions(s, info, cursor, order, first, true);
        }
        let mut generated = 0usize;
        let mut exhausted = false;
        // An action was left unsearched (the action cap, or a TT action that failed validation).
        let mut skipped = false;
        let mut batch_size = 1;
        let outcome: Option<i32> = 'search: {
            while searched < cap && !exhausted {
                let mut batch: Vec<(i32, MoveSeq)> = Vec::with_capacity(BATCH);
                let size = batch_size;
                batch_size = (2 * batch_size).min(BATCH);
                while batch.len() < size {
                    if self.check_abort() {
                        break 'search Some(0);
                    }
                    let next = {
                        let NodeBufs { info, cursor, order, .. } = &mut *nb;
                        cursor.next_with(info, s, &mut |o, sc| self.branch_order(s, info, order, o, sc))
                    };
                    match next {
                        Err(NeedBranch) => nb.info.materialize_branch(s),
                        Ok(None) => {
                            // Every legal action may need a move on an optional board. (The
                            // ordering is unchanged: nothing was searched since it was made.)
                            if generated == 0 {
                                if let Some(f) = full.take() {
                                    nb.cursor.restart(&nb.info, f);
                                    continue;
                                }
                            }
                            exhausted = true;
                            break;
                        }
                        Ok(Some(act)) => {
                            generated += 1;
                            if pre_tried.contains(&act) {
                                continue;
                            }
                            if tt_action.as_ref() == Some(&act) {
                                // Only reached when the TT action failed validation (searched
                                // ones are in `pre_tried`): it stays unsearched.
                                skipped = true;
                                continue;
                            }
                            batch.push((self.action_score(s, &act, ply), act));
                        }
                    }
                }
                batch.sort_by(|a, b| b.0.cmp(&a.0));
                for (sc, act) in batch {
                    if searched >= cap {
                        skipped = true;
                        break;
                    }
                    let mark = Self::make(s, &act);
                    let quiet = sc < 10_000;
                    let mut r = 0;
                    let mut child_check = None;
                    // Checking actions are not reduced: mating lines run through them.
                    if depth >= LMR_MIN_DEPTH && searched >= LMR_MIN_INDEX && quiet {
                        let ic = self.in_check(s);
                        child_check = Some(ic);
                        if !ic {
                            r = (lmr(depth, searched) - pv as i32).clamp(0, depth - 2);
                        }
                    }
                    let mut v;
                    self.known_check = child_check;
                    if searched == 0 {
                        v = -self.negamax(s, depth - 1, ply + 1, -beta, -alpha, pv);
                    } else {
                        v = -self.negamax(s, depth - 1 - r, ply + 1, -alpha - 1, -alpha, false);
                        if !self.aborted && v > alpha && r > 0 {
                            self.known_check = child_check;
                            v = -self.negamax(s, depth - 1, ply + 1, -alpha - 1, -alpha, false);
                        }
                        if !self.aborted && v > alpha && v < beta && pv {
                            self.known_check = child_check;
                            v = -self.negamax(s, depth - 1, ply + 1, -beta, -alpha, true);
                        }
                    }
                    s.unmake(mark);
                    if self.aborted {
                        break 'search Some(0);
                    }
                    searched += 1;
                    all_proven &= self.proven;
                    if v > best {
                        best = v;
                        best_proven = self.proven;
                        best_action = act.clone();
                        if v > alpha {
                            alpha = v;
                        }
                    }
                    if alpha >= beta {
                        self.on_cutoff(s, &act, depth, ply);
                        let proven = mate_proven(best, best_proven, false);
                        self.tt_store(key, depth, Bound::Lower, mate_to_tt(best, ply), proven, &best_action);
                        self.proven = proven;
                        break 'search Some(best);
                    }
                }
            }
            None
        };
        // A mated score needs every legal action searched: the cursor ran out, none was left
        // out, and `without_future` dropped no move on an optional board. (Only worked out
        // when it matters: for a mated score.)
        let all_proven = all_proven
            && outcome.is_none()
            && best < -MATE + 1000
            && exhausted
            && !skipped
            && !full.as_ref().is_some_and(|f| Self::trims_future(s, &nb.info, f));
        self.bufs.push(nb);
        if let Some(v) = outcome {
            // (A cutoff has set `proven`; an abort returns 0.)
            return v;
        }
        if searched == 0 && generated == 0 && tt_action.is_none() {
            return self.terminal_score(s, ply);
        }
        if searched == 0 {
            // Only the TT action existed but failed validation; fall back to static eval.
            self.proven = false;
            return self.static_eval(s);
        }
        let bound = if best > alpha0 { Bound::Exact } else { Bound::Upper };
        let proven = mate_proven(best, best_proven, all_proven);
        self.tt_store(key, depth, bound, mate_to_tt(best, ply), proven, &best_action);
        self.proven = proven;
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
            let Some(act) = self.tt.probe(s.hash(), |e| e.action.clone()) else { break };
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

/// The reported score of an internal `score`: "mate N" for a proven mate, otherwise "cp X"
/// on the reported scale (an unproven mate counts as the largest non-mate score), followed by
/// "wdl W D L" from the evaluation's logistic.
pub fn score_text(score: i32, proven: bool) -> String {
    if proven && score > MATE - 1000 {
        format!("mate {} wdl 1000 0 0", (MATE - score + 1) / 2)
    } else if proven && score < -MATE + 1000 {
        format!("mate -{} wdl 0 0 1000", (MATE + score + 1) / 2)
    } else {
        let s = score.clamp(-(MATE - 1000), MATE - 1000);
        let (w, d, l) = super::wdl(s);
        format!("cp {} wdl {} {} {}", super::report_cp(s), w, d, l)
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

/// Most search threads `Threads` accepts.
pub const MAX_THREADS: usize = 256;
/// Stack of each helper search thread (the recursion runs deep in many-timeline positions).
const SEARCH_STACK: usize = 64 << 20;
/// How long the main thread waits for the helpers' results once it is done (ms). Helpers stop
/// within a node, but one the OS has not scheduled can take a time slice or more: it is left
/// to finish on its own (and sits out the vote) rather than holding up the action.
const HELPER_GRACE_MS: u64 = 3;

/// One search for a helper thread.
struct HelperJob {
    root: State,
    roots: Vec<(i32, MoveSeq)>,
    /// Every legal root action is in `roots` (for proving the root mated).
    roots_complete: bool,
    max_depth: i32,
    start: Instant,
    tt: Arc<TtTable>,
    stop: Arc<AtomicBool>,
    /// Node counters of this search's helpers (this one's: `thread_id - 1`).
    counters: Arc<Vec<NodeCounter>>,
    done: std::sync::mpsc::Sender<ThreadResult>,
}

enum HelperMsg {
    Search(Box<HelperJob>),
    NewGame,
}

/// A Lazy SMP helper: a thread with its own searcher (killers, history, buffers, kept between
/// searches), searching the jobs it is sent in order (one still finishing a stopped search
/// joins the next as soon as it is done). It exits once its `Helper` is dropped and its
/// current search is over.
struct Helper {
    tx: std::sync::mpsc::Sender<HelperMsg>,
}

/// Run the calling thread (the main search thread) above normal priority: on a busy machine
/// it then keeps the time instead of waiting for a core behind its own helpers. (Helpers stay
/// at normal priority: below it, any other busy process would starve them.)
fn raise_thread_priority() {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentThread() -> *mut std::ffi::c_void;
            fn SetThreadPriority(thread: *mut std::ffi::c_void, priority: i32) -> i32;
        }
        const THREAD_PRIORITY_ABOVE_NORMAL: i32 = 1;
        // SAFETY: plain Win32 calls on the current thread's pseudo-handle.
        unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL) };
    }
}

impl Helper {
    fn spawn(thread_id: usize) -> Option<Helper> {
        let (tx, rx) = std::sync::mpsc::channel::<HelperMsg>();
        std::thread::Builder::new()
            .name(format!("search {}", thread_id))
            .stack_size(SEARCH_STACK)
            .spawn(move || {
                // Between searches the helper holds no table, so a replaced one is freed.
                let no_tt = Arc::new(TtTable::with_len(1));
                let mut e = AlphaBeta::with_tt(no_tt.clone());
                e.thread_id = thread_id;
                while let Ok(msg) = rx.recv() {
                    match msg {
                        HelperMsg::NewGame => e.history.iter_mut().for_each(|h| *h = 0),
                        HelperMsg::Search(job) => {
                            let HelperJob { root, roots, roots_complete, max_depth, start, tt, stop, counters, done } = *job;
                            if stop.load(Ordering::Relaxed) {
                                continue; // That search is over already.
                            }
                            e.tt = tt;
                            e.begin_search(&stop, None, None);
                            e.nodes_out = &counters[thread_id - 1].0;
                            let mut r = e.iterate(&root, roots, roots_complete, max_depth, start, None, &mut |_| {}, &[]);
                            r.nodes = e.nodes;
                            e.nodes_out = std::ptr::null();
                            e.stop_ptr = std::ptr::null();
                            e.tt = no_tt.clone();
                            // The main thread may have moved on without this result.
                            let _ = done.send(r);
                        }
                    }
                }
            })
            .ok()?;
        Some(Helper { tx })
    }
}

impl AlphaBeta {
    pub fn new_game(&mut self) {
        let n = self.tt.len();
        self.set_tt(|| TtTable::with_len(n));
        for h in self.history.iter_mut() {
            *h = 0;
        }
        for helper in &self.helpers {
            let _ = helper.tx.send(HelperMsg::NewGame);
        }
    }

    /// Replace the (shared) transposition table, freeing the old one first.
    fn set_tt(&mut self, make: impl FnOnce() -> TtTable) {
        self.tt = Arc::new(TtTable::with_len(1));
        self.tt = Arc::new(make());
    }

    /// Returns true when the option was recognized: `Hash` (MB) or `Threads`.
    pub fn set_option(&mut self, key: &str, value: &OptionValue) -> bool {
        match (key, value.as_f64()) {
            ("Hash", Some(mb)) => {
                self.set_tt(|| TtTable::new((mb as usize).max(1)));
                true
            }
            ("Threads", Some(n)) => {
                self.threads = (n.max(1.0) as usize).min(MAX_THREADS);
                self.helpers.truncate(self.threads - 1);
                true
            }
            _ => false,
        }
    }

    /// Search threads in use (`Threads`).
    pub fn threads(&self) -> usize {
        self.threads
    }

    /// Nodes the last search visited, over all its threads.
    pub fn last_search_nodes(&self) -> u64 {
        self.nodes + self.helper_nodes
    }

    /// Threads whose results the last search collected (the main thread included).
    pub fn last_search_threads(&self) -> usize {
        1 + self.helper_results
    }

    /// Ready this searcher for a new search.
    fn begin_search(&mut self, stop: &AtomicBool, node_limit: Option<u64>, deadline: Option<Instant>) {
        self.stop_ptr = stop as *const AtomicBool;
        self.nodes = 0;
        self.helper_nodes = 0;
        self.helper_results = 0;
        self.known.clear();
        self.aborted = false;
        self.node_limit = node_limit;
        self.deadline = deadline;
        for k in self.killers.iter_mut() {
            *k = [None, None];
        }
        self.qkillers.iter_mut().for_each(|k| *k = None);
    }

    /// Search `root` and return the action to play. With `Threads` above 1 this is Lazy SMP
    /// (as in Apeiron): helper threads run the same iterative deepening on the same root into
    /// the shared transposition table, odd ones a depth ahead, with no clock of their own; the
    /// calling (main) thread keeps the time, reports progress, and stops the helpers when it
    /// is done; the action played is chosen by the vote of the threads that report back within
    /// a few milliseconds (never waiting longer on a helper).
    pub fn find_best_move(&mut self, root: &State, limits: &Limits, stop: &AtomicBool, info: &mut dyn FnMut(String)) -> Option<Action> {
        let start = Instant::now();
        let plan = TimePlan::new(limits, Self::mandatory_boards(root));
        let deadline = plan.as_ref().map(|p| start + Duration::from_millis(p.maximum.saturating_sub(5).max(1)));
        self.begin_search(stop, limits.nodes, deadline);
        let max_depth = limits.depth.unwrap_or(64) as i32;

        // Root actions (all of them, up to the root cap), ordered heuristically.
        let (hc, ss) = HcInfo::build_hc(root);
        let mut agen = BestFirstCursor::default();
        self.start_actions(root, &hc, &mut agen, &mut OrderScratch::default(), Self::without_future(root, &hc, &ss), false);
        let mut full = Some(ss);
        let mut roots: Vec<(i32, MoveSeq)> = Vec::new();
        // Enumerating every root action can take seconds with many timelines: spend at most
        // a quarter of the budget on it (always keeping at least one action), and stop at once
        // when told to.
        let gen_deadline = plan.as_ref().map(|p| start + Duration::from_millis(p.optimum / 4));
        let mut complete = false;
        while roots.len() < ROOT_CAP {
            if !roots.is_empty() && (gen_deadline.map(|d| Instant::now() >= d).unwrap_or(false) || stop.load(Ordering::Relaxed)) {
                break;
            }
            match agen.next(&hc, root) {
                Some(a) => roots.push((self.action_score(root, &a, 0), a)),
                None if roots.is_empty() && full.is_some() => agen.restart(&hc, full.take().unwrap()),
                None => {
                    complete = true;
                    break;
                }
            }
        }
        if roots.is_empty() {
            return None;
        }
        // Every legal root action is searched (for proving that the root is mated).
        let roots_complete = complete && !full.as_ref().is_some_and(|f| Self::trims_future(root, &hc, f));
        // A forced action on the clock is played at once.
        if complete && roots.len() == 1 && plan.as_ref().is_some_and(|p| p.clock) {
            return Some(Action::from_moveseq(&roots[0].1, root));
        }
        roots.sort_by(|a, b| b.0.cmp(&a.0));
        if self.threads <= 1 {
            let r = self.iterate(root, roots, roots_complete, max_depth, start, plan.as_ref(), info, &[]);
            self.last_score = r.score;
            return Some(Action::from_moveseq(&r.best, root));
        }

        raise_thread_priority();
        // A helper that cannot start is done without (the search stays correct).
        while self.helpers.len() < self.threads - 1 {
            match Helper::spawn(self.helpers.len() + 1) {
                Some(h) => self.helpers.push(h),
                None => break,
            }
        }
        let counters: Arc<Vec<NodeCounter>> = Arc::new((0..self.helpers.len()).map(|_| NodeCounter::default()).collect());
        let helper_stop = Arc::new(AtomicBool::new(false));
        let (done, results_rx) = std::sync::mpsc::channel();
        let mut launched = 0;
        for h in &self.helpers {
            let job = HelperJob {
                root: root.clone(),
                roots: roots.clone(),
                roots_complete,
                max_depth,
                start,
                tt: self.tt.clone(),
                stop: helper_stop.clone(),
                counters: counters.clone(),
                done: done.clone(),
            };
            if h.tx.send(HelperMsg::Search(Box::new(job))).is_ok() {
                launched += 1;
            }
        }
        drop(done);
        let main = self.iterate(root, roots, roots_complete, max_depth, start, plan.as_ref(), info, &counters);
        helper_stop.store(true, Ordering::Relaxed);
        let mut results = vec![main];
        let grace = Instant::now() + Duration::from_millis(HELPER_GRACE_MS);
        while results.len() <= launched {
            match results_rx.recv_timeout(grace.saturating_duration_since(Instant::now())) {
                Ok(r) => results.push(r),
                Err(_) => break,
            }
        }
        // Counted as published, including helpers whose results came too late.
        self.helper_nodes = counters.iter().map(|c| c.0.load(Ordering::Relaxed)).sum();
        self.helper_results = results.len() - 1;
        for r in &mut results {
            r.pv_len = self.pv_line(root, &r.best, 3).len();
        }
        let chosen = &results[select_best_thread(&results)];
        self.last_score = chosen.score;
        Some(Action::from_moveseq(&chosen.best, root))
    }

    /// Iterative deepening over the root actions `roots` (ordered best first; all legal ones
    /// when `roots_complete`). The main thread (`thread_id` 0) keeps the time of `plan` and
    /// reports each depth through `info` with the helpers' nodes (`helper_nodes`) added;
    /// helpers search until stopped.
    #[allow(clippy::too_many_arguments)]
    fn iterate(
        &mut self,
        root: &State,
        mut roots: Vec<(i32, MoveSeq)>,
        roots_complete: bool,
        max_depth: i32,
        start: Instant,
        plan: Option<&TimePlan>,
        info: &mut dyn FnMut(String),
        helper_nodes: &[NodeCounter],
    ) -> ThreadResult {
        let main = self.thread_id == 0;
        let mut work = root.clone();
        work.enable_undo();
        let mut best: MoveSeq = roots[0].1.clone();
        let mut best_score: i32 = 0;
        // `best_score` is a proven mate (see `proven`): only then is it reported as a mate.
        let mut best_proven = false;
        let mut completed_depth = 0;
        self.seek_mate = false;
        // Clock stability: decaying count of best-action changes and running average score.
        let mut changes = 0.0f64;
        let mut avg_score: Option<i32> = None;
        // Lazy SMP: odd helpers skip depth 1 and run a depth ahead, so the threads diverge
        // and fill the table at a spread of depths instead of re-walking the same iteration.
        let ahead = self.thread_id % 2 == 1;
        let first = if ahead { 2.min(max_depth) } else { 1 };
        for base_depth in first..=max_depth {
            let depth = if ahead { (base_depth + 1).min(max_depth) } else { base_depth };
            let prev_best = best.clone();
            // seekMate bar: 3000 + 100000 / depth^2 (Stockfish's form, in our units).
            self.seek_mate = depth > 1 && best_score.abs() >= 3000 + 100_000 / (depth * depth);
            let mut alpha = -INF;
            let beta = INF;
            let mut iter_best: Option<(i32, MoveSeq, bool)> = None;
            let mut all_proven = roots_complete;
            let mut scores: Vec<i32> = Vec::with_capacity(roots.len());
            for (k, (_, act)) in roots.iter().enumerate() {
                let mark = Self::make(&mut work, act);
                let v = if k == 0 {
                    -self.negamax(&mut work, depth - 1, 1, -beta, -alpha, true)
                } else {
                    // Late root actions are reduced, never pruned.
                    let child_check = if depth >= 3 && k >= ROOT_LMR_MIN_INDEX { Some(self.in_check(&work)) } else { None };
                    let r = if child_check == Some(false) { (lmr(depth, k) - 1).clamp(0, depth - 2) } else { 0 };
                    self.known_check = child_check;
                    let mut v = -self.negamax(&mut work, depth - 1 - r, 1, -alpha - 1, -alpha, false);
                    if !self.aborted && v > alpha && r > 0 {
                        self.known_check = child_check;
                        v = -self.negamax(&mut work, depth - 1, 1, -alpha - 1, -alpha, false);
                    }
                    if !self.aborted && v > alpha {
                        self.known_check = child_check;
                        v = -self.negamax(&mut work, depth - 1, 1, -beta, -alpha, true);
                    }
                    v
                };
                work.unmake(mark);
                if self.aborted {
                    break;
                }
                scores.push(v);
                all_proven &= self.proven;
                if v > alpha {
                    alpha = v;
                    iter_best = Some((v, act.clone(), self.proven));
                }
            }
            // A partially searched iteration still improves on the previous best, except that a
            // mate an earlier depth proved survives a depth that comes back longer, not mating
            // or interrupted, and an interrupted depth's loss is not trusted (Stockfish's
            // forgottenMate and abortedLossSearch).
            // (The kept mate need not be proven: which action is played and how the next depth
            // is ordered are search decisions, left as they were. It is reported as proven or
            // not, as it was found.)
            let mut kept = false;
            if let Some((v, act, proven)) = iter_best {
                kept = depth > 1 && ((best_score > MATE - 1000 && v < best_score) || (self.aborted && v < -(MATE - 1000)));
                if !kept {
                    best = act;
                    best_score = v;
                    best_proven = mate_proven(v, proven, all_proven && !self.aborted);
                }
            }
            let elapsed = start.elapsed().as_secs_f64();
            if !self.aborted {
                completed_depth = depth;
                // Reorder roots: best first, rest by this iteration's scores.
                let mut idx: Vec<usize> = (0..roots.len()).collect();
                idx.sort_by(|&a, &b| scores.get(b).copied().unwrap_or(-INF).cmp(&scores.get(a).copied().unwrap_or(-INF)));
                roots = idx.into_iter().map(|i| roots[i].clone()).collect();
                if kept {
                    // The kept mate is searched first at the next depth.
                    if let Some(i) = roots.iter().position(|r| r.1 == best) {
                        roots[..=i].rotate_right(1);
                    }
                }
            }
            if !self.aborted && main {
                let pv = self.pv_line(root, &best, depth as usize);
                let score_txt = score_text(best_score, best_proven);
                let mut st = root.clone();
                let mut pv_txt = Vec::new();
                for a in &pv {
                    pv_txt.push(format!("{} submit", action_text(&st, a)));
                    st = Self::child(&st, a);
                }
                let nodes = self.nodes + helper_nodes.iter().map(|c| c.0.load(Ordering::Relaxed)).sum::<u64>();
                info(format!(
                    "depth {} score {} nodes {} nps {:.0} time {} roots {} pv {}",
                    depth,
                    score_txt,
                    nodes,
                    nodes as f64 / elapsed.max(1e-6),
                    (elapsed * 1000.0) as u64,
                    roots.len(),
                    pv_txt.join(" ")
                ));
            }
            if self.aborted {
                break;
            }
            if let Some(p) = plan.filter(|p| p.clock) {
                changes = changes / 2.0 + if depth > 1 && best != prev_best { 1.0 } else { 0.0 };
                let falling = avg_score.map_or(1.0, |a| (1.0 + (a - best_score) as f64 / 150.0).clamp(0.75, 1.6));
                avg_score = Some(avg_score.map_or(best_score, |a| (a + best_score) / 2));
                let instability = (0.8 + 0.6 * changes).min(2.0);
                // The next depth costs about as much as all before it together: none is
                // started past half the target.
                if elapsed * 1000.0 > p.target(falling, instability) * 0.5 {
                    break;
                }
            }
            // Keep searching after a mate (pruning can report false or long mates) until the
            // mate is well inside the searched depth. On a time budget an unproven mate does
            // not end the search early (that only changes the time used); depth- and
            // node-limited searches stop as they always did, so they stay reproducible.
            // (Helpers search on until stopped.)
            if main && best_score.abs() > MATE - 1000 && depth >= 2 * (MATE - best_score.abs()) + 2 && (best_proven || plan.is_none()) {
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
        ThreadResult { best, score: best_score, proven: best_proven, completed_depth, pv_len: 0, nodes: self.nodes, thread_id: self.thread_id }
    }

    /// The score of the action the last `find_best_move` returned (internal units, from the
    /// side to move's view; mates as `MATE - plies`, proven or not).
    pub fn last_score(&self) -> i32 {
        self.last_score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A thread's result; mate scores proven.
    fn result(from: u32, score: i32, depth: i32, thread_id: usize) -> ThreadResult {
        let m = FullMove { from: crate::core::vec4::Vec4(from), to: crate::core::vec4::Vec4(from + 1) };
        ThreadResult { best: MoveSeq::from(&[m][..]), score, proven: true, completed_depth: depth, pv_len: 5, nodes: 0, thread_id }
    }

    fn unproven(mut r: ThreadResult) -> ThreadResult {
        r.proven = false;
        r
    }

    #[test]
    fn thread_vote_prefers_proven_mates() {
        let win = MATE - 20;
        // A proven mate is not given up for a faster mate that is not proven ...
        assert_eq!(select_best_thread(&[result(1, win, 10, 0), unproven(result(2, win + 6, 10, 1))]), 0);
        // ... and wins outright over one, even outvoted.
        let r = [unproven(result(1, win + 6, 10, 0)), unproven(result(1, win + 6, 10, 1)), result(2, win, 10, 2)];
        assert_eq!(select_best_thread(&r), 2);
    }

    #[test]
    fn thread_vote_matches_stockfish() {
        let (win, loss, long_loss) = (MATE - 20, -MATE + 20, -MATE + 60);
        // A losing best yields to a normal thread; a normal best never to a proven loss.
        assert_eq!(select_best_thread(&[result(1, loss, 10, 0), result(2, 50, 10, 1)]), 1);
        assert_eq!(select_best_thread(&[result(1, 50, 10, 0), result(2, loss, 10, 1)]), 0);
        // Between losses the longest resistance, between wins the fastest mate.
        assert_eq!(select_best_thread(&[result(1, loss, 10, 0), result(2, long_loss, 10, 1)]), 1);
        assert_eq!(select_best_thread(&[result(1, win, 10, 0), result(2, win + 5, 10, 1)]), 1);
        // Votes pool by action: two threads agreeing outvote a slightly better lone one.
        assert_eq!(select_best_thread(&[result(1, 60, 10, 0), result(2, 50, 10, 1), result(2, 50, 10, 2)]), 1);
        // A deeper thread outvotes a shallower one at equal score.
        assert_eq!(select_best_thread(&[result(1, 50, 8, 0), result(2, 50, 10, 1)]), 1);
        // A thread that finished no iteration has no vote.
        assert_eq!(select_best_thread(&[result(1, 0, 3, 0), result(2, 900, 0, 1)]), 0);
        assert_eq!(select_best_thread(&[result(1, 0, 0, 0), result(2, 10, 2, 1)]), 1);
    }

    fn start_with_moves(moves: &str) -> State {
        let mut s = crate::core::variants::startpos();
        crate::core::variants::apply_uci_moves(&mut s, moves).unwrap();
        s
    }

    #[test]
    fn smp_search_returns_legal_actions_and_stops() {
        let mut e = AlphaBeta::new();
        assert!(e.set_option("Threads", &OptionValue::Int(4)));
        e.set_option("Hash", &OptionValue::Int(16));
        let stop = AtomicBool::new(false);
        for (moves, ms) in [("", 150), ("(0T1)e2e4 submit", 150), ("(0T1)e2e4 submit (0T1)e7e5 submit (0T2)g1f3 submit", 300)] {
            let s = start_with_moves(moves);
            let t = Instant::now();
            let mut last = String::new();
            let a = e.find_best_move(&s, &Limits { movetime: Some(ms), ..Default::default() }, &stop, &mut |m| last = m).expect("an action");
            assert!(t.elapsed() < Duration::from_millis(ms + 1000), "search overran its time");
            let mut c = s.clone();
            c.enable_undo();
            let seq: MoveSeq = a.moves().iter().map(|m| m.fm).collect();
            assert!(AlphaBeta::make_checked(&mut c, &seq).is_some(), "illegal action {:?}", seq);
            assert!(last.starts_with("depth"), "no report: {}", last);
        }
        // Stopped from outside, a search without limits returns promptly.
        let s = start_with_moves("(0T1)e2e4 submit (0T1)e7e5 submit");
        let stop = Arc::new(AtomicBool::new(false));
        let st = stop.clone();
        let stopper = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            st.store(true, Ordering::SeqCst);
        });
        let t = Instant::now();
        assert!(e.find_best_move(&s, &Limits::default(), &stop, &mut |_| {}).is_some());
        assert!(t.elapsed() < Duration::from_millis(200 + 300), "stop took {:?}", t.elapsed());
        stopper.join().unwrap();
        // The search right after (a reply after pondering) has every helper again.
        let free = AtomicBool::new(false);
        e.find_best_move(&s, &Limits { movetime: Some(100), ..Default::default() }, &free, &mut |_| {});
        assert!(e.helper_nodes > 0, "the helpers sat the search out: {} {} {}", e.nodes, e.helper_nodes, e.helper_results);
        // Back to one thread: the helpers are dropped.
        e.set_option("Threads", &OptionValue::Int(1));
        assert!(e.helpers.is_empty());
    }
}
