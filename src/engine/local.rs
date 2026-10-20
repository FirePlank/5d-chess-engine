//! Single-board tactics: pending captures at a quiescent leaf, resolved board by board.
//! Each board gets its own capture search and stand-pat, and the gains are summed, which is
//! far cheaper than enumerating multi-board turns.

use crate::core::bitboard::*;
use crate::core::board::Board;
use crate::core::hypercuboid::has_physical_check;
use crate::core::piece::*;

/// Capture plies searched per board.
const LOCAL_DEPTH: i32 = 6;
const ROYAL_VALUE: i32 = 20_000;

/// Piece values (eval units), indexed by white-normalized piece code.
pub type Values = [i32; 128];

/// Best material gain `c` can force on `b` (c to move) with captures and promotions only,
/// both sides free to stop at any point; never negative.
pub fn capture_gain(b: &Board, c: bool, height: i32, vals: &Values) -> i32 {
    search(b, c, 0, i32::MAX / 2, LOCAL_DEPTH, height, vals)
}

fn search(b: &Board, c: bool, alpha: i32, beta: i32, depth: i32, height: i32, vals: &Values) -> i32 {
    if depth == 0 || beta <= 0 {
        return 0;
    }
    let mut alpha = alpha.max(0);
    let mine = b.hostile(!c);
    let theirs = b.hostile(c);
    let occ = b.occupied();
    let last_rank = if c { 0 } else { height - 1 };
    // (ordering key, from, to, gain, promotes)
    let mut moves: [(i32, i32, i32, i32, bool); 64] = [(0, 0, 0, 0, false); 64];
    let mut n = 0;
    for from in marked_pos(mine) {
        let z = pmask(from);
        let mover = b.kind(from);
        let pawn = b.lpawn() & z != 0;
        let mut att = 0;
        if pawn {
            att |= if c { black_pawn_attack(from) } else { white_pawn_attack(from) };
        }
        if b.lknight() & z != 0 {
            att |= knight_attack(from);
        }
        if b.lking() & z != 0 {
            att |= king_attack(from);
        }
        if b.lrook() & z != 0 {
            att |= rook_attack(from, occ);
        }
        if b.lbishop() & z != 0 {
            att |= bishop_attack(from, occ);
        }
        let mut targets = att & theirs;
        if pawn {
            // Quiet promotions.
            let push = if c { shift_south(z) } else { shift_north(z) } & !occ;
            if push != 0 && (bb_get_pos(push) >> 3) == last_rank {
                targets |= push;
            }
        }
        for to in marked_pos(targets) {
            if n == moves.len() {
                break;
            }
            let victim = if b.royal() & pmask(to) != 0 { ROYAL_VALUE } else { vals[b.kind(to) as usize] };
            if victim == ROYAL_VALUE {
                // The opponent left a royal en prise on this board.
                return ROYAL_VALUE;
            }
            let promotes = pawn && (to >> 3) == last_rank;
            let gain = victim + if promotes { vals[QUEEN_W as usize] - vals[mover as usize] } else { 0 };
            moves[n] = (gain * 16 - vals[mover as usize] / 16, from, to, gain, promotes);
            n += 1;
        }
    }
    let moves = &mut moves[..n];
    moves.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    let mut best = 0;
    for &(_, from, to, gain, promotes) in moves.iter() {
        let mut nb = *b;
        if promotes {
            nb.clear_square(from);
            nb.set_piece(to, if c { to_black(QUEEN_W) } else { QUEEN_W });
        } else {
            nb.transfer(from, to);
        }
        if has_physical_check(&nb, c) {
            continue;
        }
        let v = gain - search(&nb, !c, gain - beta, gain - alpha, depth - 1, height, vals);
        if v > best {
            best = v;
            if v > alpha {
                alpha = v;
                if v >= beta {
                    break;
                }
            }
        }
    }
    best
}

/// Piece values for `capture_gain` from the evaluation's material weights on active boards
/// (pawn, brawn, knight, bishop, rook, queen, princess, unicorn, dragon, common king).
pub fn values_from(material: &[f32]) -> Values {
    let mut v = [0i32; 128];
    let kinds = [PAWN_W, BRAWN_W, KNIGHT_W, BISHOP_W, ROOK_W, QUEEN_W, PRINCESS_W, UNICORN_W, DRAGON_W, COMMON_KING_W];
    for (k, &p) in kinds.iter().enumerate() {
        // A piece is never worth less than a pawn in an exchange.
        v[p as usize] = (material[k].round() as i32).max(100);
    }
    v[KING_W as usize] = ROYAL_VALUE;
    v[ROYAL_QUEEN_W as usize] = ROYAL_VALUE;
    v
}
