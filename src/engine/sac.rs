//! The f7/f2 sac table.
//!
//! Most opening theory in Standard and Turn Zero is a queen or bishop travelling back onto
//! f7/f2 of a past board: a new timeline where the king faces a softmate attack 6-10 plies
//! deep on that one timeline, which the full search (where every other board doubles the
//! depth) cannot see. Past boards never change, so the outcome of such a travel depends on its
//! target board alone: it is worked out once per process by a small mate search on the new
//! timeline alone (`resolve`) and kept in tables shared by every search thread.
//!
//! The local search plays on one timeline only, the side to move always moving on its latest
//! board. A royal counts as attacked physically or by a piece travelling straight back along
//! the timeline onto a past board where the royal stood (the softmates of the f7 lines: after
//! 1. Qxf7+ Kxf7 .. 5. Qb3 the queen reaches the king's old square f7 through time, and only
//! taking the queen helps). A side that is attacked with no move on the board is mated: its
//! other escapes would create timelines, a softmate, which counts as a win. No move and no
//! attack is no mate. En passant is left out. Time-travel defences and the other timelines are
//! ignored, so a local win is a strong signal, not a proof: the evaluation terms built on it
//! (`hce`) are ones the main search can still override.
//!
//! Positions are keyed by their timeline's recent boards (the multiverse's own board keys, so
//! the evaluation reads them for free): `RESULTS` holds the outcome of each candidate travel
//! and of the latest boards of created timelines; `PROOF` every position of a local mate proof
//! with its distance, so the evaluation can follow a sac line in progress move by move.

use crate::core::bitboard::*;
use crate::core::board::Board;
use crate::core::hypercuboid::has_physical_check;
use crate::core::multiverse::{board_key, Multiverse};
use crate::core::piece::*;
use crate::core::state::State;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

/// Attacker moves the local search looks for a mate in (after the defender's first reply to
/// the sac: 1. Bxf7+ Kxf7 2. Nh3 .. 3. Ng5+ .. 4. c3 .. 5. Qb3* is four).
pub const MAX_MOVES: u32 = 4;
/// Local nodes one resolution may use before its result is left unknown.
pub const NODE_BUDGET: u64 = 400_000;
/// Turns back from the present a travel target may lie, and turns a royal attack through time
/// is looked for (a slide crosses at most 7 squares).
const MAX_BACK: i32 = 7;
/// Boards of a timeline (the latest one and those before it) a position key covers.
const WINDOW: usize = 2 * MAX_BACK as usize + 1;

// ------------------------------------------------------------------ tables

const RES_BITS: u32 = 14;
const PROOF_BITS: u32 = 20;
/// Lock-free slots: [key ^ data, data] (a torn pair never matches its key).
static RESULTS: [AtomicU64; 2 << RES_BITS] = [const { AtomicU64::new(0) }; 2 << RES_BITS];
static PROOF: [AtomicU64; 2 << PROOF_BITS] = [const { AtomicU64::new(0) }; 2 << PROOF_BITS];
/// Whether `PROOF` holds anything, and `TRAVELS` (while neither does, every term is skipped).
static ANY_PROOF: AtomicBool = AtomicBool::new(false);
static ANY_TRAVEL: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// `attacker` mates within `plies` from the position (the side to move's move included).
    Win { attacker: bool, plies: u8 },
    /// No mate within the searched depth.
    NoMate,
    /// The node budget ran out.
    Unknown,
}

fn encode(o: Outcome) -> u64 {
    match o {
        Outcome::Win { attacker, plies } => 1 | (attacker as u64) << 2 | (plies as u64) << 8,
        Outcome::NoMate => 2,
        Outcome::Unknown => 3,
    }
}

fn decode(d: u64) -> Outcome {
    match d & 3 {
        1 => Outcome::Win { attacker: d >> 2 & 1 != 0, plies: (d >> 8) as u8 },
        2 => Outcome::NoMate,
        _ => Outcome::Unknown,
    }
}

#[inline]
fn slot(bits: u32, key: u64) -> usize {
    ((key >> 20) as usize & ((1 << bits) - 1)) * 2
}

#[inline]
fn get(t: &[AtomicU64], bits: u32, key: u64) -> Option<Outcome> {
    let i = slot(bits, key);
    let d = t[i + 1].load(Ordering::Relaxed);
    if d != 0 && t[i].load(Ordering::Relaxed) ^ d == key { Some(decode(d)) } else { None }
}

#[inline]
fn put(t: &[AtomicU64], bits: u32, key: u64, o: Outcome) {
    let i = slot(bits, key);
    let d = encode(o);
    t[i].store(key ^ d, Ordering::Relaxed);
    t[i + 1].store(d, Ordering::Relaxed);
}

/// Whether any local win is known (else every term is zero and is skipped).
#[inline]
pub fn active() -> bool {
    any_proof() || any_travel()
}

#[inline]
pub fn any_proof() -> bool {
    ANY_PROOF.load(Ordering::Relaxed)
}

#[inline]
pub fn any_travel() -> bool {
    ANY_TRAVEL.load(Ordering::Relaxed)
}

#[inline(always)]
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// Position key from the board keys (`multiverse::board_key`) of a timeline's boards, oldest
/// first: the last `WINDOW` of them.
#[inline]
fn window_key(keys: &[u64]) -> u64 {
    let mut k = 0x2545f4914f6cdd1d;
    for &x in &keys[keys.len().saturating_sub(WINDOW)..] {
        k ^= x;
    }
    mix(k)
}

/// `PROOF` key of a position key: its low bits carry the outcome in the precomputed table.
#[inline]
fn proof_key(k: u64) -> u64 {
    k & !31
}

/// `RESULTS` key of "can `attacker` mate in the position `pos`".
#[inline]
fn result_key(pos: u64, attacker: bool) -> u64 {
    pos ^ if attacker { 0x5851f42d4c957f2d } else { 0x1d8e4e27c47d124f }
}

