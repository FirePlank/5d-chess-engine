//! Limited levels should retain basic board-local tactical competence.
use kairos::core::variants::{apply_uci_moves, position_from_uci};
use kairos::engine::ab::AlphaBeta;
use kairos::engine::{Limits, OptionValue};
use kairos::web::tree::action_lan;
use std::sync::atomic::AtomicBool;

#[test]
fn limited_levels_take_a_free_queen() {
    let root = position_from_uci("size 8x8 odd fen [6k1/8/8/8/8/4q3/4R3/6K1:0:1:w]").unwrap();
    let stop = AtomicBool::new(false);
    for level in 1..=7 {
        let mut engine = AlphaBeta::new();
        engine.set_option("Hash", &OptionValue::Int(1));
        engine.set_option("Skill Level", &OptionValue::Int(level));
        let action = engine.find_best_move(&root, &Limits { depth: Some(3), nodes: Some(6_000), movetime: Some(250), ..Default::default() }, &stop, &mut |_| {}).unwrap();
        let lan = action_lan(&root, &action);
        assert_eq!(lan, "(0T1)e2e3", "level {level} ignored a free queen");
        assert!(apply_uci_moves(&mut root.clone(), &lan).is_ok());
    }
}

#[test]
fn limited_levels_do_not_ignore_a_rook_attacking_their_queen() {
    let root = position_from_uci("size 8x8 odd fen [4r1k1/8/8/8/4Q3/8/8/6K1:0:1:w]").unwrap();
    let stop = AtomicBool::new(false);
    for level in 1..=7 {
        let mut engine = AlphaBeta::new();
        engine.set_option("Hash", &OptionValue::Int(1));
        engine.set_option("Skill Level", &OptionValue::Int(level));
        let action = engine.find_best_move(&root, &Limits { depth: Some(3), nodes: Some(6_000), movetime: Some(250), ..Default::default() }, &stop, &mut |_| {}).unwrap();
        let lan = action_lan(&root, &action);
        assert!(lan.starts_with("(0T1)e4"), "level {level} ignored the attack: {lan}");
        assert!(!["(0T1)e4e5", "(0T1)e4e6", "(0T1)e4e7"].contains(&lan.as_str()), "level {level} left the queen en prise: {lan}");
    }
}
