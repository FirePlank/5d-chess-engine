//! Borrowed view for finding royal captures over a state plus overlaid boards.
//! Scan order matters: the first check found decides the slice the hypercuboid search removes.
//! Turns are half-turn indices `v = 2t + c`.

use super::action::FullMove;
use super::bitboard::*;
use super::board::Board;
use super::multiverse::*;
use super::piece::Piece;
use super::state::State;
use super::vec4::Vec4;
use std::marker::PhantomData;

#[derive(Clone, Copy)]
struct LineView {
    /// First and last half-turn of the line (`start > end` when it doesn't exist).
    start: i32,
    end: i32,
    /// Half-turn from which `added` stands in (`i32::MAX` without an overlay).
    added_start: i32,
    added: *const Board,
    /// The state's boards of this line: `base_len` boards from half-turn `base_start`.
    base: *const Board,
    base_start: i32,
    base_len: i32,
}

const NO_LINE: LineView = LineView {
    start: i32::MAX,
    end: i32::MIN,
    added_start: i32::MAX,
    added: std::ptr::null(),
    base: std::ptr::null(),
    base_start: 0,
    base_len: 0,
};

/// Reusable line storage for `CheckPosition`.
#[derive(Default)]
pub struct CheckBuf(Vec<LineView>);

pub struct CheckPosition<'a> {
    first_line: i32,
    lines: Vec<LineView>,
    /// Only pieces on these squares of a source board are scanned.
    source_mask: Bitboard,
    /// The state and every overlaid board outlive the view.
    _borrow: PhantomData<&'a Board>,
}

#[derive(Clone, Copy)]
pub struct BoardOverlay<'a> {
    pub l: i32,
    pub t: i32,
    pub c: bool,
    pub value: &'a Board,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    Physical,
    Superphysical,
    Historical,
}

#[inline(always)]
fn v_of(t: Turn) -> i32 {
    2 * t.0 + t.1 as i32
}

/// The boards along one (t, l) direction, for compound slides (as `scan_compound_sliders`
/// collects them).
#[derive(Clone, Copy, Default)]
struct CompoundRay {
    count: usize,
    occupied: [Bitboard; 8],
    royal: [Bitboard; 8],
    any_royal: Bitboard,
}

impl CompoundRay {
    /// Whether a piece on `src` sliding with `diagonal` (else orthogonal) square steps along
    /// this ray captures a royal.
    #[inline]
    fn hits(&self, src: i32, diagonal: bool) -> bool {
        if self.any_royal == 0 {
            return false;
        }
        let mask_fn: fn(i32, i32) -> Bitboard = if diagonal { bishop_copy_mask } else { rook_copy_mask };
        let (mut occupied, mut royals) = (0u64, 0u64);
        for n in 1..=self.count {
            let mask = mask_fn(src, n as i32);
            occupied |= mask & self.occupied[n];
            royals |= mask & self.royal[n];
        }
        if self.count < 7 {
            occupied |= mask_fn(src, self.count as i32 + 1);
        }
        royals != 0 && royals & if diagonal { bishop_attack(src, occupied) } else { rook_attack(src, occupied) } != 0
    }
}

/// Superphysical reach of single pieces from one board: per movement component, the squares
/// from which a piece of the board's side captures an enemy royal on another board. Answers
/// the same question as a `gives_check` scan of that one piece, for every square at once.
pub struct SpReach {
    pure_orth: Bitboard,
    pure_diag: Bitboard,
    king: Bitboard,
    knight: Bitboard,
    pawn: Bitboard,
    brawn: Bitboard,
    /// `ORTHOGONAL_TL` then `DIAGONAL_TL`.
    rays: [CompoundRay; 7],
}

