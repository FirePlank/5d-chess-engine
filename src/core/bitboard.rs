//! 8x8 bitboards (bit = x + 8*y) and attack tables.
//!
//! Square iteration is MSB-first; move and candidate ordering throughout the engine
//! depends on it.

pub type Bitboard = u64;

pub const A_FILE: Bitboard = 0x0101010101010101;
pub const H_FILE: Bitboard = 0x8080808080808080;

#[inline(always)]
pub const fn pmask(pos: i32) -> Bitboard {
    1u64 << pos
}
#[inline(always)]
pub const fn shift_north(b: Bitboard) -> Bitboard {
    b << 8
}
#[inline(always)]
pub const fn shift_south(b: Bitboard) -> Bitboard {
    b >> 8
}
#[inline(always)]
pub const fn shift_west(b: Bitboard) -> Bitboard {
    (b & !A_FILE) >> 1
}
#[inline(always)]
pub const fn shift_east(b: Bitboard) -> Bitboard {
    (b & !H_FILE) << 1
}
#[inline(always)]
pub const fn shift_northwest(b: Bitboard) -> Bitboard {
    (b & !A_FILE) << 7
}
#[inline(always)]
pub const fn shift_northeast(b: Bitboard) -> Bitboard {
    (b & !H_FILE) << 9
}
#[inline(always)]
pub const fn shift_southwest(b: Bitboard) -> Bitboard {
    (b & !A_FILE) >> 9
}
#[inline(always)]
pub const fn shift_southeast(b: Bitboard) -> Bitboard {
    (b & !H_FILE) >> 7
}

/// Index of the most significant set bit.
#[inline(always)]
pub const fn bb_get_pos(b: Bitboard) -> i32 {
    63 - b.leading_zeros() as i32
}

/// Iterates set squares from the most significant bit down.
#[derive(Clone, Copy)]
pub struct MarkedPos(pub Bitboard);
impl Iterator for MarkedPos {
    type Item = i32;
    #[inline(always)]
    fn next(&mut self) -> Option<i32> {
        if self.0 == 0 {
            None
        } else {
            let p = bb_get_pos(self.0);
            self.0 ^= 1u64 << p;
            Some(p)
        }
    }
}
#[inline(always)]
pub const fn marked_pos(b: Bitboard) -> MarkedPos {
    MarkedPos(b)
}

#[inline(always)]
pub const fn white_pawn_attack(pos: i32) -> Bitboard {
    let z = 1u64 << pos;
    shift_northwest(z) | shift_northeast(z)
}
#[inline(always)]
pub const fn black_pawn_attack(pos: i32) -> Bitboard {
    let z = 1u64 << pos;
    shift_southwest(z) | shift_southeast(z)
}

const fn gen_knight() -> [Bitboard; 64] {
    let mut t = [0u64; 64];
    let mut pos = 0;
    while pos < 64 {
        let z = 1u64 << pos;
        let mut bb = 0;
        bb |= shift_north(shift_northwest(z));
        bb |= shift_west(shift_northwest(z));
        bb |= shift_north(shift_northeast(z));
        bb |= shift_east(shift_northeast(z));
        bb |= shift_south(shift_southwest(z));
        bb |= shift_west(shift_southwest(z));
        bb |= shift_south(shift_southeast(z));
        bb |= shift_east(shift_southeast(z));
        t[pos] = bb;
        pos += 1;
    }
    t
}
const fn gen_king() -> [Bitboard; 64] {
    let mut t = [0u64; 64];
    let mut pos = 0;
    while pos < 64 {
        let z = 1u64 << pos;
        t[pos] = shift_north(z)
            | shift_south(z)
            | shift_west(z)
            | shift_east(z)
            | shift_northwest(z)
            | shift_northeast(z)
            | shift_southwest(z)
            | shift_southeast(z);
        pos += 1;
    }
    t
}
const fn gen_copy(diag: bool, orth: bool) -> [Bitboard; 512] {
    let mut t = [0u64; 512];
    let mut key = 0;
    while key < 512 {
        let pos = key & 63;
        let n = key >> 6;
        let (mut a, mut b, mut c, mut d) = (1u64 << pos, 1u64 << pos, 1u64 << pos, 1u64 << pos);
        let (mut e, mut f, mut g, mut h) = (a, a, a, a);
        let mut i = 0;
        while i < n {
            a = shift_north(a);
            b = shift_south(b);
            c = shift_west(c);
            d = shift_east(d);
            e = shift_northwest(e);
            f = shift_northeast(f);
            g = shift_southwest(g);
            h = shift_southeast(h);
            i += 1;
        }
        let mut bb = 0;
        if orth {
            bb |= a | b | c | d;
        }
        if diag {
            bb |= e | f | g | h;
        }
        t[key] = bb;
        key += 1;
    }
    t
}

