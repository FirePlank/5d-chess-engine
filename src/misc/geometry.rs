//! Hypercuboids, slices and search spaces for the hypercuboid action search.

use super::integer_set::{Bits, Index, IntegerSet};

pub type Point = Vec<Index>;

/// A product of per-axis coordinate sets, stored flat: every axis gets `k` words. Word
/// buffers are recycled through a per-thread pool.
#[derive(Default, PartialEq, Eq)]
pub struct Hc {
    k: usize,
    w: Vec<u64>,
}

thread_local! {
    static HC_POOL: std::cell::RefCell<Vec<Vec<u64>>> = const { std::cell::RefCell::new(Vec::new()) };
}
const HC_POOL_MAX: usize = 1 << 16;

/// A word buffer holding a copy of `src` (from the pool when one is free).
#[inline]
fn pooled_copy(src: &[u64]) -> Vec<u64> {
    let mut w = HC_POOL.with(|p| p.borrow_mut().pop()).unwrap_or_default();
    w.clear();
    w.extend_from_slice(src);
    w
}

impl Clone for Hc {
    #[inline]
    fn clone(&self) -> Self {
        Hc { k: self.k, w: pooled_copy(&self.w) }
    }
}

impl Drop for Hc {
    #[inline]
    fn drop(&mut self) {
        if self.w.capacity() != 0 {
            let w = std::mem::take(&mut self.w);
            HC_POOL.with(|p| {
                let mut p = p.borrow_mut();
                if p.len() < HC_POOL_MAX {
                    p.push(w);
                }
            });
        }
    }
}

impl std::ops::Index<usize> for Hc {
    type Output = Bits;
    #[inline(always)]
    fn index(&self, i: usize) -> &Bits {
        Bits::from_words(&self.w[i * self.k..(i + 1) * self.k])
    }
}
impl std::ops::IndexMut<usize> for Hc {
    #[inline(always)]
    fn index_mut(&mut self, i: usize) -> &mut Bits {
        let k = self.k;
        Bits::from_words_mut(&mut self.w[i * k..(i + 1) * k])
    }
}

impl std::fmt::Debug for Hc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries((0..self.dimension()).map(|n| &self[n])).finish()
    }
}

/// Points sharing chosen coordinates on some axes; unlisted axes are free. Axes are kept in
/// ascending order.
#[derive(Clone, Debug, Default)]
pub struct Slice {
    pub fixed: Vec<(usize, IntegerSet)>,
}

impl Slice {
    pub fn new() -> Self {
        Slice { fixed: Vec::new() }
    }
    /// Fix axis `n` to `values` (replacing an earlier choice for it).
    pub fn fix_axis(&mut self, n: usize, values: IntegerSet) {
        match self.fixed.binary_search_by_key(&n, |e| e.0) {
            Ok(i) => self.fixed[i].1 = values,
            Err(i) => self.fixed.insert(i, (n, values)),
        }
    }
    /// Restrict axis `n` to `values` (intersecting an earlier choice for it).
    pub fn restrict_axis(&mut self, n: usize, values: IntegerSet) {
        match self.fixed.binary_search_by_key(&n, |e| e.0) {
            Ok(i) => self.fixed[i].1.and_assign(&values),
            Err(i) => self.fixed.insert(i, (n, values)),
        }
    }
    pub fn contains(&self, p: &Point) -> bool {
        self.fixed.iter().all(|(n, c)| c.contains(p[*n]))
    }
}

impl Hc {
    /// Axes sized to hold every value of the given sets (and of any subset of them).
    pub fn new(axes: Vec<IntegerSet>) -> Self {
        let k = axes.iter().map(|a| a.words().len()).max().unwrap_or(0).max(1);
        let mut w = vec![0u64; k * axes.len()];
        for (n, a) in axes.iter().enumerate() {
            w[n * k..n * k + a.words().len()].copy_from_slice(a.words());
        }
        Hc { k, w }
    }
    /// Make this `dim` axes, axis `n` holding {0, ..., len(n) - 1} (reusing the storage).
    pub fn set_ranges(&mut self, dim: usize, len: impl Fn(usize) -> usize) {
        let k = (0..dim).map(|n| len(n).div_ceil(64)).max().unwrap_or(0).max(1);
        self.k = k;
        self.w.clear();
        self.w.resize(k * dim, 0);
        for n in 0..dim {
            let l = len(n);
            let ax = &mut self.w[n * k..(n + 1) * k];
            ax[..l / 64].fill(u64::MAX);
            if l % 64 != 0 {
                ax[l / 64] = (1u64 << (l % 64)) - 1;
            }
        }
    }
    #[inline(always)]
    pub fn dimension(&self) -> usize {
        self.w.len() / self.k.max(1)
    }
    pub fn contains(&self, p: &Point) -> bool {
        (0..self.dimension()).all(|n| self[n].contains(p[n]))
    }
    pub fn intersects(&self, s: &Slice) -> bool {
        s.fixed.iter().all(|(i, c)| self[*i].intersects(c))
    }
    pub fn is_empty(&self) -> bool {
        (0..self.dimension()).any(|n| self[n].is_empty())
    }
    pub fn volume(&self) -> usize {
        (0..self.dimension()).map(|n| self[n].len()).product()
    }

