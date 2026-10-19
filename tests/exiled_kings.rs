//! The exiled-kings tournament rule: at a stalemate, a side without a royal piece on the
//! latest board of every active timeline loses.

use kairos::core::state::State;
use kairos::core::variants::state_from_setup;
use kairos::web::pgn::*;

const GAME: &str = include_str!("data/pgn/exiled_kings.5dpgn");

/// The position at the end of the main line of the community's exiled-kings example game.
fn final_position() -> State {
    let g = parse_5dpgn(GAME, None).expect("parse");
    let mut s = state_from_setup(&g.setup).unwrap();
    let mut node = &g.root;
    while let Some(child) = node.children.first() {
        for em in child.action.as_ref().unwrap().moves() {
            let n = s.normalize_promotion(*em).expect("promotion");
            assert!(s.apply_ext::<false>(n), "illegal move in {}", child.text);
        }
        assert!(s.submit::<false>(), "submit {}", child.text);
        node = child;
    }
    s
}

#[test]
fn white_is_the_exiled_side_in_the_example_game() {
    // White's royals all sit on inactive timelines (L2-L4); Black keeps royals on active L0.
    let s = final_position();
    assert_eq!(s.get_active_range(), (0, 1));
    assert_eq!(s.exiled_kings_loser(), Some(false));
}

#[test]
fn nobody_is_exiled_in_the_start_position() {
    let s = kairos::core::variants::startpos();
    assert_eq!(s.exiled_kings_loser(), None);
}

#[test]
fn the_action_that_ends_the_game_by_exiled_kings_is_written_with_a_mate_symbol() {
    // A self-play game that ended by the exiled-kings rule: its last action leaves a stalemate
    // with a side exiled, and is written like a mate (`#`) though it need not be a check; the
    // actions before it carry no `#`.
    let line = include_str!("data/exiled_kings_record.jsonl");
    let l = kairos::web::record::load(line, None).expect("record");
    let main = l.game.mainline();
    let last = *main.last().unwrap();
    assert!(l.game.nodes[last].san.ends_with('#'), "{}", l.game.nodes[last].san);
    for &id in &main[1..main.len() - 1] {
        assert!(!l.game.nodes[id].san.contains('#'), "{}", l.game.nodes[id].san);
    }
}
