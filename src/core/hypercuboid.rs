//! Hypercuboid legal-action search. An axis is a playable timeline or a prospective new timeline; a point picks one
//! semimove per axis; invalid regions are cut out as slices.

use super::action::{FullMove, MoveSeq};
use super::bitboard::*;
use super::board::{Board, RoyalSafety};
use super::check_position::{CheckBuf, CheckPosition};
use super::multiverse::{next_turn, MoveList};
use super::piece::*;
use super::promotion::PromotionOptions;
use super::state::{promotion_rank, State};
use super::vec4::Vec4;
use crate::misc::geometry::{Hc, Point, SearchSpace, Slice};
use crate::misc::graph::Graph;
use crate::misc::integer_set::{Index, IntegerSet};
use crate::misc::ordering::{AxisOrders, HcOrdering};

const INVALID: Index = Index::MAX;

#[derive(Clone, Copy, Debug)]
/// Moves carry their white-normalized moving and captured piece kinds.
pub enum Entry {
    /// `promo`: the piece a promoting pawn becomes when not the position's default
    /// (`NO_PIECE` otherwise); each allowed piece is its own entry.
    Physical { m: FullMove, b: Board, mover: Piece, victim: Piece, promo: Piece },
    /// `idx` is the paired departure coordinate on axis `fa`.
    Arriving { m: FullMove, b: Board, idx: Index, fa: u32, mover: Piece, victim: Piece },
    Departing { from: Vec4, b: Board },
    Null,
}

impl Entry {
    #[inline(always)]
    pub fn is_null(&self) -> bool {
        matches!(self, Entry::Null)
    }
    #[inline(always)]
    pub fn board(&self) -> &Board {
        match self {
            Entry::Physical { b, .. } | Entry::Arriving { b, .. } | Entry::Departing { b, .. } => b,
            Entry::Null => panic!("cannot extract a board from a null HC entry"),
        }
    }
    /// (t, l) of the hotspot: source for physical/departing, destination for arriving.
    #[inline(always)]
    pub fn tl(&self) -> (i32, i32) {
        let p = match self {
            Entry::Physical { m, .. } => m.from,
            Entry::Arriving { m, .. } => m.to,
            Entry::Departing { from, .. } => *from,
            Entry::Null => panic!("a null semimove has no hotspot"),
        };
        (p.t(), p.l())
    }
}

/// Public semantic view of an axis coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Semimove {
    Physical(FullMove),
    Arriving(FullMove),
    Departing(Vec4),
    Null,
}

pub struct MoveBoards<'a> {
    pub mv: FullMove,
    pub result: &'a Board,
    pub departure: Option<&'a Board>,
}

/// Hypercuboid search context. Independent of the state it was built from (which callers
/// pass back in where needed), so a search can run while the state is made/unmade. Every
/// buffer is kept when it is rebuilt for another state (`rebuild`).
#[derive(Default)]
pub struct HcInfo {
    player: bool,
    /// (line, axis) of every axis, ascending by line.
    line_axes: Vec<(i32, usize)>,
    /// Axis of line `axis_lo + k` at `axis_of[k]` (`usize::MAX`: none).
    axis_lo: i32,
    axis_of: Vec<usize>,
    /// Entries of the existing playable timelines' axes (the first `new_axis` are in use).
    axis_coords: Vec<Vec<Entry>>,
    /// Entries shared by every prospective new-timeline axis: the null one, then (once
    /// `materialize_branch` has run) one per arrival of `branch_moves`.
    branch_entries: Vec<Entry>,
    /// The branch axes' arrivals: (move, departure coordinate, departure axis, moving piece).
    branch_moves: Vec<(FullMove, Index, u32, Piece)>,
    /// Per existing axis: the branch coordinates landing on its timeline's last board (built
    /// with the branch entries).
    branch_onto: Vec<IntegerSet>,
    branch_ready: bool,
    dim: usize,
    universe: Hc,
    new_axis: usize,
    /// Coordinate groups per axis (index `new_axis` is shared by all branch axes).
    groups: Vec<AxisGroups>,
    scratch: BuildScratch,
    /// Every timeline ends on the mover's turn, so after a point only its moved timelines
    /// can give check.
    all_lines_mine: bool,
    /// Per existing axis: (board key, generation) of its physical entries (see `phys_tag`).
    phys_tags: Vec<(u64, u64)>,
    /// Per existing axis (filled on demand, cleared by `rebuild`): every superphysical check
    /// from that timeline's next board, in scan order, when it is the only moved timeline
    /// and still holds all of the opponent's pieces.
    lone_checks: std::cell::RefCell<Vec<(bool, Vec<FullMove>)>>,
    /// The same for points moving several timelines (filled on demand, cleared by `rebuild`).
    env_checks: std::cell::RefCell<EnvChecks>,
}

/// See `HcInfo::moves_may_check`.
#[derive(Default)]
struct EnvChecks {
    boards_known: bool,
    /// Per existing axis: its timeline's next board at its worst.
    boards: Vec<Board>,
    /// Per existing axis: (known, the checks from its timeline's next board).
    checks: Vec<(bool, Vec<FullMove>)>,
    /// The timelines that may give check after the last point tested (ascending).
    maybe: Vec<i32>,
}

/// See `HcInfo::lone_move_may_check`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MayCheck {
    NoCheck,
    All,
    Lines,
}

/// One jump of a build.
struct Jump {
    m: FullMove,
    mover: Piece,
    royal: bool,
    /// Whether its result leaves the mover's royals unattacked: 0 not known yet, 1 yes, 2 no.
    ok: u8,
}

impl Jump {
    #[inline]
    fn safe(&mut self, memo: &mut SafetyMemo) -> bool {
        if self.ok == 0 {
            self.ok = if memo.arrival_safe(self.m, self.royal) { 1 } else { 2 };
        }
        self.ok == 1
    }
}

#[derive(Default)]
struct BuildScratch {
    playable: Vec<i32>,
    departures: Vec<Vec<Vec4>>,
    /// Every jump, by destination line (from the lowest line), in generation order.
    arrivals: Vec<Vec<Jump>>,
    ml: MoveList,
    /// Departure coordinate of each source square, per axis.
    jump_tab: Vec<[Index; 64]>,
    /// (build stamp, per-board facts lookups: see `SafetyMemo`).
    safety: (u32, Vec<(u32, u32, u32)>),
    /// Per axis: its first departure or arrival coordinate (the null and physical ones
    /// come first).
    first_jump: Vec<usize>,
    /// (axis, coordinate) of the existing-axis arrivals whose departure was filtered out.
    orphans: Vec<(usize, Index)>,
    group_slots: GroupSlots,
}

/// Per-thread cache, keyed by board, of one board's royal-safety facts for a defender: check,
/// pins, and per square whether a departure (`dep_*`) or an own arrival (`known`/`safe`,
/// indexed by royalty) leaves the royals unattacked.
struct BoardFacts {
    board: Board,
    defender: bool,
    /// Fill count of the cache when this slot was last filled.
    fill: u32,
    in_check: bool,
    pinned: Option<Bitboard>,
    dep_known: Bitboard,
    dep_safe: Bitboard,
    known: [Bitboard; 2],
    safe: [Bitboard; 2],
}

#[derive(Default)]
struct FactsCache {
    /// `FACTS_SLOTS` slots, by board key.
    slots: Vec<Option<BoardFacts>>,
    fills: u32,
}

const FACTS_SLOTS: usize = 1024;

thread_local! {
    static FACTS_CACHE: std::cell::RefCell<FactsCache> = std::cell::RefCell::new(FactsCache::default());
}

impl FactsCache {
    /// The facts of board `b` (whose `board_key` is `key`) for `defender`: (slot, facts).
    #[inline]
    fn lookup(&mut self, b: &Board, key: u64, defender: bool) -> (usize, &mut BoardFacts) {
        if self.slots.is_empty() {
            self.slots.resize_with(FACTS_SLOTS, || None);
        }
        let i = key as usize % FACTS_SLOTS;
        let fills = &mut self.fills;
        let slot = &mut self.slots[i];
        match slot {
            Some(f) if f.defender == defender && f.board == *b => {}
            _ => {
                *fills = fills.wrapping_add(1);
                *slot = Some(BoardFacts {
                    board: *b,
                    defender,
                    fill: *fills,
                    in_check: has_physical_check(b, defender),
                    pinned: None,
                    dep_known: 0,
                    dep_safe: 0,
                    known: [0; 2],
                    safe: [0; 2],
                });
            }
        }
        (i, slot.as_mut().unwrap())
    }
}

/// The facts of the mover's boards, looked up once per build by (line, turn) (entries
/// stamped with the build).
struct SafetyMemo<'a> {
    s: &'a State,
    player: bool,
    lines_lo: i32,
    nt: usize,
    stamp: u32,
    /// (stamp, `FactsCache` slot, its fill count when looked up): a slot refilled since is
    /// looked up again.
    tab: &'a mut Vec<(u32, u32, u32)>,
    facts: &'a mut FactsCache,
}

impl SafetyMemo<'_> {
    /// The facts of the mover's board (l, t).
    #[inline]
    fn facts(&mut self, l: i32, t: i32) -> &mut BoardFacts {
        let (s, player) = (self.s, self.player);
        let e = &mut self.tab[(l - self.lines_lo) as usize * self.nt + t as usize];
        if e.0 == self.stamp && matches!(&self.facts.slots[e.1 as usize], Some(f) if f.fill == e.2) {
            return self.facts.slots[e.1 as usize].as_mut().unwrap();
        }
        let (i, f) = self.facts.lookup(s.board(l, t, player), s.multiverse().key_of(l, t, player), player);
        *e = (self.stamp, i as u32, f.fill);
        f
    }
    /// Royal safety of the mover's board (l, t).
    #[inline]
    fn get(&mut self, l: i32, t: i32) -> RoyalSafety {
        let player = self.player;
        let f = self.facts(l, t);
        let pinned = *f.pinned.get_or_insert_with(|| f.board.royal_safety(player).pinned);
        RoyalSafety { in_check: f.in_check, pinned }
    }
    /// Whether the mover's piece on `p` (its timeline's last board) leaving it leaves its
    /// royals there unattacked.
    #[inline]
    fn departure_safe(&mut self, p: Vec4) -> bool {
        let player = self.player;
        let rs = self.get(p.l(), p.t());
        let f = self.facts(p.l(), p.t());
        let z = pmask(p.xy());
        // Only a royal, a pinned piece or a board already in check can expose a royal.
        if !rs.in_check && (f.board.royal() | rs.pinned) & z == 0 {
            return true;
        }
        if f.dep_known & z == 0 {
            let mut nb = f.board;
            nb.clear_square(p.xy());
            f.dep_known |= z;
            if !has_physical_check(&nb, player) {
                f.dep_safe |= z;
            }
        }
        f.dep_safe & z != 0
    }
    /// Whether a piece of the mover arriving with the jump `m` (royal or not) leaves its
    /// royals unattacked.
    #[inline]
    fn arrival_safe(&mut self, m: FullMove, royal: bool) -> bool {
        let s = self.s;
        let player = self.player;
        let q = m.to;
        let f = self.facts(q.l(), q.t());
        if !f.in_check && !royal {
            return true;
        }
        let z = pmask(q.xy());
        let r = royal as usize;
        if f.known[r] & z == 0 {
            let ok = if f.in_check {
                !has_physical_check(&arrival_board(s, m), player)
            } else {
                // The target's own royals stay covered (the destination stays occupied):
                // only the arriving royal can be attacked.
                !f.board.is_under_attack(q.xy(), player)
            };
            f.known[r] |= z;
            if ok {
                f.safe[r] |= z;
            }
        }
        f.safe[r] & z != 0
    }
}

/// Per-thread cache, keyed by board, of a board's physical entries: its safe on-board moves
/// in generation order (the previous board is part of the key for en passant).
struct PhysSlot {
    lt: (i32, i32),
    player: bool,
    board: Board,
    prev: Option<Board>,
    entries: Vec<Entry>,
    /// Unique to this content (see `HcInfo::phys_tag`).
    generation: u64,
}

/// Generations of `PhysSlot` contents, unique across threads.
static PHYS_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Default)]
struct PhysCache {
    /// Board size and promotion rules the slots were generated under.
    ctx: Option<((i32, i32), PromotionOptions)>,
    /// `PHYS_SLOTS` slots, by board key.
    slots: Vec<Option<PhysSlot>>,
}

const PHYS_SLOTS: usize = 256;

thread_local! {
    static PHYS_CACHE: std::cell::RefCell<PhysCache> = std::cell::RefCell::new(PhysCache::default());
}

