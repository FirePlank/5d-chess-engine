//! Feature-based handcrafted evaluation: every term is an integer feature (white minus black
//! unless stated) and the evaluation is their weighted sum, so weights can be tuned on games.

use crate::core::bitboard::*;
use crate::core::board::Board;
use crate::core::check_position::CheckPosition;
use crate::core::state::State;

pub const NF: usize = 48;

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
}

pub const FEATURE_NAMES: [&str; NF] = [
    "mat_a_pawn", "mat_a_brawn", "mat_a_knight", "mat_a_bishop", "mat_a_rook", "mat_a_queen", "mat_a_princess",
    "mat_a_unicorn", "mat_a_dragon", "mat_a_cking", "mat_i_pawn", "mat_i_brawn", "mat_i_knight", "mat_i_bishop",
    "mat_i_rook", "mat_i_queen", "mat_i_princess", "mat_i_unicorn", "mat_i_dragon", "mat_i_cking", "cent_knight",
    "cent_bishop", "cent_rook", "cent_queen", "cent_other", "pawn_adv", "pawn_near", "king_cent", "tl_slots",
    "tl_slots_heavy", "k_holes", "k_knights", "k_open_orth", "k_open_diag", "k_shield", "k_zone_att", "mob_knight",
    "mob_bishop", "mob_rook", "mob_queen", "mob_king", "hanging", "royal_att", "tempo", "active_lines",
    "checks_5d", "sp_checks", "mandatory",
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
        ("mat_a_pawn", 146.5),
        ("mat_a_brawn", 150.0),
        ("mat_a_knight", -273.7),
        ("mat_a_bishop", 436.4),
        ("mat_a_rook", 472.8),
        ("mat_a_queen", 1250.0),
        ("mat_a_princess", 900.0),
        ("mat_a_unicorn", 250.0),
        ("mat_a_dragon", 150.0),
        ("mat_a_cking", 300.0),
        ("mat_i_pawn", 55.7),
        ("mat_i_brawn", 75.0),
        ("mat_i_knight", 336.4),
        ("mat_i_bishop", 814.5),
        ("mat_i_rook", 399.6),
        ("mat_i_queen", 1118.4),
        ("mat_i_princess", 450.0),
        ("mat_i_unicorn", 125.0),
        ("mat_i_dragon", 75.0),
        ("mat_i_cking", 150.0),
        ("cent_knight", 150.4),
        ("cent_bishop", 136.0),
        ("cent_rook", -51.8),
        ("cent_queen", -58.8),
        ("cent_other", 2.0),
        ("pawn_adv", 30.3),
        ("pawn_near", -329.8),
        ("king_cent", -253.1),
        ("tl_slots", 2692.1),
        ("tl_slots_heavy", 5.3),
        ("k_holes", 777.2),
        ("k_knights", 1528.1),
        ("k_open_orth", -326.8),
        ("k_open_diag", -1.3),
        ("k_shield", 865.4),
        ("k_zone_att", -430.7),
        ("mob_knight", 39.9),
        ("mob_bishop", 52.6),
        ("mob_rook", -6.1),
        ("mob_queen", 202.6),
        ("mob_king", -963.6),
        ("hanging", -26.8),
        ("royal_att", -2605.3),
        ("tempo", 407.0),
        ("active_lines", -25.4),
        ("checks_5d", 0.0),
        ("sp_checks", 0.0),
        ("mandatory", 439.9),
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

fn board_features(b: &Board, f: &mut Features, active: bool, w: i32, h: i32) {
    let white = b.white() & !b.black();
    let black = b.black() & !b.white();
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
    let base = if active { fi::MAT_A } else { fi::MAT_I };
    for k in 0..10 {
        f[base + k] += (kinds[k] & white).count_ones() as i32 - (kinds[k] & black).count_ones() as i32;
    }
    if !active {
        return;
    }
    let groups = [b.knight(), b.bishop(), b.rook(), b.queen() | b.princess(), b.unicorn() | b.dragon() | b.common_king()];
    for (g, bb) in groups.iter().enumerate() {
        for sq in marked_pos(bb & white) {
            f[fi::CENT + g] += centrality(sq, w, h);
        }
        for sq in marked_pos(bb & black) {
            f[fi::CENT + g] -= centrality(sq, w, h);
        }
    }
    for sq in marked_pos(b.lpawn() & white) {
        let adv = sq >> 3;
        f[fi::PAWN_ADV] += (adv - 1).max(0);
        f[fi::PAWN_NEAR] += (adv >= h - 3) as i32;
    }
    for sq in marked_pos(b.lpawn() & black) {
        let adv = h - 1 - (sq >> 3);
        f[fi::PAWN_ADV] -= (adv - 1).max(0);
        f[fi::PAWN_NEAR] -= (adv >= h - 3) as i32;
    }
    let sw = side_attacks(b, false);
    let sb = side_attacks(b, true);
    for k in 0..5 {
        f[fi::MOB_N + k] += sw.mob[k] - sb.mob[k];
    }
    for (c, sign, me, them, my_att, their_att) in
        [(false, 1, white, black, sw.attacks, sb.attacks), (true, -1, black, white, sb.attacks, sw.attacks)]
    {
        let own = b.friendly(c);
        for k in marked_pos(b.royal() & me) {
            f[fi::KING_CENT] += sign * centrality(k, w, h);
            let zone = king_attack(k);
            let danger_holes = (zone & !b.occupied()).count_ones() as i32;
            let danger_knights = (knight_attack(k) & b.knight() & them).count_ones() as i32;
            let open_o = open_rays(k, own, &ORTH, w, h);
            let open_d = open_rays(k, own, &DIAG, w, h);
            let fwd = if c { shift_south(pmask(k)) } else { shift_north(pmask(k)) };
            let shield_sq = fwd | shift_west(fwd) | shift_east(fwd);
            let shield = (shield_sq & b.lpawn() & me).count_ones() as i32;
            let zone_att = (zone & their_att).count_ones() as i32;
            f[fi::K_HOLES] += sign * danger_holes;
            f[fi::K_KNIGHTS] += sign * danger_knights;
            f[fi::K_OPEN_ORTH] += sign * open_o;
            f[fi::K_OPEN_DIAG] += sign * open_d;
            f[fi::K_SHIELD] += sign * shield;
            f[fi::K_ZONE_ATT] += sign * zone_att;
            if their_att & pmask(k) != 0 {
                f[fi::ROYAL_ATT] += sign;
            }
        }
        for sq in marked_pos(me & !b.royal() & their_att & !my_att) {
            f[fi::HANGING] += sign * kind_value10(b, sq);
        }
    }
}

pub fn features(s: &State) -> Features {
    features_with(s, true)
}

/// Features; the 5D check scan (`CHECKS_5D`, `SP_CHECKS`) only when `checks`.
pub fn features_with(s: &State, checks: bool) -> Features {
    let mut f = [0i32; NF];
    let (l_min, l_max) = s.get_lines_range();
    let (amin, amax) = s.get_active_range();
    let (w, h) = s.get_board_size();
    let (mut heavy, mut active_boards) = (0i32, 0i32);
    for l in l_min..=l_max {
        let (t, c) = s.get_timeline_end(l);
        let b = s.board(l, t, c);
        let active = l >= amin && l <= amax;
        board_features(b, &mut f, active, w, h);
        if active {
            heavy += (b.queen() | b.bishop() | b.princess()).count_ones() as i32;
            active_boards += 1;
        }
    }
    let (l0_min, l0_max) = s.get_initial_lines_range();
    let white_created = l_max - l0_max;
    let black_created = l0_min - l_min;
    let avail_w = (1 + black_created - white_created).clamp(0, 4);
    let avail_b = (1 + white_created - black_created).clamp(0, 4);
    let slots = avail_w - avail_b;
    let full = 6 * active_boards.max(1);
    f[fi::TL_SLOTS] = slots;
    f[fi::TL_SLOTS_HEAVY] = slots * 8 * heavy.min(full) / full;
    f[fi::TEMPO] = if s.get_present().1 { -1 } else { 1 };
    f[fi::ACTIVE_LINES] = active_boards;
    let stm = s.get_present().1;
    let sign = if stm { -1 } else { 1 };
    if checks {
        let checks = CheckPosition::for_phantom(s).checks(!stm, true);
        f[fi::CHECKS_5D] = sign * checks.len().min(8) as i32;
        f[fi::SP_CHECKS] = sign * checks.iter().filter(|m| m.from.tl() != m.to.tl()).count().min(8) as i32;
    }
    let present = s.get_present();
    f[fi::MANDATORY] = sign * (amin..=amax).filter(|&l| s.get_timeline_end(l) == present).count() as i32;
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
    let f = features_with(s, w[fi::CHECKS_5D] != 0.0 || w[fi::SP_CHECKS] != 0.0);
    let mut acc = 0f32;
    for i in 0..NF {
        acc += w[i] * f[i] as f32;
    }
    let white = acc.round() as i32;
    if s.get_present().1 { -white } else { white }
}

/// White-perspective linear score of a feature vector (used by the tuner).
pub fn linear(f: &Features, w: &Weights) -> f32 {
    let mut acc = 0f32;
    for i in 0..NF {
        acc += w[i] * f[i] as f32;
    }
    acc
}
