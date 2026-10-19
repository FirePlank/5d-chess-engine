//! Standard-chess evaluation terms of one 8x8 board: material, piece-square tables, pawn
//! structure, piece placement, threats, king safety and mobility, with a middlegame/endgame
//! taper. The coefficients of `chess_tab::TAB` are fitted offline (`scripts/chess/fit_chess.py`)
//! to Peras' centipawn evaluation of single-timeline positions; the weights of `hce` only scale
//! the groups, and fade them with the number of active timelines.
//!
//! Every term is a count of one colour (`Sink::add(term, count)`); the same code feeds the
//! evaluation (`EvalSink`: table dot product) and the fit (`ktools chessfeat`: raw counts).

use crate::core::bitboard::*;
use crate::core::board::Board;

use super::chess_tab::TAB;

pub const NG: usize = 7;
/// Term groups (`hce` features `fi::CH` ..).
pub mod g {
    pub const MAT: usize = 0;
    pub const PSQT: usize = 1;
    pub const PAWN: usize = 2;
    pub const PIECE: usize = 3;
    pub const THREAT: usize = 4;
    pub const KING: usize = 5;
    pub const MOB: usize = 6;
}

/// The groups (`g::`, one bit each) the evaluation works out; the weights of the others are 0 and
/// their terms are skipped (the attack maps of the threat, king and mobility groups are the
/// costly part). Fitting (`ktools chessfeat`) always counts every term.
pub const GROUPS: u32 = 0b1111011;
const fn on(group: usize) -> bool {
    GROUPS >> group & 1 != 0
}
/// Whether the evaluation needs per-piece attacks.
const ATTACKS: bool = on(g::THREAT) || on(g::KING) || on(g::MOB);

/// Term indices.
pub mod t {
    /// Pawns, knights, bishops, rooks, queens.
    pub const MAT: usize = 0;
    /// Piece-square tables (files mirrored): pawn, knight, bishop, rook, queen, king x 32.
    pub const PSQT: usize = MAT + 5;
    /// Passed pawns by relative rank 2..7.
    pub const PASSED: usize = PSQT + 6 * 32;
    pub const PASSED_FREE: usize = PASSED + 6;
    pub const PASSED_SUPP: usize = PASSED_FREE + 1;
    /// Enemy king's distance to the stop square minus the own king's, summed over passed pawns.
    pub const PASSED_KDIST: usize = PASSED_SUPP + 1;
    pub const ISOLATED: usize = PASSED_KDIST + 1;
    pub const DOUBLED: usize = ISOLATED + 1;
    pub const BACKWARD: usize = DOUBLED + 1;
    /// Pawns defended by a pawn, and pawns with a neighbour beside them, by relative rank 2..7.
    pub const SUPPORTED: usize = BACKWARD + 1;
    pub const PHALANX: usize = SUPPORTED + 6;
    pub const BPAIR: usize = PHALANX + 6;
    pub const ROOK_OPEN: usize = BPAIR + 1;
    pub const ROOK_SEMI: usize = ROOK_OPEN + 1;
    pub const ROOK_7TH: usize = ROOK_SEMI + 1;
    pub const N_OUTPOST: usize = ROOK_7TH + 1;
    pub const B_OUTPOST: usize = N_OUTPOST + 1;
    /// Knights and bishops with an own pawn directly in front.
    pub const MINOR_SHIELD: usize = B_OUTPOST + 1;
    /// Own pawns on the colour of each own bishop.
    pub const BAD_BISHOP: usize = MINOR_SHIELD + 1;
    pub const ROOK_TRAPPED: usize = BAD_BISHOP + 1;
    pub const BISHOP_TRAPPED: usize = ROOK_TRAPPED + 1;
    /// Threats against the enemy, when this side is to move on the board (M) or not (W): pawn
    /// attacks a minor piece, a rook, a queen; a minor piece attacks a rook, a queen; a rook
    /// attacks a queen; an attacked and undefended minor piece, rook, queen.
    pub const THREAT_M: usize = BISHOP_TRAPPED + 1;
    pub const THREAT_W: usize = THREAT_M + 9;
    pub const SHIELD1: usize = THREAT_W + 9;
    pub const SHIELD2: usize = SHIELD1 + 1;
    pub const KING_OPEN: usize = SHIELD2 + 1;
    pub const KING_SEMI: usize = KING_OPEN + 1;
    pub const STORM: usize = KING_SEMI + 1;
    /// Attack units on the king zone (knight/bishop 2, rook 3, queen 5 per attacking piece),
    /// their square, and their square when the enemy has a queen.
    pub const KUNITS: usize = STORM + 1;
    pub const KUNITS_SQ: usize = KUNITS + 1;
    pub const KUNITS_SQ_Q: usize = KUNITS_SQ + 1;
    /// Safe mobility of knights, bishops, rooks, queens.
    pub const MOB: usize = KUNITS_SQ_Q + 1;
    /// Knights and bishops with at most one safe square.
    pub const LOWMOB: usize = MOB + 4;
    pub const COUNT: usize = LOWMOB + 2;
}