/// The local mate proven for the position at the end of timeline `l`, if any:
/// (attacker, plies).
#[inline]
pub fn line_win(m: &Multiverse, l: i32) -> Option<(bool, u8)> {
    match get(&PROOF, PROOF_BITS, proof_key(window_key(m.timeline(l).keys()))) {
        Some(Outcome::Win { attacker, plies }) => Some((attacker, plies)),
        _ => None,
    }
}

/// The board a travel onto `sq` of `target` creates (the defender to move on it).
pub fn branch(target: &Board, sq: i32, queen: bool, attacker: bool) -> Board {
    let p = if queen { QUEEN_W } else { BISHOP_W };
    let mut b = *target;
    b.set_piece(sq, if attacker { to_black(p) } else { p });
    b
}

/// The line a travel of `c` creates next.
#[inline]
fn new_line(m: &Multiverse, c: bool) -> i32 {
    let (lo, hi) = m.get_lines_range();
    if c { lo - 1 } else { hi + 1 }
}

// ------------------------------------------------------------------ local mate search

const PROMO: u8 = 1;
const CASTLE: u8 = 2;
/// The piece leaves the board by time travel (its arrival elsewhere is not modelled).
const DEPART: u8 = 4;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Mv {
    from: u8,
    to: u8,
    flags: u8,
}

impl Mv {
    #[inline]
    fn code(self) -> u16 {
        self.from as u16 | (self.to as u16) << 6 | if self.flags & DEPART != 0 { 1 << 15 } else { 0 }
    }
}

/// Pseudo-legal moves of `c` on `b` (board height `h`).
fn gen_moves(b: &Board, c: bool, h: i32, out: &mut Vec<Mv>) {
    let own = b.friendly(c);
    let theirs = b.hostile(c);
    let occ = b.occupied();
    let last_rank = if c { 0 } else { h - 1 };
    for from in marked_pos(b.hostile(!c)) {
        let z = pmask(from);
        let mut t: Bitboard = 0;
        if b.lpawn() & z != 0 {
            let push = if c { shift_south(z) } else { shift_north(z) } & !occ;
            t |= push;
            if push != 0 && b.umove() & z != 0 {
                t |= if c { shift_south(push) } else { shift_north(push) } & !occ;
            }
            t |= if c { black_pawn_attack(from) } else { white_pawn_attack(from) } & theirs;
            for to in marked_pos(t) {
                let flags = if to >> 3 == last_rank { PROMO } else { 0 };
                out.push(Mv { from: from as u8, to: to as u8, flags });
            }
            continue;
        }
        if b.lknight() & z != 0 {
            t |= knight_attack(from) & !own;
        }
        if b.lking() & z != 0 {
            t |= king_attack(from) & !own;
            if b.royal() & b.umove() & z != 0 && !b.is_under_attack(from, c) {
                // Castling, as in the rules core: the first piece along the rank is an unmoved
                // rook at least two squares away; the king's two squares are not attacked.
                let (x, y) = (from & 7, from >> 3);
                for dx in [1, -1] {
                    let rx = b.first_occupied_x(x, y, dx);
                    if (0..BOARD_LENGTH).contains(&rx)
                        && (rx - x).abs() >= 2
                        && b.umove() & b.rook() & own & pmask(rx + 8 * y) != 0
                        && !b.is_under_attack(x + dx + 8 * y, c)
                        && !b.is_under_attack(x + 2 * dx + 8 * y, c)
                    {
                        out.push(Mv { from: from as u8, to: (x + 2 * dx + 8 * y) as u8, flags: CASTLE });
                    }
                }
            }
        }
        if b.lrook() & z != 0 {
            t |= rook_attack(from, occ) & !own;
        }
        if b.lbishop() & z != 0 {
            t |= bishop_attack(from, occ) & !own;
        }
        for to in marked_pos(t) {
            out.push(Mv { from: from as u8, to: to as u8, flags: 0 });
        }
    }
}

fn play(b: &Board, m: Mv, c: bool) -> Board {
    let mut nb = *b;
    let (from, to) = (m.from as i32, m.to as i32);
    if m.flags & DEPART != 0 {
        nb.clear_square(from);
    } else if m.flags & PROMO != 0 {
        nb.clear_square(from);
        nb.set_piece(to, if c { to_black(QUEEN_W) } else { QUEEN_W });
    } else if m.flags & CASTLE != 0 {
        let (x, y) = (from & 7, from >> 3);
        let dx = ((to & 7) - x).signum();
        let rx = b.first_occupied_x(x, y, dx);
        nb.transfer(rx + 8 * y, x + dx + 8 * y);
        nb.transfer(from, to);
    } else {
        nb.transfer(from, to);
    }
    nb
}

/// One step forward in time: (x, y) square steps, as (mask of squares that may move, shift).
const DIRS: [(i32, i32); 9] = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];

#[inline(always)]
fn step(b: Bitboard, dx: i32, dy: i32) -> Bitboard {
    let b = match dx {
        1 => shift_east(b),
        -1 => shift_west(b),
        2 => shift_east(shift_east(b)),
        -2 => shift_west(shift_west(b)),
        _ => b,
    };
    match dy {
        1 => b << 8,
        -1 => b >> 8,
        2 => b << 16,
        -2 => b >> 16,
        _ => b,
    }
}

/// Squares of a board from which a piece of one side would take an enemy royal on a past board
/// of the timeline: by the pieces' movement components (rook: straight back in time; bishop:
/// time and one more axis; unicorn: time and both), knights and kings.
#[derive(Clone, Copy, Default)]
struct TimeReach {
    axis: [Bitboard; 3],
    knight: Bitboard,
    king: Bitboard,
}

impl TimeReach {
    /// Whether `by`'s pieces on `b` hit anything in the reach.
    #[inline]
    fn hits(&self, b: &Board, by: bool) -> bool {
        let mine = b.hostile(!by);
        ((b.lrook() & self.axis[0]) | (b.lbishop() & self.axis[1]) | (b.lunicorn() & self.axis[2]) | (b.lknight() & self.knight) | (b.lking() & self.king)) & mine != 0
    }
}

