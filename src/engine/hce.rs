//! Feature-based handcrafted evaluation: every term is an integer feature (white minus black
//! unless stated) and the evaluation is their weighted sum, so weights can be tuned on games.

use crate::core::bitboard::*;
use crate::core::board::Board;
use crate::core::check_position::CheckPosition;
use crate::core::state::State;

pub const NF: usize = 93;

/// Feature indices.
pub mod fi {
    /// Material on active timelines' latest boards, by kind (see `KINDS`).
    pub const MAT_A: usize = 0;
    /// Material on inactive timelines' latest boards.
    pub const MAT_I: usize = 10;
    /// Centrality: knights, bishops, rooks, queens/princesses, unicorns/dragons/common kings.
    pub const CENT: usize = 20;
    pub const PAWN_ADV: usize = 25;
    /// Pawns one or two steps from promotion.
    pub const PAWN_NEAR: usize = 26;
    pub const KING_CENT: usize = 27;
    /// Timeline slots available to white minus black (each side may lead by one line).
    pub const TL_SLOTS: usize = 28;
    /// Same, times the fraction of queens/bishops left (x8).
    pub const TL_SLOTS_HEAVY: usize = 29;
    // King safety, as white's danger minus black's danger.
    pub const K_HOLES: usize = 30;
    pub const K_KNIGHTS: usize = 31;
    pub const K_OPEN_ORTH: usize = 32;
    pub const K_OPEN_DIAG: usize = 33;
    pub const K_SHIELD: usize = 34;
    pub const K_ZONE_ATT: usize = 35;
    // Mobility (pseudo-legal physical moves).
    pub const MOB_N: usize = 36;
    pub const MOB_B: usize = 37;
    pub const MOB_R: usize = 38;
    pub const MOB_Q: usize = 39;
    pub const MOB_K: usize = 40;
    /// Value (in pawns x10) of pieces attacked and undefended: white's minus black's.
    pub const HANGING: usize = 41;
    /// Boards where a royal is attacked: white's minus black's.
    pub const ROYAL_ATT: usize = 42;
    /// +1 when white is to move, -1 for black.
    pub const TEMPO: usize = 43;
    /// Number of active timelines (scales nothing by itself; lets other terms be normalized).
    pub const ACTIVE_LINES: usize = 44;
    /// Royal captures the opponent would have if the side to move passed (5D, any board),
    /// signed like the king-safety terms: white's danger minus black's.
    pub const CHECKS_5D: usize = 45;
    /// The superphysical ones among them.
    pub const SP_CHECKS: usize = 46;
    /// Boards the side to move must play on (+ for white, - for black).
    pub const MANDATORY: usize = 47;
    // Pawn structure, attackers near the enemy royal, development (white minus black).
    pub const PASSED: usize = 48;
    pub const DOUBLED: usize = 49;
    pub const ISOLATED: usize = 50;
    pub const KING_PROX: usize = 51;
    pub const UNDEVELOPED: usize = 52;
    /// Own non-pawn pieces on neighbouring timelines' latest boards within two squares of
    /// an enemy royal (they can strike across timelines).
    pub const KING_PROX_5D: usize = 53;
    /// Own queens within two squares of an enemy royal on its board.
    pub const QUEEN_PROX: usize = 54;
    /// 5D king danger (`si::TRAVEL_PAST` ..= `si::INACTIVE_ROYAL_ATT`), white's minus black's.
    pub const DANGER_5D: usize = 55;
    /// Travel landings (past and now) next to the opponent's royals that the side to move
    /// has (+ for white to move, - for black).
    pub const TRAVEL_STM: usize = 67;
    /// Timeline slots by initiative: each side's slots times the danger to the enemy royals
    /// (`royal_danger`), and the side to move's slots (+ for white to move, - for black).
    pub const TL_SLOTS_THREAT: usize = 68;
    pub const TL_SLOTS_STM: usize = 69;
    /// Time-travel exposure of the squares next to enemy royals (`ex::F7_WEAK` ..= `ex::KN_CHECKS`),
    /// white's minus black's.
    pub const F7: usize = 70;
    /// The f7 sac table (`sac`), white's minus black's: created timelines whose latest board
    /// is a known local mate for the side (and how close the mate is: 12 - plies), winning
    /// travels it can play now (and those of the side to move, + for white to move), and open
    /// routes to winning travel targets.
    pub const SAC_LINE: usize = 75;
    pub const SAC_CLOSE: usize = 76;
    pub const SAC_NOW: usize = 77;
    pub const SAC_NOW_STM: usize = 78;
    pub const SAC_OPEN: usize = 79;
    /// King safety board by board (`ex::ZONE_HOT` ..= `ex::K_FLIGHT`), white's danger minus
    /// black's.
    pub const KS: usize = 80;
    /// Timeline handling: lost timelines and boards to move (white's minus black's), and the
    /// attacks around the side to move's and its opponent's royals on the boards it must play on
    /// (+ for white to move, - for black).
    pub const TL: usize = 86;
    /// Defence: defended squares around own royals on boards where the enemy moves next and on
    /// every board, and travel landings next to own royals where the enemy moves next (white's
    /// minus black's).
    pub const DEF: usize = 90;
}

pub const FEATURE_NAMES: [&str; NF] = [
    "mat_a_pawn", "mat_a_brawn", "mat_a_knight", "mat_a_bishop", "mat_a_rook", "mat_a_queen", "mat_a_princess",
    "mat_a_unicorn", "mat_a_dragon", "mat_a_cking", "mat_i_pawn", "mat_i_brawn", "mat_i_knight", "mat_i_bishop",
    "mat_i_rook", "mat_i_queen", "mat_i_princess", "mat_i_unicorn", "mat_i_dragon", "mat_i_cking", "cent_knight",
    "cent_bishop", "cent_rook", "cent_queen", "cent_other", "pawn_adv", "pawn_near", "king_cent", "tl_slots",
    "tl_slots_heavy", "k_holes", "k_knights", "k_open_orth", "k_open_diag", "k_shield", "k_zone_att", "mob_knight",
    "mob_bishop", "mob_rook", "mob_queen", "mob_king", "hanging", "royal_att", "tempo", "active_lines",
    "checks_5d", "sp_checks", "mandatory", "passed", "doubled", "isolated", "king_prox", "undeveloped",
    "king_prox_5d", "queen_prox", "travel_past", "travel_now", "travel_backed", "weak_sq", "open_orth5",
    "open_diag5", "open_multi5", "king_off", "king_off_new", "low_flight", "exiled", "inactive_royal_att",
    "travel_stm", "tl_slots_threat", "tl_slots_stm", "f7_weak", "f7_open", "f7_qreach", "f7_broute", "kn_checks",
    "sac_line", "sac_close", "sac_now", "sac_now_stm", "sac_open", "zone_hot", "weak_hot", "dmax5", "zone_max",
    "k_def", "k_flight",
    "tl_lost", "to_move", "zone_mand_stm", "zone_mand_opp",
    "def_hot", "def_board", "travel_hot",
];

pub type Features = [i32; NF];
pub type Weights = [f32; NF];

/// Weights reproducing the previous handcrafted evaluation (no king safety or mobility).
pub fn default_weights() -> Weights {
    let mut w = [0f32; NF];
    let vals = [100.0, 150.0, 400.0, 450.0, 330.0, 1250.0, 900.0, 250.0, 150.0, 300.0];
    for k in 0..10 {
        w[fi::MAT_A + k] = vals[k];
        w[fi::MAT_I + k] = vals[k] * 0.5;
    }
    w[fi::CENT] = 8.0;
    w[fi::CENT + 1] = 5.0;
    w[fi::CENT + 2] = 2.0;
    w[fi::CENT + 3] = 2.0;
    w[fi::CENT + 4] = 2.0;
    w[fi::PAWN_ADV] = 6.0;
    w[fi::KING_CENT] = -4.0;
    w[fi::TL_SLOTS] = 30.0;
    w[fi::TEMPO] = 15.0;
    w
}

