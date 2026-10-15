//! Piece codes: ASCII letter, lowercase for black, high bit set for the "unmoved" variants of king/rook/pawn/brawn.

pub type Piece = u8;

pub const NO_PIECE: Piece = 0;
pub const WALL_PIECE: Piece = 1;

pub const KING_W: Piece = b'K';
pub const QUEEN_W: Piece = b'Q';
pub const BISHOP_W: Piece = b'B';
pub const KNIGHT_W: Piece = b'N';
pub const ROOK_W: Piece = b'R';
pub const PAWN_W: Piece = b'P';
pub const UNICORN_W: Piece = b'U';
pub const DRAGON_W: Piece = b'D';
pub const BRAWN_W: Piece = b'W';
pub const PRINCESS_W: Piece = b'S';
pub const ROYAL_QUEEN_W: Piece = b'Y';
pub const COMMON_KING_W: Piece = b'C';

pub const KING_B: Piece = b'k';
pub const QUEEN_B: Piece = b'q';
pub const BISHOP_B: Piece = b'b';
pub const KNIGHT_B: Piece = b'n';
pub const ROOK_B: Piece = b'r';
pub const PAWN_B: Piece = b'p';
pub const UNICORN_B: Piece = b'u';
pub const DRAGON_B: Piece = b'd';
pub const BRAWN_B: Piece = b'w';
pub const PRINCESS_B: Piece = b's';
pub const ROYAL_QUEEN_B: Piece = b'y';
pub const COMMON_KING_B: Piece = b'c';

pub const KING_UW: Piece = b'K' | 0x80;
pub const ROOK_UW: Piece = b'R' | 0x80;
pub const PAWN_UW: Piece = b'P' | 0x80;
pub const BRAWN_UW: Piece = b'W' | 0x80;
pub const KING_UB: Piece = b'k' | 0x80;
pub const ROOK_UB: Piece = b'r' | 0x80;
pub const PAWN_UB: Piece = b'p' | 0x80;
pub const BRAWN_UB: Piece = b'w' | 0x80;

pub const BOARD_BITS: i32 = 3;
pub const BOARD_LENGTH: i32 = 1 << BOARD_BITS;
pub const BOARD_SIZE: i32 = 1 << (2 * BOARD_BITS);

#[inline(always)]
pub const fn piece_umove_flag(p: Piece) -> bool {
    p & 0x80 != 0
}
#[inline(always)]
pub const fn piece_name(p: Piece) -> Piece {
    p & 0x7f
}
#[inline(always)]
pub const fn to_white(p: Piece) -> Piece {
    p & !0x20
}
#[inline(always)]
pub const fn to_black(p: Piece) -> Piece {
    p | 0x20
}
#[inline(always)]
pub const fn switch_color(p: Piece) -> Piece {
    p ^ 0x20
}
/// True for black (lowercase).
#[inline(always)]
pub const fn piece_color(p: Piece) -> bool {
    p & 0x20 != 0
}
#[inline(always)]
pub const fn ppos(x: i32, y: i32) -> i32 {
    x | (y << BOARD_BITS)
}