/// `TimeReach` of `by` for a board that `by` moves from at `stack[q]` (which need not exist
/// yet): the past boards `by` moves from are `stack[q - 2]`, `stack[q - 4]`, ...
fn time_reach(stack: &[Board], by: bool, q: usize) -> TimeReach {
    let mut r = TimeReach::default();
    let back = (q / 2).min(MAX_BACK as usize);
    if back == 0 {
        return r;
    }
    let royals = |j: usize| {
        let b = &stack[q - 2 * j];
        b.royal() & b.hostile(by)
    };
    if (1..=back).all(|j| royals(j) == 0) {
        return r;
    }
    for &(dx, dy) in &DIRS {
        let axes = (dx != 0) as usize + (dy != 0) as usize;
        // Rays from the royals, walked forward in time to the board at `q`.
        let mut s = 0;
        for j in (1..=back).rev() {
            s |= royals(j);
            s = step(s, dx, dy);
            if j > 1 {
                s &= !stack[q - 2 * (j - 1)].occupied();
            }
        }
        r.axis[axes] |= s;
    }
    let r1 = royals(1);
    for &(dx, dy) in &[(2, 0), (-2, 0), (0, 2), (0, -2)] {
        r.knight |= step(r1, dx, dy);
    }
    if back >= 2 {
        let r2 = royals(2);
        for &(dx, dy) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
            r.knight |= step(r2, dx, dy);
        }
    }
    for &(dx, dy) in &DIRS {
        r.king |= step(r1, dx, dy);
    }
    r
}

#[derive(Clone, Copy, Default)]
struct Slot {
    key: u64,
    /// Smallest attacker-move count known to mate, plus one (0: none).
    win: u8,
    /// Largest attacker-move count known not to, plus one (0: none).
    fail: u8,
    /// Whether the attacker is to move.
    or: bool,
    best: u16,
}

const TT_BITS: u32 = 18;
/// Positions one proof may store in `PROOF`.
const PROOF_CAP: usize = 1 << 16;
const MAX_PLY: usize = 2 * MAX_MOVES as usize + 4;

struct Local {
    tt: Vec<Slot>,
    nodes: u64,
    budget: u64,
    deadline: Option<Instant>,
    /// Set from outside to stop (null: none); only read during `resolve`.
    stop: *const AtomicBool,
    aborted: bool,
    /// Aborted by the deadline or the stop flag (not the node budget).
    timed_out: bool,
    /// The attacking side.
    a: bool,
    h: i32,
    /// The timeline, and the half-turn of `stack[0]`.
    l: i32,
    v0: i32,
    /// The timeline's boards: its history, then the searched line.
    stack: Vec<Board>,
    keys: Vec<u64>,
    /// Per ply: (order key, move, board after it).
    lists: Vec<Vec<(i32, Mv, Board)>>,
    scratch: Vec<Mv>,
    /// Per side, from * 64 + to: refutations and mating moves found.
    hist: Vec<i32>,
}

thread_local! {
    static LOCAL: RefCell<Option<Local>> = const { RefCell::new(None) };
}

fn value(b: &Board, sq: i32) -> i32 {
    match b.kind(sq) {
        PAWN_W | BRAWN_W => 1,
        KNIGHT_W | BISHOP_W => 3,
        ROOK_W => 5,
        QUEEN_W | PRINCESS_W => 9,
        NO_PIECE => 0,
        _ => 2,
    }
}

impl Local {
    fn new() -> Local {
        Local {
            tt: vec![Slot::default(); 1 << TT_BITS],
            nodes: 0,
            budget: 0,
            deadline: None,
            stop: std::ptr::null(),
            aborted: false,
            timed_out: false,
            a: false,
            h: 8,
            l: 0,
            v0: 0,
            stack: Vec::with_capacity(64),
            keys: Vec::with_capacity(64),
            lists: (0..MAX_PLY + 2).map(|_| Vec::with_capacity(64)).collect(),
            scratch: Vec::with_capacity(128),
            hist: vec![0; 2 * 64 * 64],
        }
    }

    #[inline]
    fn push(&mut self, b: Board) {
        let v = self.v0 + self.stack.len() as i32;
        self.keys.push(board_key(self.l, v, &b));
        self.stack.push(b);
    }

    #[inline]
    fn pop(&mut self) {
        self.keys.pop();
        self.stack.pop();
    }

    #[inline]
    fn probe(&self, k: u64, n: u32) -> (Option<bool>, u16) {
        let e = &self.tt[(k as usize) & ((1 << TT_BITS) - 1)];
        if e.key != k {
            return (None, 0);
        }
        if e.win != 0 && (e.win - 1) as u32 <= n {
            return (Some(true), e.best);
        }
        if e.fail != 0 && (e.fail - 1) as u32 >= n {
            return (Some(false), e.best);
        }
        (None, e.best)
    }

    #[inline]
    fn store(&mut self, k: u64, or: bool, n: u32, win: bool, best: u16) {
        let e = &mut self.tt[(k as usize) & ((1 << TT_BITS) - 1)];
        if e.key != k {
            *e = Slot { key: k, or, ..Slot::default() };
        }
        let v = n.min(250) as u8 + 1;
        if win {
            if e.win == 0 || v < e.win {
                e.win = v;
            }
        } else if v > e.fail {
            e.fail = v;
        }
        if best != 0 {
            e.best = best;
        }
    }

    /// Whether the side to move on the latest board is attacked (a royal capturable if it
    /// passed), physically or through time.
    fn attacked(&self) -> bool {
        let q = self.stack.len() - 1;
        let b = &self.stack[q];
        let stm = (self.v0 + q as i32) & 1 != 0;
        has_physical_check(b, stm) || time_reach(&self.stack, !stm, q + 1).hits(b, !stm)
    }