const fn group_table() -> [u8; t::COUNT] {
    let mut a = [0u8; t::COUNT];
    let mut i = 0;
    while i < t::COUNT {
        a[i] = if i < t::PSQT {
            g::MAT
        } else if i < t::PASSED {
            g::PSQT
        } else if i < t::BPAIR {
            g::PAWN
        } else if i < t::THREAT_M {
            g::PIECE
        } else if i < t::SHIELD1 {
            g::THREAT
        } else if i < t::MOB {
            g::KING
        } else {
            g::MOB
        } as u8;
        i += 1;
    }
    a
}
const GROUP: [u8; t::COUNT] = group_table();

/// Receives the terms of one colour.
pub trait Sink {
    /// Whether every term is counted (fitting) or only those of the `GROUPS` (evaluation).
    const FULL: bool;
    fn add(&mut self, term: usize, v: i32);
    /// One count for each piece of `bb` at term `base + square index`.
    #[inline(always)]
    fn psqt(&mut self, base: usize, bb: u64) {
        for sq in bits(bb) {
            self.add(base + SQ32[sq as usize] as usize, 1);
        }
    }
}

/// Table dot product: middlegame and endgame sums per group (eighths of a centipawn).
#[derive(Clone, Copy)]
pub struct EvalSink {
    pub mg: [i32; NG],
    pub eg: [i32; NG],
}

impl EvalSink {
    pub const ZERO: EvalSink = EvalSink { mg: [0; NG], eg: [0; NG] };
    /// Tapered group values in centipawns for a game phase 0..=24 (24: all pieces).
    #[inline]
    pub fn tapered(&self, phase: i32) -> [i32; NG] {
        std::array::from_fn(|k| (self.mg[k] * phase + self.eg[k] * (24 - phase)) / (24 * 8))
    }
}

impl Sink for EvalSink {
    const FULL: bool = false;
    #[inline(always)]
    fn add(&mut self, term: usize, v: i32) {
        let g = GROUP[term] as usize;
        let e = TAB[term];
        self.mg[g] += e[0] as i32 * v;
        self.eg[g] += e[1] as i32 * v;
    }
    #[inline(always)]
    fn psqt(&mut self, base: usize, bb: u64) {
        let (mut mg, mut eg) = (0, 0);
        for sq in bits(bb) {
            let e = TAB[base + SQ32[sq as usize] as usize];
            mg += e[0] as i32;
            eg += e[1] as i32;
        }
        self.mg[g::PSQT] += mg;
        self.eg[g::PSQT] += eg;
    }
}

/// Raw counts (for fitting).
pub struct CountSink(pub [i32; t::COUNT]);
impl Sink for CountSink {
    const FULL: bool = true;
    #[inline(always)]
    fn add(&mut self, term: usize, v: i32) {
        self.0[term] += v;
    }
}

// ------------------------------------------------------------------ masks

const FILE_MASK: [u64; 8] = {
    let mut a = [0u64; 8];
    let mut f = 0;
    while f < 8 {
        a[f] = A_FILE << f;
        f += 1;
    }
    a
};
/// Squares of the files next to file f.
const ADJ: [u64; 8] = {
    let mut a = [0u64; 8];
    let mut f = 0;
    while f < 8 {
        if f > 0 {
            a[f] |= FILE_MASK[f - 1];
        }
        if f < 7 {
            a[f] |= FILE_MASK[f + 1];
        }
        f += 1;
    }
    a
};
/// Ranks strictly above rank r (white's view).
const fn above(r: usize) -> u64 {
    if r >= 7 { 0 } else { !0u64 << (8 * (r + 1)) }
}
const DARK: u64 = 0xAA55AA55AA55AA55;

