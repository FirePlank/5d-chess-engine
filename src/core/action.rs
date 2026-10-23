//! Moves and actions.

use super::piece::*;
use super::state::State;
use super::vec4::Vec4;
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct FullMove {
    pub from: Vec4,
    pub to: Vec4,
}

const SEQ_INLINE: usize = 3;

#[derive(Clone, Copy)]
#[repr(C)]
union SeqData {
    inline: [FullMove; SEQ_INLINE],
    heap: *mut FullMove,
}

/// The moves of an action, one per moved timeline. Up to three are stored inline; longer
/// actions spill to the heap. All-zero bytes are a valid empty sequence.
pub struct MoveSeq {
    len: u32,
    /// Heap capacity; 0 while the moves are inline.
    cap: u32,
    data: SeqData,
}

// SAFETY: the heap block is owned exclusively, like a `Vec`'s.
unsafe impl Send for MoveSeq {}
unsafe impl Sync for MoveSeq {}

impl MoveSeq {
    pub const fn new() -> Self {
        MoveSeq { len: 0, cap: 0, data: SeqData { inline: [FullMove { from: Vec4(0), to: Vec4(0) }; SEQ_INLINE] } }
    }

    /// Whether the moves live on the heap.
    #[inline(always)]
    pub fn spilled(&self) -> bool {
        self.cap != 0
    }

    #[inline(always)]
    pub fn push(&mut self, m: FullMove) {
        let n = self.len as usize;
        if self.cap == 0 {
            if n < SEQ_INLINE {
                // SAFETY: inline storage is active while `cap == 0`.
                unsafe { self.data.inline[n] = m };
                self.len += 1;
                return;
            }
            let mut v = Vec::with_capacity(2 * SEQ_INLINE + 2);
            // SAFETY: as above.
            v.extend_from_slice(unsafe { &self.data.inline });
            v.push(m);
            self.set_vec(v);
            return;
        }
        // SAFETY: `heap`, `len` and `cap` describe a live allocation made by a `Vec`.
        let mut v = unsafe { Vec::from_raw_parts(self.data.heap, n, self.cap as usize) };
        v.push(m);
        self.set_vec(v);
    }

    fn set_vec(&mut self, v: Vec<FullMove>) {
        let mut v = std::mem::ManuallyDrop::new(v);
        self.data.heap = v.as_mut_ptr();
        self.len = v.len() as u32;
        self.cap = v.capacity() as u32;
    }

    pub fn pop(&mut self) -> Option<FullMove> {
        if self.len == 0 {
            return None;
        }
        let m = self.as_slice()[self.len as usize - 1];
        self.len -= 1;
        Some(m)
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Replace the contents with `src` (keeping any heap storage).
    #[inline]
    pub fn assign(&mut self, src: &[FullMove]) {
        if self.cap == 0 && src.len() <= SEQ_INLINE {
            // SAFETY: inline storage is active while `cap == 0`.
            unsafe { self.data.inline[..src.len()].copy_from_slice(src) };
            self.len = src.len() as u32;
            return;
        }
        self.clear();
        for &m in src {
            self.push(m);
        }
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[FullMove] {
        // SAFETY: the active storage holds `len` initialized moves.
        unsafe {
            if self.cap == 0 {
                &self.data.inline[..self.len as usize]
            } else {
                std::slice::from_raw_parts(self.data.heap, self.len as usize)
            }
        }
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [FullMove] {
        // SAFETY: as above.
        unsafe {
            if self.cap == 0 {
                &mut self.data.inline[..self.len as usize]
            } else {
                std::slice::from_raw_parts_mut(self.data.heap, self.len as usize)
            }
        }
    }
}

impl Drop for MoveSeq {
    fn drop(&mut self) {
        if self.cap != 0 {
            // SAFETY: the block came from a `Vec` with this capacity.
            drop(unsafe { Vec::from_raw_parts(self.data.heap, self.len as usize, self.cap as usize) });
        }
    }
}

impl Default for MoveSeq {
    fn default() -> Self {
        MoveSeq::new()
    }
}

impl Clone for MoveSeq {
    fn clone(&self) -> Self {
        let mut s = MoveSeq::new();
        s.assign(self);
        s
    }
    fn clone_from(&mut self, src: &Self) {
        self.assign(src);
    }
}

impl std::ops::Deref for MoveSeq {
    type Target = [FullMove];
    #[inline(always)]
    fn deref(&self) -> &[FullMove] {
        self.as_slice()
    }
}

impl std::ops::DerefMut for MoveSeq {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [FullMove] {
        self.as_mut_slice()
    }
}

impl PartialEq for MoveSeq {
    #[inline]
    fn eq(&self, o: &Self) -> bool {
        self.as_slice() == o.as_slice()
    }
}
impl Eq for MoveSeq {}

impl std::hash::Hash for MoveSeq {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.as_slice().hash(h)
    }
}

impl fmt::Debug for MoveSeq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_slice(), f)
    }
}