    /// Legal moves of `c` (to move on the latest board) into `lists[ply]`, best first; only
    /// attacking ones when `checks_only`. Returns how many.
    fn moves(&mut self, c: bool, ply: usize, checks_only: bool, best: u16) -> usize {
        let q = self.stack.len();
        let b = self.stack[q - 1];
        // The opponent moves from the boards the moves create (at `q`); `c` next from `q + 1`.
        let theirs = time_reach(&self.stack, !c, q);
        let mine = time_reach(&self.stack, c, q + 1);
        self.scratch.clear();
        let mut scratch = std::mem::take(&mut self.scratch);
        gen_moves(&b, c, self.h, &mut scratch);
        if c != self.a {
            // The defender's travels away (which create timelines elsewhere): on this board a
            // non-pawn piece just leaves. That is all the local search knows of time-travel
            // defences, and it keeps the boards they leave behind in the proofs.
            for from in marked_pos(b.hostile(!c) & !b.lpawn() & !b.royal()) {
                scratch.push(Mv { from: from as u8, to: from as u8, flags: DEPART });
            }
        }
        let mut list = std::mem::take(&mut self.lists[ply]);
        list.clear();
        let attacking = c == self.a;
        let enemy_royal = marked_pos(b.royal() & b.hostile(c)).next();
        for &m in &scratch {
            let nb = play(&b, m, c);
            if has_physical_check(&nb, c) || theirs.hits(&nb, !c) {
                continue;
            }
            let check = has_physical_check(&nb, !c) || mine.hits(&nb, c);
            if checks_only && !check {
                continue;
            }
            let mut sc = self.hist[(c as usize) << 12 | (m.from as usize) << 6 | m.to as usize].min(1 << 14);
            let victim = if m.flags & DEPART != 0 { 0 } else { value(&b, m.to as i32) };
            if m.code() == best {
                sc += 1 << 24;
            }
            if m.flags & DEPART != 0 {
                sc -= 1 << 12;
            }
            if victim > 0 {
                sc += (1 << 16) + 64 * victim - value(&b, m.from as i32);
            }
            if attacking {
                if check {
                    sc += 1 << 20;
                }
                // Closer to the enemy royal first.
                if let Some(k) = enemy_royal {
                    let d = ((k & 7) - (m.to as i32 & 7)).abs().max(((k >> 3) - (m.to as i32 >> 3)).abs());
                    sc += 8 * (8 - d.min(8));
                }
            }
            list.push((sc, m, nb));
        }
        list.sort_unstable_by(|x, y| y.0.cmp(&x.0));
        let n = list.len();
        self.lists[ply] = list;
        self.scratch = scratch;
        n
    }

    #[inline]
    fn tick(&mut self) -> bool {
        self.nodes += 1;
        if self.nodes > self.budget {
            self.aborted = true;
        }
        // SAFETY: `stop` outlives the `resolve` call that set it.
        if self.nodes & 1023 == 0 && (self.deadline.is_some_and(|d| Instant::now() >= d) || (!self.stop.is_null() && unsafe { &*self.stop }.load(Ordering::Relaxed))) {
            self.aborted = true;
            self.timed_out = true;
        }
        self.aborted
    }

    /// Attacker to move: whether it mates within `n` (>= 1) of its moves.
    fn or_node(&mut self, n: u32, ply: usize) -> bool {
        if self.tick() {
            return false;
        }
        let k = window_key(&self.keys);
        let (hit, best) = self.probe(k, n);
        if let Some(r) = hit {
            return r;
        }
        if ply >= MAX_PLY {
            return false;
        }
        let count = self.moves(self.a, ply, n == 1, best);
        for i in 0..count {
            let (_, m, nb) = self.lists[ply][i];
            self.push(nb);
            let r = self.and_node(n - 1, ply + 1);
            self.pop();
            if self.aborted {
                return false;
            }
            if r {
                let hi = (self.a as usize) << 12 | (m.from as usize) << 6 | m.to as usize;
                self.hist[hi] += (n * n) as i32;
                self.store(k, true, n, true, m.code());
                return true;
            }
        }
        self.store(k, true, n, false, 0);
        false
    }

    /// Defender to move: whether the attacker mates within `n` of its moves.
    fn and_node(&mut self, n: u32, ply: usize) -> bool {
        if self.tick() {
            return false;
        }
        let d = !self.a;
        let k = window_key(&self.keys);
        let (hit, best) = self.probe(k, n);
        if let Some(r) = hit {
            return r;
        }
        if ply >= MAX_PLY {
            return false;
        }
        let count = self.moves(d, ply, false, best);
        if count == 0 {
            // Mated now, or stalemated (never a mate).
            let mate = self.attacked();
            self.store(k, false, if mate { 0 } else { 250 }, mate, 0);
            return mate;
        }
        if n == 0 {
            self.store(k, false, 0, false, 0);
            return false;
        }
        for i in 0..count {
            let (_, m, nb) = self.lists[ply][i];
            self.push(nb);
            let r = self.or_node(n, ply + 1);
            self.pop();
            if self.aborted {
                return false;
            }
            if !r {
                let hi = (d as usize) << 12 | (m.from as usize) << 6 | m.to as usize;
                self.hist[hi] += (n * n) as i32;
                self.store(k, false, n, false, m.code());
                return false;
            }
        }
        self.store(k, false, n, true, 0);
        true
    }

    /// Store the proof tree of a mate within `n` from the current position (attacker to move
    /// when `or`) in `PROOF`: every defender reply, and the attacker's mating move (from the
    /// table, searched again where the table lost it). Returns false when out of budget.
    fn extract(&mut self, n: u32, or: bool, ply: usize, stored: &mut usize) -> bool {
        if self.aborted || *stored >= PROOF_CAP || ply >= MAX_PLY {
            return false;
        }
        let k = window_key(&self.keys);
        let (hit, mut best) = self.probe(k, n);
        // The shortest mate the table knows here.
        let e = &self.tt[(k as usize) & ((1 << TT_BITS) - 1)];
        let n = if e.key == k && e.win != 0 { ((e.win - 1) as u32).min(n) } else { n };
        let plies = if or { 2 * n - 1 } else { 2 * n };
        put(&PROOF, PROOF_BITS, proof_key(k), Outcome::Win { attacker: self.a, plies: plies as u8 });
        *stored += 1;
        if or {
            if hit != Some(true) || best == 0 {
                if !self.or_node(n, ply) {
                    return false;
                }
                best = self.probe(k, n).1;
            }
            let count = self.moves(self.a, ply, n == 1, best);
            let Some(&(_, _, nb)) = self.lists[ply][..count].iter().find(|x| x.1.code() == best) else { return false };
            self.push(nb);
            let r = self.extract(n - 1, false, ply + 1, stored);
            self.pop();
            r
        } else {
            if n == 0 {
                return true;
            }
            let count = self.moves(!self.a, ply, false, 0);
            for i in 0..count {
                let nb = self.lists[ply][i].2;
                self.push(nb);
                let r = self.extract(n, true, ply + 1, stored);
                self.pop();
                if !r {
                    return false;
                }
            }
            true
        }
    }
}

