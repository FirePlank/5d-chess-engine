//! MT19937 plus libc++-compatible `uniform_int_distribution`, `shuffle`,
//! `generate_canonical` and `discrete_distribution`, so seeded runs are reproducible.

#[derive(Clone)]
pub struct Mt19937 {
    mt: [u32; 624],
    idx: usize,
}

impl Mt19937 {
    pub const DEFAULT_SEED: u32 = 5489;

    pub fn new(seed: u32) -> Self {
        let mut mt = [0u32; 624];
        mt[0] = seed;
        for i in 1..624 {
            mt[i] = 1812433253u32.wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_add(i as u32);
        }
        Mt19937 { mt, idx: 624 }
    }

    /// Seeded from OS entropy (stand-in for `std::random_device`).
    pub fn from_entropy() -> Self {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};
        let mut h = RandomState::new().build_hasher();
        h.write_u64(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0));
        Self::new(h.finish() as u32)
    }

    fn twist(&mut self) {
        for i in 0..624 {
            let y = (self.mt[i] & 0x8000_0000) | (self.mt[(i + 1) % 624] & 0x7fff_ffff);
            let mut v = self.mt[(i + 397) % 624] ^ (y >> 1);
            if y & 1 != 0 {
                v ^= 0x9908_b0df;
            }
            self.mt[i] = v;
        }
        self.idx = 0;
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        if self.idx >= 624 {
            self.twist();
        }
        let mut y = self.mt[self.idx];
        self.idx += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// libc++ `uniform_int_distribution<long long>(a, b)(g)`.
    #[inline]
    pub fn uniform_i64(&mut self, a: i64, b: i64) -> i64 {
        let rp = (b as u64).wrapping_sub(a as u64).wrapping_add(1);
        if rp == 1 {
            return a;
        }
        if rp != 0 && rp <= 1u64 << 32 {
            // independent_bits with w <= 32 draws one engine word and keeps its low w bits.
            let mut w = 64 - rp.leading_zeros() as usize - 1;
            if rp & (u64::MAX >> (64 - w)) != 0 {
                w += 1;
            }
            let mask = if w >= 32 { u32::MAX as u64 } else { (1u64 << w) - 1 };
            loop {
                let u = self.next_u32() as u64 & mask;
                if u < rp {
                    return (u.wrapping_add(a as u64)) as i64;
                }
            }
        }
        let dt = 64usize;
        if rp == 0 {
            return self.independent_bits(dt) as i64;
        }
        let mut w = dt - rp.leading_zeros() as usize - 1;
        if rp & (u64::MAX >> (dt - w)) != 0 {
            w += 1;
        }
        loop {
            let u = self.independent_bits(w);
            if u < rp {
                return (u.wrapping_add(a as u64)) as i64;
            }
        }
    }

    /// libc++ `__independent_bits_engine<mt19937, uint64_t>(g, w)()`.
    fn independent_bits(&mut self, w: usize) -> u64 {
        const WDT: usize = 64; // working type digits
        const EDT: usize = 32; // engine result digits
        let rp: u64 = 1u64 << 32; // engine range
        let m = 32usize; // log2(rp)
        let mut n = w / m + (w % m != 0) as usize;
        let mut w0 = w / n;
        let mut y0 = if w0 < WDT { (rp >> w0) << w0 } else { 0 };
        if rp - y0 > y0 / n as u64 {
            n += 1;
            w0 = w / n;
            y0 = if w0 < WDT { (rp >> w0) << w0 } else { 0 };
        }
        let n0 = n - w % n;
        let y1 = if w0 < WDT - 1 { (rp >> (w0 + 1)) << (w0 + 1) } else { 0 };
        let mask0: u32 = if w0 > 0 { u32::MAX >> (EDT - w0) } else { 0 };
        let mask1: u32 = if w0 < EDT - 1 { u32::MAX >> (EDT - (w0 + 1)) } else { u32::MAX };
        let mut sp: u64 = 0;
        for _ in 0..n0 {
            let mut u;
            loop {
                u = self.next_u32() as u64;
                if u < y0 {
                    break;
                }
            }
            sp = if w0 < WDT { sp << w0 } else { 0 };
            sp = sp.wrapping_add(u & mask0 as u64);
        }
        for _ in n0..n {
            let mut u;
            loop {
                u = self.next_u32() as u64;
                if u < y1 {
                    break;
                }
            }
            sp = if w0 < WDT - 1 { sp << (w0 + 1) } else { 0 };
            sp = sp.wrapping_add(u & mask1 as u64);
        }
        sp
    }

    /// libc++ `std::shuffle(first, last, g)`.
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        let mut d = v.len() as i64;
        if d > 1 {
            let mut first = 0usize;
            let last = v.len() - 1;
            d -= 1;
            while first < last {
                let i = self.uniform_i64(0, d);
                if i != 0 {
                    v.swap(first, first + i as usize);
                }
                first += 1;
                d -= 1;
            }
        }
    }

    /// libc++ `generate_canonical<double, 53>(g)`.
    pub fn generate_canonical(&mut self) -> f64 {
        let rp = 4294967296.0f64;
        let mut base = rp;
        let mut sp = self.next_u32() as f64;
        sp += self.next_u32() as f64 * base;
        base *= rp;
        sp / base
    }

    /// libc++ `std::discrete_distribution<size_t>(weights)(g)`.
    pub fn discrete(&mut self, weights: &[f32]) -> usize {
        let cdf = discrete_cdf(weights);
        self.discrete_with_cdf(&cdf)
    }

    pub fn discrete_with_cdf(&mut self, cdf: &[f64]) -> usize {
        let x = self.generate_canonical();
        cdf.partition_point(|&p| p <= x)
    }
}

/// Cumulative table as built by libc++'s `discrete_distribution::param_type::__init`.
pub fn discrete_cdf(weights: &[f32]) -> Vec<f64> {
    let mut p: Vec<f64> = weights.iter().map(|&w| w as f64).collect();
    if p.len() <= 1 {
        return Vec::new();
    }
    let s: f64 = p.iter().fold(0.0, |a, &b| a + b);
    for x in p.iter_mut() {
        *x /= s;
    }
    let mut t = Vec::with_capacity(p.len() - 1);
    let mut acc = 0.0;
    for &x in &p[..p.len() - 1] {
        acc += x;
        t.push(acc);
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mt_reference_value() {
        let mut g = Mt19937::new(5489);
        let mut last = 0;
        for _ in 0..10000 {
            last = g.next_u32();
        }
        assert_eq!(last, 4123659995);
    }
}
