//! One 2D board as a set of bitboards. Piece types are encoded by movement components
//! (a queen is rook+bishop+unicorn+dragon).

use super::bitboard::*;
use super::piece::*;

const WHITE: usize = 0;
const BLACK: usize = 1;
const ROYAL: usize = 2;
const LKING: usize = 3;
const LKNIGHT: usize = 4;
const LPAWN: usize = 5;
const LRAWN: usize = 6;
const LROOK: usize = 7;
const LBISHOP: usize = 8;
const LUNICORN: usize = 9;
const LDRAGON: usize = 10;
const N_BBS: usize = 11;

#[derive(Clone, Copy, Hash, Default)]
pub struct Board {
    bbs: [Bitboard; N_BBS],
    umove_mask: Bitboard,
}

impl PartialEq for Board {
    /// Word by word without branching (the derived comparison calls `memcmp`).
    #[inline]
    fn eq(&self, o: &Self) -> bool {
        let mut d = self.umove_mask ^ o.umove_mask;
        for (a, b) in self.bbs.iter().zip(o.bbs.iter()) {
            d |= a ^ b;
        }
        d == 0
    }
}
impl Eq for Board {}

#[derive(Debug, Clone)]
pub struct FenError(pub String);

#[derive(Clone, Copy, Debug)]
pub struct RoyalSafety {
    pub in_check: bool,
    pub pinned: Bitboard,
}

impl std::fmt::Debug for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Board({})", self.get_fen(true))
    }
}

impl std::fmt::Display for FenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Board {
    #[inline(always)]
    pub const fn umove(&self) -> Bitboard {
        self.umove_mask
    }
    #[inline(always)]
    pub const fn white(&self) -> Bitboard {
        self.bbs[WHITE]
    }
    #[inline(always)]
    pub const fn black(&self) -> Bitboard {
        self.bbs[BLACK]
    }
    #[inline(always)]
    pub const fn wall(&self) -> Bitboard {
        self.bbs[WHITE] & self.bbs[BLACK]
    }
    #[inline(always)]
    pub const fn royal(&self) -> Bitboard {
        self.bbs[ROYAL]
    }
    #[inline(always)]
    pub const fn lking(&self) -> Bitboard {
        self.bbs[LKING]
    }
    #[inline(always)]
    pub const fn lknight(&self) -> Bitboard {
        self.bbs[LKNIGHT]
    }
    #[inline(always)]
    pub const fn lpawn(&self) -> Bitboard {
        self.bbs[LPAWN]
    }
    #[inline(always)]
    pub const fn lrawn(&self) -> Bitboard {
        self.bbs[LRAWN]
    }
    #[inline(always)]
    pub const fn lrook(&self) -> Bitboard {
        self.bbs[LROOK]
    }
    #[inline(always)]
    pub const fn lbishop(&self) -> Bitboard {
        self.bbs[LBISHOP]
    }
    #[inline(always)]
    pub const fn lunicorn(&self) -> Bitboard {
        self.bbs[LUNICORN]
    }
    #[inline(always)]
    pub const fn ldragon(&self) -> Bitboard {
        self.bbs[LDRAGON]
    }
    #[inline(always)]
    pub const fn king(&self) -> Bitboard {
        self.bbs[LKING] & self.bbs[ROYAL]
    }
    #[inline(always)]
    pub const fn common_king(&self) -> Bitboard {
        self.bbs[LKING] & !self.bbs[ROYAL]
    }
    #[inline(always)]
    pub const fn knight(&self) -> Bitboard {
        self.bbs[LKNIGHT]
    }
    #[inline(always)]
    pub const fn pawn(&self) -> Bitboard {
        self.bbs[LPAWN] & !self.bbs[LRAWN]
    }
    #[inline(always)]
    pub const fn brawn(&self) -> Bitboard {
        self.bbs[LPAWN] & self.bbs[LRAWN]
    }
    #[inline(always)]
    pub const fn rook(&self) -> Bitboard {
        self.bbs[LROOK] & !self.bbs[LBISHOP]
    }
    #[inline(always)]
    pub const fn bishop(&self) -> Bitboard {
        self.bbs[LBISHOP] & !self.bbs[LROOK]
    }
    #[inline(always)]
    pub const fn unicorn(&self) -> Bitboard {
        self.bbs[LUNICORN] & !self.bbs[LDRAGON]
    }
    #[inline(always)]
    pub const fn dragon(&self) -> Bitboard {
        self.bbs[LDRAGON] & !self.bbs[LUNICORN]
    }
    #[inline(always)]
    pub const fn princess(&self) -> Bitboard {
        self.bbs[LROOK] & self.bbs[LBISHOP] & !self.bbs[LUNICORN]
    }
    #[inline(always)]
    pub const fn royal_queen(&self) -> Bitboard {
        self.bbs[LROOK] & self.bbs[LDRAGON] & self.bbs[ROYAL]
    }
    #[inline(always)]
    pub const fn queen(&self) -> Bitboard {
        self.bbs[LROOK] & self.bbs[LDRAGON] & !self.bbs[ROYAL]
    }
    #[inline(always)]
    pub const fn sliding(&self) -> Bitboard {
        self.bbs[LROOK] | self.bbs[LBISHOP] | self.bbs[LUNICORN] | self.bbs[LDRAGON]
    }
    #[inline(always)]
    pub const fn occupied(&self) -> Bitboard {
        self.bbs[WHITE] | self.bbs[BLACK]
    }
    /// Pieces of color `c` (walls count as friendly blockers).
    #[inline(always)]
    pub const fn friendly(&self, c: bool) -> Bitboard {
        if c { self.bbs[BLACK] } else { self.bbs[WHITE] }
    }
    /// Enemy pieces of color `c`, walls excluded.
    #[inline(always)]
    pub const fn hostile(&self, c: bool) -> Bitboard {
        if c {
            self.bbs[WHITE] & !self.bbs[BLACK]
        } else {
            self.bbs[BLACK] & !self.bbs[WHITE]
        }
    }