pub static KNIGHT_ATTACK: [Bitboard; 64] = gen_knight();
pub static KING_ATTACK: [Bitboard; 64] = gen_king();
pub static ROOK_COPY: [Bitboard; 512] = gen_copy(false, true);
pub static BISHOP_COPY: [Bitboard; 512] = gen_copy(true, false);
pub static QUEEN_COPY: [Bitboard; 512] = gen_copy(true, true);

const fn gen_king_jump() -> [Bitboard; 64] {
    let k = gen_king();
    let mut t = [0u64; 64];
    let mut i = 0;
    while i < 64 {
        t[i] = k[i] | (1u64 << i);
        i += 1;
    }
    t
}
const fn gen_knight_jump(n: usize) -> [Bitboard; 64] {
    let c = gen_copy(false, true);
    let mut t = [0u64; 64];
    let mut i = 0;
    while i < 64 {
        t[i] = c[(n << 6) | i];
        i += 1;
    }
    t
}
pub static KING_JUMP_ATTACK: [Bitboard; 64] = gen_king_jump();
pub static KNIGHT_JUMP1_ATTACK: [Bitboard; 64] = gen_knight_jump(2);
pub static KNIGHT_JUMP2_ATTACK: [Bitboard; 64] = gen_knight_jump(1);

#[inline(always)]
pub fn knight_attack(pos: i32) -> Bitboard {
    KNIGHT_ATTACK[pos as usize]
}
#[inline(always)]
pub fn king_attack(pos: i32) -> Bitboard {
    KING_ATTACK[pos as usize]
}
#[inline(always)]
pub fn rook_copy_mask(pos: i32, n: i32) -> Bitboard {
    ROOK_COPY[((n << 6) | pos) as usize]
}
#[inline(always)]
pub fn bishop_copy_mask(pos: i32, n: i32) -> Bitboard {
    BISHOP_COPY[((n << 6) | pos) as usize]
}
#[inline(always)]
pub fn queen_copy_mask(pos: i32, n: i32) -> Bitboard {
    QUEEN_COPY[((n << 6) | pos) as usize]
}
#[inline(always)]
pub fn king_jump_attack(pos: i32) -> Bitboard {
    KING_JUMP_ATTACK[pos as usize]
}
#[inline(always)]
pub fn knight_jump1_attack(pos: i32) -> Bitboard {
    KNIGHT_JUMP1_ATTACK[pos as usize]
}
#[inline(always)]
pub fn knight_jump2_attack(pos: i32) -> Bitboard {
    KNIGHT_JUMP2_ATTACK[pos as usize]
}

