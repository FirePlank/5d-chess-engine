//! Static-evaluation correction history: running averages of how far search results landed
//! from the static evaluation, by side to move and by keys of the position's structure (the
//! pawns of the timelines' latest boards; the shape of the multiverse), added back to the
//! static evaluation.

use crate::core::state::State;

const BITS: usize = 14;
/// Largest correction (eval units).
const MAX: i32 = 2000;
/// Weight of one update, out of 1024, per unit of depth (capped at `MAX_WEIGHT`).
const WEIGHT_PER_DEPTH: i32 = 16;
const MAX_WEIGHT: i32 = 128;

pub struct CorrHist {
    /// [key kind][side to move][slot]
    tables: Vec<i32>,
    /// Slots of recently seen positions by position hash (direct-mapped).
    seen: Vec<(u64, [u32; 2])>,
}

const SEEN_BITS: usize = 12;

impl Default for CorrHist {
    fn default() -> Self {
        CorrHist { tables: vec![0; 2 * 2 << BITS], seen: vec![(0, [0, 0]); 1 << SEEN_BITS] }
    }
}

#[inline]
fn mix(k: u64, x: u64) -> u64 {
    let k = (k ^ x).wrapping_mul(0xbf58476d1ce4e5b9);
    k ^ (k >> 29)
}

impl CorrHist {
    /// Slots of the position's keys, through the cache.
    #[inline]
    fn slots_of(&mut self, s: &State) -> [usize; 2] {
        let h = s.hash();
        let e = &mut self.seen[(h >> (64 - SEEN_BITS)) as usize];
        if e.0 != h || h == 0 {
            let [a, b] = Self::slots(s);
            *e = (h, [a as u32, b as u32]);
        }
        [e.1[0] as usize, e.1[1] as usize]
    }

    /// Slots of the position's keys.
    fn slots(s: &State) -> [usize; 2] {
        let (amin, amax) = s.get_active_range();
        let (lmin, lmax) = s.get_lines_range();
        let present = s.get_present();
        let stm = present.1 as usize;
        let mut pawns = 0x9e3779b97f4a7c15u64;
        let mut mandatory = 0u64;
        for l in amin..=amax {
            let (t, c) = s.get_timeline_end(l);
            let b = s.board(l, t, c);
            pawns = mix(pawns, (l as u64) << 56 ^ (b.lpawn() & b.white()));
            pawns = mix(pawns, b.lpawn() & b.black());
            mandatory += ((t, c) == present) as u64;
        }
        let (l0min, l0max) = s.get_initial_lines_range();
        let slots = ((lmax - l0max) - (l0min - lmin)) as u64 & 0xff;
        let active = (amax - amin + 1) as u64;
        let inactive = (lmax - lmin + 1) as u64 - active;
        let shape = mix(mix(0x2545f4914f6cdd1d, active | inactive << 8 | mandatory << 16 | slots << 24), 1);
        let mask = (1usize << BITS) - 1;
        [(stm << BITS) | (pawns >> 40) as usize & mask, (2 + stm) << BITS | (shape >> 40) as usize & mask]
    }

    /// Correction to add to the static evaluation of `s`.
    #[inline]
    pub fn correction(&mut self, s: &State) -> i32 {
        let [a, b] = self.slots_of(s);
        ((self.tables[a] + self.tables[b]) / 2).clamp(-MAX, MAX)
    }

    /// A search of `depth` found `diff` = result - static evaluation (uncorrected).
    pub fn update(&mut self, s: &State, depth: i32, diff: i32) {
        let w = (WEIGHT_PER_DEPTH * depth).min(MAX_WEIGHT);
        let diff = diff.clamp(-2 * MAX, 2 * MAX);
        for i in self.slots_of(s) {
            let e = &mut self.tables[i];
            *e += (diff - *e) * w / 1024;
        }
    }

    pub fn clear(&mut self) {
        self.tables.fill(0);
        self.seen.fill((0, [0, 0]));
    }
}