#[derive(Default)]
struct AxisGroups {
    nonjump: IntegerSet,
    /// (departure axis, departure coordinate, arrivals depending on it); the first
    /// `dep_len` are in use (the rest keep their storage).
    by_dep: Vec<(u32, Index, IntegerSet)>,
    dep_len: usize,
    /// (departure axis, all arrivals from it); the first `from_len` are in use.
    by_from: Vec<(u32, IntegerSet)>,
    from_len: usize,
}

/// Position of each departure's (and departure axis') group while one `AxisGroups` is
/// built; entries are valid when stamped with the current build.
#[derive(Default)]
struct GroupSlots {
    stamp: u32,
    /// By departure axis * 64 + departure square.
    dep: Vec<(u32, u32)>,
    /// By departure axis.
    from: Vec<(u32, u32)>,
}

impl GroupSlots {
    fn begin(&mut self, axes: usize) {
        if self.dep.len() < axes * 64 {
            self.dep.resize(axes * 64, (0, 0));
            self.from.resize(axes, (0, 0));
        }
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.dep.iter_mut().for_each(|e| e.0 = 0);
            self.from.iter_mut().for_each(|e| e.0 = 0);
            self.stamp = 1;
        }
    }
}

impl AxisGroups {
    /// Group `entries`, the null and physical ones first (`first_jump` of them), then the
    /// departures and arrivals (which come from `axes` departure axes).
    fn rebuild(&mut self, entries: &[Entry], first_jump: usize, axes: usize, slots: &mut GroupSlots) {
        debug_assert!(entries[..first_jump].iter().all(|e| matches!(e, Entry::Null | Entry::Physical { .. })));
        self.nonjump.clear();
        self.nonjump.insert_below(first_jump as Index);
        self.dep_len = 0;
        self.from_len = 0;
        slots.begin(axes);
        let stamp = slots.stamp;
        for (i, e) in entries.iter().enumerate().skip(first_jump) {
            let i = i as Index;
            match e {
                Entry::Physical { .. } | Entry::Null => unreachable!("non-jump entry after the jumps"),
                Entry::Arriving { m, idx, fa, .. } if *idx != INVALID => {
                    let (fa, idx) = (*fa, *idx);
                    // (departure axis, departure square) identifies the departure coordinate.
                    let slot = &mut slots.dep[fa as usize * 64 + m.from.xy() as usize];
                    if slot.0 != stamp {
                        *slot = (stamp, self.dep_len as u32);
                        if self.dep_len == self.by_dep.len() {
                            self.by_dep.push((fa, idx, IntegerSet::new()));
                        } else {
                            let e = &mut self.by_dep[self.dep_len];
                            (e.0, e.1) = (fa, idx);
                            e.2.clear();
                        }
                        self.dep_len += 1;
                    }
                    self.by_dep[slot.1 as usize].2.insert(i);
                    let slot = &mut slots.from[fa as usize];
                    if slot.0 != stamp {
                        *slot = (stamp, self.from_len as u32);
                        if self.from_len == self.by_from.len() {
                            self.by_from.push((fa, IntegerSet::new()));
                        } else {
                            let e = &mut self.by_from[self.from_len];
                            e.0 = fa;
                            e.1.clear();
                        }
                        self.from_len += 1;
                    }
                    self.by_from[slot.1 as usize].1.insert(i);
                }
                _ => {}
            }
        }
    }
}

/// The present after a point (see `HcInfo::present_after`): the number of new timelines,
/// the active range, and the timeline holding the present (the first active one ending
/// earliest) with its end (a half-turn).
#[derive(Clone, Copy)]
struct PresentAfter {
    k: usize,
    amin: i32,
    amax: i32,
    w_line: i32,
    w_v: i32,
}

/// Reusable buffers for `HcInfo::take_point`; the point taken is left in `point`.
#[derive(Default)]
pub struct PointScratch {
    g: Graph,
    must_include: Vec<Index>,
    edge_refs: Vec<Index>,
    want: Vec<bool>,
    pub point: Point,
}

/// The board the jump `m` of the side to move creates on its target: the target board with
/// the piece (or the piece a brawn promotes to) on the destination square.
fn arrival_board(s: &State, m: FullMove) -> Board {
    let player = s.get_present().1;
    let (p, q) = (m.from, m.to);
    let src = s.board(p.l(), p.t(), player);
    let mut nb = *s.board(q.l(), q.t(), player);
    let (_, size_y) = s.get_board_size();
    if src.lrawn() & pmask(p.xy()) != 0 && q.y() == promotion_rank(size_y, player) {
        let promotion =
            s.normalize_promotion(super::action::ExtMove::new(m, NO_PIECE)).map(|e| e.promote_to).unwrap_or(NO_PIECE);
        if promotion != NO_PIECE {
            nb.set_piece(q.xy(), if player { to_black(promotion) } else { promotion });
            return nb;
        }
    }
    nb.place_from(q.xy(), src, p.xy());
    nb
}

/// Indices of wasted moves in an action just played on `after` (`branching(i)`: move `i`
/// created a timeline): non-branching moves whose boards all stay out of the present, other
/// than ones a branch lands on. Same rule as `HcInfo::wasted_problem`.
pub fn wasted_moves(after: &State, mvs: &[FullMove], branching: impl Fn(usize) -> bool) -> Vec<usize> {
    if !(0..mvs.len()).any(&branching) {
        return Vec::new();
    }
    let (t, c) = after.apparent_present();
    let present = 2 * t + c as i32;
    let (amin, amax) = after.get_active_range();
    let out = |l: i32| l < amin || l > amax || after.multiverse().timeline_end_v(l) > present;
    let landed = |b: Vec4| mvs.iter().enumerate().any(|(j, m)| branching(j) && m.to.tl() == b);
    (0..mvs.len())
        .filter(|&i| {
            let m = mvs[i];
            !branching(i) && out(m.to.l()) && out(m.from.l()) && !landed(m.to.tl()) && !landed(m.from.tl())
        })
        .collect()
}

/// Whether `wasted_moves` finds a wasted move.
pub fn plays_wasted_move(after: &State, mvs: &[FullMove], branching: impl Fn(usize) -> bool) -> bool {
    !wasted_moves(after, mvs, branching).is_empty()
}

/// True if a royal piece of `defender`'s color is attacked on this board.
pub fn has_physical_check(position: &Board, defender: bool) -> bool {
    let defending = if defender { position.black() } else { position.white() };
    for sq in marked_pos(position.royal() & defending) {
        if position.is_under_attack(sq, defender) {
            return true;
        }
    }
    false
}

/// Append the physical entries of the mover's board `source` at `p0` (royal safety `rs`) to
/// `locs`: each move staying on the board that leaves no royal of the mover attacked, pieces
/// in square order.
fn gen_physical_entries(s: &State, source: &Board, p0: Vec4, player: bool, rs: RoyalSafety, size_y: i32, locs: &mut Vec<Entry>) {
    let own_royals = source.royal() & source.hostile(!player);
    for pos in marked_pos(source.friendly(player) & !source.wall()) {
        let from = Vec4::from_xy(pos, p0);
        let bb = s.multiverse().gen_physical_moves(from, player);
        let z = pmask(pos);
        let mover = source.kind(pos);
        let pawn = source.lpawn() & z != 0;
        let king = source.king() & z != 0;
        let tested = rs.in_check || (source.royal() | rs.pinned) & z != 0;
        // The mover's only royal moving (not castling): only its destination can be
        // attacked, with its origin vacated.
        let lone_royal = own_royals == z;
        for sq in marked_pos(bb) {
            let q = Vec4::from_xy(sq, p0);
            let m = FullMove::new(from, q);
            let dx = q.x() - from.x();
            let castle = king && dx.abs() > 1;
            let mut nb = *source;
            let mut needs_test = tested;
            if pawn && dx != 0 && source.occupied() & pmask(sq) == 0 {
                nb.clear_square(ppos(q.x(), from.y()));
                nb.transfer(pos, sq);
                needs_test = true;
            } else if castle {
                let rook_x1 = source.first_occupied_x(from.x(), from.y(), dx.signum());
                let rook_x2 = from.x() + dx.signum();
                nb.transfer(ppos(rook_x1, from.y()), ppos(rook_x2, q.y()));
                nb.transfer(pos, sq);
            } else {
                nb.transfer(pos, sq);
            }
            // The pieces a promotion may give: the default, then the others the position
            // allows (custom positions), each its own entry.
            let mut promos = [NO_PIECE; 8];
            let mut n_promos = 1;
            if pawn && q.y() == promotion_rank(size_y, player) {
                let promotion = s
                    .normalize_promotion(super::action::ExtMove::new(m, NO_PIECE))
                    .map(|e| e.promote_to)
                    .unwrap_or(NO_PIECE);
                if promotion != NO_PIECE {
                    nb.set_piece(sq, if player { to_black(promotion) } else { promotion });
                    for p in super::promotion::allowed_promotions(s.get_promotion_options()) {
                        if p != promotion && n_promos < promos.len() {
                            promos[n_promos] = p;
                            n_promos += 1;
                        }
                    }
                }
            }
            let safe = if !needs_test {
                true
            } else if lone_royal && !castle {
                !source.is_attacked_through(sq, player, source.occupied() & !z)
            } else {
                !has_physical_check(&nb, player)
            };
            if safe {
                let victim = if mover == KING_W && dx.abs() > 1 {
                    NO_PIECE
                } else {
                    let v = source.kind(sq);
                    if v == NO_PIECE && (mover == PAWN_W || mover == BRAWN_W) && dx != 0 { source.kind(ppos(q.x(), from.y())) } else { v }
                };
                locs.push(Entry::Physical { m, b: nb, mover, victim, promo: NO_PIECE });
                // (The promoted piece's kind cannot change the mover's own royal safety.)
                for &p in &promos[1..n_promos] {
                    let mut b = nb;
                    b.set_piece(sq, if player { to_black(p) } else { p });
                    locs.push(Entry::Physical { m, b, mover, victim, promo: p });
                }
            }
        }
    }
}

fn signum(x: i32) -> i32 {
    (x > 0) as i32 - (x < 0) as i32
}

impl HcInfo {
    #[inline(always)]
    pub fn get_universe(&self) -> &Hc {
        &self.universe
    }
    #[inline(always)]
    pub fn dimension(&self) -> usize {
        self.dim
    }
    #[inline(always)]
    pub fn player(&self) -> bool {
        self.player
    }
    #[inline(always)]
    pub fn axis_len(&self, n: usize) -> usize {
        if n < self.new_axis { self.axis_coords[n].len() } else { self.branch_moves.len() + 1 }
    }
    /// (board key, generation) of existing axis `n`'s physical entries (coordinates 1 up to
    /// `first_jump(n)`): equal tags mean the same entries, in the same order, so whatever is
    /// worked out from them alone can be kept by tag.
    #[inline]
    pub fn phys_tag(&self, n: usize) -> (u64, u64) {
        self.phys_tags[n]
    }

    /// Coordinates of existing axis `n` before its first departure or arrival (the null one and
    /// the physical moves).
    #[inline]
    pub fn first_jump(&self, n: usize) -> usize {
        self.scratch.first_jump[n]
    }

    #[inline(always)]
    pub fn first_branch_axis(&self) -> usize {
        self.new_axis
    }
    #[inline(always)]
    pub fn line_to_axis(&self, l: i32) -> Option<usize> {
        match self.axis_of.get(l.wrapping_sub(self.axis_lo) as u32 as usize) {
            Some(&a) if a != usize::MAX => Some(a),
            _ => None,
        }
    }
    #[inline(always)]
    pub fn entry(&self, n: usize, i: Index) -> &Entry {
        if n < self.new_axis {
            &self.axis_coords[n][i as usize]
        } else {
            debug_assert!(i == 0 || self.branch_ready, "branch entries not built");
            &self.branch_entries[i as usize]
        }
    }

    /// Whether the branch axes' arrival entries are built (see `rebuild`).
    #[inline(always)]
    pub fn branch_ready(&self) -> bool {
        self.branch_ready
    }

    /// Whether working on `hc` needs the branch entries and they are not built yet.
    #[inline]
    pub fn needs_branch(&self, hc: &Hc) -> bool {
        !self.branch_ready && (self.new_axis..hc.dimension()).any(|n| hc[n].has_other_than(0))
    }

    /// The arrival (move and moving piece) of branch coordinate `i`, built or not.
    pub fn branch_arrival(&self, i: Index) -> Option<(FullMove, Piece)> {
        let &(m, _, _, mover) = self.branch_moves.get((i as usize).checked_sub(1)?)?;
        Some((m, mover))
    }
    /// The arrival move, departure coordinate and departure axis of branch coordinate `i`,
    /// built or not.
    pub fn branch_departure(&self, i: Index) -> Option<(FullMove, Index, usize)> {
        let &(m, dep, fa, _) = self.branch_moves.get((i as usize).checked_sub(1)?)?;
        Some((m, dep, fa as usize))
    }
    pub fn get_semimove(&self, n: usize, i: Index) -> Semimove {
        match self.entry(n, i) {
            Entry::Physical { m, .. } => Semimove::Physical(*m),
            Entry::Arriving { m, .. } => Semimove::Arriving(*m),
            Entry::Departing { from, .. } => Semimove::Departing(*from),
            Entry::Null => Semimove::Null,
        }
    }
    pub fn arrival_move(&self, n: usize, i: Index) -> Option<FullMove> {
        match self.entry(n, i) {
            Entry::Arriving { m, .. } => Some(*m),
            _ => None,
        }
    }