    /// Remove a slice the HC is known to intersect; the pieces go to `out` in order.
    pub fn remove_slice_with(&self, s: &Slice, mut out: impl FnMut(Hc)) {
        let mut remaining = self.clone();
        for (i, fixed) in &s.fixed {
            let i = *i;
            let mut x = remaining.clone();
            x[i].minus(fixed);
            // A slice may list coordinates outside this HC: the later pieces must not gain them.
            remaining[i].and_assign(fixed);
            if !x[i].is_empty() {
                out(x);
            }
        }
    }

    pub fn remove_slice(&self, s: &Slice) -> SearchSpace {
        let mut r = SearchSpace::default();
        self.remove_slice_with(s, |h| r.hcs.push(h));
        r
    }

    /// Everything but the point `p`, as disjoint pieces (in axis order) passed to `out`.
    pub fn remove_point_with(&self, p: &Point, mut out: impl FnMut(Hc)) {
        let mut remaining = self.clone();
        for i in 0..p.len() {
            if remaining[i].has_other_than(p[i]) {
                let mut x = remaining.clone();
                x[i].erase(p[i]);
                out(x);
            }
            remaining[i].assign_singleton(p[i]);
        }
    }

    pub fn remove_point(&self, p: &Point) -> SearchSpace {
        let mut r = SearchSpace::default();
        self.remove_point_with(p, |h| r.hcs.push(h));
        r
    }

    pub fn remove_slice_carefully(&self, s: &Slice) -> SearchSpace {
        for (i, fixed) in &s.fixed {
            if !self[*i].intersects(fixed) {
                return SearchSpace { hcs: vec![self.clone()] };
            }
        }
        self.remove_intersected(s)
    }

    fn remove_intersected(&self, s: &Slice) -> SearchSpace {
        let mut result = SearchSpace::default();
        let mut remaining = self.clone();
        for (i, fixed) in &s.fixed {
            let i = *i;
            let mut x = remaining.clone();
            // x[i] = remaining[i] \ fixed; remaining[i] = remaining[i] ∩ fixed.
            x[i].minus(fixed);
            remaining[i].and_assign(fixed);
            if !x[i].is_empty() {
                result.hcs.push(x);
            }
        }
        result
    }

    pub fn is_slice_good(&self, s: &Slice, max_codim: usize) -> bool {
        let mut count = 0;
        for (i, fixed) in &s.fixed {
            if !self[*i].intersects(fixed) {
                return false;
            }
            if !self[*i].is_subset_of(fixed) {
                if count >= max_codim {
                    return false;
                }
                count += 1;
            }
        }
        true
    }

    pub fn remove_slice_if_good(&self, s: &Slice, max_codim: usize) -> SearchSpace {
        if !self.is_slice_good(s, max_codim) {
            return SearchSpace { hcs: vec![self.clone()] };
        }
        self.remove_intersected(s)
    }

    /// (part with value i on axis n, part without it)
    pub fn split(&self, n: usize, i: Index) -> (Hc, Hc) {
        let mut a = self.clone();
        let mut b = self.clone();
        a[n].assign_singleton(i);
        b[n].erase(i);
        (a, b)
    }
}

/// Union of hypercuboids, consumed from the back.
#[derive(Clone, Debug, Default)]
pub struct SearchSpace {
    pub hcs: Vec<Hc>,
}

impl SearchSpace {
    pub fn from_vec(hcs: Vec<Hc>) -> Self {
        SearchSpace { hcs }
    }
    pub fn is_empty(&self) -> bool {
        self.hcs.iter().all(|h| h.is_empty())
    }
    pub fn len(&self) -> usize {
        self.hcs.len()
    }
    pub fn volume(&self) -> usize {
        self.hcs.iter().map(|h| h.volume()).sum()
    }
    pub fn contains(&self, p: &Point) -> bool {
        self.hcs.iter().any(|h| h.contains(p))
    }
    pub fn concat(&mut self, other: SearchSpace) {
        self.hcs.extend(other.hcs);
    }
    pub fn prune_empty(&mut self) {
        self.hcs.retain(|h| !h.is_empty());
    }
    pub fn push_back(&mut self, hc: Hc) {
        self.hcs.push(hc);
    }
    pub fn push_front(&mut self, hc: Hc) {
        self.hcs.insert(0, hc);
    }
    pub fn back(&mut self) -> &mut Hc {
        self.hcs.last_mut().unwrap()
    }
    pub fn pop_back(&mut self) -> Option<Hc> {
        self.hcs.pop()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, Hc> {
        self.hcs.iter()
    }
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Hc> {
        self.hcs.iter_mut()
    }

    /// Starting at the back, remove `s` while intersections stay dense, replacing each HC in place.
    pub fn remove_slice_backwards(&mut self, s: &Slice, mut force_back: bool, disjoint_weight: usize, max_codim: usize) {
        if self.hcs.is_empty() {
            return;
        }
        let mut intersect = 0usize;
        let mut disjoint = 0usize;
        let mut current = self.hcs.len() - 1;
        loop {
            let has_previous = current != 0;
            if self.hcs[current].intersects(s) {
                let pieces = if force_back {
                    self.hcs[current].remove_slice_carefully(s)
                } else {
                    self.hcs[current].remove_slice_if_good(s, max_codim)
                };
                self.hcs.splice(current..current + 1, pieces.hcs);
                intersect += 1;
            } else {
                disjoint += 1;
            }
            if !has_previous || disjoint * disjoint_weight >= intersect {
                break;
            }
            force_back = false;
            current -= 1;
        }
    }
}
