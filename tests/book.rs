//! The opening book (data/book_theory.txt, from data/book_theory_src.txt) and `OwnBook`.

use kairos::core::variants::{apply_uci_moves, position_from_uci};
use kairos::engine::ab::AlphaBeta;
use kairos::engine::book::{generate_tuned, Book};
use kairos::engine::{Limits, OptionValue};
use kairos::web::api::{resolve_position, Api};
use kairos::web::tree::{parse_lan_moves, play_action};
use std::sync::atomic::AtomicBool;

const SRC: &str = include_str!("../data/book_theory_src.txt");
const BOOK: &str = include_str!("../data/book_theory.txt");
const TUNING: &str = include_str!("../data/book_tuning.txt");

fn lf(s: &str) -> String {
    s.replace("\r\n", "\n")
}

/// A variant's start position followed by 5DUCI moves.
fn position(variant: &str, moves: &str) -> kairos::core::state::State {
    let mut s = position_from_uci(&resolve_position(variant).unwrap()).unwrap();
    apply_uci_moves(&mut s, moves).unwrap();
    s
}

#[test]
fn book_file_is_generated_from_its_source() {
    assert_eq!(lf(&generate_tuned(&lf(SRC), &lf(TUNING)).expect("source and tuning build")), lf(BOOK), "run `ktools bookgen > data/book_theory.txt`");
}

#[test]
fn book_parses_with_every_action_legal() {
    let b = Book::parse(BOOK).expect("book parses");
    let (positions, moves) = b.size();
    assert!(positions > 200 && moves > positions, "{} positions, {} moves", positions, moves);
    assert_eq!(Book::theory().size(), (positions, moves), "the embedded book is the file");
}

#[test]
fn positions_are_found_however_they_were_reached() {
    // T0 after 1. Nf3 Nf6 2. d4: d5 is the main reply, c5 (Sarp's) the alternative.
    let moves = "(0T1)g1f3 submit (0T1)g8f6 submit (0T2)d2d4 submit";
    let s = position("Standard - Turn Zero", moves);
    let found = Book::theory().probe(&s).expect("book position");
    assert!(found.iter().any(|m| m.play && m.lan == "(0T2)d7d5" && m.weight == 10));
    assert!(found.iter().any(|m| m.play && m.lan == "(0T2)c7c5"));
    // The same position reached by playing actions (as kairos-web and the game tree do).
    let mut t = position_from_uci(&resolve_position("Standard - Turn Zero").unwrap()).unwrap();
    for act in moves.split("submit").filter(|a| !a.trim().is_empty()) {
        t = play_action(&t, &parse_lan_moves(act).unwrap()).unwrap();
    }
    assert_eq!(t.hash(), s.hash());
    // Standard has its own section: White plays 1. Nf3 (the London study's line), Black answers
    // everything with Nf6 (the f7 sac defences), and the Turn Zero positions are not Standard ones.
    let std_start = Book::theory().probe(&position("Standard", "")).expect("Standard start");
    assert!(std_start.iter().all(|m| m.play == (m.lan == "(0T1)g1f3")), "{:?}", std_start.iter().map(|m| &m.lan).collect::<Vec<_>>());
    let std_nf6 = Book::theory().probe(&position("Standard", "(0T1)e2e3 submit")).expect("Standard 1. e3");
    assert!(std_nf6.iter().any(|m| m.play && m.lan == "(0T1)g8f6"));
    let std_d4 = Book::theory().probe(&position("Standard", moves)).expect("Standard 1. Nf3 Nf6 2. d4");
    assert!(std_d4.iter().any(|m| m.play && m.lan == "(0T2)d7d5" && m.weight == 10));
    assert!(!std_d4.iter().any(|m| m.lan == "(0T2)c7c5"), "Sarp's is Turn Zero theory only");
    // Out of book.
    assert!(Book::theory().probe(&position("Standard - Turn Zero", "(0T1)h2h4 submit (0T1)h7h5 submit")).is_none());
}

