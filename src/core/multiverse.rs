//! The multiverse: all timelines of boards plus 5D move generation.
//!
//! Coordinates: L (timeline, signed) and T (turn). Internally a timeline is stored at index
//! `u = l_to_u(l)` and a board at half-turn index `v = 2t + c`.

use super::bitboard::*;
use super::board::{Board, FenError};
use super::piece::*;
use super::vec4::Vec4;

pub type Turn = (i32, bool);

/// Active timelines for a given timeline span: each side may lead by at most one line.
pub fn active_range_for(layout: TimelineLayout, l_min: i32, l_max: i32) -> (i32, i32) {
    let left_extent = layout.mirror(l_min);
    let common_extent = left_extent.min(l_max);
    let radius = common_extent + (common_extent < left_extent.max(l_max)) as i32;
    (l_min.max(layout.mirror(radius)), l_max.min(radius))
}

#[inline(always)]
pub fn next_turn(t: Turn) -> Turn {
    let v = (t.0 << 1 | t.1 as i32) + 1;
    (v >> 1, v & 1 != 0)
}
#[inline(always)]
pub fn previous_turn(t: Turn) -> Turn {
    let v = (t.0 << 1 | t.1 as i32) - 1;
    (v >> 1, v & 1 != 0)
}
pub fn show_turn(t: Turn) -> String {
    format!("{}{}", t.0, if t.1 { "b" } else { "w" })
}
/// Lexicographic comparison of (t, c) pairs, as `std::pair` does.
#[inline(always)]
pub fn turn_lt(a: Turn, b: Turn) -> bool {
    (a.0, a.1) < (b.0, b.1)
}