impl FromIterator<FullMove> for MoveSeq {
    fn from_iter<I: IntoIterator<Item = FullMove>>(it: I) -> Self {
        let mut s = MoveSeq::new();
        for m in it {
            s.push(m);
        }
        s
    }
}

impl From<&[FullMove]> for MoveSeq {
    fn from(v: &[FullMove]) -> Self {
        v.iter().copied().collect()
    }
}

impl<'a> IntoIterator for &'a MoveSeq {
    type Item = &'a FullMove;
    type IntoIter = std::slice::Iter<'a, FullMove>;
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

pub mod special {
    pub const NONE: u8 = 0;
    pub const CAPTURE: u8 = 1;
    pub const EN_PASSANT: u8 = 1 << 1;
    pub const PROMOTION: u8 = 1 << 2;
    pub const CASTLE_KINGSIDE: u8 = 1 << 3;
    pub const CASTLE_QUEENSIDE: u8 = 1 << 4;
    pub const CASTLE: u8 = (1 << 3) | (1 << 4);
    pub const SUPERPHYSICAL: u8 = 1 << 5;
    pub const BRANCHING: u8 = 1 << 6;
}

fn parse_int(s: &[u8], i: &mut usize) -> Option<i32> {
    while *i < s.len() && s[*i].is_ascii_whitespace() {
        *i += 1;
    }
    let start = *i;
    if *i < s.len() && (s[*i] == b'-' || s[*i] == b'+') {
        *i += 1;
    }
    let ds = *i;
    while *i < s.len() && s[*i].is_ascii_digit() {
        *i += 1;
    }
    if *i == ds {
        return None;
    }
    std::str::from_utf8(&s[start..*i]).ok()?.parse().ok()
}

fn parse_lt(s: &[u8], i: &mut usize) -> Option<(i32, i32)> {
    if *i >= s.len() || s[*i] != b'(' {
        return None;
    }
    *i += 1;
    let l = parse_int(s, i)?;
    if *i >= s.len() || s[*i] != b'T' {
        return None;
    }
    *i += 1;
    let t = parse_int(s, i)?;
    if *i >= s.len() || s[*i] != b')' {
        return None;
    }
    *i += 1;
    Some((l, t))
}

fn valid_square(f: u8, r: u8) -> bool {
    (b'a'..=b'h').contains(&f) && (b'1'..=b'8').contains(&r)
}

impl FullMove {
    #[inline(always)]
    pub const fn new(from: Vec4, to: Vec4) -> Self {
        FullMove { from, to }
    }

    /// Parse long algebraic notation such as `(0T1)e2e4` or `(0T5)Qd1>>(0T4)d2`.
    pub fn parse(input: &str) -> Result<FullMove, String> {
        let mut s: Vec<u8> = input.as_bytes().to_vec();
        let first_close = s.iter().position(|&c| c == b')');
        if let Some(fc) = first_close
            && fc + 1 < s.len() && s[fc + 1].is_ascii_uppercase() {
                s.remove(fc + 1);
            }
        let search_from = first_close.map(|f| f + 1).unwrap_or(0);
        if let Some(rel) = s[search_from.min(s.len())..].iter().position(|&c| c == b'(') {
            let second_open = search_from + rel;
            let mut sep = second_open;
            while sep > 0 && s[sep - 1] == b'>' {
                sep -= 1;
            }
            s.drain(sep..second_open);
        }
        let err = || format!("Cannot match this move in any known pattern: {}", input);
        let mut i = 0;
        let (l1, t1) = parse_lt(&s, &mut i).ok_or_else(err)?;
        if i + 2 > s.len() {
            return Err(err());
        }
        let (fx, fy) = (s[i], s[i + 1]);
        i += 2;
        if i + 2 == s.len() && s[i] != b'(' {
            let (tx, ty) = (s[i], s[i + 1]);
            if !valid_square(fx, fy) || !valid_square(tx, ty) {
                return Err(err());
            }
            return Ok(FullMove::new(
                Vec4::new((fx - b'a') as i32, (fy - b'1') as i32, t1, l1),
                Vec4::new((tx - b'a') as i32, (ty - b'1') as i32, t1, l1),
            ));
        }
        let (l2, t2) = parse_lt(&s, &mut i).ok_or_else(err)?;
        if i + 2 != s.len() {
            return Err(err());
        }
        let (tx, ty) = (s[i], s[i + 1]);
        if !valid_square(fx, fy) || !valid_square(tx, ty) {
            return Err(err());
        }
        Ok(FullMove::new(
            Vec4::new((fx - b'a') as i32, (fy - b'1') as i32, t1, l1),
            Vec4::new((tx - b'a') as i32, (ty - b'1') as i32, t2, l2),
        ))
    }
}

impl fmt::Display for FullMove {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (p, q) = (self.from, self.to);
        let d = q - p;
        let sq = |v: Vec4| format!("{}{}", (v.x() as u8 + b'a') as char, (v.y() as u8 + b'1') as char);
        if d.t() == 0 && d.l() == 0 {
            write!(f, "({}T{}){}{}", p.l(), p.t(), sq(p), sq(q))
        } else {
            write!(f, "({}T{}){}({}T{}){}", p.l(), p.t(), sq(p), q.l(), q.t(), sq(q))
        }
    }
}
impl fmt::Debug for FullMove {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Debug)]
pub struct ExtMove {
    pub fm: FullMove,
    pub promote_to: Piece,
}

