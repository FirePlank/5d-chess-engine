//! A board NNUE that gives the whole static evaluation (`docs/design/nnue_plan.md`, first model).
//!
//! Every timeline's latest board is encoded alone by a shared network: its pieces as sparse
//! features (kind x own/enemy x square, from one colour's view) summed into an accumulator, seen
//! from both colours, then compressed to a small board embedding. The embeddings are summed per
//! board role (must play, may play, opponent's, inactive) and a small head reads those sums, the
//! role counts and dense "bridge" inputs: the handcrafted evaluation's per-side counts
//! (`hce::Sides`, unweighted), which carry what history does to travel and royal danger until a
//! learned relation channel replaces them. The HCE score itself is not an input.
//!
//! Inference is integer and exact, so any evaluation order (SIMD or not, Rust or the trainer's
//! reference in scripts/nnue/train.py) gives the same score. Accumulators are i16 (`Model::
//! from_bytes` proves they cannot overflow), activations u8 in [0, 127] standing for [0, 1],
//! weights after the first layer i8 at scale 64. Boards are cached by contents with both views'
//! accumulators; a board missing from the cache is updated from its predecessor on the same
//! timeline (a move changes a few squares) when that one is cached.

use super::hce::{self, Sides};
use crate::core::bitboard::*;
use crate::core::board::Board;
use crate::core::multiverse::MoveList;
use crate::core::piece::*;
use crate::core::state::State;
use crate::core::vec4::Vec4;

/// Piece kinds with their own features (white-normalized `Piece`s, in index order).
const KINDS: [Piece; 12] = [KING_W, QUEEN_W, BISHOP_W, KNIGHT_W, ROOK_W, PAWN_W, UNICORN_W, DRAGON_W, BRAWN_W, PRINCESS_W, ROYAL_QUEEN_W, COMMON_KING_W];
/// Kinds whose unmoved flag (castling, double steps) is a feature of its own.
const UNMOVED: [Piece; 4] = [KING_W, ROOK_W, PAWN_W, BRAWN_W];
/// Board features: (kind, own/enemy, square), then (unmoved kind, own/enemy, square).
pub const NB: usize = KINDS.len() * 2 * 64 + UNMOVED.len() * 2 * 64;
/// Board roles, the order of the pooled sums: the side to move's boards on the present (it
/// must play them), its other playable boards, the opponent's latest boards, inactive lines.
pub const ROLES: usize = 4;
pub const ROLE_MANDATORY: u8 = 0;
pub const ROLE_OPTIONAL: u8 = 1;
pub const ROLE_OPPONENT: u8 = 2;
pub const ROLE_INACTIVE: u8 = 3;
/// Dense bridge inputs: both sides' `hce::Sides` counts (side to move first), then active and
/// inactive timelines, heavy pieces and mandatory boards.
pub const ND: usize = 2 * hce::si::COUNT + 2 * hce::ex::COUNT + 4;
/// Most features of one board from one view (64 squares, each a piece and an unmoved flag).
pub const MAX_BOARD_FEATURES: usize = 128;
/// Largest layer widths a model may have (the evaluation works in fixed buffers).
const MAX_ACC: usize = 512;
const MAX_EMB: usize = 64;
const MAX_HIDDEN: usize = 64;
const MAX_RACC: usize = 128;
const MAX_INPUTS: usize = ROLES * MAX_EMB + ROLES + MAX_RACC + ND;
/// Activation of 1.0, and the weight scale after the first layer (a shift of `SHIFT`).
pub const ONE: i32 = 127;
pub const SHIFT: u32 = 6;
/// Fixed-point bits of the dense input scales and of the output scale.
pub const FIX: u32 = 16;
/// Boards back along a timeline a board missing from the cache looks for one to update from.
const UPDATE_BACK: usize = 6;

#[inline]
fn kind_index(k: Piece) -> Option<usize> {
    KINDS.iter().position(|&x| x == k)
}

/// The features of the piece on `sq` of board `b` (height `h`) from colour `view`'s side (true =
/// black): its own pieces first in colour, ranks counted from its own back rank. Writes 0, 1 or
/// 2 features (the piece, its unmoved flag) and returns the count.
#[inline]
fn square_features(b: &Board, h: i32, view: bool, sq: i32, out: &mut [u16; 2]) -> usize {
    let z = pmask(sq);
    if (b.white() | b.black()) & !b.wall() & z == 0 {
        return 0;
    }
    let kind = b.kind(sq);
    let Some(k) = kind_index(kind) else { return 0 };
    let rel = ((b.white() & z == 0) != view) as usize;
    let s = if view { (sq & 7) + 8 * (h - 1 - (sq >> 3)) } else { sq } as usize;
    out[0] = ((k * 2 + rel) * 64 + s) as u16;
    if b.umove() & z != 0
        && let Some(u) = UNMOVED.iter().position(|&x| x == kind) {
            out[1] = (KINDS.len() * 128 + (u * 2 + rel) * 64 + s) as u16;
            return 2;
        }
    1
}

/// Every feature of board `b` (height `h`) from colour `view`'s side. Returns the count written.
pub fn board_features(b: &Board, h: i32, view: bool, out: &mut [u16; MAX_BOARD_FEATURES]) -> usize {
    let mut n = 0;
    let mut f = [0u16; 2];
    for sq in marked_pos((b.white() | b.black()) & !b.wall()) {
        let k = square_features(b, h, view, sq, &mut f);
        out[n..n + k].copy_from_slice(&f[..k]);
        n += k;
    }
    n
}