/// Weights used by the alpha-beta search with `use_hce` (Texel-tuned, see bin/tune.rs).
pub fn tuned_weights() -> Weights {
    weights_by_name(&[
        ("mat_a_pawn", 508.5),
        ("mat_a_brawn", 150.0),
        ("mat_a_knight", 909.1),
        ("mat_a_bishop", 1638.7),
        ("mat_a_rook", 1094.2),
        ("mat_a_queen", 2341.8),
        ("mat_a_princess", 899.8),
        ("mat_a_unicorn", 250.0),
        ("mat_a_dragon", 150.0),
        ("mat_a_cking", 251.3),
        ("mat_i_pawn", -69.3),
        ("mat_i_brawn", 74.9),
        ("mat_i_knight", 218.9),
        ("mat_i_bishop", 656.9),
        ("mat_i_rook", 329.3),
        ("mat_i_queen", 1076.5),
        ("mat_i_princess", 450.0),
        ("mat_i_unicorn", 125.1),
        ("mat_i_dragon", 74.9),
        ("mat_i_cking", 150.0),
        ("cent_knight", 105.1),
        ("cent_bishop", -27.6),
        ("cent_rook", 22.7),
        ("cent_queen", 0.4),
        ("cent_other", 95.8),
        ("pawn_adv", -52.7),
        ("pawn_near", 374.2),
        ("king_cent", -49.3),
        ("tl_slots", 2516.4),
        ("tl_slots_heavy", -118.4),
        ("k_holes", -247.3),
        ("k_knights", 2548.7),
        ("k_open_orth", -540.5),
        ("k_open_diag", 113.6),
        ("k_shield", 763.6),
        ("k_zone_att", 105.1),
        ("mob_knight", 98.2),
        ("mob_bishop", 137.3),
        ("mob_rook", 130.5),
        ("mob_queen", 89.8),
        ("mob_king", 155.1),
        ("hanging", -16.7),
        ("royal_att", -2034.7),
        ("tempo", -140.4),
        ("active_lines", 75.3),
        ("checks_5d", 0.0),
        ("sp_checks", 0.0),
        ("mandatory", 454.0),
        ("passed", 107.8),
        ("doubled", 7.8),
        ("isolated", -42.7),
        ("king_prox", 991.5),
        ("undeveloped", -252.9),
        ("king_prox_5d", 62.7),
        ("queen_prox", -379.6),
        ("travel_past", 80.5),
        ("travel_now", 461.6),
        ("travel_backed", -522.7),
        ("weak_sq", 325.5),
        ("open_orth5", 0.0),
        ("open_diag5", -7.5),
        ("open_multi5", -16.5),
        ("king_off", -788.5),
        ("king_off_new", -1323.5),
        ("low_flight", 713.3),
        ("exiled", -8.7),
        ("inactive_royal_att", 0.0),
        ("travel_stm", 596.0),
        ("tl_slots_threat", 553.8),
        ("tl_slots_stm", -369.3),
        ("f7_weak", 15.1),
        ("f7_open", 76.0),
        ("f7_qreach", 197.5),
        ("f7_broute", 66.4),
        ("kn_checks", 280.9),
        // Set by hand (untuned; see `split_sac`), for defence: the local proofs ignore
        // time-travel defences and the other timelines, so they only nudge. A side to move
        // with a verified f7 travel on hand is a pawn and a half up (its opponent should have
        // stopped it), open routes to such a target cost the defender a little (prophylaxis
        // such as ...Nf6 or ...d5 is cheap), and a created timeline the local search shows
        // lost for the defender counts against it a little more. A closer proven mate there
        // adds a little per ply saved (moderate: the proofs can be false, e.g. Trojan's Trap),
        // so the f7 line does not hang on the net's correction alone.
        ("sac_line", 600.0),
        ("sac_close", 100.0),
        ("sac_now", 0.0),
        ("sac_now_stm", 700.0),
        ("sac_open", 150.0),
        ("zone_hot", 236.7),
        ("weak_hot", -42.4),
        ("dmax5", -136.7),
        ("zone_max", -338.5),
        ("k_def", -3.1),
        ("k_flight", 180.5),
        ("tl_lost", -61.3),
        ("to_move", 197.5),
        ("zone_mand_stm", -6.0),
        ("zone_mand_opp", 447.6),
        ("def_hot", -91.3),
        ("def_board", -3.1),
        ("travel_hot", -71.5),
    ])
}

#[inline(always)]
pub(crate) fn centrality(sq: i32, w: i32, h: i32) -> i32 {
    let x = sq & 7;
    let y = sq >> 3;
    let dx = (2 * x - (w - 1)).abs();
    let dy = (2 * y - (h - 1)).abs();
    ((w + h - 2) - dx - dy).max(0) / 2
}

pub(crate) const ORTH: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
pub(crate) const DIAG: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

/// Rays from `sq` that leave the board without meeting a piece of `own`.
pub(crate) fn open_rays(sq: i32, own: Bitboard, dirs: &[(i32, i32)], w: i32, h: i32) -> i32 {
    let (x0, y0) = (sq & 7, sq >> 3);
    let mut n = 0;
    for &(dx, dy) in dirs {
        let (mut x, mut y) = (x0 + dx, y0 + dy);
        let mut open = true;
        while x >= 0 && y >= 0 && x < w && y < h {
            if own & pmask(x + 8 * y) != 0 {
                open = false;
                break;
            }
            x += dx;
            y += dy;
        }
        n += open as i32;
    }
    n
}

pub(crate) fn kind_value10(b: &Board, sq: i32) -> i32 {
    let z = pmask(sq);
    if b.lpawn() & z != 0 {
        10
    } else if b.lknight() & z != 0 {
        40
    } else if b.lrook() & b.ldragon() & z != 0 {
        125
    } else if b.lrook() & b.lbishop() & z != 0 {
        90
    } else if b.lbishop() & z != 0 {
        45
    } else if b.lrook() & z != 0 {
        33
    } else if b.lking() & z != 0 && b.royal() & z == 0 {
        30
    } else if b.lunicorn() & z != 0 {
        25
    } else if b.ldragon() & z != 0 {
        15
    } else {
        0
    }
}

/// Attack map of one color's pieces on a board, with per-kind mobility counts.
pub(crate) struct Side {
    pub(crate) attacks: Bitboard,
    /// Squares attacked by pieces other than royals.
    pub(crate) attacks_nr: Bitboard,
    pub(crate) mob: [i32; 5],
}

pub(crate) fn side_attacks(b: &Board, c: bool) -> Side {
    let own = b.friendly(c);
    let mine = b.hostile(!c);
    let occ = b.occupied();
    let mut s = Side { attacks: 0, attacks_nr: 0, mob: [0; 5] };
    for sq in marked_pos(mine) {
        let z = pmask(sq);
        let a = if b.lpawn() & z != 0 {
            if c { black_pawn_attack(sq) } else { white_pawn_attack(sq) }
        } else if b.lknight() & z != 0 {
            let a = knight_attack(sq);
            s.mob[0] += (a & !own).count_ones() as i32;
            a
        } else if b.lking() & z != 0 {
            let a = king_attack(sq);
            s.mob[4] += (a & !own).count_ones() as i32;
            a
        } else {
            let r = b.lrook() & z != 0;
            let bi = b.lbishop() & z != 0;
            let mut a = 0;
            if r {
                a |= rook_attack(sq, occ);
            }
            if bi {
                a |= bishop_attack(sq, occ);
            }
            let m = (a & !own).count_ones() as i32;
            match (r, bi) {
                (true, true) => s.mob[3] += m,
                (true, false) => s.mob[2] += m,
                (false, true) => s.mob[1] += m,
                _ => {}
            }
            a
        };
        s.attacks |= a;
        if b.royal() & z == 0 {
            s.attacks_nr |= a;
        }
    }
    s
}