impl SpReach {
    /// Whether the piece on `sq` of `b` (the board this reach was computed for, or one that
    /// differs from it only in its own pieces) captures an enemy royal superphysically.
    pub fn attacks(&self, b: &Board, sq: i32) -> bool {
        let z = pmask(sq);
        if b.lrook() & z != 0 && self.pure_orth & z != 0 {
            return true;
        }
        if b.lbishop() & z != 0 && self.pure_diag & z != 0 {
            return true;
        }
        let (bishop, unicorn, dragon) = (b.lbishop() & z != 0, b.lunicorn() & z != 0, b.ldragon() & z != 0);
        if bishop || unicorn {
            for ray in &self.rays[..3] {
                if (bishop && ray.hits(sq, false)) || (unicorn && ray.hits(sq, true)) {
                    return true;
                }
            }
        }
        if unicorn || dragon {
            for ray in &self.rays[3..] {
                if (unicorn && ray.hits(sq, false)) || (dragon && ray.hits(sq, true)) {
                    return true;
                }
            }
        }
        (b.lking() & z != 0 && self.king & z != 0)
            || (b.lknight() & z != 0 && self.knight & z != 0)
            || (b.lpawn() & z != 0 && self.pawn & z != 0)
            || (b.lrawn() & z != 0 && self.brawn & z != 0)
    }
}

/// Squares with a king-step (or no step) to a square of `r`.
#[inline]
fn king_spread(r: Bitboard) -> Bitboard {
    let h = r | shift_east(r) | shift_west(r);
    h | shift_north(h) | shift_south(h)
}

impl<'a> CheckPosition<'a> {
    pub fn new(s: &'a State) -> Self {
        let (a, b) = s.get_lines_range();
        Self::with_range(s, a, b)
    }

    pub fn with_range(s: &'a State, l_min: i32, l_max: i32) -> Self {
        Self::with_range_in(s, l_min, l_max, CheckBuf::default())
    }

    /// `with_range` reusing `buf`'s storage (get it back with `into_buf`).
    pub fn with_range_in(s: &'a State, l_min: i32, l_max: i32, buf: CheckBuf) -> Self {
        let mut lines = buf.0;
        lines.clear();
        lines.resize((l_max - l_min + 1) as usize, NO_LINE);
        let (first, last) = s.get_lines_range();
        debug_assert!(l_min <= first && last <= l_max);
        let m = s.multiverse();
        for l in first..=last {
            let tl = m.timeline(l);
            let line = &mut lines[(l - l_min) as usize];
            line.start = tl.start;
            line.end = tl.end();
            line.base = tl.boards.as_ptr();
            line.base_start = tl.start;
            line.base_len = tl.boards.len() as i32;
        }
        CheckPosition { first_line: l_min, lines, source_mask: !0, _borrow: PhantomData }
    }

    pub fn into_buf(self) -> CheckBuf {
        CheckBuf(self.lines)
    }

    fn for_present(s: &'a State, present: Turn, buf: CheckBuf) -> Self {
        let (a, b) = s.get_lines_range();
        let mut r = Self::with_range_in(s, a, b, buf);
        let (lo, hi) = s.get_active_range();
        for l in lo..=hi {
            let (t, c) = s.get_timeline_end(l);
            if (t, c) == present {
                r.add_board(l, next_turn((t, c)), s.board(l, t, c));
            }
        }
        r
    }

    pub fn for_phantom(s: &'a State) -> Self {
        Self::for_phantom_in(s, CheckBuf::default())
    }

    pub fn for_phantom_in(s: &'a State, buf: CheckBuf) -> Self {
        Self::for_present(s, s.get_present(), buf)
    }

    pub fn for_check_symbols(s: &'a State) -> Self {
        let mut present = s.apparent_present();
        if present.1 == s.get_present().1 {
            present = next_turn(present);
        }
        Self::for_present(s, present, CheckBuf::default())
    }

    pub fn for_move_candidates(s: &'a State) -> Self {
        Self::for_move_candidates_in(s, CheckBuf::default())
    }