/// Board feature `f` seen from the other colour's side, on a board of height `h`: own and enemy
/// swap and ranks count from the other back rank (`board_features` for the other view, feature
/// by feature).
#[inline]
pub fn other_view(f: u16, h: i32) -> u16 {
    let s = (f & 63) as i32;
    ((((f >> 6) ^ 1) << 6) as i32 + (s & 7) + 8 * (h - 1 - (s >> 3))) as u16
}

/// One latest board of a position, as the network sees it.
pub struct BoardInput<'a> {
    pub board: &'a Board,
    pub role: u8,
    /// Where it is: timeline, turn and colour.
    pub l: i32,
    pub t: i32,
    pub c: bool,
}

/// The latest boards of `s` with their roles, in timeline order.
pub fn boards(s: &State) -> impl Iterator<Item = BoardInput<'_>> {
    let present = s.get_present();
    let stm = present.1;
    let (amin, amax) = s.get_active_range();
    let (lo, hi) = s.get_lines_range();
    (lo..=hi).map(move |l| {
        let (t, c) = s.get_timeline_end(l);
        let role = if l < amin || l > amax {
            ROLE_INACTIVE
        } else if c != stm {
            ROLE_OPPONENT
        } else if (t, c) == present {
            ROLE_MANDATORY
        } else {
            ROLE_OPTIONAL
        };
        BoardInput { board: s.board(l, t, c), role, l, t, c }
    })
}

/// The dense bridge inputs of `s` (side to move's view).
pub fn dense(sd: &Sides, s: &State) -> [i16; ND] {
    let stm = s.get_present().1 as usize;
    let (ns, ne) = (hce::si::COUNT, hce::ex::COUNT);
    let mut x = [0i32; ND];
    x[..ns].copy_from_slice(&sd.side[stm]);
    x[ns..2 * ns].copy_from_slice(&sd.side[1 - stm]);
    x[2 * ns..2 * ns + ne].copy_from_slice(&sd.extra[stm]);
    x[2 * ns + ne..2 * ns + 2 * ne].copy_from_slice(&sd.extra[1 - stm]);
    let g = &mut x[2 * ns + 2 * ne..];
    g[0] = sd.active;
    g[1] = sd.inactive;
    g[2] = sd.heavy;
    g[3] = sd.mandatory;
    x.map(|v| v.clamp(-32767, 32767) as i16)
}

/// Relation features, from the moves through time or across timelines of the pieces on every
/// latest board (by the side whose turn it is there):
/// - reaching an enemy royal's square or one next to it: (side to move's / opponent's piece,
///   kind, direction from the royal (0: onto it), landing board (latest of its timeline, or
///   past by 0, 1, 2, 3+ turns), captures, square defended by a non-royal);
/// - capturing a piece: (side, kind, victim kind, landing board, defended).
pub const NR_ROYAL: usize = 2 * 12 * 9 * 5 * 2 * 2;
pub const NR: usize = NR_ROYAL + 2 * 12 * 12 * 5 * 2;
/// Most relation features of a position (more are left out).
pub const MAX_RELATIONS: usize = 4096;

/// The relation features of `s` (see `NR`), appended to `out`.
pub fn relation_features(s: &State, out: &mut Vec<u16>) {
    let m = s.multiverse();
    let stm = s.get_present().1;
    let (lo, hi) = s.get_lines_range();
    let mut ml = MoveList::new();
    for l in lo..=hi {
        let (t, c) = s.get_timeline_end(l);
        // The pieces of the side whose turn it is on this board.
        let b = s.board(l, t, c);
        let side = (c != stm) as usize;
        for sq in marked_pos(b.hostile(!c) & !b.wall()) {
            let Some(k) = kind_index(b.kind(sq)) else { continue };
            ml.clear();
            m.gen_superphysical_moves(Vec4::from_xy(sq, Vec4::new(0, 0, t, l)), c, &mut ml);
            for &(q, bb) in &ml {
                let Some(tb) = s.get_board_ptr(q.l(), q.t(), c) else { continue };
                let royals = tb.royal() & tb.hostile(c);
                let mut zone = royals;
                for r in marked_pos(royals) {
                    zone |= king_attack(r);
                }
                let defenders = tb.hostile(c) & !tb.royal();
                let (hits, caps) = (bb & zone, bb & defenders);
                if hits | caps == 0 {
                    continue;
                }
                let class = if (q.t(), c) == s.get_timeline_end(q.l()) { 0 } else { 1 + (t - q.t()).clamp(0, 3) as usize };
                for dst in marked_pos(caps) {
                    let Some(v) = kind_index(tb.kind(dst)) else { continue };
                    let def = (tb.attacks_to(dst) & defenders != 0) as usize;
                    if out.len() < MAX_RELATIONS {
                        out.push((NR_ROYAL + (((side * 12 + k) * 12 + v) * 5 + class) * 2 + def) as u16);
                    }
                }
                for dst in marked_pos(hits) {
                    let cap = (tb.hostile(c) & !tb.royal() & pmask(dst) != 0) as usize;
                    let def = (tb.attacks_to(dst) & defenders != 0) as usize;
                    for r in marked_pos(royals & (king_attack(dst) | pmask(dst))) {
                        let (dx, mut dy) = ((dst & 7) - (r & 7), (dst >> 3) - (r >> 3));
                        if c {
                            dy = -dy;
                        }
                        let dir = if (dx, dy) == (0, 0) { 0 } else { 1 + [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)].iter().position(|&d| d == (dx, dy)).unwrap() };
                        if out.len() < MAX_RELATIONS {
                            out.push(((((side * 12 + k) * 9 + dir) * 5 + class) * 4 + cap * 2 + def) as u16);
                        }
                    }
                }
            }
        }
    }
}

