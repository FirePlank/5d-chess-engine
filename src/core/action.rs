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

pub type MoveSeq = Vec<FullMove>;

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
        if let Some(fc) = first_close {
            if fc + 1 < s.len() && s[fc + 1].is_ascii_uppercase() {
                s.remove(fc + 1);
            }
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