impl ExtMove {
    pub const fn new(fm: FullMove, promote_to: Piece) -> Self {
        ExtMove { fm, promote_to }
    }
    /// Parse LAN with an optional trailing promotion letter (left unnormalized).
    pub fn parse(s: &str) -> Result<ExtMove, String> {
        let last = s.as_bytes().last().copied();
        match last {
            Some(c) if c.is_ascii_uppercase() => Ok(ExtMove::new(FullMove::parse(&s[..s.len() - 1])?, c)),
            _ => Ok(ExtMove::new(FullMove::parse(s)?, NO_PIECE)),
        }
    }
    /// Resolve the configured promotion for `fm` in `s`.
    pub fn resolve(fm: FullMove, s: &State) -> ExtMove {
        s.normalize_promotion(ExtMove::new(fm, NO_PIECE))
            .expect("ext_move: cannot normalize promotion")
    }
}

impl fmt::Display for ExtMove {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.fm)?;
        if self.promote_to != NO_PIECE {
            write!(f, "{}", self.promote_to as char)?;
        }
        Ok(())
    }
}

/// A turn's moves in standard order: non-branching moves first, then branching moves in
/// creation order of their new timelines.
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct Action {
    mvs: Vec<ExtMove>,
    branching_index: i32,
}

impl Action {
    /// Sort moves into standard order; returns the branching index.
    pub fn sort(mvs: &mut [ExtMove], s: &State) -> i32 {
        let mut rbranching = 0usize;
        let player = s.get_present().1;
        let mut moved_lines: Vec<i32> = Vec::new();
        for i in 0..mvs.len() {
            let p = mvs[i].fm.from;
            let q = mvs[i].fm.to;
            let tc1 = (q.t(), player);
            let tc2 = s.get_timeline_end(q.l());
            let branching = tc1 < tc2 || (tc1 == tc2 && moved_lines.contains(&q.l()));
            if !moved_lines.contains(&p.l()) {
                moved_lines.push(p.l());
            }
            if branching {
                mvs.swap(i, rbranching);
                rbranching += 1;
            } else if !moved_lines.contains(&q.l()) {
                moved_lines.push(q.l());
            }
        }
        if rbranching < mvs.len() {
            let sign = if player { -1 } else { 1 };
            mvs[rbranching..].sort_by_key(|m| sign * m.fm.to.l());
            mvs.rotate_left(rbranching);
        }
        (mvs.len() - rbranching) as i32
    }

    pub fn from_vector(mvs: &[ExtMove], s: &State) -> Action {
        let mut v = mvs.to_vec();
        let bi = Self::sort(&mut v, s);
        for m in v.iter_mut() {
            *m = s
                .normalize_promotion(*m)
                .expect("action::from_vector(): Invalid promotion piece");
        }
        Action { mvs: v, branching_index: bi }
    }

    pub fn from_moveseq(mvs: &[FullMove], s: &State) -> Action {
        let ext: Vec<ExtMove> = mvs.iter().map(|&fm| ExtMove::resolve(fm, s)).collect();
        Self::from_vector(&ext, s)
    }

    pub fn moves(&self) -> &[ExtMove] {
        &self.mvs
    }
    pub fn len(&self) -> usize {
        self.mvs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.mvs.is_empty()
    }
    pub fn branching_index(&self) -> i32 {
        self.branching_index
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for m in &self.mvs {
            write!(f, "{} ", m)?;
        }
        Ok(())
    }
}
