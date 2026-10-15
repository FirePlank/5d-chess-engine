//! Growable bitset of small non-negative integers; the first 64 values live inline.

pub type Index = u32;

#[derive(Clone, Default, Debug)]
pub struct IntegerSet {
    first: u64,
    rest: Vec<u64>,
}

impl PartialEq for IntegerSet {
    fn eq(&self, o: &Self) -> bool {
        if self.first != o.first {
            return false;
        }
        let n = self.rest.len().max(o.rest.len());
        (0..n).all(|i| self.rest.get(i).copied().unwrap_or(0) == o.rest.get(i).copied().unwrap_or(0))
    }
}
impl Eq for IntegerSet {}

impl IntegerSet {
    #[inline(always)]
    pub const fn new() -> Self {
        IntegerSet { first: 0, rest: Vec::new() }
    }
    #[inline(always)]
    pub fn singleton(v: Index) -> Self {
        let mut s = Self::new();
        s.insert(v);
        s
    }
    /// {0, 1, ..., n-1}
    pub fn range(n: Index) -> Self {
        let mut s = Self::new();
        if n == 0 {
            return s;
        }
        let blocks = (n as usize).div_ceil(64);
        if blocks > 1 {
            s.rest = vec![u64::MAX; blocks - 1];
        }
        s.first = u64::MAX;
        let tail = n as usize % 64;
        if tail != 0 {
            let m = (1u64 << tail) - 1;
            if blocks == 1 {
                s.first = m;
            } else {
                *s.rest.last_mut().unwrap() = m;
            }
        }
        s
    }
    #[inline(always)]
    fn block(&self, i: usize) -> u64 {
        if i == 0 { self.first } else { self.rest.get(i - 1).copied().unwrap_or(0) }
    }
    #[inline(always)]
    fn block_mut(&mut self, i: usize) -> &mut u64 {
        if i == 0 { &mut self.first } else { &mut self.rest[i - 1] }
    }
    #[inline(always)]
    fn nblocks(&self) -> usize {
        self.rest.len() + 1
    }

    #[inline(always)]
    pub fn insert(&mut self, v: Index) {
        let b = (v >> 6) as usize;
        if b == 0 {
            self.first |= 1u64 << (v & 63);
        } else {
            if self.rest.len() < b {
                self.rest.resize(b, 0);
            }
            self.rest[b - 1] |= 1u64 << (v & 63);
        }
    }
    #[inline(always)]
    pub fn contains(&self, v: Index) -> bool {
        let b = (v >> 6) as usize;
        if b >= self.nblocks() {
            return false;
        }
        self.block(b) >> (v & 63) & 1 != 0
    }
    #[inline(always)]
    pub fn erase(&mut self, v: Index) -> bool {
        let b = (v >> 6) as usize;
        if b >= self.nblocks() {
            return false;
        }
        let blk = self.block_mut(b);
        let was = *blk >> (v & 63) & 1 != 0;
        *blk &= !(1u64 << (v & 63));
        was
    }
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.first == 0 && self.rest.iter().all(|&b| b == 0)
    }
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.first.count_ones() as usize + self.rest.iter().map(|b| b.count_ones() as usize).sum::<usize>()
    }
    #[inline(always)]
    pub fn intersects(&self, o: &IntegerSet) -> bool {
        if self.first & o.first != 0 {
            return true;
        }
        self.rest.iter().zip(o.rest.iter()).any(|(a, b)| a & b != 0)
    }
    pub fn is_subset_of(&self, o: &IntegerSet) -> bool {
        if self.first & !o.first != 0 {
            return false;
        }
        for (i, &b) in self.rest.iter().enumerate() {
            if b & !o.rest.get(i).copied().unwrap_or(0) != 0 {
                return false;
            }
        }
        true
    }
    #[inline]
    pub fn minus(&mut self, o: &IntegerSet) {
        self.first &= !o.first;
        for (a, b) in self.rest.iter_mut().zip(o.rest.iter()) {
            *a &= !b;
        }
    }
    pub fn or_assign(&mut self, o: &IntegerSet) {
        self.first |= o.first;
        if self.rest.len() < o.rest.len() {
            self.rest.resize(o.rest.len(), 0);
        }
        for (a, b) in self.rest.iter_mut().zip(o.rest.iter()) {
            *a |= b;
        }
    }
    pub fn and_assign(&mut self, o: &IntegerSet) {
        self.first &= o.first;
        let n = self.rest.len().min(o.rest.len());
        self.rest.truncate(n);
        for (a, b) in self.rest.iter_mut().zip(o.rest.iter()) {
            *a &= b;
        }
    }
    pub fn and(&self, o: &IntegerSet) -> IntegerSet {
        let mut r = self.clone();
        r.and_assign(o);
        r
    }
    /// Remove every element for which `pred` holds (visited in ascending order).
    pub fn erase_if(&mut self, mut pred: impl FnMut(Index) -> bool) {
        for bi in 0..self.nblocks() {
            let mut bits = self.block(bi);
            while bits != 0 {
                let k = bits.trailing_zeros();
                bits &= bits - 1;
                let v = ((bi as u32) << 6) | k;
                if pred(v) {
                    *self.block_mut(bi) &= !(1u64 << k);
                }
            }
        }
    }
    #[inline(always)]
    pub fn iter(&self) -> IntegerSetIter<'_> {
        IntegerSetIter { set: self, block: 0, bits: self.first }
    }
    pub fn first(&self) -> Option<Index> {
        self.iter().next()
    }
    pub fn max(&self) -> Option<Index> {
        for bi in (0..self.nblocks()).rev() {
            let b = self.block(bi);
            if b != 0 {
                return Some(((bi as u32) << 6) | (63 - b.leading_zeros()));
            }
        }
        None
    }
}

pub struct IntegerSetIter<'a> {
    set: &'a IntegerSet,
    block: usize,
    bits: u64,
}
impl Iterator for IntegerSetIter<'_> {
    type Item = Index;
    #[inline(always)]
    fn next(&mut self) -> Option<Index> {
        loop {
            if self.bits != 0 {
                let k = self.bits.trailing_zeros();
                self.bits &= self.bits - 1;
                return Some(((self.block as u32) << 6) | k);
            }
            self.block += 1;
            if self.block >= self.set.nblocks() {
                return None;
            }
            self.bits = self.set.block(self.block);
        }
    }
}
impl<'a> IntoIterator for &'a IntegerSet {
    type Item = Index;
    type IntoIter = IntegerSetIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl std::fmt::Display for IntegerSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;
        let mut first = true;
        for v in self.iter() {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            write!(f, "{}", v)?;
        }
        write!(f, "}}")
    }
}
