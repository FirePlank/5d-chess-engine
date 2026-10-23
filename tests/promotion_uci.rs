//! The 5DUCI `position` argument carries the pieces pawns promote to.

use kairos::core::promotion::{format_promotions, PromotionOptions};
use kairos::core::variants::{apply_uci_moves, position_from_uci};

const BOARD: &str = "[4k3/P7/8/8/8/8/8/4K3:0:1:w]";

#[test]
fn promotions_token_sets_the_pieces() {
    let s = position_from_uci(&format!("size 8x8 odd promotions QRN fen {BOARD}")).unwrap();
    assert_eq!(format_promotions(s.get_promotion_options()), "QRN");
    let q = position_from_uci(&format!("size 8x8 odd fen {BOARD}")).unwrap();
    assert_eq!(q.get_promotion_options(), PromotionOptions::QUEEN);
}

#[test]
fn only_the_listed_pieces_are_legal() {
    let mut s = position_from_uci(&format!("size 8x8 odd promotions RN fen {BOARD}")).unwrap();
    // No queen in the set: a queen promotion is refused, a knight is applied, and the default
    // (a move without a letter) is the first of the set (the rook).
    assert!(apply_uci_moves(&mut s.clone(), "(0T1)a7a8Q submit").is_err());
    assert!(apply_uci_moves(&mut s.clone(), "(0T1)a7a8N submit").is_ok());
    assert!(apply_uci_moves(&mut s, "(0T1)a7a8 submit").is_ok());
}

/// The search plays an underpromotion when the position allows it and it is better: here 1. h7+
/// Kxf7 2. h8=N# is mate in 2, while promoting to a queen mates later.
#[test]
fn the_search_underpromotes() {
    use kairos::engine::ab::AlphaBeta;
    use kairos::engine::Limits;
    use std::sync::atomic::AtomicBool;
    const FEN: &str = "[5bk*1/5N2/7P/6K1/8/8/8/8:0:1:w]";
    let search = |promotions: &str, moves: &str, depth: u32| {
        let mut s = position_from_uci(&format!("size 8x8 odd {promotions}fen {FEN}")).unwrap();
        apply_uci_moves(&mut s, moves).unwrap();
        let mut lines = Vec::new();
        let best = AlphaBeta::new().find_best_move(&s, &Limits { depth: Some(depth), ..Default::default() }, &AtomicBool::new(false), &mut |l| lines.push(l));
        (best.map(|a| a.to_string()), lines.into_iter().filter(|l| l.contains(" score ")).last().unwrap_or_default())
    };
    let (_, line) = search("promotions QRBN ", "", 6);
    assert!(line.contains("score mate 2"), "{line}");
    let (best, line) = search("promotions QRBN ", "(0T1)h6h7 submit (0T1)g8f7 submit", 3);
    assert_eq!(best.as_deref().map(str::trim), Some("(0T2)h7h8N"), "{line}");
    assert!(line.contains("score mate 1") && line.contains("h7h8N"), "{line}");
    // Queen only: no mate in 2.
    let (_, line) = search("", "", 6);
    assert!(!line.contains("score mate 2"), "{line}");
}
