//! Packed 4D coordinate (x, y, t, l) in one u32 (x:4, y:4, t:12, l:12 from the low bits
//! up). Ordering compares the raw word; destination-board iteration order depends on it.

use std::fmt;

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Vec4(pub u32);

const X_BITS: u32 = 4;
const Y_BITS: u32 = 4;
const T_BITS: u32 = 12;
const L_BITS: u32 = 12;
const U_X: u32 = 1;
const U_Y: u32 = U_X << X_BITS;
const U_T: u32 = U_Y << Y_BITS;
const U_L: u32 = U_T << T_BITS;
const L_MASK: u32 = 0u32.wrapping_sub(U_L);
const T_MASK: u32 = U_L - U_T;
const Y_MASK: u32 = U_T - U_Y;
const X_MASK: u32 = U_Y - U_X;
const MASK_TOP: u32 =
    (U_X << (X_BITS - 1)) | (U_Y << (Y_BITS - 1)) | (U_T << (T_BITS - 1)) | (U_L << (L_BITS - 1));
const MASK_LOWER: u32 = !MASK_TOP;
const TL_MASK: u32 = L_MASK | T_MASK;
const ONES: u32 = U_X | U_Y | U_T | U_L;

impl Vec4 {
    pub const T_MIN: i32 = -(1 << (T_BITS - 1));
    pub const T_MAX: i32 = (1 << (T_BITS - 1)) - 1;
    pub const L_MIN: i32 = -(1 << (L_BITS - 1));
    pub const L_MAX: i32 = (1 << (L_BITS - 1)) - 1;
    pub const ZERO: Vec4 = Vec4(0);

    #[inline(always)]
    pub const fn new(x: i32, y: i32, t: i32, l: i32) -> Vec4 {
        Vec4((((l as u32) << (T_BITS + Y_BITS + X_BITS)) & L_MASK)
            | (((t as u32) << (Y_BITS + X_BITS)) & T_MASK)
            | (((y as u32) << X_BITS) & Y_MASK)
            | ((x as u32) & X_MASK))
    }

    /// Square index `xy` (0..64) placed on the board `tl`.
    #[inline(always)]
    pub const fn from_xy(xy: i32, tl: Vec4) -> Vec4 {
        Vec4((tl.0 & TL_MASK) | (((xy & 0b111000) as u32) << 1) | ((xy & 0b111) as u32))
    }

    #[inline(always)]
    pub const fn tl_of(t: i32, l: i32) -> Vec4 {
        Vec4::new(0, 0, t, l)
    }

    #[inline(always)]
    pub const fn l(self) -> i32 {
        ((self.0 as i32) >> (T_BITS + Y_BITS + X_BITS)) as i32
    }
    #[inline(always)]
    pub const fn t(self) -> i32 {
        (((self.0 << L_BITS) as i32) >> (L_BITS + Y_BITS + X_BITS)) as i32
    }
    #[inline(always)]
    pub const fn y(self) -> i32 {
        (((self.0 << (L_BITS + T_BITS)) as i32) >> (L_BITS + T_BITS + X_BITS)) as i32
    }
    #[inline(always)]
    pub const fn x(self) -> i32 {
        (((self.0 << (32 - X_BITS)) as i32) >> (32 - X_BITS)) as i32
    }

    #[inline(always)]
    pub const fn add(self, v: Vec4) -> Vec4 {
        let a = self.0;
        let b = v.0;
        Vec4(((a & MASK_LOWER).wrapping_add(b & MASK_LOWER)) ^ ((a ^ b) & MASK_TOP))
    }
    #[inline(always)]
    pub const fn neg(self) -> Vec4 {
        Vec4(!self.0).add(Vec4(ONES))
    }
    #[inline(always)]
    pub const fn sub(self, v: Vec4) -> Vec4 {
        self.add(v.neg())
    }
    #[inline(always)]
    pub const fn mul(self, k: i32) -> Vec4 {
        Vec4::new(k * self.x(), k * self.y(), k * self.t(), k * self.l())
    }
    #[inline(always)]
    pub const fn dot(self, o: Vec4) -> i32 {
        self.x() * o.x() + self.y() * o.y() + self.t() * o.t() + self.l() * o.l()
    }
    /// True when x or y lies outside 0..8.
    #[inline(always)]
    pub const fn outbound(self) -> bool {
        (!Vec4::new(7, 7, -1, -1).0 & self.0) != 0
    }
    #[inline(always)]
    pub const fn xy(self) -> i32 {
        ((self.0 & 0b111) | ((self.0 >> 1) & 0b111000)) as i32
    }
    #[inline(always)]
    pub const fn tl(self) -> Vec4 {
        Vec4(self.0 & TL_MASK)
    }
}

impl std::ops::Add for Vec4 {
    type Output = Vec4;
    #[inline(always)]
    fn add(self, o: Vec4) -> Vec4 {
        Vec4::add(self, o)
    }
}
impl std::ops::Sub for Vec4 {
    type Output = Vec4;
    #[inline(always)]
    fn sub(self, o: Vec4) -> Vec4 {
        Vec4::sub(self, o)
    }
}
impl std::ops::Neg for Vec4 {
    type Output = Vec4;
    #[inline(always)]
    fn neg(self) -> Vec4 {
        Vec4::neg(self)
    }
}
impl std::ops::Mul<i32> for Vec4 {
    type Output = Vec4;
    #[inline(always)]
    fn mul(self, k: i32) -> Vec4 {
        Vec4::mul(self, k)
    }
}

impl fmt::Debug for Vec4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:08x}({},{},{},{})", self.0, self.x(), self.y(), self.t(), self.l())
    }
}
impl fmt::Display for Vec4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        for l in [-2048, -5, -1, 0, 1, 7, 2047] {
            for t in [-3, 0, 1, 100, 2047] {
                for y in 0..8 {
                    for x in 0..8 {
                        let v = Vec4::new(x, y, t, l);
                        assert_eq!((v.x(), v.y(), v.t(), v.l()), (x, y, t, l));
                        assert!(!v.outbound());
                        assert_eq!(v.xy(), x + 8 * y);
                        assert_eq!(Vec4::from_xy(v.xy(), v.tl()), v);
                    }
                }
            }
        }
        let a = Vec4::new(3, 4, 5, -2);
        let b = Vec4::new(-1, 2, -7, 3);
        let s = a + b;
        assert_eq!((s.x(), s.y(), s.t(), s.l()), (2, 6, -2, 1));
        let d = a - b;
        assert_eq!((d.x(), d.y(), d.t(), d.l()), (4, 2, 12, -5));
        assert!(Vec4::new(8, 0, 0, 0).outbound());
        assert!(Vec4::new(0, -1, 0, 0).outbound());
    }
}