/// Per-side term indices of `Sides` (also the eval net's per-side inputs).
pub mod si {
    /// Pawn, brawn, knight, bishop, rook, queen, princess, unicorn, dragon, common king
    /// on active timelines' latest boards.
    pub const MAT_A: usize = 0;
    /// The same on inactive timelines.
    pub const MAT_I: usize = 10;
    /// Centrality sums: knights, bishops, rooks, queens/princesses, other leapers.
    pub const CENT: usize = 20;
    pub const PAWN_ADV: usize = 25;
    pub const PAWN_NEAR: usize = 26;
    pub const KING_CENT: usize = 27;
    /// Timeline slots this side can still use (0..4).
    pub const SLOTS: usize = 28;
    // King danger.
    pub const K_HOLES: usize = 29;
    pub const K_KNIGHTS: usize = 30;
    pub const K_OPEN_ORTH: usize = 31;
    pub const K_OPEN_DIAG: usize = 32;
    pub const K_SHIELD: usize = 33;
    pub const K_ZONE_ATT: usize = 34;
    /// Mobility: knights, bishops, rooks, queens, kings.
    pub const MOB: usize = 35;
    /// Value (pawns x10) of own pieces attacked and undefended.
    pub const HANGING: usize = 40;
    /// Boards where an own royal is attacked.
    pub const ROYAL_ATT: usize = 41;
    // 5D king danger (see `royal_5d`).
    /// Enemy time-travel landings onto past boards next to an own royal, on squares no other
    /// own piece defends (the f7 weakness of the f-sac).
    pub const TRAVEL_PAST: usize = 42;
    /// The same onto other timelines' latest boards (jumps without branching).
    pub const TRAVEL_NOW: usize = 43;
    /// Those landings (past or now) also covered by another enemy piece on the landing board
    /// ("defended travel": the royal cannot take the traveller).
    pub const TRAVEL_BACKED: usize = 44;
    /// Squares next to own royals on active boards that only the royal defends and the enemy
    /// attacks, physically or across time and timelines.
    pub const WEAK_SQ: usize = 45;
    /// Enemy sliders lined up on own royals through timelines or time, blocked by no own
    /// piece: orthogonals, diagonals, triagonals and quadragonals.
    pub const OPEN_ORTH5: usize = 46;
    pub const OPEN_DIAG5: usize = 47;
    pub const OPEN_MULTI5: usize = 48;
    /// Own royals on active boards off their home rank, and those on created timelines.
    pub const KING_OFF: usize = 49;
    pub const KING_OFF_NEW: usize = 50;
    /// Own royals on active boards with at most one safe square, under attack or with an
    /// attacked neighbour.
    pub const LOW_FLIGHT: usize = 51;
    /// Every own royal on an inactive timeline (loses a stalemate).
    pub const EXILED: usize = 52;
    /// Own royals on inactive timelines' latest boards under attack.
    pub const INACTIVE_ROYAL_ATT: usize = 53;
    pub const COUNT: usize = 54;
}

/// Per-side terms of a position from one pass over the timelines' latest boards; the
/// feature vector is (mostly) their white-minus-black difference.
pub struct Sides {
    /// [white, black]
    pub side: [[i32; si::COUNT]; 2],
    /// Queens, bishops and princesses on active boards.
    pub heavy: i32,
    pub active: i32,
    pub inactive: i32,
    /// Active boards the side to move must play on.
    pub mandatory: i32,
    /// Extra per-side terms (`ex`), not part of the eval net's inputs.
    pub extra: [[i32; ex::COUNT]; 2],
}

/// Extra per-side term indices of `Sides`.
pub mod ex {
    pub const PASSED: usize = 0;
    pub const DOUBLED: usize = 1;
    pub const ISOLATED: usize = 2;
    /// Own non-pawn pieces within two squares of an enemy royal.
    pub const KING_PROX: usize = 3;
    /// Own knights and bishops still on the back rank.
    pub const UNDEVELOPED: usize = 4;
    pub const KING_PROX_5D: usize = 5;
    pub const QUEEN_PROX: usize = 6;
    /// The f7 weakness, against enemy royals on their home rank: squares in front of them that
    /// only the royal defends (empty or holding a pawn; the targets of time travel into the
    /// past), open lines from those squares towards this side (up to the past boards' travel
    /// squares, b3/f3/h5 for f7), whether an own queen is on or can reach such a line, and own
    /// bishops on the back rank of such a square's file with the pawn in front gone (the f1
    /// bishop's T7 -> T1 route once the f-pawn has moved).
    pub const F7_WEAK: usize = 7;
    pub const F7_OPEN: usize = 8;
    pub const F7_QREACH: usize = 9;
    pub const F7_BROUTE: usize = 10;
    /// Knight checks available against enemy royals (Nh3-g5+ in the f7 attack).
    pub const KN_CHECKS: usize = 11;
    /// The f7 sac table (`fi::SAC_LINE` ..).
    pub const SAC_LINE: usize = 12;
    pub const SAC_CLOSE: usize = 13;
    pub const SAC_NOW: usize = 14;
    pub const SAC_OPEN: usize = 15;
    // King safety board by board (dangers to this side's royals).
    /// Squares around own royals the enemy attacks, on boards where the enemy moves next.
    pub const ZONE_HOT: usize = 16;
    /// `si::WEAK_SQ` on boards where the enemy moves next.
    pub const WEAK_HOT: usize = 17;
    /// The most weak squares and travel landings around own royals on any one board (the
    /// worst board decides).
    pub const DMAX5: usize = 18;
    /// The most attacked squares around own royals on any one board.
    pub const ZONE_MAX: usize = 19;
    /// Squares around own royals that other own pieces defend.
    pub const K_DEF: usize = 20;
    /// Safe flight squares of own royals (at most 3 each).
    pub const K_FLIGHT: usize = 21;
    /// Timelines this side created that are inactive.
    pub const TL_LOST: usize = 22;
    /// Active boards where it is this side's turn.
    pub const TO_MOVE: usize = 23;
    /// Squares around this side's royals the enemy attacks, on the boards the side to move
    /// must play on.
    pub const ZONE_MAND: usize = 24;
    /// Squares around own royals that other own pieces defend (per board).
    pub const DEF_BOARD: usize = 25;
    /// `DEF_BOARD` on boards where the enemy moves next (where it can travel to next).
    pub const DEF_HOT: usize = 26;
    /// `si::TRAVEL_NOW` landings next to own royals on boards where the enemy moves next.
    pub const TRAVEL_HOT: usize = 27;
    pub const COUNT: usize = 28;
}

/// Squares within king distance 2.
#[inline]
fn ring2(sq: i32) -> Bitboard {
    let r1 = king_attack(sq) | pmask(sq);
    let r = r1 | shift_north(r1) | shift_south(r1);
    r | shift_east(r) | shift_west(r)
}