const MAGIC: &[u8; 8] = b"KNNUE003";

static EMBEDDED: std::sync::OnceLock<Option<std::sync::Arc<Model>>> = std::sync::OnceLock::new();

/// The network built into the engine (data/kairos.nnue): its evaluation unless `EvalFile` says
/// otherwise.
pub fn embedded() -> Option<std::sync::Arc<Model>> {
    EMBEDDED.get_or_init(|| Model::from_bytes(include_bytes!("../../data/kairos.nnue")).ok().map(std::sync::Arc::new)).clone()
}

/// A trained, quantized network (`scripts/nnue/train.py --out`).
pub struct Model {
    /// Accumulator and board embedding widths, head width, relation accumulator width.
    pub acc: usize,
    pub emb: usize,
    pub hidden: usize,
    pub racc: usize,
    /// Whether the head reads the dense bridge inputs (else they are 0 and never computed).
    pub use_dense: bool,
    /// [NB][acc] at scale `ONE`, and its bias.
    ft_w: Vec<i16>,
    ft_b: Vec<i16>,
    /// [2 * acc][emb] (input-major) at scale 64; bias at `ONE` * 64.
    l2_w: Vec<i8>,
    l2_b: Vec<i32>,
    /// [NR][racc] at scale `ONE`, and its bias (an i32 accumulator: a position has up to
    /// `MAX_RELATIONS` relations).
    rel_w: Vec<i16>,
    rel_b: Vec<i32>,
    /// Dense input i's activation is (d * dense_m[i]) >> FIX (rounded), at scale `ONE`.
    dense_m: Vec<i32>,
    /// [head inputs][hidden] (input-major) at scale 64; bias at `ONE` * 64.
    head_w: Vec<i8>,
    head_b: Vec<i32>,
    /// Output weights at scale 64 and bias at `ONE` * 64; the score is (out * out_q) >> FIX.
    out_w: Vec<i32>,
    out_b: i32,
    out_q: i32,
    /// `l2_w` by groups of 4 inputs: [input / 4][emb][4] (one `maddubs` group per output).
    l2_c: Vec<i8>,
    /// `head_w` by pairs of inputs, widened: [input / 2][hidden][2] (one `madd` pair per output).
    head_p: Vec<i16>,
}

/// Rounded arithmetic shift right.
#[inline]
fn shr_round(x: i64, bits: u32) -> i64 {
    (x + (1 << (bits - 1))) >> bits
}