    pub fn get_move_boards(&self, axis: usize, coord: Index) -> Option<MoveBoards<'_>> {
        match self.entry(axis, coord) {
            Entry::Physical { m, b, .. } => Some(MoveBoards { mv: *m, result: b, departure: None }),
            Entry::Arriving { m, b, idx, .. } => {
                let src = self.line_to_axis(m.from.l()).expect("departure axis");
                let dep = match self.entry(src, *idx) {
                    Entry::Departing { b, .. } => b,
                    _ => panic!("arrival without departure"),
                };
                Some(MoveBoards { mv: *m, result: b, departure: Some(dep) })
            }
            _ => None,
        }
    }

    pub fn build_hc(s: &State) -> (HcInfo, SearchSpace) {
        let mut info = HcInfo::default();
        let ss = info.rebuild(s, false);
        (info, ss)
    }

    /// Rebuild the search context for `s` in place and return the initial search space.
    /// With `defer_branch`, branch-axis arrival entries wait for `materialize_branch`.
    pub fn rebuild(&mut self, s: &State, defer_branch: bool) -> SearchSpace {
        let _prof = crate::misc::prof::scope(0);
        let _prof = crate::misc::prof::scope(15);
        let HcInfo {
            player: info_player,
            line_axes,
            axis_lo,
            axis_of,
            axis_coords,
            branch_entries,
            branch_moves,
            branch_onto: _,
            branch_ready,
            dim,
            universe,
            new_axis,
            groups,
            scratch,
            all_lines_mine,
            lone_checks,
            env_checks,
            phys_tags,
        } = self;
        let BuildScratch {
            playable,
            departures,
            arrivals,
            ml,
            jump_tab,
            safety,
            first_jump,
            orphans,
            group_slots,
        } = scratch;
        let present = s.get_present();
        let player = present.1;
        let (lines_lo, lines_hi) = s.get_lines_range();
        let (amin, amax) = s.get_active_range();
        // Playable timelines: the mandatory ones, then the optional ones.
        playable.clear();
        for l in lines_lo..=lines_hi {
            if amin <= l && l <= amax && s.get_timeline_end(l) == present {
                playable.push(l);
            }
        }
        for l in lines_lo..=lines_hi {
            let tc = s.get_timeline_end(l);
            if !(amin <= l && l <= amax && tc == present) && tc.1 == player {
                playable.push(l);
            }
        }
        let np = playable.len();
        // Axis of each line (existing playable timelines and the new ones branches create).
        *axis_lo = lines_lo - np as i32;
        axis_of.clear();
        axis_of.resize((lines_hi - lines_lo + 1) as usize + 2 * np, usize::MAX);
        for (axis, &l) in playable.iter().enumerate() {
            axis_of[(l - *axis_lo) as usize] = axis;
        }
        let axis_lo = *axis_lo;
        let axis_of_line = |axis_of: &[usize], l: i32| axis_of[(l - axis_lo) as usize];
        if departures.len() < np {
            departures.resize_with(np, Vec::new);
        }
        if axis_coords.len() < np {
            axis_coords.resize_with(np, Vec::new);
        }
        let nl = (lines_hi - lines_lo + 1) as usize;
        if arrivals.len() < nl {
            arrivals.resize_with(nl, Vec::new);
        }
        arrivals[..nl].iter_mut().for_each(|a| a.clear());
        let (_, size_y) = s.get_board_size();
        // Royal safety per board, memoized by (line, turn) and stamped with this build. Only
        // royal moves, pins, en passant or an existing check need the full check test.
        let nt = (lines_lo..=lines_hi).map(|l| s.get_timeline_end(l).0).max().unwrap_or(0) as usize + 1;
        let need = (lines_hi - lines_lo + 1) as usize * nt;
        if safety.1.len() < need {
            safety.1.resize(need, (0, 0, 0));
        }
        safety.0 = safety.0.wrapping_add(1);
        if safety.0 == 0 {
            safety.1.iter_mut().for_each(|e| e.0 = 0);
            safety.0 = 1;
        }
        let stamp = safety.0;
        let mut facts = FACTS_CACHE.with(|c| std::mem::take(&mut *c.borrow_mut()));
        let mut memo = SafetyMemo { s, player, lines_lo, nt, stamp, tab: &mut safety.1, facts: &mut facts };
        drop(());
        let _prof = crate::misc::prof::scope(1);
        // Every piece's moves: the physical ones become entries (those that leave no royal of
        // the mover attacked; a board played before reuses its entries), the jumps are
        // collected.
        let mut cache = PHYS_CACHE.with(|c| std::mem::take(&mut *c.borrow_mut()));
        let ctx = Some((s.get_board_size(), s.get_promotion_options()));
        if cache.ctx != ctx {
            cache.ctx = ctx;
            cache.slots.clear();
            cache.slots.resize_with(PHYS_SLOTS, || None);
        }
        phys_tags.clear();
        for (axis, &l) in playable.iter().enumerate() {
            departures[axis].clear();
            let t = s.get_timeline_end(l).0;
            let p0 = Vec4::new(0, 0, t, l);
            let source = s.board(l, t, player);
            let prev = s.multiverse().previous_board(l, t, player).copied();
            let locs = &mut axis_coords[axis];
            locs.clear();
            locs.push(Entry::Null);
            let _prof = crate::misc::prof::scope(18);
            let key = s.multiverse().key_of(l, t, player);
            let slot = &mut cache.slots[key as usize % PHYS_SLOTS];
            match slot {
                Some(e) if e.lt == (l, t) && e.player == player && e.board == *source && e.prev == prev => {
                    locs.extend_from_slice(&e.entries);
                    phys_tags.push((key, e.generation));
                }
                _ => {
                    gen_physical_entries(s, source, p0, player, memo.get(l, t), size_y, locs);
                    let generation = PHYS_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let e = slot.get_or_insert_with(|| PhysSlot { lt: (l, t), player, board: *source, prev, entries: Vec::new(), generation });
                    (e.lt, e.player, e.board, e.prev, e.generation) = ((l, t), player, *source, prev, generation);
                    e.entries.clear();
                    e.entries.extend_from_slice(&locs[1..]);
                    phys_tags.push((key, generation));
                }
            }
            drop(());
            let _prof = crate::misc::prof::scope(19);
            for pos in marked_pos(source.friendly(player) & !source.wall()) {
                let from = Vec4::from_xy(pos, p0);
                ml.clear();
                s.multiverse().gen_superphysical_moves(from, player, ml);
                if ml.is_empty() {
                    continue;
                }
                departures[axis].push(from);
                let mover = source.kind(pos);
                let royal = source.royal() & pmask(pos) != 0;
                for &(r, bb) in ml.iter() {
                    let jumps = &mut arrivals[(r.l() - lines_lo) as usize];
                    for sq in marked_pos(bb) {
                        jumps.push(Jump { m: FullMove::new(from, Vec4::from_xy(sq, r)), mover, royal, ok: 0 });
                    }
                }
            }
        }
        PHYS_CACHE.with(|c| *c.borrow_mut() = cache);
        drop(());
        let max_branch = departures[..np].iter().filter(|d| !d.is_empty()).count();
        jump_tab.clear();
        jump_tab.resize(np, [INVALID; 64]);
        let find_jump = |jt: &[[Index; 64]], axis_of: &[usize], from: Vec4| {
            let i = jt[axis_of_line(axis_of, from.l())][from.xy() as usize];
            if i == INVALID { None } else { Some(i) }
        };
        let _prof = crate::misc::prof::scope(12);
        if axis_coords.len() < np {
            axis_coords.resize_with(np, Vec::new);
        }
        // The departures first, so each arrival finds its departure coordinate.
        first_jump.clear();
        for axis in 0..np {
            let locs = &mut axis_coords[axis];
            first_jump.push(locs.len());
            for &p in &departures[axis] {
                let idx = locs.len() as Index;
                if memo.departure_safe(p) {
                    let mut nb = *s.board(p.l(), p.t(), player);
                    nb.clear_square(p.xy());
                    locs.push(Entry::Departing { from: p, b: nb });
                    jump_tab[axis][p.xy() as usize] = idx;
                }
            }
        }
        // Arrivals onto each line's last board (when it is the mover's); the ones whose
        // departure was filtered out lose their coordinate.
        orphans.clear();
        for axis in 0..np {
            let l = playable[axis];
            let (end_t, end_c) = s.get_timeline_end(l);
            if player != end_c {
                continue;
            }
            let locs = &mut axis_coords[axis];
            let target = s.board(l, end_t, player);
            for a in arrivals[(l - lines_lo) as usize].iter_mut() {
                let (m, mover) = (a.m, a.mover);
                if m.to.t() == end_t && a.safe(&mut memo) {
                    let victim = target.kind(m.to.xy());
                    let fa = axis_of_line(axis_of, m.from.l()) as u32;
                    let idx = find_jump(jump_tab, axis_of, m.from).unwrap_or(INVALID);
                    if idx == INVALID {
                        orphans.push((axis, locs.len() as Index));
                    }
                    locs.push(Entry::Arriving { m, b: arrival_board(s, m), idx, fa, mover, victim });
                }
            }
        }

        drop(());
        let _prof = crate::misc::prof::scope(13);
        // The branch arrivals: only the ones whose result needs a check test get their board
        // here; the entries are built by `materialize_branch`.
        branch_moves.clear();
        for a in arrivals[..nl].iter_mut().flatten() {
            if let Some(dep) = find_jump(jump_tab, axis_of, a.m.from)
                && a.safe(&mut memo) {
                    let fa = axis_of_line(axis_of, a.m.from.l()) as u32;
                    branch_moves.push((a.m, dep, fa, a.mover));
                }
        }
        branch_entries.clear();
        branch_entries.push(Entry::Null);
        *branch_ready = false;
        drop(());
        let _prof = crate::misc::prof::scope(14);
        let new_l = s.new_line();
        let sign = signum(new_l);
        line_axes.clear();
        for (axis, &l) in playable.iter().enumerate() {
            line_axes.push((l, axis));
        }
        for i in 0..max_branch {
            let l = new_l + sign * i as i32;
            line_axes.push((l, np + i));
            axis_of[(l - axis_lo) as usize] = np + i;
        }
        line_axes.sort_unstable_by_key(|e| e.0);
        let dimension = np + max_branch;
        let n_branch = branch_moves.len() + 1;
        universe.set_ranges(dimension, |n| if n < np { axis_coords[n].len() } else { n_branch });
        // (Branch arrivals were only admitted with a live departure.)
        for &(n, i) in orphans.iter() {
            universe[n].erase(i);
        }
        // The search only tests same-board checks on boards the action creates. A royal already
        // attacked on the same board of an opponent-to-move board stays capturable whatever the
        // mover does, so no action is legal (unreachable by legal play; set-up positions only).
        if dimension > 0
            && (lines_lo..=lines_hi).any(|l| {
                let (t, c) = s.get_timeline_end(l);
                c != player && facts.lookup(s.board(l, t, c), s.multiverse().key_of(l, t, c), player).1.in_check
            })
        {
            universe[0].clear();
        }
        FACTS_CACHE.with(|c| *c.borrow_mut() = facts);
        let mut hc_n_lines = universe.clone();
        let mut non_null = IntegerSet::new();
        if np < dimension {
            non_null = IntegerSet::range(n_branch as Index);
            non_null.erase(0);
            for n in np..dimension {
                hc_n_lines[n].assign_singleton(0);
            }
        }
        // [all branch axes non-null, ..., the first one non-null, none]
        let mut hcs = Vec::with_capacity(dimension - np + 1);
        for n in np..dimension {
            hcs.push(hc_n_lines.clone());
            hc_n_lines[n].assign(&non_null);
        }
        hcs.push(hc_n_lines);
        hcs.reverse();
        let ss = SearchSpace::from_vec(hcs);
        if groups.len() < np + 1 {
            groups.resize_with(np + 1, AxisGroups::default);
        }
        for n in 0..np {
            groups[n].rebuild(&axis_coords[n], first_jump[n], np, group_slots);
        }
        groups[np].rebuild(branch_entries, 1, np, group_slots);
        *info_player = player;
        *dim = dimension;
        *new_axis = np;
        *all_lines_mine = (lines_lo..=lines_hi).all(|l| s.get_timeline_end(l).1 == player);
        lone_checks.get_mut().iter_mut().for_each(|e| e.0 = false);
        let env = env_checks.get_mut();
        env.boards_known = false;
        env.checks.iter_mut().for_each(|e| e.0 = false);
        if !defer_branch {
            self.materialize_branch(s);
        }
        ss
    }

    /// Build the branch axes' arrival entries `rebuild` deferred (`s` is the state it was
    /// built from).
    pub fn materialize_branch(&mut self, s: &State) {
        if self.branch_ready {
            return;
        }
        let np = self.new_axis;
        if self.branch_onto.len() < np {
            self.branch_onto.resize_with(np, IntegerSet::new);
        }
        self.branch_onto[..np].iter_mut().for_each(|o| o.clear());
        for (i, &(m, dep, fa, mover)) in self.branch_moves.iter().enumerate() {
            let victim = s.board(m.to.l(), m.to.t(), self.player).kind(m.to.xy());
            self.branch_entries.push(Entry::Arriving { m, b: arrival_board(s, m), idx: dep, fa, mover, victim });
            if let Some(a) = self.line_to_axis(m.to.l()).filter(|&a| a < np)
                && s.get_timeline_end(m.to.l()) == (m.to.t(), self.player) {
                    self.branch_onto[a].insert(i as Index + 1);
                }
        }
        self.groups[np].rebuild(&self.branch_entries, 1, np, &mut self.scratch.group_slots);
        self.branch_ready = true;
    }

    /// Pick a point in `hc` whose departures and arrivals pair up (into `sc.point`). Prunes
    /// arrivals whose departure is no longer available.
    pub fn take_point(&self, hc: &mut Hc, order: &HcOrdering, sc: &mut PointScratch) -> bool {
        let _prof = crate::misc::prof::scope(2);
        let dim = self.dimension();
        let PointScratch { g, must_include, edge_refs, want, point: result } = sc;
        g.reset(dim);
        must_include.clear();
        edge_refs.clear();
        edge_refs.resize(dim * dim, INVALID);
        result.clear();
        result.resize(dim, INVALID);
        want.clear();
        want.resize(dim, false);
        // Arrivals on branch axes whose departure left the hypercuboid (the departure axes
        // all come first, so this is the same for every branch axis).
        let mut branch_ghosts: Option<IntegerSet> = None;
        for n in 0..dim {
            // An axis held to its null coordinate (an unplayed board or branch axis: most axes
            // at interior nodes) takes it: no arrival is the null one, so there is nothing to
            // prune, pair or include (the general path below gives the same).
            if hc[n].single() == Some(0) && matches!(self.entry(n, 0), Entry::Null) && (matches!(order, HcOrdering::Natural) || self.universe[n].contains(0)) {
                result[n] = 0;
                continue;
            }
            let grp = &self.groups[n.min(self.new_axis)];
            
            // Arrivals whose departure left the hypercuboid can never be paired.
            if n < self.new_axis {
                let mut ghost: Option<IntegerSet> = None;
                for (fa, dep, set) in &grp.by_dep[..grp.dep_len] {
                    if !hc[*fa as usize].contains(*dep) && set.intersects(&hc[n]) {
                        ghost.get_or_insert_with(IntegerSet::new).or_assign(set);
                    }
                }
                if let Some(gs) = ghost {
                    hc[n].minus(&gs);
                }
            } else {
                let gs = branch_ghosts.get_or_insert_with(|| {
                    let mut g = IntegerSet::new();
                    for (fa, dep, set) in &grp.by_dep[..grp.dep_len] {
                        if !hc[*fa as usize].contains(*dep) {
                            g.or_assign(set);
                        }
                    }
                    g
                });
                hc[n].minus(gs);
            }
            if hc[n].is_empty() {
                return false;
            }
            let live = &hc[n];
            let need_nonjump = live.intersects(&grp.nonjump);
            let mut needed = need_nonjump as usize;
            for (fa, set) in &grp.by_from[..grp.from_len] {
                // An edge already claimed from the other endpoint keeps its first pairing.
                if edge_refs[*fa as usize * dim + n] == INVALID && set.intersects(live) {
                    want[*fa as usize] = true;
                    needed += 1;
                }
            }
            // First coordinate in axis order of each category: the non-jump choice, and one
            // arrival per departure axis (its matching edge).
            let mut found = 0;
            let mut visit = |i: Index| -> bool {
                match self.entry(n, i) {
                    Entry::Physical { .. } | Entry::Null => {
                        if result[n] == INVALID {
                            result[n] = i;
                            found += 1;
                        }
                    }
                    Entry::Arriving { idx, fa, .. } => {
                        let fa = *fa as usize;
                        if want[fa] {
                            want[fa] = false;
                            g.add_edge(fa as Index, n as Index);
                            edge_refs[fa * dim + n] = *idx;
                            edge_refs[n * dim + fa] = i;
                            found += 1;
                        }
                    }
                    Entry::Departing { .. } => {}
                }
                found == needed
            };
            match order {
                // In natural order the first coordinate of each category is the smallest one
                // live in its group (orphan arrivals are outside the universe).
                HcOrdering::Natural => {
                    if need_nonjump && let Some(i) = live.first_common(&grp.nonjump) {
                        visit(i);
                    }
                    for (_, set) in &grp.by_from[..grp.from_len] {
                        if let Some(i) = live.first_common(set) {
                            visit(i);
                        }
                    }
                }
                HcOrdering::Precomputed(o) => match live.single() {
                    // Orders list the universe: a lone live coordinate is the only one the
                    // scan below would visit.
                    Some(i) => {
                        if self.universe[n].contains(i) {
                            visit(i);
                        }
                    }
                    None => {
                        for &i in o.axis(n) {
                            if live.contains(i) && visit(i) {
                                break;
                            }
                        }
                    }
                },
            }
            if !need_nonjump {
                must_include.push(n as Index);
            }
        }
        let Some(matching) = g.find_matching(must_include) else { return false };
        for &(u, v) in matching {
            let (u, v) = (u as usize, v as usize);
            result[u] = edge_refs[u * dim + v];
            result[v] = edge_refs[v * dim + u];
        }
        debug_assert!(result.iter().all(|&i| i != INVALID));
        true
    }


    /// Why `p` is no legal action. With `regions`, a jump order problem is the region over every
    /// branch axis (`jump_order_region`), else the slice of its two axes.
    pub fn find_problem(&self, s: &State, p: &Point, hc: &Hc, buf: &mut CheckBuf, regions: bool) -> Option<Problem> {
        let _prof = crate::misc::prof::scope(3);
        let jump = if regions { self.jump_order_region(s, p) } else { self.jump_order_consistent(s, p, hc).map(Problem::Slice) };
        if jump.is_some() {
            return jump;
        }
        let r = self.present_after(s, p).and_then(|pa| self.present_problem(s, p, &pa).or_else(|| self.wasted_problem(s, p, &pa)));
        drop(());
        if r.is_some() {
            return r.map(Problem::Slice);
        }
        let _prof = crate::misc::prof::scope(4);
        self.find_checks(s, p, hc, buf).map(Problem::Slice)
    }

    /// `jump_order_consistent`'s test, its problem as the region over every branch axis.
    fn jump_order_region(&self, st: &State, p: &Point) -> Option<Problem> {
        let (_, c) = st.get_present();
        // The arrivals' target boards (line, time) so far, in branch axis order.
        let mut arrived: Vec<Vec4> = Vec::new();
        let branch = |f: &dyn Fn(&FullMove) -> bool| -> IntegerSet {
            let mut set = IntegerSet::new();
            for (k, b) in self.branch_moves.iter().enumerate() {
                if f(&b.0) {
                    set.insert(k as Index + 1);
                }
            }
            set
        };
        for n in self.new_axis..self.dimension() {
            let m_arr = match self.entry(n, p[n]) {
                Entry::Null => break,
                Entry::Arriving { m, .. } => *m,
                _ => unreachable!(),
            };
            let (from, to) = (m_arr.from, m_arr.to);
            if let Some(m) = self.line_to_axis(to.l()) {
                let im = p[m];
                if self.entry(m, im).is_null() && st.get_timeline_end(to.l()) == (to.t(), c) {
                    return Some(Problem::Unplayed { m, nulls: IntegerSet::singleton(im), onto: branch(&|b| b.to.tl() == to.tl()) });
                }
            }
            let critical = from.tl();
            if arrived.contains(&critical) {
                return Some(Problem::Order { arr: branch(&|b| b.to.tl() == critical), dep: branch(&|b| b.from.tl() == critical) });
            }
            arrived.push(to.tl());
        }
        None
    }

    pub fn jump_order_consistent(&self, st: &State, p: &Point, hc: &Hc) -> Option<Slice> {
        let mut jump_map: Vec<(Vec4, usize)> = Vec::new();
        let (_, c) = st.get_present();
        for n in self.new_axis..self.dimension() {
            let m_arr = match self.entry(n, p[n]) {
                Entry::Null => break,
                Entry::Arriving { m, .. } => *m,
                _ => unreachable!(),
            };
            let (from, to) = (m_arr.from, m_arr.to);
            if let Some(m) = self.line_to_axis(to.l()) {
                let im = p[m];
                if self.entry(m, im).is_null() && st.get_timeline_end(to.l()) == (to.t(), c) {
                    let mut s = IntegerSet::new();
                    for i in hc[n].iter() {
                        if let Entry::Arriving { m: m3, .. } = self.entry(n, i)
                            && m3.to.tl() == to.tl() {
                                s.insert(i);
                            }
                    }
                    let mut problem = Slice::new();
                    problem.fix_axis(n, s);
                    problem.fix_axis(m, IntegerSet::singleton(im));
                    debug_assert!(problem.contains(p));
                    return Some(problem);
                }
            }
            let critical = from.tl();
            if let Some(&(_, axis_branch)) = jump_map.iter().find(|j| j.0 == critical) {
                let mut s1 = IntegerSet::new();
                let mut s2 = IntegerSet::new();
                for i in hc[n].iter() {
                    if let Entry::Arriving { m: m1, .. } = self.entry(n, i)
                        && m1.from.tl() == critical {
                            s1.insert(i);
                        }
                }
                for i in hc[axis_branch].iter() {
                    if let Entry::Arriving { m: m2, .. } = self.entry(axis_branch, i)
                        && m2.to.tl() == critical {
                            s2.insert(i);
                        }
                }
                let mut problem = Slice::new();
                problem.fix_axis(n, s1);
                problem.fix_axis(axis_branch, s2);
                debug_assert!(problem.contains(p));
                return Some(problem);
            }
            match jump_map.iter_mut().find(|j| j.0 == to.tl()) {
                Some(j) => j.1 = n,
                None => jump_map.push((to.tl(), n)),
            }
        }
        None
    }

    /// The active range once `k` new timelines exist. Every bound only widens as `k` grows,
    /// so a timeline active with `k` new ones is active with more, and one inactive with `k`
    /// is inactive with fewer.
    fn active_after(&self, st: &State, k: usize) -> (i32, i32) {
        let (l_min, l_max) = st.get_lines_range();
        let (a, b) = if self.player { (l_min - k as i32, l_max) } else { (l_min, l_max + k as i32) };
        super::multiverse::active_range_for(st.multiverse().layout(), a, b)
    }

    /// The end (half-turn) of the timeline the `j`-th branch axis of `p` creates.
    #[inline]
    fn new_end(&self, p: &Point, j: usize) -> i32 {
        match self.entry(self.new_axis + j, p[self.new_axis + j]) {
            Entry::Arriving { m, .. } => 2 * m.to.t() + self.player as i32 + 1,
            _ => unreachable!("null branch coordinate inside the branch prefix"),
        }
    }

    /// The exact present after the point, with the active range recomputed from the final
    /// timeline span. `None` when no timeline is active.
    fn present_after(&self, st: &State, p: &Point) -> Option<PresentAfter> {
        let (l_min, l_max) = st.get_lines_range();
        let c = self.player;
        // (The branch axes form a prefix.)
        let k = (self.new_axis..self.dim).take_while(|&n| !self.entry(n, p[n]).is_null()).count();
        let (amin, amax) = self.active_after(st, k);
        let mut wit: Option<(i32, i32)> = None;
        for l in amin..=amax {
            let v = if l < l_min || l > l_max {
                self.new_end(p, (if c { l_min - 1 - l } else { l - l_max - 1 }) as usize)
            } else {
                let v = st.multiverse().timeline_end_v(l);
                match self.line_to_axis(l) {
                    Some(n) if !self.entry(n, p[n]).is_null() => v + 1,
                    _ => v,
                }
            };
            if wit.map(|w| v < w.1).unwrap_or(true) {
                wit = Some((l, v));
            }
        }
        let (w_line, w_v) = wit?;
        Some(PresentAfter { k, amin, amax, w_line, w_v })
    }

    /// Whether the point moves the present; if not, a slice of points that fail for the same
    /// reason (see `present_after`).
    pub fn test_present(&self, st: &State, p: &Point, _hc: &Hc) -> Option<Slice> {
        self.present_problem(st, p, &self.present_after(st, p)?)
    }

    /// Whether the point (which moves the present) plays a wasted move; if so, a slice of
    /// points wasting one for the same reason (see `wasted_problem`).
    pub fn test_wasted(&self, st: &State, p: &Point, _hc: &Hc) -> Option<Slice> {
        let pa = self.present_after(st, p)?;
        if self.present_problem(st, p, &pa).is_some() {
            return None;
        }
        self.wasted_problem(st, p, &pa)
    }

    fn present_problem(&self, st: &State, p: &Point, pa: &PresentAfter) -> Option<Slice> {
        let c = self.player;
        let (l_min, l_max) = st.get_lines_range();
        let PresentAfter { k, amin, amax, w_line, w_v } = *pa;
        if (w_v & 1 != 0) != c {
            return None;
        }
        // Every point below keeps the witness timeline unmoved and active and adds no active
        // timeline that ends (on the opponent's turn) before it.
        let mut problem = Slice::new();
        let w_axis = self.line_to_axis(w_line).expect("witness axis");
        problem.fix_axis(w_axis, IntegerSet::singleton(0));
        for &(l, n) in &self.line_axes {
            if n >= self.new_axis || n == w_axis || l < amin || l > amax {
                continue;
            }
            if st.multiverse().timeline_end_v(l) + 1 < w_v {
                problem.fix_axis(n, IntegerSet::singleton(0));
            }
        }
        let t_w = w_v >> 1;
        for (j, n) in (self.new_axis..self.dim).enumerate() {
            let mut s = IntegerSet::new();
            if j < k {
                let line = if c { l_min - 1 - j as i32 } else { l_max + 1 + j as i32 };
                let active = line >= amin && line <= amax;
                for i in self.universe[n].iter() {
                    if let Entry::Arriving { m, .. } = self.entry(n, i)
                        && (!active || m.to.t() >= t_w) {
                            s.insert(i);
                        }
                }
            } else {
                s.insert(0);
            }
            problem.fix_axis(n, s);
        }
        debug_assert!(problem.contains(p));
        Some(problem)
    }

    /// Slice of points that branch and also play a wasted move: one whose boards all stay out
    /// of the present. Dropping it gives an equally legal action, so these are not generated.
    /// The null semimove stays outside the slice, so the action without that move survives.
    fn wasted_problem(&self, st: &State, p: &Point, pa: &PresentAfter) -> Option<Slice> {
        if pa.k == 0 {
            return None;
        }
        let mv = st.multiverse();
        let v_p = pa.w_v;
        let active_p = |l: i32| l >= pa.amin && l <= pa.amax;
        // Why the next board of a playable timeline is out of the present: `Some(true)` it
        // comes after it, `Some(false)` it only is inactive.
        let out = |l: i32| -> Option<bool> {
            if mv.timeline_end_v(l) >= v_p {
                Some(true)
            } else if !active_p(l) {
                Some(false)
            } else {
                None
            }
        };
        let BuildScratch { playable, first_jump, .. } = &self.scratch;
        let na = self.new_axis;
        // A semimove on a board the point's branches land on is what makes them branch.
        let landed = |a: usize| (na..na + pa.k).any(|j| self.branch_onto[a].contains(p[j]));
        let (n, reasons) = (0..na).find_map(|n| {
            let l = playable[n];
            let (lf, fa) = match self.entry(n, p[n]) {
                Entry::Physical { .. } => (l, n),
                Entry::Arriving { m, fa, .. } => (m.from.l(), *fa as usize),
                _ => return None,
            };
            let rn = out(l)?;
            let rf = if lf == l { rn } else { out(lf)? };
            if landed(n) || landed(fa) {
                return None;
            }
            Some((n, [(l, rn), (lf, rf)]))
        })?;
        // The slice's points move the present before `bound` (when a board is wasted for
        // coming after it) and create at most `pa.k` timelines (when one is inactive).
        let mut bound = i32::MAX;
        let mut at_most_k = false;
        for (l, later) in reasons {
            if later {
                bound = bound.min(mv.timeline_end_v(l) + 1);
            } else {
                at_most_k = true;
            }
        }
        let wasted_from = |l: i32| mv.timeline_end_v(l) + 1 >= bound || (at_most_k && !active_p(l));
        let mut on_n = IntegerSet::range(first_jump[n] as Index);
        on_n.erase(0);
        // The branch coordinates the slice's points never use: those landing on a board a
        // wasted semimove of the slice is played on.
        let mut landing = self.branch_onto[n].clone();
        let grp = &self.groups[n];
        for (fa, set) in &grp.by_from[..grp.from_len] {
            let fa = *fa as usize;
            if wasted_from(playable[fa]) && !landed(fa) {
                on_n.or_assign(set);
                landing.or_assign(&self.branch_onto[fa]);
            }
        }
        let mut problem = Slice::new();
        problem.fix_axis(n, on_n);
        if !landing.is_empty() {
            let mut keep = IntegerSet::from_bits(&self.universe[na]);
            keep.minus(&landing);
            for j in na..self.dim {
                problem.fix_axis(j, keep.clone());
            }
        }
        let non_null = || {
            let mut s = IntegerSet::from_bits(&self.universe[na]);
            s.erase(0);
            s
        };
        if bound != i32::MAX {
            // Pick the cheapest active timeline that ends before `bound` in every point of
            // the slice, preferring one whose end does not depend on its axis.
            let (l_min, l_max) = st.get_lines_range();
            let (a1, b1) = self.active_after(st, 1);
            #[derive(Clone, Copy)]
            enum Witness {
                Free,
                Unmoved(usize),
                New(usize),
            }
            let mut best: Option<(u8, Witness, bool)> = None;
            for l in pa.amin..=pa.amax {
                let cand = if l < l_min || l > l_max {
                    let j = (if self.player { l_min - 1 - l } else { l - l_max - 1 }) as usize;
                    if self.new_end(p, j) >= bound {
                        continue;
                    }
                    let (aj, bj) = self.active_after(st, j + 1);
                    let wide = !(aj <= l && l <= bj);
                    (3 * wide as u8 + if j == 0 { 1 } else { 2 }, Witness::New(j), wide)
                } else {
                    let v = mv.timeline_end_v(l);
                    let axis = self.line_to_axis(l);
                    let moved = axis.is_some_and(|a| p[a] != 0);
                    if v + moved as i32 >= bound {
                        continue;
                    }
                    let wide = !(a1 <= l && l <= b1);
                    match axis {
                        Some(a) if v + 1 >= bound => (3 * wide as u8 + 2, Witness::Unmoved(a), wide),
                        _ => (3 * wide as u8, Witness::Free, wide),
                    }
                };
                if best.is_none_or(|b| cand.0 < b.0) {
                    best = Some(cand);
                }
            }
            let (_, witness, wide) = best.expect("the present is held by an active timeline");
            match witness {
                Witness::Free => {}
                Witness::Unmoved(a) => problem.restrict_axis(a, IntegerSet::singleton(0)),
                Witness::New(j) => {
                    let c = self.player as i32;
                    let mut s = IntegerSet::new();
                    for i in self.universe[na + j].iter() {
                        if let Entry::Arriving { m, .. } = self.entry(na + j, i)
                            && 2 * m.to.t() + c + 1 < bound {
                                s.insert(i);
                            }
                    }
                    problem.restrict_axis(na + j, s);
                }
            }
            if wide {
                problem.restrict_axis(na + pa.k - 1, non_null());
            }
        }
        problem.restrict_axis(na, non_null());
        if at_most_k && na + pa.k < self.dim {
            problem.restrict_axis(na + pa.k, IntegerSet::singleton(0));
        }
        debug_assert!(problem.contains(p));
        Some(problem)
    }

    /// Whether a check is possible after a point moving only existing timelines: `NoCheck`,
    /// or `Lines` when only `env_checks.maybe` may give one. Valid only when every timeline
    /// ends on the mover's turn.
    fn lone_move_may_check(&self, st: &State, p: &Point, lo: i32, hi: i32, buf: &mut CheckBuf) -> MayCheck {
        let mut moved = None;
        let mut moved_lines = 0;
        for &(l, axis) in &self.line_axes {
            if !self.entry(axis, p[axis]).is_null() {
                if axis >= self.new_axis {
                    return MayCheck::All;
                }
                moved_lines += 1;
                moved.get_or_insert((l, axis));
            }
        }
        let Some((l, axis)) = moved else { return MayCheck::All };
        if moved_lines > 1 {
            return if self.moves_may_check(st, p, lo, hi, buf) { MayCheck::Lines } else { MayCheck::NoCheck };
        }
        let Entry::Physical { b: result, .. } = self.entry(axis, p[axis]) else { return MayCheck::All };
        let c = self.player;
        let mut cache = self.lone_checks.borrow_mut();
        if cache.len() < self.new_axis {
            cache.resize_with(self.new_axis, Default::default);
        }
        let (known, checks) = &mut cache[axis];
        if !*known {
            // The timeline's next board with every opponent piece in place: the moved board
            // differs only by the mover's pieces and possibly one captured piece.
            let (t, _) = st.get_timeline_end(l);
            let mut cp = CheckPosition::with_range_in(st, lo, hi, std::mem::take(buf));
            cp.add_board(l, next_turn((t, c)), st.board(l, t, c));
            checks.clear();
            cp.checks_into(!c, false, checks);
            *buf = cp.into_buf();
            *known = true;
        }
        let attackers = result.hostile(c);
        if checks.iter().any(|m| attackers & pmask(m.from.xy()) != 0) { MayCheck::All } else { MayCheck::NoCheck }
    }

    /// `lone_move_may_check` for several moved timelines. Checks are precomputed per build
    /// against a worst case of the other boards (royals on every square they could hold);
    /// timelines with a surviving attacker are left in `env_checks.maybe`.
    fn moves_may_check(&self, st: &State, p: &Point, lo: i32, hi: i32, buf: &mut CheckBuf) -> bool {
        let c = self.player;
        let mut env = self.env_checks.borrow_mut();
        let EnvChecks { boards_known, boards, checks, maybe } = &mut *env;
        if !*boards_known {
            boards.clear();
            for n in 0..self.new_axis {
                let mut royals = 0;
                for i in 1..self.axis_coords[n].len() {
                    let b = self.axis_coords[n][i].board();
                    royals |= b.royal() & b.hostile(!c);
                }
                let mut b = Board::default();
                for sq in marked_pos(royals) {
                    b.set_piece(sq, if c { KING_B } else { KING_W });
                }
                boards.push(b);
            }
            *boards_known = true;
        }
        if checks.len() < self.new_axis {
            checks.resize_with(self.new_axis, Default::default);
        }
        maybe.clear();
        for &(l, axis) in &self.line_axes {
            if axis >= self.new_axis || self.entry(axis, p[axis]).is_null() {
                continue;
            }
            let (known, found) = &mut checks[axis];
            if !*known {
                let mut cp = CheckPosition::with_range_in(st, lo, hi, std::mem::take(buf));
                for &(l2, n) in &self.line_axes {
                    if n >= self.new_axis {
                        continue;
                    }
                    let t2 = st.get_timeline_end(l2).0;
                    cp.add_board(l2, next_turn((t2, c)), if n == axis { st.board(l2, t2, c) } else { &boards[n] });
                }
                found.clear();
                cp.checks_into(!c, false, found);
                *buf = cp.into_buf();
                *known = true;
            }
            let attackers = self.entry(axis, p[axis]).board().hostile(c);
            if found.iter().any(|m| attackers & pmask(m.from.xy()) != 0) {
                maybe.push(l);
            }
        }
        !maybe.is_empty()
    }

    /// The first check the opponent has after point `p` (the scan `find_checks` builds its
    /// slice from), with the position it was found in; `buf` is lent to that position (get it
    /// back with `into_buf`) and left in place when there is none.
    fn first_check_after<'a>(&'a self, st: &'a State, p: &Point, buf: &mut CheckBuf) -> Option<(FullMove, CheckPosition<'a>)> {
        let (_, c) = st.get_present();
        let (mut lo, mut hi) = st.get_lines_range();
        if let (Some(&(first, _)), Some(&(last, _))) = (self.line_axes.first(), self.line_axes.last()) {
            lo = lo.min(first);
            hi = hi.max(last);
        }
        let may = if self.all_lines_mine { self.lone_move_may_check(st, p, lo, hi, buf) } else { MayCheck::All };
        if may == MayCheck::NoCheck {
            return None;
        }
        let mut newstate = CheckPosition::with_range_in(st, lo, hi, std::mem::take(buf));
        for &(l, axis) in &self.line_axes {
            let loc = self.entry(axis, p[axis]);
            if loc.is_null() {
                continue;
            }
            let (old_t, _) = loc.tl();
            newstate.add_board(l, next_turn((old_t, c)), loc.board());
        }
        // Timelines that cannot give check need no scan (`first_check` scans by timeline).
        let first = match may {
            MayCheck::Lines => newstate.first_check_in(!c, &self.env_checks.borrow().maybe),
            _ => newstate.first_check(!c, false),
        };
        match first {
            Some(check) => Some((check, newstate)),
            None => {
                *buf = newstate.into_buf();
                None
            }
        }
    }

    /// Whether the point `p` of a context with one playable board, every branch axis null, is
    /// a legal action: `find_problem(..).is_none()` without building the slice.
    pub fn lone_point_legal(&self, st: &State, p: &Point, buf: &mut CheckBuf) -> bool {
        debug_assert!(self.new_axis == 1 && p[1..].iter().all(|&i| i == 0));
        // Without a branch the jumps are in order. Every timeline ending on the mover's turn is
        // playable, so moving the one playable board (the one at the present) moves the
        // present, and leaving it unmoved does not.
        if self.entry(0, p[0]).is_null() {
            return false;
        }
        match self.first_check_after(st, p, buf) {
            Some((_, newstate)) => {
                *buf = newstate.into_buf();
                false
            }
            None => true,
        }
    }

    pub fn find_checks(&self, st: &State, p: &Point, hc: &Hc, buf: &mut CheckBuf) -> Option<Slice> {
        let c = st.get_present().1;
        let (check, newstate) = self.first_check_after(st, p, buf)?;
        let (path, sliding_type) = get_move_path(&newstate, check, !c);
        let is_next = |t1: i32, t2: i32| if c { t1 + 1 == t2 } else { t1 == t2 };
        let sliding_bb = |nb: &Board| -> Bitboard {
            let bb = if c { nb.white() } else { nb.black() };
            bb & match sliding_type {
                1 => nb.lrook(),
                2 => nb.lbishop(),
                3 => nb.lunicorn(),
                4 => nb.ldragon(),
                _ => unreachable!("wrong sliding type inferred"),
            }
        };
        let mut problem = Slice::new();
        if let Some(n1) = self.line_to_axis(check.from.l()) {
            let mut not_taking = IntegerSet::new();
            for i in hc[n1].iter() {
                let loc = self.entry(n1, i);
                if loc.is_null() || !is_next(loc.tl().0, check.from.t()) {
                    continue;
                }
                let nb = loc.board();
                if sliding_type != 0 {
                    if pmask(check.from.xy()) & sliding_bb(nb) != 0 {
                        not_taking.insert(i);
                    }
                } else if nb.get_piece(check.from.xy()) == newstate.get_piece(check.from, !c) {
                    not_taking.insert(i);
                }
            }
            problem.fix_axis(n1, not_taking);
        }
        let l2 = check.to.l();
        if let Some(n2) = self.line_to_axis(l2) {
            let loc0 = self.entry(n2, p[n2]);
            if !loc0.is_null() && is_next(loc0.tl().0, check.to.t()) {
                let mut expose = IntegerSet::new();
                for i in hc[n2].iter() {
                    let loc = self.entry(n2, i);
                    if loc.is_null() || !is_next(loc.tl().0, check.to.t()) {
                        continue;
                    }
                    let nb = loc.board();
                    let friendly = if c { nb.black() } else { nb.white() };
                    if pmask(check.to.xy()) & nb.royal() & friendly != 0 {
                        expose.insert(i);
                    }
                }
                problem.fix_axis(n2, expose);
            }
        }
        for &crossed in &path {
            if let Some(n) = self.line_to_axis(crossed.l()) {
                let loc0 = self.entry(n, p[n]);
                if loc0.is_null() || !is_next(loc0.tl().0, crossed.t()) {
                    continue;
                }
                let z = pmask(crossed.xy());
                let mut not_blocking = IntegerSet::new();
                for i in hc[n].iter() {
                    let loc = self.entry(n, i);
                    if loc.is_null() || !is_next(loc.tl().0, crossed.t()) {
                        continue;
                    }
                    let nb = loc.board();
                    if z & nb.occupied() == 0 {
                        not_blocking.insert(i);
                        continue;
                    }
                    if sliding_type != 0 && z & sliding_bb(nb) != 0 {
                        not_blocking.insert(i);
                        continue;
                    }
                    let friendly = if c { nb.black() } else { nb.white() };
                    if z & nb.royal() & friendly != 0 {
                        not_blocking.insert(i);
                        continue;
                    }
                }
                problem.fix_axis(n, not_blocking);
            }
        }
        debug_assert!(problem.contains(p));
        *buf = newstate.into_buf();
        Some(problem)
    }

    pub fn to_action(&self, p: &Point) -> MoveSeq {
        let mut mvs = MoveSeq::new();
        // Lines in ascending order for white, descending for black.
        let mut push = |e: &Entry| match e {
            Entry::Physical { m, promo, .. } => mvs.push_promo(*m, *promo),
            Entry::Arriving { m, .. } => mvs.push(*m),
            _ => {}
        };
        if self.player {
            for &(_, i) in self.line_axes.iter().rev() {
                push(self.entry(i, p[i]));
            }
        } else {
            for &(_, i) in &self.line_axes {
                push(self.entry(i, p[i]));
            }
        }
        mvs
    }

    pub fn search<'a>(&'a self, s: &'a State, ss: SearchSpace) -> HcSearch<'a> {
        HcSearch { info: self, s, cursor: HcCursor::new(ss, HcOrdering::Natural) }
    }
}

