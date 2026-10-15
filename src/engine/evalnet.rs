//! Eval net: a small MLP over per-side handcrafted features that adds a capped correction
//! (residual) to the handcrafted evaluation. Inputs are from the side to move's view: its
//! own features, the opponent's, then a few global terms (including the handcrafted score).
//!
//! Weights live in `data/evalnet.bin`, written by `scripts/evalnet/train.py` from
//! `ktools netdata` records.

use crate::core::state::State;
use std::sync::OnceLock;

pub use super::hce::si;
use super::hce::Sides;

/// Features per side.
pub const NS: usize = si::COUNT;
/// Global features.
pub const NG: usize = 5;
pub const NI: usize = 2 * NS + NG;

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

/// Net inputs for `s`, given the handcrafted score from the side to move's view.
pub fn inputs(s: &State, base_stm: i32) -> [f32; NI] {
    inputs_from(&super::hce::sides(s), s, base_stm)
}

pub fn inputs_from(sd: &Sides, s: &State, base_stm: i32) -> [f32; NI] {
    let stm = s.get_present().1 as usize;
    let mut x = [0f32; NI];
    for i in 0..NS {
        x[i] = sd.side[stm][i] as f32;
        x[NS + i] = sd.side[1 - stm][i] as f32;
    }
    let full = 6 * sd.active.max(1);
    let g = &mut x[2 * NS..];
    g[gi::ACTIVE] = sd.active as f32;
    g[gi::INACTIVE] = sd.inactive as f32;
    g[gi::HEAVY] = (8 * sd.heavy.min(full) / full) as f32;
    g[gi::BASE] = (base_stm as f32 / 100.0).clamp(-30.0, 30.0);
    g[gi::MANDATORY] = sd.mandatory as f32;
    x
}

/// Float MLP NI -> H1 -> H2 -> 1 with clipped ReLU; input normalization is folded into the
/// first layer by the exporter.
/// Largest hidden layer the forward pass supports.
const MAX_H: usize = 256;

pub struct Net {
    h1: usize,
    h2: usize,
    /// Input-major [ni][h1], so the forward pass accumulates whole hidden rows (vectorizes).
    w1t: Vec<f32>,
    b1: Vec<f32>,
    /// [h1][h2]
    w2t: Vec<f32>,
    b2: Vec<f32>,
    w3: Vec<f32>,
    b3: f32,
    /// Output units to centipawns, and the residual cap.
    scale: f32,
    cap: f32,
}

fn transpose(w: &[f32], rows: usize, cols: usize) -> Vec<f32> {
    let mut t = vec![0f32; w.len()];
    for r in 0..rows {
        for c in 0..cols {
            t[c * rows + r] = w[r * cols + c];
        }
    }
    t
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
        if ni != NI || h1 > MAX_H || h2 > MAX_H {
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
        let w1 = take(h1 * ni)?;
        let b1 = take(h1)?;
        let w2 = take(h2 * h1)?;
        let net = Net {
            h1,
            h2,
            w1t: transpose(&w1, h1, ni),
            b1,
            w2t: transpose(&w2, h2, h1),
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
        let (h1, h2) = (self.h1, self.h2);
        let mut a1 = [0f32; MAX_H];
        a1[..h1].copy_from_slice(&self.b1);
        for i in 0..NI {
            let xi = x[i];
            if xi != 0.0 {
                let row = &self.w1t[i * h1..(i + 1) * h1];
                for (a, w) in a1[..h1].iter_mut().zip(row) {
                    *a += w * xi;
                }
            }
        }
        let mut a2 = [0f32; MAX_H];
        a2[..h2].copy_from_slice(&self.b2);
        for j in 0..h1 {
            let aj = a1[j].clamp(0.0, 1.0);
            if aj != 0.0 {
                let row = &self.w2t[j * h2..(j + 1) * h2];
                for (a, w) in a2[..h2].iter_mut().zip(row) {
                    *a += w * aj;
                }
            }
        }
        let mut out = self.b3;
        for k in 0..h2 {
            out += self.w3[k] * a2[k].clamp(0.0, 1.0);
        }
        (out * self.scale).clamp(-self.cap, self.cap) as i32
    }
}

static NET: OnceLock<Option<Net>> = OnceLock::new();

/// The embedded net, if `data/evalnet.bin` holds one for the current inputs.
pub fn embedded() -> Option<&'static Net> {
    NET.get_or_init(|| Net::from_bytes(include_bytes!("../../data/evalnet.bin"))).as_ref()
}