#[test]
fn know_moves_are_never_chosen() {
    // 1. Nf3 Nf6 2. d4 d5 3. Bd2 c6 4. Nc3: Nc3 is a blunder (know), Qa5# the punishment (play); e3 and c4 are played.
    let s = position(
        "Standard - Turn Zero",
        "(0T1)g1f3 submit (0T1)g8f6 submit (0T2)d2d4 submit (0T2)d7d5 submit (0T3)c1d2 submit (0T3)c7c6 submit",
    );
    let b = Book::theory();
    assert!(b.probe(&s).unwrap().iter().any(|m| !m.play && m.lan == "(0T4)b1c3"));
    for seed in 0..200 {
        let m = b.choose(&s, seed).unwrap();
        assert!(m.play && (m.lan == "(0T4)e2e3" || m.lan == "(0T4)c2c4"), "{}", m.lan);
    }
    // White's first move in T0: Nf3 (10), e3 (5) or e4 (2); the other first moves are known,
    // not played.
    let start = position("Standard - Turn Zero", "");
    let mut seen = std::collections::HashMap::new();
    for seed in 0..2000 {
        *seen.entry(b.choose(&start, seed).unwrap().lan.clone()).or_insert(0) += 1;
    }
    assert_eq!(seen.len(), 3, "{:?}", seen);
    assert!(seen["(0T1)g1f3"] > seen["(0T1)e2e3"] && seen["(0T1)e2e3"] > seen["(0T1)e2e4"], "{:?}", seen);
    // A position with only known moves has nothing to play.
    let s = position(
        "Standard - Turn Zero",
        "(0T1)g1f3 submit (0T1)g8f6 submit (0T2)d2d4 submit (0T2)d7d5 submit (0T3)c2c3 submit (0T3)c7c6 submit (0T4)e2e3 submit",
    );
    assert!(b.probe(&s).is_some() && b.choose(&s, 1).is_none());
}

#[test]
fn weighted_choice_uses_every_play_move() {
    // T0 after 1. Nf3 Nf6: d4 (10), c4 (5), Na3 (3), e3 (2) are played, Nd4 (know) is not.
    let s = position("Standard - Turn Zero", "(0T1)g1f3 submit (0T1)g8f6 submit");
    let mut seen = std::collections::HashMap::new();
    for seed in 0..2000 {
        *seen.entry(Book::theory().choose(&s, seed).unwrap().lan.clone()).or_insert(0) += 1;
    }
    assert_eq!(seen.len(), 4, "{:?}", seen);
    assert!(seen["(0T2)d2d4"] > seen["(0T2)c2c4"] && seen["(0T2)c2c4"] > seen["(0T2)e2e3"], "{:?}", seen);
}

fn search(own_book: Option<bool>, variant: &str, moves: &str) -> (String, Vec<String>) {
    let s = position(variant, moves);
    let mut e = AlphaBeta::new();
    if let Some(b) = own_book {
        assert!(e.set_option("OwnBook", &OptionValue::Bool(b)));
    }
    let mut lines = Vec::new();
    let best = e.find_best_move(&s, &Limits { depth: Some(1), ..Default::default() }, &AtomicBool::new(false), &mut |l| lines.push(l));
    (kairos::web::tree::action_lan(&s, &best.unwrap()), lines)
}

#[test]
fn own_book_plays_book_moves_only_when_on() {
    // Off by default: a normal search.
    let (_, lines) = search(None, "Standard", "");
    assert!(lines.iter().all(|l| !l.contains("book")) && lines.iter().any(|l| l.starts_with("depth")));
    // On: White's first move from the book, with no search.
    let (best, lines) = search(Some(true), "Standard - Turn Zero", "");
    assert!(["(0T1)g1f3", "(0T1)e2e3", "(0T1)e2e4"].contains(&best.as_str()), "{}", best);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with(&format!("string book {}", best)), "{}", lines[0]);
    // On in Standard: its own book (1. Nf3).
    let (best, lines) = search(Some(true), "Standard", "");
    assert_eq!(best, "(0T1)g1f3");
    assert!(lines.len() == 1 && lines[0].starts_with("string book (0T1)g1f3"), "{:?}", lines);
    // On, out of book: a normal search.
    let (_, lines) = search(Some(true), "Standard - Turn Zero", "(0T1)h2h4 submit (0T1)h7h5 submit");
    assert!(lines.iter().all(|l| !l.contains("book")) && lines.iter().any(|l| l.starts_with("depth")));
    // Off again.
    let (_, lines) = search(Some(false), "Standard - Turn Zero", "");
    assert!(lines.iter().all(|l| !l.contains("book")));
}

