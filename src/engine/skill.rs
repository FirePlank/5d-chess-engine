//! Strength handicaps sample exact MultiPV scores with decreasing noise.
//! The ladder targets even rating steps; actual Elo needs matches at a fixed time control.

pub(super) fn pick(scores: &[i32], level: u8, mut seed: u64) -> Option<usize> {
    let temperature = [0.80, 0.108, 0.069, 0.0438, 0.0276, 0.014, 0.003][usize::from(level.clamp(1, 7) - 1)];
    let mut best = None;
    let mut value = f64::NEG_INFINITY;
    for (k, &score) in scores.iter().enumerate() {
        seed = seed.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^= z >> 31;
        let u = ((z >> 11) as f64 + 0.5) / ((1u64 << 53) as f64);
        let win = 1.0 / (1.0 + (-super::WDL_K * f64::from(score)).exp());
        let noisy = win - temperature * (-u.ln()).ln();
        if noisy > value {
            value = noisy;
            best = Some(k);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    #[test]
    fn stronger_levels_lose_less_on_average() {
        // Close alternatives keep the strong levels from saturating at the best move.
        let scores = [6000, 5900, 5600, 5000, 3000, 0, -6000];
        let mut previous = i64::MIN;
        for level in 1..=7 {
            let sum: i64 = (0..10000).map(|seed| i64::from(scores[super::pick(&scores, level, seed).unwrap()])).sum();
            assert!(sum > previous, "level {level}: {sum} <= {previous}");
            previous = sum;
        }
        assert_eq!(super::pick(&[], 1, 0), None);
        assert_eq!(super::pick(&[123], 1, 0), Some(0));
    }
}
