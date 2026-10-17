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
        #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
        let out = match (self.h1, self.h2) {
            // SAFETY: the target has AVX2 (checked at compile time), and the layer
            // sizes match the blob's (checked by `from_bytes`).
            (64, 32) => unsafe { self.forward_avx::<8>(x) },
            (96, 32) => unsafe { self.forward_avx::<12>(x) },
            (128, 32) => unsafe { self.forward_avx::<16>(x) },
            _ => self.forward(x),
        };
        #[cfg(not(all(target_arch = "x86_64", target_feature = "avx2")))]
        let out = match (self.h1, self.h2) {
            (64, 32) => self.forward_fixed::<64, 32>(x),
            (96, 32) => self.forward_fixed::<96, 32>(x),
            (128, 32) => self.forward_fixed::<128, 32>(x),
            _ => self.forward(x),
        };
        (out * self.scale).clamp(-self.cap, self.cap) as i32
    }

    /// `forward_fixed` with AVX2 intrinsics for a first layer of 8 * L units and a second of 32:
    /// the first layer's accumulators stay in registers across the non-zero inputs. Same
    /// operations in the same order (no fused multiply-adds), so the result is bit-identical.
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    #[inline]
    unsafe fn forward_avx<const L: usize>(&self, x: &[f32; NI]) -> f32 {
        use std::arch::x86_64::*;
        unsafe {
        let h1 = 8 * L;
        let w1 = self.w1t.as_ptr();
        let mut acc = [_mm256_setzero_ps(); L];
        for (l, a) in acc.iter_mut().enumerate() {
            *a = _mm256_loadu_ps(self.b1.as_ptr().add(8 * l));
        }
        for (i, &xi) in x.iter().enumerate() {
            if xi != 0.0 {
                let v = _mm256_set1_ps(xi);
                let row = w1.add(i * h1);
                for (l, a) in acc.iter_mut().enumerate() {
                    *a = _mm256_add_ps(*a, _mm256_mul_ps(_mm256_loadu_ps(row.add(8 * l)), v));
                }
            }
        }
        let (zero, one) = (_mm256_setzero_ps(), _mm256_set1_ps(1.0));
        let mut a1 = [0f32; 128];
        for (l, a) in acc.iter().enumerate() {
            _mm256_storeu_ps(a1.as_mut_ptr().add(8 * l), _mm256_min_ps(_mm256_max_ps(*a, zero), one));
        }
        let b2 = self.b2.as_ptr();
        let mut a2 = [_mm256_loadu_ps(b2), _mm256_loadu_ps(b2.add(8)), _mm256_loadu_ps(b2.add(16)), _mm256_loadu_ps(b2.add(24))];
        let w2 = self.w2t.as_ptr();
        for (j, &aj) in a1[..h1].iter().enumerate() {
            if aj != 0.0 {
                let v = _mm256_set1_ps(aj);
                let row = w2.add(j * 32);
                for (k, a) in a2.iter_mut().enumerate() {
                    *a = _mm256_add_ps(*a, _mm256_mul_ps(_mm256_loadu_ps(row.add(8 * k)), v));
                }
            }
        }
        let mut t = [0f32; 32];
        for (k, a) in a2.iter().enumerate() {
            _mm256_storeu_ps(t.as_mut_ptr().add(8 * k), _mm256_min_ps(_mm256_max_ps(*a, zero), one));
        }
        let mut out = self.b3;
        for k in 0..32 {
            out += self.w3[k] * t[k];
        }
        out
        }
    }

    /// `forward` with the layer sizes known at compile time (same operations, same order).
    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx2")))]
    #[inline]
    fn forward_fixed<const H1: usize, const H2: usize>(&self, x: &[f32; NI]) -> f32 {
        let mut a1: [f32; H1] = self.b1[..H1].try_into().unwrap();
        for i in 0..NI {
            let xi = x[i];
            if xi != 0.0 {
                let row: &[f32; H1] = self.w1t[i * H1..(i + 1) * H1].try_into().unwrap();
                for j in 0..H1 {
                    a1[j] += row[j] * xi;
                }
            }
        }
        let mut a2: [f32; H2] = self.b2[..H2].try_into().unwrap();
        for j in 0..H1 {
            let aj = a1[j].clamp(0.0, 1.0);
            if aj != 0.0 {
                let row: &[f32; H2] = self.w2t[j * H2..(j + 1) * H2].try_into().unwrap();
                for k in 0..H2 {
                    a2[k] += row[k] * aj;
                }
            }
        }
        let mut out = self.b3;
        for k in 0..H2 {
            out += self.w3[k] * a2[k].clamp(0.0, 1.0);
        }
        out
    }

    /// Output before scaling, for any layer sizes.
    fn forward(&self, x: &[f32; NI]) -> f32 {
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
        out
    }
}

static NET: OnceLock<Option<Net>> = OnceLock::new();

/// Use the net in `bytes` (`data/evalnet.bin` format) instead of the embedded one, for tests
/// and tools; only takes effect before the first evaluation. Returns whether it did.
pub fn install(bytes: &[u8]) -> bool {
    NET.set(Net::from_bytes(bytes)).is_ok()
}

/// The embedded net, if `data/evalnet.bin` holds one for the current inputs.
pub fn embedded() -> Option<&'static Net> {
    NET.get_or_init(|| Net::from_bytes(include_bytes!("../../data/evalnet.bin"))).as_ref()
}