    pub fn from_fen(fen: &str, size_x: i32, size_y: i32) -> Result<Board, FenError> {
        let arr = parse_array_fen(fen, size_x, size_y)?;
        let mut b = Board::default();
        for i in 0..64 {
            let p0 = arr[i as usize];
            b.set_piece(i, piece_name(p0));
            if piece_umove_flag(p0) {
                b.umove_mask |= pmask(i);
            }
        }
        Ok(b)
    }

    pub fn get_piece(&self, pos: i32) -> Piece {
        let z = pmask(pos);
        let white = z & self.bbs[WHITE] != 0;
        let black = z & self.bbs[BLACK] != 0;
        if !white && !black {
            return NO_PIECE;
        }
        let w = if z & self.king() != 0 {
            KING_W
        } else if z & self.common_king() != 0 {
            COMMON_KING_W
        } else if z & self.queen() != 0 {
            QUEEN_W
        } else if z & self.royal_queen() != 0 {
            ROYAL_QUEEN_W
        } else if z & self.bishop() != 0 {
            BISHOP_W
        } else if z & self.knight() != 0 {
            KNIGHT_W
        } else if z & self.rook() != 0 {
            ROOK_W
        } else if z & self.pawn() != 0 {
            PAWN_W
        } else if z & self.unicorn() != 0 {
            UNICORN_W
        } else if z & self.dragon() != 0 {
            DRAGON_W
        } else if z & self.brawn() != 0 {
            BRAWN_W
        } else if z & self.princess() != 0 {
            PRINCESS_W
        } else if white && z & self.wall() != 0 {
            return WALL_PIECE;
        } else {
            panic!("board::get_piece: unknown piece");
        };
        if white { w } else { to_black(w) }
    }