/// Whether `attacker` can force a mate on timeline `l` alone within `max_moves` of its moves,
/// searching at most `budget` nodes. `history`: the timeline's boards up to the position (the
/// side to move moves on the last one), the first at half-turn `v0`. Wins store every proof
/// position in `PROOF`. Returns the outcome, the nodes searched, and whether `deadline` or
/// `stop` cut the search short (its unknown outcome is then no verdict on the position).
#[allow(clippy::too_many_arguments)]
pub fn resolve(history: &[Board], l: i32, v0: i32, attacker: bool, h: i32, max_moves: u32, budget: u64, deadline: Option<Instant>, stop: Option<&AtomicBool>) -> (Outcome, u64, bool) {
    LOCAL.with(|cell| {
        let mut opt = cell.borrow_mut();
        let lc = opt.get_or_insert_with(Local::new);
        lc.tt.iter_mut().for_each(|e| *e = Slot::default());
        lc.hist.iter_mut().for_each(|x| *x = 0);
        lc.nodes = 0;
        lc.budget = budget;
        lc.deadline = deadline;
        lc.stop = stop.map_or(std::ptr::null(), |s| s as *const AtomicBool);
        lc.aborted = false;
        lc.timed_out = false;
        lc.a = attacker;
        lc.h = h;
        lc.l = l;
        lc.stack.clear();
        lc.keys.clear();
        // Only the boards a royal attack through time can reach matter.
        let from = history.len().saturating_sub(WINDOW);
        lc.v0 = v0 + from as i32;
        for &b in &history[from..] {
            lc.push(b);
        }
        let stm = (lc.v0 + lc.stack.len() as i32 - 1) & 1 != 0;
        let or_root = stm == attacker;
        let mut outcome = Outcome::NoMate;
        let first = if or_root { 1 } else { 0 };
        for n in first..=max_moves {
            let r = if or_root { lc.or_node(n, 0) } else { lc.and_node(n, 0) };
            if lc.aborted {
                outcome = Outcome::Unknown;
                break;
            }
            if r {
                let plies = if or_root { 2 * n - 1 } else { 2 * n };
                outcome = Outcome::Win { attacker, plies: plies as u8 };
                break;
            }
        }
        if let Outcome::Win { plies, .. } = outcome {
            export_proof(lc);
            let n = (plies as u32).div_ceil(2);
            let mut stored = 0;
            let complete = lc.extract(n, or_root, 0, &mut stored);
            if std::env::var_os("SAC_VERBOSE").is_some() {
                eprintln!("sac proof: {} positions{}", stored, if complete { "" } else { " (cut short)" });
            }
        }
        lc.stop = std::ptr::null();
        (outcome, lc.nodes, lc.timed_out)
    })
}

/// Store the local table's proven wins in `PROOF`, with their distance: an entry with the
/// attacker to move mates in 2n - 1 plies, one with the defender to move in 2n.
fn export_proof(l: &Local) {
    let mut stored = false;
    for e in &l.tt {
        if e.key != 0 && e.win != 0 {
            let n = (e.win - 1) as u32;
            let plies = if e.or { 2 * n - 1 } else { 2 * n };
            put(&PROOF, PROOF_BITS, proof_key(e.key), Outcome::Win { attacker: l.a, plies: plies as u8 });
            stored = true;
        }
    }
    if stored {
        ANY_PROOF.store(true, Ordering::Relaxed);
    }
}

// ------------------------------------------------------------------ travel targets

/// Half-turn from which color `c`'s pieces on a line ending at `end` move next (the latest
/// board's when it is `c`'s turn there, else the one after, which the latest board stands in
/// for).
#[inline(always)]
fn src_v(end: i32, c: bool) -> i32 {
    if (end & 1 != 0) == c { end } else { end + 1 }
}

/// Board of line `l` at half-turn `v`; one past the end is the latest board.
#[inline(always)]
fn at(m: &Multiverse, l: i32, v: i32) -> Option<&Board> {
    let (lo, hi) = m.get_lines_range();
    if l < lo || l > hi {
        return None;
    }
    let tl = m.timeline(l);
    let k = v - tl.start;
    let n = tl.boards.len() as i32;
    if k < 0 || k > n {
        None
    } else if k == n {
        tl.boards.last()
    } else {
        Some(&tl.boards[k as usize])
    }
}

/// The f7 squares of attacker `c` on line `l`: on its past boards (up to `MAX_BACK` turns
/// back), the squares next to a defender royal that no other defender piece covers and where a
/// queen (any of them) or a bishop (the diagonal ones) gives check on landing. `f(v, sq, queen,
/// board)`.
fn line_targets(m: &Multiverse, l: i32, c: bool, mut f: impl FnMut(i32, i32, bool, &Board)) {
    let tl = m.timeline(l);
    let vs = src_v(tl.end(), c);
    for k in 1..=MAX_BACK {
        let v = vs - 2 * k;
        if v < tl.start {
            break;
        }
        let b = &tl.boards[(v - tl.start) as usize];
        let royals = b.royal() & b.hostile(c);
        for r in marked_pos(royals) {
            for sq in marked_pos(king_attack(r) & !b.friendly(c)) {
                if b.attacks_to(sq) & b.hostile(c) & !b.royal() != 0 {
                    continue;
                }
                f(v, sq, true, b);
                if (sq & 7) != (r & 7) && (sq >> 3) != (r >> 3) {
                    f(v, sq, false, b);
                }
            }
        }
    }
}