    pub fn for_move_candidates_in(s: &'a State, buf: CheckBuf) -> Self {
        let (lo, hi) = s.get_lines_range();
        let branch = s.new_line();
        let mut r = Self::with_range_in(s, lo.min(branch), hi.max(branch), buf);
        let player = s.get_present().1;
        for l in lo..=hi {
            let at = s.get_timeline_end(l);
            if at.1 != player {
                r.add_board(l, next_turn(at), s.board(l, at.0, at.1));
            }
        }
        r
    }

    pub fn add_board(&mut self, l: i32, at: Turn, b: &'a Board) {
        let v = v_of(at);
        let line = &mut self.lines[(l - self.first_line) as usize];
        debug_assert!(line.added.is_null());
        if line.start <= line.end {
            debug_assert!(v == line.end + 1);
        } else {
            line.start = v;
        }
        line.added_start = v;
        line.end = v;
        line.added = b;
    }

    /// Board at half-turn `v` of line `l`.
    #[inline(always)]
    fn board_v(&self, l: i32, v: i32) -> Option<&'a Board> {
        let i = l.wrapping_sub(self.first_line) as u32 as usize;
        let line = self.lines.get(i)?;
        if v < line.start || v > line.end {
            return None;
        }
        // SAFETY: the pointers come from the state and overlays borrowed for 'a.
        unsafe {
            if v >= line.added_start {
                return Some(&*line.added);
            }
            let k = v - line.base_start;
            if (k as u32) < line.base_len as u32 { Some(&*line.base.add(k as usize)) } else { None }
        }
    }

    #[inline]
    pub fn get_board_ptr(&self, l: i32, t: i32, c: bool) -> Option<&'a Board> {
        self.board_v(l, 2 * t + c as i32)
    }

    pub fn get_piece(&self, p: Vec4, c: bool) -> Piece {
        self.get_board_ptr(p.l(), p.t(), c).expect("check_position::get_piece").get_piece(p.xy())
    }

    /// Test for royal captures by `attacker` from the overlaid timelines.
    pub fn gives_check(
        &mut self,
        kind: CheckKind,
        mv: BoardOverlay<'a>,
        attacker: bool,
        departure: Option<BoardOverlay<'a>>,
    ) -> bool {
        let overlays = [Some(mv), departure];
        let mut sources = [0i32; 2];
        let mut saved = [NO_LINE; 2];
        let mut count = 0;
        for upd in overlays.iter().flatten() {
            let start = 2 * upd.t + upd.c as i32;
            sources[count] = upd.l;
            let line = &mut self.lines[(upd.l - self.first_line) as usize];
            saved[count] = *line;
            if line.start > line.end {
                line.start = start;
            }
            line.added_start = start;
            line.end = start + 1;
            line.added = upd.value;
            count += 1;
        }
        let result = {
            let lines = &self.lines;
            let first_line = self.first_line;
            let attacker_v = attacker as i32;
            let mut emit = |m: FullMove| -> bool {
                match kind {
                    CheckKind::Physical => m.from.tl() == m.to.tl(),
                    CheckKind::Superphysical => m.from.tl() != m.to.tl(),
                    CheckKind::Historical => {
                        let target = &lines[(m.to.l() - first_line) as usize];
                        2 * m.to.t() + attacker_v != target.end
                    }
                }
            };
            let inc_phys = kind == CheckKind::Physical;
            self.scan(attacker, inc_phys, &mut emit, &sources[..count], !inc_phys)
        };
        for i in 0..count {
            self.lines[(sources[i] - self.first_line) as usize] = saved[i];
        }
        result
    }

    /// Superphysical reach of single pieces of the side to move on line `l` at half-turn `v`
    /// (as a source board placed there would see the rest of this position).
    pub fn sp_reach(&self, l: i32, v: i32) -> SpReach {
        let c = v & 1 != 0;
        let p0 = Vec4::new(0, 0, v >> 1, l);
        let royals_at = |d: Vec4| self.board_at(p0 + d, c).map(|b| b.royal() & b.hostile(c)).unwrap_or(0);
        let pure = |dirs: &[Vec4]| -> Bitboard {
            let mut a = 0;
            for &d in dirs {
                let mut reach = !0u64;
                let mut q = p0 + d;
                while reach != 0 {
                    let Some(b) = self.board_at(q, c) else { break };
                    a |= reach & b.royal() & b.hostile(c);
                    reach &= !b.occupied();
                    q = q + d;
                }
            }
            a
        };
        let mut rays = [CompoundRay::default(); 7];
        for (ray, &d) in rays.iter_mut().zip(ORTHOGONAL_TL.iter().chain(DIAGONAL_TL.iter())) {
            let mut q = p0 + d;
            while ray.count < 7 {
                let Some(b) = self.board_at(q, c) else { break };
                ray.count += 1;
                ray.occupied[ray.count] = b.occupied();
                ray.royal[ray.count] = b.royal() & b.hostile(c);
                ray.any_royal |= ray.royal[ray.count];
                q = q + d;
            }
        }
        let mut king = 0;
        let mut knight = 0;
        for d in ORTHOGONAL_TL {
            king |= king_spread(royals_at(d));
            let r1 = royals_at(d);
            let e2 = shift_east(shift_east(r1));
            let w2 = shift_west(shift_west(r1));
            knight |= shift_north(shift_north(r1)) | shift_south(shift_south(r1)) | e2 | w2;
            let r2 = royals_at(d * 2);
            knight |= shift_north(r2) | shift_south(r2) | shift_east(r2) | shift_west(r2);
        }
        for d in DIAGONAL_TL {
            king |= king_spread(royals_at(d));
        }
        for d in SP_KNIGHT {
            knight |= royals_at(d);
        }
        let forward_l = if c { 1 } else { -1 };
        let pawn = royals_at(Vec4::new(0, 0, -1, forward_l)) | royals_at(Vec4::new(0, 0, 1, forward_l));
        // Sources whose forward square (or side squares) is in `r`.
        let behind = |r: Bitboard| if c { shift_north(r) } else { shift_south(r) };
        let rf = royals_at(Vec4::new(0, 0, 0, forward_l));
        let brawn = behind(rf) | shift_east(rf) | shift_west(rf) | behind(royals_at(Vec4::new(0, 0, -1, 0)));
        SpReach { pure_orth: pure(&ORTHOGONAL_TL), pure_diag: pure(&DIAGONAL_TL), king, knight, pawn, brawn, rays }
    }

    /// `gives_check` scanning only the overlaid boards' pieces on `mask`.
    pub fn gives_check_masked(
        &mut self,
        kind: CheckKind,
        mv: BoardOverlay<'a>,
        attacker: bool,
        departure: Option<BoardOverlay<'a>>,
        mask: Bitboard,
    ) -> bool {
        self.source_mask = mask;
        let r = self.gives_check(kind, mv, attacker, departure);
        self.source_mask = !0;
        r
    }

    pub fn first_check(&self, attacker: bool, include_physical: bool) -> Option<FullMove> {
        let mut r = None;
        self.scan(
            attacker,
            include_physical,
            &mut |m| {
                r = Some(m);
                true
            },
            &[],
            true,
        );
        r
    }

    /// `first_check(attacker, false)` when every other timeline is known to give no check:
    /// only `lines` (ascending, not empty) are scanned.
    pub fn first_check_in(&self, attacker: bool, lines: &[i32]) -> Option<FullMove> {
        debug_assert!(!lines.is_empty() && lines.windows(2).all(|w| w[0] < w[1]));
        let mut r = None;
        self.scan(
            attacker,
            false,
            &mut |m| {
                r = Some(m);
                true
            },
            lines,
            true,
        );
        r
    }

    pub fn checks(&self, attacker: bool, include_physical: bool) -> Vec<FullMove> {
        let mut r = Vec::new();
        self.checks_into(attacker, include_physical, &mut r);
        r
    }

    /// `checks` appended to `out`.
    pub fn checks_into(&self, attacker: bool, include_physical: bool, out: &mut Vec<FullMove>) {
        self.scan(
            attacker,
            include_physical,
            &mut |m| {
                out.push(m);
                false
            },
            &[],
            true,
        );
    }

    fn scan(
        &self,
        c: bool,
        include_physical: bool,
        emit: &mut dyn FnMut(FullMove) -> bool,
        sources: &[i32],
        include_superphysical: bool,
    ) -> bool {
        let scan_line = |i: usize, emit: &mut dyn FnMut(FullMove) -> bool| -> bool {
            let line = &self.lines[i];
            if line.start > line.end || (line.end & 1 != 0) != c {
                return false;
            }
            let l = self.first_line + i as i32;
            let p0 = Vec4::new(0, 0, line.end >> 1, l);
            let source = self.board_v(l, line.end).unwrap();
            let friendly = source.hostile(!c) & self.source_mask;
            (include_physical && self.scan_physical(c, p0, source, friendly, emit))
                || (include_superphysical
                    && (self.scan_pure_sliders(c, p0, source, friendly, emit)
                        || self.scan_compound_sliders(c, p0, source, friendly, emit)
                        || self.scan_jumps(c, p0, source, friendly, emit)))
        };
        if !sources.is_empty() {
            for &l in sources {
                if scan_line((l - self.first_line) as usize, emit) {
                    return true;
                }
            }
        } else {
            for i in 0..self.lines.len() {
                if scan_line(i, emit) {
                    return true;
                }
            }
        }
        false
    }

    fn scan_physical(&self, c: bool, p0: Vec4, source: &Board, friendly: Bitboard, emit: &mut dyn FnMut(FullMove) -> bool) -> bool {
        for dst in marked_pos(source.royal() & source.hostile(c)) {
            for src in marked_pos(source.attacks_to(dst) & friendly) {
                if emit(FullMove::new(Vec4::from_xy(src, p0), Vec4::from_xy(dst, p0))) {
                    return true;
                }
            }
        }
        false
    }

    /// Board one or more steps from `p0` along a (t, l) direction, for color `c`.
    #[inline(always)]
    fn board_at(&self, q: Vec4, c: bool) -> Option<&'a Board> {
        self.board_v(q.l(), 2 * q.t() + c as i32)
    }

    fn scan_pure_sliders(
        &self,
        c: bool,
        p0: Vec4,
        source: &Board,
        friendly: Bitboard,
        emit: &mut dyn FnMut(FullMove) -> bool,
    ) -> bool {
        let scan_direction = |direction: Vec4, mut pieces: Bitboard, emit: &mut dyn FnMut(FullMove) -> bool| {
            let mut q = p0 + direction;
            while pieces != 0 {
                let b = match self.board_at(q, c) {
                    Some(b) => b,
                    None => break,
                };
                let hits = pieces & b.royal() & b.hostile(c);
                for xy in marked_pos(hits) {
                    if emit(FullMove::new(Vec4::from_xy(xy, p0), Vec4::from_xy(xy, q))) {
                        return true;
                    }
                }
                pieces &= !b.occupied();
                q = q + direction;
            }
            false
        };
        let rooks = source.lrook() & friendly;
        if rooks != 0 {
            for d in ORTHOGONAL_TL {
                if scan_direction(d, rooks, emit) {
                    return true;
                }
            }
        }
        let bishops = source.lbishop() & friendly;
        if bishops != 0 {
            for d in DIAGONAL_TL {
                if scan_direction(d, bishops, emit) {
                    return true;
                }
            }
        }
        false
    }

    fn scan_compound_sliders(
        &self,
        c: bool,
        p0: Vec4,
        source: &Board,
        friendly: Bitboard,
        emit: &mut dyn FnMut(FullMove) -> bool,
    ) -> bool {
        let scan_direction = |direction: Vec4,
                              orth: Bitboard,
                              diag: Bitboard,
                              emit: &mut dyn FnMut(FullMove) -> bool|
         -> bool {
            if orth | diag == 0 {
                return false;
            }
            let mut occupied_boards = [0u64; 8];
            let mut royal_boards = [0u64; 8];
            let mut count = 0usize;
            // Royals some piece's projection reaches (`n` square steps on the `n`-th board),
            // ignoring blockers: when none, nothing below can hit.
            let mut reachable = 0u64;
            let (mut on, mut os, mut oe, mut ow) = (orth, orth, orth, orth);
            let (mut dne, mut dnw, mut dse, mut dsw) = (diag, diag, diag, diag);
            let mut q = p0 + direction;
            while count < 7 {
                let b = match self.board_at(q, c) {
                    Some(b) => b,
                    None => break,
                };
                count += 1;
                occupied_boards[count] = b.occupied();
                royal_boards[count] = b.royal() & b.hostile(c);
                on = shift_north(on);
                os = shift_south(os);
                oe = shift_east(oe);
                ow = shift_west(ow);
                dne = shift_northeast(dne);
                dnw = shift_northwest(dnw);
                dse = shift_southeast(dse);
                dsw = shift_southwest(dsw);
                reachable |= royal_boards[count] & (on | os | oe | ow | dne | dnw | dse | dsw);
                q = q + direction;
            }
            if reachable == 0 {
                return false;
            }
            let project = |diagonal: bool, pieces: Bitboard, emit: &mut dyn FnMut(FullMove) -> bool| -> bool {
                let mask_fn: fn(i32, i32) -> Bitboard = if diagonal { bishop_copy_mask } else { rook_copy_mask };
                for src in marked_pos(pieces) {
                    let (mut occupied, mut royals) = (0u64, 0u64);
                    for n in 1..=count {
                        let mask = mask_fn(src, n as i32);
                        occupied |= mask & occupied_boards[n];
                        royals |= mask & royal_boards[n];
                    }
                    if count < 7 {
                        occupied |= mask_fn(src, count as i32 + 1);
                    }
                    if royals == 0 {
                        continue;
                    }
                    let hits = royals & if diagonal { bishop_attack(src, occupied) } else { rook_attack(src, occupied) };
                    for dst in marked_pos(hits) {
                        let p = Vec4::from_xy(src, p0);
                        let xy = Vec4::from_xy(dst, p0);
                        let distance = (xy.x() - p.x()).abs().max((xy.y() - p.y()).abs());
                        if emit(FullMove::new(p, Vec4::from_xy(dst, p0 + direction * distance))) {
                            return true;
                        }
                    }
                }
                false
            };
            project(false, orth, emit) || project(true, diag, emit)
        };
        let (bishops, unicorns, dragons) = (source.lbishop() & friendly, source.lunicorn() & friendly, source.ldragon() & friendly);
        if bishops | unicorns != 0 {
            for d in ORTHOGONAL_TL {
                if scan_direction(d, bishops, unicorns, emit) {
                    return true;
                }
            }
        }
        if unicorns | dragons != 0 {
            for d in DIAGONAL_TL {
                if scan_direction(d, unicorns, dragons, emit) {
                    return true;
                }
            }
        }
        false
    }

    /// Whether some leap (king, knight, pawn or brawn step to another board) of `friendly`
    /// pieces captures a royal: exactly when `scan_jumps` would emit something.
    fn jump_threat(&self, c: bool, p0: Vec4, source: &Board, friendly: Bitboard) -> bool {
        let royals_at = |d: Vec4| self.board_at(p0 + d, c).map(|b| b.royal() & b.hostile(c)).unwrap_or(0);
        let kings = source.lking() & friendly;
        let knights = source.lknight() & friendly;
        if kings | knights != 0 {
            for d in ORTHOGONAL_TL {
                let r1 = royals_at(d);
                if kings & king_spread(r1) != 0 {
                    return true;
                }
                if knights != 0 {
                    let jump1 = shift_north(shift_north(r1)) | shift_south(shift_south(r1)) | shift_east(shift_east(r1)) | shift_west(shift_west(r1));
                    let r2 = royals_at(d * 2);
                    let jump2 = shift_north(r2) | shift_south(r2) | shift_east(r2) | shift_west(r2);
                    if knights & (jump1 | jump2) != 0 {
                        return true;
                    }
                }
            }
        }
        if kings != 0 && DIAGONAL_TL.iter().any(|&d| kings & king_spread(royals_at(d)) != 0) {
            return true;
        }
        if knights != 0 && SP_KNIGHT.iter().any(|&d| knights & royals_at(d) != 0) {
            return true;
        }
        let forward_l = if c { 1 } else { -1 };
        let pawns = source.lpawn() & friendly;
        if pawns != 0 && [-1, 1].iter().any(|&dt| pawns & royals_at(Vec4::new(0, 0, dt, forward_l)) != 0) {
            return true;
        }
        let brawns = source.lrawn() & friendly;
        if brawns != 0 {
            let behind = |r: Bitboard| if c { shift_north(r) } else { shift_south(r) };
            let rf = royals_at(Vec4::new(0, 0, 0, forward_l));
            if brawns & (behind(rf) | shift_east(rf) | shift_west(rf) | behind(royals_at(Vec4::new(0, 0, -1, 0)))) != 0 {
                return true;
            }
        }
        false
    }

    fn scan_jumps(
        &self,
        c: bool,
        p0: Vec4,
        source: &Board,
        friendly: Bitboard,
        emit: &mut dyn FnMut(FullMove) -> bool,
    ) -> bool {
        if !self.jump_threat(c, p0, source, friendly) {
            return false;
        }
        let jump = |direction: Vec4,
                    pieces: Bitboard,
                    attack: &dyn Fn(i32) -> Bitboard,
                    emit: &mut dyn FnMut(FullMove) -> bool|
         -> bool {
            if pieces == 0 {
                return false;
            }
            let q = p0 + direction;
            let b = match self.board_at(q, c) {
                Some(b) => b,
                None => return false,
            };
            let royals = b.royal() & b.hostile(c);
            if royals == 0 {
                return false;
            }
            for src in marked_pos(pieces) {
                let p = Vec4::from_xy(src, p0);
                for dst in marked_pos(attack(src) & royals) {
                    if emit(FullMove::new(p, Vec4::from_xy(dst, q))) {
                        return true;
                    }
                }
            }
            false
        };
        let same_xy = |src: i32| pmask(src);
        let kja = |src: i32| king_jump_attack(src);
        let nj1 = |src: i32| knight_jump1_attack(src);
        let nj2 = |src: i32| knight_jump2_attack(src);
        let kings = source.lking() & friendly;
        let knights = source.lknight() & friendly;
        if kings | knights != 0 {
            for d in ORTHOGONAL_TL {
                if jump(d, kings, &kja, emit) {
                    return true;
                }
                if jump(d, knights, &nj1, emit) {
                    return true;
                }
                if jump(d * 2, knights, &nj2, emit) {
                    return true;
                }
            }
        }
        if kings != 0 {
            for d in DIAGONAL_TL {
                if jump(d, kings, &kja, emit) {
                    return true;
                }
            }
        }
        if knights != 0 {
            for d in SP_KNIGHT {
                if jump(d, knights, &same_xy, emit) {
                    return true;
                }
            }
        }
        let forward_l = if c { 1 } else { -1 };
        let pawns = source.lpawn() & friendly;
        if pawns != 0 {
            for dt in [-1, 1] {
                if jump(Vec4::new(0, 0, dt, forward_l), pawns, &same_xy, emit) {
                    return true;
                }
            }
        }
        let forward_rank = move |src: i32| if c { shift_south(pmask(src)) } else { shift_north(pmask(src)) };
        let brawn_fwd = move |src: i32| forward_rank(src) | shift_west(pmask(src)) | shift_east(pmask(src));
        let brawns = source.lrawn() & friendly;
        if jump(Vec4::new(0, 0, 0, forward_l), brawns, &brawn_fwd, emit) {
            return true;
        }
        if jump(Vec4::new(0, 0, -1, 0), brawns, &forward_rank, emit) {
            return true;
        }
        false
    }
}