    /// White-normalized piece on `sq` (same as `to_white(piece_name(get_piece(sq)))`),
    /// decoded straight from the bitboards.
    #[inline(always)]
    pub fn kind(&self, sq: i32) -> Piece {
        let z = 1u64 << sq;
        if self.occupied() & z == 0 {
            return NO_PIECE;
        }
        if self.lpawn() & z != 0 {
            return if self.lrawn() & z != 0 { BRAWN_W } else { PAWN_W };
        }
        if self.lknight() & z != 0 {
            return KNIGHT_W;
        }
        if self.lking() & z != 0 {
            return if self.royal() & z != 0 { KING_W } else { COMMON_KING_W };
        }
        let (r, bi, d) = (self.lrook() & z != 0, self.lbishop() & z != 0, self.ldragon() & z != 0);
        if r && d {
            return if self.royal() & z != 0 { ROYAL_QUEEN_W } else { QUEEN_W };
        }
        if r && bi {
            return PRINCESS_W;
        }
        if r {
            return ROOK_W;
        }
        if bi {
            return BISHOP_W;
        }
        if self.lunicorn() & z != 0 {
            return UNICORN_W;
        }
        if d {
            return DRAGON_W;
        }
        WALL_PIECE
    }

    pub fn set_piece(&mut self, pos: i32, p: Piece) {
        let z = pmask(pos);
        self.umove_mask &= !z;
        for bb in self.bbs.iter_mut() {
            *bb &= !z;
        }
        if p == NO_PIECE {
            return;
        }
        if p == WALL_PIECE {
            self.bbs[WHITE] |= z;
            self.bbs[BLACK] |= z;
            return;
        }
        if !piece_color(p) {
            self.bbs[WHITE] |= z;
        } else {
            self.bbs[BLACK] |= z;
        }
        match to_white(piece_name(p)) {
            KING_W => {
                self.bbs[ROYAL] |= z;
                self.bbs[LKING] |= z;
            }
            COMMON_KING_W => self.bbs[LKING] |= z,
            ROOK_W => self.bbs[LROOK] |= z,
            BISHOP_W => self.bbs[LBISHOP] |= z,
            UNICORN_W => self.bbs[LUNICORN] |= z,
            DRAGON_W => self.bbs[LDRAGON] |= z,
            ROYAL_QUEEN_W | QUEEN_W => {
                if to_white(piece_name(p)) == ROYAL_QUEEN_W {
                    self.bbs[ROYAL] |= z;
                }
                self.bbs[LROOK] |= z;
                self.bbs[LBISHOP] |= z;
                self.bbs[LUNICORN] |= z;
                self.bbs[LDRAGON] |= z;
            }
            PRINCESS_W => {
                self.bbs[LROOK] |= z;
                self.bbs[LBISHOP] |= z;
            }
            KNIGHT_W => self.bbs[LKNIGHT] |= z,
            BRAWN_W => {
                self.bbs[LRAWN] |= z;
                self.bbs[LPAWN] |= z;
            }
            PAWN_W => self.bbs[LPAWN] |= z,
            _ => panic!("board::set_piece: {} not implemented", p),
        }
    }

    /// Move whatever stands on `from` to `to` (capturing), clearing unmoved flags on both.
    /// Same result as `set_piece(to, get_piece(from)); set_piece(from, NO_PIECE)`.
    #[inline(always)]
    pub fn transfer(&mut self, from: i32, to: i32) {
        let (zf, zt) = (pmask(from), pmask(to));
        for bb in self.bbs.iter_mut() {
            let had = *bb & zf != 0;
            *bb &= !(zf | zt);
            if had {
                *bb |= zt;
            }
        }
        self.umove_mask &= !(zf | zt);
    }