fn board_extra(b: &Board, extra: &mut [[i32; ex::COUNT]; 2], w: i32, h: i32, att_nr: [Bitboard; 2]) {
    let colors = [b.white() & !b.black(), b.black() & !b.white()];
    let occ = b.occupied();
    for c in 0..2 {
        let (me, them) = (colors[c], colors[1 - c]);
        let (own_p, their_p) = (b.lpawn() & me, b.lpawn() & them);
        let f = &mut extra[c];
        for sq in marked_pos(own_p) {
            let (x, y) = (sq & 7, sq >> 3);
            let file = A_FILE << x;
            let adj = if x > 0 { A_FILE << (x - 1) } else { 0 } | if x < 7 { A_FILE << (x + 1) } else { 0 };
            let ahead = if c == 0 {
                if y >= 7 { 0 } else { u64::MAX << (8 * (y + 1)) }
            } else if y == 0 {
                0
            } else {
                (1u64 << (8 * y)) - 1
            };
            if their_p & (file | adj) & ahead == 0 {
                f[ex::PASSED] += 1;
            }
            if own_p & file & ahead != 0 {
                f[ex::DOUBLED] += 1;
            }
            if own_p & adj == 0 {
                f[ex::ISOLATED] += 1;
            }
        }
        let pieces = me & !b.lpawn() & !b.royal();
        for k in marked_pos(b.royal() & them) {
            let r = ring2(k);
            f[ex::KING_PROX] += (r & pieces).count_ones() as i32;
            f[ex::QUEEN_PROX] += (r & b.queen() & me).count_ones() as i32;
        }
        let back = if c == 0 { 0xff } else { 0xffu64 << (8 * (h - 1)) };
        f[ex::UNDEVELOPED] += ((b.knight() | b.bishop()) & me & back).count_ones() as i32;
        // The f7 weakness of the enemy royals (see `ex::F7_WEAK`).
        for k in marked_pos(b.royal() & me) {
            f[ex::K_DEF] += (king_attack(k) & att_nr[c]).count_ones() as i32;
        }
        for k in marked_pos(b.royal() & me) {
            f[ex::DEF_BOARD] += (king_attack(k) & att_nr[c]).count_ones() as i32;
        }
        let royals = b.royal() & them;
        if royals == 0 {
            continue;
        }
        let queens = b.queen() & me;
        let mut qreach = queens;
        for q in marked_pos(queens) {
            qreach |= rook_attack(q, occ) | bishop_attack(q, occ);
        }
        let mut kn_to = 0;
        for n in marked_pos(b.knight() & me) {
            kn_to |= knight_attack(n);
        }
        kn_to &= !me;
        let (home, dy) = if c == 0 { (h - 1, -1) } else { (0, 1) };
        let front_rank = 0xffu64 << (8 * (home + dy));
        let bishops = b.lbishop() & !b.lrook() & me;
        let my_back = if c == 0 { 0 } else { h - 1 };
        for k in marked_pos(royals) {
            f[ex::KN_CHECKS] += (kn_to & knight_attack(k)).count_ones() as i32;
            if k >> 3 != home {
                continue;
            }
            let weak = king_attack(k) & front_rank & !att_nr[1 - c] & !(occ & !(b.lpawn() & them));
            for sq in marked_pos(weak) {
                f[ex::F7_WEAK] += 1;
                let mut lines = 0u64;
                let mut queen_on = false;
                for dx in -1..=1 {
                    let (mut x, mut y) = ((sq & 7) + dx, (sq >> 3) + dy);
                    let mut n = 0;
                    while x >= 0 && y >= 0 && x < w && y < h {
                        let z = pmask(x + 8 * y);
                        if occ & z != 0 {
                            queen_on |= queens & z != 0;
                            break;
                        }
                        lines |= z;
                        n += 1;
                        x += dx;
                        y += dy;
                    }
                    f[ex::F7_OPEN] += (n >= 2) as i32;
                }
                f[ex::F7_QREACH] += (queen_on || qreach & lines != 0) as i32;
                // The bishop's route back in time up the file is open once the pawn in front
                // of it has moved (the past boards' squares are what matter, not this one's).
                let file = sq & 7;
                f[ex::F7_BROUTE] += (bishops & pmask(file + 8 * my_back) != 0 && b.lpawn() & me & pmask(file + 8 * (my_back - dy)) == 0) as i32;
            }
        }
    }
}

/// Per-side terms of one active board.
fn board_sides(b: &Board, side: &mut [[i32; si::COUNT]; 2], w: i32, h: i32) -> [Side; 2] {
    let colors = [b.white() & !b.black(), b.black() & !b.white()];
    let kinds = [
        b.pawn(),
        b.brawn(),
        b.knight(),
        b.bishop(),
        b.rook(),
        b.queen(),
        b.princess(),
        b.unicorn(),
        b.dragon(),
        b.common_king(),
    ];
    for c in 0..2 {
        for k in 0..10 {
            side[c][si::MAT_A + k] += (kinds[k] & colors[c]).count_ones() as i32;
        }
    }
    let groups = [b.knight(), b.bishop(), b.rook(), b.queen() | b.princess(), b.unicorn() | b.dragon() | b.common_king()];
    let att = [side_attacks(b, false), side_attacks(b, true)];
    for c in 0..2 {
        let (me, them) = (colors[c], colors[1 - c]);
        let f = &mut side[c];
        for (g, bb) in groups.iter().enumerate() {
            for sq in marked_pos(bb & me) {
                f[si::CENT + g] += centrality(sq, w, h);
            }
        }
        for sq in marked_pos(b.lpawn() & me) {
            let rank = sq >> 3;
            let adv = if c == 1 { h - 1 - rank } else { rank };
            f[si::PAWN_ADV] += (adv - 1).max(0);
            f[si::PAWN_NEAR] += (adv >= h - 3) as i32;
        }
        for k in 0..5 {
            f[si::MOB + k] += att[c].mob[k];
        }
        let (my_att, their_att) = (att[c].attacks, att[1 - c].attacks);
        let own = b.friendly(c == 1);
        for k in marked_pos(b.royal() & me) {
            f[si::KING_CENT] += centrality(k, w, h);
            let zone = king_attack(k);
            f[si::K_HOLES] += (zone & !b.occupied()).count_ones() as i32;
            f[si::K_KNIGHTS] += (knight_attack(k) & b.knight() & them).count_ones() as i32;
            f[si::K_OPEN_ORTH] += open_rays(k, own, &ORTH, w, h);
            f[si::K_OPEN_DIAG] += open_rays(k, own, &DIAG, w, h);
            let fwd = if c == 1 { shift_south(pmask(k)) } else { shift_north(pmask(k)) };
            f[si::K_SHIELD] += ((fwd | shift_west(fwd) | shift_east(fwd)) & b.lpawn() & me).count_ones() as i32;
            f[si::K_ZONE_ATT] += (zone & their_att).count_ones() as i32;
            if their_att & pmask(k) != 0 {
                f[si::ROYAL_ATT] += 1;
            }
        }
        for sq in marked_pos(me & !b.royal() & their_att & !my_att) {
            f[si::HANGING] += kind_value10(b, sq);
        }
    }
    att
}

/// One board's contribution to `Sides` while its timeline is active (its material counts are
/// also what it contributes while inactive).
#[derive(Clone, Copy)]
pub struct BoardTerms {
    side: [[i32; si::COUNT]; 2],
    extra: [[i32; ex::COUNT]; 2],
    heavy: i32,
    /// Squares each color attacks: all pieces, and pieces other than royals.
    att: [Bitboard; 2],
    att_nr: [Bitboard; 2],
}

impl BoardTerms {
    const ZERO: BoardTerms =
        BoardTerms { side: [[0; si::COUNT]; 2], extra: [[0; ex::COUNT]; 2], heavy: 0, att: [0; 2], att_nr: [0; 2] };
}

