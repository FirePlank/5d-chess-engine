//! Two strength axes: board-local chess competence and multiverse understanding.
//! The levels are a relative ladder, not calibrated standard-chess or 5D Elo ratings.

use super::hce::{self, fi, Features, Weights};
use crate::core::action::MoveSeq;
use crate::core::piece::*;
use crate::core::state::State;

pub(super) fn material_value(piece: Piece) -> i32 {
    match to_white(piece_name(piece)) {
        PAWN_W => 100, BRAWN_W => 150, KNIGHT_W => 320, BISHOP_W => 330,
        ROOK_W => 500, QUEEN_W | ROYAL_QUEEN_W => 900, PRINCESS_W => 650,
        UNICORN_W => 250, DRAGON_W => 150, COMMON_KING_W => 300, KING_W => 20_000,
        _ => 0,
    }
}

/// Only ordinary captures count here: overlooking a time-travel attack remains possible.
/// Capture search includes recaptures and pins, so equal exchanges are not mistaken for hangs.
pub(super) fn material_risk(s: &State, mover: bool, won: i32) -> i32 {
    let mut values = [0; 128];
    for (p, value) in values.iter_mut().enumerate() { *value = material_value(p as Piece); }
    let (lo, hi) = s.get_active_range();
    let mut loss = 0;
    for l in lo..=hi {
        let (t, c) = s.get_timeline_end(l);
        if c != mover {
            loss += super::local::capture_gain(s.board(l, t, c), c, s.get_board_size().1, &values);
        }
    }
    (loss - won).max(0)
}

#[derive(Clone, Copy)]
pub(super) struct Config {
    pub depth: u32,
    pub nodes: u64,
    pub candidates: usize,
    /// Percentage of the full evaluation's non-spatial component understood.
    pub understanding: i32,
    /// Extra plies lost when reasoning through a move across time or timelines.
    pub travel_reduction: i32,
    /// Sampling is in internal evaluation units, without winning-chance saturation.
    pub temperature: f64,
    /// Extra immediate material loss allowed relative to the safest candidate, in ordinary cp.
    pub material_slack: i32,
    /// Maximum evaluation loss relative to the best tactically plausible candidate.
    pub max_loss: i32,
}

const CONFIGS: [Config; 7] = [
    Config { depth: 3, nodes: 6_000, candidates: 8, understanding: 8, travel_reduction: 2, temperature: 140.0, material_slack: 100, max_loss: 600 },
    Config { depth: 4, nodes: 12_000, candidates: 8, understanding: 16, travel_reduction: 2, temperature: 110.0, material_slack: 85, max_loss: 480 },
    Config { depth: 5, nodes: 24_000, candidates: 8, understanding: 27, travel_reduction: 2, temperature: 85.0, material_slack: 70, max_loss: 380 },
    Config { depth: 6, nodes: 48_000, candidates: 8, understanding: 35, travel_reduction: 2, temperature: 65.0, material_slack: 55, max_loss: 290 },
    Config { depth: 7, nodes: 96_000, candidates: 8, understanding: 55, travel_reduction: 1, temperature: 45.0, material_slack: 40, max_loss: 210 },
    Config { depth: 9, nodes: 243_000, candidates: 6, understanding: 72, travel_reduction: 1, temperature: 28.0, material_slack: 25, max_loss: 140 },
    Config { depth: 12, nodes: 729_000, candidates: 4, understanding: 100, travel_reduction: 0, temperature: 12.0, material_slack: 10, max_loss: 70 },
];

pub(super) fn config(level: u8) -> Config {
    CONFIGS[usize::from(level.clamp(1, 7) - 1)]
}

pub(super) fn travel_reduction(level: u8, action: &MoveSeq) -> i32 {
    if level == 8 || action.iter().all(|m| m.from.tl() == m.to.tl()) {
        0
    } else {
        config(level).travel_reduction
    }
}