    /// Put the piece standing on `from` of `src` onto `to` of this board (capturing).
    /// Same result as `set_piece(to, src.get_piece(from))`.
    #[inline(always)]
    pub fn place_from(&mut self, to: i32, src: &Board, from: i32) {
        let (zf, zt) = (pmask(from), pmask(to));
        for (bb, sb) in self.bbs.iter_mut().zip(src.bbs.iter()) {
            *bb &= !zt;
            if sb & zf != 0 {
                *bb |= zt;
            }
        }
        self.umove_mask &= !zt;
    }

    /// Empty one square.
    #[inline(always)]
    pub fn clear_square(&mut self, pos: i32) {
        let z = !pmask(pos);
        for bb in self.bbs.iter_mut() {
            *bb &= z;
        }
        self.umove_mask &= z;
    }

    /// Copy of this board with one square replaced.
    #[inline]
    pub fn replace_piece(&self, pos: i32, p: Piece) -> Board {
        let mut b = *self;
        b.set_piece(pos, p);
        b
    }
    /// Copy of this board with the piece on `from` moved to `to` (losing its unmoved flag).
    #[inline]
    pub fn move_piece(&self, from: i32, to: i32) -> Board {
        let mut b = *self;
        b.transfer(from, to);
        b
    }

    pub fn get_fen(&self, show_umove: bool) -> String {
        let mut result = String::from("/");
        for y in (0..8).rev() {
            for x in 0..8 {
                match self.get_piece(ppos(x, y)) {
                    NO_PIECE => {
                        let last = *result.as_bytes().last().unwrap();
                        if last.is_ascii_digit() {
                            result.pop();
                            result.push((last + 1) as char);
                        } else {
                            result.push('1');
                        }
                    }
                    WALL_PIECE => continue,
                    p => {
                        result.push(p as char);
                        if show_umove && self.umove() & pmask(ppos(x, y)) != 0 {
                            result.push('*');
                        }
                    }
                }
            }
            if !result.ends_with('/') {
                result.push('/');
            }
        }
        result.remove(0);
        result.pop();
        result
    }

    pub fn to_string_pretty(&self) -> String {
        let mut s = String::new();
        for y in (0..8).rev() {
            for x in 0..8 {
                let pos = ppos(x, y);
                let p = self.get_piece(pos);
                let um = self.umove() & pmask(pos) != 0;
                match p {
                    NO_PIECE => s.push_str(". "),
                    WALL_PIECE => s.push_str("##"),
                    _ => {
                        s.push(p as char);
                        s.push(if um { '\'' } else { ' ' });
                    }
                }
            }
            s.push('\n');
        }
        s
    }

    /// File of the first occupied square (walls included) from (x, y) along the rank in
    /// direction `dx`, or a file off the board when there is none.
    pub fn first_occupied_x(&self, x: i32, y: i32, dx: i32) -> i32 {
        let mut i = x + dx;
        while (0..BOARD_LENGTH).contains(&i) && self.occupied() & pmask(ppos(i, y)) == 0 {
            i += dx;
        }
        i
    }

    /// All pieces (both colors) that attack `pos` on this board.
    pub fn attacks_to(&self, pos: i32) -> Bitboard {
        let w = self.white();
        let b = self.black();
        let pawns = self.lpawn();
        let all = w | b;
        (white_pawn_attack(pos) & pawns & b)
            | (black_pawn_attack(pos) & pawns & w)
            | (king_attack(pos) & self.lking())
            | (knight_attack(pos) & self.lknight())
            | (rook_attack(pos, all) & self.lrook())
            | (bishop_attack(pos, all) & self.lbishop())
    }

    /// Whether a piece hostile to `color` attacks `pos`.
    #[inline]
    pub fn is_under_attack(&self, pos: i32, color: bool) -> bool {
        self.is_attacked_through(pos, color, self.occupied())
    }

