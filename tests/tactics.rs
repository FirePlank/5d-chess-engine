//! The tactics suite (data/tactics.g, from data/tactics_src.txt): legal positions and actions.

use kairos::misc::tactics::{generate, parse};

const SRC: &str = include_str!("../data/tactics_src.txt");
const SUITE: &str = include_str!("../data/tactics.g");

fn lf(s: &str) -> String {
    s.replace("\r\n", "\n")
}

#[test]
fn suite_is_generated_from_its_source() {
    assert_eq!(lf(&generate(&lf(SRC)).expect("source builds")), lf(SUITE), "run `ktools tacticsgen > data/tactics.g`");
}

#[test]
fn every_position_and_best_action_is_legal() {
    // `parse` replays every G line and plays every best action with the rules core.
    let suite = parse(SUITE).expect("suite parses");
    assert!(suite.len() >= 50, "{} positions", suite.len());
    for t in &suite {
        assert!(!t.best.is_empty(), "{}", t.name);
        assert_eq!(t.state.get_mate_type(), kairos::core::state::MateType::None, "{} is over", t.name);
    }
}