/// Retain material, development, pawn structure, mobility, threats and local king safety.
/// Timeline resources, travel threats, history and the learned residual belong to the 5D axis.
pub(super) fn spatial_score(f: &Features, weights: &Weights, black: bool) -> i32 {
    let mut local = [0; hce::NF];
    for range in [fi::MAT_A..fi::MAT_A + 10, fi::CENT..fi::TL_SLOTS,
        fi::K_HOLES..fi::ACTIVE_LINES, fi::PASSED..fi::KING_PROX_5D,
        fi::QUEEN_PROX..fi::DANGER_5D, fi::KS..fi::TL,
        fi::DEF..fi::DEF + 2, fi::CH..fi::FADE] {
        local[range.clone()].copy_from_slice(&f[range]);
    }
    local[fi::ACTIVE_LINES] = f[fi::ACTIVE_LINES];
    hce::score(&local, weights, black)
}

pub(super) fn blend(local: i32, full: i32, level: u8) -> i32 {
    if level == 8 { return full; }
    local + ((i64::from(full) - i64::from(local)) * i64::from(config(level).understanding) / 100) as i32
}

/// Sample plausible mistakes, never using a saturated win probability to excuse a giveaway.
/// `risks` includes a board-local capture search with recaptures, minus material just won.
pub(super) fn pick(scores: &[i32], risks: &[i32], level: u8, mut seed: u64) -> Option<usize> {
    if scores.is_empty() || risks.len() != scores.len() { return None; }
    let cfg = config(level);
    let best = (0..scores.len()).max_by_key(|&k| scores[k])?;
    // Do not veto a calculated winning sacrifice, or choose a known mate to save a piece.
    let mate = super::ab::MATE - 1000;
    if scores[best].abs() >= mate { return Some(best); }
    let viable = |k: usize| scores[k] > -mate;
    let safest = (0..scores.len()).filter(|&k| viable(k)).map(|k| risks[k]).min()?;
    let safe_score = (0..scores.len()).filter(|&k| viable(k) && risks[k] <= safest.saturating_add(cfg.material_slack)).map(|k| scores[k]).max()?;
    let plausible = |k: usize| {
        // A substantially better searched result can justify a sacrifice. Beginners trust
        // such 5D compensation less; close scores cannot excuse a hanging piece. Eight
        // internal units per ordinary cp approximates the board evaluation's material scale.
        let compensation = (i64::from(scores[k]) - i64::from(safe_score)).max(0) * i64::from(cfg.understanding) / 800;
        viable(k) && i64::from(risks[k]) <= i64::from(safest) + i64::from(cfg.material_slack) + compensation
    };
    let best_score = scores.iter().enumerate().filter(|(k, _)| plausible(*k)).map(|(_, s)| *s).max()?;
    let mut best = None;
    let mut value = f64::NEG_INFINITY;
    for (k, &score) in scores.iter().enumerate() {
        if !plausible(k) || best_score.saturating_sub(score) > cfg.max_loss { continue; }
        seed = seed.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^= z >> 31;
        let u = ((z >> 11) as f64 + 0.5) / ((1u64 << 53) as f64);
        let noisy = f64::from(score) - cfg.temperature * (-u.ln()).ln();
        if noisy > value { value = noisy; best = Some(k); }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_skills_increase_without_weakening_local_terms() {
        for pair in CONFIGS.windows(2) {
            assert!(pair[0].depth < pair[1].depth);
            assert!(pair[0].nodes < pair[1].nodes);
            assert!(pair[0].understanding < pair[1].understanding);
            assert!(pair[0].travel_reduction >= pair[1].travel_reduction);
        }
        let mut f = [0; hce::NF];
        let mut w = [0.0; hce::NF];
        f[fi::PASSED] = 10; w[fi::PASSED] = 20.0;
        f[fi::TL_SLOTS] = 2; w[fi::TL_SLOTS] = 1000.0;
        f[fi::TRAVEL_STM] = 3; w[fi::TRAVEL_STM] = 1000.0;
        let local = spatial_score(&f, &w, false);
        assert_eq!(local, 200);
        for level in 1..=7 {
            assert_eq!(blend(local, local, level), local);
            assert_eq!(spatial_score(&f, &w, true), -local);
        }
        assert_eq!(blend(local, 5200, 1), 600);
        assert_eq!(blend(local, 5200, 8), 5200);
    }

    #[test]
    fn does_not_give_away_material_even_when_winning_or_losing() {
        for offset in [-20_000, 0, 20_000] {
            for level in 1..=7 {
                for seed in 0..1000 {
                    // A favourable 5D evaluation cannot excuse an obvious local queen hang.
                    assert_eq!(pick(&[offset + 500, offset], &[900, 0], level, seed), Some(1));
                    // A forced bad position still has a legal choice; avoid an even worse loss.
                    assert_eq!(pick(&[offset, offset - 20], &[300, 900], level, seed), Some(0));
                }
            }
        }
    }

    #[test]
    fn stronger_levels_make_smaller_plausible_errors() {
        let scores = [6000, 5980, 5950, 5900, 5800, 5600, 5000];
        let mut previous = i64::MIN;
        for level in 1..=7 {
            let sum: i64 = (0..10000).map(|seed| i64::from(scores[pick(&scores, &[0; 7], level, seed).unwrap()])).sum();
            assert!(sum > previous, "level {level}: {sum} <= {previous}");
            previous = sum;
        }
        assert_eq!(pick(&[], &[], 1, 0), None);
        assert_eq!(pick(&[123], &[0], 1, 0), Some(0));
    }

    #[test]
    fn local_safety_understands_recaptures_and_compensation() {
        let state = |fen: &str| crate::core::variants::position_from_uci(&format!("size 8x8 odd fen [{fen}:0:1:b]")).unwrap();
        let hanging = state("4r1k1/8/8/8/4Q3/8/8/6K1");
        assert_eq!(material_risk(&hanging, false, 0), 900);
        assert_eq!(material_risk(&hanging, false, 900), 0);
        let equal_exchange = state("4q1k1/8/8/8/4Q3/8/8/4R1K1");
        assert_eq!(material_risk(&equal_exchange, false, 0), 0);
        // After e7-e8=Q, ...Rxe8 Kxe8 wins a rook for the original pawn.
        let promotion = state("r3Q2k/3K4/8/8/8/8/8/8");
        assert_eq!(material_risk(&promotion, false, 800), 0);
    }

    #[test]
    fn travel_understanding_does_not_reduce_ordinary_moves() {
        use crate::core::action::FullMove;
        use crate::core::vec4::Vec4;
        let action = |t, l| MoveSeq::from(&[FullMove { from: Vec4::new(4, 1, 3, 0), to: Vec4::new(4, 3, t, l) }][..]);
        for level in 1..=8 {
            assert_eq!(travel_reduction(level, &action(3, 0)), 0);
        }
        assert_eq!(travel_reduction(1, &action(1, 0)), 2);
        assert_eq!(travel_reduction(5, &action(3, 1)), 1);
        assert_eq!(travel_reduction(8, &action(1, 0)), 0);
    }

    #[test]
    fn safety_does_not_reject_winning_sacrifices_or_prefer_mate() {
        for level in 1..=7 {
            assert_eq!(pick(&[super::super::ab::MATE - 3, 100], &[900, 0], level, 0), Some(0));
            assert_eq!(pick(&[-super::super::ab::MATE + 3, 100], &[0, 900], level, 0), Some(1));
        }
        // Advanced 5D understanding can accept enough concrete compensation for a queen.
        assert_eq!(pick(&[12000, 0], &[900, 0], 7, 0), Some(0));
        assert_eq!(pick(&[12000, 0], &[900, 0], 1, 0), Some(1));
    }
}
