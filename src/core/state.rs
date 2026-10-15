//! Game state: a multiverse plus whose turn it is and the stored present.

use super::action::{special, Action, ExtMove, FullMove};
use super::bitboard::*;
use super::board::Board;
use super::check_position::CheckPosition;
use super::hypercuboid::HcInfo;
use super::multiverse::*;
use super::piece::*;
use super::promotion::*;
use super::vec4::Vec4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MateType {
    None,
    Checkmate,
    Softmate,
    Stalemate,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MatchStatus {
    Playing,
    WhiteWins,
    BlackWins,
    Stalemate,
}

#[derive(Clone)]
pub struct State {
    pub(crate) m: Multiverse,
    pub(crate) present: i32,
    pub(crate) player: bool,
    pub(crate) promotions: PromotionOptions,
    /// Board mutations since tracking was enabled, for `unmake`.
    ops: Vec<Op>,
    track: bool,
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Append(i32),
    Insert(i32, (i32, i32, i32, i32)),
}

/// Restore point for `State::unmake`.
#[derive(Clone, Copy, Debug)]
pub struct Mark {
    ops: usize,
    present: i32,
    player: bool,
}

/// Timelines split by playability for the player to move.
#[derive(Clone, Default, Debug)]
pub struct TimelineStatus {
    pub mandatory: Vec<i32>,
    pub optional: Vec<i32>,
    pub unplayable: Vec<i32>,
}
impl TimelineStatus {
    pub fn playable(&self) -> Vec<i32> {
        let mut v = self.mandatory.clone();
        v.extend_from_slice(&self.optional);
        v
    }
}

impl State {
    pub fn new(m: Multiverse, promotions: PromotionOptions) -> State {
        let (present, player) = m.get_present();
        State { m, present, player, promotions, ops: Vec::new(), track: false }
    }

    #[inline(always)]
    pub fn multiverse(&self) -> &Multiverse {
        &self.m
    }
    #[inline(always)]
    pub fn get_present(&self) -> Turn {
        (self.present, self.player)
    }
    #[inline(always)]
    pub fn player(&self) -> bool {
        self.player
    }
    /// Position hash (all boards, side to move, stored present).
    #[inline(always)]
    pub fn hash(&self) -> u64 {
        self.m.hash() ^ (self.player as u64).wrapping_mul(0x9e3779b97f4a7c15) ^ (self.present as u64).wrapping_mul(0xd6e8feb86659fd93)
    }
    #[inline(always)]
    pub fn apparent_present(&self) -> Turn {
        self.m.get_present()
    }
    #[inline(always)]
    pub fn get_promotion_options(&self) -> PromotionOptions {
        self.promotions
    }
    pub fn set_promotion_options(&mut self, p: PromotionOptions) {
        self.promotions = p;
    }
    #[inline(always)]
    pub fn get_board_size(&self) -> (i32, i32) {
        self.m.get_board_size()
    }
    #[inline(always)]
    pub fn get_initial_lines_range(&self) -> (i32, i32) {
        self.m.get_initial_lines_range()
    }
    #[inline(always)]
    pub fn get_lines_range(&self) -> (i32, i32) {
        self.m.get_lines_range()
    }
    #[inline(always)]
    pub fn get_active_range(&self) -> (i32, i32) {
        self.m.get_active_range()
    }
    #[inline(always)]
    pub fn get_timeline_start(&self, l: i32) -> Turn {
        self.m.get_timeline_start(l)
    }
    #[inline(always)]
    pub fn get_timeline_end(&self, l: i32) -> Turn {
        self.m.get_timeline_end(l)
    }
    #[inline(always)]
    pub fn get_piece(&self, p: Vec4, color: bool) -> Piece {
        self.m.get_piece(p, color)
    }
    #[inline(always)]
    pub fn get_board_ptr(&self, l: i32, t: i32, c: bool) -> Option<&Board> {
        self.m.get_board_ptr(l, t, c)
    }
    #[inline(always)]
    pub fn board(&self, l: i32, t: i32, c: bool) -> &Board {
        self.m.board(l, t, c)
    }
    pub fn get_boards(&self) -> Vec<(i32, i32, bool, String)> {
        self.m.get_boards(false)
    }
    pub fn pretty_l(&self, l: i32) -> String {
        self.m.pretty_l(l)
    }
    pub fn pretty_lt(&self, p: Vec4) -> String {
        self.m.pretty_lt(p)
    }
    pub fn gen_piece_move(&self, p: Vec4, out: &mut Vec<Vec4>) {
        self.m.gen_piece_move(p, self.player, out)
    }

    /// Record board mutations from now on so they can be undone with `unmake`.
    pub fn enable_undo(&mut self) {
        self.track = true;
        self.ops.clear();
    }

    #[inline]
    pub fn mark(&self) -> Mark {
        Mark { ops: self.ops.len(), present: self.present, player: self.player }
    }

    /// Undo every move and submit since `mark` (requires `enable_undo`).
    pub fn unmake(&mut self, mark: Mark) {
        while self.ops.len() > mark.ops {
            match self.ops.pop().unwrap() {
                Op::Append(l) => self.m.pop_board(l),
                Op::Insert(l, ranges) => self.m.remove_last_line(l, ranges),
            }
        }
        self.present = mark.present;
        self.player = mark.player;
    }

    #[inline]
    fn append(&mut self, l: i32, b: Board) {
        if self.track {
            self.ops.push(Op::Append(l));
        }
        self.m.append_board(l, b);
    }

    /// A pass: every board the side to move must play on is carried forward unchanged (the
    /// phantom turn behind check detection) and the turn is submitted. Not a legal action;
    /// for null-move search. Undo with `unmake`.
    pub fn make_null(&mut self) -> Mark {
        let mark = self.mark();
        let (amin, amax) = self.m.get_active_range();
        let present = (self.present, self.player);
        for l in amin..=amax {
            let (t, c) = self.m.get_timeline_end(l);
            if (t, c) == present {
                let b = *self.m.board(l, t, c);
                self.append(l, b);
            }
        }
        self.submit::<true>();
        mark
    }

    pub fn new_line(&self) -> i32 {
        let (l_min, l_max) = self.m.get_lines_range();
        if !self.player { l_max + 1 } else { l_min - 1 }
    }

    pub fn can_submit(&self) -> Option<State> {
        let mut s = self.clone();
        if s.submit::<false>() { Some(s) } else { None }
    }

    pub fn can_apply(&self, fm: FullMove, promote_to: Piece) -> Option<State> {
        let normalized = self.normalize_promotion(ExtMove::new(fm, promote_to))?;
        let mut s = self.clone();
        if s.apply_ext::<false>(normalized) { Some(s) } else { None }
    }

    pub fn can_apply_action(&self, act: &Action) -> Option<State> {
        let mut s = self.clone();
        for em in act.moves() {
            if !s.apply_ext::<false>(*em) {
                return None;
            }
        }
        if !s.submit::<false>() {
            return None;
        }
        Some(s)
    }

    /// Apply a move, resolving the configured promotion. Returns false if illegal
    /// (pseudo-legality only) unless `UNSAFE`.
    pub fn apply_move<const UNSAFE: bool>(&mut self, fm: FullMove) -> bool {
        match self.normalize_promotion(ExtMove::new(fm, NO_PIECE)) {
            Some(n) => self.apply_ext::<UNSAFE>(n),
            None => false,
        }
    }

    pub fn apply_ext<const UNSAFE: bool>(&mut self, mv: ExtMove) -> bool {
        let ExtMove { fm, promote_to } = mv;
        let p = fm.from;
        let q = fm.to;
        let d = q - p;
        let player = self.player;
        if !UNSAFE {
            match self.normalize_promotion(mv) {
                Some(n) if n == mv => {}
                _ => return false,
            }
            match self.m.get_board_ptr(p.l(), p.t(), player) {
                Some(b) if b.hostile(!player) & pmask(p.xy()) != 0 => {}
                _ => return false,
            }
            let mut ml = MoveList::new();
            self.m.gen_moves(p, player, &mut ml);
            match ml.iter().find(|(tl, _)| *tl == q.tl()) {
                Some((_, bb)) => {
                    if pmask(q.xy()) & bb == 0 {
                        return false;
                    }
                }
                None => return false,
            }
        }
        let (_, size_y) = self.m.get_board_size();
        if d.tl() == Vec4::ZERO {
            let b = *self.m.board(p.l(), p.t(), player);
            let z = pmask(p.xy());
            let nb = if b.lpawn() & z != 0 && d.x() != 0 && b.get_piece(q.xy()) == NO_PIECE {
                b.replace_piece(ppos(q.x(), p.y()), NO_PIECE).move_piece(p.xy(), q.xy())
            } else if b.lpawn() & z != 0 && (q.y() == 0 || q.y() == size_y - 1) {
                if self.promotions == PromotionOptions::NONE {
                    b.move_piece(p.xy(), q.xy())
                } else {
                    let promoted = if player { to_black(promote_to) } else { promote_to };
                    b.replace_piece(p.xy(), NO_PIECE).replace_piece(q.xy(), promoted)
                }
            } else if b.king() & z != 0 && d.x().abs() > 1 {
                let rook_x1 = b.first_occupied_x(p.x(), p.y(), d.x().signum());
                let rook_x2 = p.x() + d.x().signum();
                b.move_piece(ppos(rook_x1, p.y()), ppos(rook_x2, q.y())).move_piece(p.xy(), q.xy())
            } else {
                b.move_piece(p.xy(), q.xy())
            };
            self.append(p.l(), nb);
        } else if (q.t(), player) == self.m.get_timeline_end(q.l()) {
            let b = *self.m.board(p.l(), p.t(), player);
            let pic = piece_name(b.get_piece(p.xy()));
            self.append(p.l(), b.replace_piece(p.xy(), NO_PIECE));
            let z = pmask(p.xy());
            let cb = *self.m.board(q.l(), q.t(), player);
            let nb = if b.lrawn() & z != 0 && (q.y() == 0 || q.y() == size_y - 1) {
                if self.promotions == PromotionOptions::NONE {
                    cb.replace_piece(q.xy(), pic)
                } else {
                    let promoted = if player { to_black(promote_to) } else { promote_to };
                    cb.replace_piece(q.xy(), promoted)
                }
            } else {
                cb.replace_piece(q.xy(), pic)
            };
            self.append(q.l(), nb);
        } else {
            let b = *self.m.board(p.l(), p.t(), player);
            let pic = piece_name(b.get_piece(p.xy()));
            self.append(p.l(), b.replace_piece(p.xy(), NO_PIECE));
            let xb = *self.m.board(q.l(), q.t(), player);
            let (t, c) = next_turn((q.t(), player));
            let z = pmask(p.xy());
            let nb = if b.lrawn() & z != 0 && (q.y() == 0 || q.y() == size_y - 1) {
                if self.promotions == PromotionOptions::NONE {
                    xb.replace_piece(q.xy(), pic)
                } else {
                    let promoted = if player { to_black(promote_to) } else { promote_to };
                    xb.replace_piece(q.xy(), promoted)
                }
            } else {
                xb.replace_piece(q.xy(), pic)
            };
            let nl = self.new_line();
            if self.track {
                self.ops.push(Op::Insert(nl, self.m.ranges()));
            }
            self.m.insert_board(nl, t, c, nb);
            let (new_present, _) = self.m.get_present();
            if new_present < self.present {
                self.present = new_present;
            }
        }
        true
    }

    pub fn normalize_promotion(&self, mut mv: ExtMove) -> Option<ExtMove> {
        let p = mv.fm.from;
        let q = mv.fm.to;
        let d = q - p;
        let (_, size_y) = self.get_board_size();
        let last_rank = q.y() == 0 || q.y() == size_y - 1;
        if !last_rank {
            mv.promote_to = NO_PIECE;
            return Some(mv);
        }
        let piece = piece_name(self.get_piece(p, self.player));
        let physical = d.l() == 0
            && d.t() == 0
            && (piece == PAWN_W || piece == PAWN_B || piece == BRAWN_W || piece == BRAWN_B);
        let superphysical = (d.l() != 0 || d.t() != 0) && (piece == BRAWN_W || piece == BRAWN_B);
        if !physical && !superphysical {
            mv.promote_to = NO_PIECE;
            return Some(mv);
        }
        if self.promotions == PromotionOptions::NONE {
            if mv.promote_to != NO_PIECE {
                return None;
            }
            return Some(mv);
        }
        if mv.promote_to == NO_PIECE {
            mv.promote_to = default_promote_to(self.promotions);
        }
        if !can_promote_to(self.promotions, mv.promote_to) {
            return None;
        }
        Some(mv)
    }

    pub fn submit<const UNSAFE: bool>(&mut self) -> bool {
        let (t, c) = self.m.get_present();
        if !UNSAFE && self.player == c {
            return false;
        }
        self.present = t;
        self.player = c;
        true
    }

    pub fn has_phantom_check(&self) -> bool {
        CheckPosition::for_phantom(self).first_check(!self.player, true).is_some()
    }

    pub fn phantom(&self) -> State {
        let (l_min, l_max) = self.m.get_active_range();
        let mut s = self.clone();
        for l in l_min..=l_max {
            let turn = self.get_timeline_end(l);
            if turn == self.get_present() {
                s.m.append_board(l, *self.m.board(l, turn.0, turn.1));
            }
        }
        s
    }

    pub fn get_timeline_status(&self) -> TimelineStatus {
        self.get_timeline_status_at(self.present, self.player)
    }

    pub fn get_timeline_status_at(&self, present_t: i32, present_c: bool) -> TimelineStatus {
        let (l_min, l_max) = self.m.get_lines_range();
        let (amin, amax) = self.m.get_active_range();
        let mut st = TimelineStatus::default();
        for l in l_min..=l_max {
            let tc = self.m.get_timeline_end(l);
            if amin <= l && amax >= l && tc == (present_t, present_c) {
                st.mandatory.push(l);
            } else if present_c == tc.1 {
                st.optional.push(l);
            } else {
                st.unplayable.push(l);
            }
        }
        st
    }

    /// Moves by `attacker` that capture an enemy royal piece (lazy: stops when `f` returns true).
    pub fn find_checks_with(&self, attacker: bool, mut f: impl FnMut(FullMove) -> bool) {
        let (l_min, l_max) = self.m.get_lines_range();
        let mut ml = MoveList::new();
        for l in l_min..=l_max {
            let (t, color) = self.m.get_timeline_end(l);
            if color != attacker {
                continue;
            }
            let b = self.m.board(l, t, attacker);
            let pieces = b.friendly(attacker) & !b.wall();
            for src in marked_pos(pieces) {
                let p = Vec4::from_xy(src, Vec4::new(0, 0, t, l));
                ml.clear();
                self.m.gen_moves(p, attacker, &mut ml);
                for &(q0, bb) in &ml {
                    let b1 = self.m.board(q0.l(), q0.t(), attacker);
                    if bb != 0 {
                        let cp = bb & b1.royal();
                        for dst in marked_pos(cp) {
                            if f(FullMove::new(p, Vec4::from_xy(dst, q0))) {
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn find_checks(&self, attacker: bool) -> Vec<FullMove> {
        let mut v = Vec::new();
        self.find_checks_with(attacker, |m| {
            v.push(m);
            false
        });
        v
    }

    pub fn first_check(&self, attacker: bool) -> Option<FullMove> {
        let mut r = None;
        self.find_checks_with(attacker, |m| {
            r = Some(m);
            true
        });
        r
    }

    pub fn get_movable_pieces(&self) -> Vec<Vec4> {
        let st = self.get_timeline_status();
        self.get_movable_pieces_in(&st.playable())
    }

    pub fn get_movable_pieces_in(&self, lines: &[i32]) -> Vec<Vec4> {
        let c = self.player;
        let mut out = Vec::new();
        for &l in lines {
            let (t, _) = self.get_timeline_end(l);
            let p0 = Vec4::new(0, 0, t, l);
            let b = self.m.board(l, t, c);
            let pieces = b.friendly(c) & !b.wall();
            for src in marked_pos(pieces) {
                let p = Vec4::from_xy(src, p0);
                if self.m.has_any_move(p, c) {
                    out.push(p);
                }
            }
        }
        out
    }

    pub fn get_all_pieces(&self, lines: &[i32]) -> Vec<Vec4> {
        let mut out = Vec::new();
        for &l in lines {
            let t = self.get_timeline_end(l).0;
            let p0 = Vec4::new(0, 0, t, l);
            let b = self.m.board(l, t, self.player);
            let pieces = (if self.player { b.black() } else { b.white() }) & !b.wall();
            for pos in marked_pos(pieces) {
                out.push(Vec4::from_xy(pos, p0));
            }
        }
        out
    }

    pub fn get_mate_type(&self) -> MateType {
        self.get_mate_type_impl(false)
    }
    pub fn is_softmate(&self) -> bool {
        self.get_mate_type_impl(true) == MateType::Softmate
    }

    fn get_mate_type_impl(&self, legal_action_witness: bool) -> MateType {
        let _p = crate::misc::prof::scope(10);
        let (w, mut ss) = HcInfo::build_hc(self);
        let back = ss.pop_back().unwrap();
        let ss1 = crate::misc::geometry::SearchSpace::from_vec(vec![back]);
        if w.search(self, ss1).next().is_some() {
            return MateType::None;
        }
        let mut ss2 = ss.clone();
        let (l0_min, l0_max) = self.get_initial_lines_range();
        let (l_min, l_max) = self.get_lines_range();
        let whites_lines = l_max - l0_max;
        let blacks_lines = l0_min - l_min;
        let timeline_advantage =
            if self.player { whites_lines - blacks_lines } else { blacks_lines - whites_lines };
        let fba = w.first_branch_axis() as i32;
        let max_axis = (fba + timeline_advantage + 1).min(w.dimension() as i32 - 1);
        let old_t = self.present;
        for hc in ss2.iter_mut() {
            let mut n = fba;
            while n <= max_axis {
                let axis = n as usize;
                hc[axis].erase_if(|i| match w.arrival_move(axis, i) {
                    Some(m) => m.to.t() < old_t,
                    None => false,
                });
                n += 1;
            }
        }
        if w.search(self, ss2).next().is_some() {
            return MateType::None;
        }
        if legal_action_witness {
            return if self.has_phantom_check() { MateType::Softmate } else { MateType::None };
        }
        if w.search(self, ss).next().is_some() {
            if self.has_phantom_check() { MateType::Softmate } else { MateType::None }
        } else if self.has_phantom_check() {
            MateType::Checkmate
        } else {
            MateType::Stalemate
        }
    }

    pub fn show_fen(&self) -> String {
        let mut s = String::new();
        for (l, t, c, fen) in self.m.get_boards(true) {
            s += &format!("[{}:{}:{}:{}]\n", fen, self.m.pretty_l(l), t, if c { "b" } else { "w" });
        }
        s
    }

    pub fn to_string_pretty(&self) -> String {
        format!("State(present={}, player={}):\n", self.present, self.player as i32) + &self.m.to_string_pretty()
    }
}

// ------------------------------------------------------------------ move queries

impl FullMove {
    /// Coordinates of the moved piece after the move.
    pub fn new_position(&self, s: &State) -> Vec4 {
        let player = s.get_present().1;
        let branching = self.from.tl() != self.to.tl() && s.get_timeline_end(self.to.l()) != (self.to.t(), player);
        Vec4::new(self.to.x(), self.to.y(), self.to.t() + 1, if branching { s.new_line() } else { self.to.l() })
    }

    pub fn moved_piece(&self, s: &State) -> Piece {
        s.get_piece(self.from, s.get_present().1)
    }

    pub fn captured_piece(&self, s: &State) -> Piece {
        let player = s.get_present().1;
        if self.from.tl() == self.to.tl() {
            let source = s.board(self.from.l(), self.from.t(), player);
            let origin = pmask(self.from.xy());
            if source.king() & origin != 0 && (self.from.x() - self.to.x()).abs() > 1 {
                return NO_PIECE;
            }
            let destination = source.get_piece(self.to.xy());
            if source.lpawn() & origin != 0 && self.from.x() != self.to.x() && destination == NO_PIECE {
                return source.get_piece(ppos(self.to.x(), self.from.y()));
            }
            return destination;
        }
        s.get_piece(self.to, player)
    }

    pub fn move_type(&self, s: &State) -> u8 {
        let mut result = special::NONE;
        let player = s.get_present().1;
        let source = s.board(self.from.l(), self.from.t(), player);
        let origin = pmask(self.from.xy());
        let height = s.get_board_size().1;
        let physical = self.from.tl() == self.to.tl();
        if !physical {
            result |= special::SUPERPHYSICAL;
            if s.get_timeline_end(self.to.l()) != (self.to.t(), player) {
                result |= special::BRANCHING;
            }
        }
        if self.captured_piece(s) != NO_PIECE {
            result |= special::CAPTURE;
        }
        if physical && source.lpawn() & origin != 0 {
            if self.from.x() != self.to.x() && source.get_piece(self.to.xy()) == NO_PIECE {
                result |= special::EN_PASSANT;
            } else if self.to.y() == 0 || self.to.y() == height - 1 {
                result |= special::PROMOTION;
            }
        } else if !physical && source.lrawn() & origin != 0 && (self.to.y() == 0 || self.to.y() == height - 1) {
            result |= special::PROMOTION;
        } else if physical && source.king() & origin != 0 && (self.from.x() - self.to.x()).abs() > 1 {
            result |= if self.to.x() > self.from.x() { special::CASTLE_KINGSIDE } else { special::CASTLE_QUEENSIDE };
        }
        result
    }

    /// LAN with an explicit promotion letter where it applies.
    pub fn lan(&self, s: &State, promote_to: Piece) -> String {
        let player = s.get_present().1;
        let pic = to_white(piece_name(s.get_piece(self.from, player)));
        let mut r = self.to_string();
        if promote_to != NO_PIECE
            && (pic == PAWN_W || pic == BRAWN_W)
            && self.to.y() == if player { 0 } else { s.get_board_size().1 - 1 }
        {
            r.push(promote_to as char);
        }
        r
    }
}

impl ExtMove {
    pub fn lan(&self, s: &State) -> String {
        match s.normalize_promotion(*self) {
            Some(n) => self.fm.lan(s, n.promote_to),
            None => "---INVALID MOVE---".into(),
        }
    }
}
