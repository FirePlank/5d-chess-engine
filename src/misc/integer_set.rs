//! Bitsets of small non-negative integers. `Bits` is a borrowed view over words (a hypercuboid
//! axis stores its coordinates in place); `IntegerSet` is an owned set whose first two words
//! live inline.

pub type Index = u32;

/// A bitset over a borrowed word slice; values beyond the slice are absent.
#[repr(transparent)]
pub struct Bits([u64]);

impl Bits {
    #[inline(always)]
    pub fn from_words(w: &[u64]) -> &Bits {
        // SAFETY: `Bits` is a transparent wrapper around `[u64]`.
        unsafe { &*(w as *const [u64] as *const Bits) }
    }
    #[inline(always)]
    pub fn from_words_mut(w: &mut [u64]) -> &mut Bits {
        // SAFETY: as above.
        unsafe { &mut *(w as *mut [u64] as *mut Bits) }
    }
    #[inline(always)]
    pub fn words(&self) -> &[u64] {
        &self.0
    }
    #[inline(always)]
    pub fn words_mut(&mut self) -> &mut [u64] {
        &mut self.0
    }
    #[inline(always)]
    fn word(&self, i: usize) -> u64 {
        self.0.get(i).copied().unwrap_or(0)
    }

    #[inline(always)]
    pub fn contains(&self, v: Index) -> bool {
        let b = (v >> 6) as usize;
        b < self.0.len() && self.0[b] >> (v & 63) & 1 != 0
    }
    /// Insert a value inside the view's capacity.
    #[inline(always)]
    pub fn insert(&mut self, v: Index) {
        self.0[(v >> 6) as usize] |= 1u64 << (v & 63);
    }
    #[inline(always)]
    pub fn erase(&mut self, v: Index) -> bool {
        let b = (v >> 6) as usize;
        if b >= self.0.len() {
            return false;
        }
        let was = self.0[b] >> (v & 63) & 1 != 0;
        self.0[b] &= !(1u64 << (v & 63));
        was
    }
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&w| w == 0)
    }
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.iter().map(|w| w.count_ones() as usize).sum()
    }
    #[inline(always)]
    pub fn intersects(&self, o: &Bits) -> bool {
        self.0.iter().zip(o.0.iter()).any(|(a, b)| a & b != 0)
    }
    pub fn is_subset_of(&self, o: &Bits) -> bool {
        self.0.iter().enumerate().all(|(i, &w)| w & !o.word(i) == 0)
    }
    /// Whether this set has an element other than `v`.
    #[inline(always)]
    pub fn has_other_than(&self, v: Index) -> bool {
        let b = (v >> 6) as usize;
        self.0.iter().enumerate().any(|(i, &w)| if i == b { w & !(1u64 << (v & 63)) != 0 } else { w != 0 })
    }
    #[inline(always)]
    pub fn minus(&mut self, o: &Bits) {
        for (a, b) in self.0.iter_mut().zip(o.0.iter()) {
            *a &= !b;
        }
    }
    #[inline(always)]
    pub fn and_assign(&mut self, o: &Bits) {
        for (i, a) in self.0.iter_mut().enumerate() {
            *a &= o.word(i);
        }
    }
    /// Union with `o`, whose elements must fit this view.
    #[inline(always)]
    pub fn or_assign(&mut self, o: &Bits) {
        debug_assert!(o.0.iter().skip(self.0.len()).all(|&w| w == 0));
        for (a, b) in self.0.iter_mut().zip(o.0.iter()) {
            *a |= b;
        }
    }
    /// Replace the contents with `o`, whose elements must fit this view.
    #[inline(always)]
    pub fn assign(&mut self, o: &Bits) {
        debug_assert!(o.0.iter().skip(self.0.len()).all(|&w| w == 0));
        for (i, a) in self.0.iter_mut().enumerate() {
            *a = o.word(i);
        }
    }
    /// Replace the contents with `{v}`.
    #[inline(always)]
    pub fn assign_singleton(&mut self, v: Index) {
        self.0.fill(0);
        self.insert(v);
    }
    #[inline(always)]
    pub fn clear(&mut self) {
        self.0.fill(0);
    }
    /// Remove every element for which `pred` holds (visited in ascending order).
    pub fn erase_if(&mut self, mut pred: impl FnMut(Index) -> bool) {
        for bi in 0..self.0.len() {
            let mut bits = self.0[bi];
            while bits != 0 {
                let k = bits.trailing_zeros();
                bits &= bits - 1;
                if pred(((bi as u32) << 6) | k) {
                    self.0[bi] &= !(1u64 << k);
                }
            }
        }
    }
    #[inline(always)]
    pub fn iter(&self) -> BitsIter<'_> {
        BitsIter { words: &self.0, block: 0, bits: self.word(0) }
    }
    #[inline(always)]
    pub fn first(&self) -> Option<Index> {
        self.iter().next()
    }
    /// The element, when there is exactly one.
    #[inline]
    pub fn single(&self) -> Option<Index> {
        let mut found = None;
        for (bi, &w) in self.0.iter().enumerate() {
            if w != 0 {
                if found.is_some() || w & (w - 1) != 0 {
                    return None;
                }
                found = Some(((bi as u32) << 6) | w.trailing_zeros());
            }
        }
        found
    }
    pub fn max(&self) -> Option<Index> {
        for bi in (0..self.0.len()).rev() {
            let b = self.0[bi];
            if b != 0 {
                return Some(((bi as u32) << 6) | (63 - b.leading_zeros()));
            }
        }
        None
    }
    /// Words needed to hold every element.
    #[inline(always)]
    pub fn used_words(&self) -> usize {
        self.0.iter().rposition(|&w| w != 0).map(|i| i + 1).unwrap_or(0)
    }
}