#[inline(always)]
fn flip(x: u64, black: bool) -> u64 {
    if black { x.swap_bytes() } else { x }
}
#[inline(always)]
fn fill_n(mut b: u64) -> u64 {
    b |= b << 8;
    b |= b << 16;
    b |= b << 32;
    b
}
#[inline(always)]
fn fill_s(mut b: u64) -> u64 {
    b |= b >> 8;
    b |= b >> 16;
    b |= b >> 32;
    b
}
#[inline(always)]
fn cheb(a: i32, b: i32) -> i32 {
    ((a & 7) - (b & 7)).abs().max(((a >> 3) - (b >> 3)).abs())
}
struct Bits(u64);
impl Iterator for Bits {
    type Item = i32;
    #[inline(always)]
    fn next(&mut self) -> Option<i32> {
        if self.0 == 0 {
            None
        } else {
            let s = self.0.trailing_zeros() as i32;
            self.0 &= self.0 - 1;
            Some(s)
        }
    }
}
#[inline(always)]
fn bits(b: u64) -> Bits {
    Bits(b)
}
/// Mirrored-file square index 0..32 of a square.
const SQ32: [u8; 64] = {
    let mut a = [0u8; 64];
    let mut s = 0;
    while s < 64 {
        let f = s & 7;
        let m = if f < 7 - f { f } else { 7 - f };
        a[s] = ((s >> 3) * 4 + m) as u8;
        s += 1;
    }
    a
};

// ------------------------------------------------------------------ analysis

/// One colour's pieces (true board coordinates).
#[derive(Clone, Copy, Default)]
struct Pcs {
    p: u64,
    n: u64,
    b: u64,
    r: u64,
    q: u64,
    k: u64,
    all: u64,
}

/// One colour's attacks (true coordinates) and per-piece counts.
#[derive(Clone, Copy, Default)]
struct Att {
    pawn: u64,
    minor: u64,
    rook: u64,
    queen: u64,
    all: u64,
    /// Safe mobility: knights, bishops, rooks, queens.
    mob: [i32; 4],
    low: [i32; 2],
    /// Attack units this colour's pieces put on the enemy king's zone.
    units: i32,
    trapped_rooks: i32,
}

/// What `terms` needs to know about a board; `None` for boards that are not plain chess.
pub struct Info {
    pcs: [Pcs; 2],
    att: [Att; 2],
    /// Game phase 0..=24.
    pub phase: i32,
}

/// `full`: also the attack maps whatever `GROUPS` says.
pub fn analyze(b: &Board, w: i32, h: i32, full: bool) -> Option<Info> {
    if w != 8 || h != 8 {
        return None;
    }
    let col = [b.white() & !b.black(), b.black() & !b.white()];
    let (pawns, knights, bishops, rooks, queens, kings) = (b.pawn(), b.knight(), b.bishop(), b.rook(), b.queen(), b.king());
    let mut pcs = [Pcs::default(); 2];
    for c in 0..2 {
        let m = col[c];
        pcs[c] = Pcs { p: pawns & m, n: knights & m, b: bishops & m, r: rooks & m, q: queens & m, k: kings & m, all: m };
    }
    let occ = b.occupied();
    let mut att = [Att::default(); 2];
    let mut zone = [0u64; 2];
    for c in 0..2 {
        if pcs[c].k != 0 {
            let ks = pcs[c].k.trailing_zeros() as i32;
            zone[c] = king_attack(ks) | pcs[c].k;
        }
    }
    for c in 0..2 {
        let (me, en) = (&pcs[c], &pcs[1 - c]);
        let pawn_att = if c == 0 { shift_northwest(me.p) | shift_northeast(me.p) } else { shift_southwest(me.p) | shift_southeast(me.p) };
        let en_pawn_att =
            if c == 0 { shift_southwest(en.p) | shift_southeast(en.p) } else { shift_northwest(en.p) | shift_northeast(en.p) };
        let area = !me.all & !en_pawn_att;
        let ez = zone[1 - c];
        let mut a = Att { pawn: pawn_att, ..Default::default() };
        if !(full || ATTACKS) {
            att[c] = a;
            continue;
        }
        for sq in bits(me.n) {
            let x = knight_attack(sq);
            a.minor |= x;
            a.mob[0] += (x & area).count_ones() as i32;
            a.low[0] += ((x & area).count_ones() <= 1) as i32;
            if x & ez != 0 {
                a.units += 2;
            }
        }
        for sq in bits(me.b) {
            let x = bishop_attack(sq, occ);
            a.minor |= x;
            a.mob[1] += (x & area).count_ones() as i32;
            a.low[1] += ((x & area).count_ones() <= 1) as i32;
            if x & ez != 0 {
                a.units += 2;
            }
        }
        let ksq = if me.k != 0 { me.k.trailing_zeros() as i32 } else { -1 };
        for sq in bits(me.r) {
            let x = rook_attack(sq, occ);
            a.rook |= x;
            let m = (x & area).count_ones() as i32;
            a.mob[2] += m;
            if x & ez != 0 {
                a.units += 3;
            }
            if ksq >= 0 {
                let home = if c == 0 { 0 } else { 7 };
                let (rx, kx) = (sq & 7, ksq & 7);
                if m <= 3 && (sq >> 3) == home && (ksq >> 3) == home && (kx >= 4) == (rx > kx) {
                    a.trapped_rooks += 1;
                }
            }
        }
        for sq in bits(me.q) {
            let x = queen_attack(sq, occ);
            a.queen |= x;
            a.mob[3] += (x & area).count_ones() as i32;
            if x & ez != 0 {
                a.units += 5;
            }
        }
        a.all = a.pawn | a.minor | a.rook | a.queen | if me.k != 0 { king_attack(ksq) } else { 0 };
        att[c] = a;
    }
    let np = |x: &Pcs| (x.n | x.b).count_ones() as i32 + 2 * x.r.count_ones() as i32 + 4 * x.q.count_ones() as i32;
    let phase = (np(&pcs[0]) + np(&pcs[1])).min(24);
    Some(Info { pcs, att, phase })
}