/// Sliding path (endpoints excluded) and slider type (0 none, 1 rook .. 4 dragon).
fn get_move_path(s: &CheckPosition<'_>, fm: FullMove, c: bool) -> (Vec<Vec4>, i32) {
    let (p, q) = (fm.from, fm.to);
    let d = q - p;
    let b = s.get_board_ptr(p.l(), p.t(), c).unwrap();
    if b.sliding() & pmask(p.xy()) != 0 {
        let dir = Vec4::new(signum(d.x()), signum(d.y()), signum(d.t()), signum(d.l()));
        let st = dir.dot(dir);
        let mut path = Vec::new();
        let mut r = p + dir;
        while r != q {
            path.push(r);
            r = r + dir;
        }
        (path, st)
    } else {
        (Vec::new(), 0)
    }
}


/// What interrupts the cursors of this thread (see `interrupt_on`): a stop flag, a deadline
/// and a bound on the steps between two nodes.
#[derive(Clone, Copy)]
struct Interrupt {
    stop: *const std::sync::atomic::AtomicBool,
    deadline: Option<std::time::Instant>,
    /// Steps the cursors may take between two `interrupt_mark`s (see `interrupt_bounded`).
    max_steps: u32,
    /// Steps taken since the last `interrupt_mark`.
    since: u32,
    /// The cursors ran past `max_steps`: the interrupt stays fired.
    overrun: bool,
}

