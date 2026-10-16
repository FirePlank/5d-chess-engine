//! Mate reporting: the engine reports "score mate N" only for proven mates.
//!
//! Runs with the eval net of commit 406688d (tests/data/evalnet_406688d.bin), under which the
//! mate-in-7 puzzle of data/puzzles.g used to be reported as "mate 6" from depth 12 on: every
//! defence the pruned search looked at got mated, but not every defence was looked at.

use kairos::core::perft::parse_g_line;
use kairos::engine::ab::{score_text, AlphaBeta, MATE};
use kairos::engine::{evalnet, internal_cp, report_cp, wdl, Limits};
use std::sync::atomic::AtomicBool;

fn setup() {
    // Every test installs the same net before searching, so whichever runs first wins.
    evalnet::install(include_bytes!("data/evalnet_406688d.bin"));
    assert!(evalnet::embedded().is_some());
}

/// The info lines of a fixed-depth search of the position in a G line.
fn search(g: &str, depth: u32) -> Vec<String> {
    setup();
    let s = parse_g_line(g).unwrap();
    let mut lines = Vec::new();
    let stop = AtomicBool::new(false);
    AlphaBeta::new().find_best_move(&s, &Limits { depth: Some(depth), ..Default::default() }, &stop, &mut |l| lines.push(l));
    lines
}

/// The "score ..." part of an info line: ("mate", n) or ("cp", x).
fn score(line: &str) -> (String, i32) {
    let mut it = line.split_whitespace().skip_while(|w| *w != "score").skip(1);
    (it.next().unwrap().to_string(), it.next().unwrap().parse().unwrap())
}

#[test]
fn true_mates_are_reported() {
    // Mate in 1 (Rh8).
    let lines = search("G 1 size 8x8 odd fen [k7/8/1K6/8/8/8/8/7R:0:1:w]", 3);
    assert!(lines.iter().all(|l| score(l) == ("mate".into(), 1)), "{:?}", lines);
    // Back-rank mate in 1 (Ra8).
    let lines = search("G 1 size 8x8 odd fen [6k1/5p*p*p*/8/8/8/8/8/R5K1:0:1:w]", 3);
    assert!(lines.iter().all(|l| score(l) == ("mate".into(), 1)), "{:?}", lines);
    // Mate in 2 through time (Bc6 Kb8 Bxb7), found from depth 4.
    let lines = search("G 1 size 8x8 odd fen [k*7/p*n6/K*7/8/8/8/6B/8:0:1:w]", 6);
    assert!(lines[3..].iter().all(|l| score(l) == ("mate".into(), 2)), "{:?}", lines);
    // The mated side: after Bc6 only Kb8 is legal, and Bxb7 mates.
    let lines = search("G 1 size 8x8 odd fen [k*7/p*n6/K*7/8/8/8/6B/8:0:1:w] moves (0T1)g2c6 submit", 4);
    assert!(lines[1..].iter().all(|l| score(l) == ("mate".into(), -1)), "{:?}", lines);
}

#[test]
fn threaded_searches_report_mates() {
    // Lazy SMP: helper threads share the table (and its proofs) with the reporting thread.
    setup();
    let mut e = AlphaBeta::new();
    e.set_option("Threads", &kairos::engine::OptionValue::Int(4));
    let stop = AtomicBool::new(false);
    let mut run = |g: &str, depth: u32| {
        let s = parse_g_line(g).unwrap();
        let mut lines = Vec::new();
        e.find_best_move(&s, &Limits { depth: Some(depth), ..Default::default() }, &stop, &mut |l| lines.push(l));
        lines
    };
    let lines = run("G 1 size 8x8 odd fen [k7/8/1K6/8/8/8/8/7R:0:1:w]", 3);
    assert!(lines.iter().all(|l| score(l) == ("mate".into(), 1)), "{:?}", lines);
    let lines = run("G 1 size 8x8 odd fen [k*7/p*n6/K*7/8/8/8/6B/8:0:1:w]", 6);
    assert_eq!(score(lines.last().unwrap()), ("mate".into(), 2), "{:?}", lines);
}

#[test]
fn reported_scale() {
    // +100 reported is about a 75% expected score, +200 about 90%, +400 about 99%.
    assert_eq!(report_cp(3000), 100);
    assert_eq!(report_cp(-6000), -200);
    assert_eq!(internal_cp(100), 3000);
    assert_eq!(wdl(3000), (752, 0, 248));
    assert_eq!(wdl(6000).0, 902);
    assert_eq!(wdl(12000).0, 988);
    assert_eq!(wdl(0), (500, 0, 500));
    assert_eq!(score_text(MATE - 5), "mate 3 wdl 1000 0 0");
    assert_eq!(score_text(-(MATE - 4)), "mate -2 wdl 0 0 1000");
    assert_eq!(score_text(142), "cp 5 wdl 513 0 487");
}
