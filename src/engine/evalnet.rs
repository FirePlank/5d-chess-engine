//! Eval net: a small MLP over per-side handcrafted features that adds a capped correction
//! (residual) to the handcrafted evaluation. Inputs are from the side to move's view: its
//! own features, the opponent's, then a few global terms (including the handcrafted score).
//!
//! Weights live in `data/evalnet.bin` (written by `scripts/evalnet/train.py`); an empty or
//! mismatching file leaves the net disabled.

use super::hce::{centrality, kind_value10, open_rays, side_attacks, DIAG, ORTH};
use crate::core::bitboard::*;
use crate::core::board::Board;
use crate::core::state::State;
use std::sync::OnceLock;

/// Features per side.
pub const NS: usize = 42;
/// Global features.
pub const NG: usize = 5;
pub const NI: usize = 2 * NS + NG;

/// Per-side feature indices.
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
}

/// Global feature indices (after both sides' blocks).
pub mod gi {
    pub const ACTIVE: usize = 0;
    pub const INACTIVE: usize = 1;
    /// Queens/bishops/princesses left, as eighths of the full complement.
    pub const HEAVY: usize = 2;
    /// Handcrafted score (side to move), pawns.
    pub const BASE: usize = 3;
    /// Boards the side to move must play on.
    pub const MANDATORY: usize = 4;
}

fn board_inputs(b: &Board, side: &mut [[f32; NS]; 2], active: bool, w: i32, h: i32) {
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
            side[c][base + k] += (kinds[k] & colors[c]).count_ones() as f32;
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
                f[si::CENT + g] += centrality(sq, w, h) as f32;
            }
        }
        for sq in marked_pos(b.lpawn() & me) {
            let rank = sq >> 3;
            let adv = if c == 1 { h - 1 - rank } else { rank };
            f[si::PAWN_ADV] += (adv - 1).max(0) as f32;
            f[si::PAWN_NEAR] += (adv >= h - 3) as i32 as f32;
        }
        for k in 0..5 {
            f[si::MOB + k] += att[c].mob[k] as f32;
        }
        let (my_att, their_att) = (att[c].attacks, att[1 - c].attacks);
        let own = b.friendly(c == 1);
        for k in marked_pos(b.royal() & me) {
            f[si::KING_CENT] += centrality(k, w, h) as f32;
            let zone = king_attack(k);
            f[si::K_HOLES] += (zone & !b.occupied()).count_ones() as f32;
            f[si::K_KNIGHTS] += (knight_attack(k) & b.knight() & them).count_ones() as f32;
            f[si::K_OPEN_ORTH] += open_rays(k, own, &ORTH, w, h) as f32;
            f[si::K_OPEN_DIAG] += open_rays(k, own, &DIAG, w, h) as f32;
            let fwd = if c == 1 { shift_south(pmask(k)) } else { shift_north(pmask(k)) };
            f[si::K_SHIELD] += ((fwd | shift_west(fwd) | shift_east(fwd)) & b.lpawn() & me).count_ones() as f32;
            f[si::K_ZONE_ATT] += (zone & their_att).count_ones() as f32;
            if their_att & pmask(k) != 0 {
                f[si::ROYAL_ATT] += 1.0;
            }
        }
        for sq in marked_pos(me & !b.royal() & their_att & !my_att) {
            f[si::HANGING] += kind_value10(b, sq) as f32;
        }
    }
}