pub fn board_terms(b: &Board, w: i32, h: i32) -> BoardTerms {
    let mut t = BoardTerms::ZERO;
    let att = board_sides(b, &mut t.side, w, h);
    t.att = [att[0].attacks, att[1].attacks];
    t.att_nr = [att[0].attacks_nr, att[1].attacks_nr];
    board_extra(b, &mut t.extra, w, h, t.att_nr);
    t.heavy = (b.queen() | b.bishop() | b.princess()).count_ones() as i32;
    t
}

/// Where `sides` gets each board's terms.
pub trait TermSource {
    fn terms(&mut self, b: &Board, w: i32, h: i32) -> &BoardTerms;
    /// Memoized `past_line` results by `past_key`.
    fn past_get(&mut self, _key: u64) -> Option<[i32; 2]> {
        None
    }
    fn past_put(&mut self, _key: u64, _v: [i32; 2]) {}
    /// The winning travels of the search's root (`sac::root_travels`), if known.
    fn sac_root(&self) -> Option<&super::sac::RootTravels> {
        None
    }
}

/// Terms computed on every request.
struct Direct(BoardTerms);
impl TermSource for Direct {
    fn terms(&mut self, b: &Board, w: i32, h: i32) -> &BoardTerms {
        self.0 = board_terms(b, w, h);
        &self.0
    }
}

/// Board terms memoized by board contents (direct-mapped; a hit is an exact board match).
pub struct TermCache {
    size: (i32, i32),
    slots: Vec<(Board, bool, BoardTerms)>,
    past: Vec<(u64, [i32; 2])>,
    /// The winning travels of the current search's root.
    pub sac_root: super::sac::RootTravels,
}

impl Default for TermCache {
    fn default() -> Self {
        TermCache {
            size: (0, 0),
            slots: vec![(Board::default(), false, BoardTerms::ZERO); 1 << 12],
            past: vec![(0, [0; 2]); 1 << 14],
            sac_root: super::sac::RootTravels::default(),
        }
    }
}

impl TermSource for TermCache {
    #[inline]
    fn past_get(&mut self, key: u64) -> Option<[i32; 2]> {
        let e = &self.past[(key >> 50) as usize];
        if e.0 == key && key != 0 { Some(e.1) } else { None }
    }
    #[inline]
    fn past_put(&mut self, key: u64, v: [i32; 2]) {
        self.past[(key >> 50) as usize] = (key, v);
    }
    #[inline]
    fn sac_root(&self) -> Option<&super::sac::RootTravels> {
        Some(&self.sac_root)
    }
    #[inline]
    fn terms(&mut self, b: &Board, w: i32, h: i32) -> &BoardTerms {
        if self.size != (w, h) {
            self.size = (w, h);
            self.slots.iter_mut().for_each(|e| e.1 = false);
        }
        let mut k = 0x9e3779b97f4a7c15u64;
        for x in b.raw_bbs() {
            k = (k ^ x).wrapping_mul(0xbf58476d1ce4e5b9);
            k ^= k >> 29;
        }
        let i = (k >> 52) as usize & (self.slots.len() - 1);
        let e = &mut self.slots[i];
        if !e.1 || e.0 != *b {
            *e = (*b, true, board_terms(b, w, h));
        }
        &e.2
    }
}

pub fn sides(s: &State) -> Sides {
    sides_with(s, &mut Direct(BoardTerms::ZERO))
}

pub fn sides_with(s: &State, src: &mut impl TermSource) -> Sides {
    let mut sd = Sides { side: [[0; si::COUNT]; 2], heavy: 0, active: 0, inactive: 0, mandatory: 0, extra: [[0; ex::COUNT]; 2] };
    let (l_min, l_max) = s.get_lines_range();
    let (amin, amax) = s.get_active_range();
    let (w, h) = s.get_board_size();
    let present = s.get_present();
    let (il_min, il_max) = s.get_initial_lines_range();
    let mut atts = [[0u64; 4]; ATT_LINES];
    for l in l_min..=l_max {
        let (t, c) = s.get_timeline_end(l);
        let b = s.board(l, t, c);
        let active = l >= amin && l <= amax;
        let bt = src.terms(b, w, h);
        if let Some(a) = atts.get_mut((l - l_min) as usize) {
            *a = [bt.att[0], bt.att[1], bt.att_nr[0], bt.att_nr[1]];
        }
        if active {
            for c in 0..2 {
                for i in 0..si::COUNT {
                    sd.side[c][i] += bt.side[c][i];
                }
                for i in 0..ex::COUNT {
                    sd.extra[c][i] += bt.extra[c][i];
                }
            }
            sd.heavy += bt.heavy;
            sd.active += 1;
            sd.mandatory += ((t, c) == present) as i32;
            for d in 0..2 {
                if c as usize != d {
                    sd.extra[d][ex::DEF_HOT] += bt.extra[d][ex::DEF_BOARD];
                }
            }
            sd.extra[c as usize][ex::TO_MOVE] += 1;
            if (t, c) == present {
                for d in 0..2 {
                    sd.extra[d][ex::ZONE_MAND] += bt.side[d][si::K_ZONE_ATT];
                }
            }
            for d in 0..2 {
                let z = bt.side[d][si::K_ZONE_ATT];
                if c as usize != d {
                    sd.extra[d][ex::ZONE_HOT] += z;
                }
                sd.extra[d][ex::ZONE_MAX] = sd.extra[d][ex::ZONE_MAX].max(z);
            }
        } else {
            for c in 0..2 {
                for k in 0..10 {
                    sd.side[c][si::MAT_I + k] += bt.side[c][si::MAT_A + k];
                }
            }
            sd.inactive += 1;
            if l > il_max {
                sd.extra[0][ex::TL_LOST] += 1;
            } else if l < il_min {
                sd.extra[1][ex::TL_LOST] += 1;
            }
        }
    }
    // Pieces on neighbouring timelines near each active royal.
    for l in amin..=amax {
        let (t, c) = s.get_timeline_end(l);
        let b = s.board(l, t, c);
        for col in 0..2usize {
            let mine = if col == 0 { b.white() & !b.black() } else { b.black() & !b.white() };
            for k in marked_pos(b.royal() & mine) {
                let r = ring2(k);
                for l2 in (l - 2).max(l_min)..=(l + 2).min(l_max) {
                    if l2 == l {
                        continue;
                    }
                    let (t2, c2) = s.get_timeline_end(l2);
                    let b2 = s.board(l2, t2, c2);
                    let enemy = if col == 0 { b2.black() & !b2.white() } else { b2.white() & !b2.black() };
                    sd.extra[1 - col][ex::KING_PROX_5D] += (r & enemy & !b2.lpawn() & !b2.royal()).count_ones() as i32;
                }
            }
        }
    }
    royal_5d(s, &mut sd, src, &atts);
    if super::sac::active() {
        sac_terms(s, &mut sd, src);
    }
    let (l0_min, l0_max) = s.get_initial_lines_range();
    let white_created = l_max - l0_max;
    let black_created = l0_min - l_min;
    sd.side[0][si::SLOTS] = (1 + black_created - white_created).clamp(0, 4);
    sd.side[1][si::SLOTS] = (1 + white_created - black_created).clamp(0, 4);
    sd
}

