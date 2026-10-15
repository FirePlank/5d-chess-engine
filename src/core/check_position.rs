//! Borrowed view for finding royal captures over a state plus overlaid boards.
//! Scan order is behavior: the first check found decides the problem slice the
//! hypercuboid search removes.

use super::action::FullMove;
use super::bitboard::*;
use super::board::Board;
use super::multiverse::*;
use super::piece::Piece;
use super::state::State;
use super::vec4::Vec4;

#[derive(Clone, Copy, Default)]
struct LineView<'a> {
    exists: bool,
    in_base: bool,
    start: Turn,
    end: Turn,
    added_start: Turn,
    added: Option<&'a Board>,
}

pub struct CheckPosition<'a> {
    base: &'a State,
    first_line: i32,
    lines: Vec<LineView<'a>>,
    /// Only pieces on these squares of a source board are scanned.
    source_mask: Bitboard,
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

impl<'a> CheckPosition<'a> {
    pub fn new(s: &'a State) -> Self {
        let (a, b) = s.get_lines_range();
        Self::with_range(s, a, b)
    }

    pub fn with_range(s: &'a State, l_min: i32, l_max: i32) -> Self {
        let mut lines = vec![LineView::default(); (l_max - l_min + 1) as usize];
        let (first, last) = s.get_lines_range();
        debug_assert!(l_min <= first && last <= l_max);
        for l in first..=last {
            let line = &mut lines[(l - l_min) as usize];
            line.exists = true;
            line.in_base = true;
            line.start = s.get_timeline_start(l);
            line.end = s.get_timeline_end(l);
        }
        CheckPosition { base: s, first_line: l_min, lines, source_mask: !0 }
    }

    fn for_present(s: &'a State, present: Turn) -> Self {
        let mut r = Self::new(s);
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
        Self::for_present(s, s.get_present())
    }

    pub fn for_check_symbols(s: &'a State) -> Self {
        let mut present = s.apparent_present();
        if present.1 == s.get_present().1 {
            present = next_turn(present);
        }
        Self::for_present(s, present)
    }

    pub fn for_move_candidates(s: &'a State) -> Self {
        let (lo, hi) = s.get_lines_range();
        let branch = s.new_line();
        let mut r = Self::with_range(s, lo.min(branch), hi.max(branch));
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
        let line = &mut self.lines[(l - self.first_line) as usize];
        debug_assert!(line.added.is_none());
        if line.exists {
            debug_assert!(at == next_turn(line.end));
        } else {
            line.exists = true;
            line.start = at;
        }
        line.added_start = at;
        line.end = at;
        line.added = Some(b);
    }

    #[inline]
    pub fn get_board_ptr(&self, l: i32, t: i32, c: bool) -> Option<&'a Board> {
        if l < self.first_line || l - self.first_line >= self.lines.len() as i32 {
            return None;
        }
        let line = &self.lines[(l - self.first_line) as usize];
        let at = (t, c);
        if !line.exists || at < line.start || at > line.end {
            return None;
        }
        if let Some(a) = line.added {
            if at >= line.added_start {
                return Some(a);
            }
        }
        if line.in_base { self.base.get_board_ptr(l, t, c) } else { None }
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
        let mut saved = [LineView::default(); 2];
        let mut count = 0;
        for upd in overlays.iter().flatten() {
            let start = (upd.t, upd.c);
            sources[count] = upd.l;
            let line = &mut self.lines[(upd.l - self.first_line) as usize];
            saved[count] = *line;
            if !line.exists {
                line.start = start;
            }
            line.exists = true;
            line.added_start = start;
            line.end = next_turn(start);
            line.added = Some(upd.value);
            count += 1;
        }
        let first_line = self.first_line;
        let result = {
            let lines = &self.lines;
            let mut emit = |m: FullMove| -> bool {
                match kind {
                    CheckKind::Physical => m.from.tl() == m.to.tl(),
                    CheckKind::Superphysical => m.from.tl() != m.to.tl(),
                    CheckKind::Historical => {
                        let target = &lines[(m.to.l() - first_line) as usize];
                        (m.to.t(), attacker) != target.end
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

    pub fn checks(&self, attacker: bool, include_physical: bool) -> Vec<FullMove> {
        let mut r = Vec::new();
        self.scan(
            attacker,
            include_physical,
            &mut |m| {
                r.push(m);
                false
            },
            &[],
            true,
        );
        r
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
            if !line.exists || line.end.1 != c {
                return false;
            }
            let p0 = Vec4::new(0, 0, line.end.0, self.first_line + i as i32);
            let source = self.get_board_ptr(p0.l(), p0.t(), c).unwrap();
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
                let b = match self.get_board_ptr(q.l(), q.t(), c) {
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
        for d in ORTHOGONAL_TL {
            if scan_direction(d, source.lrook() & friendly, emit) {
                return true;
            }
        }
        for d in DIAGONAL_TL {
            if scan_direction(d, source.lbishop() & friendly, emit) {
                return true;
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
            let mut any_royal = 0u64;
            let mut q = p0 + direction;
            while count < 7 {
                let b = match self.get_board_ptr(q.l(), q.t(), c) {
                    Some(b) => b,
                    None => break,
                };
                count += 1;
                occupied_boards[count] = b.occupied();
                royal_boards[count] = b.royal() & b.hostile(c);
                any_royal |= royal_boards[count];
                q = q + direction;
            }
            if any_royal == 0 {
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
        for d in ORTHOGONAL_TL {
            if scan_direction(d, source.lbishop() & friendly, source.lunicorn() & friendly, emit) {
                return true;
            }
        }
        for d in DIAGONAL_TL {
            if scan_direction(d, source.lunicorn() & friendly, source.ldragon() & friendly, emit) {
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
        let jump = |direction: Vec4,
                    pieces: Bitboard,
                    attack: &dyn Fn(i32) -> Bitboard,
                    emit: &mut dyn FnMut(FullMove) -> bool|
         -> bool {
            if pieces == 0 {
                return false;
            }
            let q = p0 + direction;
            let b = match self.get_board_ptr(q.l(), q.t(), c) {
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
        for d in ORTHOGONAL_TL {
            if jump(d, source.lking() & friendly, &kja, emit) {
                return true;
            }
            if jump(d, source.lknight() & friendly, &nj1, emit) {
                return true;
            }
            if jump(d * 2, source.lknight() & friendly, &nj2, emit) {
                return true;
            }
        }
        for d in DIAGONAL_TL {
            if jump(d, source.lking() & friendly, &kja, emit) {
                return true;
            }
        }
        for d in SP_KNIGHT {
            if jump(d, source.lknight() & friendly, &same_xy, emit) {
                return true;
            }
        }
        let forward_l = if c { 1 } else { -1 };
        for dt in [-1, 1] {
            if jump(Vec4::new(0, 0, dt, forward_l), source.lpawn() & friendly, &same_xy, emit) {
                return true;
            }
        }
        let forward_rank = move |src: i32| if c { shift_south(pmask(src)) } else { shift_north(pmask(src)) };
        let brawn_fwd = move |src: i32| forward_rank(src) | shift_west(pmask(src)) | shift_east(pmask(src));
        if jump(Vec4::new(0, 0, 0, forward_l), source.lrawn() & friendly, &brawn_fwd, emit) {
            return true;
        }
        if jump(Vec4::new(0, 0, -1, 0), source.lrawn() & friendly, &forward_rank, emit) {
            return true;
        }
        false
    }
}
