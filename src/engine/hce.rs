//! Feature-based handcrafted evaluation: every term is an integer feature (white minus black
//! unless stated) and the evaluation is their weighted sum, so weights can be tuned on games.

use crate::core::bitboard::*;
use crate::core::board::Board;
use crate::core::check_position::CheckPosition;
use crate::core::state::State;

pub const NF: usize = 53;

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
}

pub const FEATURE_NAMES: [&str; NF] = [
    "mat_a_pawn", "mat_a_brawn", "mat_a_knight", "mat_a_bishop", "mat_a_rook", "mat_a_queen", "mat_a_princess",
    "mat_a_unicorn", "mat_a_dragon", "mat_a_cking", "mat_i_pawn", "mat_i_brawn", "mat_i_knight", "mat_i_bishop",
    "mat_i_rook", "mat_i_queen", "mat_i_princess", "mat_i_unicorn", "mat_i_dragon", "mat_i_cking", "cent_knight",
    "cent_bishop", "cent_rook", "cent_queen", "cent_other", "pawn_adv", "pawn_near", "king_cent", "tl_slots",
    "tl_slots_heavy", "k_holes", "k_knights", "k_open_orth", "k_open_diag", "k_shield", "k_zone_att", "mob_knight",
    "mob_bishop", "mob_rook", "mob_queen", "mob_king", "hanging", "royal_att", "tempo", "active_lines",
    "checks_5d", "sp_checks", "mandatory", "passed", "doubled", "isolated", "king_prox", "undeveloped",
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
        ("mat_a_pawn", 180.1),
        ("mat_a_brawn", 150.0),
        ("mat_a_knight", 79.2),
        ("mat_a_bishop", 974.4),
        ("mat_a_rook", 630.4),
        ("mat_a_queen", 1250.0),
        ("mat_a_princess", 900.0),
        ("mat_a_unicorn", 250.0),
        ("mat_a_dragon", 150.0),
        ("mat_a_cking", 300.0),
        ("mat_i_pawn", 75.1),
        ("mat_i_brawn", 75.0),
        ("mat_i_knight", 339.0),
        ("mat_i_bishop", 870.7),
        ("mat_i_rook", 393.3),
        ("mat_i_queen", 1173.5),
        ("mat_i_princess", 450.0),
        ("mat_i_unicorn", 125.0),
        ("mat_i_dragon", 75.0),
        ("mat_i_cking", 150.0),
        ("cent_knight", 119.5),
        ("cent_bishop", 59.3),
        ("cent_rook", -64.3),
        ("cent_queen", -54.8),
        ("cent_other", 2.0),
        ("pawn_adv", 43.4),
        ("pawn_near", -198.7),
        ("king_cent", -258.9),
        ("tl_slots", 2901.0),
        ("tl_slots_heavy", -14.5),
        ("k_holes", -44.8),
        ("k_knights", 2334.4),
        ("k_open_orth", -353.5),
        ("k_open_diag", -1.4),
        ("k_shield", 883.4),
        ("k_zone_att", -426.3),
        ("mob_knight", 20.4),
        ("mob_bishop", 57.9),
        ("mob_rook", -11.3),
        ("mob_queen", 224.0),
        ("mob_king", -158.9),
        ("hanging", -30.3),
        ("royal_att", -2133.4),
        ("tempo", 430.8),
        ("active_lines", -32.1),
        ("checks_5d", 0.0),
        ("sp_checks", 0.0),
        ("mandatory", 533.6),
        ("passed", -148.4),
        ("doubled", -49.3),
        ("isolated", -60.2),
        ("king_prox", 1151.3),
        ("undeveloped", -345.3),
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
    pub(crate) mob: [i32; 5],
}

pub(crate) fn side_attacks(b: &Board, c: bool) -> Side {
    let own = b.friendly(c);
    let mine = b.hostile(!c);
    let occ = b.occupied();
    let mut s = Side { attacks: 0, mob: [0; 5] };
    for sq in marked_pos(mine) {
        let z = pmask(sq);
        if b.lpawn() & z != 0 {
            s.attacks |= if c { black_pawn_attack(sq) } else { white_pawn_attack(sq) };
        } else if b.lknight() & z != 0 {
            let a = knight_attack(sq);
            s.attacks |= a;
            s.mob[0] += (a & !own).count_ones() as i32;
        } else if b.lking() & z != 0 {
            let a = king_attack(sq);
            s.attacks |= a;
            s.mob[4] += (a & !own).count_ones() as i32;
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
            s.attacks |= a;
            let m = (a & !own).count_ones() as i32;
            match (r, bi) {
                (true, true) => s.mob[3] += m,
                (true, false) => s.mob[2] += m,
                (false, true) => s.mob[1] += m,
                _ => {}
            }
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
    pub const COUNT: usize = 42;
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
    pub const COUNT: usize = 5;
}

/// Squares within king distance 2.
#[inline]
fn ring2(sq: i32) -> Bitboard {
    let r1 = king_attack(sq) | pmask(sq);
    let r = r1 | shift_north(r1) | shift_south(r1);
    r | shift_east(r) | shift_west(r)
}

fn board_extra(b: &Board, extra: &mut [[i32; ex::COUNT]; 2], h: i32) {
    let colors = [b.white() & !b.black(), b.black() & !b.white()];
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
            f[ex::KING_PROX] += (ring2(k) & pieces).count_ones() as i32;
        }
        let back = if c == 0 { 0xff } else { 0xffu64 << (8 * (h - 1)) };
        f[ex::UNDEVELOPED] += ((b.knight() | b.bishop()) & me & back).count_ones() as i32;
    }
}

fn board_sides(b: &Board, side: &mut [[i32; si::COUNT]; 2], active: bool, w: i32, h: i32) {
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
    let base = if active { si::MAT_A } else { si::MAT_I };
    for c in 0..2 {
        for k in 0..10 {
            side[c][base + k] += (kinds[k] & colors[c]).count_ones() as i32;
        }
    }
    if !active {
        return;
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
}

pub fn sides(s: &State) -> Sides {
    let mut sd = Sides { side: [[0; si::COUNT]; 2], heavy: 0, active: 0, inactive: 0, mandatory: 0, extra: [[0; ex::COUNT]; 2] };
    let (l_min, l_max) = s.get_lines_range();
    let (amin, amax) = s.get_active_range();
    let (w, h) = s.get_board_size();
    let present = s.get_present();
    for l in l_min..=l_max {
        let (t, c) = s.get_timeline_end(l);
        let b = s.board(l, t, c);
        let active = l >= amin && l <= amax;
        board_sides(b, &mut sd.side, active, w, h);
        if active {
            board_extra(b, &mut sd.extra, h);
            sd.heavy += (b.queen() | b.bishop() | b.princess()).count_ones() as i32;
            sd.active += 1;
            sd.mandatory += ((t, c) == present) as i32;
        } else {
            sd.inactive += 1;
        }
    }
    let (l0_min, l0_max) = s.get_initial_lines_range();
    let white_created = l_max - l0_max;
    let black_created = l0_min - l_min;
    sd.side[0][si::SLOTS] = (1 + black_created - white_created).clamp(0, 4);
    sd.side[1][si::SLOTS] = (1 + white_created - black_created).clamp(0, 4);
    sd
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
    f
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