/// The f7 sac table's terms (`ex::SAC_LINE` ..): created timelines whose latest board is a
/// local mate, and the root's winning travels still on hand.
fn sac_terms(s: &State, sd: &mut Sides, src: &mut impl TermSource) {
    use super::sac;
    let m = s.multiverse();
    let (amin, amax) = s.get_active_range();
    let (l0, l1) = s.get_initial_lines_range();
    if (amin < l0 || amax > l1) && sac::any_proof() {
        for l in (amin..l0).chain(l1 + 1..=amax) {
            if let Some((attacker, plies)) = sac::line_win(m, l) {
                let e = &mut sd.extra[attacker as usize];
                e[ex::SAC_LINE] += 1;
                e[ex::SAC_CLOSE] += 12 - (plies as i32).min(11);
            }
        }
    }
    if let Some(rt) = src.sac_root() {
        if rt.n > 0 {
            let (now, open) = sac::root_terms(m, rt);
            for c in 0..2 {
                sd.extra[c][ex::SAC_NOW] += now[c];
                sd.extra[c][ex::SAC_OPEN] += open[c];
            }
        }
    }
}

// ------------------------------------------------------------------ 5D royal danger

/// Half-turn from which color `c`'s pieces on a line ending at `end` move next: the latest
/// board's when it is `c`'s turn there, else the one after (`Geo::at` stands in for it with
/// the latest board, as if the opponent passed).
#[inline(always)]
fn src_v(end: i32, c: bool) -> i32 {
    if (end & 1 != 0) == c { end } else { end + 1 }
}

struct Geo<'a> {
    m: &'a crate::core::multiverse::Multiverse,
    lo: i32,
    hi: i32,
}

impl<'a> Geo<'a> {
    #[inline(always)]
    fn end(&self, l: i32) -> i32 {
        self.m.timeline_end_v(l)
    }
    /// Board of line `l` at half-turn `v`; one past the end is the latest board.
    #[inline(always)]
    fn at(&self, l: i32, v: i32) -> Option<&'a Board> {
        if l < self.lo || l > self.hi {
            return None;
        }
        let tl = self.m.timeline(l);
        let k = v - tl.start;
        let n = tl.boards.len() as i32;
        if k < 0 || k > n {
            None
        } else if k == n {
            tl.boards.last()
        } else {
            Some(&tl.boards[k as usize])
        }
    }
}

#[inline(always)]
fn shift_xy(b: Bitboard, dx: i32, dy: i32) -> Bitboard {
    let b = match dx {
        1 => shift_east(b),
        -1 => shift_west(b),
        2 => shift_east(shift_east(b)),
        -2 => shift_west(shift_west(b)),
        _ => b,
    };
    match dy {
        1 => b << 8,
        -1 => b >> 8,
        2 => b << 16,
        -2 => b >> 16,
        _ => b,
    }
}

const XY9: [(i32, i32); 9] = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];

/// One square step as (mask, shift): the squares that may move, then the bit shift.
type Step = (Bitboard, i32);

const fn step_of(dx: i32, dy: i32) -> Step {
    let m = if dx > 0 { !H_FILE } else if dx < 0 { !A_FILE } else { !0 };
    (m, dx + 8 * dy)
}

/// `XY9` as steps, and their reverses.
const STEP9: [Step; 9] = {
    let mut a = [(0, 0); 9];
    let mut i = 0;
    while i < 9 {
        a[i] = step_of(XY9[i].0, XY9[i].1);
        i += 1;
    }
    a
};
const BACK9: [Step; 9] = {
    let mut a = [(0, 0); 9];
    let mut i = 0;
    while i < 9 {
        a[i] = step_of(-XY9[i].0, -XY9[i].1);
        i += 1;
    }
    a
};

#[inline(always)]
fn step(b: Bitboard, (m, s): Step) -> Bitboard {
    if s >= 0 { (b & m) << s } else { (b & m) >> -s }
}


/// Knight leaps (dx, dy, dt) into the past of the same timeline (dt < 0, turns).
const KN_PAST: [(i32, i32, i32); 8] =
    [(2, 0, -1), (-2, 0, -1), (0, 2, -1), (0, -2, -1), (1, 0, -2), (-1, 0, -2), (0, 1, -2), (0, -1, -2)];

/// Knight leaps that can land on another timeline's latest board: (dl, dt) and the square
/// steps that go with it.
const KN_NOW: [(i32, i32, &[(i32, i32)]); 8] = [
    (1, 0, &[(2, 0), (-2, 0), (0, 2), (0, -2)]),
    (-1, 0, &[(2, 0), (-2, 0), (0, 2), (0, -2)]),
    (2, 0, &[(1, 0), (-1, 0), (0, 1), (0, -1)]),
    (-2, 0, &[(1, 0), (-1, 0), (0, 1), (0, -1)]),
    (1, -2, &[(0, 0)]),
    (-1, -2, &[(0, 0)]),
    (2, -1, &[(0, 0)]),
    (-2, -1, &[(0, 0)]),
];

/// Squares at king distance at most one from `r`.
#[inline(always)]
fn spread1(r: Bitboard) -> Bitboard {
    let h = r | shift_east(r) | shift_west(r);
    h | shift_north(h) | shift_south(h)
}

/// Squares next to enemy royals on the past boards of line `l` where attacker `c`'s pieces
/// there can land: [those no other enemy piece defends, and those among them `c` also covers]. (Landings
/// on other timelines' past boards are left out: rarer, and they would tie the result to every
/// neighbouring line.)
fn past_line(g: &Geo, l: i32, c: bool) -> [i32; 2] {
    let mut past = [0i32; 2];
    let vs = src_v(g.end(l), c);
    let Some(b0) = g.at(l, vs) else { return past };
    let mine = b0.hostile(!c);
    // The line's past boards and the landing squares next to an enemy royal on them.
    let mut boards: [&Board; 8] = [b0; 8];
    let mut zone = [0u64; 8];
    let mut hits = [0u64; 8];
    let mut n = 0;
    while n < 8 {
        let Some(b) = g.at(l, vs - 2 * (n as i32 + 1)) else { break };
        boards[n] = b;
        let r = b.royal() & b.hostile(c);
        zone[n] = spread1(r) & !r & !b.friendly(c);
        n += 1;
    }
    let comp = [b0.lrook() & mine, b0.lbishop() & mine, b0.lunicorn() & mine];
    if comp.iter().any(|&x| x != 0) {
        for (j, &(sx, sy)) in XY9.iter().enumerate() {
            let mut pos = comp[(sx != 0) as usize + (sy != 0) as usize];
            let st = STEP9[j];
            for k in 0..n {
                pos = step(pos, st);
                if pos == 0 {
                    break;
                }
                hits[k] |= pos & zone[k];
                pos &= !boards[k].occupied();
            }
        }
    }
    let kn = b0.lknight() & mine;
    if kn != 0 {
        for &(dx, dy, dt) in &KN_PAST {
            let k = (-dt - 1) as usize;
            if k < n {
                hits[k] |= shift_xy(kn, dx, dy) & zone[k];
            }
        }
    }
    // Squares no other enemy piece defends, and those `c` also covers there.
    for k in 0..n {
        if hits[k] != 0 {
            let weak = hits[k] & !side_attacks(boards[k], !c).attacks_nr;
            if weak != 0 {
                past[0] += weak.count_ones() as i32;
                past[1] += (weak & side_attacks(boards[k], c).attacks).count_ones() as i32;
            }
        }
    }
    past
}

/// Identity of everything `past_line(g, l, c)` reads: the line's boards it can reach.
fn past_key(g: &Geo, l: i32, c: bool) -> u64 {
    let tl = g.m.timeline(l);
    let end = g.end(l);
    let vs = src_v(end, c);
    // Board keys already depend on the line and half-turn, so XOR combines them safely.
    let mut k = 0;
    for &x in &tl.keys()[((vs - 16 - tl.start).max(0)) as usize..] {
        k ^= x;
    }
    k ^= ((vs as u64) << 1 | c as u64).wrapping_mul(0xd6e8feb86659fd93);
    k ^ k >> 29
}

