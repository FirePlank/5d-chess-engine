//! Alpha-beta engine: iterative-deepening PVS over complete actions.
//! Actions are generated lazily, best-first, by the hypercuboid search; checks come first.
//! Quiescence searches captures and promotions, with one quiet filler move per board.

use super::hce;
use super::time::TimePlan;
use super::{Limits, OptionValue};
use crate::core::action::{Action, FullMove, MoveSeq};
use crate::core::bitboard::{marked_pos, pmask, Bitboard};
use crate::core::board::Board;
use crate::core::check_position::{BoardOverlay, CheckBuf, CheckKind, CheckPosition, SpReach};
use crate::core::hypercuboid::{has_physical_check, interrupt_on, interrupted, plays_wasted_move, BestFirstCursor, Entry, HcCursor, HcInfo, NeedBranch};
use crate::misc::geometry::{Hc, Point, Slice};
use crate::core::multiverse::{active_range_for, next_turn};
use crate::core::piece::*;
use crate::core::state::promotion_rank;
use crate::core::state::{Mark, State};
use crate::misc::geometry::SearchSpace;
use crate::misc::integer_set::{Index, IntegerSet};
use crate::misc::ordering::{AxisOrders, HcOrdering};
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const MATE: i32 = 30000;

/// Diagnostics (`ktools matescan` with MREGION_VERIFY): every region `no_mate_region` returns
/// is checked on up to `REGION_VERIFY_POINTS` of its legal actions, its fact tested directly.
pub static REGION_VERIFY: AtomicBool = AtomicBool::new(false);
/// Regions returned, actions checked, actions breaking their region's fact.
pub static REGION_STATS: [AtomicU64; 3] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];
const REGION_VERIFY_POINTS: usize = 2000;

/// The squares on which two boards differ (any piece, color or unmoved flag).
fn board_diff(a: &Board, b: &Board) -> Bitboard {
    a.raw_bbs().iter().zip(b.raw_bbs().iter()).fold(0, |d, (x, y)| d | (x ^ y))
}


pub const INF: i32 = 32000;
const MAX_PLY: usize = 128;

/// Actions searched at non-PV nodes by remaining depth (index = depth, last repeats).
const LMP: [usize; 5] = [0, 20, 32, 44, 56];
/// Multiplier for the PV-node action cap.
const PV_CAP_MULT: usize = 4;
/// Root actions considered.
const ROOT_CAP: usize = 1000;
/// Actions generated (and sorted by action score) at a time at interior nodes: the first
/// batch holds one action and each next one twice as many, up to this.
const BATCH: usize = 12;
const QDEPTH: i32 = 2;
const QCAP: usize = 4;
/// Internal iterative reduction: nodes of this depth or more without a TT action lose a ply.
const IIR_DEPTH: i32 = 4;
/// Singular extension: depth from which the TT action is tested, and the margin per depth.
const SING_DEPTH: i32 = 4;
const SING_MARGIN: i32 = 100;
const LMR_MIN_DEPTH: i32 = 3;
const LMR_MIN_INDEX: usize = 4;
/// Late root actions (by the previous iteration's order) are searched shallower from this
/// index on, and re-searched at full depth if they beat alpha.
const ROOT_LMR_MIN_INDEX: usize = 6;
/// Hypercuboids per checking travel in the root's mate-in-one probes. Root pre-search work is
/// bounded by counts, not time, so it is the same on any machine and for any `movetime`.
const ROOT_PROBE_STEPS: u32 = 2048;
/// Root mate-in-one scan budget: the first `ROOT_SCAN_ACTIONS` actions are always tried; the
/// rest (up to `ROOT_SCAN_MORE_ACTIONS`) only within a time share: 1/`ROOT_SCAN_MORE_DIV` of the
/// optimum time capped at `ROOT_SCAN_MORE_CAP_MS` in a game, `ROOT_SCAN_ANALYSIS_MS` in analysis.
const ROOT_SCAN_ACTIONS: usize = 1024;
const ROOT_SCAN_MORE_STEPS: u32 = 1 << 18;
const ROOT_SCAN_MORE_ACTIONS: usize = 1 << 15;
const ROOT_SCAN_MORE_DIV: u64 = 12;
const ROOT_SCAN_MORE_CAP_MS: u64 = 100;
const ROOT_SCAN_ANALYSIS_MS: u64 = 1000;
/// Hypercuboids an escape search that ran out of its quick budget (`TRAVEL_MATE_STEPS`) may go
/// on to at the root: a mate has no escape, so proving it takes the whole space of them.
const MATE_RECHECK_STEPS: u32 = 4096;
/// Hypercuboids an escape search takes between looks at the clock once a deadline applies.
const MATE_CHUNK_STEPS: u32 = 256;
/// Escapes found last, replayed (checked for legality) before a new escape search is built:
/// the replies to a scan's checks are mostly the same few actions.
const ESCAPE_MEMO: usize = 4;
/// Positions whose escape search result is remembered (see `MateMemo`) before it is cleared.
const MATE_MEMO_CAP: usize = 1 << 18;
/// A root action's region (`no_mate_region`) is looked for only when at least two of its axes
/// play a physical move with this many alternatives (a smaller region costs more than it saves).
const REGION_MIN_MOVES: usize = 4;
const ROOT_CHECKING_SCAN_STEPS: u32 = 512;
const ROOT_CHECKING_SCAN_ACTIONS: usize = 64;
/// Hypercuboids to the first root action in the space without moves on optional boards (past
/// them the whole space is enumerated instead), and to the rest of the root actions.
const ROOT_FIRST_STEPS: u32 = 1 << 16;
const ROOT_GEN_STEPS: u32 = 1 << 20;
/// Root actions with a move on an optional board tried for a checkmate in one.
const ROOT_FUTURE_MATE_PROBE: usize = 16;
/// Root actions with a move on an optional board added when every root action loses to a
/// mate (interior nodes take up to their action cap).
const ROOT_WIDEN_CAP: usize = 256;
/// Interior nodes up to this ply (the root is 0) look for a checkmate in one by a checking
/// travel the node would search late or never (`travel_mate`).
const TRAVEL_MATE_PLY: usize = 2;
/// Hypercuboids the search for an evasion of such a check may look at (an evasion is usually
/// found in a few; past this the check counts as no mate).
const TRAVEL_MATE_STEPS: u32 = 64;
/// Legal completions (moves on the other boards the side to move must play) tried with each
/// checking travel of `travel_mate`.
const TRAVEL_MATE_COMPLETIONS: usize = 8;
/// The same at the root, which has the time for more.
const ROOT_TRAVEL_MATE_COMPLETIONS: usize = 512;
/// Hypercuboids the cursor may take on a capturing travel (`optional_travels`) before it gives
/// it up (when most of its combinations with the other boards' moves are illegal, finding one
/// can take thousands).
const TRAVEL_CAPTURE_STEPS: u32 = 16;
/// Aspiration windows: from ASP_MIN_DEPTH the root searches [prev - w, prev + w], w starting at
/// ASP_WINDOW and multiplied by ASP_FAIL_MULT on each fail; past ASP_MAX_WINDOW or
/// ASP_MAX_TRIES tries, a full-window search.
const ASP_MIN_DEPTH: i32 = 2;
const ASP_WINDOW: i32 = 750;
const ASP_FAIL_MULT: i32 = 2;
const ASP_MAX_WINDOW: i32 = ASP_WINDOW * 1000 / 60;
const ASP_MAX_TRIES: i32 = 4;

/// Late move reduction for the `index`-th action at `depth`: 1 + ln(depth) ln(index) / 3.
#[inline]
fn lmr(depth: i32, index: usize) -> i32 {
    (1.0 + (depth.max(1) as f32).ln() * (index.max(1) as f32).ln() / 3.0) as i32
}
/// Ordering bonus of a checking semimove (above every capture).
const CHECK_BONUS: i32 = 30_000;
/// Local nodes the f7 sac table's probes may use before every search, whatever the clock (the
/// same work for the same position, in analysis and in games).
const SAC_PREP_NODES: u64 = 50_000;
/// Reverse futility pruning: at non-PV nodes up to this depth, return the static eval when
/// it beats beta by RFP_MARGIN per ply.
const RFP_DEPTH: i32 = 2;
const RFP_MARGIN: i32 = 250;
/// Null move (a pass, i.e. a phantom turn): at non-PV nodes from this depth, when the static
/// eval is at least beta, search the pass with depth reduced by NMP_R + depth / NMP_DIV.
const NMP_MIN_DEPTH: i32 = 3;
const NMP_R: i32 = 2;
const NMP_DIV: i32 = 4;
const DEFAULT_HASH_MB: usize = 64;
/// Static evaluations are kept within +-EVAL_MAX, well clear of mate scores.
const EVAL_MAX: i32 = 20000;

/// How far the search for a checkmate in one goes (`MateVerify` option, `MateVerify::Auto` by
/// default): in a game a share of the move's time, in an analysis a few seconds.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MateVerify {
    /// A game: the first `ROOT_SCAN_ACTIONS` actions, then 1/12 of the optimum time (at most
    /// 100 ms). An analysis (no clock): `ROOT_SCAN_ANALYSIS_MS`.
    #[default]
    Auto,
    /// That many milliseconds past the counted part (in a game at most a quarter of the optimum).
    Millis(u64),
    /// Every legal action, however long it takes (in a game: a quarter of the optimum).
    Exhaustive,
}

impl MateVerify {
    /// From an option value: `auto`, `exhaustive`, milliseconds, or the integer coding
    /// (0 auto, negative exhaustive, positive milliseconds).
    pub fn from_option(v: &OptionValue) -> Option<MateVerify> {
        match v {
            OptionValue::Str(s) if s.eq_ignore_ascii_case("auto") => Some(MateVerify::Auto),
            OptionValue::Str(s) if s.eq_ignore_ascii_case("exhaustive") => Some(MateVerify::Exhaustive),
            OptionValue::Str(s) => s.trim().parse::<i64>().ok().map(Self::from_int),
            OptionValue::Int(n) => Some(Self::from_int(*n)),
            OptionValue::Float(f) if f.is_finite() => Some(Self::from_int(*f as i64)),
            _ => None,
        }
    }

    fn from_int(n: i64) -> MateVerify {
        match n {
            0 => MateVerify::Auto,
            n if n < 0 => MateVerify::Exhaustive,
            n => MateVerify::Millis(n as u64),
        }
    }
}

/// What a scan of the legal actions for a checkmate in one may do.
#[derive(Clone, Copy, Debug)]
struct MateBudget {
    /// The first this many actions are tried whatever the time.
    free: usize,
    /// Most actions tried.
    actions: usize,
    /// Hypercuboids each of the two enumerations may look at.
    steps: u32,
    /// Past `free` actions the scan goes on only until then.
    deadline: Option<Instant>,
}

/// How a scan ended.
#[derive(Debug)]
enum ScanEnd {
    Found(MoveSeq),
    /// Every legal action was tried and each is proven no mate: there is no checkmate in one.
    Exhausted,
    /// The budget ran out after `tried` distinct actions, `unresolved` of them checks whose
    /// escape search ran out too (possible mates).
    OutOfBudget { tried: usize, unresolved: usize },
}

/// Hashes of `u64` position keys (already uniformly distributed) as they are.
#[derive(Default, Clone, Copy)]
struct KeyHasher(u64);

impl std::hash::Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, _: &[u8]) {
        unreachable!("only u64 keys")
    }
    fn write_u64(&mut self, k: u64) {
        self.0 = k;
    }
}

type KeyBuild = std::hash::BuildHasherDefault<KeyHasher>;

/// State shared by a node's mate-in-one scans: positions already reached (skipped when seen
/// again) and checks whose escape search ran out (retried with larger budgets).
#[derive(Default)]
struct ScanMemo {
    seen: std::collections::HashSet<u64, KeyBuild>,
    pending: Vec<MoveSeq>,
}

/// Whether the side to move has a legal action, as far as the hypercuboids looked at show.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Evasion {
    Yes,
    No,
    /// The budget ran out before one was found.
    Unknown,
}

/// Escape search results by position (`State::hash`): proven ones, and the budget a search
/// that ran out had (a smaller one would run out too), so the same check met again (the
/// probes, the scans, the next iteration's interior nodes) costs a lookup.
#[derive(Default)]
struct MateMemo {
    map: std::collections::HashMap<u64, (Evasion, u32), KeyBuild>,
}

impl MateMemo {
    fn get(&self, key: u64, steps: u32) -> Option<Evasion> {
        match self.map.get(&key) {
            Some(&(Evasion::Unknown, tried)) if tried >= steps => Some(Evasion::Unknown),
            Some(&(Evasion::Unknown, _)) | None => None,
            Some(&(e, _)) => Some(e),
        }
    }
    fn put(&mut self, key: u64, e: Evasion, steps: u32) {
        if self.map.len() >= MATE_MEMO_CAP {
            self.map.clear();
        }
        self.map.insert(key, (e, steps));
    }
}

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
    /// Held while the entry is read or written, since actions may live on the heap. Only ever
    /// tried: a thread finding it held treats the probe as a miss or skips the store.
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

/// Where the main thread's last search left its root, for a search with `Limits::resume` of
/// the same position (as Apeiron's analysis resumes a deeper search).
#[derive(Clone)]
struct SearchResume {
    hash: u64,
    /// Root actions, best first, as the last completed iteration ordered them.
    roots: Vec<(i32, MoveSeq)>,
    roots_complete: bool,
    completed_depth: i32,
    best: MoveSeq,
    score: i32,
    proven: bool,
    /// The line of `best` (the triangular PV, see `iterate`).
    pv: Vec<MoveSeq>,
    /// `MultiPV` of that search, and its lines (empty at 1).
    multipv: usize,
    lines: Vec<PvLine>,
}

/// One MultiPV line: an exact root score and its line (the root action first).
type PvLine = (i32, Vec<MoveSeq>);

/// One depth of the MultiPV root search (`AlphaBeta::multipv_pass`).
struct MultiPass {
    /// The best exact score, its action and whether a mate score is proven.
    best: Option<(i32, MoveSeq, bool)>,
    best_pv: Vec<MoveSeq>,
    /// Every root score is proven (and the roots are complete).
    all_proven: bool,
    /// Each searched action's score, in root order.
    scores: Vec<i32>,
    /// The best `MultiPV` exact lines, best first.
    lines: Vec<PvLine>,
}

/// One thread's search result, for the vote between threads.
#[derive(Clone, Debug)]
struct ThreadResult {
    best: MoveSeq,
    score: i32,
    /// A mate `score` is proven (see `AlphaBeta::proven`).
    proven: bool,
    /// Deepest iteration the thread finished (0: none, and then it does not vote).
    completed_depth: i32,
    /// Principal variation of `best` (the search's own, from the triangular PV table).
    pv: Vec<MoveSeq>,
    pv_len: usize,
    nodes: u64,
    thread_id: usize,
    /// With `MultiPV` above 1: the lines of the last completed depth, best first (else empty).
    lines: Vec<PvLine>,
}