impl Info {
    /// Whether colour `c`'s king is attacked.
    pub fn in_check(&self, c: usize) -> bool {
        self.pcs[c].k & self.att[1 - c].all != 0
    }
}

/// Terms of colour `c` (0 white, 1 black) other than the threats.
pub fn terms<S: Sink>(i: &Info, c: usize, s: &mut S) {
    let black = c == 1;
    let (me, en) = (&i.pcs[c], &i.pcs[1 - c]);
    let (am, ae) = (&i.att[c], &i.att[1 - c]);
    let fl = |x: u64| flip(x, black);
    let (p, n, bi, r, q, k) = (fl(me.p), fl(me.n), fl(me.b), fl(me.r), fl(me.q), fl(me.k));
    let ep = fl(en.p);
    let ek = fl(en.k);
    let my_pawn_att = fl(am.pawn);
    let en_pawn_att = fl(ae.pawn);
    let occ = fl(me.all | en.all);
    // Material and piece-square tables.
    if S::FULL || on(g::MAT) {
        s.add(t::MAT, p.count_ones() as i32);
        s.add(t::MAT + 1, n.count_ones() as i32);
        s.add(t::MAT + 2, bi.count_ones() as i32);
        s.add(t::MAT + 3, r.count_ones() as i32);
        s.add(t::MAT + 4, q.count_ones() as i32);
    }
    if S::FULL || on(g::PSQT) {
        for (kind, bb) in [p, n, bi, r, q, k].into_iter().enumerate() {
            s.psqt(t::PSQT + kind * 32, bb);
        }
    }
    let ksq = if k != 0 { k.trailing_zeros() as i32 } else { -1 };
    let eksq = if ek != 0 { ek.trailing_zeros() as i32 } else { -1 };
    // Pawns, all at once with fills (squares in the own frame: north is forward).
    if S::FULL || on(g::PAWN) {
        let nb = fill_n(shift_east(p) | shift_west(p)); // at or above a neighbouring pawn, on the pawn's file
        let neighbours = nb | fill_s(shift_east(p) | shift_west(p));
        let isolated = p & !neighbours;
        let es = fill_s(shift_south(ep));
        let passed = p & !(es | shift_west(es) | shift_east(es));
        let supported = p & my_pawn_att;
        let phalanx = p & shift_west(p);
        let backward = p & !nb & shift_south(en_pawn_att) & !isolated & !passed;
        s.add(t::ISOLATED, isolated.count_ones() as i32);
        s.add(t::DOUBLED, (p & fill_s(shift_south(p))).count_ones() as i32);
        s.add(t::BACKWARD, backward.count_ones() as i32);
        for rank in 1..7 {
            let row = 0xFFu64 << (8 * rank);
            s.add(t::PASSED + rank - 1, (passed & row).count_ones() as i32);
            s.add(t::SUPPORTED + rank - 1, (supported & row).count_ones() as i32);
            s.add(t::PHALANX + rank - 1, (phalanx & row).count_ones() as i32);
        }
        if passed != 0 {
            s.add(t::PASSED_FREE, (passed & !shift_south(occ)).count_ones() as i32);
            s.add(t::PASSED_SUPP, (passed & my_pawn_att).count_ones() as i32);
            if ksq >= 0 && eksq >= 0 {
                let mut d = 0;
                for sq in bits(passed) {
                    if sq < 56 {
                        d += cheb(eksq, sq + 8) - cheb(ksq, sq + 8);
                    }
                }
                s.add(t::PASSED_KDIST, d);
            }
        }
    }
    // Pieces.
    if S::FULL || on(g::PIECE) {
        if bi.count_ones() >= 2 {
            s.add(t::BPAIR, 1);
        }
        let (pf, epf) = (fill_n(p) | fill_s(p), fill_n(ep) | fill_s(ep));
        s.add(t::ROOK_OPEN, (r & !pf & !epf).count_ones() as i32);
        s.add(t::ROOK_SEMI, (r & !pf & epf).count_ones() as i32);
        s.add(t::ROOK_7TH, (r & (0xFFu64 << 48)).count_ones() as i32);
        let outposts = (0xFFu64 << 24 | 0xFFu64 << 32 | 0xFFu64 << 40) & my_pawn_att & !fill_s(en_pawn_att);
        s.add(t::N_OUTPOST, (n & outposts).count_ones() as i32);
        s.add(t::B_OUTPOST, (bi & outposts).count_ones() as i32);
        s.add(t::MINOR_SHIELD, (shift_north(n | bi) & p).count_ones() as i32);
        s.add(
            t::BAD_BISHOP,
            ((bi & DARK).count_ones() * (p & DARK).count_ones() + (bi & !DARK).count_ones() * (p & !DARK).count_ones()) as i32,
        );
        if S::FULL || ATTACKS {
            s.add(t::ROOK_TRAPPED, am.trapped_rooks);
        }
        s.add(
            t::BISHOP_TRAPPED,
            ((bi & (1 << 48) != 0 && ep & (1 << 41) != 0) as i32) + ((bi & (1 << 55) != 0 && ep & (1 << 46) != 0) as i32),
        );
    }
    // King.
    if ksq >= 0 && (S::FULL || on(g::KING)) {
        let (kf, kr) = ((ksq & 7) as usize, (ksq >> 3) as usize);
        let fwd1 = shift_north(k) | shift_northwest(k) | shift_northeast(k);
        s.add(t::SHIELD1, (p & fwd1).count_ones() as i32);
        s.add(t::SHIELD2, (p & shift_north(fwd1)).count_ones() as i32);
        if p & FILE_MASK[kf] == 0 {
            s.add(t::KING_OPEN, 1);
        }
        let semi = (kf > 0 && p & FILE_MASK[kf - 1] == 0) as i32 + (kf < 7 && p & FILE_MASK[kf + 1] == 0) as i32;
        s.add(t::KING_SEMI, semi);
        let three = FILE_MASK[kf] | ADJ[kf];
        let ranks3 = above(kr) & !above((kr + 3).min(7));
        s.add(t::STORM, (ep & three & ranks3).count_ones() as i32);
        let u = ae.units.min(30);
        s.add(t::KUNITS, u);
        s.add(t::KUNITS_SQ, u * u);
        if en.q != 0 {
            s.add(t::KUNITS_SQ_Q, u * u);
        }
    }
    // Mobility.
    if S::FULL || on(g::MOB) {
        for j in 0..4 {
            s.add(t::MOB + j, am.mob[j]);
        }
        s.add(t::LOWMOB, am.low[0]);
        s.add(t::LOWMOB + 1, am.low[1]);
    }
}