/// The routes of a queen (`queen`) or bishop travel of `c` onto `sq` of its past board at
/// half-turn `v` of line `l`: forward in time from the target, one turn and at most one square
/// per axis per step, every past board on the way with its square empty. Calls `f(square,
/// goes_on)` with the square each open route reaches on the side's latest board there, and
/// whether it goes on (on the board) one turn later.
fn routes(m: &Multiverse, l: i32, c: bool, v: i32, sq: i32, queen: bool, mut f: impl FnMut(i32, bool)) {
    let (w, h) = m.get_board_size();
    let vs = src_v(m.timeline_end_v(l), c);
    let dirs = if queen { &DIRS[..] } else { &DIRS[1..5] };
    let (x0, y0) = (sq & 7, sq >> 3);
    let on = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h;
    'dir: for &(dx, dy) in dirs {
        let mut j = 1;
        loop {
            let (x, y) = (x0 + j * dx, y0 + j * dy);
            if !on(x, y) {
                continue 'dir;
            }
            let vj = v + 2 * j;
            if vj == vs {
                f(x + 8 * y, on(x + dx, y + dy));
                continue 'dir;
            }
            match at(m, l, vj) {
                Some(b) if b.occupied() & pmask(x + 8 * y) == 0 => j += 1,
                _ => continue 'dir,
            }
        }
    }
}

//// The winning travels each side has at the root of a search, worked out once there: per
/// (line, side), the squares of the side's latest board on the line from where a queen or
/// bishop travels onto a target whose travel is a known local win. Past boards do not change
/// inside a search, so a node only checks that the side still moves from that same board
/// (`vs`) and that the piece is still on such a square (`root_terms`).
#[derive(Clone, Copy, Default, Debug)]
pub struct RootTravels {
    pub n: u8,
    pub e: [RootTravel; 4],
}

#[derive(Clone, Copy, Default, Debug)]
pub struct RootTravel {
    pub l: i32,
    pub vs: i32,
    pub c: bool,
    pub queen_from: Bitboard,
    pub bishop_from: Bitboard,
    /// Squares of that board that must stay empty for a queen's (bishop's) route to such a
    /// target to go on to a later board.
    pub queen_open: Bitboard,
    pub bishop_open: Bitboard,
}

/// The squares of `c`'s latest board on line `l` from where a queen or bishop travel lands on
/// a target with a known winning outcome (`TRAVELS`), and those of the routes going on.
fn win_sources(m: &Multiverse, l: i32, c: bool) -> RootTravel {
    let mut e = RootTravel { l, vs: src_v(m.timeline_end_v(l), c), c, ..RootTravel::default() };
    let tl = m.timeline(l);
    let vs = src_v(tl.end(), c);
    for k in 1..=MAX_BACK {
        let v = vs - 2 * k;
        if v < tl.start {
            break;
        }
        let i = (v - tl.start) as usize;
        let (brd, bk) = (&tl.boards[i], tl.keys()[i]);
        let royals = brd.royal() & brd.hostile(c);
        if royals == 0 {
            continue;
        }
        let mut zone = 0;
        for r in marked_pos(royals) {
            zone |= king_attack(r);
        }
        for sq in marked_pos(zone & !brd.friendly(c)) {
            for queen in [true, false] {
                if known_travel(travel_key(bk, sq, queen, c)) {
                    routes(m, l, c, v, sq, queen, |from, goes_on| {
                        let (now, open) = if queen { (&mut e.queen_from, &mut e.queen_open) } else { (&mut e.bishop_from, &mut e.bishop_open) };
                        *now |= pmask(from);
                        if goes_on {
                            *open |= pmask(from);
                        }
                    });
                }
            }
        }
    }
    e
}

/// Whether the f7 sac table's terms apply to games from the start of `s`: Turn Zero starts
/// (an original timeline with a board at turn 0, Black to move), where both sides' f7/f2 travels
/// land on boards that are still the start position. In Standard-like starts only White gets
/// such a target, and the terms cost Black more than they saved (SPRT of f7_sac_defense: 0.36
/// as Black in Standard, 0.38 in Q/N swapped).
pub fn turn_zero(s: &State) -> bool {
    let (l0, l1) = s.get_initial_lines_range();
    (l0..=l1).any(|l| s.get_board_ptr(l, 0, true).is_some())
}

/// `RootTravels` of the position `s` (empty unless a winning travel is known).
pub fn root_travels(s: &State) -> RootTravels {
    let mut rt = RootTravels::default();
    if !any_travel() {
        return rt;
    }
    let m = s.multiverse();
    let (amin, amax) = s.get_active_range();
    for l in amin..=amax {
        for c in [false, true] {
            let e = win_sources(m, l, c);
            if e.queen_from | e.bishop_from != 0 && (rt.n as usize) < rt.e.len() {
                rt.e[rt.n as usize] = e;
                rt.n += 1;
            }
        }
    }
    rt
}

/// At a node of the search, from `rt`, per side (white, black): the winning travels it can still
/// play (its latest board on the line is the root's, the piece still on its square), and its
/// open routes to winning targets (the square still empty, a piece of the kind on the board).
#[inline]
pub fn root_terms(m: &Multiverse, rt: &RootTravels) -> ([i32; 2], [i32; 2]) {
    let (mut now, mut open) = ([0; 2], [0; 2]);
    let (lo, hi) = m.get_lines_range();
    for e in &rt.e[..rt.n as usize] {
        if e.l < lo || e.l > hi || src_v(m.timeline_end_v(e.l), e.c) != e.vs {
            continue;
        }
        let Some(src) = at(m, e.l, e.vs) else { continue };
        let mine = src.hostile(!e.c);
        let (q, b) = (src.queen() & mine, src.bishop() & mine);
        if (q & e.queen_from) | (b & e.bishop_from) != 0 {
            now[e.c as usize] += 1;
        }
        let empty = !src.occupied();
        open[e.c as usize] += (if q != 0 { e.queen_open & empty } else { 0 } | if b != 0 { e.bishop_open & empty } else { 0 }).count_ones() as i32;
    }
    (now, open)
}

// ------------------------------------------------------------------ the search's use