impl Interrupt {
    fn fired(&self) -> bool {
        // SAFETY: the guard that installed the flag borrows it for as long as it is installed.
        self.overrun || unsafe { &*self.stop }.load(std::sync::atomic::Ordering::Relaxed) || self.deadline.is_some_and(|d| std::time::Instant::now() >= d)
    }
}

thread_local! {
    static INTERRUPT: std::cell::Cell<Option<Interrupt>> = const { std::cell::Cell::new(None) };
}

/// Steps of one `BestFirstCursor` call after which (and after each doubling) a depth-first
/// search checks whether any action is left.
const EXHAUST_CHECK_STEPS: u32 = 256;
/// Cursor steps (hypercuboids taken) between interrupt checks.
const INTERRUPT_EVERY: u32 = 4;

/// While the guard lives, this thread's cursors stop once `stop` is set or `deadline` passes.
/// They then report no further action, so callers must check `interrupted()` before treating
/// that as the end. Needed because finding a legal action can take arbitrarily long.
pub fn interrupt_on(stop: &std::sync::atomic::AtomicBool, deadline: Option<std::time::Instant>) -> InterruptGuard<'_> {
    interrupt_bounded(stop, deadline, u32::MAX)
}

/// `interrupt_on` that also fires, for good, once the cursors take `max_steps` steps with no
/// `interrupt_mark` between: a search with no clock (a node limit) is otherwise never
/// interrupted, and with many timelines a node can enumerate for minutes (finding the next
/// legal action, or probing mates). A count, so the same on any machine.
pub fn interrupt_bounded(stop: &std::sync::atomic::AtomicBool, deadline: Option<std::time::Instant>, max_steps: u32) -> InterruptGuard<'_> {
    let prev = INTERRUPT.with(|c| c.replace(Some(Interrupt { stop: stop as *const _, deadline, max_steps, since: 0, overrun: false })));
    InterruptGuard { prev, _stop: std::marker::PhantomData }
}