/// Threat counts of colour `c`, in the order of `t::THREAT_M`.
#[inline(always)]
pub fn threat_counts(i: &Info, c: usize) -> [i32; 9] {
    let (am, ae) = (&i.att[c], &i.att[1 - c]);
    let en = &i.pcs[1 - c];
    let minor = en.n | en.b;
    let cnt = |x: u64| x.count_ones() as i32;
    let weak = am.all & !ae.all;
    [
        cnt(am.pawn & minor),
        cnt(am.pawn & en.r),
        cnt(am.pawn & en.q),
        cnt(am.minor & en.r),
        cnt(am.minor & en.q),
        cnt(am.rook & en.q),
        cnt(weak & minor),
        cnt(weak & en.r),
        cnt(weak & en.q),
    ]
}

/// Threat terms of colour `c`, from term `base` (`t::THREAT_M` or `t::THREAT_W`).
pub fn threats<S: Sink>(i: &Info, c: usize, base: usize, s: &mut S) {
    for (j, v) in threat_counts(i, c).into_iter().enumerate() {
        s.add(base + j, v);
    }
}

/// A board's tapered group values: [colour][group], with the threat group split by whether the
/// colour is to move on the board: `thr[colour][0]` if it is, `[1]` if not.
#[derive(Clone, Copy)]
pub struct BoardChess {
    pub v: [[i32; NG]; 2],
    pub thr: [[i32; 2]; 2],
}

