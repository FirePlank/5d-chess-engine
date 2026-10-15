//! Per-axis coordinate orderings used when taking points from hypercuboids.

use super::integer_set::{Bits, Index};

#[derive(Clone, Debug)]
pub enum HcOrdering {
    /// Ascending coordinate order.
    Natural,
    /// Explicit per-axis order.
    Precomputed(Vec<Vec<Index>>),
}

impl HcOrdering {
    /// The explicit per-axis orders (switching to an empty explicit ordering if natural).
    pub fn axes_mut(&mut self) -> &mut Vec<Vec<Index>> {
        if let HcOrdering::Natural = self {
            *self = HcOrdering::Precomputed(Vec::new());
        }
        match self {
            HcOrdering::Precomputed(o) => o,
            HcOrdering::Natural => unreachable!(),
        }
    }

    #[inline]
    pub fn for_each(&self, n: usize, s: &Bits, mut f: impl FnMut(Index)) {
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
    pub fn first(&self, n: usize, s: &Bits) -> Option<Index> {
        match self {
            HcOrdering::Natural => s.first(),
            HcOrdering::Precomputed(o) => o[n].iter().copied().find(|&i| s.contains(i)),
        }
    }
}
