//! Handcrafted evaluation in centipawns (side-to-move perspective).
//!
//! Scores the endpoint board of every timeline (material + piece-square), weighting active
//! timelines fully and inactive ones partially, plus timeline-reserve and tempo terms.

use crate::core::bitboard::*;
use crate::core::board::Board;
use crate::core::piece::*;
use crate::core::state::State;

#[derive(Clone, Debug)]
pub struct EvalParams {
    pub pawn: i32,
    pub knight: i32,
    pub bishop: i32,
    pub rook: i32,
    pub queen: i32,
    pub princess: i32,
    pub unicorn: i32,
    pub dragon: i32,
    pub brawn: i32,
    pub common_king: i32,
    /// Weight (in 1/16) of inactive timelines relative to active ones.
    pub inactive_weight: i32,
    pub timeline_slot: i32,
    /// Extra value per timeline slot at full queen/bishop material, fading as they come off.
    pub timeline_slot_scaled: i32,
    pub tempo: i32,
    pub center_knight: i32,
    pub center_bishop: i32,
    pub center_other: i32,
    pub pawn_advance: i32,
    pub king_center_penalty: i32,
}

impl Default for EvalParams {
    fn default() -> Self {
        EvalParams {
            pawn: 100,
            knight: 400,
            bishop: 450,
            rook: 330,
            queen: 1250,
            princess: 900,
            unicorn: 250,
            dragon: 150,
            brawn: 150,
            common_king: 300,
            inactive_weight: 8,
            timeline_slot: 30,
            timeline_slot_scaled: 0,
            tempo: 15,
            center_knight: 8,
            center_bishop: 5,
            center_other: 2,
            pawn_advance: 6,
            king_center_penalty: 4,
        }
    }
}

#[inline(always)]
fn centrality(sq: i32, w: i32, h: i32) -> i32 {
    // 0 at the edge, grows toward the middle (for 8x8: 0..3).
    let x = sq & 7;
    let y = sq >> 3;
    let dx = (2 * x - (w - 1)).abs();
    let dy = (2 * y - (h - 1)).abs();
    ((w + h - 2) - dx - dy).max(0) / 2
}

/// White-minus-black score of one board.
fn board_score(b: &Board, p: &EvalParams, w: i32, h: i32) -> i32 {
    let mut score = 0;
    for (color, sign) in [(b.white() & !b.black(), 1), (b.black() & !b.white(), -1)] {
        let black = sign < 0;
        let mut add = |bb: Bitboard, value: i32, center: i32| {
            for sq in marked_pos(bb) {
                score += sign * (value + center * centrality(sq, w, h));
            }
        };
        add(b.pawn() & color, p.pawn, 0);
        add(b.brawn() & color, p.brawn, 0);
        add(b.knight() & color, p.knight, p.center_knight);
        add(b.bishop() & color, p.bishop, p.center_bishop);
        add(b.rook() & color, p.rook, p.center_other);
        add(b.queen() & color, p.queen, p.center_other);
        add(b.princess() & color, p.princess, p.center_other);
        add(b.unicorn() & color, p.unicorn, p.center_other);
        add(b.dragon() & color, p.dragon, p.center_other);
        add(b.common_king() & color, p.common_king, p.center_other);
        for sq in marked_pos(b.lpawn() & color) {
            let rank = sq >> 3;
            let adv = if black { h - 1 - rank } else { rank };
            score += sign * p.pawn_advance * (adv - 1).max(0);
        }
        for sq in marked_pos(b.royal() & color) {
            score -= sign * p.king_center_penalty * centrality(sq, w, h);
        }
    }
    score
}

pub fn evaluate(s: &State, p: &EvalParams) -> i32 {
    let _p = crate::misc::prof::scope(9);
    let (l_min, l_max) = s.get_lines_range();
    let (amin, amax) = s.get_active_range();
    let (w, h) = s.get_board_size();
    let mut total = 0i32;
    let (mut heavy, mut active_boards) = (0i32, 0i32);
    for l in l_min..=l_max {
        let (t, c) = s.get_timeline_end(l);
        let b = s.board(l, t, c);
        let sc = board_score(b, p, w, h);
        if l >= amin && l <= amax {
            total += sc * 16;
            heavy += (b.queen() | b.bishop() | b.princess()).count_ones() as i32;
            active_boards += 1;
        } else {
            total += sc * p.inactive_weight;
        }
    }
    total /= 16;
    let (l0_min, l0_max) = s.get_initial_lines_range();
    let white_created = l_max - l0_max;
    let black_created = l0_min - l_min;
    let avail_w = (1 + black_created - white_created).clamp(0, 4);
    let avail_b = (1 + white_created - black_created).clamp(0, 4);
    let slot = if p.timeline_slot_scaled != 0 {
        // Six queens/bishops per board is the full complement.
        let full = 6 * active_boards.max(1);
        p.timeline_slot + p.timeline_slot_scaled * heavy.min(full) / full
    } else {
        p.timeline_slot
    };
    total += slot * (avail_w - avail_b);
    let stm = if s.get_present().1 { -total } else { total };
    stm + p.tempo
}

/// Static value of a piece for move ordering.
#[inline]
pub fn piece_value(pc: Piece, p: &EvalParams) -> i32 {
    match to_white(piece_name(pc)) {
        PAWN_W => p.pawn,
        BRAWN_W => p.brawn,
        KNIGHT_W => p.knight,
        BISHOP_W => p.bishop,
        ROOK_W => p.rook,
        QUEEN_W => p.queen,
        PRINCESS_W => p.princess,
        UNICORN_W => p.unicorn,
        DRAGON_W => p.dragon,
        COMMON_KING_W => p.common_king,
        KING_W | ROYAL_QUEEN_W => 2000,
        _ => 0,
    }
}