impl BoardChess {
    pub const ZERO: BoardChess = BoardChess { v: [[0; NG]; 2], thr: [[0; 2]; 2] };
}

pub fn board_chess(b: &Board, w: i32, h: i32) -> BoardChess {
    let Some(info) = analyze(b, w, h, false) else { return BoardChess::ZERO };
    let mut out = BoardChess::ZERO;
    let ph = info.phase;
    for c in 0..2 {
        let mut s = EvalSink::ZERO;
        terms(&info, c, &mut s);
        for k in 0..NG {
            out.v[c][k] = (s.mg[k] * ph + s.eg[k] * (24 - ph)) / (24 * 8);
        }
        if !on(g::THREAT) {
            continue;
        }
        let cnt = threat_counts(&info, c);
        for (m, base) in [t::THREAT_M, t::THREAT_W].into_iter().enumerate() {
            let (mut mg, mut eg) = (0, 0);
            for j in 0..9 {
                mg += cnt[j] * TAB[base + j][0] as i32;
                eg += cnt[j] * TAB[base + j][1] as i32;
            }
            out.thr[c][m] = (mg * ph + eg * (24 - ph)) / (24 * 8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(fen: &str) -> ([i32; t::COUNT], [i32; t::COUNT], i32) {
        let b = Board::from_fen(fen, 8, 8).unwrap();
        let i = analyze(&b, 8, 8, true).unwrap();
        let (mut w, mut k) = (CountSink([0; t::COUNT]), CountSink([0; t::COUNT]));
        terms(&i, 0, &mut w);
        threats(&i, 0, t::THREAT_M, &mut w);
        terms(&i, 1, &mut k);
        threats(&i, 1, t::THREAT_M, &mut k);
        (w.0, k.0, i.phase)
    }

    /// Colours and ranks swapped: white's terms become black's.
    fn mirrored(fen: &str) -> String {
        fen.split('/')
            .rev()
            .map(|r| {
                r.chars().map(|c| if c.is_ascii_uppercase() { c.to_ascii_lowercase() } else { c.to_ascii_uppercase() }).collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    #[test]
    fn colours_are_symmetric() {
        for fen in [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR",
            "r1bq1rk1/pp2bppp/2n1pn2/3p4/2PP4/2N1PN2/PP1B1PPP/R2QKB1R",
            "8/5pk1/6p1/1P6/2P5/6P1/5K2/8",
            "2r3k1/1b3ppp/p3p3/1p1pP3/3P1P2/1P2B2P/P5P1/2R3K1",
        ] {
            let (w, k, ph) = counts(fen);
            let (w2, k2, ph2) = counts(&mirrored(fen));
            assert_eq!(ph, ph2);
            assert_eq!(w, k2, "{fen}");
            assert_eq!(k, w2, "{fen}");
        }
        let (w, k, ph) = counts("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR");
        assert_eq!(w, k);
        assert_eq!(ph, 24);
    }

    #[test]
    fn pawn_structure_counts() {
        // White a2, c3, c4, h4 against black b7, g7: every pawn is isolated, c3/c4 are doubled,
        // and each is stopped by an enemy pawn on a neighbouring file ahead (none is passed).
        let (w, k, _) = counts("4k3/1p4p1/8/8/2P4P/2P5/P7/4K3");
        assert_eq!(w[t::DOUBLED], 1);
        assert_eq!(w[t::ISOLATED], 4);
        assert_eq!(w[t::PASSED..t::PASSED + 6].iter().sum::<i32>(), 0);
        assert_eq!(k[t::ISOLATED], 2);
        assert_eq!(k[t::PASSED..t::PASSED + 6].iter().sum::<i32>(), 0);
        // A lone h-pawn against nothing on the g/h files is passed, on its fourth rank.
        let (w, _, _) = counts("4k3/1p6/8/8/7P/8/8/4K3");
        assert_eq!(w[t::PASSED + 2], 1);
    }
}