/// While the guard lives, the cursors of this thread are not interrupted (an answer that must
/// be exact, such as `State::get_mate_type`, takes no stop for the end of the actions).
pub fn interrupt_off() -> InterruptGuard<'static> {
    let prev = INTERRUPT.with(|c| c.replace(None));
    InterruptGuard { prev, _stop: std::marker::PhantomData }
}

/// Restart the step count of `interrupt_bounded` (a search calls it at every node); whether
/// the count already ran out.
#[inline]
pub fn interrupt_mark() -> bool {
    INTERRUPT.with(|c| match c.get() {
        None => false,
        Some(mut i) => {
            i.since = 0;
            c.set(Some(i));
            i.overrun
        }
    })
}

/// Whether the cursor that took its `steps`-th step (of this call) is interrupted: counts the
/// step for `interrupt_bounded`, and polls `interrupted` every `INTERRUPT_EVERY` steps.
fn interrupted_step(steps: u32) -> bool {
    INTERRUPT.with(|c| {
        let Some(mut i) = c.get() else { return false };
        i.since = i.since.saturating_add(1);
        if i.since >= i.max_steps {
            i.overrun = true;
        }
        c.set(Some(i));
        i.overrun || steps.is_multiple_of(INTERRUPT_EVERY) && i.fired()
    })
}

/// Restores the previous interrupt of the thread when dropped (see `interrupt_on`).
pub struct InterruptGuard<'a> {
    prev: Option<Interrupt>,
    _stop: std::marker::PhantomData<&'a std::sync::atomic::AtomicBool>,
}

impl Drop for InterruptGuard<'_> {
    fn drop(&mut self) {
        INTERRUPT.with(|c| c.set(self.prev));
    }
}

/// Whether the interrupt of this thread (see `interrupt_on`) has fired: its stop flag is set,
/// its deadline has passed or the cursors overran their steps (`interrupt_bounded`).
pub fn interrupted() -> bool {
    INTERRUPT.with(|c| c.get().is_some_and(|i| i.fired()))
}

/// Why a point is no legal action, and every other point failing for the same reason.
pub enum Problem {
    Slice(Slice),
    /// A branch axis takes an arrival of `arr` and a later one an arrival of `dep`: a jump onto
    /// a board before a jump leaving it, where the first would not branch (the jump order the
    /// action means is the other one). Over every pair of branch axes, which no slice holds.
    Order { arr: IntegerSet, dep: IntegerSet },
    /// Axis `m` takes a coordinate of `nulls` (its board unplayed) and some branch axis an
    /// arrival of `onto`, onto that board's line's last board: such a jump does not branch.
    Unplayed { m: usize, nulls: IntegerSet, onto: IntegerSet },
}

impl Problem {
    /// Whether the point `p` (branch axes from `b0`) fails for this reason.
    fn contains(&self, p: &Point, b0: usize) -> bool {
        match self {
            Problem::Slice(s) => s.contains(p),
            Problem::Order { arr, dep } => {
                let mut seen = false;
                for &i in &p[b0..] {
                    if seen && dep.contains(i) {
                        return true;
                    }
                    seen |= arr.contains(i);
                }
                false
            }
            Problem::Unplayed { m, nulls, onto } => nulls.contains(p[*m]) && p[b0..].iter().any(|&i| onto.contains(i)),
        }
    }

    /// Whether `hc` has a point of the problem.
    fn intersects(&self, hc: &Hc, b0: usize) -> bool {
        match self {
            Problem::Slice(s) => hc.intersects(s),
            Problem::Order { arr, dep } => {
                let mut seen = false;
                for n in b0..hc.dimension() {
                    if seen && hc[n].intersects(dep) {
                        return true;
                    }
                    seen |= hc[n].intersects(arr);
                }
                false
            }
            Problem::Unplayed { m, nulls, onto } => hc[*m].intersects(nulls) && (b0..hc.dimension()).any(|n| hc[n].intersects(onto)),
        }
    }