    /// `is_under_attack` with sliders blocked by `all` instead of this board's occupancy.
    #[inline]
    pub fn is_attacked_through(&self, pos: i32, color: bool, all: Bitboard) -> bool {
        let hostile = if color { self.white() } else { self.black() };
        let pawns = self.lpawn();
        if !color && white_pawn_attack(pos) & pawns & hostile != 0 {
            return true;
        }
        if color && black_pawn_attack(pos) & pawns & hostile != 0 {
            return true;
        }
        if king_attack(pos) & self.lking() & hostile != 0 {
            return true;
        }
        if knight_attack(pos) & self.lknight() & hostile != 0 {
            return true;
        }
        if bishop_attack(pos, all) & self.lbishop() & hostile != 0 {
            return true;
        }
        if rook_attack(pos, all) & self.lrook() & hostile != 0 {
            return true;
        }
        false
    }

    /// Whether color `c` has a royal under attack, and which of its pieces are pinned to a
    /// royal by an enemy slider (moving them off the line could expose it).
    pub fn royal_safety(&self, c: bool) -> RoyalSafety {
        let own = self.friendly(c);
        let enemy = self.hostile(c);
        let occ = self.occupied();
        let rooks = self.lrook() & enemy;
        let bishops = self.lbishop() & enemy;
        let mut r = RoyalSafety { in_check: false, pinned: 0 };
        for k in marked_pos(self.royal() & own) {
            if self.is_under_attack(k, c) {
                r.in_check = true;
            }
            let pinners = (rook_attack(k, 0) & rooks) | (bishop_attack(k, 0) & bishops);
            for sq in marked_pos(pinners) {
                let btw = BETWEEN[k as usize][sq as usize] & occ;
                if btw.count_ones() == 1 {
                    r.pinned |= btw & own;
                }
            }
        }
        r
    }

    pub fn raw_bbs(&self) -> [Bitboard; 12] {
        let mut out = [0; 12];
        out[..11].copy_from_slice(&self.bbs);
        out[11] = self.umove_mask;
        out
    }
}

fn parse_array_fen(fen: &str, x_size: i32, y_size: i32) -> Result<[Piece; 64], FenError> {
    let mut piece = [WALL_PIECE; 64];
    let bytes = fen.as_bytes();
    let (mut x, mut y) = (0i32, y_size - 1);
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            if x >= x_size {
                return Err(FenError(format!("Bad FEN, x={} overflows in \n{}", x, fen)));
            }
            let p = match c {
                b'K' => KING_UW,
                b'R' => ROOK_UW,
                b'P' => PAWN_UW,
                b'W' => BRAWN_UW,
                b'k' => KING_UB,
                b'r' => ROOK_UB,
                b'p' => PAWN_UB,
                b'w' => BRAWN_UB,
                _ => {
                    return Err(FenError(format!(
                        "Bad FEN: undefined behavior: * after [{}] in \n{}",
                        c as char, fen
                    )));
                }
            };
            piece[ppos(x, y) as usize] = p;
            i += 2;
            x += 1;
            continue;
        } else if c == b'/' {
            y -= 1;
            if y < 0 {
                return Err(FenError(format!("Bad FEN, y={} overflows in \n{}", y, fen)));
            }
            x = 0;
        } else if c.is_ascii_whitespace() {
        } else {
            let shift = if c.is_ascii_digit() { (c - b'0') as i32 } else { 1 };
            if x + shift > x_size {
                return Err(FenError(format!("Bad FEN, x={} overflows in \n{}", x + shift, fen)));
            }
            if c.is_ascii_digit() {
                for _ in 0..shift {
                    piece[ppos(x, y) as usize] = NO_PIECE;
                    x += 1;
                }
            } else if b"KQBNRPUDWSYCkqbnrpudwsyc".contains(&c) {
                piece[ppos(x, y) as usize] = c;
                x += 1;
            } else {
                return Err(FenError(format!("Bad FEN, unknown piece [{}] in \n{}", c as char, fen)));
            }
        }
        i += 1;
    }
    Ok(piece)
}