impl Model {
    /// Head inputs: pooled embeddings per role, role counts, relation activations, dense inputs.
    pub fn head_inputs(&self) -> usize {
        ROLES * self.emb + ROLES + self.racc + ND
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Model, String> {
        if bytes.len() < 44 || &bytes[..8] != MAGIC {
            return Err("not a kairos NNUE file (KNNUE003)".into());
        }
        let u = |i: usize| u32::from_le_bytes(bytes[8 + 4 * i..12 + 4 * i].try_into().unwrap()) as usize;
        let (nb, acc, emb, roles, nd, hidden, nr, racc, use_dense) = (u(0), u(1), u(2), u(3), u(4), u(5), u(6), u(7), u(8) != 0);
        if nb != NB || roles != ROLES || nd != ND || nr != NR {
            return Err(format!("feature layout {}x{}x{}x{} differs from the engine's {}x{}x{}x{}", nb, roles, nd, nr, NB, ROLES, ND, NR));
        }
        if acc > MAX_ACC || emb > MAX_EMB || hidden > MAX_HIDDEN || racc > MAX_RACC || acc % 2 != 0 || emb % 8 != 0 || hidden % 8 != 0 || racc % 2 != 0 || acc * emb * hidden == 0 {
            return Err(format!("layers {}x{}x{}x{}: need even, multiples of 8, at most {}x{}x{}x{}", acc, emb, hidden, racc, MAX_ACC, MAX_EMB, MAX_HIDDEN, MAX_RACC));
        }
        let mut at = 44;
        let mut take = |n: usize| -> Result<&[u8], String> {
            let r = bytes.get(at..at + n).ok_or("truncated NNUE file")?;
            at += n;
            Ok(r)
        };
        let i16s = |b: &[u8]| b.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>();
        let i32s = |b: &[u8]| b.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect::<Vec<_>>();
        let i8s = |b: &[u8]| b.iter().map(|&x| x as i8).collect::<Vec<_>>();
        let inputs = ROLES * emb + ROLES + racc + ND;
        let ft_w = i16s(take(2 * NB * acc)?);
        let ft_b = i16s(take(2 * acc)?);
        let l2_w = i8s(take(2 * acc * emb)?);
        let l2_b = i32s(take(4 * emb)?);
        let rel_w = i16s(take(2 * NR * racc)?);
        let rel_b = i32s(take(4 * racc)?);
        let dense_m = i32s(take(4 * ND)?);
        let head_w = i8s(take(inputs * hidden)?);
        let head_b = i32s(take(4 * hidden)?);
        let out_w = i32s(take(4 * hidden)?);
        let tail = i32s(take(8)?);
        if at != bytes.len() {
            return Err("trailing data in NNUE file".into());
        }
        // No accumulator can leave i16: the bias plus the largest weights of a full board.
        for j in 0..acc {
            let mut col: Vec<i32> = (0..NB).map(|f| (ft_w[f * acc + j] as i32).abs()).collect();
            col.sort_unstable_by(|a, b| b.cmp(a));
            let bound = (ft_b[j] as i32).abs() + col[..MAX_BOARD_FEATURES].iter().sum::<i32>();
            if bound > i16::MAX as i32 {
                return Err(format!("accumulator {} can reach {}, past i16", j, bound));
            }
        }
        Ok(Model { acc, emb, hidden, racc, use_dense, ft_w, ft_b, l2_w, l2_b, rel_w, rel_b, dense_m, head_w, head_b, out_w, out_b: tail[0], out_q: tail[1], l2_c: Vec::new(), head_p: Vec::new() }.with_layouts())
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = MAGIC.to_vec();
        for v in [NB, self.acc, self.emb, ROLES, ND, self.hidden, NR, self.racc, self.use_dense as usize] {
            b.extend_from_slice(&(v as u32).to_le_bytes());
        }
        self.ft_w.iter().chain(&self.ft_b).for_each(|x| b.extend_from_slice(&x.to_le_bytes()));
        self.l2_w.iter().for_each(|x| b.push(*x as u8));
        self.l2_b.iter().for_each(|x| b.extend_from_slice(&x.to_le_bytes()));
        self.rel_w.iter().for_each(|x| b.extend_from_slice(&x.to_le_bytes()));
        self.rel_b.iter().chain(&self.dense_m).for_each(|x| b.extend_from_slice(&x.to_le_bytes()));
        self.head_w.iter().for_each(|x| b.push(*x as u8));
        self.head_b.iter().chain(&self.out_w).chain([&self.out_b, &self.out_q]).for_each(|x| b.extend_from_slice(&x.to_le_bytes()));
        b
    }

    /// A model with pseudo-random weights (tests and the speed-cost measurement).
    pub fn random(acc: usize, emb: usize, hidden: usize, racc: usize, use_dense: bool, seed: u64) -> Model {
        let mut z = seed;
        let mut r = move |lo: i32, hi: i32| -> i32 {
            z = z.wrapping_add(0x9e3779b97f4a7c15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
            lo + ((x >> 33) % (hi - lo + 1) as u64) as i32
        };
        let inputs = ROLES * emb + ROLES + racc + ND;
        Model {
            acc,
            emb,
            hidden,
            racc,
            use_dense,
            ft_w: (0..NB * acc).map(|_| r(-24, 24) as i16).collect(),
            ft_b: (0..acc).map(|_| r(-40, 80) as i16).collect(),
            l2_w: (0..2 * acc * emb).map(|_| r(-40, 40) as i8).collect(),
            l2_b: (0..emb).map(|_| r(-2000, 4000)).collect(),
            rel_w: (0..NR * racc).map(|_| r(-60, 60) as i16).collect(),
            rel_b: (0..racc).map(|_| r(-40, 80)).collect(),
            dense_m: (0..ND).map(|_| r(1000, 300000)).collect(),
            head_w: (0..inputs * hidden).map(|_| r(-30, 30) as i8).collect(),
            head_b: (0..hidden).map(|_| r(-2000, 4000)).collect(),
            out_w: (0..hidden).map(|_| r(-200, 200)).collect(),
            out_b: 0,
            out_q: 1 << 14,
            l2_c: Vec::new(),
            head_p: Vec::new(),
        }
        .with_layouts()
    }

    /// The kernels' weight layouts (`l2_c`, `head_p`) from the file's.
    fn with_layouts(mut self) -> Model {
        let (e, hd) = (self.emb, self.hidden);
        self.l2_c = vec![0; self.l2_w.len()];
        for i in 0..2 * self.acc {
            for j in 0..e {
                self.l2_c[((i / 4) * e + j) * 4 + i % 4] = self.l2_w[i * e + j];
            }
        }
        self.head_p = vec![0; self.head_w.len()];
        for i in 0..self.head_inputs() {
            for j in 0..hd {
                self.head_p[((i / 2) * hd + j) * 2 + i % 2] = self.head_w[i * hd + j] as i16;
            }
        }
        self
    }

    /// `acc` from scratch: the bias plus the rows of `feats`.
    fn refresh(&self, feats: &[u16], acc: &mut [i16]) {
        acc.copy_from_slice(&self.ft_b);
        for &f in feats {
            self.add_row(f, acc);
        }
    }

    #[inline]
    fn add_row(&self, f: u16, acc: &mut [i16]) {
        let row = &self.ft_w[f as usize * self.acc..(f as usize + 1) * self.acc];
        for (a, w) in acc.iter_mut().zip(row) {
            *a = a.wrapping_add(*w);
        }
    }

    #[inline]
    fn sub_row(&self, f: u16, acc: &mut [i16]) {
        let row = &self.ft_w[f as usize * self.acc..(f as usize + 1) * self.acc];
        for (a, w) in acc.iter_mut().zip(row) {
            *a = a.wrapping_sub(*w);
        }
    }

    /// Both views' accumulators of `b` (white's view, then black's) from scratch.
    fn refresh_board(&self, b: &Board, h: i32, acc: &mut [i16]) {
        let mut f = [0u16; MAX_BOARD_FEATURES];
        for v in 0..2 {
            let n = board_features(b, h, v == 1, &mut f);
            self.refresh(&f[..n], &mut acc[v * self.acc..(v + 1) * self.acc]);
        }
    }

    /// `acc`, both views' accumulators of `prev`, turned into those of `b`: the squares whose
    /// contents differ lose their old features and gain the new ones.
    fn update_board(&self, prev: &Board, b: &Board, h: i32, acc: &mut [i16]) {
        let (pa, ba) = (prev.raw_bbs(), b.raw_bbs());
        let changed = pa.iter().zip(&ba).fold(0, |m, (x, y)| m | (x ^ y));
        let mut f = [0u16; 2];
        for sq in marked_pos(changed) {
            for v in 0..2 {
                let a = &mut acc[v * self.acc..(v + 1) * self.acc];
                let n = square_features(prev, h, v == 1, sq, &mut f);
                for &x in &f[..n] {
                    self.sub_row(x, a);
                }
                let n = square_features(b, h, v == 1, sq, &mut f);
                for &x in &f[..n] {
                    self.add_row(x, a);
                }
            }
        }
    }

    /// The embedding of a board for side to move `stm` from both views' accumulators.
    fn embed(&self, acc: &[i16], stm: bool, out: &mut [u8]) {
        let (own, other) = if stm { (&acc[self.acc..2 * self.acc], &acc[..self.acc]) } else { (&acc[..self.acc], &acc[self.acc..2 * self.acc]) };
        let mut a = [0u8; 2 * MAX_ACC];
        for (x, &v) in a[..self.acc].iter_mut().zip(own) {
            *x = v.clamp(0, ONE as i16) as u8;
        }
        for (x, &v) in a[self.acc..2 * self.acc].iter_mut().zip(other) {
            *x = v.clamp(0, ONE as i16) as u8;
        }
        let mut sum = [0i32; MAX_EMB];
        let sum = &mut sum[..self.emb];
        sum.copy_from_slice(&self.l2_b);
        kernels::chunks4(&a[..2 * self.acc], &self.l2_c, sum);
        for (o, &s) in out.iter_mut().zip(sum.iter()) {
            *o = (s >> SHIFT).clamp(0, ONE) as u8;
        }
    }

    /// The score from the pooled embeddings `pooled` (per role), the role counts and the dense
    /// inputs.
    fn head(&self, pooled: &[i32], counts: &[i32; ROLES], rel: &[u16], d: &[i16; ND]) -> i32 {
        let _prof = crate::misc::prof::scope(22);
        let mut x = [0i16; MAX_INPUTS];
        let x = &mut x[..self.head_inputs()];
        for (o, &p) in x.iter_mut().zip(pooled) {
            *o = p.min(i16::MAX as i32) as i16;
        }
        let base = ROLES * self.emb;
        for (r, &n) in counts.iter().enumerate() {
            x[base + r] = (n * ONE).min(i16::MAX as i32) as i16;
        }
        self.relations(rel, &mut x[base + ROLES..base + ROLES + self.racc]);
        let base = base + self.racc;
        for (i, &v) in d.iter().enumerate() {
            if v != 0 {
                x[base + ROLES + i] = shr_round(v as i64 * self.dense_m[i] as i64, FIX).clamp(-32767, 32767) as i16;
            }
        }
        let mut sum = [0i32; MAX_HIDDEN];
        let sum = &mut sum[..self.hidden];
        sum.copy_from_slice(&self.head_b);
        kernels::pairs(x, &self.head_p, sum);
        let mut out = self.out_b as i64;
        for (&s, &w) in sum.iter().zip(&self.out_w) {
            out += (s >> SHIFT).clamp(0, ONE) as i64 * w as i64;
        }
        shr_round(out * self.out_q as i64, FIX) as i32
    }

    /// The relation accumulator's activations (at scale `ONE`) from relation features `rel`.
    fn relations(&self, rel: &[u16], out: &mut [i16]) {
        let mut acc = [0i32; MAX_RACC];
        let acc = &mut acc[..self.racc];
        acc.copy_from_slice(&self.rel_b);
        for &f in rel {
            let row = &self.rel_w[f as usize * self.racc..(f as usize + 1) * self.racc];
            for (a, &w) in acc.iter_mut().zip(row) {
                *a += w as i32;
            }
        }
        for (o, &a) in out.iter_mut().zip(acc.iter()) {
            *o = a.clamp(0, ONE) as i16;
        }
    }

    /// The evaluation of `s` for the side to move, in internal units (not clamped). `sd`: the
    /// handcrafted counts, needed only when the model reads them (`use_dense`).
    pub fn evaluate(&self, s: &State, sd: Option<&Sides>, cache: &mut Cache) -> i32 {
        let stm = s.get_present().1;
        let h = s.get_board_size().1;
        let mut pooled = [0i32; ROLES * MAX_EMB];
        let pooled = &mut pooled[..ROLES * self.emb];
        let mut counts = [0i32; ROLES];
        {
            let _prof = crate::misc::prof::scope(20);
            for bi in boards(s) {
                let e = cache.embedding(self, s, &bi, h, stm);
                let r = bi.role as usize;
                for (p, &x) in pooled[r * self.emb..(r + 1) * self.emb].iter_mut().zip(e) {
                    *p += x as i32;
                }
                counts[r] += 1;
            }
        }
        let d = match sd {
            Some(sd) if self.use_dense => dense(sd, s),
            _ => [0; ND],
        };
        let mut rel = std::mem::take(&mut cache.rel);
        rel.clear();
        // (A model without a relation accumulator never pays for them.)
        if self.racc > 0 {
            let _prof = crate::misc::prof::scope(23);
            relation_features(s, &mut rel);
        }
        let v = self.head(pooled, &counts, &rel, &d);
        cache.rel = rel;
        v
    }

    /// The evaluation from exported features (`ktools nnuedata`): each latest board's role and
    /// features from the side to move's view and the other's, and the dense inputs.
    pub fn evaluate_features(&self, boards: &[(u8, &[u16], &[u16])], rel: &[u16], d: &[i16; ND]) -> i32 {
        let d = if self.use_dense { *d } else { [0; ND] };
        let mut pooled = vec![0i32; ROLES * self.emb];
        let mut counts = [0i32; ROLES];
        let mut acc = vec![0i16; 2 * self.acc];
        let mut e = vec![0u8; self.emb];
        for &(role, own, other) in boards {
            // As stored for a white side to move: its view first.
            self.refresh(own, &mut acc[..self.acc]);
            self.refresh(other, &mut acc[self.acc..]);
            self.embed(&acc, false, &mut e);
            let r = role as usize;
            for (p, &x) in pooled[r * self.emb..(r + 1) * self.emb].iter_mut().zip(&e) {
                *p += x as i32;
            }
            counts[r] += 1;
        }
        self.head(&pooled, &counts, rel, &d)
    }
}

/// Boards by contents (direct-mapped; a hit is an exact match): both views' accumulators and,
/// once asked for, the embedding for each side to move.
pub struct Cache {
    acc: usize,
    emb: usize,
    keys: Vec<(Board, i32)>,
    /// Bit 0: in use; bits 1, 2: embedding for white, black to move computed.
    flags: Vec<u8>,
    accs: Vec<i16>,
    embs: Vec<u8>,
    /// Scratch for a position's relation features.
    rel: Vec<u16>,
}

impl Cache {
    const BITS: u32 = 13;

    pub fn new() -> Cache {
        Cache { acc: 0, emb: 0, keys: Vec::new(), flags: Vec::new(), accs: Vec::new(), embs: Vec::new(), rel: Vec::with_capacity(MAX_RELATIONS) }
    }

    /// Forget every entry (another model, or a new one).
    pub fn clear(&mut self) {
        self.flags.iter_mut().for_each(|f| *f = 0);
    }

    #[inline]
    fn slot(b: &Board, h: i32) -> usize {
        let mut k = 0x9e3779b97f4a7c15u64 ^ h as u64;
        for x in b.raw_bbs() {
            k = (k ^ x).wrapping_mul(0xbf58476d1ce4e5b9);
            k ^= k >> 29;
        }
        (k >> (64 - Self::BITS)) as usize
    }

    /// The embedding of latest board `bi` of `s` (boards of height `h`) for side to move `stm`.
    fn embedding(&mut self, m: &Model, s: &State, bi: &BoardInput, h: i32, stm: bool) -> &[u8] {
        let b = bi.board;
        let n = 1usize << Self::BITS;
        if (self.acc, self.emb) != (m.acc, m.emb) {
            (self.acc, self.emb) = (m.acc, m.emb);
            self.keys = vec![(Board::default(), 0); n];
            self.flags = vec![0; n];
            self.accs = vec![0; n * 2 * m.acc];
            self.embs = vec![0; n * 2 * m.emb];
        }
        let i = Self::slot(b, h);
        let bit = 2u8 << stm as u8;
        let hit = self.flags[i] & 1 != 0 && self.keys[i] == (*b, h);
        if !hit || self.flags[i] & bit == 0 {
            let _prof = crate::misc::prof::scope(21);
            if !hit {
                let _ft = crate::misc::prof::scope(24);
                let mut acc = [0i16; 2 * MAX_ACC];
                let acc = &mut acc[..2 * m.acc];
                // From the nearest earlier board of its timeline that is cached (only leaves are
                // evaluated, so the one just before often is not): boards a few moves apart
                // differ on a few squares.
                let (mut from, (mut t, mut c)) = (None, (bi.t, bi.c));
                for _ in 0..UPDATE_BACK {
                    (t, c) = if c { (t, false) } else { (t - 1, true) };
                    let Some(p) = s.get_board_ptr(bi.l, t, c) else { break };
                    let j = Self::slot(p, h);
                    if self.flags[j] & 1 != 0 && self.keys[j] == (*p, h) {
                        from = Some((p, j));
                        break;
                    }
                }
                match from {
                    Some((p, j)) => {
                        acc.copy_from_slice(&self.accs[j * 2 * m.acc..(j + 1) * 2 * m.acc]);
                        m.update_board(p, b, h, acc);
                    }
                    None => m.refresh_board(b, h, acc),
                }
                self.accs[i * 2 * m.acc..(i + 1) * 2 * m.acc].copy_from_slice(acc);
                self.keys[i] = (*b, h);
                self.flags[i] = 1;
            }
            let (accs, embs) = (&self.accs[i * 2 * m.acc..(i + 1) * 2 * m.acc], &mut self.embs[(2 * i + stm as usize) * m.emb..(2 * i + stm as usize + 1) * m.emb]);
            let _l2 = crate::misc::prof::scope(25);
            m.embed(accs, stm, embs);
            self.flags[i] |= bit;
        }
        &self.embs[(2 * i + stm as usize) * m.emb..(2 * i + stm as usize + 1) * m.emb]
    }
}

impl Default for Cache {
    fn default() -> Self {
        Cache::new()
    }
}

/// The two sparse affine kernels, exact in any order: with AVX2 (`maddubs`/`madd`, as
/// Stockfish's sparse layer) or portable.
#[allow(clippy::needless_range_loop)] // registers indexed alongside pointer offsets
mod kernels {
    /// `sum[j] += Σ a[i] * w[i][j]` over groups of 4 inputs (`w` as `Model::l2_c`), skipping
    /// groups of zeros. a[i] <= 127 and |w| <= 128, so the `maddubs` pairs cannot saturate.
    #[inline]
    pub fn chunks4(a: &[u8], w: &[i8], sum: &mut [i32]) {
        #[cfg(target_feature = "avx2")]
        {
            // SAFETY: AVX2 is enabled at compile time; the slices hold whole groups.
            unsafe {
                match sum.len() {
                    8 => chunks4_avx2::<1>(a, w, sum),
                    16 => chunks4_avx2::<2>(a, w, sum),
                    24 => chunks4_avx2::<3>(a, w, sum),
                    32 => chunks4_avx2::<4>(a, w, sum),
                    40 => chunks4_avx2::<5>(a, w, sum),
                    48 => chunks4_avx2::<6>(a, w, sum),
                    56 => chunks4_avx2::<7>(a, w, sum),
                    _ => chunks4_avx2::<8>(a, w, sum),
                }
            }
        }
        #[cfg(not(target_feature = "avx2"))]
        chunks4_scalar(a, w, sum)
    }

    #[allow(dead_code)]
    pub fn chunks4_scalar(a: &[u8], w: &[i8], sum: &mut [i32]) {
        let e = sum.len();
        for (c, x) in a.chunks_exact(4).enumerate() {
            if u32::from_le_bytes([x[0], x[1], x[2], x[3]]) == 0 {
                continue;
            }
            let g = &w[c * e * 4..(c + 1) * e * 4];
            for (j, s) in sum.iter_mut().enumerate() {
                *s += (0..4).map(|k| x[k] as i32 * g[j * 4 + k] as i32).sum::<i32>();
            }
        }
    }

    #[cfg(target_feature = "avx2")]
    #[target_feature(enable = "avx2")]
    unsafe fn chunks4_avx2<const N: usize>(a: &[u8], w: &[i8], sum: &mut [i32]) {
        // SAFETY: the caller guarantees AVX2 and whole groups; every pointer stays in its slice.
        unsafe {
            use std::arch::x86_64::*;
            let e = sum.len();
            let ones = _mm256_set1_epi16(1);
            let mut acc = [_mm256_setzero_si256(); N];
            for (c, x) in a.chunks_exact(4).enumerate() {
                let v = u32::from_le_bytes([x[0], x[1], x[2], x[3]]);
                if v == 0 {
                    continue;
                }
                let xv = _mm256_set1_epi32(v as i32);
                let g = w.as_ptr().add(c * e * 4) as *const __m256i;
                for k in 0..N {
                    let p = _mm256_maddubs_epi16(xv, _mm256_loadu_si256(g.add(k)));
                    acc[k] = _mm256_add_epi32(acc[k], _mm256_madd_epi16(p, ones));
                }
            }
            for k in 0..N {
                let s = _mm256_loadu_si256(sum.as_ptr().add(8 * k) as *const __m256i);
                _mm256_storeu_si256(sum.as_mut_ptr().add(8 * k) as *mut __m256i, _mm256_add_epi32(s, acc[k]));
            }
        }
    }

    /// `sum[j] += Σ x[i] * w[i][j]` over pairs of inputs (`w` as `Model::head_p`), skipping
    /// pairs of zeros. |x| <= 32767 and |w| <= 128, so a pair cannot leave i32.
    #[inline]
    pub fn pairs(x: &[i16], w: &[i16], sum: &mut [i32]) {
        #[cfg(target_feature = "avx2")]
        {
            // SAFETY: AVX2 is enabled at compile time; the slices hold whole pairs.
            unsafe {
                match sum.len() {
                    8 => pairs_avx2::<1>(x, w, sum),
                    16 => pairs_avx2::<2>(x, w, sum),
                    24 => pairs_avx2::<3>(x, w, sum),
                    32 => pairs_avx2::<4>(x, w, sum),
                    40 => pairs_avx2::<5>(x, w, sum),
                    48 => pairs_avx2::<6>(x, w, sum),
                    56 => pairs_avx2::<7>(x, w, sum),
                    _ => pairs_avx2::<8>(x, w, sum),
                }
            }
        }
        #[cfg(not(target_feature = "avx2"))]
        pairs_scalar(x, w, sum)
    }

    #[allow(dead_code)]
    pub fn pairs_scalar(x: &[i16], w: &[i16], sum: &mut [i32]) {
        let h = sum.len();
        for (p, v) in x.chunks_exact(2).enumerate() {
            if v[0] == 0 && v[1] == 0 {
                continue;
            }
            let g = &w[p * h * 2..(p + 1) * h * 2];
            for (j, s) in sum.iter_mut().enumerate() {
                *s += v[0] as i32 * g[2 * j] as i32 + v[1] as i32 * g[2 * j + 1] as i32;
            }
        }
    }

    #[cfg(target_feature = "avx2")]
    #[target_feature(enable = "avx2")]
    unsafe fn pairs_avx2<const N: usize>(x: &[i16], w: &[i16], sum: &mut [i32]) {
        // SAFETY: the caller guarantees AVX2 and whole groups; every pointer stays in its slice.
        unsafe {
            use std::arch::x86_64::*;
            let h = sum.len();
            let mut acc = [_mm256_setzero_si256(); N];
            for (p, v) in x.chunks_exact(2).enumerate() {
                let both = (v[0] as u16 as u32) | (v[1] as u16 as u32) << 16;
                if both == 0 {
                    continue;
                }
                let xv = _mm256_set1_epi32(both as i32);
                let g = w.as_ptr().add(p * h * 2) as *const __m256i;
                for k in 0..N {
                    acc[k] = _mm256_add_epi32(acc[k], _mm256_madd_epi16(xv, _mm256_loadu_si256(g.add(k))));
                }
            }
            for k in 0..N {
                let s = _mm256_loadu_si256(sum.as_ptr().add(8 * k) as *const __m256i);
                _mm256_storeu_si256(sum.as_mut_ptr().add(8 * k) as *mut __m256i, _mm256_add_epi32(s, acc[k]));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::variants::{apply_uci_moves, position_from_uci};

    fn start() -> State {
        position_from_uci("startpos").expect("start position")
    }

    #[test]
    fn file_round_trip() {
        let m = Model::random(16, 8, 8, 16, true, 7);
        let b = m.to_bytes();
        let m2 = Model::from_bytes(&b).unwrap();
        assert_eq!(m2.to_bytes(), b);
        assert!(Model::from_bytes(&b[..b.len() - 4]).is_err());
    }

    #[test]
    fn other_view_is_the_other_colours_features() {
        let mut s = start();
        let line = "(0T1)e2e3 (0T1)g8f6 (0T2)g1f3 (0T2)e7e6 (0T3)c2c3 (0T3)b8c6 (0T4)d1b3 (0T4)f6g4 (0T5)b3(0T1)f7 (1T1)e8f7";
        for mv in line.split(' ') {
            apply_uci_moves(&mut s, &format!("{} submit", mv)).unwrap();
            let h = s.get_board_size().1;
            for bi in boards(&s) {
                for view in [false, true] {
                    let (mut a, mut b) = ([0u16; MAX_BOARD_FEATURES], [0u16; MAX_BOARD_FEATURES]);
                    let (n, m) = (board_features(bi.board, h, view, &mut a), board_features(bi.board, h, !view, &mut b));
                    let mut mapped: Vec<u16> = a[..n].iter().map(|&f| other_view(f, h)).collect();
                    let mut direct = b[..m].to_vec();
                    mapped.sort();
                    direct.sort();
                    assert_eq!(mapped, direct, "after {}", mv);
                }
            }
        }
    }

    #[test]
    fn views_mirror_the_start_position() {
        // The standard start position looks the same from both sides.
        let s = start();
        let (t, c) = s.get_timeline_end(0);
        let b = s.board(0, t, c);
        let h = s.get_board_size().1;
        let (mut w, mut k) = ([0u16; MAX_BOARD_FEATURES], [0u16; MAX_BOARD_FEATURES]);
        let (nw, nk) = (board_features(b, h, false, &mut w), board_features(b, h, true, &mut k));
        assert_eq!(nw, nk);
        let (mut w, mut k) = (w[..nw].to_vec(), k[..nk].to_vec());
        w.sort();
        k.sort();
        assert_eq!(w, k);
    }

    /// The reference: every board refreshed from its features, no cache.
    fn reference(m: &Model, s: &State, sd: &Sides) -> i32 {
        let h = s.get_board_size().1;
        let stm = s.get_present().1;
        let feats: Vec<(u8, Vec<u16>, Vec<u16>)> = boards(s)
            .map(|bi| {
                let (mut own, mut other) = ([0u16; MAX_BOARD_FEATURES], [0u16; MAX_BOARD_FEATURES]);
                let n = board_features(bi.board, h, stm, &mut own);
                let k = board_features(bi.board, h, !stm, &mut other);
                (bi.role, own[..n].to_vec(), other[..k].to_vec())
            })
            .collect();
        let refs: Vec<(u8, &[u16], &[u16])> = feats.iter().map(|(r, o, t)| (*r, &o[..], &t[..])).collect();
        let mut rel = Vec::new();
        relation_features(s, &mut rel);
        m.evaluate_features(&refs, &rel, &dense(sd, s))
    }

    #[test]
    fn kernels_match_scalar() {
        let m = Model::random(64, 32, 32, 32, true, 11);
        let mut z = 5u32;
        let mut r = move || {
            z = z.wrapping_mul(1664525).wrapping_add(1013904223);
            z >> 8
        };
        for _ in 0..200 {
            let a: Vec<u8> = (0..128).map(|_| if r() % 3 == 0 { 0 } else { (r() % 128) as u8 }).collect();
            let (mut s1, mut s2) = (vec![7i32; 32], vec![7i32; 32]);
            kernels::chunks4(&a, &m.l2_c, &mut s1);
            kernels::chunks4_scalar(&a, &m.l2_c, &mut s2);
            assert_eq!(s1, s2);
            let x: Vec<i16> = (0..m.head_inputs()).map(|_| if r() % 2 == 0 { 0 } else { (r() % 65535) as i32 - 32767 } as i16).collect();
            let (mut s1, mut s2) = (vec![-3i32; 32], vec![-3i32; 32]);
            kernels::pairs(&x, &m.head_p, &mut s1);
            kernels::pairs_scalar(&x, &m.head_p, &mut s2);
            assert_eq!(s1, s2);
        }
    }

    #[test]
    fn cached_and_incremental_match_the_reference() {
        let m = Model::random(32, 16, 8, 16, true, 3);
        let mut s = start();
        let mut cache = Cache::new();
        // Captures, a travel that creates a timeline (5.Qb3>>x(0T1)f7+) and moves on both lines.
        let line = "(0T1)e2e3 (0T1)g8f6 (0T2)g1f3 (0T2)e7e6 (0T3)c2c3 (0T3)b8c6 (0T4)d1b3 (0T4)f6g4 (0T5)b3(0T1)f7 (1T1)e8f7 (1T2)g1h3 (1T2)g7g6 (1T3)e2e3 (1T3)a7a6 (1T4)h3g5 (1T4)f7e8";
        for mv in line.split(' ') {
            apply_uci_moves(&mut s, &format!("{} submit", mv)).unwrap();
            let sd = hce::sides(&s);
            let a = m.evaluate(&s, Some(&sd), &mut cache);
            assert_eq!(a, m.evaluate(&s, Some(&sd), &mut Cache::new()), "after {}", mv);
            assert_eq!(a, reference(&m, &s, &sd), "after {}", mv);
        }
    }
}