    /// The points of `hc` outside the problem, as disjoint hypercuboids passed to `out`.
    fn remove_from(&self, hc: Hc, b0: usize, mut out: impl FnMut(Hc)) {
        match self {
            Problem::Slice(s) => hc.into_remove_slice_with(s, out),
            // By the first branch axis taking an arrival of `arr` (none: every axis avoids
            // them), after which every axis avoids `dep`.
            Problem::Order { arr, dep } => {
                let dim = hc.dimension();
                let mut before = hc;
                for k in b0..dim {
                    if before[k].intersects(arr) {
                        let mut piece = before.clone();
                        piece[k].and_assign(arr);
                        for j in k + 1..dim {
                            piece[j].minus(dep);
                        }
                        if !piece.is_empty() {
                            out(piece);
                        }
                    }
                    before[k].minus(arr);
                    if before[k].is_empty() {
                        return;
                    }
                }
                out(before);
            }
            Problem::Unplayed { m, nulls, onto } => {
                let mut played = hc.clone();
                played[*m].minus(nulls);
                if !played[*m].is_empty() {
                    out(played);
                }
                let mut unplayed = hc;
                unplayed[*m].and_assign(nulls);
                for n in b0..unplayed.dimension() {
                    unplayed[n].minus(onto);
                }
                if !unplayed.is_empty() {
                    out(unplayed);
                }
            }
        }
    }
}

/// Hypercuboids from the back of a search space a region problem is cut from (each into up to
/// one piece per branch axis): unbounded, one step could multiply a long queue many times over.
const REGION_LOOKBACK: usize = 16;

/// Starting at the back of `ss`, cut `problem` from the last `REGION_LOOKBACK` hypercuboids
/// while they keep meeting it (as `SearchSpace::remove_slice_backwards` does for a slice).
fn remove_problem_backwards(ss: &mut SearchSpace, problem: &Problem, b0: usize) {
    let (mut intersect, mut disjoint) = (0usize, 0usize);
    let mut current = ss.hcs.len();
    let stop = current.saturating_sub(REGION_LOOKBACK);
    while current > stop {
        current -= 1;
        if problem.intersects(&ss.hcs[current], b0) {
            let hc = std::mem::take(&mut ss.hcs[current]);
            let mut pieces = Vec::new();
            problem.remove_from(hc, b0, |h| pieces.push(h));
            ss.hcs.splice(current..current + 1, pieces);
            intersect += 1;
        } else {
            disjoint += 1;
        }
        if disjoint * 10 >= intersect {
            break;
        }
    }
}

/// Problem slices found while enumerating one context. Each holds only failing points, so a
/// later hypercuboid or point inside one is cut without re-finding the same problem.
#[derive(Default)]
struct ProblemMemo {
    slices: Vec<Problem>,
    next: usize,
}

/// Problems a memo keeps (the oldest is replaced).
const PROBLEM_MEMO: usize = 32;
/// Contexts with fewer boards to play (existing axes) keep no memo: their problems are few and
/// rarely found twice, so it would only cost time.
const PROBLEM_MEMO_MIN_AXES: usize = 2;

/// Turns the problem memos off (diagnostics: `ktools legalcheck` compares the actions found
/// with and without them).
pub static NO_PROBLEM_MEMO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl ProblemMemo {
    fn clear(&mut self) {
        self.slices.clear();
        self.next = 0;
    }
    fn add(&mut self, s: Problem) {
        if NO_PROBLEM_MEMO.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        if self.slices.len() < PROBLEM_MEMO {
            self.slices.push(s);
        } else {
            self.slices[self.next] = s;
            self.next = (self.next + 1) % PROBLEM_MEMO;
        }
    }
    /// Whether every point of `hc` lies in one remembered problem slice.
    fn covers(&self, hc: &Hc) -> bool {
        self.slices.iter().any(|s| matches!(s, Problem::Slice(s) if s.fixed.iter().all(|(n, c)| hc[*n].is_subset_of(c))))
    }
    /// A remembered problem holding `p` (branch axes from `b0`).
    fn holding(&self, p: &Point, b0: usize) -> Option<&Problem> {
        self.slices.iter().find(|s| s.contains(p, b0))
    }
}

/// Caller-excluded regions (e.g. proven mate-free, see `exclude`), skipped like problems.
/// Bounded: the oldest is replaced, which costs time but never loses an action.
#[derive(Clone, Default)]
struct RegionMemo {
    slices: Vec<Slice>,
    next: usize,
}

/// Regions a cursor keeps (`RegionMemo`).
const REGION_MEMO: usize = 64;

impl RegionMemo {
    fn clear(&mut self) {
        self.slices.clear();
        self.next = 0;
    }
    fn add(&mut self, s: Slice) {
        if self.slices.len() < REGION_MEMO {
            self.slices.push(s);
        } else {
            self.slices[self.next] = s;
            self.next = (self.next + 1) % REGION_MEMO;
        }
    }
    fn covers(&self, hc: &Hc) -> bool {
        self.slices.iter().any(|s| s.fixed.iter().all(|(n, c)| hc[*n].is_subset_of(c)))
    }
    fn holding(&self, p: &Point) -> Option<&Slice> {
        self.slices.iter().find(|s| s.contains(p))
    }
}

/// Resumable legal-action enumeration. The caller supplies the context and the state it
/// was built from on every call (the state may be modified between calls if restored).
pub struct HcCursor {
    pub ss: SearchSpace,
    pub order: HcOrdering,
    /// The branch axes' shared order is still to be filled (see `next_with`).
    pub branch_pending: bool,
    /// Hypercuboids it may still look at: once none is left, `next` finds no further action
    /// (the caller tells that apart from the end by `budget == 0`).
    pub budget: u32,
    /// Cut a jump order problem from every branch axis at once (`HcInfo::find_problem`): an
    /// escape search proving a mate with many branch axes takes tens of thousands of steps
    /// as pairs of axes, a few hundred so. (Elsewhere the regions' pieces cost more.)
    pub regions: bool,
    sc: PointScratch,
    checks: CheckBuf,
    /// Problems found in the current context (cleared by `forget`).
    memo: ProblemMemo,
    /// Regions skipped (`exclude`; cleared by `forget`).
    excluded: RegionMemo,
    /// The point of the last action found.
    last: Point,
}

impl Default for HcCursor {
    fn default() -> Self {
        HcCursor::new(SearchSpace::default(), HcOrdering::Natural)
    }
}

impl HcCursor {
    pub fn new(ss: SearchSpace, order: HcOrdering) -> Self {
        HcCursor {
            ss,
            order,
            branch_pending: false,
            budget: u32::MAX,
            regions: false,
            sc: PointScratch::default(),
            checks: CheckBuf::default(),
            memo: ProblemMemo::default(),
            excluded: RegionMemo::default(),
            last: Point::new(),
        }
    }

    pub fn next(&mut self, info: &HcInfo, s: &State) -> Option<MoveSeq> {
        self.next_with(info, s, &mut |_| {}).expect("branch entries built")
    }

    /// Whether no legal action is left among the remaining hypercuboids (`none_left_in`).
    #[cold]
    #[inline(never)]
    fn none_left(&self, info: &HcInfo, s: &State, steps: u32) -> bool {
        none_left_in(self.ss.hcs.clone(), &self.excluded, info, s, steps)
    }

    /// Forget the problems found so far: call when `ss` is set for another context (state).
    pub fn forget(&mut self) {
        self.memo.clear();
        self.excluded.clear();
    }

    /// Skip every point of `region` from now on (points the caller has no use for, e.g. actions
    /// proven to be no mate).
    pub fn exclude(&mut self, region: Slice) {
        self.excluded.add(region);
    }

    /// The point (coordinates) of the last action found.
    pub fn last_point(&self) -> &Point {
        &self.last
    }

    /// `next`, filling branch-axis order lazily via `fill_branch` when first needed. Returns
    /// `NeedBranch` without progress when the next hypercuboid needs deferred branch entries.
    pub fn next_with(
        &mut self,
        info: &HcInfo,
        s: &State,
        fill_branch: &mut dyn FnMut(&mut AxisOrders),
    ) -> Result<Option<MoveSeq>, NeedBranch> {
        let mut steps = 0u32;
        while !self.ss.is_empty() {
            steps += 1;
            if interrupted_step(steps) || self.budget == 0 {
                return Ok(None);
            }
            // An ordered, unbounded enumeration long without an action: often none is left (a
            // mate in quiescence), which natural order with regions proves far faster.
            if steps >= EXHAUST_CHECK_STEPS && steps.is_power_of_two() && !self.regions && self.budget > u32::MAX / 2 && self.none_left(info, s, steps) {
                self.ss.hcs.clear();
                return Ok(None);
            }
            self.budget -= 1;
            let mut hc = self.ss.pop_back().unwrap();
            if info.needs_branch(&hc) {
                self.ss.push_back(hc);
                return Err(NeedBranch);
            }
            if self.branch_pending
                && (info.first_branch_axis()..hc.dimension()).any(|n| !hc[n].is_empty() && hc[n].single().is_none())
            {
                fill_branch(self.order.axes_mut());
                self.branch_pending = false;
            }
            let memo = info.first_branch_axis() >= PROBLEM_MEMO_MIN_AXES;
            if memo && self.memo.covers(&hc) || self.excluded.covers(&hc) {
                continue;
            }
            if !info.take_point(&mut hc, &self.order, &mut self.sc) {
                continue;
            }
            let pt = &self.sc.point;
            if let Some(region) = self.excluded.holding(pt) {
                self.ss.push_back(hc);
                self.ss.remove_slice_backwards(region, true, 10, 1);
                continue;
            }
            let b0 = info.first_branch_axis();
            if let Some(problem) = memo.then(|| self.memo.holding(pt, b0)).flatten() {
                let _prof = crate::misc::prof::scope(5);
                self.ss.push_back(hc);
                match problem {
                    Problem::Slice(slice) => self.ss.remove_slice_backwards(slice, true, 10, 1),
                    region => remove_problem_backwards(&mut self.ss, region, b0),
                }
                continue;
            }
            match info.find_problem(s, pt, &hc, &mut self.checks, self.regions) {
                None => {
                    let act = info.to_action(pt);
                    hc.into_remove_point_with(pt, |h| self.ss.push_back(h));
                    self.last.clone_from(pt);
                    return Ok(Some(act));
                }
                Some(problem) => {
                    let _prof = crate::misc::prof::scope(5);
                    self.ss.push_back(hc);
                    match &problem {
                        Problem::Slice(slice) => self.ss.remove_slice_backwards(slice, true, 10, 1),
                        region => remove_problem_backwards(&mut self.ss, region, b0),
                    }
                    if memo {
                        self.memo.add(problem);
                    }
                }
            }
        }
        Ok(None)
    }
}

/// A cursor stopped: the next hypercuboid needs branch entries its context deferred (build
/// them with `HcInfo::materialize_branch` and call again).
#[derive(Debug)]
pub struct NeedBranch;

/// Legal actions roughly best-first by summed per-axis scores (`order` sorted by `scores`
/// descending), via a heap of hypercuboids keyed by their best point. With one playable
/// board, non-branching actions are walked directly in that board's order.
pub struct BestFirstCursor {
    heap: BfHeap,
    sc: PointScratch,
    checks: CheckBuf,
    lone: Option<LoneWalk>,
    /// Class of the last action found.
    last_class: i32,
    /// Classes below this one (`limit_classes`) are dropped after `limit_steps` hypercuboids
    /// without an action.
    limit_below: i32,
    limit_steps: u32,
    /// The limited class being enumerated and the hypercuboids it has taken.
    limit_class: i32,
    limit_taken: u32,
    /// Hypercuboids (and walked points) it may still look at, when bounded (`set_budget`): once
    /// none is left `next` finds no further action (`budget_spent` tells that from the end).
    budget: Option<u32>,
    /// Problems found since the last `restart` of another context (see `ProblemMemo`).
    memo: ProblemMemo,
    /// Regions skipped (`exclude`; cleared by `forget`).
    excluded: RegionMemo,
    /// The point of the last action found.
    last: Point,
}

/// The first class of a context with one playable board: the coordinates of its axis 0 still
/// to test, from position `pos` of the axis order on.
struct LoneWalk {
    hc: Hc,
    pos: usize,
    point: Point,
}

struct BfHeap {
    heap: std::collections::BinaryHeap<BfItem>,
    order: HcOrdering,
    scores: Vec<Vec<i32>>,
    /// The coordinates the orders list (the context's universe).
    universe: Hc,
    seq: u64,
    /// The branch axes (from `first_branch`) are not ordered yet: only the null
    /// coordinate's score is known.
    branch_pending: bool,
    first_branch: usize,
}

/// Fills the branch axes' order and scores of a cursor whose branch ordering is pending.
pub type BranchFill<'a> = dyn FnMut(&mut AxisOrders, &mut Vec<Vec<i32>>) + 'a;

