//! The opening book (data/book_theory.txt, from data/book_theory_src.txt) and `OwnBook`.

use kairos::core::variants::{apply_uci_moves, position_from_uci};
use kairos::engine::ab::AlphaBeta;
use kairos::engine::book::{generate, Book};
use kairos::engine::{Limits, OptionValue};
use kairos::web::api::{resolve_position, Api};
use kairos::web::tree::{parse_lan_moves, play_action};
use std::sync::atomic::AtomicBool;

const SRC: &str = include_str!("../data/book_theory_src.txt");
const BOOK: &str = include_str!("../data/book_theory.txt");

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
    assert_eq!(lf(&generate(&lf(SRC)).expect("source builds")), lf(BOOK), "run `ktools bookgen > data/book_theory.txt`");
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
    // Standard and Turn Zero are different books.
    let std = position("Standard", "(0T1)g1f3 submit (0T1)g8f6 submit (0T2)d2d4 submit");
    assert!(Book::theory().probe(&std).is_some_and(|v| v.iter().all(|m| m.lan == "(0T2)d7d5")));
    // Out of book.
    assert!(Book::theory().probe(&position("Standard", "(0T1)h2h4 submit (0T1)h7h5 submit")).is_none());
}

#[test]
fn know_moves_are_never_chosen() {
    // 1. Nf3 Nf6 2. d4 d5 3. Bd2 c6 4. Nc3: Nc3 is a blunder (know), Qa5# the punishment (play).
    let s = position(
        "Standard - Turn Zero",
        "(0T1)g1f3 submit (0T1)g8f6 submit (0T2)d2d4 submit (0T2)d7d5 submit (0T3)c1d2 submit (0T3)c7c6 submit",
    );
    let b = Book::theory();
    assert!(b.probe(&s).unwrap().iter().any(|m| !m.play && m.lan == "(0T4)b1c3"));
    for seed in 0..200 {
        let m = b.choose(&s, seed).unwrap();
        assert!(m.play && m.lan == "(0T4)e2e3", "{}", m.lan);
    }
    // White's first move in T0: always Nf3 (the other first moves are known, not played).
    let start = position("Standard - Turn Zero", "");
    assert!((0..200).all(|seed| b.choose(&start, seed).unwrap().lan == "(0T1)g1f3"));
    // A position with only known moves has nothing to play.
    let s = position(
        "Standard - Turn Zero",
        "(0T1)g1f3 submit (0T1)g8f6 submit (0T2)d2d4 submit (0T2)d7d5 submit (0T3)c2c3 submit (0T3)c7c6 submit (0T4)e2e3 submit",
    );
    assert!(b.probe(&s).is_some() && b.choose(&s, 1).is_none());
}

#[test]
fn weighted_choice_uses_every_play_move() {
    // T0 after 1. Nf3 Nf6: d4 (10), c4 (5), e3 (2) are played, Nd4 (know) is not.
    let s = position("Standard - Turn Zero", "(0T1)g1f3 submit (0T1)g8f6 submit");
    let mut seen = std::collections::HashMap::new();
    for seed in 0..2000 {
        *seen.entry(Book::theory().choose(&s, seed).unwrap().lan.clone()).or_insert(0) += 1;
    }
    assert_eq!(seen.len(), 3, "{:?}", seen);
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
    let (best, lines) = search(Some(true), "Standard", "");
    assert_eq!(best, "(0T1)g1f3");
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("string book (0T1)g1f3"), "{}", lines[0]);
    // On, out of book: a normal search.
    let (_, lines) = search(Some(true), "Standard", "(0T1)h2h4 submit (0T1)h7h5 submit");
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
    let pos = resolve_position("Standard").unwrap();
    let r = api.call(&format!(r#"{{"cmd":"search","position":"{}","depth":1,"book":true}}"#, pos), &mut info);
    assert!(r.contains("\"bestmove\":\"(0T1)g1f3\""), "{}", r);
    let r = api.call(&format!(r#"{{"cmd":"book","position":"{} moves (0T1)h2h4 submit"}}"#, pos), &mut info);
    assert!(r.contains("\"moves\":[]"), "{}", r);
}
