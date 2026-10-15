//! Hypercuboids, slices and search spaces for the hypercuboid action search.

use super::integer_set::{Index, IntegerSet};
use std::collections::BTreeMap;

pub type Point = Vec<Index>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hc {
    pub axes: Vec<IntegerSet>,
}

impl std::ops::Index<usize> for Hc {
    type Output = IntegerSet;
    #[inline(always)]
    fn index(&self, i: usize) -> &IntegerSet {
        &self.axes[i]
    }
}
impl std::ops::IndexMut<usize> for Hc {
    #[inline(always)]
    fn index_mut(&mut self, i: usize) -> &mut IntegerSet {
        &mut self.axes[i]
    }
}

/// Points sharing chosen coordinates on some axes; unlisted axes are free.
#[derive(Clone, Debug, Default)]
pub struct Slice {
    pub fixed: BTreeMap<usize, IntegerSet>,
}

impl Slice {
    pub fn new() -> Self {
        Slice { fixed: BTreeMap::new() }
    }
    pub fn fix_axis(&mut self, n: usize, values: IntegerSet) {
        self.fixed.insert(n, values);
    }
    pub fn contains(&self, p: &Point) -> bool {
        self.fixed.iter().all(|(&n, c)| c.contains(p[n]))
    }
}

impl Hc {
    pub fn new(axes: Vec<IntegerSet>) -> Self {
        Hc { axes }
    }
    #[inline(always)]
    pub fn dimension(&self) -> usize {
        self.axes.len()
    }
    pub fn contains(&self, p: &Point) -> bool {
        self.axes.iter().zip(p.iter()).all(|(a, &v)| a.contains(v))
    }
    pub fn intersects(&self, s: &Slice) -> bool {
        s.fixed.iter().all(|(&i, c)| self.axes[i].intersects(c))
    }
    pub fn is_empty(&self) -> bool {
        self.axes.iter().any(|a| a.is_empty())
    }
    pub fn volume(&self) -> usize {
        self.axes.iter().map(|a| a.len()).product()
    }

    /// Remove a slice the HC is known to intersect.
    pub fn remove_slice(&self, s: &Slice) -> SearchSpace {
        let mut result = SearchSpace::default();
        let mut remaining = self.clone();
        for (&i, fixed) in &s.fixed {
            let mut x = remaining.clone();
            x.axes[i].minus(fixed);
            remaining.axes[i] = fixed.clone();
            if !x.axes[i].is_empty() {
                result.hcs.push(x);
            }
        }
        result
    }

    pub fn remove_point(&self, p: &Point) -> SearchSpace {
        let mut result = SearchSpace::default();
        let mut remaining = self.clone();
        for i in 0..p.len() {
            let mut x = remaining.clone();
            x.axes[i].erase(p[i]);
            remaining.axes[i] = IntegerSet::singleton(p[i]);
            if !x.axes[i].is_empty() {
                result.hcs.push(x);
            }
        }
        result
    }

    pub fn remove_slice_carefully(&self, s: &Slice) -> SearchSpace {
        for (&i, fixed) in &s.fixed {
            if !self.axes[i].intersects(fixed) {
                return SearchSpace { hcs: vec![self.clone()] };
            }
        }
        self.remove_intersected(s)
    }

    fn remove_intersected(&self, s: &Slice) -> SearchSpace {
        let mut result = SearchSpace::default();
        let mut remaining = self.clone();
        for (&i, fixed) in &s.fixed {
            let inter = fixed.and(&remaining.axes[i]);
            let mut x = remaining.clone();
            x.axes[i].minus(&inter);
            remaining.axes[i] = inter;
            if !x.axes[i].is_empty() {
                result.hcs.push(x);
            }
        }
        result
    }

    pub fn is_slice_good(&self, s: &Slice, max_codim: usize) -> bool {
        let mut count = 0;
        for (&i, fixed) in &s.fixed {
            if !self.axes[i].intersects(fixed) {
                return false;
            }
            if !self.axes[i].is_subset_of(fixed) {
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
        a.axes[n] = IntegerSet::singleton(i);
        b.axes[n].erase(i);
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
