//! Per-axis coordinate orderings used when taking points from hypercuboids.

use super::integer_set::{Bits, Index};

/// Explicit per-axis orders. Axes from `shared` on all use the list of axis `shared` (the
/// hypercuboid search's branch axes have identical coordinates).
#[derive(Clone, Debug)]
pub struct AxisOrders {
    pub lists: Vec<Vec<Index>>,
    pub shared: usize,
    /// Per list, the same order split by kind of coordinate (see `AxisSplit`). Whoever writes
    /// a list rebuilds its split or marks it invalid.
    pub splits: Vec<AxisSplit>,
}

/// An axis order split by what its coordinates are to the hypercuboid search: the non-jump
/// ones (null and physical moves) and the arrivals from each departure axis, each in order.
#[derive(Clone, Debug, Default)]
pub struct AxisSplit {
    pub valid: bool,
    pub nonjump: Vec<Index>,
    /// The arrivals from departure axis `fa` are `arrivals[from[fa]..from[fa + 1]]`.
    pub arrivals: Vec<Index>,
    pub from: Vec<u32>,
}

impl AxisSplit {
    /// The arrivals from departure axis `fa`, in order.
    #[inline]
    pub fn arrivals_from(&self, fa: usize) -> &[Index] {
        match self.from.get(fa..fa + 2) {
            Some(r) => &self.arrivals[r[0] as usize..r[1] as usize],
            None => &[],
        }
    }
}

impl Default for AxisOrders {
    fn default() -> Self {
        AxisOrders::new(Vec::new())
    }
}

impl AxisOrders {
    /// One list per axis, none shared.
    pub fn new(lists: Vec<Vec<Index>>) -> Self {
        AxisOrders { lists, shared: usize::MAX, splits: Vec::new() }
    }

    /// The order of axis `n`.
    #[inline(always)]
    pub fn axis(&self, n: usize) -> &[Index] {
        &self.lists[n.min(self.shared)]
    }

    /// The split of axis `n`'s order, when it has a valid one.
    #[inline(always)]
    pub fn split(&self, n: usize) -> Option<&AxisSplit> {
        self.splits.get(n.min(self.shared)).filter(|s| s.valid)
    }
}

#[derive(Clone, Debug)]
pub enum HcOrdering {
    /// Ascending coordinate order.
    Natural,
    /// Explicit per-axis order.
    Precomputed(AxisOrders),
}

impl HcOrdering {
    /// The explicit per-axis orders (switching to an empty explicit ordering if natural).
    pub fn axes_mut(&mut self) -> &mut AxisOrders {
        if let HcOrdering::Natural = self {
            *self = HcOrdering::Precomputed(AxisOrders::default());
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
                for &i in o.axis(n) {
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
            HcOrdering::Precomputed(o) => o.axis(n).iter().copied().find(|&i| s.contains(i)),
        }
    }
}