/// Lines whose latest boards' attack maps `sides_with` keeps for `royal_5d` (the terms of
/// further lines are looked up again).
const ATT_LINES: usize = 32;

/// 5D danger terms of each side's royals (`si::TRAVEL_PAST` ..= `si::INACTIVE_ROYAL_ATT`).
fn royal_5d(s: &State, sd: &mut Sides, src: &mut impl TermSource, atts: &[[Bitboard; 4]; ATT_LINES]) {
    let (lo, hi) = s.get_lines_range();
    let (amin, amax) = s.get_active_range();
    let (l0_min, l0_max) = s.get_initial_lines_range();
    let (w, h) = s.get_board_size();
    let g = Geo { m: s.multiverse(), lo, hi };
    for ci in 0..2 {
        let c = ci == 1;
        let d = 1 - ci;
        // Time travel into the past: walk every source line's pieces backwards.
        let mut past = [0i32; 2];
        for l in lo..=hi {
            let key = past_key(&g, l, c);
            let p = match src.past_get(key) {
                Some(p) => p,
                None => {
                    let p = past_line(&g, l, c);
                    src.past_put(key, p);
                    p
                }
            };
            past[0] += p[0];
            past[1] += p[1];
        }
        let f = &mut sd.side[d];
        f[si::TRAVEL_PAST] += past[0];
        f[si::TRAVEL_BACKED] += past[1];
        // Latest boards: lines to the enemy royals and landings next to them.
        let mut royals_active = false;
        let mut royals_any = false;
        for l2 in lo..=hi {
            let v2 = src_v(g.end(l2), c);
            let b2 = g.at(l2, v2).unwrap();
            let r = b2.royal() & b2.hostile(c);
            if r == 0 {
                continue;
            }
            royals_any = true;
            let (att_c, def_nr) = match atts.get((l2 - lo) as usize) {
                Some(a) => (a[ci], a[2 + d]),
                None => {
                    let t = src.terms(b2, w, h);
                    (t.att[ci], t.att_nr[d])
                }
            };
            if l2 < amin || l2 > amax {
                sd.side[d][si::INACTIVE_ROYAL_ATT] += (r & att_c).count_ones() as i32;
                continue;
            }
            royals_active = true;
            let zone = spread1(r) & !r & !b2.wall();
            let land = zone & !b2.friendly(c);
            let mut sp = 0u64;
            let mut now = [0i32; 2];
            let mut open = [0i32; 3];
            let mut count_landing = |t: Bitboard, sp: &mut Bitboard| {
                *sp |= t;
                let weak = t & !def_nr;
                now[0] += weak.count_ones() as i32;
                now[1] += (weak & att_c).count_ones() as i32;
            };
            for &(sl, st) in &[(-1i32, 0i32), (1, 0), (-1, -1), (1, -1)] {
                // The boards outward from the target along this (timeline, time) direction:
                // what blocks there and the attacker's movable pieces by axis count.
                // (Up to three timelines away; `near` collects attacker sliders within reach
                // of the target squares at all.)
                let mut occ = [0u64; 3];
                let mut block = [0u64; 3];
                let mut att = [[0u64; 4]; 3];
                let mut n = 0;
                let mut reg = land | r;
                let mut near = 0;
                while n < 3 {
                    let i = n as i32 + 1;
                    let (l1, v1) = (l2 - i * sl, v2 - 2 * i * st);
                    let Some(b1) = g.at(l1, v1) else { break };
                    occ[n] = b1.occupied();
                    block[n] = b1.friendly(!c);
                    reg = spread1(reg);
                    if v1 == src_v(g.end(l1), c) {
                        let mine = b1.hostile(!c);
                        att[n] = [b1.lrook() & mine, b1.lbishop() & mine, b1.lunicorn() & mine, b1.ldragon() & mine];
                        near |= (att[n][0] | att[n][1] | att[n][2] | att[n][3]) & reg;
                    }
                    n += 1;
                }
                if near == 0 {
                    continue;
                }
                let mut any = [0u64; 4];
                for a in &att[..n] {
                    for j in 0..4 {
                        any[j] |= a[j];
                    }
                }
                for (j, &(sx, sy)) in XY9.iter().enumerate() {
                    let axes = (sl != 0) as usize + (st != 0) as usize + (sx != 0) as usize + (sy != 0) as usize;
                    if any[axes - 1] == 0 {
                        continue;
                    }
                    let (fwd, back) = (STEP9[j], BACK9[j]);
                    // Walk outward from the target towards where an attacker would stand.
                    let mut cand = land;
                    let mut xr = r;
                    for k in 0..n {
                        cand = step(cand, back);
                        xr = step(xr, back);
                        if cand | xr == 0 {
                            break;
                        }
                        let a = att[k][axes - 1];
                        if a != 0 {
                            let hc = cand & a;
                            if hc != 0 {
                                let mut t = hc;
                                for _ in 0..=k {
                                    t = step(t, fwd);
                                }
                                count_landing(t, &mut sp);
                            }
                            let hx = xr & a;
                            if hx != 0 {
                                open[axes.min(3) - 1] += hx.count_ones() as i32;
                                xr &= !hx;
                            }
                        }
                        cand &= !occ[k];
                        xr &= !block[k];
                    }
                }
            }
            for &(dl, dt, leaps) in &KN_NOW {
                let (l1, v1) = (l2 - dl, v2 - 2 * dt);
                let Some(b1) = g.at(l1, v1) else { continue };
                if v1 != src_v(g.end(l1), c) {
                    continue;
                }
                let kn = b1.lknight() & b1.hostile(!c);
                if kn == 0 {
                    continue;
                }
                for &(dx, dy) in leaps {
                    let t = shift_xy(kn, dx, dy) & land;
                    if t != 0 {
                        count_landing(t, &mut sp);
                    }
                }
            }
            let f = &mut sd.side[d];
            f[si::TRAVEL_NOW] += now[0];
            if src_v(g.end(l2), c) == g.end(l2) {
                sd.extra[d][ex::TRAVEL_HOT] += now[0];
            }
            f[si::TRAVEL_BACKED] += now[1];
            f[si::OPEN_ORTH5] += open[0];
            f[si::OPEN_DIAG5] += open[1];
            f[si::OPEN_MULTI5] += open[2];
            let hit = att_c | sp;
            let weak = (zone & !def_nr & hit).count_ones() as i32;
            f[si::WEAK_SQ] += weak;
            let e = &mut sd.extra[d];
            if src_v(g.end(l2), c) == g.end(l2) {
                e[ex::WEAK_HOT] += weak;
            }
            e[ex::DMAX5] = e[ex::DMAX5].max(weak + now[0]);
            let home = if d == 0 { 0 } else { h - 1 };
            let created = l2 < l0_min || l2 > l0_max;
            for k in marked_pos(r) {
                if k >> 3 != home {
                    f[si::KING_OFF] += 1;
                    f[si::KING_OFF_NEW] += created as i32;
                }
                let near = king_attack(k);
                let flight = near & !b2.friendly(d == 1) & !hit;
                f[si::LOW_FLIGHT] += (flight.count_ones() <= 1 && (near | pmask(k)) & hit != 0) as i32;
                sd.extra[d][ex::K_FLIGHT] += flight.count_ones().min(3) as i32;
            }
        }
        sd.side[d][si::EXILED] = (royals_any && !royals_active) as i32;
    }
}

pub fn features(s: &State) -> Features {
    features_with(s, true)
}

