//! Checking sacrifices by time travel: the f-sac signature travels (`SacBot`, a test
//! opponent) and the general rule the search extends (`is_sac_travel`).

use crate::core::action::FullMove;
use crate::core::bitboard::*;
use crate::core::hypercuboid::has_physical_check;
use crate::core::multiverse::MoveList;
use crate::core::vec4::Vec4;
use crate::core::piece::*;
use crate::core::state::State;

// ------------------------------------------------------------------ the f-sac signature

/// Turns up to which a travel's target board makes the f-sac signature.
pub const SIG_TURN: i32 = 6;

/// The side to move's f-sac travels: a queen or bishop onto a square next to an enemy royal on
/// a past board up to turn `SIG_TURN`, capturing or checking. Ordered f7/f2 first, then queens.
pub fn signature_travels(s: &State) -> Vec<FullMove> {
    let m = s.multiverse();
    let c = s.get_present().1;
    let (lo, hi) = m.get_lines_range();
    let mut out: Vec<(i32, FullMove)> = Vec::new();
    let mut ml = MoveList::new();
    for l in lo..=hi {
        let (t, col) = m.get_timeline_end(l);
        if col != c {
            continue;
        }
        let b = m.board(l, t, c);
        for sq in marked_pos((b.queen() | b.bishop()) & b.hostile(!c)) {
            let queen = b.queen() & pmask(sq) != 0;
            let p = Vec4::from_xy(sq, Vec4::new(0, 0, t, l));
            ml.clear();
            m.gen_superphysical_moves(p, c, &mut ml);
            for &(q0, bb) in &ml {
                if q0.t() > SIG_TURN || m.get_timeline_end(q0.l()) == (q0.t(), c) {
                    continue;
                }
                let Some(tb) = m.get_board_ptr(q0.l(), q0.t(), c) else { continue };
                let royals = tb.royal() & tb.hostile(c);
                let mut zone = 0;
                for r in marked_pos(royals) {
                    zone |= king_attack(r);
                }
                for dst in marked_pos(bb & zone) {
                    let capture = tb.hostile(c) & pmask(dst) != 0;
                    let check = queen || marked_pos(royals & king_attack(dst)).any(|r| (r & 7) != (dst & 7) && (r >> 3) != (dst >> 3));
                    if capture || check {
                        let f_square = (dst & 7) == 5 && (dst >> 3 == 1 || dst >> 3 == 6);
                        out.push(((!f_square) as i32 * 2 + (!queen) as i32, FullMove::new(p, Vec4::from_xy(dst, q0))));
                    }
                }
            }
        }
    }
    out.sort_by_key(|x| x.0);
    out.into_iter().map(|x| x.1).collect()
}

/// Whether `m`, a move of the side to move in `s`, is a checking sacrifice by time travel: it
/// lands on a past board (creating a timeline), captures a piece there, gives check on the board
/// it creates, and only the opponent's royal can capture the piece back on that board.
pub fn is_sac_travel(s: &State, m: &FullMove) -> bool {
    let c = s.get_present().1;
    if (m.from.l(), m.from.t()) == (m.to.l(), m.to.t()) || (m.to.t(), c) == s.get_timeline_end(m.to.l()) {
        return false;
    }
    let (Some(src), Some(target)) = (s.get_board_ptr(m.from.l(), m.from.t(), c), s.get_board_ptr(m.to.l(), m.to.t(), c)) else { return false };
    let piece = src.kind(m.from.xy());
    let dst = m.to.xy();
    let victim = target.kind(dst);
    if piece == NO_PIECE || matches!(piece, KING_W | ROYAL_QUEEN_W) || matches!(victim, KING_W | ROYAL_QUEEN_W) {
        return false;
    }
    // Only a royal next to the landing square can be the one piece to take back (a royal queen
    // reaches further): most travels end here, before the board is built and checks are found.
    let royals = target.royal() & target.hostile(c);
    if royals & target.royal_queen() == 0 && marked_pos(royals).all(|r| king_attack(r) & pmask(dst) == 0) {
        return false;
    }
    // The board the travel creates: the piece stands on the captured piece's square.
    let mut b = *target;
    b.set_piece(dst, if c { to_black(piece) } else { piece });
    if !has_physical_check(&b, !c) {
        return false;
    }
    let enemy = b.hostile(c);
    let back = b.attacks_to(dst) & enemy;
    back != 0 && back & !b.royal() == 0
}