/// Known winning travels, by `travel_key` (small enough to ask at every move and evaluation).
const TRAVEL_BITS: u32 = 10;
static TRAVELS: [AtomicU64; 1 << TRAVEL_BITS] = [const { AtomicU64::new(0) }; 1 << TRAVEL_BITS];

#[inline]
fn known_travel(k: u64) -> bool {
    TRAVELS[(k >> 40) as usize & ((1 << TRAVEL_BITS) - 1)].load(Ordering::Relaxed) == k
}

/// Key of a queen (`queen`) or bishop travel of `c` onto `sq` of the board with multiverse key
/// `board` (`Multiverse::key_of`).
#[inline]
fn travel_key(board: u64, sq: i32, queen: bool, c: bool) -> u64 {
    mix(board ^ (sq as u64 | (queen as u64) << 6 | (c as u64) << 7).wrapping_mul(0x9e3779b97f4a7c15)) | 1
}

/// What one `prepare` call did.
#[derive(Default, Debug, Clone, Copy)]
pub struct PrepStats {
    pub resolved: u32,
    pub wins: u32,
    pub nodes: u64,
}

/// Local nodes of one probe in a game (about a millisecond): the precomputed table
/// (`data/fsac_table.bin`) holds the opening's usual targets, a probe only covers the rest.
pub const PROBE_NODES: u64 = 5_000;
/// Travel targets on boards past this turn are left alone (the theory lives in the opening).
const MAX_TARGET_TURN: i32 = 8;

fn note_travel(tk: Option<u64>, o: Outcome) {
    if let (Some(k), Outcome::Win { .. }) = (tk, o) {
        TRAVELS[(k >> 40) as usize & ((1 << TRAVEL_BITS) - 1)].store(k, Ordering::Relaxed);
        ANY_TRAVEL.store(true, Ordering::Relaxed);
    }
}

/// Work out the local outcomes the position may need, most urgent first, with at most
/// `probe_nodes` local nodes per probe and `budget` in all, before `deadline` and until `stop`
/// is set: the latest boards of active created timelines (for both sides) with no known proof,
/// then the f7 travels a side can play now, then those it can line up with one move. Only
/// targets up to turn `MAX_TARGET_TURN` with the defender's royal on its home rank. Known
/// outcomes (this process's or the precomputed table's) are not searched again. With `all`,
/// every f7 target of the past boards is worked out whatever its routes (for the precomputed
/// table).
pub fn prepare(s: &State, budget: u64, deadline: Option<Instant>, stop: Option<&AtomicBool>, probe_nodes: u64, all: bool) -> PrepStats {
    load_table();
    let m = s.multiverse();
    let (_, h) = s.get_board_size();
    let (amin, amax) = s.get_active_range();
    let (l0, l1) = s.get_initial_lines_range();
    let mut st = PrepStats::default();
    // (priority, line, first board's half-turn, boards, attacker, travel_key of a travel)
    let mut jobs: Vec<(i32, i32, i32, Vec<Board>, bool, Option<u64>)> = Vec::new();
    for l in amin..=amax {
        if (l < l0 || l > l1) && line_win(m, l).is_none() {
            let tl = m.timeline(l);
            let from = tl.boards.len().saturating_sub(WINDOW);
            let boards = tl.boards[from..].to_vec();
            let v0 = tl.start + from as i32;
            jobs.push((0, l, v0, boards.clone(), false, None));
            jobs.push((0, l, v0, boards, true, None));
        }
        for c in [false, true] {
            let vs = src_v(m.timeline_end_v(l), c);
            let Some(src) = at(m, l, vs) else { continue };
            let mine = src.hostile(!c);
            let (q, bsh) = (src.queen() & mine, src.bishop() & mine);
            if q | bsh == 0 {
                continue;
            }
            let occ = src.occupied();
            // Squares the side's queens and bishops reach with one move on that board.
            let (mut q1, mut b1) = (q, bsh);
            for x in marked_pos(q) {
                q1 |= queen_attack(x, occ);
            }
            for x in marked_pos(bsh) {
                b1 |= bishop_attack(x, occ);
            }
            let home = if c { 0 } else { h - 1 };
            line_targets(m, l, c, |v, sq, queen, b| {
                if v >> 1 > MAX_TARGET_TURN || b.royal() & b.hostile(c) & (0xffu64 << (8 * home)) == 0 {
                    return;
                }
                let (piece, reach) = if queen { (q, q1) } else { (bsh, b1) };
                if piece == 0 {
                    return;
                }
                let (mut now, mut soon) = (false, false);
                routes(m, l, c, v, sq, queen, |from, goes_on| {
                    now |= piece & pmask(from) != 0;
                    soon |= goes_on && reach & pmask(from) != 0;
                });
                if all || now || soon {
                    let back = (vs - v) / 2;
                    let tk = travel_key(m.timeline(l).keys()[(v - m.timeline(l).start) as usize], sq, queen, c);
                    jobs.push((if now { 16 } else { 32 } + back, new_line(m, c), v + 1, vec![branch(b, sq, queen, c)], c, Some(tk)));
                }
            });
        }
    }
    if jobs.is_empty() {
        return st;
    }
    jobs.sort_by_key(|j| j.0);
    let t0 = Instant::now();
    let verbose = std::env::var_os("SAC_VERBOSE").is_some();
    let mut spent = false;
    for (_, l, v0, boards, attacker, tk) in jobs {
        let last = boards.last().unwrap();
        // Both sides need a royal on the board for a mate to mean anything.
        if last.royal() & last.hostile(attacker) == 0 || last.royal() & last.hostile(!attacker) == 0 {
            continue;
        }
        let mut keys = [0u64; WINDOW];
        for (i, b) in boards.iter().enumerate() {
            keys[i] = board_key(l, v0 + i as i32, b);
        }
        let rk = result_key(window_key(&keys[..boards.len()]), attacker);
        if let Some(o) = get(&RESULTS, RES_BITS, rk) {
            note_travel(tk, o);
            continue;
        }
        // Out of budget, the known outcomes of the jobs left are still noted.
        if spent || st.nodes + probe_nodes > budget || deadline.is_some_and(|d| Instant::now() >= d) || stop.is_some_and(|s| s.load(Ordering::Relaxed)) {
            spent = true;
            continue;
        }
        let (o, n, timed_out) = resolve(&boards, l, v0, attacker, h, MAX_MOVES, probe_nodes, deadline, stop);
        if verbose {
            eprintln!("sac L{} v{} {} attacker {} -> {:?} ({} nodes)", l, v0 + boards.len() as i32 - 1, last.get_fen(false), attacker as u8, o, n);
        }
        st.nodes += n;
        if timed_out {
            spent = true;
            continue;
        }
        st.resolved += 1;
        put(&RESULTS, RES_BITS, rk, o);
        note_travel(tk, o);
        st.wins += matches!(o, Outcome::Win { .. }) as u32;
    }
    if verbose {
        eprintln!("sac prepare: {:?} in {:.3}s", st, t0.elapsed().as_secs_f64());
    }
    st
}