impl BfHeap {
    /// Order the branch axes now when `hc` has a branch coordinate other than the null one.
    fn need_branch(&mut self, hc: &Hc, several: bool, fill: &mut BranchFill) {
        if self.branch_pending
            && (self.first_branch..hc.dimension()).any(|n| if several { !hc[n].is_empty() && hc[n].single().is_none() } else { hc[n].has_other_than(0) })
        {
            fill(self.order.axes_mut(), &mut self.scores);
            self.branch_pending = false;
        }
    }

    /// One of the initial hypercuboids: each has a class of its own, so its bound never
    /// decides an order and is not computed.
    fn push_initial(&mut self, hc: Hc, class: i32) {
        if (0..hc.dimension()).any(|n| hc[n].is_empty()) {
            return;
        }
        self.seq += 1;
        self.heap.push(BfItem { key: (-class, 0, self.seq), hc });
    }

    fn push(&mut self, hc: Hc, class: i32, fill: &mut BranchFill) {
        let _prof = crate::misc::prof::scope(11);
        self.need_branch(&hc, false, fill);
        let mut bound = 0i64;
        let shared = match &self.order {
            HcOrdering::Precomputed(o) => o.shared,
            HcOrdering::Natural => usize::MAX,
        };
        for n in 0..hc.dimension() {
            let sc = &self.scores[n.min(shared)];
            let set = &hc[n];
            let best = match &self.order {
                HcOrdering::Precomputed(_) if set.single().is_some_and(|i| self.universe[n].contains(i)) => {
                    set.single().map(|i| sc[i as usize])
                }
                // An order lists its axis' universe best first, so the first member of the
                // set has the set's best score.
                HcOrdering::Precomputed(o) if set.is_subset_of(&self.universe[n]) => {
                    o.axis(n).iter().find(|&&i| set.contains(i)).map(|&i| sc[i as usize])
                }
                _ => set.iter().map(|i| sc.get(i as usize).copied().unwrap_or(0)).max(),
            };
            match best {
                Some(b) => bound += b as i64,
                None => return,
            }
        }
        self.seq += 1;
        self.heap.push(BfItem { key: (-class, bound, self.seq), hc });
    }
}

struct BfItem {
    key: (i32, i64, u64),
    hc: Hc,
}
impl PartialEq for BfItem {
    fn eq(&self, o: &Self) -> bool {
        self.key == o.key
    }
}
impl Eq for BfItem {}
impl PartialOrd for BfItem {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for BfItem {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.key.cmp(&o.key)
    }
}

impl Default for BestFirstCursor {
    fn default() -> Self {
        BestFirstCursor {
            heap: BfHeap {
                heap: std::collections::BinaryHeap::new(),
                order: HcOrdering::Precomputed(AxisOrders::default()),
                scores: Vec::new(),
                universe: Hc::default(),
                seq: 0,
                branch_pending: false,
                first_branch: 0,
            },
            sc: PointScratch::default(),
            checks: CheckBuf::default(),
            lone: None,
            last_class: 0,
            limit_below: 0,
            limit_steps: 0,
            limit_class: -1,
            limit_taken: 0,
            budget: None,
            memo: ProblemMemo::default(),
            excluded: RegionMemo::default(),
            last: Point::new(),
        }
    }
}

impl BestFirstCursor {
    /// The per-axis orders (each listing its axis' universe coordinates, best first) and
    /// coordinate scores, to fill before `restart` (their storage is kept between uses).
    pub fn ordering_mut(&mut self) -> (&mut AxisOrders, &mut Vec<Vec<i32>>) {
        (self.heap.order.axes_mut(), &mut self.heap.scores)
    }

    /// Whether the branch axes' order is still to come (`next_with` asks for it when a
    /// hypercuboid first needs it); until then only their null coordinate is scored.
    pub fn set_branch_pending(&mut self, pending: bool) {
        self.heap.branch_pending = pending;
    }

    /// Forget the problems found so far (see `ProblemMemo`): call before enumerating another
    /// context. A `restart` on the same context keeps them.
    pub fn forget(&mut self) {
        self.memo.clear();
        self.excluded.clear();
    }

    /// Skip every point of `region` from now on (see `HcCursor::exclude`).
    pub fn exclude(&mut self, region: Slice) {
        self.excluded.add(region);
    }

    /// The point (coordinates) of the last action found.
    pub fn last_point(&self) -> &Point {
        &self.last
    }

    /// Per axis, each coordinate's ordering score.
    pub fn scores(&self) -> &[Vec<i32>] {
        &self.heap.scores
    }

    /// Enumerate `ss` (of the context `info`) from the start under the current ordering.
    pub fn restart(&mut self, info: &HcInfo, ss: SearchSpace) {
        self.limit_below = 0;
        self.limit_class = -1;
        self.heap.heap.clear();
        self.heap.seq = 0;
        self.heap.universe.clone_from(info.get_universe());
        self.heap.first_branch = info.first_branch_axis();
        let mut hcs = ss.hcs;
        // The first class is the last hypercuboid; with one playable board and every branch
        // axis null there, its points are the coordinates of axis 0 (see `next_with`).
        self.lone = None;
        if info.first_branch_axis() == 1 && matches!(self.heap.order, HcOrdering::Precomputed(_))
            && let Some(hc) = hcs.last()
                && !hc[0].is_empty() && (1..hc.dimension()).all(|n| hc[n].single() == Some(0)) {
                    let hc = hcs.pop().unwrap();
                    let dim = hc.dimension();
                    self.lone = Some(LoneWalk { hc, pos: 0, point: vec![0; dim] });
                }
        // (The classes count from the first, as if it were still there.)
        let n = hcs.len() + self.lone.is_some() as usize;
        for (k, hc) in hcs.into_iter().enumerate() {
            self.heap.push_initial(hc, (n - 1 - k) as i32);
        }
    }

    pub fn next(&mut self, info: &HcInfo, s: &State) -> Option<MoveSeq> {
        self.next_with(info, s, &mut |_, _| {}).expect("branch entries built")
    }

    /// The class of the last action found: the initial hypercuboid it came from, counted from
    /// the last one (0) back.
    pub fn last_class(&self) -> i32 {
        self.last_class
    }

    /// Until the next `restart`, each of the classes below `classes` (the first ones) is
    /// dropped once `steps` of its hypercuboids gave no action.
    pub fn limit_classes(&mut self, classes: i32, steps: u32) {
        self.limit_below = classes;
        self.limit_steps = steps;
        self.limit_class = -1;
    }

    /// Bound the enumeration to `steps` more hypercuboids (and walked points), or lift the bound:
    /// a count, so the same on any machine and for any time given to the search.
    pub fn set_budget(&mut self, steps: Option<u32>) {
        self.budget = steps;
    }

    /// Whether the bound of `set_budget` ran out (so that no action found is not the end).
    pub fn budget_spent(&self) -> bool {
        self.budget == Some(0)
    }

    /// Spend a step of the budget; false when none is left.
    fn spend(&mut self) -> bool {
        match &mut self.budget {
            None => true,
            Some(0) => false,
            Some(b) => {
                *b -= 1;
                true
            }
        }
    }

    /// Drop the rest of the actions of class `class`, the one being enumerated.
    pub fn drop_class(&mut self, class: i32) {
        while self.heap.heap.peek().is_some_and(|it| it.key.0 == -class) {
            self.heap.heap.pop();
        }
    }

    /// `next`, ordering pending branch axes with `fill` once needed; stops with `NeedBranch`
    /// (and no progress) when the next hypercuboid needs the branch entries `info` deferred.
    pub fn next_with(&mut self, info: &HcInfo, s: &State, fill: &mut BranchFill) -> Result<Option<MoveSeq>, NeedBranch> {
        if let Some(walk) = &mut self.lone {
            // Each live non-jump coordinate in axis order: the point a hypercuboid of this class
            // takes next (a problem only removes coordinates whose points all fail, a found
            // action only its own).
            let HcOrdering::Precomputed(o) = &self.heap.order else { unreachable!() };
            let order = o.axis(0);
            while walk.pos < order.len() {
                if !match &mut self.budget {
                    None => true,
                    Some(0) => false,
                    Some(b) => {
                        *b -= 1;
                        true
                    }
                } {
                    return Ok(None);
                }
                let i = order[walk.pos];
                walk.pos += 1;
                if !walk.hc[0].contains(i) || !matches!(info.entry(0, i), Entry::Physical { .. } | Entry::Null) {
                    continue;
                }
                walk.point[0] = i;
                if self.excluded.holding(&walk.point).is_some() {
                    continue;
                }
                if info.lone_point_legal(s, &walk.point, &mut self.checks) {
                    self.last_class = 0;
                    self.last.clone_from(&walk.point);
                    return Ok(Some(info.to_action(&walk.point)));
                }
            }
            self.lone = None;
        }
        let mut steps = 0u32;
        while !self.heap.heap.is_empty() {
            steps += 1;
            if interrupted_step(steps) || !self.spend() {
                return Ok(None);
            }
            // Long without an action: often none is left (a mate), which best-first proves
            // slowly (each remembered problem splits its hypercuboids again and again) and
            // depth-first quickly.
            if steps >= EXHAUST_CHECK_STEPS && steps.is_power_of_two() && self.budget.is_none() && self.none_left(info, s, steps) {
                self.heap.heap.clear();
                return Ok(None);
            }
            let BfItem { key, mut hc } = self.heap.heap.pop().unwrap();
            if info.needs_branch(&hc) {
                self.heap.heap.push(BfItem { key, hc });
                return Err(NeedBranch);
            }
            let class = -key.0;
            if class < self.limit_below {
                if class != self.limit_class {
                    self.limit_class = class;
                    self.limit_taken = 0;
                }
                self.limit_taken += 1;
                if self.limit_taken > self.limit_steps {
                    self.drop_class(class);
                    continue;
                }
            }
            let memo = info.first_branch_axis() >= PROBLEM_MEMO_MIN_AXES;
            if memo && self.memo.covers(&hc) || self.excluded.covers(&hc) {
                continue;
            }
            self.heap.need_branch(&hc, true, fill);
            if !info.take_point(&mut hc, &self.heap.order, &mut self.sc) {
                continue;
            }
            let pt = &self.sc.point;
            let heap = &mut self.heap;
            if let Some(region) = self.excluded.holding(pt) {
                hc.into_remove_slice_with(region, |h| heap.push(h, class, fill));
                continue;
            }
            let b0 = info.first_branch_axis();
            if let Some(problem) = memo.then(|| self.memo.holding(pt, b0)).flatten() {
                let _prof = crate::misc::prof::scope(5);
                problem.remove_from(hc, b0, |h| heap.push(h, class, fill));
                continue;
            }
            match info.find_problem(s, pt, &hc, &mut self.checks, false) {
                None => {
                    let act = info.to_action(pt);
                    hc.into_remove_point_with(pt, |h| heap.push(h, class, fill));
                    self.last_class = class;
                    self.last.clone_from(pt);
                    return Ok(Some(act));
                }
                Some(problem) => {
                    let _prof = crate::misc::prof::scope(5);
                    problem.remove_from(hc, b0, |h| heap.push(h, class, fill));
                    if memo {
                        self.memo.add(problem);
                    }
                }
            }
        }
        Ok(None)
    }
}

impl BestFirstCursor {
    /// Whether no legal action is left among the hypercuboids still queued (`none_left_in`).
    #[cold]
    #[inline(never)]
    fn none_left(&self, info: &HcInfo, s: &State, steps: u32) -> bool {
        none_left_in(self.heap.heap.iter().map(|it| it.hc.clone()).collect(), &self.excluded, info, s, steps)
    }
}

/// Whether `hcs` hold no legal action outside `excluded`: a natural-order search, with and then
/// without regions (each fastest on some mates), finds none within four times `steps` steps.
fn none_left_in(hcs: Vec<Hc>, excluded: &RegionMemo, info: &HcInfo, s: &State, steps: u32) -> bool {
    [true, false].into_iter().any(|regions| {
        let mut c = HcCursor::new(SearchSpace::from_vec(hcs.clone()), HcOrdering::Natural);
        c.excluded = excluded.clone();
        c.regions = regions;
        c.budget = steps.saturating_mul(4);
        matches!(c.next_with(info, s, &mut |_| {}), Ok(None)) && c.budget > 0 && !interrupted()
    })
}

/// Lazy enumeration of legal actions over a borrowed state.
pub struct HcSearch<'a> {
    info: &'a HcInfo,
    s: &'a State,
    cursor: HcCursor,
}

impl Iterator for HcSearch<'_> {
    type Item = MoveSeq;
    fn next(&mut self) -> Option<MoveSeq> {
        self.cursor.next(self.info, self.s)
    }
}
