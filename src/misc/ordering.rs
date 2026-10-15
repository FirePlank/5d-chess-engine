//! Per-axis coordinate orderings used when taking points from hypercuboids.

use super::geometry::Hc;
use super::integer_set::{Index, IntegerSet};
use super::rng::{discrete_cdf, Mt19937};

#[derive(Clone, Debug)]
pub enum HcOrdering {
    /// Ascending coordinate order.
    Natural,
    /// Explicit per-axis order (random / scored / weighted).
    Precomputed(Vec<Vec<Index>>),
}

impl HcOrdering {
    #[inline]
    pub fn for_each(&self, n: usize, s: &IntegerSet, mut f: impl FnMut(Index)) {
        match self {
            HcOrdering::Natural => {
                for i in s.iter() {
                    f(i);
                }
            }
            HcOrdering::Precomputed(o) => {
                for &i in &o[n] {
                    if s.contains(i) {
                        f(i);
                    }
                }
            }
        }
    }

    /// First element of `s` in this axis order.
    #[inline]
    pub fn first(&self, n: usize, s: &IntegerSet) -> Option<Index> {
        match self {
            HcOrdering::Natural => s.first(),
            HcOrdering::Precomputed(o) => o[n].iter().copied().find(|&i| s.contains(i)),
        }
    }

    pub fn random(hc: &Hc, rng: &mut Mt19937) -> Self {
        let _p = crate::misc::prof::scope(6);
        let mut orderings = Vec::with_capacity(hc.dimension());
        for n in 0..hc.dimension() {
            let mut v: Vec<Index> = hc[n].iter().collect();
            rng.shuffle(&mut v);
            orderings.push(v);
        }
        HcOrdering::Precomputed(orderings)
    }

    /// Higher scores first; ties broken by a random shuffle.
    pub fn scored(hc: &Hc, scores: &[Vec<f32>], rng: &mut Mt19937) -> Self {
        let mut orderings = Vec::with_capacity(hc.dimension());
        for n in 0..hc.dimension() {
            let mut v: Vec<Index> = hc[n].iter().collect();
            rng.shuffle(&mut v);
            let sc = &scores[n];
            super::libcxx_sort::sort_by(&mut v, |&i, &j| sc[i as usize] > sc[j as usize]);
            orderings.push(v);
        }
        HcOrdering::Precomputed(orderings)
    }

    /// Weighted random permutation: repeatedly draw the next element proportionally to weight.
    pub fn weighted(hc: &Hc, mut weights: Vec<Vec<f32>>, rng: &mut Mt19937) -> Self {
        let _p = crate::misc::prof::scope(6);
        let mut orderings: Vec<Vec<Index>> = (0..hc.dimension()).map(|n| hc[n].iter().collect()).collect();
        for n in 0..hc.dimension() {
            let ord = &mut orderings[n];
            let w = &mut weights[n];
            for position in 0..ord.len() {
                let cdf = discrete_cdf(&w[position..]);
                let selected = position + rng.discrete_with_cdf(&cdf);
                ord.swap(position, selected);
                w.swap(position, selected);
            }
        }
        HcOrdering::Precomputed(orderings)
    }
}