/// Features; the 5D check scan (`CHECKS_5D`, `SP_CHECKS`) only when `checks`.
pub fn features_with(s: &State, checks: bool) -> Features {
    features_from(&sides(s), s, checks)
}

pub fn features_from(sd: &Sides, s: &State, checks: bool) -> Features {
    let mut f = [0i32; NF];
    let d = |i: usize| sd.side[0][i] - sd.side[1][i];
    for k in 0..10 {
        f[fi::MAT_A + k] = d(si::MAT_A + k);
        f[fi::MAT_I + k] = d(si::MAT_I + k);
    }
    for g in 0..5 {
        f[fi::CENT + g] = d(si::CENT + g);
        f[fi::MOB_N + g] = d(si::MOB + g);
    }
    f[fi::PAWN_ADV] = d(si::PAWN_ADV);
    f[fi::PAWN_NEAR] = d(si::PAWN_NEAR);
    f[fi::KING_CENT] = d(si::KING_CENT);
    f[fi::K_HOLES] = d(si::K_HOLES);
    f[fi::K_KNIGHTS] = d(si::K_KNIGHTS);
    f[fi::K_OPEN_ORTH] = d(si::K_OPEN_ORTH);
    f[fi::K_OPEN_DIAG] = d(si::K_OPEN_DIAG);
    f[fi::K_SHIELD] = d(si::K_SHIELD);
    f[fi::K_ZONE_ATT] = d(si::K_ZONE_ATT);
    f[fi::HANGING] = d(si::HANGING);
    f[fi::ROYAL_ATT] = d(si::ROYAL_ATT);
    let slots = d(si::SLOTS);
    let full = 6 * sd.active.max(1);
    f[fi::TL_SLOTS] = slots;
    f[fi::TL_SLOTS_HEAVY] = slots * 8 * sd.heavy.min(full) / full;
    f[fi::TEMPO] = if s.get_present().1 { -1 } else { 1 };
    f[fi::ACTIVE_LINES] = sd.active;
    let stm = s.get_present().1;
    let sign = if stm { -1 } else { 1 };
    if checks {
        let checks = CheckPosition::for_phantom(s).checks(!stm, true);
        f[fi::CHECKS_5D] = sign * checks.len().min(8) as i32;
        f[fi::SP_CHECKS] = sign * checks.iter().filter(|m| m.from.tl() != m.to.tl()).count().min(8) as i32;
    }
    f[fi::MANDATORY] = sign * sd.mandatory;
    let e = |i: usize| sd.extra[0][i] - sd.extra[1][i];
    f[fi::PASSED] = e(ex::PASSED);
    f[fi::DOUBLED] = e(ex::DOUBLED);
    f[fi::ISOLATED] = e(ex::ISOLATED);
    f[fi::KING_PROX] = e(ex::KING_PROX);
    f[fi::UNDEVELOPED] = e(ex::UNDEVELOPED);
    f[fi::KING_PROX_5D] = e(ex::KING_PROX_5D);
    f[fi::QUEEN_PROX] = e(ex::QUEEN_PROX);
    for k in 0..=ex::KN_CHECKS - ex::F7_WEAK {
        f[fi::F7 + k] = e(ex::F7_WEAK + k);
    }
    let capped = |i: usize, cap: i32| sd.extra[0][i].min(cap) - sd.extra[1][i].min(cap);
    f[fi::SAC_LINE] = capped(ex::SAC_LINE, 2);
    f[fi::SAC_CLOSE] = capped(ex::SAC_CLOSE, 22);
    f[fi::SAC_NOW] = capped(ex::SAC_NOW, 1);
    f[fi::SAC_NOW_STM] = sign * sd.extra[stm as usize][ex::SAC_NOW].min(1);
    f[fi::SAC_OPEN] = capped(ex::SAC_OPEN, 8);
    for k in 0..=ex::K_FLIGHT - ex::ZONE_HOT {
        f[fi::KS + k] = e(ex::ZONE_HOT + k);
    }
    f[fi::TL] = e(ex::TL_LOST);
    f[fi::TL + 1] = e(ex::TO_MOVE);
    f[fi::TL + 2] = sign * sd.extra[stm as usize][ex::ZONE_MAND];
    f[fi::TL + 3] = sign * sd.extra[1 - stm as usize][ex::ZONE_MAND];
    f[fi::DEF] = e(ex::DEF_HOT);
    f[fi::DEF + 1] = e(ex::DEF_BOARD);
    f[fi::DEF + 2] = e(ex::TRAVEL_HOT);
    for k in 0..=si::INACTIVE_ROYAL_ATT - si::TRAVEL_PAST {
        f[fi::DANGER_5D + k] = d(si::TRAVEL_PAST + k);
    }
    let opp = sd.side[1 - stm as usize];
    f[fi::TRAVEL_STM] = sign * (opp[si::TRAVEL_PAST] + opp[si::TRAVEL_NOW]);
    let sl = [sd.side[0][si::SLOTS], sd.side[1][si::SLOTS]];
    let dg = [royal_danger(&sd.side[0]), royal_danger(&sd.side[1])];
    f[fi::TL_SLOTS_THREAT] = sl[0] * dg[1] - sl[1] * dg[0];
    f[fi::TL_SLOTS_STM] = sign * sl[stm as usize];
    f
}

const DANGER_CAP: i32 = 6;

/// How much a side's royals are under attack (0..DANGER_CAP): attacked, travel landings next
/// to them, weak squares, few flight squares, off the home rank.
#[inline]
fn royal_danger(f: &[i32; si::COUNT]) -> i32 {
    (f[si::ROYAL_ATT] + f[si::TRAVEL_PAST] + f[si::TRAVEL_NOW] + f[si::WEAK_SQ] + f[si::LOW_FLIGHT] + f[si::KING_OFF]).min(DANGER_CAP)
}

/// Move the f7 sac table's features (`fi::SAC_LINE` ..) out of `f` into the returned vector:
/// the search adds their score after the eval net's correction, which never saw them.
pub fn split_sac(f: &mut Features) -> Features {
    let mut s = [0i32; NF];
    for i in fi::SAC_LINE..=fi::SAC_OPEN {
        s[i] = std::mem::take(&mut f[i]);
    }
    s
}

/// Weights from (feature name, value) pairs; unnamed features get zero.
pub fn weights_by_name(pairs: &[(&str, f32)]) -> Weights {
    let mut w = [0f32; NF];
    for (n, v) in pairs {
        if let Some(i) = FEATURE_NAMES.iter().position(|x| x == n) {
            w[i] = *v;
        }
    }
    w
}

/// Evaluation in centipawns from the side to move's perspective.
pub fn evaluate(s: &State, w: &Weights) -> i32 {
    let f = features_with(s, needs_checks(w));
    score(&f, w, s.get_present().1)
}

/// Whether the weights use the 5D check scan.
#[inline]
pub fn needs_checks(w: &Weights) -> bool {
    w[fi::CHECKS_5D] != 0.0 || w[fi::SP_CHECKS] != 0.0
}

/// Weighted sum of features, from the side to move's view.
#[inline]
pub fn score(f: &Features, w: &Weights, black_to_move: bool) -> i32 {
    let mut acc = 0f32;
    for i in 0..NF {
        acc += w[i] * f[i] as f32;
    }
    let white = acc.round() as i32;
    if black_to_move { -white } else { white }
}

/// White-perspective linear score of a feature vector (used by the tuner).
pub fn linear(f: &Features, w: &Weights) -> f32 {
    let mut acc = 0f32;
    for i in 0..NF {
        acc += w[i] * f[i] as f32;
    }
    acc
}