/// Picks the thread result to play by Stockfish-style voting, weighted by score and depth.
/// The fastest proven mate wins outright; a loss is never switched to; threads that finished
/// no iteration do not vote.
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
    /// Static ordering scores of boards' physical moves, by `HcInfo::phys_tag` (see
    /// `PhysStatic`).
    phys_static: std::cell::RefCell<Vec<PhysStaticSlot>>,
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
    /// Whether the mate score the last `negamax`/`qsearch` returned is a proven forced mate
    /// (nothing pruned on the losing side). Only meaningful for mate scores; never steers search.
    proven: bool,
    /// Singular-extension search: the TT action it excludes and the key of its node.
    excluded: Option<MoveSeq>,
    excl_key: u64,
    /// Per-node buffers, reused down the recursion.
    #[allow(clippy::vec_box)] // popped and pushed per node: moving a pointer, not the buffers
    bufs: Vec<Box<NodeBufs>>,
    /// Evaluation terms of recently seen boards.
    terms: Box<hce::TermCache>,
    /// Whether the position the next `negamax` call searches is in check, when known.
    known_check: Option<bool>,
    /// Static evaluations and in-check status of recently seen positions.
    known: KnownCache,
    /// The score (internal units, side to move) of the last `find_best_move`.
    last_score: i32,
    /// Whether that score, a mate score, is proven (see `proven`).
    last_proven: bool,
    /// The main thread's last search, for a resumed one.
    resume: Option<SearchResume>,
    /// Search threads (`Threads`), this one included.
    threads: usize,
    /// `MultiPV`: root lines searched exactly and reported (1: the plain search).
    multipv: usize,
    /// `OwnBook`: play a book action (`book::Book::theory`) without searching when the root
    /// is a book position with one.
    own_book: bool,
    /// How far the search for a checkmate in one goes (`MateVerify`).
    mate_verify: MateVerify,
    /// Hypercuboids an escape search that ran out of `TRAVEL_MATE_STEPS` may go on to (0: none;
    /// the root's search sets it for its probes, see `is_mate`).
    mate_recheck: u32,
    /// Escapes found last, most recent first (see `ESCAPE_MEMO`).
    escapes: Vec<MoveSeq>,
    /// Escape search results by position.
    mate_memo: MateMemo,
    skill_level: u8,
    /// `SacBot` (testing): play an f-sac signature travel (`sac::signature_travels`) whenever
    /// one is legal, without a search: the opponent of the sac gauntlet.
    sac_bot: bool,
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
    /// Triangular PV table: `pv_rows[ply][..pv_lens[ply]]` is the best line from the node at
    /// `ply`. Each node clears its row on entry; only PV nodes fill it.
    pv_rows: Vec<Vec<MoveSeq>>,
    pv_lens: Vec<usize>,
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

    /// `gives_check` of a physical move past its own board: the moved piece's superphysical
    /// reach from its line's next board.
    fn reach_check(&mut self, axis: usize, coord: Index) -> bool {
        let info: &'a HcInfo = self.info;
        let Some(mb) = info.get_move_boards(axis, coord) else { return false };
        let s: &'a State = self.s;
        if self.checks.is_none() {
            self.checks = Some(CheckPosition::for_move_candidates_in(s, std::mem::take(&mut self.buf)));
        }
        let checks = self.checks.as_mut().unwrap();
        let player = s.get_present().1;
        let mv = mb.mv;
        let at = next_turn((mv.to.t(), player));
        let l = mv.to.l();
        let i = match self.reach.iter().position(|r| r.0 == l) {
            Some(i) => i,
            None => {
                self.reach.push((l, checks.sp_reach(l, 2 * at.0 + at.1 as i32 + 1)));
                self.reach.len() - 1
            }
        };
        self.reach[i].1.attacks(mb.result, mv.to.xy())
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

/// The part of a physical move's ordering score that depends on its board alone (see
/// `AlphaBeta::coord_score`): kept per board, so that a board met again needs only its moves'
/// history values and superphysical checks.
#[derive(Clone, Copy)]
struct PhysStatic {
    /// Score without the history value and the check bonus.
    base: i32,
    history: u16,
    capture: bool,
    /// Gives check on its own board.
    physical_check: bool,
}

#[derive(Default)]
struct PhysStaticSlot {
    tag: (u64, u64),
    moves: Vec<PhysStatic>,
}

/// Slots of `AlphaBeta::phys_static`.
const PHYS_STATIC_SLOTS: usize = 1024;

/// Scratch of `node_ordering`.
#[derive(Default)]
struct OrderScratch {
    sort: Vec<u64>,
    checks: CheckBuf,
    reach: Vec<(i32, SpReach)>,
    /// History values of the branch coordinates, by coordinate, as they were when the
    /// node was ordered (the branch axes may be scored later).
    branch_history: Vec<i32>,
    /// Existing axes `start_actions` left unscored: every hypercuboid it started on holds a
    /// single coordinate there (boards the side to move need not play, at interior nodes).
    /// `order_skipped` scores them once a wider space needs them.
    skipped: Vec<bool>,
    /// `node_ordering` leaves the `skipped` axes out.
    skip: bool,
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
            excluded: None,
            excl_key: 0,
            bufs: Vec::new(),
            terms: Box::default(),
            phys_static: std::cell::RefCell::new(Vec::new()),
            known_check: None,
            known: KnownCache::new(),
            last_score: 0,
            last_proven: false,
            resume: None,
            threads: 1,
            multipv: 1,
            own_book: false,
            mate_verify: MateVerify::Auto,
            mate_recheck: 0,
            escapes: Vec::new(),
            mate_memo: MateMemo::default(),
            skill_level: 8,
            sac_bot: false,
            helpers: Vec::new(),
            helper_nodes: 0,
            helper_results: 0,
            thread_id: 0,
            nodes_out: std::ptr::null(),
            pv_rows: vec![Vec::new(); MAX_PLY + 2],
            pv_lens: vec![0; MAX_PLY + 2],
        }
    }

    #[inline]
    fn static_eval(&mut self, s: &State) -> i32 {
        crate::misc::prof::scope(9);
        let key = s.hash();
        let i = self.known.slot(key);
        let e = self.known.slots[i];
        if e.key == key && e.flags & KNOWN_EVAL != 0 {
            return e.eval;
        }
        let sd = hce::sides_with(s, &mut *self.terms);
        let mut f = hce::features_from(&sd, s, hce::needs_checks(&self.hce));
        let spatial = if self.skill_level < 8 { Some(super::skill::spatial_score(&f, &self.hce, s.get_present().1)) } else { None };
        let sac = hce::split_sac(&mut f);
        let chess = if hce::CHESS_IN_NET { [0; hce::NF] } else { hce::split_chess(&mut f) };
        let base = hce::score(&f, &self.hce, s.get_present().1);
        let v = match super::evalnet::embedded() {
            Some(net) => base + net.residual(&super::evalnet::inputs_from(&sd, s, base)),
            None => base,
        };
        let v = v + hce::score(&sac, &self.hce, s.get_present().1);
        let v = v + hce::score_chess(&chess, &self.hce, s.get_present().1);
        let v = spatial.map_or(v, |local| super::skill::blend(local, v, self.skill_level));
        // Terms add up over every board, so with many timelines the sum can reach the mate
        // range (where it would be taken for a mate score).
        let v = v.clamp(-EVAL_MAX, EVAL_MAX);
        self.known.store(i, key, |e| {
            e.eval = v;
            e.flags |= KNOWN_EVAL;
        });
        v
    }

    /// Whether the side to move is in check (royal captures threatened if it passed).
    fn in_check(&mut self, s: &State) -> bool {
        crate::misc::prof::scope(10);
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
        if key == self.excl_key && self.excluded.is_some() {
            return;
        }
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
    /// The board-only part of a physical coordinate's `coord_score`.
    fn phys_static(s: &State, info: &HcInfo, n: usize, i: Index) -> PhysStatic {
        let Entry::Physical { m, b, mover, victim } = info.entry(n, i) else { unreachable!("physical coordinate") };
        let (mover, victim) = (*mover, *victim);
        let mut base = -10 * exchange_loss(b, m.to.xy(), s.get_present().1, piece_value(mover));
        if victim != NO_PIECE {
            base += 10_000 + 10 * piece_value(victim) - piece_value(mover) / 10;
        }
        if (mover == PAWN_W || mover == BRAWN_W) && m.to.y() == promotion_rank(s.get_board_size().1, s.get_present().1) {
            base += 9000;
        }
        if mover == KING_W || mover == COMMON_KING_W {
            base -= 200;
        }
        PhysStatic { base, history: history_index_of(mover, m) as u16, capture: victim != NO_PIECE, physical_check: has_physical_check(b, !s.get_present().1) }
    }

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
                if (mover == PAWN_W || mover == BRAWN_W) && m.to.y() == promotion_rank(s.get_board_size().1, s.get_present().1) {
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
        crate::misc::prof::scope(6);
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
            if scratch.skip && n < fba && scratch.skipped.get(n).copied().unwrap_or(false) {
                // A single coordinate in every hypercuboid: its order is never read, and its
                // score adds the same to every bound of a class.
                sc.resize(u[n].max().map(|i| i as usize + 1).unwrap_or(0), 0);
                orders[n].clear();
                continue;
            }
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
            // The physical moves' board-only parts, kept per board (see `PhysStatic`).
            let mut cache = self.phys_static.borrow_mut();
            let statics: Option<&[PhysStatic]> = if n < fba {
                if cache.is_empty() {
                    cache.resize_with(PHYS_STATIC_SLOTS, PhysStaticSlot::default);
                }
                let tag = info.phys_tag(n);
                let fj = info.first_jump(n);
                let slot = &mut cache[tag.0 as usize % PHYS_STATIC_SLOTS];
                if slot.tag != tag || slot.moves.len() != fj.saturating_sub(1) {
                    slot.tag = tag;
                    slot.moves.clear();
                    slot.moves.extend((1..fj).map(|i| Self::phys_static(s, info, n, i as Index)));
                }
                Some(&slot.moves)
            } else {
                None
            };
            // Sort keys: score descending, then coordinate ascending.
            let v = &mut scratch.sort;
            v.clear();
            for i in u[n].iter() {
                let recorded = if n == fba && branch == BranchOrder::Recorded { Some(scratch.branch_history[i as usize]) } else { None };
                let score = match statics {
                    Some(st) if i >= 1 && (i as usize) <= st.len() => {
                        let p = st[i as usize - 1];
                        let mut sc = p.base;
                        if !p.capture {
                            sc += self.history[p.history as usize].clamp(-5000, 5000);
                        }
                        if let Some(d) = detector.as_mut()
                            && (p.physical_check || d.reach_check(n, i)) {
                                sc += CHECK_BONUS;
                            }
                        sc
                    }
                    _ => self.coord_score(s, info, n, i, &mut detector, recorded),
                };
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
                if let Some(n) = optional(l)
                    && !hc[n].contains(0) {
                        continue 'hc;
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

    /// The points of `ss` that `without_future` leaves out: actions with a move on a board the
    /// side to move need not play. Disjoint hypercuboids: the k-th optional axis played, the
    /// optional axes before it not.
    fn future_only(s: &State, info: &HcInfo, ss: &SearchSpace) -> SearchSpace {
        let present = s.get_present();
        let (amin, amax) = s.get_active_range();
        let (lo, hi) = s.get_lines_range();
        let optional: Vec<usize> = (lo..=hi)
            .filter(|&l| !(l >= amin && l <= amax && s.get_timeline_end(l) == present))
            .filter_map(|l| info.line_to_axis(l))
            .collect();
        let mut out = SearchSpace::default();
        for hc in &ss.hcs {
            for (k, &n) in optional.iter().enumerate() {
                if !hc[n].has_other_than(0) {
                    continue;
                }
                if optional[..k].iter().any(|&p| !hc[p].contains(0)) {
                    break;
                }
                let mut h = hc.clone();
                for &p in &optional[..k] {
                    h[p].assign_singleton(0);
                }
                h[n].erase(0);
                out.hcs.push(h);
            }
        }
        out
    }

    /// Finds a mate in one by a checking travel the normal ordering reaches late or never
    /// (branching, or touching optional boards). A check with no evasion within
    /// `TRAVEL_MATE_STEPS` hypercuboids is a mate; travels tried are pushed to `checks`.
    #[allow(clippy::too_many_arguments)]
    fn travel_mate(&mut self, s: &mut State, info: &mut HcInfo, ss: &SearchSpace, scratch: &mut OrderScratch, mut checks: Option<&mut Vec<MoveSeq>>, completions: usize, steps: Option<u32>) -> Option<MoveSeq> {
        let dim = info.dimension();
        let fba = info.first_branch_axis();
        let present = s.get_present();
        let (amin, amax) = s.get_active_range();
        let (lo, hi) = s.get_lines_range();
        let optional: Vec<usize> = (lo..=hi)
            .filter(|&l| !(l >= amin && l <= amax && s.get_timeline_end(l) == present))
            .filter_map(|l| info.line_to_axis(l))
            .collect();
        if fba >= dim && optional.is_empty() {
            return None;
        }
        let mandatory: Vec<usize> = (0..fba).filter(|n| !optional.contains(n)).collect();
        let nl = s.new_line();
        let (na, nb) = active_range_for(s.multiverse().layout(), lo.min(nl), hi.max(nl));
        let moves_back = |m: &FullMove| nl >= na && nl <= nb && (m.to.t(), present.1) <= present;
        // (departure axis, departure coordinate, arrival axis, arrival coordinate)
        let mut travels: Vec<(usize, Index, usize, Index)> = Vec::new();
        let u = info.get_universe();
        // Branch axes share their entries: the first one stands for all.
        for n in 0..dim.min(fba + 1) {
            let wanted = n >= fba || optional.contains(&n);
            for i in u[n].iter() {
                let (m, dep, fa) = if n < fba {
                    let Entry::Arriving { m, idx, fa, .. } = info.entry(n, i) else { continue };
                    (*m, *idx, *fa as usize)
                } else {
                    let Some(b) = info.branch_departure(i) else { continue };
                    b
                };
                if (wanted || optional.contains(&fa)) && (n >= fba && moves_back(&m) || mandatory.iter().all(|&a| a == fa || a == n)) {
                    travels.push((fa, dep, n, i));
                }
            }
        }
        if travels.is_empty() {
            return None;
        }
        info.materialize_branch(s);
        let info: &HcInfo = info;
        let mut det = CheckDetector::new(info, s, scratch);
        travels.retain(|&(_, _, n, i)| det.gives_check(n, i));
        det.finish(scratch);
        for &(fa, dep, n, i) in &travels {
            // The travel plus a few legal moves on the other required boards. Search every
            // hypercuboid holding it: legal completions need not lie in the first.
            let mut space = Vec::new();
            for hc in ss.hcs.iter().filter(|hc| {
                hc[fa].contains(dep) && hc[n].contains(i) && (0..dim).all(|a| a == fa || a == n || mandatory.contains(&a) || hc[a].contains(0))
            }) {
                let mut h: Hc = hc.clone();
                for a in 0..dim {
                    // (A branching travel can activate a line whose board must then be played
                    // too, so the other playable boards keep their moves; extra branches do not.)
                    if a != fa && a != n && a >= fba {
                        h[a].assign_singleton(0);
                    }
                }
                h[fa].assign_singleton(dep);
                h[n].assign_singleton(i);
                space.push(h);
            }
            if space.is_empty() {
                continue;
            }
            // Best first (checks, then captures): the helper moves of a mate are usually
            // captures or blocks, which the natural order reaches last.
            let mut probe = BestFirstCursor::default();
            self.start_actions(s, info, &mut probe, scratch, SearchSpace::from_vec(space), false);
            probe.set_budget(steps);
            for k in 0..completions {
                if interrupted() {
                    return None;
                }
                let Some(act) = probe.next(info, s) else { break };
                if k == 0
                    && let Some(c) = checks.as_deref_mut() {
                        c.push(act.clone());
                    }
                let mark = Self::make(s, &act);
                let mate = self.is_mate(s);
                s.unmake(mark);
                if mate {
                    return Some(act);
                }
            }
        }
        None
    }

    /// Finds a mate in one by a lone travel (every other board passing). Each candidate is
    /// tried exactly, since the check detector misses checks from other timelines.
    fn lone_travel_mate(&mut self, s: &mut State, info: &HcInfo) -> Option<MoveSeq> {
        let dim = info.dimension();
        let fba = info.first_branch_axis();
        let u = info.get_universe();
        let mut tried: Vec<FullMove> = Vec::new();
        // Branch axes share their entries: the first one stands for all.
        for n in 0..dim.min(fba + 1) {
            for i in u[n].iter() {
                let m = if n < fba {
                    let Entry::Arriving { m, .. } = info.entry(n, i) else { continue };
                    *m
                } else {
                    let Some((m, _, _)) = info.branch_departure(i) else { continue };
                    m
                };
                if tried.contains(&m) {
                    continue;
                }
                tried.push(m);
                let mut act = MoveSeq::new();
                act.push(m);
                if interrupted() {
                    return None;
                }
                if !Self::is_legal(s, &act) {
                    continue;
                }
                let mark = Self::make(s, &act);
                let mate = self.is_mate(s);
                s.unmake(mark);
                if mate && !interrupted() {
                    return Some(act);
                }
            }
        }
        None
    }

    /// Travels touching an optional board (left out by `without_future`) that create no
    /// timeline, each its own hypercuboid: captures appended to `first` (searched first, best
    /// victim first), quiet ones from an optional board inserted at its front (searched last).
    /// Returns how many capturing hypercuboids were appended.
    fn optional_travels(&mut self, s: &State, info: &HcInfo, ss: &SearchSpace, first: &mut SearchSpace) -> usize {
        let dim = info.dimension();
        let fba = info.first_branch_axis();
        let present = s.get_present();
        let (amin, amax) = s.get_active_range();
        let (lo, hi) = s.get_lines_range();
        let optional: Vec<usize> = (lo..=hi)
            .filter(|&l| !(l >= amin && l <= amax && s.get_timeline_end(l) == present))
            .filter_map(|l| info.line_to_axis(l))
            .collect();
        if optional.is_empty() || fba > 64 {
            return 0;
        }
        // (priority, departure axis, departure coordinate, arrival axis, arrival coordinate)
        let mut captures: Vec<(i32, usize, Index, usize, Index)> = Vec::new();
        let mut quiet: Vec<(i32, usize, Index, usize, Index)> = Vec::new();
        let u = info.get_universe();
        for n in 0..fba {
            let wanted = optional.contains(&n);
            for i in u[n].iter() {
                let Entry::Arriving { idx, fa, victim, .. } = info.entry(n, i) else { continue };
                let fa = *fa as usize;
                if *victim != NO_PIECE {
                    if wanted || optional.contains(&fa) {
                        captures.push((piece_value(*victim), fa, *idx, n, i));
                    }
                } else if !wanted && optional.contains(&fa) {
                    quiet.push((0, fa, *idx, n, i));
                }
            }
        }
        if captures.is_empty() && quiet.is_empty() {
            return 0;
        }
        // Per hypercuboid of `ss` with every branch axis able to pass: the optional axes that
        // cannot pass (bit per axis).
        let bases: Vec<(&Hc, u64)> = ss
            .hcs
            .iter()
            .filter(|hc| (fba..dim).all(|b| hc[b].contains(0)))
            .map(|hc| (hc, optional.iter().filter(|&&p| !hc[p].contains(0)).fold(0u64, |m, &p| m | 1 << p)))
            .collect();
        let lone = |&(_, fa, dep, n, i): &(i32, usize, Index, usize, Index), out: &mut Vec<Hc>| {
            for &(hc, stuck) in &bases {
                if stuck & !(1 << fa | 1 << n) != 0 || !hc[fa].contains(dep) || !hc[n].contains(i) {
                    continue;
                }
                let mut h: Hc = hc.clone();
                for &p in &optional {
                    h[p].assign_singleton(0);
                }
                for b in fba..dim {
                    h[b].assign_singleton(0);
                }
                h[fa].assign_singleton(dep);
                h[n].assign_singleton(i);
                out.push(h);
            }
        };
        let mut front: Vec<Hc> = Vec::new();
        for t in &quiet {
            lone(t, &mut front);
        }
        first.hcs.splice(0..0, front);
        // The last hypercuboid comes first.
        captures.sort_by_key(|t| t.0);
        let k = first.hcs.len();
        for t in &captures {
            lone(t, &mut first.hcs);
        }
        first.hcs.len() - k
    }

    /// Whether the action `cursor` just found is a capturing travel's (its class is one of the
    /// first `travels`, see `optional_travels`): a travel is searched in one action, so the
    /// rest of its class is dropped.
    fn travel_class(cursor: &mut BestFirstCursor, travels: usize) -> bool {
        let class = cursor.last_class();
        if class as usize >= travels {
            return false;
        }
        cursor.drop_class(class);
        true
    }

    /// Whether the side to move has a legal action within `steps` hypercuboids (searched in
    /// `MATE_CHUNK_STEPS` chunks until `deadline`), and the hypercuboids used. The action found
    /// is remembered for `replay_escape`.
    fn evasion(&mut self, s: &State, steps: u32, deadline: Option<Instant>) -> (Evasion, u32) {
        let mut nb = self.bufs.pop().unwrap_or_default();
        let r = {
            let NodeBufs { info, evasions, .. } = &mut *nb;
            evasions.ss = info.rebuild(s, false);
            evasions.forget();
            evasions.order = HcOrdering::Natural;
            evasions.branch_pending = false;
            let mut left = steps;
            let r = loop {
                let chunk = if deadline.is_some() { left.min(MATE_CHUNK_STEPS) } else { left };
                evasions.budget = chunk;
                if let Some(act) = evasions.next(info, s) {
                    self.escapes.retain(|e| *e != act);
                    self.escapes.insert(0, act);
                    self.escapes.truncate(ESCAPE_MEMO);
                    break Evasion::Yes;
                }
                if interrupted() {
                    break Evasion::Unknown;
                }
                if evasions.budget > 0 {
                    break Evasion::No;
                }
                left -= chunk;
                if left == 0 || deadline.is_some_and(|d| Instant::now() >= d) {
                    break Evasion::Unknown;
                }
            };
            evasions.budget = u32::MAX;
            (r, steps - left)
        };
        self.bufs.push(nb);
        r
    }

    /// Whether `act` is a legal action of the side to move in `s` (any legal action: wasted
    /// moves or not), played and taken back.
    fn is_legal(s: &mut State, act: &MoveSeq) -> bool {
        let player = s.get_present().1;
        let mark = s.mark();
        let mut ok = true;
        for &m in act {
            if s.get_board_ptr(m.from.l(), m.from.t(), player).is_none()
                || s.get_timeline_end(m.from.l()) != (m.from.t(), player)
                || s.get_board_ptr(m.to.l(), m.to.t(), player).is_none()
                || !s.apply_move::<false>(m)
            {
                ok = false;
                break;
            }
        }
        let ok = ok && s.submit::<false>() && !s.royal_capturable(s.get_present().1);
        s.unmake(mark);
        ok
    }

    /// One of the escapes found last (`escapes`) that is legal in `s` too (moved to the front):
    /// an exact refutation of the check, without building the escape search. The replies to
    /// the checks of one scan are mostly the same few actions.
    fn replay_escape(&mut self, s: &mut State) -> bool {
        crate::misc::prof::scope(11);
        for i in 0..self.escapes.len() {
            if Self::is_legal(s, &self.escapes[i]) {
                let e = self.escapes.remove(i);
                self.escapes.insert(0, e);
                return true;
            }
        }
        false
    }

    /// Whether the side to move is checkmated: `No` escape is a proven mate, `Yes` an escape (or
    /// no check), `Unknown` a search that ran out of `steps` or `deadline`. Uses `mate_memo`.
    fn mate_status(&mut self, s: &mut State, steps: u32, deadline: Option<Instant>) -> Evasion {
        if !self.in_check(s) {
            return Evasion::Yes;
        }
        let key = s.hash();
        if let Some(e) = self.mate_memo.get(key, steps) {
            return e;
        }
        let (e, spent) = if self.replay_escape(s) { (Evasion::Yes, 0) } else { self.evasion(s, steps, deadline) };
        // An interrupted search proves nothing about a larger budget.
        if e != Evasion::Unknown || !interrupted() {
            self.mate_memo.put(key, e, spent);
        }
        e
    }

    /// Whether the side to move is checkmated, searching `TRAVEL_MATE_STEPS` (or `mate_recheck`
    /// if larger) hypercuboids for an escape. A search that runs out counts as an escape.
    fn is_mate(&mut self, s: &mut State) -> bool {
        self.mate_status(s, TRAVEL_MATE_STEPS.max(self.mate_recheck), None) == Evasion::No
    }

    /// Finds a mate in one by a checking travel back from an optional board (which
    /// `without_future` leaves out), trying the first `ROOT_FUTURE_MATE_PROBE` actions of each
    /// within `ROOT_PROBE_STEPS` hypercuboids.
    fn future_mate_in_one(&mut self, s: &mut State, info: &HcInfo, ss: &SearchSpace, scratch: &mut OrderScratch) -> Option<MoveSeq> {
        let present = s.get_present();
        let (amin, amax) = s.get_active_range();
        let (lo, hi) = s.get_lines_range();
        let optional: Vec<usize> = (lo..=hi)
            .filter(|&l| !(l >= amin && l <= amax && s.get_timeline_end(l) == present))
            .filter_map(|l| info.line_to_axis(l))
            .collect();
        if optional.is_empty() {
            return None;
        }
        // Checking travels from optional boards: departure (axis, coordinate) and, unless the
        // travel branches off a new timeline, arrival (axis, coordinate).
        let fba = info.first_branch_axis();
        let mut travels = Vec::new();
        let mut det = CheckDetector::new(info, s, scratch);
        let u = info.get_universe();
        // Branch axes share their entries: the first one stands for all.
        for n in 0..info.dimension().min(fba + 1) {
            for i in u[n].iter() {
                if let Entry::Arriving { idx, fa, .. } = info.entry(n, i) {
                    let t = (*fa as usize, *idx, if n < fba { Some((n, i)) } else { None });
                    if optional.contains(&t.0) && !travels.contains(&t) && det.gives_check(n, i) {
                        travels.push(t);
                    }
                }
            }
        }
        det.finish(scratch);
        let mut cursor = BestFirstCursor::default();
        for (k, &(fa, idx, arrival)) in travels.iter().enumerate() {
            let mut space = SearchSpace::default();
            for hc in &ss.hcs {
                if !hc[fa].contains(idx)
                    || optional.iter().any(|&p| p != fa && !hc[p].contains(0))
                    || arrival.is_some_and(|(n, i)| !hc[n].contains(i))
                {
                    continue;
                }
                let mut h = hc.clone();
                for &p in &optional {
                    h[p].assign_singleton(if p == fa { idx } else { 0 });
                }
                if let Some((n, i)) = arrival {
                    h[n].assign_singleton(i);
                }
                space.hcs.push(h);
            }
            if k == 0 {
                self.start_actions(s, info, &mut cursor, scratch, space, false);
            } else {
                cursor.restart(info, space);
            }
            cursor.set_budget(Some(ROOT_PROBE_STEPS));
            for _ in 0..ROOT_FUTURE_MATE_PROBE {
                let act = cursor.next(info, s);
                // An interrupted enumeration finds no action, mate or not.
                if interrupted() {
                    return None;
                }
                let Some(act) = act else { break };
                let mark = Self::make(s, &act);
                let mate = self.is_mate(s);
                s.unmake(mark);
                if mate && !interrupted() {
                    return Some(act);
                }
            }
        }
        None
    }

    /// The sub-space of `ss` whose actions contain a checking entry or any travel, where most
    /// mates in one are. All travels are included since the detector misses cross-timeline checks.
    fn checking_space(&mut self, s: &State, info: &HcInfo, ss: &SearchSpace, scratch: &mut OrderScratch) -> SearchSpace {
        let dim = info.dimension();
        let fba = info.first_branch_axis();
        let mut checking: Vec<(usize, Index)> = Vec::new();
        {
            let mut det = CheckDetector::new(info, s, scratch);
            let u = info.get_universe();
            for n in 0..dim.min(fba + 1) {
                for i in u[n].iter() {
                    if n >= fba || matches!(info.entry(n, i), Entry::Arriving { .. }) || det.gives_check(n, i) {
                        checking.push((n, i));
                    }
                }
            }
            det.finish(scratch);
        }
        let mut space = SearchSpace::default();
        for &(n, i) in &checking {
            for hc in &ss.hcs {
                if n < fba {
                    if hc[n].contains(i) {
                        let mut h = hc.clone();
                        h[n].assign_singleton(i);
                        space.hcs.push(h);
                    }
                } else {
                    for b in fba..dim {
                        if hc[b].contains(i) {
                            let mut h = hc.clone();
                            h[b].assign_singleton(i);
                            space.hcs.push(h);
                        }
                    }
                }
            }
        }
        space
    }

    /// A checkmate in one among the legal actions of `s`: first the actions containing a
    /// precomputed checking entry or a travel (most mates, found at once), then every action,
    /// as far as `budget` goes.
    fn root_mate_scan(&mut self, s: &mut State, info: &mut HcInfo, ss: &SearchSpace, scratch: &mut OrderScratch, budget: &MateBudget) -> ScanEnd {
        info.materialize_branch(s);
        let info: &HcInfo = info;
        let space = self.checking_space(s, info, ss, scratch);
        // The actions the quick scan tried are not tried again, its unresolved checks are.
        let mut memo = ScanMemo::default();
        if !space.hcs.is_empty() {
            let quick = MateBudget { free: ROOT_CHECKING_SCAN_ACTIONS, actions: ROOT_CHECKING_SCAN_ACTIONS, steps: ROOT_CHECKING_SCAN_STEPS, deadline: None };
            if let found @ ScanEnd::Found(_) = self.scan_for_mate(s, info, scratch, space, &quick, &mut memo) {
                return found;
            }
            if interrupted() {
                return ScanEnd::OutOfBudget { tried: memo.seen.len(), unresolved: memo.pending.len() };
            }
        }
        // Then every legal action: the checking entries are not a sure necessary condition (a mate
        // can come from a discovered check, or from one the detector does not flag because it
        // only exists with the other boards' moves played).
        self.scan_for_mate(s, info, scratch, SearchSpace::from_vec(ss.hcs.clone()), budget, &mut memo)
    }

    /// Finds the first mating action of `space` within `budget`, alternating a best-first and a
    /// natural-order enumeration and skipping positions in `memo`. An exhausted space proves no
    /// mate, once the unresolved checks in `memo.pending` are resolved.
    fn scan_for_mate(&mut self, s: &mut State, info: &HcInfo, scratch: &mut OrderScratch, space: SearchSpace, budget: &MateBudget, memo: &mut ScanMemo) -> ScanEnd {
        let mut natural = HcCursor::new(SearchSpace::from_vec(space.hcs.clone()), HcOrdering::Natural);
        natural.budget = budget.steps;
        let mut best_first = BestFirstCursor::default();
        self.start_actions(s, info, &mut best_first, scratch, space, false);
        best_first.set_budget(Some(budget.steps));
        let (mut natural_over, mut best_over) = (false, false);
        let (mut turn, mut tried) = (0usize, 0usize);
        let recheck = TRAVEL_MATE_STEPS.max(self.mate_recheck);
        let out = |tried: usize, memo: &ScanMemo| ScanEnd::OutOfBudget { tried, unresolved: memo.pending.len() };
        while !(natural_over && best_over) {
            // Past the free actions the clock is looked at before each one (and inside its
            // escape search).
            let deadline = budget.deadline.filter(|_| tried >= budget.free);
            if tried >= budget.actions || deadline.is_some_and(|d| Instant::now() >= d) {
                return out(tried, memo);
            }
            turn += 1;
            let use_natural = (turn % 2 == 0 && !natural_over) || best_over;
            let act = if use_natural { natural.next(info, s) } else { best_first.next(info, s) };
            if interrupted() {
                return out(tried, memo);
            }
            let Some(act) = act else {
                // No action left in that enumeration: its space is done, or its hypercuboids are.
                if use_natural {
                    if natural.budget > 0 {
                        return self.resolve_pending(s, budget, tried, memo);
                    }
                    natural_over = true;
                } else {
                    if !best_first.budget_spent() {
                        return self.resolve_pending(s, budget, tried, memo);
                    }
                    best_over = true;
                }
                continue;
            };
            let mark = Self::make(s, &act);
            if !memo.seen.insert(s.hash()) {
                s.unmake(mark);
                continue;
            }
            tried += 1;
            let status = self.mate_status(s, recheck, deadline);
            // What makes the action no mate, to rule out the region around it: no check, or a
            // legal escape (the one just found or replayed, when it still is).
            let refuted = match status {
                Evasion::Yes if !self.in_check(s) => Some(None),
                Evasion::Yes => self.escapes.iter().position(|e| Self::is_legal(s, e)).map(|i| Some(self.escapes[i].clone())),
                _ => None,
            };
            s.unmake(mark);
            // Past the free actions only: the free part is bounded by counts, and the time the
            // region takes is paid back by the actions it skips only within a time share.
            if let Some(escape) = refuted.filter(|_| deadline.is_some()) {
                let pt = if use_natural { natural.last_point() } else { best_first.last_point() }.clone();
                if let Some(region) = self.no_mate_region(s, info, &pt, &act, escape.as_ref()) {
                    natural.exclude(region.clone());
                    best_first.exclude(region);
                }
            }
            match status {
                Evasion::No if !interrupted() => return ScanEnd::Found(act),
                Evasion::Unknown if !interrupted() => memo.pending.push(act),
                _ => {}
            }
        }
        out(tried, memo)
    }

    /// A region around non-mating root action `act` (no check, or `escape` legal) with no mate.
    /// It is the product of per-axis physical moves that keep that fact, over axes whose changed
    /// squares cannot share a ray from a royal or escape source (so they cannot interact).
    fn no_mate_region(&mut self, s: &mut State, info: &HcInfo, pt: &Point, act: &MoveSeq, escape: Option<&MoveSeq>) -> Option<Slice> {
        let fba = info.first_branch_axis();
        if fba > 64 {
            return None;
        }
        // Worth the cost (a few dozen positions played) only when the region can be large:
        // at least two axes playing a physical move with a few alternatives each.
        let u = info.get_universe();
        let wide = (0..fba)
            .filter(|&y| {
                matches!(info.entry(y, pt[y]), Entry::Physical { .. })
                    && u[y].iter().filter(|&e| matches!(info.entry(y, e), Entry::Physical { .. })).count() >= REGION_MIN_MOVES
            })
            .count();
        if wide < 2 {
            return None;
        }
        // A castling escape's legality also depends on the squares its king passes (origins this
        // does not track): an escape with a move along a rank of two or more squares is left
        // alone.
        if escape.is_some_and(|d| d.iter().any(|m| m.from.tl() == m.to.tl() && m.from.y() == m.to.y() && (m.to.x() - m.from.x()).abs() > 1)) {
            return None;
        }
        let player = s.get_present().1;
        // (axis, the moves that keep the fact, squares where they change the board, line,
        // half-turn of the board the move adds)
        let mut cands: Vec<(usize, IntegerSet, Bitboard, i32, i32)> = Vec::new();
        for y in 0..fba {
            let Entry::Physical { m: ma, b: ba, .. } = info.entry(y, pt[y]) else { continue };
            let (ma, ba) = (*ma, *ba);
            // The action with this move replaced in place: a later move may travel onto the
            // board it leaves (a branch only once it is played).
            let Some(at) = act.iter().position(|&m| m == ma) else { continue };
            let mark = s.mark();
            for &m in &act[..at] {
                s.apply_move::<true>(m);
            }
            let others = s.mark();
            let mut set = IntegerSet::singleton(pt[y]);
            let mut diff: Bitboard = 0;
            for e in u[y].iter() {
                let Entry::Physical { m, b, victim, .. } = info.entry(y, e) else { continue };
                if e == pt[y] || *victim == KING_W || *victim == ROYAL_QUEEN_W {
                    continue;
                }
                let (m, b) = (*m, *b);
                s.apply_move::<true>(m);
                for &m in &act[at + 1..] {
                    s.apply_move::<true>(m);
                }
                s.submit::<true>();
                let holds = match escape {
                    None => !s.has_phantom_check(),
                    Some(d) => Self::is_legal(s, d),
                };
                s.unmake(others);
                if holds {
                    set.insert(e);
                    diff |= board_diff(&b, &ba);
                }
            }
            s.unmake(mark);
            if set.len() > 1 {
                cands.push((y, set, diff, ma.from.l(), 2 * ma.from.t() + player as i32 + 1));
            }
        }
        if cands.is_empty() {
            return None;
        }
        // Two candidates interact when a changed square of each lies on one ray from an origin
        // (a royal, or an escape move's source). Changed squares lie on half-turns v and v + 1;
        // squares on different lines share a ray only when every nonzero offset equals k.
        let mark = s.mark();
        for &m in act {
            s.apply_move::<true>(m);
        }
        s.submit::<true>();
        if let Some(d) = escape {
            for &m in d {
                s.apply_move::<true>(m);
            }
            s.submit::<true>();
        }
        let defender = !player;
        let (size_x, size_y) = s.get_board_size();
        let (lo, hi) = s.get_lines_range();
        let sources: Vec<(i32, i32, i32, i32)> = escape
            .map(|d| d.iter().map(|m| (m.from.x(), m.from.y(), m.from.l(), 2 * m.from.t() + defender as i32)).collect())
            .unwrap_or_default();
        // Whether the board at (l, w) exists (or is the phantom copy of a line's last board),
        // and whether an origin stands on (x, y) of it.
        let origin_at = |s: &State, x: i32, y: i32, l: i32, w: i32| -> Option<bool> {
            if l < lo || l > hi {
                return None;
            }
            let b = s.get_board_ptr(l, w >> 1, w & 1 != 0).or_else(|| {
                let p = w - 1;
                let end = s.get_timeline_end(l);
                (2 * end.0 + end.1 as i32 == p).then(|| s.get_board_ptr(l, p >> 1, p & 1 != 0)).flatten()
            })?;
            let royal = b.royal() & if defender { b.black() } else { b.white() } & !b.wall() & pmask(y * 8 + x) != 0;
            Some(royal || sources.contains(&(x, y, l, w)))
        };
        // An origin on the line through (x, y, l, w) with step (sx, sy, sl, st) (st in whole
        // turns), past the point (walking `dir` = -1 from it backwards, or +1 forwards).
        let origin_on = |s: &State, (x, y, l, w): (i32, i32, i32, i32), (sx, sy, sl, st): (i32, i32, i32, i32), dir: i32| -> bool {
            let (mut x, mut y, mut l, mut w) = (x, y, l, w);
            loop {
                x += dir * sx;
                y += dir * sy;
                l += dir * sl;
                w += dir * 2 * st;
                if x < 0 || y < 0 || x >= size_x || y >= size_y {
                    return false;
                }
                match origin_at(s, x, y, l, w) {
                    None => return false,
                    Some(true) => return true,
                    Some(false) => {}
                }
            }
        };
        let mut conflicts = vec![0u64; cands.len()];
        for i in 0..cands.len() {
            for j in i + 1..cands.len() {
                let (ci, cj) = (&cands[i], &cands[j]);
                let k = (cj.3 - ci.3).abs();
                let sl = (cj.3 - ci.3).signum();
                let mut hit = false;
                'pairs: for wi in [ci.4, ci.4 + 1] {
                    for wj in [cj.4, cj.4 + 1] {
                        if (wj - wi) % 2 != 0 {
                            continue;
                        }
                        let dt = (wj - wi) / 2;
                        if dt != 0 && dt.abs() != k {
                            continue;
                        }
                        let st = dt.signum();
                        for p in marked_pos(ci.2) {
                            let (x1, y1) = (p & 7, p >> 3);
                            for sx in -1..=1 {
                                for sy in -1..=1 {
                                    let (x2, y2) = (x1 + sx * k, y1 + sy * k);
                                    if x2 < 0 || y2 < 0 || x2 >= 8 || y2 >= 8 || cj.2 & pmask(y2 * 8 + x2) == 0 {
                                        continue;
                                    }
                                    let step = (sx, sy, sl, st);
                                    if origin_on(s, (x1, y1, ci.3, wi), step, -1) || origin_on(s, (x2, y2, cj.3, wj), step, 1) {
                                        hit = true;
                                        break 'pairs;
                                    }
                                }
                            }
                        }
                    }
                }
                if hit {
                    conflicts[i] |= 1 << j;
                    conflicts[j] |= 1 << i;
                }
            }
        }
        s.unmake(mark);
        let mut order: Vec<usize> = (0..cands.len()).collect();
        order.sort_by_key(|&j| std::cmp::Reverse(cands[j].1.len()));
        let mut chosen = 0u64;
        for j in order {
            if conflicts[j] & chosen == 0 {
                chosen |= 1 << j;
            }
        }
        if chosen == 0 {
            return None;
        }
        let mut region = Slice::new();
        for (n, &c) in pt.iter().enumerate() {
            region.fix_axis(n, IntegerSet::singleton(c));
        }
        for (j, c) in cands.into_iter().enumerate() {
            if chosen >> j & 1 != 0 {
                region.fix_axis(c.0, c.1);
            }
        }
        if REGION_VERIFY.load(Ordering::Relaxed) {
            Self::verify_region(s, info, &region, escape);
        }
        Some(region)
    }

    /// Diagnostics: the fact of `region` (see `no_mate_region`) tested on its legal actions.
    fn verify_region(s: &mut State, info: &HcInfo, region: &Slice, escape: Option<&MoveSeq>) {
        let hc = Hc::new(region.fixed.iter().map(|(_, c)| c.clone()).collect());
        let mut cursor = HcCursor::new(SearchSpace::from_vec(vec![hc]), HcOrdering::Natural);
        REGION_STATS[0].fetch_add(1, Ordering::Relaxed);
        for _ in 0..REGION_VERIFY_POINTS {
            let Some(a) = cursor.next(info, s) else { break };
            let mark = Self::make(s, &a);
            let holds = match escape {
                None => !s.has_phantom_check(),
                Some(d) => Self::is_legal(s, d),
            };
            s.unmake(mark);
            REGION_STATS[1].fetch_add(1, Ordering::Relaxed);
            if !holds {
                REGION_STATS[2].fetch_add(1, Ordering::Relaxed);
                eprintln!("region broken: {} ({})", Action::from_moveseq(&a, s), if escape.is_some() { "escape" } else { "no check" });
            }
        }
    }

    /// Re-searches the unresolved checks in `memo.pending`, doubling the budget each round until
    /// the deadline (or `budget.steps`). With none left unresolved, the space has no mate.
    fn resolve_pending(&mut self, s: &mut State, budget: &MateBudget, tried: usize, memo: &mut ScanMemo) -> ScanEnd {
        let mut steps = TRAVEL_MATE_STEPS.max(self.mate_recheck);
        let deadline = budget.deadline.filter(|_| tried >= budget.free);
        while !memo.pending.is_empty() {
            if steps >= budget.steps || interrupted() || deadline.is_some_and(|d| Instant::now() >= d) {
                break;
            }
            steps = steps.saturating_mul(2).min(budget.steps);
            let mut i = 0;
            while i < memo.pending.len() {
                let act = memo.pending[i].clone();
                let mark = Self::make(s, &act);
                let status = self.mate_status(s, steps, deadline);
                s.unmake(mark);
                if interrupted() {
                    return ScanEnd::OutOfBudget { tried, unresolved: memo.pending.len() };
                }
                match status {
                    Evasion::No => return ScanEnd::Found(act),
                    Evasion::Yes => {
                        memo.pending.swap_remove(i);
                    }
                    Evasion::Unknown => i += 1,
                }
                if deadline.is_some_and(|d| Instant::now() >= d) {
                    break;
                }
            }
        }
        if memo.pending.is_empty() {
            ScanEnd::Exhausted
        } else {
            ScanEnd::OutOfBudget { tried, unresolved: memo.pending.len() }
        }
    }

    /// What `MateVerify` allows the search for a checkmate in one at the root, given the move's
    /// time plan (none: an analysis without a limit). Returns the scan's budget and the steps of
    /// the second look at an evasion search that ran out (`MATE_RECHECK_STEPS`).
    fn root_mate_budget(&self, plan: Option<&TimePlan>) -> (MateBudget, u32) {
        let clock = plan.is_some_and(|p| p.clock);
        let now = Instant::now();
        let mut b = MateBudget { free: ROOT_SCAN_ACTIONS, actions: ROOT_SCAN_MORE_ACTIONS, steps: ROOT_SCAN_MORE_STEPS, deadline: None };
        let mut recheck = MATE_RECHECK_STEPS;
        // A game never spends more than a quarter of the move on it, whatever is asked.
        let game_cap = plan.map(|p| Duration::from_millis(p.optimum / 4));
        let ms = |n: u64| Duration::from_millis(n);
        match self.mate_verify {
            MateVerify::Auto if clock => {
                let p = plan.expect("clock plan");
                b.deadline = Some(now + ms((p.optimum / ROOT_SCAN_MORE_DIV).min(ROOT_SCAN_MORE_CAP_MS)));
            }
            MateVerify::Auto => {
                b.actions = 1 << 22;
                b.steps = 1 << 24;
                recheck = 1 << 16;
                b.deadline = Some(now + ms(ROOT_SCAN_ANALYSIS_MS));
            }
            MateVerify::Millis(n) => {
                b.actions = 1 << 22;
                b.steps = 1 << 24;
                b.deadline = Some(now + game_cap.filter(|_| clock).map_or(ms(n), |c| ms(n).min(c)));
                if !clock {
                    recheck = 1 << 16;
                }
            }
            MateVerify::Exhaustive => {
                b.actions = usize::MAX;
                b.steps = u32::MAX;
                recheck = u32::MAX;
                b.deadline = game_cap.filter(|_| clock).map(|c| now + c);
            }
        }
        (b, recheck)
    }

    /// Root actions with a move on a board the side to move need not play (the ones
    /// `without_future` leaves out) that `roots` lacks: at most `ROOT_WIDEN_CAP`, in the
    /// cursor's order (checks, then captures), travels first.
    fn root_widen(&self, root: &State, roots: &[(i32, MoveSeq)]) -> Vec<(i32, MoveSeq)> {
        let (hc, ss) = HcInfo::build_hc(root);
        if !Self::trims_future(root, &hc, &ss) {
            return Vec::new();
        }
        let mut cursor = BestFirstCursor::default();
        self.start_actions(root, &hc, &mut cursor, &mut OrderScratch::default(), Self::future_only(root, &hc, &ss), false);
        let mut out: Vec<(i32, MoveSeq)> = Vec::new();
        while out.len() < ROOT_WIDEN_CAP {
            let Some(a) = cursor.next(&hc, root) else { break };
            if !roots.iter().any(|r| r.1 == a) {
                out.push((self.action_score(root, &a, 0), a));
            }
        }
        out.sort_by_key(|(_, a)| !a.iter().any(|m| m.from.tl() != m.to.tl()));
        out
    }

    /// Order the node's coordinates into `cursor` and start it on `ss`.
    /// Order the node's coordinates into `cursor` and start it on `ss`; with `defer`, the
    /// branch axes are ordered once the cursor needs them (`next_with` and `branch_order`).
    fn start_actions(&self, s: &State, info: &HcInfo, cursor: &mut BestFirstCursor, scratch: &mut OrderScratch, ss: SearchSpace, defer: bool) {
        let (orders, scores) = cursor.ordering_mut();
        let branch = if defer { BranchOrder::Defer } else { BranchOrder::Now };
        // Interior nodes skip the axes no hypercuboid varies (see `OrderScratch::skipped`).
        let fba = info.first_branch_axis();
        scratch.skipped.clear();
        if defer {
            scratch.skipped.extend((0..fba).map(|n| ss.hcs.iter().all(|hc| hc[n].single().is_some() || hc[n].is_empty())));
        }
        scratch.skip = true;
        self.node_ordering(s, info, true, scratch, orders, scores, 0..usize::MAX, branch);
        scratch.skip = false;
        cursor.forget();
        cursor.set_branch_pending(defer && info.first_branch_axis() < info.dimension());
        cursor.restart(info, ss);
    }

    /// Score the axes `start_actions` skipped, before the cursor is restarted on a space that
    /// may vary them.
    fn order_skipped(&self, s: &State, info: &HcInfo, scratch: &mut OrderScratch, cursor: &mut BestFirstCursor) {
        let skipped = std::mem::take(&mut scratch.skipped);
        let (orders, scores) = cursor.ordering_mut();
        for (n, _) in skipped.iter().enumerate().filter(|x| *x.1) {
            self.node_ordering(s, info, true, scratch, orders, scores, n..n + 1, BranchOrder::Now);
        }
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
        // ORDER_NO_FUTURE: without moves on optional boards, as interior nodes enumerate.
        let ss = if std::env::var_os("ORDER_NO_FUTURE").is_some() { Self::without_future(s, &info, &ss) } else { ss };
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
                || ((pc == PAWN_W || pc == BRAWN_W) && m.to.y() == promotion_rank(h, s.get_present().1) && m.from.tl() == m.to.tl())
        })
    }

    /// Play an action generated as legal for `s`; undo with `s.unmake(mark)`.
    fn make(s: &mut State, act: &MoveSeq) -> Mark {
        crate::misc::prof::scope(7);
        let mark = s.mark();
        for &m in act {
            s.apply_move::<true>(m);
        }
        s.submit::<true>();
        mark
    }

    /// Play a remembered action (TT/killer) only if it is legal in `s`.
    fn make_checked(s: &mut State, act: &MoveSeq) -> Option<Mark> {
        crate::misc::prof::scope(11);
        let player = s.get_present().1;
        let mark = s.mark();
        let mut ok = true;
        // Bit i: move i created a timeline (moves past the 64th count as creating one).
        let mut branching = 0u64;
        for (i, &m) in act.iter().enumerate() {
            if s.get_board_ptr(m.from.l(), m.from.t(), player).is_none()
                || s.get_timeline_end(m.from.l()) != (m.from.t(), player)
                || s.get_board_ptr(m.to.l(), m.to.t(), player).is_none()
            {
                ok = false;
                break;
            }
            if i < 64 && s.get_timeline_end(m.to.l()) != (m.to.t(), player) {
                branching |= 1 << i;
            }
            if !s.apply_move::<false>(m) {
                ok = false;
                break;
            }
        }
        // A remembered action that wastes a move here is not one the search would generate.
        let wasted = |s: &State| branching != 0 && plays_wasted_move(s, act, |i| i >= 64 || branching >> i & 1 != 0);
        if ok && !wasted(s) && s.submit::<false>() && !s.royal_capturable(s.get_present().1) {
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

    /// Null move is allowed only when some active board has two non-pawn, non-royal pieces of
    /// the side to move (counted per board); otherwise zugzwang is too common.
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
    /// Evasions that raise alpha inside the window extend the PV table (mates end in them);
    /// in a null window none can, so only PV windows pay for it.
    fn qsearch(&mut self, s: &mut State, ply: usize, mut alpha: i32, beta: i32, qd: i32, known_check: Option<bool>) -> i32 {
        self.nodes += 1;
        self.pv_clear(ply);
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
        if let Some(k) = qkiller.take()
            && let Some(mark) = Self::make_checked(s, &k) {
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
                    if let Some(k) = &qkiller {
                        self.pv_update(ply, k);
                    }
                }
            }
        // Search the evasions.
        let mut nb = self.bufs.pop().unwrap_or_default();
        {
            let NodeBufs { info, evasions, order, evasion_scores, .. } = &mut *nb;
            evasions.ss = info.rebuild(s, true);
            evasions.forget();
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
                    if interrupted() {
                        // Out of time, not out of evasions.
                        self.aborted = true;
                        break 'search true;
                    }
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
                        self.pv_update(ply, &act);
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
    #[allow(clippy::too_many_arguments)]
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
        self.pv_clear(ply);
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
        let key = s.live_hash();
        let mut tt_action: Option<MoveSeq> = None;
        // Ok((score, proven)): a cutoff; Err(action): the remembered action, if any. (Read
        // under the slot's lock, so the score, its proof and the action are one entry's.)
        let excl_node = self.excluded.is_some() && key == self.excl_key;
        let probe = self.tt.probe(key, |e| {
            let sc = mate_from_tt(e.score, ply);
            if !pv && e.depth as i32 >= depth && !(excl_node) {
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
        // Internal iterative reduction: with no remembered action the node is searched a ply
        // shallower, which also leaves one for the next iteration to find.
        let depth = if tt_action.is_none() && depth >= IIR_DEPTH && !known_check.unwrap_or_else(|| self.in_check(s)) { depth - 1 } else { depth };

        if !pv && beta.abs() < MATE - 1000 && (depth <= RFP_DEPTH || depth >= NMP_MIN_DEPTH) && !known_check.unwrap_or_else(|| self.in_check(s)) {
            if depth <= RFP_DEPTH {
                let e = self.static_eval(s);
                if e - RFP_MARGIN * depth >= beta {
                    return e;
                }
            }
            if depth >= NMP_MIN_DEPTH && Self::has_null_material(s) && self.static_eval(s) >= beta {
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
        let mut cap = {
            let base = LMP[(depth as usize).min(LMP.len() - 1)];
            if pv { base * PV_CAP_MULT } else { base }
        };
        // When every action searched loses to a mate, the cap is raised once by its own size:
        // a mated score made of the actions the cap let in claims mates the side to move
        // escapes (see `mated_cap`).
        let mut cap_raised = false;
        // Checking actions (ordered first) are never cut by the cap: mating lines run through them.
        let mut checks_left = true;

        // Remembered actions first, before paying for the action space: the TT action,
        // then the killers.
        let mut pre_tried: Vec<MoveSeq> = Vec::new();
        let mut tt_ext = 0;
        if depth >= SING_DEPTH && self.excluded.is_none() && ply > 0
            && let Some(ta) = tt_action.clone() {
                let info = self.tt.probe(key, |e| (mate_from_tt(e.score, ply), e.depth as i32, e.bound));
                if let Some((sc, ed, bound)) = info
                    && ed >= depth - 3 && matches!(bound, Bound::Lower | Bound::Exact) && sc.abs() < MATE - 1000 {
                        let sb = sc - SING_MARGIN * depth;
                        self.excluded = Some(ta);
                        self.excl_key = key;
                        let v = self.negamax(s, (depth - 1) / 2, ply, sb - 1, sb, false);
                        self.excluded = None;
                        self.excl_key = 0;
                        if self.aborted {
                            return 0;
                        }
                        if v < sb {
                            tt_ext = 1;
                        }
                    }
            }
        let mut remembered: Vec<MoveSeq> = tt_action.iter().filter(|a| self.excluded.as_ref() != Some(*a)).cloned().collect();
        if let Some(x) = &self.excluded {
            pre_tried.push(x.clone());
        }
        if ply <= MAX_PLY {
            remembered.extend(self.killers[ply].iter().flatten().filter(|k| tt_action.as_ref() != Some(*k)).cloned());
        }
        for (k, act) in remembered.into_iter().enumerate() {
            let Some(mark) = Self::make_checked(s, &act) else { continue };
            let child_depth = depth - super::skill::travel_reduction(self.skill_level, &act);
            // The TT action keeps the node's PV status; killers after it are null-window.
            let v = if k == 0 && tt_action.is_some() && self.excluded.as_ref() != tt_action.as_ref() {
                let v = -self.negamax(s, child_depth - 1 + tt_ext, ply + 1, -beta, -alpha, pv);
                s.unmake(mark);
                v
            } else {
                self.search_child(s, mark, searched == 0, child_depth, ply, alpha, beta, pv)
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
                    if pv {
                        self.pv_update(ply, &act);
                    }
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
        // At ply 1, search travel_mate's checking travels right after the remembered actions;
        // otherwise they fall past the action cap and their mates go unseen.
        let mut tchecks: Vec<MoveSeq> = Vec::new();
        if ply <= TRAVEL_MATE_PLY {
            let mate = {
                let NodeBufs { info, order, .. } = &mut *nb;
                self.travel_mate(s, info, &ss, order, if ply == 1 { Some(&mut tchecks) } else { None }, TRAVEL_MATE_COMPLETIONS, None)
            };
            if let Some(act) = mate {
                self.bufs.push(nb);
                let v = MATE - (ply as i32 + 1);
                self.pv_clear(ply + 1);
                if pv {
                    self.pv_update(ply, &act);
                }
                self.tt_store(key, depth, Bound::Exact, mate_to_tt(v, ply), true, &act);
                self.proven = true;
                return v;
            }
        }
        for act in tchecks {
            if pre_tried.contains(&act) {
                continue;
            }
            let mark = Self::make(s, &act);
            let child_depth = depth - super::skill::travel_reduction(self.skill_level, &act);
            let v = self.search_child(s, mark, searched == 0, child_depth, ply, alpha, beta, pv);
            if self.aborted {
                self.bufs.push(nb);
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
                    if pv {
                        self.pv_update(ply, &act);
                    }
                }
            }
            if alpha >= beta {
                self.bufs.push(nb);
                self.on_cutoff(s, &act, depth, ply);
                let proven = mate_proven(best, best_proven, false);
                self.tt_store(key, depth, Bound::Lower, mate_to_tt(best, ply), proven, &best_action);
                self.proven = proven;
                return best;
            }
            pre_tried.push(act);
        }
        let mut first = Self::without_future(s, &nb.info, &ss);
        // Capturing travels first: the first classes of the cursor (see `travel_class`).
        let travels = self.optional_travels(s, &nb.info, &ss, &mut first);
        let mut full = Some(ss);
        {
            let NodeBufs { info, cursor, order, .. } = &mut *nb;
            self.start_actions(s, info, cursor, order, first, true);
            cursor.limit_classes(travels as i32, TRAVEL_CAPTURE_STEPS);
        }
        let mut generated = 0usize;
        let mut exhausted = false;
        // An action was left unsearched (the action cap, or a TT action that failed validation).
        let mut skipped = false;
        let mut batch_size = 1;
        let outcome: Option<i32> = 'search: {
            while !exhausted {
                if searched >= cap && !checks_left && !Self::mated_cap(&mut cap, &mut cap_raised, best) {
                    break;
                }
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
                        // Out of time before the actions ran out: abort, never "no actions".
                        Ok(None) if interrupted() => {
                            self.aborted = true;
                            break 'search Some(0);
                        }
                        Ok(None) => {
                            // Every legal action may need a move on an optional board. (The
                            // ordering is unchanged: nothing was searched since it was made.)
                            if generated == 0
                                && let Some(f) = full.take() {
                                    {
                                        let NodeBufs { info, cursor, order, .. } = &mut *nb;
                                        self.order_skipped(s, info, order, cursor);
                                    }
                                    nb.cursor.restart(&nb.info, f);
                                    continue;
                                }
                            exhausted = true;
                            break;
                        }
                        Ok(Some(act)) => {
                            generated += 1;
                            let travel = full.is_some() && Self::travel_class(&mut nb.cursor, travels);
                            if pre_tried.contains(&act) {
                                continue;
                            }
                            if travel {
                                // (One creating a timeline comes again among those actions.)
                                pre_tried.push(act.clone());
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
                    checks_left = sc >= CHECK_BONUS;
                    if searched >= cap && !checks_left && !Self::mated_cap(&mut cap, &mut cap_raised, best) {
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
                    let child_depth = depth - super::skill::travel_reduction(self.skill_level, &act);
                    self.known_check = child_check;
                    if searched == 0 {
                        v = -self.negamax(s, child_depth - 1, ply + 1, -beta, -alpha, pv);
                    } else {
                        v = -self.negamax(s, child_depth - 1 - r, ply + 1, -alpha - 1, -alpha, false);
                        if !self.aborted && v > alpha && r > 0 {
                            self.known_check = child_check;
                            v = -self.negamax(s, child_depth - 1, ply + 1, -alpha - 1, -alpha, false);
                        }
                        if !self.aborted && v > alpha && v < beta && pv {
                            self.known_check = child_check;
                            v = -self.negamax(s, child_depth - 1, ply + 1, -beta, -alpha, true);
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
                            if pv {
                                self.pv_update(ply, &act);
                            }
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
        // Every action searched loses to a mate: the ones `without_future` left out (moves on
        // boards the side to move need not play, travels back from them among them) may
        // hold, so they are searched too, travels first, until one escapes the mate.
        let mut outcome = outcome;
        let mut widen_complete = false;
        if outcome.is_none() && best < -MATE + 1000 && full.as_ref().is_some_and(|f| Self::trims_future(s, &nb.info, f)) {
            let space = Self::future_only(s, &nb.info, full.as_ref().expect("checked"));
            {
                let NodeBufs { info, cursor, order, .. } = &mut *nb;
                self.order_skipped(s, info, order, cursor);
            }
            nb.cursor.restart(&nb.info, space);
            let mut widened = 0usize;
            outcome = 'widen: loop {
                // A batch in the cursor's order (checks, then captures), travels first.
                let mut batch: Vec<MoveSeq> = Vec::with_capacity(BATCH);
                let mut done = false;
                while batch.len() < BATCH {
                    if self.check_abort() {
                        break 'widen Some(0);
                    }
                    let next = {
                        let NodeBufs { info, cursor, order, .. } = &mut *nb;
                        cursor.next_with(info, s, &mut |o, sc| self.branch_order(s, info, order, o, sc))
                    };
                    match next {
                        Err(NeedBranch) => nb.info.materialize_branch(s),
                        Ok(None) if interrupted() => {
                            self.aborted = true;
                            break 'widen Some(0);
                        }
                        Ok(None) => {
                            done = true;
                            break;
                        }
                        Ok(Some(act)) => {
                            if !pre_tried.contains(&act) {
                                batch.push(act);
                            }
                        }
                    }
                }
                batch.sort_by_key(|a| !a.iter().any(|m| m.from.tl() != m.to.tl()));
                for act in batch {
                    if widened >= cap {
                        break 'widen None;
                    }
                    let mark = Self::make(s, &act);
                    let child_depth = depth - super::skill::travel_reduction(self.skill_level, &act);
                    let v = self.search_child(s, mark, false, child_depth, ply, alpha, beta, pv);
                    if self.aborted {
                        break 'widen Some(0);
                    }
                    widened += 1;
                    searched += 1;
                    all_proven &= self.proven;
                    if v > best {
                        best = v;
                        best_proven = self.proven;
                        best_action = act.clone();
                        if v > alpha {
                            alpha = v;
                            if pv {
                                self.pv_update(ply, &act);
                            }
                        }
                    }
                    if alpha >= beta {
                        self.on_cutoff(s, &act, depth, ply);
                        let proven = mate_proven(best, best_proven, false);
                        self.tt_store(key, depth, Bound::Lower, mate_to_tt(best, ply), proven, &best_action);
                        self.proven = proven;
                        break 'widen Some(best);
                    }
                    if v > -MATE + 1000 {
                        break 'widen None;
                    }
                }
                if done {
                    widen_complete = true;
                    break None;
                }
            };
        }
        // A mated score needs every legal action searched: the cursor ran out, none was left
        // out, and every move on an optional board `without_future` dropped was searched
        // after all. (Only worked out when it matters: for a mated score.)
        let all_proven = all_proven
            && outcome.is_none()
            && best < -MATE + 1000
            && exhausted
            && !skipped
            && (widen_complete || !full.as_ref().is_some_and(|f| Self::trims_future(s, &nb.info, f)));
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
        // A fail-low with actions the action cap left unsearched bounds nothing: one of those
        // might score higher (e.g. the only mating move), and a stored "at most" would poison
        // deeper searches of this node.
        if bound != Bound::Upper || (exhausted && !skipped) {
            self.tt_store(key, depth, bound, mate_to_tt(best, ply), proven, &best_action);
        }
        self.proven = proven;
        best
    }

    /// Doubles the action cap once when the node's best score `best` is mated, returning whether
    /// it did, since the escape often lies past the cap.
    #[inline]
    fn mated_cap(cap: &mut usize, raised: &mut bool, best: i32) -> bool {
        if *raised || best >= -(MATE - 1000) {
            return false;
        }
        *raised = true;
        *cap *= 2;
        true
    }

    fn on_cutoff(&mut self, s: &State, act: &MoveSeq, depth: i32, ply: usize) {
        let tactical = Self::is_tactical(s, act);
        for m in act {
            // A tactical action still rewards its quiet semimoves (killers it never takes).
            if tactical && mover_victim(s, m).1 != NO_PIECE {
                continue;
            }
            let h = &mut self.history[history_index(s, m)];
            *h += depth * depth;
            if *h > 20000 {
                for x in self.history.iter_mut() {
                    *x /= 2;
                }
            }
        }
        if !tactical && ply <= MAX_PLY && self.killers[ply][0].as_ref() != Some(act) {
            self.killers[ply][1] = self.killers[ply][0].take();
            self.killers[ply][0] = Some(act.clone());
        }
    }

    #[inline]
    fn pv_clear(&mut self, ply: usize) {
        if let Some(n) = self.pv_lens.get_mut(ply) {
            *n = 0;
        }
    }

    /// At a PV node at `ply` whose action `act` raised alpha: its line is `act` followed by
    /// the child's.
    fn pv_update(&mut self, ply: usize, act: &MoveSeq) {
        if ply + 1 >= self.pv_rows.len() {
            return;
        }
        let n = self.pv_lens[ply + 1];
        let (head, tail) = self.pv_rows.split_at_mut(ply + 1);
        let row = &mut head[ply];
        for (i, a) in std::iter::once(act).chain(&tail[0][..n]).enumerate() {
            match row.get_mut(i) {
                Some(r) => r.clone_from(a),
                None => row.push(a.clone()),
            }
        }
        self.pv_lens[ply] = n + 1;
    }

    /// The line the root's child left in the PV table, after the root action `act`.
    fn root_pv(&self, act: &MoveSeq) -> Vec<MoveSeq> {
        let mut pv = Vec::with_capacity(self.pv_lens[1] + 1);
        pv.push(act.clone());
        pv.extend_from_slice(&self.pv_rows[1][..self.pv_lens[1]]);
        pv
    }

    /// The line to report for `pv`, extended with legal TT actions up to the mate or `depth`
    /// when short, stopping at a repeated position.
    fn report_pv(&self, root: &State, pv: &[MoveSeq], score: i32, depth: i32) -> Vec<MoveSeq> {
        // (A score past MATE is no mate distance: a line of none is asked for, not one of
        // a wrapped-around length, which aborted the engine.)
        let len = if score.abs() > MATE - 1000 { (MATE - score.abs()).max(0) as usize } else { depth.max(0) as usize };
        let mut line: Vec<MoveSeq> = Vec::with_capacity(len.max(pv.len()));
        let mut s = root.clone();
        s.enable_undo();
        let mut seen = vec![s.hash()];
        for act in pv {
            if Self::make_checked(&mut s, act).is_none() {
                return line;
            }
            seen.push(s.hash());
            line.push(act.clone());
        }
        while line.len() < len {
            let Some(act) = self.tt.probe(s.live_hash(), |e| e.action.clone()) else { break };
            if act.is_empty() || Self::make_checked(&mut s, &act).is_none() {
                break;
            }
            if seen.contains(&s.hash()) {
                break;
            }
            seen.push(s.hash());
            line.push(act);
        }
        line
    }

    /// The `info` line of a finished depth; with `MultiPV` above 1, of its line `multipv`
    /// (1-based; 0 leaves the tag out, as a plain search does).
    #[allow(clippy::too_many_arguments)]
    fn info_text(&self, root: &State, depth: i32, multipv: usize, score: i32, pv: &[MoveSeq], nodes: u64, elapsed: f64, roots: usize) -> String {
        let pv = self.report_pv(root, pv, score, depth);
        let mut st = root.clone();
        let mut pv_txt = Vec::new();
        for a in &pv {
            pv_txt.push(format!("{} submit", action_text(&st, a)));
            st = Self::child(&st, a);
        }
        let tag = if multipv > 0 { format!(" multipv {}", multipv) } else { String::new() };
        format!(
            "depth {}{} score {} nodes {} nps {:.0} time {} roots {} pv {}",
            depth,
            tag,
            score_text(score),
            nodes,
            nodes as f64 / elapsed.max(1e-6),
            (elapsed * 1000.0) as u64,
            roots,
            pv_txt.join(" ")
        )
    }
}

/// The reported score of an internal `score`: "mate N" when the line ends in mate (found with
/// pruning, so its length is the search's, not a proof), otherwise "cp X" on the reported
/// scale, followed by "wdl W D L" from the evaluation's logistic.
pub fn score_text(score: i32) -> String {
    if score > MATE - 1000 {
        format!("mate {} wdl 1000 0 0", (MATE - score + 1) / 2)
    } else if score < -MATE + 1000 {
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
/// Most lines `MultiPV` accepts.
pub const MAX_MULTIPV: usize = 256;
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
    /// Depth already completed by the search this one resumes (0: none).
    resumed_depth: i32,
    /// The main thread's `MultiPV` (helpers search the same lines, as in Apeiron).
    multipv: usize,
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

/// A Lazy SMP helper thread with its own persistent searcher, running the jobs it is sent in
/// order. It exits once its `Helper` is dropped and its current search is over.
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
                            let HelperJob { root, roots, roots_complete, max_depth, resumed_depth, multipv, start, tt, stop, counters, done } = *job;
                            if stop.load(Ordering::Relaxed) {
                                continue; // That search is over already.
                            }
                            e.tt = tt;
                            e.multipv = multipv;
                            e.begin_search(&stop, None, None);
                            e.nodes_out = &counters[thread_id - 1].0;
                            let interrupt = interrupt_on(&stop, None);
                            let mut r = e.iterate(&root, roots, roots_complete, max_depth, start, None, &mut |_| {}, &[], resumed_depth, None);
                            drop(interrupt);
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

    /// Returns true when the option was recognized: `Hash` (MB), `Threads`, `MultiPV` or
    /// `OwnBook` (true/false, or a number: non-zero is on).
    pub fn set_option(&mut self, key: &str, value: &OptionValue) -> bool {
        if key == "SacBot" {
            self.sac_bot = match value {
                OptionValue::Bool(b) => *b,
                OptionValue::Str(s) => s.eq_ignore_ascii_case("true"),
                v => v.as_f64().is_some_and(|x| x != 0.0),
            };
            return true;
        }
        if key == "MateVerify" {
            return match MateVerify::from_option(value) {
                Some(m) => {
                    self.mate_verify = m;
                    true
                }
                None => false,
            };
        }
        if key == "OwnBook" {
            self.own_book = match value {
                OptionValue::Bool(b) => *b,
                OptionValue::Str(s) => s.eq_ignore_ascii_case("true"),
                v => v.as_f64().is_some_and(|x| x != 0.0),
            };
            return true;
        }
        match (key, value.as_f64()) {
            ("Skill Level", Some(n)) if n.is_finite() => {
                let level = n.clamp(1.0, 8.0) as u8;
                if level != self.skill_level {
                    self.new_game();
                    self.resume = None;
                }
                self.skill_level = level;
                true
            }
            ("Hash", Some(mb)) => {
                self.set_tt(|| TtTable::new((mb as usize).max(1)));
                true
            }
            ("Threads", Some(n)) => {
                self.threads = (n.max(1.0) as usize).min(MAX_THREADS);
                self.helpers.truncate(self.threads - 1);
                true
            }
            ("MultiPV", Some(n)) => {
                self.multipv = (n.max(1.0) as usize).min(MAX_MULTIPV);
                true
            }
            _ => false,
        }
    }

    /// The `multipv` tag of reported line `k` (1-based): none for a plain search.
    fn line_tag(&self, k: usize) -> usize {
        if self.multipv > 1 { k } else { 0 }
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

    /// Search `root` and return the action to play. With `Threads` above 1 this is Lazy SMP:
    /// helpers share the TT, the main thread keeps time and stops them, and the action is
    /// chosen by vote of the threads that report back promptly.
    pub fn find_best_move(&mut self, root: &State, limits: &Limits, stop: &AtomicBool, info: &mut dyn FnMut(String)) -> Option<Action> {
        if self.skill_level == 8 {
            return self.find_best_move_full(root, limits, stop, info);
        }
        let cfg = super::skill::config(self.skill_level);
        let mut limited = limits.clone();
        limited.depth = Some(limited.depth.unwrap_or(cfg.depth).min(cfg.depth));
        limited.nodes = Some(limited.nodes.unwrap_or(cfg.nodes).min(cfg.nodes));
        limited.resume = false;
        let saved = (self.multipv, self.threads, self.own_book);
        // Spend the budget on understanding replies, rather than accurately scoring 32 moves.
        self.multipv = cfg.candidates;
        self.threads = 1;
        self.resume = None;
        let fallback = self.find_best_move_full(root, &limited, stop, info);
        let lines = self.resume.take().map(|r| r.lines).unwrap_or_default();
        (self.multipv, self.threads, self.own_book) = saved;
        let scores: Vec<i32> = lines.iter().map(|l| l.0).collect();
        let mut work = root.clone();
        work.enable_undo();
        let risks: Vec<i32> = lines.iter().map(|line| {
            let Some(action) = line.1.first() else { return i32::MAX; };
            let won = action.iter().map(|m| {
                let (mover, victim) = mover_victim(root, m);
                let promotion = crate::core::action::ExtMove::resolve(*m, root).promote_to;
                super::skill::material_value(victim) + if promotion != NO_PIECE {
                    super::skill::material_value(promotion) - super::skill::material_value(mover)
                } else { 0 }
            }).sum();
            let mark = Self::make(&mut work, action);
            let risk = super::skill::material_risk(&work, root.get_present().1, won);
            work.unmake(mark);
            risk
        }).collect();
        let choice = super::skill::pick(&scores, &risks, self.skill_level, super::book::random_seed());
        if let Some(k) = choice
            && let Some(action) = lines[k].1.first() {
                self.last_score = lines[k].0;
                self.last_proven = false;
                return Some(Action::from_moveseq(action, root));
            }
        fallback
    }

    fn find_best_move_full(&mut self, root: &State, limits: &Limits, stop: &AtomicBool, info: &mut dyn FnMut(String)) -> Option<Action> {
        if self.own_book {
            let book = super::book::Book::theory();
            if let Some(m) = book.choose(root, super::book::random_seed()) {
                info(format!("string book {} ; {} (weight {} of {})", m.lan, m.pgn, m.weight, book.play_weight(root)));
                self.last_score = 0;
                self.last_proven = false;
                return Some(m.action.clone());
            }
        }
        if self.sac_bot {
            let mut work = root.clone();
            work.enable_undo();
            for fm in super::sac::signature_travels(root) {
                let seq: MoveSeq = MoveSeq::from(&[fm][..]);
                if let Some(mark) = Self::make_checked(&mut work, &seq) {
                    work.unmake(mark);
                    info(format!("string sacbot {}", fm));
                    self.last_score = 0;
                    return Some(Action::from_moveseq(&seq, root));
                }
            }
        }
        let start = Instant::now();
        let plan = TimePlan::new(limits, Self::mandatory_boards(root), root.active_timelines());
        let deadline = plan.as_ref().map(|p| start + Duration::from_millis(p.maximum.saturating_sub(5).max(1)));
        // The f7 sac table: local outcomes of the travels (and created timelines) this
        // position may need, kept for the rest of the game.
        super::sac::prepare(root, SAC_PREP_NODES, None, Some(stop), super::sac::PROBE_NODES, false);
        self.begin_search(stop, limits.nodes, deadline);
        // Enumerating actions stops at the deadline or when told to stop (see `interrupted`).
        let _interrupt = interrupt_on(stop, deadline);
        let max_depth = limits.depth.unwrap_or(64) as i32;

        // Resuming the last search of this position: its root actions, order and depth.
        let resume = if limits.resume {
            self.resume.clone().filter(|r| r.hash == root.hash() && r.completed_depth > 0 && r.multipv == self.multipv)
        } else {
            None
        };
        if let Some(r) = &resume {
            if r.completed_depth >= max_depth {
                self.last_score = r.score;
                self.last_proven = r.proven;
                return Some(Action::from_moveseq(&r.best, root));
            }
            return self.search_roots(root, r.roots.clone(), r.roots_complete, max_depth, start, plan.as_ref(), info, Some(r));
        }

        // Root actions (all of them, up to the root cap), ordered heuristically.
        let (mut hc, ss) = HcInfo::build_hc(root);
        // Mate-in-one probes before the search, in order: lone travels, travels from optional
        // boards, late-ordered checking travels (`travel_mate`, which also become root actions),
        // then a scan of every legal action, checks first.
        let mut travel_checks: Vec<MoveSeq> = Vec::new();
        let scan_start = Instant::now();
        let (budget, recheck) = self.root_mate_budget(plan.as_ref());
        self.mate_recheck = recheck;
        let (future_mate, scan_end) = {
            let mut s = root.clone();
            s.enable_undo();
            let mut scratch = OrderScratch::default();
            hc.materialize_branch(&s);
            let probed = self
                .lone_travel_mate(&mut s, &hc)
                .or_else(|| self.future_mate_in_one(&mut s, &hc, &ss, &mut scratch))
                .or_else(|| self.travel_mate(&mut s, &mut hc, &ss, &mut scratch, Some(&mut travel_checks), ROOT_TRAVEL_MATE_COMPLETIONS, Some(ROOT_PROBE_STEPS)));
            match probed {
                Some(m) => (Some(m), None),
                None => match self.root_mate_scan(&mut s, &mut hc, &ss, &mut scratch, &budget) {
                    ScanEnd::Found(m) => (Some(m), None),
                    end => (None, Some(end)),
                },
            }
        };
        self.mate_recheck = 0;
        // An analysis (no clock) is told how far the search for a mate in one went.
        if !plan.as_ref().is_some_and(|p| p.clock) && future_mate.is_none() && !interrupted() {
            match scan_end {
                Some(ScanEnd::Exhausted) => info("string mate1 none (every action tried)".to_string()),
                Some(ScanEnd::OutOfBudget { tried, unresolved }) => {
                    info(format!("string mate1 partial ({} actions, {} unresolved, {} ms)", tried, unresolved, scan_start.elapsed().as_millis()))
                }
                _ => {}
            }
        }
        // A checkmate in one the probes found is proven (each confirms the mate exactly): it is
        // played at once, with no root actions generated and no search (MultiPV wants the
        // other lines too).
        if let (Some(m), true) = (&future_mate, self.multipv <= 1) {
            info(self.info_text(root, 1, self.line_tag(1), MATE - 1, std::slice::from_ref(m), self.nodes, start.elapsed().as_secs_f64(), 1));
            self.last_score = MATE - 1;
            self.last_proven = true;
            self.resume = None;
            return Some(Action::from_moveseq(m, root));
        }
        let mut agen = BestFirstCursor::default();
        let mut first = Self::without_future(root, &hc, &ss);
        let travels = self.optional_travels(root, &hc, &ss, &mut first);
        let mut travel_acts: Vec<MoveSeq> = Vec::new();
        self.start_actions(root, &hc, &mut agen, &mut OrderScratch::default(), first, false);
        agen.limit_classes(travels as i32, TRAVEL_CAPTURE_STEPS);
        let mut full = Some(ss);
        let mut roots: Vec<(i32, MoveSeq)> = Vec::new();
        // Enumerating every root action can take seconds with many timelines: spend at most
        // a quarter of the budget on it (always keeping at least one action), and stop at once
        // when told to.
        let mut gen_bounded = false;
        let mut complete = false;
        while roots.len() < ROOT_CAP {
            if !roots.is_empty() && stop.load(Ordering::Relaxed) {
                break;
            }
            // The space without optional boards gets ROOT_FIRST_STEPS to yield a first action,
            // falling back to the whole space; the remaining actions get ROOT_GEN_STEPS.
            if roots.is_empty() {
                agen.set_budget(if full.is_some() { Some(ROOT_FIRST_STEPS) } else { None });
            } else if !gen_bounded {
                agen.set_budget(Some(ROOT_GEN_STEPS));
                gen_bounded = true;
            }
            let next = agen.next(&hc, root);
            match next {
                Some(a) => {
                    if full.is_some() && Self::travel_class(&mut agen, travels) {
                        travel_acts.push(a.clone());
                        roots.push((self.action_score(root, &a, 0), a));
                    } else if !travel_acts.contains(&a) {
                        roots.push((self.action_score(root, &a, 0), a));
                    }
                }
                // Out of time: with no action found yet, none can be played.
                None if interrupted() => break,
                None if roots.is_empty() && full.is_some() => agen.restart(&hc, full.take().unwrap()),
                // Out of steps with actions found: the root searches those.
                None if agen.budget_spent() => break,
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
        if complete && roots.len() == 1 && plan.as_ref().is_some_and(|p| p.clock) && future_mate.is_none() {
            return Some(Action::from_moveseq(&roots[0].1, root));
        }
        // A checking travel the root's actions come to late or never (one creating a timeline
        // that moves the present back, so that it is an action alone, comes after every action
        // creating none) is searched as a root action: it can start a mate in more than one.
        for a in travel_checks {
            if !roots.iter().any(|r| r.1 == a) {
                roots.push((self.action_score(root, &a, 0), a));
            }
        }
        roots.sort_by(|a, b| b.0.cmp(&a.0));
        // The probe's mate is a root action, searched first: every depth's line (and a
        // resumed search) has it.
        if let Some(m) = &future_mate {
            roots.retain(|r| r.1 != *m);
            roots.insert(0, (i32::MAX, m.clone()));
        }
        let best = self.search_roots(root, roots, roots_complete, max_depth, start, plan.as_ref(), info, None);
        match future_mate {
            Some(m) if self.last_score < MATE - 1 => {
                info(self.info_text(root, 1, self.line_tag(1), MATE - 1, std::slice::from_ref(&m), self.nodes, start.elapsed().as_secs_f64(), 1));
                self.last_score = MATE - 1;
                self.last_proven = true;
                self.resume = None;
                Some(Action::from_moveseq(&m, root))
            }
            _ => best,
        }
    }

    /// Search the ordered root actions `roots` (Lazy SMP with helpers when `Threads` > 1),
    /// from scratch or continuing `resume`.
    #[allow(clippy::too_many_arguments)]
    fn search_roots(
        &mut self,
        root: &State,
        roots: Vec<(i32, MoveSeq)>,
        roots_complete: bool,
        max_depth: i32,
        start: Instant,
        plan: Option<&TimePlan>,
        info: &mut dyn FnMut(String),
        resume: Option<&SearchResume>,
    ) -> Option<Action> {
        let resumed_depth = resume.map_or(0, |r| r.completed_depth);
        if self.threads <= 1 {
            let r = self.iterate(root, roots, roots_complete, max_depth, start, plan, info, &[], resumed_depth, resume);
            self.last_score = r.score;
            self.last_proven = r.proven;
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
                resumed_depth,
                multipv: self.multipv,
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
        let root_count = roots.len();
        let main = self.iterate(root, roots, roots_complete, max_depth, start, plan, info, &counters, resumed_depth, resume);
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
        let chosen = &results[select_best_thread(&results)];
        if chosen.thread_id != 0 && chosen.completed_depth > 0 {
            // The vote went to a helper: its depth and line (lines) are the ones reported last.
            let nodes = self.nodes + self.helper_nodes;
            let elapsed = start.elapsed().as_secs_f64();
            if self.multipv > 1 {
                // Its best line first, then its other lines of the last depth it completed.
                let rest = chosen.lines.iter().filter(|l| l.1.first() != Some(&chosen.best));
                let lines = std::iter::once((chosen.score, chosen.pv.clone())).chain(rest.cloned()).take(self.multipv);
                for (k, (score, pv)) in lines.enumerate() {
                    info(self.info_text(root, chosen.completed_depth, k + 1, score, &pv, nodes, elapsed, root_count));
                }
            } else {
                let line = self.info_text(root, chosen.completed_depth, 0, chosen.score, &chosen.pv, nodes, elapsed, root_count);
                info(line);
            }
        }
        self.last_score = chosen.score;
        self.last_proven = chosen.proven;
        Some(Action::from_moveseq(&chosen.best, root))
    }

    /// Iterative deepening over `roots` (best first). The main thread (`thread_id` 0) keeps
    /// time and reports each depth through `info`; helpers search until stopped.
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
        resumed_depth: i32,
        resume: Option<&SearchResume>,
    ) -> ThreadResult {
        let main = self.thread_id == 0;
        // The f7 sac table's winning travels at this root, for the evaluation.
        self.terms.sac_root = super::sac::root_travels(root);
        let mut work = root.clone();
        work.enable_undo();
        let mut best: MoveSeq = resume.map_or_else(|| roots[0].1.clone(), |r| r.best.clone());
        let mut best_score: i32 = resume.map_or(0, |r| r.score);
        // `best_score` is a proven mate (see `proven`): only then is it reported as a mate.
        let mut best_proven = resume.is_some_and(|r| r.proven);
        // The line of `best`, from the PV table of the depth that chose it.
        let mut best_pv: Vec<MoveSeq> = resume.map_or_else(|| vec![best.clone()], |r| r.pv.clone());
        // Only the main thread carries the resumed search's result; a helper of a resumed
        // search starts past the resumed depth too, but has finished (and votes with) only
        // the depths it searches itself.
        let mut completed_depth = if resume.is_some() { resumed_depth } else { 0 };
        // MultiPV (`multipv_pass`): the lines of the last completed depth, best first. A plain
        // search (`MultiPV` 1) never touches them.
        let multi = self.multipv > 1;
        let mut lines: Vec<PvLine> = resume.map_or_else(Vec::new, |r| r.lines.clone());
        // Clock stability: decaying count of best-action changes and running average score.
        let mut changes = 0.0f64;
        let mut avg_score: Option<i32> = None;
        // Lazy SMP: odd helpers skip depth 1 and run a depth ahead, so the threads diverge
        // and fill the table at a spread of depths instead of re-walking the same iteration.
        let ahead = self.thread_id % 2 == 1;
        let first = if ahead { 2.min(max_depth) } else { 1 }.max((resumed_depth + 1).min(max_depth));
        // The optional-board root actions were added (see below).
        let mut root_widened = false;
        let mut base_depth = first;
        while base_depth <= max_depth {
            let depth = if ahead { (base_depth + 1).min(max_depth) } else { base_depth };
            let prev_best = best.clone();
            // Aspiration window around the last completed depth's score (see ASP_WINDOW).
            let prev_score = best_score;
            let aspire = depth >= ASP_MIN_DEPTH && completed_depth > 0 && prev_score.abs() < MATE - 1000;
            let mut window = ASP_WINDOW;
            let (mut lo, mut hi) = if aspire { (prev_score - window, prev_score + window) } else { (-INF, INF) };
            let mut tries = 0;
            // The action and line of the last fail high: kept if the re-search is interrupted
            // before it has a best of its own.
            let mut fail_high: Option<(MoveSeq, Vec<MoveSeq>)> = None;
            let mut iter_best: Option<(i32, MoveSeq, bool)>;
            let mut iter_pv: Vec<MoveSeq>;
            let mut all_proven;
            let mut scores: Vec<i32>;
            let mut depth_lines: Vec<PvLine> = Vec::new();
            if multi {
                let pass = self.multipv_pass(&mut work, &roots, depth, lo, hi, roots_complete);
                // Its scores are exact: no fail high is a bound to keep.
                hi = INF;
                iter_best = pass.best;
                iter_pv = pass.best_pv;
                all_proven = pass.all_proven;
                scores = pass.scores;
                depth_lines = pass.lines;
            } else {
                loop {
                    let mut alpha = lo;
                    let beta = hi;
                    iter_best = None;
                    iter_pv = Vec::new();
                    all_proven = roots_complete;
                    scores = Vec::with_capacity(roots.len());
                    let mut top = -INF;
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
                        top = top.max(v);
                        if v > alpha {
                            alpha = v;
                            iter_best = Some((v, act.clone(), self.proven));
                            iter_pv = self.root_pv(act);
                            if v >= beta {
                                break; // Fail high.
                            }
                        }
                    }
                    tries += 1;
                    if self.aborted || (lo == -INF && hi == INF) {
                        break;
                    }
                    if top <= lo {
                        window *= ASP_FAIL_MULT;
                        lo = (prev_score - window).max(-INF);
                        // Nothing reached the last depth's score: an earlier fail high is stale.
                        fail_high = None;
                    } else if top >= hi {
                        window *= ASP_FAIL_MULT;
                        hi = (prev_score + window).min(INF);
                        // The action that failed high is searched first again.
                        let (_, act, _) = iter_best.take().expect("a fail high has a best action");
                        if let Some(i) = roots.iter().position(|r| r.1 == act) {
                            roots[..=i].rotate_right(1);
                        }
                        fail_high = Some((act, std::mem::take(&mut iter_pv)));
                    } else {
                        break;
                    }
                    if window > ASP_MAX_WINDOW || tries >= ASP_MAX_TRIES {
                        (lo, hi) = (-INF, INF);
                    }
                }
            }
            // Every root action searched loses to a mate, or the best wins only by a mate in 3
            // or more: the actions on boards the side to move need not play are added (once,
            // searched first) and the depth is searched again (they may mate sooner or save).
            if !self.aborted && !root_widened && iter_best.as_ref().is_some_and(|b| b.0 < -MATE + 1000 || (b.0 > MATE - 1000 && b.0 <= MATE - 5)) {
                root_widened = true;
                let extra = self.root_widen(root, &roots);
                if !extra.is_empty() {
                    roots.splice(0..0, extra);
                    continue;
                }
            }
            base_depth += 1;
            if self.aborted {
                // An interrupted try's fail high is only a bound.
                if iter_best.as_ref().is_some_and(|b| b.0 >= hi) {
                    let (_, act, _) = iter_best.take().expect("checked");
                    fail_high = Some((act, std::mem::take(&mut iter_pv)));
                }
                // With no best of its own, the interrupted re-search keeps the action that failed
                // high; its score (a bound) is not taken: the last completed depth's stands.
                if let Some((act, pv)) = fail_high.take().filter(|_| iter_best.is_none()) {
                    if best_pv.first() != Some(&act) {
                        best_pv = pv;
                    }
                    best = act;
                }
            }
            // A partial iteration replaces the best, except: an earlier mate survives a worse or
            // interrupted depth (unless unproven and refuted here), an interrupted loss is not
            // trusted, and a proven loss is final.
            let refuted = !best_proven && roots.first().is_some_and(|r| r.1 == best) && scores.first().is_some_and(|&v| v <= MATE - 1000);
            let mut kept = false;
            if let Some((v, act, proven)) = iter_best {
                let mate_kept = best_score > MATE - 1000 && v < best_score && !refuted;
                let loss_kept = best_score < -(MATE - 1000) && v >= -(MATE - 1000) && best_proven;
                kept = depth > 1 && (mate_kept || loss_kept || (self.aborted && v < -(MATE - 1000)));
                if !kept {
                    // An interrupted depth's line for the same action stops where the search
                    // did: the last depth's longer one stands.
                    if !(self.aborted && iter_pv.len() < best_pv.len() && best_pv.first() == Some(&act)) {
                        best_pv = std::mem::take(&mut iter_pv);
                    }
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
                // A kept mate keeps the lines it was found with (as in Apeiron).
                if multi && !kept {
                    lines = depth_lines;
                }
            }
            if !self.aborted && main {
                let nodes = self.nodes + helper_nodes.iter().map(|c| c.0.load(Ordering::Relaxed)).sum::<u64>();
                if multi {
                    for (k, (score, pv)) in lines.iter().enumerate() {
                        info(self.info_text(root, depth, k + 1, *score, pv, nodes, elapsed, roots.len()));
                    }
                } else {
                    info(self.info_text(root, depth, 0, best_score, &best_pv, nodes, elapsed, roots.len()));
                }
            }
            if self.aborted {
                break;
            }
            // Delay clocks: a depth is started on the free time alone (see `TimePlan::free`).
            let free = plan.map_or(0, |p| p.free);
            let mut on_free = false;
            if let Some(p) = plan.filter(|p| p.clock) {
                changes = changes / 2.0 + if depth > 1 && best != prev_best { 1.0 } else { 0.0 };
                let falling = avg_score.map_or(1.0, |a| (1.0 + (a - best_score) as f64 / 150.0).clamp(0.75, 1.6));
                avg_score = Some(avg_score.map_or(best_score, |a| (a + best_score) / 2));
                let instability = (0.8 + 0.6 * changes).min(2.0);
                // The next depth costs about as much as all before it together: none is
                // started past half the target.
                if elapsed * 1000.0 > p.target(falling, instability) * 0.5 {
                    if elapsed * 1000.0 >= p.free as f64 {
                        break;
                    }
                    on_free = true;
                }
            }
            // Keep searching after a mate (pruning can report false or long mates) until the
            // mate is well inside the searched depth (with MultiPV: every line's). (Helpers
            // search on until stopped.)
            let mate_done = if multi {
                !lines.is_empty() && lines.iter().all(|l| l.0.abs() > MATE - 1000 && depth >= 2 * (MATE - l.0.abs()) + 2)
            } else {
                best_score.abs() > MATE - 1000 && depth >= 2 * (MATE - best_score.abs()) + 2
            };
            if main && mate_done {
                break;
            }
            if let Some(d) = self.deadline {
                // Do not start an iteration that cannot finish.
                let spent = start.elapsed();
                let left = d.saturating_duration_since(Instant::now());
                if left < spent {
                    if spent.as_millis() as u64 >= free {
                        break;
                    }
                    on_free = true;
                }
            }
            if on_free {
                // A delay clock's free time is left: search on, but only until it runs out.
                let soft = start + Duration::from_millis(free);
                self.deadline = Some(self.deadline.map_or(soft, |d| d.min(soft)));
            }
        }
        if main && completed_depth > 0 {
            self.resume = Some(SearchResume {
                hash: root.hash(),
                roots,
                roots_complete,
                completed_depth,
                best: best.clone(),
                score: best_score,
                proven: best_proven,
                pv: best_pv.clone(),
                multipv: self.multipv,
                lines: lines.clone(),
            });
        }
        let pv_len = best_pv.len();
        ThreadResult { best, score: best_score, proven: best_proven, completed_depth, pv: best_pv, pv_len, nodes: self.nodes, thread_id: self.thread_id, lines }
    }

    /// One depth of MultiPV root search: the first action uses the window `lo..hi`, the next
    /// get full windows until `MultiPV` are exact, then each is scouted at the `MultiPV`-th best
    /// score and re-searched only if it beats it.
    fn multipv_pass(&mut self, work: &mut State, roots: &[(i32, MoveSeq)], depth: i32, lo: i32, hi: i32, roots_complete: bool) -> MultiPass {
        let want = self.multipv;
        let mut pass = MultiPass { best: None, best_pv: Vec::new(), all_proven: roots_complete, scores: Vec::with_capacity(roots.len()), lines: Vec::new() };
        // The best exact scores so far, best first, at most `want` of them.
        let mut top: Vec<i32> = Vec::with_capacity(want + 1);
        for (k, (_, act)) in roots.iter().enumerate() {
            let bar = if top.len() >= want { top[want - 1] } else { -INF };
            let mark = Self::make(work, act);
            let child_depth = depth - super::skill::travel_reduction(self.skill_level, act);
            let mut exact = true;
            let v = if k == 0 {
                let mut v = -self.negamax(work, child_depth - 1, 1, -hi, -lo, true);
                if !self.aborted && (v <= lo || v >= hi) && (lo > -INF || hi < INF) {
                    v = -self.negamax(work, child_depth - 1, 1, -INF, INF, true);
                }
                v
            } else if bar == -INF {
                -self.negamax(work, child_depth - 1, 1, -INF, INF, true)
            } else {
                let child_check = if depth >= 3 && k >= ROOT_LMR_MIN_INDEX { Some(self.in_check(work)) } else { None };
                let r = if child_check == Some(false) { (lmr(depth, k) - 1).clamp(0, depth - 2) } else { 0 };
                self.known_check = child_check;
                let mut v = -self.negamax(work, child_depth - 1 - r, 1, -bar - 1, -bar, false);
                if !self.aborted && v > bar && r > 0 {
                    self.known_check = child_check;
                    v = -self.negamax(work, child_depth - 1, 1, -bar - 1, -bar, false);
                }
                if !self.aborted && v > bar {
                    self.known_check = child_check;
                    v = -self.negamax(work, child_depth - 1, 1, -INF, -bar, true);
                }
                exact = v > bar;
                if exact { v } else { v.min(bar - 1) }
            };
            work.unmake(mark);
            if self.aborted {
                break;
            }
            pass.scores.push(v);
            pass.all_proven &= self.proven;
            if exact {
                let pv = self.root_pv(act);
                if pass.best.as_ref().is_none_or(|b| v > b.0) {
                    pass.best = Some((v, act.clone(), self.proven));
                    pass.best_pv.clone_from(&pv);
                }
                let i = top.partition_point(|&t| t >= v);
                top.insert(i, v);
                top.truncate(want);
                pass.lines.push((v, pv));
            }
        }
        // Best first (a stable sort: on equal scores, the earlier action, as `best`).
        pass.lines.sort_by(|a, b| b.0.cmp(&a.0));
        pass.lines.truncate(want);
        pass
    }

    /// The score of the action the last `find_best_move` returned (internal units, from the
    /// side to move's view; mates as `MATE - plies`, proven or not).
    pub fn last_score(&self) -> i32 {
        self.last_score
    }

    /// Whether `last_score`, a mate score, is a proven mate (a forced mate within its distance
    /// for a mating score, every legal action searched for a mated one).
    pub fn last_proven(&self) -> bool {
        self.last_proven
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::state::MateType;

    /// A thread's result; mate scores proven.
    fn result(from: u32, score: i32, depth: i32, thread_id: usize) -> ThreadResult {
        let m = FullMove { from: crate::core::vec4::Vec4(from), to: crate::core::vec4::Vec4(from + 1) };
        ThreadResult { best: MoveSeq::from(&[m][..]), score, proven: true, completed_depth: depth, pv: Vec::new(), pv_len: 5, nodes: 0, thread_id, lines: Vec::new() }
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

    /// The field after `key` in an info line, and the first action of its line.
    fn info_field<'a>(line: &'a str, key: &str) -> &'a str {
        let at = line.find(&format!(" {} ", key)).map(|i| i + key.len() + 2).unwrap_or_else(|| panic!("no {} in {}", key, line));
        let rest = &line[at..];
        let end = match key {
            "score" => rest.find(" nodes").unwrap(),
            "pv" => rest.find(" submit").unwrap(),
            _ => rest.find(' ').unwrap_or(rest.len()),
        };
        &rest[..end]
    }

    #[test]
    fn skill_levels_return_legal_actions_and_restore_options() {
        let s = crate::core::variants::startpos();
        let stop = AtomicBool::new(false);
        let mut e = AlphaBeta::new();
        e.set_option("Hash", &OptionValue::Int(1));
        for level in 1..=8 {
            assert!(e.set_option("Skill Level", &OptionValue::Int(level)));
            let a = e.find_best_move(&s, &Limits { depth: Some(1), movetime: Some(100), ..Default::default() }, &stop, &mut |_| {}).unwrap();
            let mut state = s.clone();
            let lan = crate::web::tree::action_lan(&s, &a);
            assert!(crate::core::variants::apply_uci_moves(&mut state, &lan).is_ok(), "level {level}");
            assert_eq!((e.multipv, e.threads, e.own_book), (1, 1, false));
        }
    }

    /// MultiPV 3 reports three distinct lines per depth, best first, matching the plain search's
    /// score for its action; a plain search tags no line.
    #[test]
    fn multipv_reports_distinct_sorted_lines() {
        let s = start_with_moves("(0T1)e2e4 submit (0T1)e7e5 submit");
        let stop = AtomicBool::new(false);
        let depth = 4;
        let limits = Limits { depth: Some(depth), ..Default::default() };
        let mut plain = Vec::new();
        let mut e = AlphaBeta::new();
        e.set_option("Hash", &OptionValue::Int(16));
        e.find_best_move(&s, &limits, &stop, &mut |m| plain.push(m)).expect("an action");
        assert!(plain.iter().all(|l| !l.contains("multipv")));
        let plain_last = plain.last().unwrap().clone();

        let mut out = Vec::new();
        let mut e = AlphaBeta::new();
        e.set_option("Hash", &OptionValue::Int(16));
        assert!(e.set_option("MultiPV", &OptionValue::Int(3)));
        e.find_best_move(&s, &limits, &stop, &mut |m| out.push(m)).expect("an action");
        for d in 1..=depth {
            let lines: Vec<&String> = out.iter().filter(|l| l.starts_with(&format!("depth {} ", d))).collect();
            assert_eq!(lines.len(), 3, "depth {}: {:?}", d, lines);
            let tags: Vec<&str> = lines.iter().map(|l| info_field(l, "multipv")).collect();
            assert_eq!(tags, ["1", "2", "3"]);
            let acts: Vec<&str> = lines.iter().map(|l| info_field(l, "pv")).collect();
            assert!(acts[0] != acts[1] && acts[1] != acts[2] && acts[0] != acts[2], "{:?}", acts);
            let cps: Vec<i64> = lines.iter().map(|l| info_field(l, "cp").parse().unwrap()).collect();
            assert!(cps[0] >= cps[1] && cps[1] >= cps[2], "{:?}", cps);
        }
        let last: Vec<&String> = out.iter().filter(|l| l.starts_with(&format!("depth {} ", depth))).collect();
        let same = last.iter().find(|l| info_field(l, "pv") == info_field(&plain_last, "pv")).expect("the plain search's action is a line");
        // (Pruning makes a score depend a little on the window it was searched in, so the two
        // modes agree only to within a few centipawns.)
        let cp = |l: &str| info_field(l, "cp").parse::<i64>().unwrap();
        assert!((cp(same) - cp(&plain_last)).abs() <= 40, "{}\n{}", same, plain_last);
    }

    /// Mate in one by (0T10)Bf1>>(0T4)f7: the bishop travels back from a board ahead of the
    /// present, which the root's actions leave out (tactics suite, T0 1.e3 Ng4 line).
    #[test]
    fn mate_in_one_by_travel_from_a_future_board() {
        let s = crate::core::perft::parse_g_line(
            "G 1 size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:0:b] \
             [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w] moves (0T1)e2e3 submit \
             (0T1)g8f6 submit (0T2)g1f3 submit (0T2)f6g4 submit (0T3)f3g5 submit (0T3)f7f5 submit (0T4)d1g4 submit \
             (0T4)f5g4 submit (0T5)g5f7 submit (0T5)e7e6 submit (0T6)f7d8 submit (0T6)e8d8 submit (0T7)a2a3 submit \
             (0T7)b8a6 submit (0T8)f2f4 submit (0T8)a6c5 submit (0T9)g2g3 submit (0T9)f8(0T3)f2 submit \
             (-1T4)e1(0T5)d1 submit (-1T4)e7e6 submit",
        )
        .unwrap();
        let mut e = AlphaBeta::new();
        let stop = AtomicBool::new(false);
        let mut last = String::new();
        let a = e.find_best_move(&s, &Limits { depth: Some(3), ..Default::default() }, &stop, &mut |m| last = m).unwrap();
        let seq: MoveSeq = a.moves().iter().map(|m| m.fm).collect();
        assert_eq!(AlphaBeta::child(&s, &seq).get_mate_type(), MateType::Checkmate, "played {:?}", seq);
        assert_eq!(e.last_score(), MATE - 1);
        assert!(last.contains("score mate 1 "), "{}", last);
    }

    /// Mate in one by a travel from a board ahead of the present onto a board at the present
    /// ((0T6)Qb2>>(0T3)e5, Knights vs. Bishops): it branches a timeline and activates another
    /// line, whose board must be played with it (reported by a user).
    #[test]
    fn mate_in_one_by_travel_onto_the_present() {
        let s = crate::core::perft::parse_g_line("G 1 size 6x6 odd fen [r*bqk*br*/p*p*p*p*p*p*/6/6/P*P*P*P*P*P*/R*NQK*NR*:0:1:w] moves (0T1)f2f4 submit (0T1)e5e4 submit (0T2)b2b3 submit (0T2)d5d3 submit (0T3)c1b2 submit (0T3)c5c3 submit (0T4)b3c4 submit (0T4)d3e2 submit (0T5)d1c1 submit (0T5)c6(0T1)c2 submit (-1T2)e1c2 submit (-1T2)e5(0T2)e5 submit").unwrap();
        let mut e = AlphaBeta::new();
        let stop = AtomicBool::new(false);
        let mut last = String::new();
        let a = e.find_best_move(&s, &Limits { depth: Some(2), ..Default::default() }, &stop, &mut |m| last = m).unwrap();
        let seq: MoveSeq = a.moves().iter().map(|m| m.fm).collect();
        assert_eq!(AlphaBeta::child(&s, &seq).get_mate_type(), MateType::Checkmate, "played {:?}", seq);
        assert!(last.contains("score mate 1 "), "{}", last);
    }

    /// Mate in one with three boards to play, the third a travel from a board ahead of the
    /// present ((0T8)Ne1>>x(0T7)e3, Knights vs. Bishops): the mating action lies past the root
    /// scan's first `ROOT_SCAN_ACTIONS` actions.
    #[test]
    fn mate_in_one_deep_in_the_action_space() {
        let s = crate::core::perft::parse_g_line("G 1 size 6x6 odd fen [r*bqk*br*/p*p*p*p*p*p*/6/6/P*P*P*P*P*P*/R*NQK*NR*:0:1:w] moves (0T1)f2f4 submit (0T1)e5e4 submit (0T2)b2b3 submit (0T2)a5a3 submit (0T3)b1a3 submit (0T3)b6a5 submit (0T4)c1b2 submit (0T4)a5d2 submit (0T5)d1d2 submit (0T5)c5c3 submit (0T6)b3c4 submit (0T6)e4e3 submit (0T7)d2c1 submit (0T7)c6(0T3)c2 submit (-1T4)a3c2 submit (-1T4)b6(0T5)b6 submit (-1T5)c2d4 submit (-1T5)c5d4 submit").unwrap();
        let mut e = AlphaBeta::new();
        let stop = AtomicBool::new(false);
        let mut last = String::new();
        let a = e.find_best_move(&s, &Limits { depth: Some(2), ..Default::default() }, &stop, &mut |m| last = m).unwrap();
        let seq: MoveSeq = a.moves().iter().map(|m| m.fm).collect();
        assert_eq!(AlphaBeta::child(&s, &seq).get_mate_type(), MateType::Checkmate, "played {:?}", seq);
        assert!(last.contains("score mate 1 "), "{}", last);
    }

    #[test]
    fn mate_verify_option_values() {
        use crate::engine::OptionValue as V;
        assert_eq!(MateVerify::from_option(&V::Str("auto".into())), Some(MateVerify::Auto));
        assert_eq!(MateVerify::from_option(&V::Str("Exhaustive".into())), Some(MateVerify::Exhaustive));
        assert_eq!(MateVerify::from_option(&V::Str("2500".into())), Some(MateVerify::Millis(2500)));
        assert_eq!(MateVerify::from_option(&V::Int(0)), Some(MateVerify::Auto));
        assert_eq!(MateVerify::from_option(&V::Int(-1)), Some(MateVerify::Exhaustive));
        assert_eq!(MateVerify::from_option(&V::Int(40)), Some(MateVerify::Millis(40)));
        assert_eq!(MateVerify::from_option(&V::Str("soon".into())), None);
        let mut e = AlphaBeta::new();
        assert!(e.set_option("MateVerify", &V::Str("exhaustive".into())));
        assert!(!e.set_option("MateVerify", &V::Str("soon".into())));
    }

    /// An analysis is told when the scan of every action found no mate in one (the start
    /// position has about twenty actions), and an exhaustive one finds the deep mate too.
    #[test]
    fn mate_in_one_scan_reports_and_exhausts() {
        let stop = AtomicBool::new(false);
        let mut e = AlphaBeta::new();
        let mut lines = Vec::new();
        e.find_best_move(&crate::core::variants::startpos(), &Limits { depth: Some(1), ..Default::default() }, &stop, &mut |m| lines.push(m));
        assert!(lines.iter().any(|l| l.starts_with("string mate1 none")), "{:?}", lines);
        let s = crate::core::perft::parse_g_line(include_str!("../../tests/data/mate1_deep.g").lines().next().unwrap()).unwrap();
        let mut e = AlphaBeta::new();
        e.set_option("MateVerify", &crate::engine::OptionValue::Str("exhaustive".into()));
        let mut last = String::new();
        e.find_best_move(&s, &Limits { depth: Some(1), ..Default::default() }, &stop, &mut |m| last = m);
        assert!(last.contains("score mate 1 "), "{}", last);
    }

    /// A position with a legal escape is no checkmate under a stop guard either (an interrupted
    /// enumeration finds no action, which read as checkmate).
    #[test]
    fn mate_type_ignores_a_stop() {
        use crate::core::state::MateType;
        let s = crate::core::perft::parse_g_line(include_str!("../../tests/data/mate_interrupt.g").trim()).unwrap();
        let (h, ss) = HcInfo::build_hc(&s);
        let a = h.search(&s, ss).next().unwrap();
        let child = AlphaBeta::child(&s, &a);
        assert_eq!(child.get_mate_type(), MateType::Softmate);
        let stop = AtomicBool::new(true);
        let _guard = interrupt_on(&stop, None);
        assert_eq!(child.get_mate_type(), MateType::Softmate);
    }

    /// A mate in one by a lone travel moving the present back, (-2T18)c5(2T14)c5, with every
    /// other board passing, missed in a game: its check comes from another timeline (the
    /// detector does not flag it) and it creates a timeline (last in the scan's order).
    #[test]
    fn mate_in_one_by_a_lone_travel_on_the_clock() {
        let mut lines = include_str!("../../tests/data/lone_travel_mate.txt").lines();
        let mut s = crate::core::variants::position_from_uci(lines.next().unwrap()).unwrap();
        crate::core::variants::apply_uci_moves(&mut s, lines.next().unwrap()).unwrap();
        let stop = AtomicBool::new(false);
        let mut e = AlphaBeta::new();
        let limits = Limits { depth: Some(1), time_left: Some(10_000), increment: Some(100), ..Default::default() };
        let a = e.find_best_move(&s, &limits, &stop, &mut |_| {}).unwrap();
        let child = s.can_apply_action(&a).unwrap();
        assert_eq!(child.get_mate_type(), crate::core::state::MateType::Checkmate, "played {}", a);
    }

    /// Mate in one by (-1T3)Uc2>>c3 from a board ahead of the present (Timeline Strategos).
    #[test]
    fn mate_in_one_by_travel_from_the_future_with_other_boards() {
        let s = crate::core::perft::parse_g_line("G 1 size 5x5 even fen [nbk*ur*/p*p*p*p*p*/5/5/5:-0:1:w] [5/5/5/P*P*P*P*P*/R*UK*BN:+0:1:w] moves (0T1)b1(-1T1)c2 submit (-1T1)a5(0T1)a3 submit (0T2)e1(-1T2)e3 submit (-1T2)d5(0T1)e5 (0T2)a3(0T1)a5 submit").unwrap();
        let mut e = AlphaBeta::new();
        let stop = AtomicBool::new(false);
        let mut last = String::new();
        let a = e.find_best_move(&s, &Limits { depth: Some(2), ..Default::default() }, &stop, &mut |m| last = m).unwrap();
        let seq: MoveSeq = a.moves().iter().map(|m| m.fm).collect();
        assert_eq!(AlphaBeta::child(&s, &seq).get_mate_type(), MateType::Checkmate, "played {:?}", seq);
        assert!(last.contains("score mate 1 "), "{}", last);
    }

    /// Mate in one by (0T8)Qb2>>x(0T5)b5 with two helper knight moves on the other boards, one
    /// of them a capture (Knights vs. Bishops, reported by a user): the root scans every legal
    /// action for a checkmate.
    #[test]
    fn mate_in_one_needing_helper_captures() {
        let s = crate::core::perft::parse_g_line("G 1 size 6x6 odd fen [r*bqk*br*/p*p*p*p*p*p*/6/6/P*P*P*P*P*P*/R*NQK*NR*:0:1:w] moves (0T1)f2f4 submit (0T1)e5e4 submit (0T2)b2b3 submit (0T2)a5a3 submit (0T3)b1a3 submit (0T3)c5c3 submit (0T4)b3c4 submit (0T4)b6d4 submit (0T5)a3b5 submit (0T5)c6b5 submit (0T6)c4b5 submit (0T6)a6a5 submit (0T7)c1b2 submit (0T7)a5(0T3)a5 submit (-1T4)a3c4 submit (-1T4)d6(0T4)c5 submit").unwrap();
        let mut e = AlphaBeta::new();
        let stop = AtomicBool::new(false);
        let mut last = String::new();
        let a = e.find_best_move(&s, &Limits { depth: Some(2), ..Default::default() }, &stop, &mut |m| last = m).unwrap();
        let seq: MoveSeq = a.moves().iter().map(|m| m.fm).collect();
        assert_eq!(AlphaBeta::child(&s, &seq).get_mate_type(), MateType::Checkmate, "played {:?}", seq);
        assert!(last.contains("score mate 1 "), "{}", last);
    }

    /// Mate in one with 18 million legal actions in the position (the root is in check): the
    /// root's scan of every action finds it at once (Standard Turn Zero game, reported by a
    /// user as taking seconds).
    #[test]
    fn mate_in_one_among_millions_of_actions_is_instant() {
        let s = crate::core::perft::parse_g_line("G 1 size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:0:b] [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w] moves (0T1)g1f3 submit (0T1)g8f6 submit (0T2)g2g3 submit (0T2)b8c6 submit (0T3)f1g2 submit (0T3)e7e6 submit (0T4)e1g1 submit (0T4)f8b4 submit (0T5)a2a3 submit (0T5)b4(0T4)b5 submit (-1T5)b1c3 (0T6)b1c3 submit (-1T5)b5a6 submit (-1T6)d2d4 submit (-1T6)a6(0T6)b6 submit (-1T7)d4d5 (0T7)d2d4 submit (-1T7)f6d5 (0T7)e8(-1T7)e7 submit (-1T8)c3d5 (0T8)d4d5 submit (0T8)f6d5 (-1T8)f8d6 submit (-1T9)d5e3 (0T9)c3d5 submit (0T9)e6d5 (-1T9)d8f6 submit (-2T8)d5c6 (-1T10)c2c3 (0T10)c1f4 submit (0T10)d8f6 (-1T10)h7h5 submit (-1T11)f3d4 (0T11)f1e1 submit (0T11)c8(-1T11)b8 submit (-1T12)d4c6 (0T12)c2c3 submit (0T12)b6(-1T12)c6 submit (-1T13)d1d4 (0T13)e2e4 submit (0T13)d7d6 (-1T13)b7b6 submit (-1T14)d4f6 (0T14)e4d5 submit (0T14)a7a6 (-1T14)g7f6 submit (-1T15)e3c4 (0T15)d5c6 submit (0T15)b7c6 (-1T15)c8a6 submit (-1T16)c4d6 (0T16)f4g5 submit (0T16)f6f5 (-1T16)e8(-1T15)e7 submit (-3T16)c4d6 (-1T17)d6f7 (0T17)d1b1 submit (0T17)g7g6 (-1T17)h8g8 submit (-1T18)g2c6 (0T18)b1f5 submit (0T18)g6f5 (-1T18)d7c6 submit (-1T19)f7(0T19)f5 submit (0T19)a8b8 (-1T19)a6e2 submit (-1T20)f1e1 (0T20)e1e7 submit (0T20)b8b5 (-1T20)e2f3 submit (0T21)f5(-1T21)f3 submit (0T21)h8d8 (-1T21)e6e5 submit (0T22)g5(-1T22)h5 submit (0T22)b5b2 (-1T22)g8h8 submit (-1T23)h5(0T23)g5 submit (0T23)b2b5 (-1T23)h8h3 submit (0T24)g5(-1T24)g4 submit (0T24)d8c8 (-1T24)h3h2 submit (-1T25)f3h2 (0T25)e7f7 submit (-1T25)b8(0T25)b7 submit (-1T26)c1h6 (0T26)f7h7 submit (0T26)c8e8 (-1T26)a8h8 submit (-1T27)h6g7 (0T27)h7c7 submit (0T27)e8e2 (-1T27)h8h2 submit (-1T28)g1h2 (0T28)f3d4 submit (0T28)e2f2 (-1T28)c7(0T27)c7 submit (-1T29)g7f6 (0T29)g1f2 submit (0T29)b5b2 (-1T29)e5(0T29)e5 submit (-1T30)g4f3 (0T30)f2g1 submit (0T30)b2g2 (-1T30)b6(0T30)b6 submit (-1T31)f3c6 (0T31)g1g2 submit (0T31)b7(-1T31)c7 submit (-1T32)e1e7 (0T32)c7c6 submit (-1T32)c7(0T32)c6 submit").unwrap();
        let mut e = AlphaBeta::new();
        let stop = AtomicBool::new(false);
        let mut last = String::new();
        let t = Instant::now();
        let a = e.find_best_move(&s, &Limits { depth: Some(3), ..Default::default() }, &stop, &mut |m| last = m).unwrap();
        let seq: MoveSeq = a.moves().iter().map(|m| m.fm).collect();
        assert_eq!(AlphaBeta::child(&s, &seq).get_mate_type(), MateType::Checkmate, "played {:?}", seq);
        assert!(last.contains("score mate 1 "), "{}", last);
        assert!(t.elapsed() < Duration::from_millis(2000), "took {:?}", t.elapsed());
    }

    /// Turn Zero, Black's move 30: (-2T16)Na5xb3 loses to the branching knight travel
    /// (-2T17)Ng5>>x(-2T16)g7#, which must be found at interior nodes.
    #[test]
    fn checking_travel_reply_is_searched() {
        let s = start_with_moves(
            "(0T1)e2e3 submit (0T1)g8f6 submit (0T2)b2b4 submit (0T2)d7d5 submit (0T3)c1b2 submit (0T3)c8g4 submit \
             (0T4)g1f3 submit (0T4)b8c6 submit (0T5)b4b5 submit (0T5)c6a5 submit (0T6)h2h3 submit (0T6)g4h5 submit \
             (0T7)g2g4 submit (0T7)h5g6 submit (0T8)d2d3 submit (0T8)e7e6 submit (0T9)b1d2 submit (0T9)f8d6 submit \
             (0T10)d1e2 submit (0T10)d8d7 submit (0T11)a2a4 submit (0T11)e8g8 submit (0T12)h3h4 submit (0T12)a5c4 submit \
             (0T13)d3c4 submit (0T13)d5d4 submit (0T14)b2d4 submit (0T14)e6e5 submit (0T15)d4c3 submit (0T15)g6c2 submit \
             (0T16)f1h3 submit (0T16)e5e4 submit (0T17)c3f6 submit (0T17)g7f6 submit (0T18)f3d4 submit (0T18)d6e5 submit \
             (0T19)d4c2 submit (0T19)e5a1 submit (0T20)c2a1 submit (0T20)d7d6 submit (0T21)g4g5 submit (0T21)d6e5 submit \
             (0T22)e2d1 submit (0T22)e5(0T18)a1 submit (-1T19)d2b1 submit (-1T19)a1b1 submit (0T23)d1(0T11)d1 submit \
             (1T11)d7b5 submit (1T12)f3g5 submit (1T12)b5b2 submit (1T13)a1b1 submit (1T13)b2c3 submit (1T14)h3h4 submit \
             (1T14)e8f8 submit (1T15)h4h5 submit (1T15)g6h5 submit (1T16)g4h5 submit (1T16)a5(1T15)a3 submit \
             (-2T16)b1b3 submit",
        );
        let losing = FullMove::parse("(-2T16)a5b3").unwrap();
        let mut e = AlphaBeta::new();
        let stop = AtomicBool::new(false);
        let a = e.find_best_move(&s, &Limits { depth: Some(3), ..Default::default() }, &stop, &mut |_| {}).unwrap();
        let seq: MoveSeq = a.moves().iter().map(|m| m.fm).collect();
        assert!(seq.as_slice() != [losing], "played {:?}", seq);
    }

    /// "Focused - Just Unicorns" after 1.Ka1b2 Ke5d4 2.Kb2a1 Kd4c5.
    const UNICORNS_A: &str = "G 1 size 5x5 odd fen [1u1uk*/5/5/5/K*U1U1:0:1:w] moves (0T1)a1b2 submit (0T1)e5d4 submit \
         (0T2)b2a1 submit (0T2)d4c5 submit";

    /// `UNICORNS_A` then 3.(0T3)Ub1>>(0T1)d3 (1T1)Ke5d4 4.(1T2)Ud1>>(0T1)d2+: the unicorn on
    /// inactive L2 mates with (2T1)Ud5>>(0T1)b3#; a mated node at ply 3 must search optional
    /// boards.
    #[test]
    fn mated_node_searches_optional_boards() {
        let mut s = crate::core::perft::parse_g_line(&format!("{} (0T3)b1(0T1)d3 submit (1T1)e5d4 submit (1T2)d1(0T1)d2 submit", UNICORNS_A)).unwrap();
        s.enable_undo();
        let mut e = AlphaBeta::new();
        let stop = AtomicBool::new(false);
        e.begin_search(&stop, None, None);
        let v = e.negamax(&mut s, 4, 3, -INF, INF, true);
        assert_eq!(v, MATE - 4, "the mate in one from the inactive timeline");
        assert_eq!(e.pv_rows[3][0], MoveSeq::from(&[FullMove::parse("(2T1)d5(0T1)b3").unwrap()][..]));
    }

    /// The same game two moves earlier: 3.(0T3)Ub1>>(0T1)d3 was reported as a mate in 4
    /// through 4.(1T2)Ud1>>(0T1)d2+, refuted by that unicorn.
    #[test]
    fn no_mate_claim_refuted_on_an_optional_board() {
        let s = crate::core::perft::parse_g_line(UNICORNS_A).unwrap();
        let mut e = AlphaBeta::new();
        let stop = AtomicBool::new(false);
        let mut lines = Vec::new();
        e.find_best_move(&s, &Limits { depth: Some(10), ..Default::default() }, &stop, &mut |m| lines.push(m));
        assert!(!lines.iter().any(|l| l.contains("score mate ") && info_field(l, "pv") == "(0T3)b1(0T1)d3"), "{:#?}", lines);
    }

    /// "Focused - Just Unicorns", 1.Ka1b2 Ke5d4 2.Kb2c1 Kd4c5 3.Kc1b2 Kc5d4 4.Kb2c1
    /// (0T4)Ud5>>(0T1)a2+ 5.(-1T2)Kb2xa2 Ke5d4 6.Ka2b3 Kd4d3 7.(0T5)Ub1>>(-1T4)b2#: the root
    /// probe's mate is reported at every depth and by a resumed search.
    #[test]
    fn future_board_mate_is_reported_at_every_depth() {
        let s = crate::core::perft::parse_g_line(
            "G 1 size 5x5 odd fen [1u1uk*/5/5/5/K*U1U1:0:1:w] moves (0T1)a1b2 submit (0T1)e5d4 submit (0T2)b2c1 submit \
             (0T2)d4c5 submit (0T3)c1b2 submit (0T3)c5d4 submit (0T4)b2c1 submit (0T4)d5(0T1)a2 submit (-1T2)b2a2 submit \
             (-1T2)e5d4 submit (-1T3)a2b3 submit (-1T3)d4d3 submit",
        )
        .unwrap();
        let mut e = AlphaBeta::new();
        let stop = AtomicBool::new(false);
        for (depth, resume) in [(4, false), (6, true)] {
            let mut lines = Vec::new();
            let a = e.find_best_move(&s, &Limits { depth: Some(depth), resume, ..Default::default() }, &stop, &mut |m| lines.push(m)).unwrap();
            assert_eq!(a.moves().iter().map(|m| m.fm).collect::<MoveSeq>().as_slice(), [FullMove::parse("(0T5)b1(-1T4)b2").unwrap()]);
            assert!(!lines.is_empty() && lines.iter().all(|l| info_field(l, "score") == "mate 1 wdl 1000 0 0"), "{:#?}", lines);
        }
    }

    /// An even 8x8 game: Black mates in one only by (-1T15)Ne4>>(-1T13)e3, a travel creating a
    /// timeline, which comes after the 178 thousand actions creating none (the root takes
    /// 1000). The root probes such travels as interior nodes do (`travel_mate`).
    #[test]
    fn root_finds_a_timeline_creating_mate_in_one() {
        let s = crate::core::perft::parse_g_line(
            "G 1 size 8x8 even fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:-0:1:w]\
             [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:+0:1:w] moves (-1T1)g1f3 (0T1)e2e4 submit \
             (0T1)g8f6 (-1T1)e7e6 submit (-1T2)g2g3 (0T2)b1c3 submit (0T2)b8c6 (-1T2)b8c6 submit (-1T3)d2d4 (0T3)g1f3 submit \
             (0T3)d7d6 (-1T3)f8b4 submit (-1T4)c2c3 (0T4)d1e2 submit (0T4)c8g4 (-1T4)b4f8 submit (-1T5)b1d2 (0T5)h2h3 submit \
             (0T5)g4h5 (-1T5)g8f6 submit (-1T6)d1c2 (0T6)a2a3 submit (0T6)d8c8 (-1T6)f6g4 submit (-1T7)d2e4 (0T7)e2e3 submit \
             (0T7)c8d7 (-1T7)d7d5 submit (-1T8)c1g5 (0T8)g2g4 submit (0T8)h5(-1T8)g5 submit (-1T9)e4g5 (0T9)e3g5 submit \
             (0T9)c6e5 (-1T9)d8f6 submit (-1T10)f1h3 (0T10)f3e5 submit (0T10)d6e5 (-1T10)g4f2 submit (-1T11)g5e6 (0T11)f1c4 \
             submit (0T11)e7e6 (-1T11)c8e6 submit (-1T12)h3e6 (0T12)g5e5 submit (0T12)f8d6 (-1T12)f7e6 submit (-1T13)h1f1 \
             (0T13)e5b5 submit (0T13)e8g8 (-1T13)f2e4 submit (-1T14)f3d2 (0T14)b5d7 submit (0T14)f6d7 (-1T14)f6f1 submit \
             (0T15)e1(0T14)f1 submit (1T14)d7b5 submit (-1T15)d2f1 (1T15)c4b5 submit",
        )
        .unwrap();
        let mut e = AlphaBeta::new();
        let stop = AtomicBool::new(false);
        let a = e.find_best_move(&s, &Limits { depth: Some(2), ..Default::default() }, &stop, &mut |_| {}).unwrap();
        let seq: MoveSeq = a.moves().iter().map(|m| m.fm).collect();
        assert_eq!(AlphaBeta::child(&s, &seq).get_mate_type(), MateType::Checkmate, "played {:?}", seq);
        assert_eq!(e.last_score(), MATE - 1);
    }

    /// Turn Zero game, after 15...Qf6>x(1T10)f3: 16. (1T11)Bg5>>x(1T9)e5* creates a timeline and
    /// moves the present back (an action alone, after every action creating none) and mates in 2
    /// with 17. (-3T9)Qg4d7#. The root searches such checking travels.
    #[test]
    fn checking_travel_starts_a_mate_in_two() {
        let s = crate::core::perft::parse_g_line(
            "G 1 size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:0:b] [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w] moves \
             (0T1)g1f3 submit (0T1)g8f6 submit (0T2)d2d4 submit (0T2)d7d5 submit (0T3)c2c3 submit \
             (0T3)c7c6 submit (0T4)c1g5 submit (0T4)c8g4 submit (0T5)e2e3 submit (0T5)f6e4 submit \
             (0T6)h2h3 submit (0T6)g4f3 submit (0T7)d1f3 submit (0T7)e4g5 submit (0T8)f3f4 submit \
             (0T8)e7e5 submit (0T9)f4(0T6)f7 submit (1T6)e8f7 submit (1T7)f3e5 submit (1T7)f7e8 submit \
             (1T8)d1g4 submit (0T9)e5(1T8)e5 submit (1T9)g4h5 submit (1T9)g7g6 submit \
             (0T10)a2a4 (1T10)h5f3 submit (0T10)d8(0T9)e7 submit (-1T10)h3h4 submit (-1T10)e7(0T9)f6 submit \
             (-2T10)d4e5 submit (-2T10)f6(1T10)f3 submit",
        )
        .unwrap();
        let mut e = AlphaBeta::new();
        let stop = AtomicBool::new(false);
        let a = e.find_best_move(&s, &Limits { depth: Some(3), ..Default::default() }, &stop, &mut |_| {}).unwrap();
        let travel = FullMove::parse("(1T11)g5(1T9)e5").unwrap();
        assert!(a.moves().iter().any(|m| m.fm == travel), "played {}", a);
        assert_eq!(e.last_score(), MATE - 3);
    }
}