// ------------------------------------------------------------------ the precomputed table

/// `data/fsac_table.bin`: the outcomes, proofs and winning travels `ktools sactable` worked
/// out for the boards of the opening book and the tactics suite, loaded once into the tables.
static TABLE: &[u8] = include_bytes!("../../data/fsac_table.bin");
static LOADED: std::sync::Once = std::sync::Once::new();

fn load_table() {
    LOADED.call_once(|| {
        let mut r = TABLE;
        let u64_at = |r: &mut &[u8]| -> Option<u64> {
            let (x, rest) = r.split_first_chunk::<8>()?;
            *r = rest;
            Some(u64::from_le_bytes(*x))
        };
        if u64_at(&mut r) != Some(TABLE_MAGIC) {
            return;
        }
        for t in 0..3 {
            let Some(n) = u64_at(&mut r) else { return };
            for _ in 0..n {
                let Some(k) = u64_at(&mut r) else { return };
                match t {
                    0 => {
                        let Some(d) = u64_at(&mut r) else { return };
                        put(&RESULTS, RES_BITS, k, decode(d));
                    }
                    // A proof position: its key, with (attacker, plies) in the low bits.
                    1 => {
                        put(&PROOF, PROOF_BITS, proof_key(k), Outcome::Win { attacker: k & 16 != 0, plies: (k & 15) as u8 });
                        ANY_PROOF.store(true, Ordering::Relaxed);
                    }
                    _ => note_travel(Some(k), Outcome::Win { attacker: false, plies: 0 }),
                }
            }
        }
    });
}

/// Start from empty tables (for building the precomputed one).
pub fn skip_table() {
    LOADED.call_once(|| {});
}

const TABLE_MAGIC: u64 = 0x3142_4154_4341_5346; // "FSACTAB1"

/// The tables' known outcomes (no unknowns), proof positions (not mated ones) and winning
/// travels, in the format
/// `load_table` reads.
pub fn dump_table() -> Vec<u8> {
    let mut out = TABLE_MAGIC.to_le_bytes().to_vec();
    let section = |entries: Vec<(u64, u64)>, out: &mut Vec<u8>, with_data: bool| {
        out.extend((entries.len() as u64).to_le_bytes());
        for (k, d) in entries {
            out.extend(k.to_le_bytes());
            if with_data {
                out.extend(d.to_le_bytes());
            }
        }
    };
    let pairs = |t: &[AtomicU64], keep: fn(Outcome) -> bool| -> Vec<(u64, u64)> {
        t.chunks(2)
            .filter_map(|e| {
                let d = e[1].load(Ordering::Relaxed);
                (d != 0 && keep(decode(d))).then(|| (e[0].load(Ordering::Relaxed) ^ d, d))
            })
            .collect()
    };
    section(pairs(&RESULTS, |o| o != Outcome::Unknown), &mut out, true);
    // Mated positions are left to the search, which sees those (and keeps the file small).
    let proofs = pairs(&PROOF, |o| matches!(o, Outcome::Win { plies, .. } if (1..16).contains(&plies)));
    let packed = proofs
        .into_iter()
        .map(|(k, d)| match decode(d) {
            Outcome::Win { attacker, plies } => (k | (attacker as u64) << 4 | plies as u64, 0),
            _ => unreachable!(),
        })
        .collect();
    section(packed, &mut out, false);
    section(TRAVELS.iter().map(|k| k.load(Ordering::Relaxed)).filter(|&k| k != 0).map(|k| (k, 0)).collect(), &mut out, false);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::multiverse::tc_to_v;

    #[test]
    fn turn_zero_starts_only() {
        let g = |fen: &str| crate::core::perft::parse_g_line(&format!("G 1 size 8x8 {} moves", fen)).unwrap();
        let t0 = "odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:0:b] [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w]";
        assert!(turn_zero(&g(t0)));
        assert!(!turn_zero(&g("odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w]")));
        let two = "even fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:-0:1:w] [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:+0:1:w]";
        assert!(!turn_zero(&g(two)));
    }

    /// 1. Bxf7+ (or Qxf7+) onto the start position: 1...Kxf7 2. Nh3 .. 3. Ng5+ .. 4. c3 ..
    /// 5. Qb3*, the softmate a check through time on the king's old square f7.
    #[test]
    fn f7_sac_on_the_start_position_is_a_local_mate() {
        let s = crate::core::variants::startpos();
        let (t, c) = s.get_present();
        let target = *s.board(0, t, c);
        let f7 = 5 + 8 * 6;
        let v = tc_to_v(t, c) + 1;
        for queen in [false, true] {
            let b = branch(&target, f7, queen, false);
            let (o, _, _) = resolve(&[b], 1, v, false, 8, MAX_MOVES, NODE_BUDGET, None, None);
            assert_eq!(o, Outcome::Win { attacker: false, plies: 8 }, "queen {}", queen);
        }
        // Nor does the defender mate there.
        let b = branch(&target, f7, true, false);
        assert_eq!(resolve(&[b], 1, v, true, 8, MAX_MOVES, NODE_BUDGET, None, None).0, Outcome::NoMate);
    }
}