#[inline(always)]
pub const fn l_to_u(l: i32) -> i32 {
    if l >= 0 { ((l as u32) << 1) as i32 } else { !((l as u32) << 1) as i32 }
}
#[inline(always)]
pub const fn tc_to_v(t: i32, c: bool) -> i32 {
    (((t as u32) << 1) | c as u32) as i32
}
#[inline(always)]
pub const fn u_to_l(u: i32) -> i32 {
    if u & 1 != 0 { !(u >> 1) } else { u >> 1 }
}
#[inline(always)]
pub const fn v_to_tc(v: i32) -> Turn {
    (v >> 1, v & 1 != 0)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TimelineLayout {
    pub even: bool,
}
impl TimelineLayout {
    pub const fn odd() -> Self {
        TimelineLayout { even: false }
    }
    pub const fn even() -> Self {
        TimelineLayout { even: true }
    }
    #[inline(always)]
    pub const fn is_even(&self) -> bool {
        self.even
    }
    #[inline(always)]
    pub const fn mirror(&self, l: i32) -> i32 {
        -l - self.even as i32
    }
    #[inline(always)]
    pub const fn initial_range(&self) -> (i32, i32) {
        (self.mirror(0), 0)
    }
    pub fn pretty_l(&self, l: i32) -> String {
        if !self.even {
            return l.to_string();
        }
        if l >= 0 { format!("+{}", l) } else { format!("-{}", self.mirror(l)) }
    }
}

pub const ORTHOGONAL_TL: [Vec4; 3] = [Vec4::new(0, 0, 0, 1), Vec4::new(0, 0, 0, -1), Vec4::new(0, 0, -1, 0)];
pub const DIAGONAL_TL: [Vec4; 4] = [
    Vec4::new(0, 0, 1, 1),
    Vec4::new(0, 0, 1, -1),
    Vec4::new(0, 0, -1, 1),
    Vec4::new(0, 0, -1, -1),
];
pub const BOTH_TL: [Vec4; 7] = [
    Vec4::new(0, 0, 0, 1),
    Vec4::new(0, 0, 0, -1),
    Vec4::new(0, 0, -1, 0),
    Vec4::new(0, 0, 1, 1),
    Vec4::new(0, 0, 1, -1),
    Vec4::new(0, 0, -1, 1),
    Vec4::new(0, 0, -1, -1),
];
pub const DOUBLE_ORTHOGONAL_TL: [Vec4; 3] =
    [Vec4::new(0, 0, 0, 2), Vec4::new(0, 0, 0, -2), Vec4::new(0, 0, -2, 0)];
pub const SP_KNIGHT: [Vec4; 8] = [
    Vec4::new(0, 0, 2, 1),
    Vec4::new(0, 0, 1, 2),
    Vec4::new(0, 0, -2, 1),
    Vec4::new(0, 0, 1, -2),
    Vec4::new(0, 0, 2, -1),
    Vec4::new(0, 0, -1, 2),
    Vec4::new(0, 0, -2, -1),
    Vec4::new(0, 0, -1, -2),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Axes {
    Orthogonal,
    Diagonal,
    Both,
}

/// Destination board -> destination squares, in generation order.
pub type MoveList = Vec<(Vec4, Bitboard)>;

/// Merge (k, bb) into `out[start..]`, which is kept sorted by destination board.
#[inline]
fn region_insert(out: &mut MoveList, start: usize, k: Vec4, bb: Bitboard) {
    match out[start..].binary_search_by(|e| e.0.cmp(&k)) {
        Ok(i) => out[start + i].1 |= bb,
        Err(i) => out.insert(start + i, (k, bb)),
    }
}

#[derive(Clone)]
pub struct Timeline {
    /// First stored half-turn index, `i32::MAX` when empty.
    pub start: i32,
    pub boards: Vec<Board>,
}
impl Timeline {
    #[inline(always)]
    pub fn end(&self) -> i32 {
        if self.boards.is_empty() { i32::MIN } else { self.start + self.boards.len() as i32 - 1 }
    }
}

#[derive(Clone)]
pub struct Multiverse {
    layout: TimelineLayout,
    size_x: i32,
    size_y: i32,
    lines: Vec<Timeline>,
    l_min: i32,
    l_max: i32,
    active_min: i32,
    active_max: i32,
    /// XOR of `board_key` over every board; maintained incrementally.
    hash: u64,
}

#[inline(always)]
fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// Hash of one board's contents at a given (timeline, half-turn) slot.
#[inline]
pub fn board_key(l: i32, v: i32, b: &Board) -> u64 {
    let mut h = mix64(((l as u32 as u64) << 32 | v as u32 as u64) ^ 0x9e3779b97f4a7c15);
    for w in b.raw_bbs() {
        h = mix64(h ^ w).wrapping_add(0x632be59bd9b4e019);
    }
    h
}

#[derive(Debug, Clone)]
pub struct MultiverseError(pub String);
impl From<FenError> for MultiverseError {
    fn from(e: FenError) -> Self {
        MultiverseError(e.0)
    }
}

impl Multiverse {
    /// `boards` are (l, t, color, fen) tuples in the internal (mirrored) l numbering.
    pub fn new(
        layout: TimelineLayout,
        boards: &[(i32, i32, bool, String)],
        size_x: i32,
        size_y: i32,
    ) -> Result<Multiverse, MultiverseError> {
        if boards.is_empty() {
            return Err(MultiverseError("multiverse(): Empty input".into()));
        }
        // Gather with explicit slots first: input order is arbitrary and may leave gaps.
        let mut slots: Vec<Vec<Option<Board>>> = Vec::new();
        let (mut l_min, mut l_max) = (0, 0);
        for (l, t, c, fen) in boards {
            let (l, t, c) = (*l, *t, *c);
            if t < 0 {
                return Err(MultiverseError(
                    "multiverse::insert_board_impl(): Negative time is not supported.".into(),
                ));
            }
            let u = l_to_u(l) as usize;
            let v = tc_to_v(t, c) as usize;
            if u >= slots.len() {
                slots.resize(u + 1, Vec::new());
            }
            l_min = l_min.min(l);
            l_max = l_max.max(l);
            let tl = &mut slots[u];
            if v >= tl.len() {
                tl.resize(v + 1, None);
            }
            if tl[v].is_some() {
                return Err(MultiverseError(format!(
                    "multiverse::insert_board_impl(): Duplicate definition of the board on L={} (plain notation), T={} C={}",
                    l,
                    t,
                    if c { "b" } else { "w" }
                )));
            }
            tl[v] = Some(Board::from_fen(fen, size_x, size_y)?);
        }
        let mut lines = Vec::with_capacity(slots.len());
        for (u, slot) in slots.into_iter().enumerate() {
            let first = slot.iter().position(|b| b.is_some());
            match first {
                None => lines.push(Timeline { start: i32::MAX, boards: Vec::new() }),
                Some(s) => {
                    let mut bs = Vec::with_capacity(slot.len() - s);
                    for (k, b) in slot[s..].iter().enumerate() {
                        match b {
                            Some(b) => bs.push(*b),
                            None => {
                                let _ = k;
                                return Err(MultiverseError(format!(
                                    "multiverse(): There is a gap between boards on timeline L{}.",
                                    u_to_l(u as i32)
                                )));
                            }
                        }
                    }
                    lines.push(Timeline { start: s as i32, boards: bs });
                }
            }
        }
        for l in l_min..=l_max {
            let u = l_to_u(l) as usize;
            if u >= lines.len() || lines[u].boards.is_empty() {
                return Err(MultiverseError("multiverse(): There is a gap between timelines.".into()));
            }
        }
        let mut m = Multiverse {
            layout,
            size_x,
            size_y,
            lines,
            l_min,
            l_max,
            active_min: 0,
            active_max: 0,
            hash: 0,
        };
        let mut h = 0u64;
        for (u, tl) in m.lines.iter().enumerate() {
            for (k, b) in tl.boards.iter().enumerate() {
                h ^= board_key(u_to_l(u as i32), tl.start + k as i32, b);
            }
        }
        m.hash = h;
        let (a, b) = m.calculate_active_range();
        m.active_min = a;
        m.active_max = b;
        Ok(m)
    }

    #[inline(always)]
    pub fn layout(&self) -> TimelineLayout {
        self.layout
    }
    #[inline(always)]
    pub fn get_board_size(&self) -> (i32, i32) {
        (self.size_x, self.size_y)
    }
    #[inline(always)]
    pub fn get_initial_lines_range(&self) -> (i32, i32) {
        self.layout.initial_range()
    }
    #[inline(always)]
    pub fn get_lines_range(&self) -> (i32, i32) {
        (self.l_min, self.l_max)
    }
    #[inline(always)]
    pub fn get_active_range(&self) -> (i32, i32) {
        (self.active_min, self.active_max)
    }
    #[inline(always)]
    pub fn timeline(&self, l: i32) -> &Timeline {
        &self.lines[l_to_u(l) as usize]
    }
    #[inline(always)]
    pub fn get_timeline_start(&self, l: i32) -> Turn {
        v_to_tc(self.timeline(l).start)
    }
    #[inline(always)]
    pub fn get_timeline_end(&self, l: i32) -> Turn {
        v_to_tc(self.timeline(l).end())
    }
    #[inline(always)]
    pub fn timeline_end_v(&self, l: i32) -> i32 {
        self.timeline(l).end()
    }

    pub fn get_present(&self) -> Turn {
        let mut present_v = i32::MAX;
        for l in self.active_min..=self.active_max {
            present_v = present_v.min(self.timeline(l).end());
        }
        v_to_tc(present_v)
    }

    /// Borrowed board, or None when the turn is absent.
    #[inline(always)]
    pub fn get_board_ptr(&self, l: i32, t: i32, c: bool) -> Option<&Board> {
        let u = l_to_u(l) as usize;
        if u >= self.lines.len() || t < 0 {
            return None;
        }
        let tl = &self.lines[u];
        let v = t * 2 + c as i32;
        let idx = v - tl.start;
        if v < tl.start || idx as usize >= tl.boards.len() {
            return None;
        }
        Some(&tl.boards[idx as usize])
    }
    /// Board that must exist (panics otherwise).
    #[inline(always)]
    pub fn board(&self, l: i32, t: i32, c: bool) -> &Board {
        match self.get_board_ptr(l, t, c) {
            Some(b) => b,
            None => panic!("Error: Out of range in multiverse::get_board({}, {}, {})", l, t, c),
        }
    }

    pub fn append_board(&mut self, l: i32, b: Board) {
        let u = l_to_u(l) as usize;
        let tl = &mut self.lines[u];
        self.hash ^= board_key(l, tl.start + tl.boards.len() as i32, &b);
        tl.boards.push(b);
    }
    #[inline(always)]
    pub fn hash(&self) -> u64 {
        self.hash
    }

    fn insert_board_impl(&mut self, l: i32, t: i32, c: bool, b: Board) {
        let u = l_to_u(l) as usize;
        let v = tc_to_v(t, c);
        if u >= self.lines.len() {
            self.lines.resize(u + 1, Timeline { start: i32::MAX, boards: Vec::new() });
        }
        self.l_min = self.l_min.min(l);
        self.l_max = self.l_max.max(l);
        let tl = &mut self.lines[u];
        assert!(tl.boards.is_empty(), "insert_board on a populated timeline");
        tl.start = v;
        self.hash ^= board_key(l, v, &b);
        tl.boards.push(b);
    }

    /// Start a new timeline (only used for branching moves).
    pub fn insert_board(&mut self, l: i32, t: i32, c: bool, b: Board) {
        self.insert_board_impl(l, t, c, b);
        let (l0_min, l0_max) = self.get_initial_lines_range();
        let whites_lines = self.l_max - l0_max;
        let blacks_lines = l0_min - self.l_min;
        if l > l0_max && whites_lines <= blacks_lines + 1 && l > self.active_max {
            self.active_max += 1;
            if self.l_min < self.active_min {
                self.active_min -= 1;
            }
        } else if l < l0_min && blacks_lines <= whites_lines + 1 && l < self.active_min {
            self.active_min -= 1;
            if self.l_max > self.active_max {
                self.active_max += 1;
            }
        }
        debug_assert_eq!((self.active_min, self.active_max), self.calculate_active_range());
    }

    /// Undo the most recent `insert_board` (used by make/unmake).
    pub fn remove_last_line(&mut self, l: i32, saved_ranges: (i32, i32, i32, i32)) {
        let u = l_to_u(l) as usize;
        if let Some(b) = self.lines[u].boards.first() {
            self.hash ^= board_key(l, self.lines[u].start, b);
        }
        self.lines[u].boards.clear();
        self.lines[u].start = i32::MAX;
        if u + 1 == self.lines.len() {
            self.lines.pop();
        }
        (self.l_min, self.l_max, self.active_min, self.active_max) = saved_ranges;
    }
    pub fn pop_board(&mut self, l: i32) {
        let u = l_to_u(l) as usize;
        let tl = &mut self.lines[u];
        if let Some(b) = tl.boards.pop() {
            self.hash ^= board_key(l, tl.start + tl.boards.len() as i32, &b);
        }
    }
    #[inline(always)]
    pub fn ranges(&self) -> (i32, i32, i32, i32) {
        (self.l_min, self.l_max, self.active_min, self.active_max)
    }

    fn calculate_active_range(&self) -> (i32, i32) {
        active_range_for(self.layout, self.l_min, self.l_max)
    }

    /// (l, t, c, fen) for every board, timelines in u order.
    pub fn get_boards(&self, show_umove: bool) -> Vec<(i32, i32, bool, String)> {
        let mut out = Vec::new();
        for (u, tl) in self.lines.iter().enumerate() {
            let l = u_to_l(u as i32);
            for (k, b) in tl.boards.iter().enumerate() {
                let (t, c) = v_to_tc(tl.start + k as i32);
                out.push((l, t, c, b.get_fen(show_umove)));
            }
        }
        out
    }

    pub fn to_string_pretty(&self) -> String {
        let (present, player) = self.get_present();
        let mut s = format!(
            "Multiverse present: T{}{}\nlines range:({},{})\tactive range:({},{})\n",
            present,
            if player { 'b' } else { 'w' },
            self.l_min,
            self.l_max,
            self.active_min,
            self.active_max
        );
        for (u, tl) in self.lines.iter().enumerate() {
            let l = u_to_l(u as i32);
            for (k, b) in tl.boards.iter().enumerate() {
                let (t, c) = v_to_tc(tl.start + k as i32);
                s += &format!("L{}T{}{}  aka.{}\n", l, t, if c { 'b' } else { 'w' }, self.pretty_lt(Vec4::new(0, 0, t, l)));
                s += &b.to_string_pretty();
            }
        }
        s
    }

    #[inline(always)]
    pub fn inbound(&self, a: Vec4, color: bool) -> bool {
        let l = a.l();
        if a.outbound() || l < self.l_min || l > self.l_max {
            return false;
        }
        let tl = &self.lines[l_to_u(l) as usize];
        let v = tc_to_v(a.t(), color);
        tl.start <= v && v <= tl.end()
    }

    #[inline(always)]
    pub fn get_piece(&self, a: Vec4, color: bool) -> Piece {
        self.board(a.l(), a.t(), color).get_piece(a.xy())
    }
    #[inline(always)]
    pub fn get_umove_flag(&self, a: Vec4, color: bool) -> bool {
        self.board(a.l(), a.t(), color).umove() & pmask(a.xy()) != 0
    }

    pub fn pretty_l(&self, l: i32) -> String {
        self.layout.pretty_l(l)
    }
    pub fn pretty_lt(&self, p: Vec4) -> String {
        format!("({}T{})", self.pretty_l(p.l()), p.t())
    }

    // ------------------------------------------------------------------ movegen

    #[inline(always)]
    fn piece_with_flag(&self, p: Vec4, c: bool) -> Piece {
        let b = self.board(p.l(), p.t(), c);
        let mut piece = b.get_piece(p.xy());
        if b.umove() & pmask(p.xy()) != 0 {
            piece |= 0x80;
        }
        piece
    }

    /// Destination squares of a purely physical move for the piece on `p`.
    pub fn gen_physical_moves(&self, p: Vec4, c: bool) -> Bitboard {
        let piece = self.piece_with_flag(p, c);
        self.gen_physical_moves_impl(piece, p, c)
    }

    /// All destinations (physical first, then superphysical) grouped by board.
    pub fn gen_moves(&self, p: Vec4, c: bool, out: &mut MoveList) {
        let piece = self.piece_with_flag(p, c);
        if piece == NO_PIECE {
            panic!("gen_moves: applied on NO_PIECE");
        }
        let bb = self.gen_physical_moves_impl(piece, p, c);
        if bb != 0 {
            out.push((p.tl(), bb));
        }
        self.gen_sp_impl(piece, p, c, out);
    }

    pub fn gen_superphysical_moves(&self, p: Vec4, c: bool, out: &mut MoveList) {
        let piece = self.piece_with_flag(p, c);
        self.gen_sp_impl(piece, p, c, out);
    }

    /// Every destination square of the piece on `p`, in generation order.
    pub fn gen_piece_move(&self, p: Vec4, board_color: bool, out: &mut Vec<Vec4>) {
        let mut ml = MoveList::new();
        self.gen_piece_move_with(p, board_color, out, &mut ml);
    }

    /// `gen_piece_move` with a caller-provided scratch buffer.
    pub fn gen_piece_move_with(&self, p: Vec4, board_color: bool, out: &mut Vec<Vec4>, scratch: &mut MoveList) {
        scratch.clear();
        self.gen_moves(p, board_color, scratch);
        for &(r, bb) in scratch.iter() {
            for pos in marked_pos(bb) {
                out.push(Vec4::from_xy(pos, r));
            }
        }
    }

    /// Whether color `c` has a capture or promotion from the board (l, t), by attack masks and
    /// superphysical targets (pseudo-legal: legality is not checked).
    pub fn has_tactical_move(&self, l: i32, t: i32, c: bool, scratch: &mut MoveList) -> bool {
        let b = self.board(l, t, c);
        let own = b.hostile(!c);
        let enemy = b.hostile(c);
        let occ = b.occupied();
        for sq in marked_pos(own) {
            let z = pmask(sq);
            let att = if b.lpawn() & z != 0 {
                if c { black_pawn_attack(sq) } else { white_pawn_attack(sq) }
            } else if b.lknight() & z != 0 {
                knight_attack(sq)
            } else if b.lking() & z != 0 {
                king_attack(sq)
            } else {
                let mut a = 0;
                if b.lrook() & z != 0 {
                    a |= rook_attack(sq, occ);
                }
                if b.lbishop() & z != 0 {
                    a |= bishop_attack(sq, occ);
                }
                a
            };
            if att & enemy != 0 {
                return true;
            }
        }
        let last = if c { 0 } else { self.size_y - 1 };
        for sq in marked_pos(b.lpawn() & own) {
            let ahead = if c { sq - 8 } else { sq + 8 };
            if (sq >> 3) + if c { -1 } else { 1 } == last && ahead >= 0 && ahead < 64 && occ & pmask(ahead) == 0 {
                return true;
            }
        }
        for sq in marked_pos(own) {
            let p = Vec4::from_xy(sq, Vec4::new(0, 0, t, l));
            scratch.clear();
            self.gen_superphysical_moves(p, c, scratch);
            for &(q, bb) in scratch.iter() {
                if bb & self.board(q.l(), q.t(), c).hostile(c) != 0 {
                    return true;
                }
            }
        }
        false
    }

    /// Whether the piece on `p` has at least one destination.
    pub fn has_any_move(&self, p: Vec4, c: bool) -> bool {
        let piece = self.piece_with_flag(p, c);
        if self.gen_physical_moves_impl(piece, p, c) != 0 {
            return true;
        }
        let mut ml = MoveList::new();
        self.gen_sp_impl(piece, p, c, &mut ml);
        !ml.is_empty()
    }

    fn gen_physical_moves_impl(&self, piece: Piece, p: Vec4, c: bool) -> Bitboard {
        let b = self.board(p.l(), p.t(), c);
        let friendly = b.friendly(c);
        let hostile = b.hostile(c);
        let pos = p.xy();
        let z = pmask(pos);
        let empty = !(friendly | hostile);
        match piece {
            KING_W | KING_B | COMMON_KING_W | COMMON_KING_B => king_attack(pos) & !friendly,
            KING_UW | KING_UB => {
                let mut a = king_attack(pos) & !friendly;
                let urook = b.umove() & b.rook() & friendly;
                if !b.is_under_attack(pos, c) {
                    for d in [Vec4::new(1, 0, 0, 0), Vec4::new(-1, 0, 0, 0)] {
                        let mut i = 0;
                        let mut q = p + d;
                        while !q.outbound() {
                            let w = pmask(q.xy());
                            if i < 2 && b.is_under_attack(q.xy(), c) {
                                break;
                            } else if w & urook != 0 {
                                if (q + d).outbound() {
                                    a |= pmask((p + d + d).xy());
                                }
                                break;
                            } else if w & b.occupied() != 0 {
                                break;
                            }
                            i += 1;
                            q = q + d;
                        }
                    }
                }
                a
            }
            ROOK_W | ROOK_B | ROOK_UW | ROOK_UB => rook_attack(pos, b.occupied()) & !friendly,
            BISHOP_W | BISHOP_B => bishop_attack(pos, b.occupied()) & !friendly,
            QUEEN_W | QUEEN_B | PRINCESS_W | PRINCESS_B | ROYAL_QUEEN_W | ROYAL_QUEEN_B => {
                queen_attack(pos, b.occupied()) & !friendly
            }
            PAWN_W | BRAWN_W | PAWN_UW | BRAWN_UW => {
                let patt = white_pawn_attack(pos);
                let mut a = (patt & hostile) | (shift_north(z) & empty);
                let r = (shift_west(z) | shift_east(z)) & hostile & b.lpawn();
                let s = shift_north(shift_north(r)) & empty;
                if s != 0 {
                    let q = p + Vec4::new(0, 2, -1, 0);
                    if self.inbound(q, c) {
                        let b1 = self.board(q.l(), q.t(), c);
                        let j = s & b1.umove() & !friendly & b1.lpawn();
                        a |= shift_south(j) & empty;
                    }
                }
                if piece == PAWN_UW || piece == BRAWN_UW {
                    a |= shift_north(shift_north(z) & empty) & empty;
                }
                a
            }
            PAWN_B | BRAWN_B | PAWN_UB | BRAWN_UB => {
                let patt = black_pawn_attack(pos);
                let mut a = (patt & hostile) | (shift_south(z) & empty);
                let r = (shift_west(z) | shift_east(z)) & hostile & b.lpawn();
                let s = shift_south(shift_south(r)) & empty;
                if s != 0 {
                    let q = p + Vec4::new(0, 2, -1, 0);
                    if self.inbound(q, c) {
                        let b1 = self.board(q.l(), q.t(), c);
                        let j = s & b1.umove() & !friendly & b1.lpawn();
                        a |= shift_north(j) & empty;
                    }
                }
                if piece == PAWN_UB || piece == BRAWN_UB {
                    a |= shift_south(shift_south(z) & empty) & empty;
                }
                a
            }
            KNIGHT_W | KNIGHT_B => knight_attack(pos) & !friendly,
            UNICORN_W | UNICORN_B | DRAGON_W | DRAGON_B => 0,
            _ => panic!(
                "gen_physical_moves: Unknown piece {}{}",
                piece_name(piece) as char,
                if piece & 0x80 != 0 { "*" } else { "" }
            ),
        }
    }

    /// Purely superphysical slides of the single piece `z` along `dirs`, appended in order.
    #[inline]
    fn sp_slides(&self, p0: Vec4, c: bool, z: Bitboard, dirs: &[Vec4], mut emit: impl FnMut(Vec4, Bitboard)) {
        for &d in dirs {
            let mut remaining = z;
            let mut p1 = p0 + d;
            while remaining != 0 && self.inbound(p1, c) {
                let b1 = self.board(p1.l(), p1.t(), c);
                remaining &= !b1.friendly(c);
                if remaining != 0 {
                    emit(p1.tl(), remaining);
                    remaining &= !b1.hostile(c);
                }
                p1 = p1 + d;
            }
        }
    }

    fn gen_compound_moves(&self, p: Vec4, c: bool, tl: Axes, xy: Axes, out: &mut MoveList, start: usize) {
        let pos = p.xy();
        let deltas: &[Vec4] = match tl {
            Axes::Orthogonal => &ORTHOGONAL_TL,
            Axes::Diagonal => &DIAGONAL_TL,
            Axes::Both => &BOTH_TL,
        };
        let copy_mask_fn: fn(i32, i32) -> Bitboard = match xy {
            Axes::Orthogonal => rook_copy_mask,
            Axes::Diagonal => bishop_copy_mask,
            Axes::Both => queen_copy_mask,
        };
        for &d in deltas {
            let mut q = p;
            let (mut occ, mut fri) = (0u64, 0u64);
            for n in 1..8 {
                let cm = copy_mask_fn(pos, n);
                q = q + d;
                if self.inbound(q, c) {
                    let b = self.board(q.l(), q.t(), c);
                    occ |= cm & b.occupied();
                    fri |= cm & b.friendly(c);
                } else {
                    occ |= cm;
                    fri |= cm;
                    break;
                }
            }
            let loc = !fri
                & match xy {
                    Axes::Orthogonal => rook_attack(pos, occ),
                    Axes::Diagonal => bishop_attack(pos, occ),
                    Axes::Both => queen_attack(pos, occ),
                };
            q = p;
            for n in 1..8 {
                let cm = copy_mask_fn(pos, n);
                q = q + d;
                let cc = loc & cm;
                if cc != 0 {
                    region_insert(out, start, q.tl(), cc);
                } else {
                    break;
                }
            }
        }
    }

    fn gen_sp_impl(&self, piece: Piece, p: Vec4, c: bool, out: &mut MoveList) {
        let z = pmask(p.xy());
        match piece {
            KING_W | KING_B | COMMON_KING_W | COMMON_KING_B | KING_UW | KING_UB => {
                for d in BOTH_TL {
                    let q = p + d;
                    if self.inbound(q, c) {
                        let b = self.board(q.l(), q.t(), c);
                        let bb = king_jump_attack(p.xy()) & !b.friendly(c);
                        if bb != 0 {
                            out.push((q.tl(), bb));
                        }
                    }
                }
            }
            ROOK_W | ROOK_B | ROOK_UW | ROOK_UB => {
                self.sp_slides(p, c, z, &ORTHOGONAL_TL, |k, bb| out.push((k, bb)));
            }
            BISHOP_W | BISHOP_B => {
                self.sp_slides(p, c, z, &DIAGONAL_TL, |k, bb| out.push((k, bb)));
                let start = out.len();
                self.gen_compound_moves(p, c, Axes::Orthogonal, Axes::Orthogonal, out, start);
            }
            PRINCESS_W | PRINCESS_B => {
                let start = out.len();
                self.sp_slides(p, c, z, &ORTHOGONAL_TL, |k, bb| region_insert(out, start, k, bb));
                self.sp_slides(p, c, z, &DIAGONAL_TL, |k, bb| region_insert(out, start, k, bb));
                self.gen_compound_moves(p, c, Axes::Orthogonal, Axes::Orthogonal, out, start);
            }
            QUEEN_W | QUEEN_B | ROYAL_QUEEN_W | ROYAL_QUEEN_B => {
                let start = out.len();
                self.sp_slides(p, c, z, &ORTHOGONAL_TL, |k, bb| region_insert(out, start, k, bb));
                self.sp_slides(p, c, z, &DIAGONAL_TL, |k, bb| region_insert(out, start, k, bb));
                self.gen_compound_moves(p, c, Axes::Both, Axes::Both, out, start);
            }
            PAWN_W | BRAWN_W | PAWN_UW | BRAWN_UW | PAWN_B | BRAWN_B | PAWN_UB | BRAWN_UB => {
                let black = piece_color(piece);
                let brawn = to_white(piece_name(piece)) == BRAWN_W;
                let unmoved = piece_umove_flag(piece);
                let fl = if black { 1 } else { -1 };
                for d in [Vec4::new(0, 0, 1, fl), Vec4::new(0, 0, -1, fl)] {
                    let q = p + d;
                    if self.inbound(q, c) {
                        let b = self.board(q.l(), q.t(), c);
                        let bb = z & b.hostile(c);
                        if bb != 0 {
                            out.push((q.tl(), bb));
                        }
                    }
                }
                let q = p + Vec4::new(0, 0, 0, fl);
                if self.inbound(q, c) {
                    let b = self.board(q.l(), q.t(), c);
                    let mut bb = z & !b.occupied();
                    if bb != 0 && unmoved {
                        let r = q + Vec4::new(0, 0, 0, fl);
                        if self.inbound(r, c) {
                            let b1 = self.board(r.l(), r.t(), c);
                            let bc = z & !b1.occupied();
                            if bc != 0 {
                                out.push((r.tl(), bc));
                            }
                        }
                    }
                    if brawn {
                        let fwd = if black { shift_south(z) } else { shift_north(z) };
                        let mask = fwd | shift_west(z) | shift_east(z);
                        bb |= mask & b.hostile(c);
                    }
                    if bb != 0 {
                        out.push((q.tl(), bb));
                    }
                }
                if brawn {
                    let q = p + Vec4::new(0, if black { -1 } else { 1 }, -1, 0);
                    if self.inbound(q, c) {
                        let b = self.board(q.l(), q.t(), c);
                        let bb = pmask(q.xy()) & b.hostile(c);
                        if bb != 0 {
                            out.push((q.tl(), bb));
                        }
                    }
                }
            }
            KNIGHT_W | KNIGHT_B => {
                for delta in SP_KNIGHT {
                    let p1 = p + delta;
                    if self.inbound(p1, c) {
                        let b1 = self.board(p1.l(), p1.t(), c);
                        let bb = z & !b1.friendly(c);
                        if bb != 0 {
                            out.push((p1.tl(), bb));
                        }
                    }
                }
                for d in ORTHOGONAL_TL {
                    let q = p + d;
                    if self.inbound(q, c) {
                        let b = self.board(q.l(), q.t(), c);
                        let bb = knight_jump1_attack(p.xy()) & !b.friendly(c);
                        if bb != 0 {
                            out.push((q.tl(), bb));
                        }
                    }
                }
                for d in DOUBLE_ORTHOGONAL_TL {
                    let q = p + d;
                    if self.inbound(q, c) {
                        let b = self.board(q.l(), q.t(), c);
                        let bb = knight_jump2_attack(p.xy()) & !b.friendly(c);
                        if bb != 0 {
                            out.push((q.tl(), bb));
                        }
                    }
                }
            }
            UNICORN_W | UNICORN_B => {
                let s1 = out.len();
                self.gen_compound_moves(p, c, Axes::Orthogonal, Axes::Diagonal, out, s1);
                let s2 = out.len();
                self.gen_compound_moves(p, c, Axes::Diagonal, Axes::Orthogonal, out, s2);
            }
            DRAGON_W | DRAGON_B => {
                let start = out.len();
                self.gen_compound_moves(p, c, Axes::Diagonal, Axes::Diagonal, out, start);
            }
            _ => panic!(
                "gen_superphysical_moves: Unknown piece {}{}",
                piece_name(piece) as char,
                if piece & 0x80 != 0 { "*" } else { "" }
            ),
        }
    }
}