const fn gen_between() -> [[Bitboard; 64]; 64] {
    let mut t = [[0u64; 64]; 64];
    let mut a = 0;
    while a < 64 {
        let (ax, ay) = ((a & 7) as i32, (a >> 3) as i32);
        let mut b = 0;
        while b < 64 {
            let (bx, by) = ((b & 7) as i32, (b >> 3) as i32);
            let (dx, dy) = (bx - ax, by - ay);
            let aligned = (dx == 0 || dy == 0 || dx == dy || dx == -dy) && a != b;
            if aligned {
                let sx = if dx > 0 { 1 } else if dx < 0 { -1 } else { 0 };
                let sy = if dy > 0 { 1 } else if dy < 0 { -1 } else { 0 };
                let (mut x, mut y) = (ax + sx, ay + sy);
                let mut bb = 0u64;
                while x != bx || y != by {
                    bb |= 1u64 << (x + 8 * y);
                    x += sx;
                    y += sy;
                }
                t[a][b] = bb;
            }
            b += 1;
        }
        a += 1;
    }
    t
}
/// Squares strictly between two aligned squares (empty otherwise).
pub static BETWEEN: [[Bitboard; 64]; 64] = gen_between();

// ---- Sliding attacks (PEXT-indexed tables generated by build.rs) ----

#[repr(C)]
struct AlignedAs<A, B: ?Sized> {
    _align: [A; 0],
    bytes: B,
}
static RAW: &AlignedAs<u64, [u8]> = &AlignedAs {
    _align: [],
    bytes: *include_bytes!(concat!(env!("OUT_DIR"), "/sliders.bin")),
};

#[inline(always)]
fn words() -> &'static [u64] {
    // SAFETY: the blob is u64-aligned by `AlignedAs` and holds little-endian u64 words.
    unsafe { std::slice::from_raw_parts(RAW.bytes.as_ptr() as *const u64, RAW.bytes.len() / 8) }
}

#[inline(always)]
fn pext(src: u64, mask: u64) -> u64 {
    #[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
    unsafe {
        std::arch::x86_64::_pext_u64(src, mask)
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "bmi2")))]
    {
        let mut out = 0u64;
        let mut m = mask;
        let mut bit = 1u64;
        while m != 0 {
            let low = m & m.wrapping_neg();
            if src & low != 0 {
                out |= bit;
            }
            bit <<= 1;
            m &= m - 1;
        }
        out
    }
}

#[inline(always)]
pub fn rook_attack(pos: i32, blocker: Bitboard) -> Bitboard {
    let w = words();
    let p = pos as usize;
    let mask = w[p];
    let off = w[64 + p] as usize;
    w[256 + off + pext(blocker, mask) as usize]
}
#[inline(always)]
pub fn bishop_attack(pos: i32, blocker: Bitboard) -> Bitboard {
    let w = words();
    let p = pos as usize;
    let mask = w[128 + p];
    let off = w[192 + p] as usize;
    w[256 + off + pext(blocker, mask) as usize]
}
#[inline(always)]
pub fn queen_attack(pos: i32, blocker: Bitboard) -> Bitboard {
    rook_attack(pos, blocker) | bishop_attack(pos, blocker)
}

pub fn bb_to_string(bb: Bitboard) -> String {
    let mut s = format!("0x{:016x}\n", bb);
    for y in (0..8).rev() {
        for x in 0..8 {
            s.push_str(if bb & (1u64 << (x + 8 * y)) != 0 { "1 " } else { ". " });
        }
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    fn slow(sq: i32, occ: u64, dirs: &[(i32, i32)]) -> u64 {
        let (x0, y0) = (sq & 7, sq >> 3);
        let mut out = 0;
        for &(dx, dy) in dirs {
            let (mut x, mut y) = (x0 + dx, y0 + dy);
            while (0..8).contains(&x) && (0..8).contains(&y) {
                let b = 1u64 << (x + 8 * y);
                out |= b;
                if occ & b != 0 {
                    break;
                }
                x += dx;
                y += dy;
            }
        }
        out
    }
    #[test]
    fn sliders_match_rays() {
        let mut s = 0x9E3779B97F4A7C15u64;
        for _ in 0..20000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let occ = s & (s >> 3);
            let sq = (s >> 58) as i32;
            assert_eq!(rook_attack(sq, occ), slow(sq, occ, &[(1, 0), (-1, 0), (0, 1), (0, -1)]));
            assert_eq!(bishop_attack(sq, occ), slow(sq, occ, &[(1, 1), (1, -1), (-1, 1), (-1, -1)]));
        }
    }
}