/// Net inputs for `s`, given the handcrafted score from the side to move's view.
pub fn inputs(s: &State, base_stm: i32) -> [f32; NI] {
    let mut side = [[0f32; NS]; 2];
    let (l_min, l_max) = s.get_lines_range();
    let (amin, amax) = s.get_active_range();
    let (w, h) = s.get_board_size();
    let present = s.get_present();
    let (mut heavy, mut active, mut inactive, mut mandatory) = (0i32, 0i32, 0i32, 0i32);
    for l in l_min..=l_max {
        let (t, c) = s.get_timeline_end(l);
        let b = s.board(l, t, c);
        let is_active = l >= amin && l <= amax;
        board_inputs(b, &mut side, is_active, w, h);
        if is_active {
            heavy += (b.queen() | b.bishop() | b.princess()).count_ones() as i32;
            active += 1;
            mandatory += ((t, c) == present) as i32;
        } else {
            inactive += 1;
        }
    }
    let (l0_min, l0_max) = s.get_initial_lines_range();
    let white_created = l_max - l0_max;
    let black_created = l0_min - l_min;
    side[0][si::SLOTS] = (1 + black_created - white_created).clamp(0, 4) as f32;
    side[1][si::SLOTS] = (1 + white_created - black_created).clamp(0, 4) as f32;
    let full = 6 * active.max(1);
    let stm = s.get_present().1 as usize;
    let mut x = [0f32; NI];
    x[..NS].copy_from_slice(&side[stm]);
    x[NS..2 * NS].copy_from_slice(&side[1 - stm]);
    let g = &mut x[2 * NS..];
    g[gi::ACTIVE] = active as f32;
    g[gi::INACTIVE] = inactive as f32;
    g[gi::HEAVY] = (8 * heavy.min(full) / full) as f32;
    g[gi::BASE] = (base_stm as f32 / 100.0).clamp(-30.0, 30.0);
    g[gi::MANDATORY] = mandatory as f32;
    x
}

/// Float MLP NI -> H1 -> H2 -> 1 with clipped ReLU; input normalization is folded into the
/// first layer by the exporter.
pub struct Net {
    h1: usize,
    h2: usize,
    w1: Vec<f32>,
    b1: Vec<f32>,
    w2: Vec<f32>,
    b2: Vec<f32>,
    w3: Vec<f32>,
    b3: f32,
    /// Output units to centipawns, and the residual cap.
    scale: f32,
    cap: f32,
}

impl Net {
    /// Format: b"KNET0001", u32 ni, h1, h2, f32 scale, cap, then w1 [h1][ni], b1, w2 [h2][h1],
    /// b2, w3 [h2], b3 (little endian).
    pub fn from_bytes(bytes: &[u8]) -> Option<Net> {
        if bytes.len() < 28 || &bytes[..8] != b"KNET0001" {
            return None;
        }
        let u = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        let (ni, h1, h2) = (u(8), u(12), u(16));
        if ni != NI {
            return None;
        }
        let mut pos = 20;
        let mut take = |n: usize| -> Option<Vec<f32>> {
            let end = pos + 4 * n;
            let v = bytes.get(pos..end)?.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
            pos = end;
            Some(v)
        };
        let sc = take(2)?;
        let net = Net {
            h1,
            h2,
            w1: take(h1 * ni)?,
            b1: take(h1)?,
            w2: take(h2 * h1)?,
            b2: take(h2)?,
            w3: take(h2)?,
            b3: take(1)?[0],
            scale: sc[0],
            cap: sc[1],
        };
        Some(net)
    }

    /// Residual in centipawns (side to move).
    pub fn residual(&self, x: &[f32; NI]) -> i32 {
        let mut a1 = vec![0f32; self.h1];
        for (j, a) in a1.iter_mut().enumerate() {
            let row = &self.w1[j * NI..(j + 1) * NI];
            let mut acc = self.b1[j];
            for i in 0..NI {
                acc += row[i] * x[i];
            }
            *a = acc.clamp(0.0, 1.0);
        }
        let mut out = self.b3;
        for k in 0..self.h2 {
            let row = &self.w2[k * self.h1..(k + 1) * self.h1];
            let mut acc = self.b2[k];
            for j in 0..self.h1 {
                acc += row[j] * a1[j];
            }
            out += self.w3[k] * acc.clamp(0.0, 1.0);
        }
        (out * self.scale).clamp(-self.cap, self.cap) as i32
    }
}

static NET: OnceLock<Option<Net>> = OnceLock::new();

/// The embedded net, if `data/evalnet.bin` holds one for the current inputs.
pub fn embedded() -> Option<&'static Net> {
    NET.get_or_init(|| Net::from_bytes(include_bytes!("../../data/evalnet.bin"))).as_ref()
}
