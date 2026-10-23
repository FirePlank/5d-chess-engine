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
