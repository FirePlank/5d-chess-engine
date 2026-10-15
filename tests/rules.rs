//! Rules regressions found by cross-checking against other implementations (see RULES_VERIFICATION.md).

use kairos::core::perft::parse_g_line;
use kairos::core::state::State;
use kairos::core::vec4::Vec4;

/// Destination squares (x, y) of the piece on (x, y) of the side to move's board (l, t).
fn targets(s: &State, l: i32, t: i32, x: i32, y: i32) -> Vec<(i32, i32, i32, i32)> {
    let mut out = Vec::new();
    s.gen_piece_move(Vec4::new(x, y, t, l), &mut out);
    let mut v: Vec<_> = out.iter().map(|q| (q.l(), q.t(), q.x(), q.y())).collect();
    v.sort();
    v
}

#[test]
fn castling_on_boards_narrower_than_eight_files() {
    // King d1, rooks a1 and g1 on a 7-file board: both castlings are available.
    let s = parse_g_line("G 1 size 7x7 odd fen [r*2k*2r*/p*p*p*p*p*p*p*/7/7/7/P*P*P*P*P*P*P*/R*2K*2R*:0:1:w]").unwrap();
    let k = targets(&s, 0, 1, 3, 0);
    assert!(k.contains(&(0, 1, 1, 0)), "queenside castling missing: {:?}", k);
    assert!(k.contains(&(0, 1, 5, 0)), "kingside castling missing: {:?}", k);
    let mut t = s.clone();
    assert!(t.apply_move::<false>(kairos::core::action::FullMove::parse("(0T1)d1f1").unwrap()));
    assert_eq!(t.board(0, 1, true).get_fen(false), "r2k2r/ppppppp/7/7/7/PPPPPPP/R3RK1");
}

/// Castle on (l, t) with the king on (x, 0) to (to_x, 0) and return the new rank 1.
fn castle(s: &State, x: i32, to_x: i32) -> String {
    let mut t = s.clone();
    let m = kairos::core::action::FullMove::new(Vec4::new(x, 0, 1, 0), Vec4::new(to_x, 0, 1, 0));
    assert!(t.apply_move::<false>(m), "castling {} -> {} rejected", x, to_x);
    t.board(0, 1, true).get_fen(false).rsplit('/').next().unwrap().to_string()
}

#[test]
fn castling_with_a_rook_two_squares_away() {
    // Variant rule (Wikipedia, note a): a rook at least two squares away may castle; when it
    // is exactly two away the king lands on the rook's square and the rook on the one crossed.
    let s = parse_g_line("G 1 size 6x6 odd fen [r*1k*2r*/p*p*p*p*p*p*/6/6/P*P*P*P*P*P*/R*1K*2R*:0:1:w]").unwrap();
    let k = targets(&s, 0, 1, 2, 0);
    assert!(k.contains(&(0, 1, 0, 0)) && k.contains(&(0, 1, 4, 0)), "castling missing: {:?}", k);
    assert_eq!(castle(&s, 2, 0), "KR3R");
    assert_eq!(castle(&s, 2, 4), "R2RK1");
}

#[test]
fn castling_with_rooks_off_the_corners() {
    // Rooks on b1 and g1 (unmoved, as a variant may set them up): the king castles with each.
    let s = parse_g_line("G 1 size 8x8 odd fen [4k3/8/8/8/8/8/8/1R*2K*1R*1:0:1:w]").unwrap();
    let k = targets(&s, 0, 1, 4, 0);
    assert!(k.contains(&(0, 1, 2, 0)) && k.contains(&(0, 1, 6, 0)), "castling missing: {:?}", k);
    assert_eq!(castle(&s, 4, 2), "2KR2R1");
    assert_eq!(castle(&s, 4, 6), "1R3RK1");
    // A knight between king and rook, or a moved rook, still blocks castling.
    let s = parse_g_line("G 1 size 8x8 odd fen [4k3/8/8/8/8/8/8/1R*N1K*1R1:0:1:w]").unwrap();
    let k = targets(&s, 0, 1, 4, 0);
    assert!(!k.contains(&(0, 1, 2, 0)) && !k.contains(&(0, 1, 6, 0)), "illegal castling: {:?}", k);
}
