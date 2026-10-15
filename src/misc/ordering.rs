//! Per-axis coordinate orderings used when taking points from hypercuboids.

use super::integer_set::{Bits, Index};

/// Explicit per-axis orders. Axes from `shared` on all use the list of axis `shared` (the
/// hypercuboid search's branch axes have identical coordinates).
#[derive(Clone, Debug)]
pub struct AxisOrders {
    pub lists: Vec<Vec<Index>>,
    pub shared: usize,
}

impl Default for AxisOrders {
    fn default() -> Self {
        AxisOrders::new(Vec::new())
    }
}

impl AxisOrders {
    /// One list per axis, none shared.
    pub fn new(lists: Vec<Vec<Index>>) -> Self {
        AxisOrders { lists, shared: usize::MAX }
    }

    /// The order of axis `n`.
    #[inline(always)]
    pub fn axis(&self, n: usize) -> &[Index] {
        &self.lists[n.min(self.shared)]
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