impl PartialEq for Bits {
    fn eq(&self, o: &Self) -> bool {
        let n = self.0.len().max(o.0.len());
        (0..n).all(|i| self.word(i) == o.word(i))
    }
}
impl Eq for Bits {}

impl std::fmt::Debug for Bits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl std::fmt::Display for Bits {
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

pub struct BitsIter<'a> {
    words: &'a [u64],
    block: usize,
    bits: u64,
}
impl Iterator for BitsIter<'_> {
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
            if self.block >= self.words.len() {
                return None;
            }
            self.bits = self.words[self.block];
        }
    }
}
impl<'a> IntoIterator for &'a Bits {
    type Item = Index;
    type IntoIter = BitsIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

const INLINE: usize = 2;

/// Owned growable bitset; up to 128 values without allocating.
#[derive(Clone, Default)]
pub struct IntegerSet {
    /// Words in use; inline words past `n` are zero.
    n: u32,
    inline: [u64; INLINE],
    heap: Vec<u64>,
}

impl IntegerSet {
    #[inline(always)]
    pub const fn new() -> Self {
        IntegerSet { n: 0, inline: [0; INLINE], heap: Vec::new() }
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
        s.grow(blocks);
        let w = s.words_mut();
        w.fill(u64::MAX);
        let tail = n as usize % 64;
        if tail != 0 {
            w[blocks - 1] = (1u64 << tail) - 1;
        }
        s
    }
    /// A copy of a view.
    pub fn from_bits(b: &Bits) -> Self {
        let mut s = Self::new();
        s.grow(b.used_words());
        let n = s.n as usize;
        s.words_mut().copy_from_slice(&b.words()[..n]);
        s
    }
    #[inline(always)]
    fn grow(&mut self, nw: usize) {
        let n = self.n as usize;
        if nw <= n {
            return;
        }
        if nw > INLINE {
            if n <= INLINE {
                self.heap = self.inline[..n].to_vec();
            }
            self.heap.resize(nw, 0);
        }
        self.n = nw as u32;
    }
    #[inline(always)]
    pub fn insert(&mut self, v: Index) {
        self.grow((v >> 6) as usize + 1);
        Bits::insert(self, v);
    }
    /// Insert {0, ..., n-1}.
    pub fn insert_below(&mut self, n: Index) {
        let full = n as usize / 64;
        let tail = n as usize % 64;
        self.grow(full + (tail != 0) as usize);
        let w = self.words_mut();
        w[..full].fill(u64::MAX);
        if tail != 0 {
            w[full] |= (1u64 << tail) - 1;
        }
    }
    /// Union with `o` (any size).
    pub fn or_assign(&mut self, o: &Bits) {
        self.grow(o.used_words());
        Bits::or_assign(self, o);
    }
    pub fn and(&self, o: &Bits) -> IntegerSet {
        let mut r = self.clone();
        r.and_assign(o);
        r
    }
}

impl std::ops::Deref for IntegerSet {
    type Target = Bits;
    #[inline(always)]
    fn deref(&self) -> &Bits {
        let n = self.n as usize;
        Bits::from_words(if n <= INLINE { &self.inline[..n] } else { &self.heap[..] })
    }
}
impl std::ops::DerefMut for IntegerSet {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Bits {
        let n = self.n as usize;
        Bits::from_words_mut(if n <= INLINE { &mut self.inline[..n] } else { &mut self.heap[..] })
    }
}

impl PartialEq for IntegerSet {
    fn eq(&self, o: &Self) -> bool {
        **self == **o
    }
}
impl Eq for IntegerSet {}

impl std::fmt::Debug for IntegerSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&**self, f)
    }
}
impl std::fmt::Display for IntegerSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&**self, f)
    }
}

impl<'a> IntoIterator for &'a IntegerSet {
    type Item = Index;
    type IntoIter = BitsIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grow_and_ops() {
        let mut s = IntegerSet::new();
        for v in [3, 70, 129, 300] {
            s.insert(v);
        }
        assert_eq!(s.iter().collect::<Vec<_>>(), vec![3, 70, 129, 300]);
        assert_eq!(s.len(), 4);
        let r = IntegerSet::range(200);
        assert_eq!(r.len(), 200);
        assert!(s.intersects(&r));
        let mut t = s.clone();
        t.minus(&r);
        assert_eq!(t.iter().collect::<Vec<_>>(), vec![300]);
        assert_eq!(s.and(&r).iter().collect::<Vec<_>>(), vec![3, 70, 129]);
        assert_eq!(s.max(), Some(300));
        let mut u = IntegerSet::singleton(200);
        u.insert_below(130);
        assert_eq!(u.len(), 131);
        assert!(u.contains(129) && !u.contains(130) && u.contains(200));
        assert!(IntegerSet::singleton(5) == IntegerSet::from_bits(&IntegerSet::range(6).and(&IntegerSet::singleton(5))));
    }
}