#[test]
fn api_book_hint_and_book_search() {
    let mut api = Api::new();
    let mut info = |_: &str| {};
    let r = api.call(r#"{"cmd":"new","position":"Standard - Turn Zero"}"#, &mut info);
    assert!(r.contains("\"ok\":true"), "{}", r);
    let r = api.call(r#"{"cmd":"book","node":0}"#, &mut info);
    assert!(r.contains("\"lan\":\"(0T1)g1f3\"") && r.contains("\"play\":true") && r.contains("\"play\":false"), "{}", r);
    let pos = resolve_position("Standard - Turn Zero").unwrap();
    let r = api.call(&format!(r#"{{"cmd":"search","position":"{}","depth":1,"book":true}}"#, pos), &mut info);
    assert!(["(0T1)g1f3", "(0T1)e2e3", "(0T1)e2e4"].iter().any(|m| r.contains(&format!("\"bestmove\":\"{}\"", m))), "{}", r);
    let r = api.call(&format!(r#"{{"cmd":"book","position":"{} moves (0T1)h2h4 submit"}}"#, pos), &mut info);
    assert!(r.contains("\"moves\":[]"), "{}", r);
}

/// 5DUCI moves of the London line 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3
/// 7. Qxd3 Bd6 8. Ng5, followed by `more`.
fn london(more: &str) -> String {
    let main = "(0T1)g1f3 submit (0T1)g8f6 submit (0T2)d2d4 submit (0T2)d7d5 submit (0T3)c2c3 submit (0T3)c7c6 submit                 (0T4)c1f4 submit (0T4)c8f5 submit (0T5)e2e3 submit (0T5)e7e6 submit (0T6)f1d3 submit (0T6)f5d3 submit                 (0T7)d1d3 submit (0T7)f8d6 submit (0T8)f3g5 submit";
    format!("{} {}", main, more)
}

#[test]
fn london_ng5_study_in_both_variants() {
    for variant in ["Standard - Turn Zero", "Standard"] {
        let b = Book::theory();
        // 8...Bxf4: 9. exf4 is the main line, 9. Nxf7!? a sideline.
        let after_bxf4 = b.probe(&position(variant, &london("(0T8)d6f4 submit"))).expect("8...Bxf4");
        assert!(after_bxf4.iter().any(|m| m.play && m.weight == 10 && m.lan == "(0T9)e3f4"), "{}", variant);
        assert!(after_bxf4.iter().any(|m| m.play && m.weight == 3 && m.lan == "(0T9)g5f7"), "{}", variant);
        // Black's 8th moves are known, not played (the study is White's repertoire).
        let after_ng5 = b.probe(&position(variant, &london(""))).expect("8. Ng5");
        assert!(after_ng5.iter().all(|m| !m.play) && after_ng5.len() >= 4, "{}", variant);
        // Travel idea 1: after 9. exf4 Qe7 10. Nd2 Nbd7 11. Qe2 e5 12. Ne6 Qxe6 the queen goes
        // from e2 to the T9 board's e6.
        let s = position(
            variant,
            &london("(0T8)d6f4 submit (0T9)e3f4 submit (0T9)d8e7 submit (0T10)b1d2 submit (0T10)b8d7 submit (0T11)d3e2 submit (0T11)e6e5 submit (0T12)g5e6 submit (0T12)e7e6 submit"),
        );
        // (The engine measured the study's travel 334 cp below its best move there: it stays in the
        // book as known theory, no longer played; see data/book_tuning.txt.)
        let moves = b.probe(&s).expect("travel idea 1");
        assert!(moves.iter().any(|m| m.lan == "(0T13)e2(0T9)e6" && !m.play), "{}", variant);
    }
}

#[test]
fn bg5_bg4_e3_black_avoids_ne4_and_bxf3() {
    let pre = "(0T1)g1f3 submit (0T1)g8f6 submit (0T2)d2d4 submit (0T2)d7d5 submit (0T3)c2c3 submit (0T3)c7c6 submit (0T4)c1g5 submit (0T4)c8g4 submit (0T5)e2e3 submit";
    for variant in ["Standard - Turn Zero", "Standard"] {
        let b = Book::theory();
        let s = position(variant, pre);
        assert!(b.probe(&s).unwrap().iter().any(|m| !m.play && m.lan == "(0T5)f6e4"), "{}", variant);
        for seed in 0..200 {
            let m = b.choose(&s, seed).unwrap();
            assert!(m.lan == "(0T5)e7e6" || m.lan == "(0T5)b8d7", "{} {}", variant, m.lan);
        }
        // After 5...Ne4 6. h3 the bishop retreats; Bxf3 is known, not played.
        let s = position(variant, &format!("{} (0T5)f6e4 submit (0T6)h2h3 submit", pre));
        for seed in 0..50 {
            assert_eq!(b.choose(&s, seed).unwrap().lan, "(0T6)g4h5", "{}", variant);
        }
        assert!(b.probe(&s).unwrap().iter().any(|m| !m.play && m.lan == "(0T6)g4f3"));
    }
}

#[test]
fn openings_are_named_where_they_are_reached() {
    let b = Book::theory();
    assert!(b.name_count() > 80, "{} names", b.name_count());
    let name = |v: &str, m: &str| b.opening_name(&position(v, m)).map(str::to_string);
    assert_eq!(name("Standard - Turn Zero", "(0T1)g1f3 submit (0T1)g8f6 submit (0T2)d2d4 submit (0T2)c7c5 submit").as_deref(), Some("Sarp's Counterattack"));
    assert_eq!(name("Standard - Turn Zero", "(0T1)g1f3 submit (0T1)g8f6 submit (0T2)b1a3 submit").as_deref(), Some("Digolian Attack"));
    assert_eq!(name("Standard - Turn Zero", "(0T1)g1f3 submit (0T1)g8f6 submit (0T2)h2h3 submit"), None);
    assert!(b.probe(&position("Standard - Defended Pawn", "")).unwrap().iter().any(|m| m.play && m.lan == "(0T1)c2c4"));
}

#[test]
fn api_opening_follows_the_selected_branch_and_survives_leaving_the_book() {
    let mut api = Api::new();
    let mut call = |request: &str| {
        let response = api.call(request, &mut |_| {});
        let json = kairos::web::json::Json::parse(&response).unwrap();
        assert_eq!(json.get("ok").bool(), Some(true), "{response}");
        json
    };
    call(r#"{"cmd":"new","position":"Standard - Turn Zero"}"#);
    let mut node = 0;
    for lan in ["(0T1)g1f3", "(0T1)g8f6", "(0T2)b1a3"] {
        node = call(&format!(r#"{{"cmd":"play","node":{node},"lan":"{lan}"}}"#)).get("node").int().unwrap();
    }
    let book = call(&format!(r#"{{"cmd":"book","node":{node}}}"#));
    assert_eq!(book.get("opening").get("name").str(), Some("Digolian Attack"));
    assert_eq!(book.get("opening").get("ply").int(), Some(3));
    let next = call(&format!(r#"{{"cmd":"play","node":{node},"lan":"(0T2)h7h5"}}"#)).get("node").int().unwrap();
    let book = call(&format!(r#"{{"cmd":"book","node":{next}}}"#));
    assert_eq!(book.get("opening").get("name").str(), Some("Digolian Attack"));
    assert_eq!(book.get("opening").get("node").int(), Some(node));
    let other = call(r#"{"cmd":"play","node":2,"lan":"(0T2)h2h3"}"#).get("node").int().unwrap();
    assert!(call(&format!(r#"{{"cmd":"book","node":{other}}}"#)).get("opening").is_null());
    assert!(call(r#"{"cmd":"book","node":0}"#).get("opening").is_null());
    call(r#"{"cmd":"new","position":"Standard"}"#);
    assert!(call(r#"{"cmd":"book","node":0}"#).get("opening").is_null());
}

#[test]
fn transposed_move_orders_find_the_same_book_position() {
    // 1. Nf3 Nf6 2. d4 d5 3. c3 c6 and 1. Nf3 Nf6 2. c3 d5 3. d4 c6 are one position (the
    // Classical f7 Defense structure), and 1. d4 Nf6 2. Bf4 is in the book too.
    let a = position("Standard - Turn Zero", "(0T1)g1f3 submit (0T1)g8f6 submit (0T2)d2d4 submit (0T2)d7d5 submit (0T3)c2c3 submit (0T3)c7c6 submit");
    let b = position("Standard - Turn Zero", "(0T1)g1f3 submit (0T1)g8f6 submit (0T2)c2c3 submit (0T2)d7d5 submit (0T3)d2d4 submit (0T3)c7c6 submit");
    let c = position("Standard - Turn Zero", "(0T1)d2d4 submit (0T1)g8f6 submit (0T2)c1f4 submit");
    let book = Book::theory();
    assert_eq!(book.opening_name(&a), book.opening_name(&b));
    assert!(book.opening_name(&a).is_some());
    let (pa, pb) = (book.probe(&a).unwrap(), book.probe(&b).unwrap());
    // (Each known history has its own moves.)
    assert!(!pa.is_empty() && !pb.is_empty());
    assert!(book.probe(&c).is_some());
    // The Standard variant's start is not Turn Zero's.
    let std_start = book.probe(&position("Standard", ""));
    let t0_start = book.probe(&position("Standard - Turn Zero", "")).unwrap();
    assert!(std_start.is_none_or(|m| m.len() != t0_start.len() || m.iter().zip(t0_start).any(|(x, y)| x.lan != y.lan || x.play != y.play)));
}

#[test]
fn moves_that_transpose_into_the_book_are_listed_and_played() {
    // 1. d4 Nf6 2. Nf3 reaches 1. Nf3 Nf6 2. d4 by another move order, a position with played
    // moves: the transposition is listed and played too (at a low weight).
    let s = position("Standard - Turn Zero", "(0T1)d2d4 submit (0T1)g8f6 submit");
    let b = Book::theory();
    let t = b.transpositions(&s);
    assert!(t.iter().any(|m| m.lan == "(0T2)g1f3" && m.play && m.weight == kairos::engine::book::TRANSPOSE_WEIGHT), "{:?}", t.iter().map(|m| &m.lan).collect::<Vec<_>>());
    // Its own book moves are not repeated.
    assert!(!t.iter().any(|m| m.lan == "(0T2)c1f4"));
    let mut seen = std::collections::HashSet::new();
    for seed in 0..500 {
        seen.insert(b.choose(&s, seed).unwrap().lan);
    }
    assert!(seen.contains("(0T2)g1f3") && seen.contains("(0T2)c1f4"), "{:?}", seen);
}

#[test]
fn an_exact_line_gives_a_transposed_history_known_moves_only() {
    // Both orders reach the same board, but in `other` the f3 knight was not there on turn 1.
    // The line is marked {exact}, so 3. b3 is played only in the guide's order.
    let guide = position("Standard - Turn Zero", "(0T1)g1f3 submit (0T1)g8f6 submit (0T2)e2e3 submit (0T2)c7c5 submit");
    let other = position("Standard - Turn Zero", "(0T1)e2e3 submit (0T1)g8f6 submit (0T2)g1f3 submit (0T2)c7c5 submit");
    let b = Book::theory();
    assert!(b.probe(&guide).unwrap().iter().any(|m| m.play && m.lan == "(0T3)b2b3"));
    let moves = b.probe(&other).expect("the book still knows the board");
    assert!(moves.iter().all(|m| !m.play) && moves.iter().any(|m| m.lan == "(0T3)b2b3"), "{:?}", moves.iter().map(|m| (&m.lan, m.play)).collect::<Vec<_>>());
    assert!(b.choose(&other, 1).is_none());
}
