//! 5dpgn parser and printer: the files in tests/data/pgn, round trips, and (ignored) the
//! external corpus.

use kairos::core::action::Action;
use kairos::core::hypercuboid::HcInfo;
use kairos::core::promotion::PromotionOptions;
use kairos::core::state::State;
use kairos::core::variants::{parse_board_fen_metadata, state_from_setup, VariantSetup};
use kairos::web::pgn::*;
use std::path::{Path, PathBuf};

const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/pgn");

/// The position of expert_m7.5dpgn: `size 8x8 odd fen [k7/p*n6/K7/8/8/8/6P*B/8:0:1:w]`.
fn m7_setup() -> VariantSetup {
    VariantSetup {
        size_x: 8,
        size_y: 8,
        boards: vec![parse_board_fen_metadata("k7/p*n6/K7/8/8/8/6P*B/8:0:1:w").unwrap()],
        even: false,
        promotions: PromotionOptions::QUEEN,
    }
}

fn data_files() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(DATA).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "5dpgn")).collect();
    v.sort();
    v
}

fn parse_file(path: &Path) -> PgnGame {
    let text = std::fs::read_to_string(path).unwrap();
    parse_5dpgn(&text, Some(&m7_setup())).unwrap_or_else(|e| panic!("{}: {}", path.display(), e))
}

/// Every (parent state, action, next main-line action) of the tree, variations included.
fn walk<'g>(node: &'g PgnNode, s: &State, out: &mut Vec<(State, &'g Action, Option<&'g Action>)>) {
    for c in &node.children {
        let act = c.action.as_ref().unwrap();
        out.push((s.clone(), act, c.children.first().and_then(|n| n.action.as_ref())));
        let next = s.can_apply_action(act).expect("stored action applies");
        walk(c, &next, out);
    }
}

fn all_actions(g: &PgnGame) -> Vec<(State, &Action, Option<&Action>)> {
    let mut out = Vec::new();
    walk(&g.root, &state_from_setup(&g.setup).unwrap(), &mut out);
    out
}

fn lan(a: &Action) -> String {
    a.moves().iter().map(|m| m.to_string()).collect::<Vec<_>>().join(" ")
}

/// Print each action (given its continuation, if any) and each of its moves and parse them back.
fn check_round_trip(s: &State, act: &Action, witness: Option<&Action>) {
    let text = action_to_pgn_with_witness(s, act, witness);
    let back = parse_action(s, &text).unwrap_or_else(|e| panic!("{} -> {}: {}", lan(act), text, e));
    assert_eq!(&back, act, "{} printed as {}", lan(act), text);
    let mut t = s.clone();
    for m in act.moves() {
        let mt = move_to_pgn(&t, m);
        assert_eq!(parse_move(&t, &mt).unwrap_or_else(|e| panic!("{}: {}", mt, e)), *m, "{}", mt);
        assert!(t.apply_ext::<false>(*m));
    }
}

#[test]
fn parses_repo_files() {
    let files = data_files();
    assert!(files.len() >= 8);
    for f in &files {
        let g = parse_file(f);
        assert!(!g.root.mainline().is_empty(), "{}", f.display());
    }
}

#[test]
fn expert_m7_matches_canonical_moves() {
    let g = parse_file(&Path::new(DATA).join("expert_m7.5dpgn"));
    let got: String = g.root.mainline().iter().map(|a| lan(a) + " submit ").collect();
    let want = "(0T1)h2g1 submit (0T1)a8b8 submit (0T2)g1d4 submit (0T2)b8c8 submit (0T3)d4c3 submit \
                (0T3)b7(0T1)c7 submit (0T4)c3(0T1)c6 submit (0T4)c8(0T3)b8 (1T1)a8(0T2)a8 submit \
                (1T2)c6(0T1)c6 submit (2T1)a8(1T2)a8 submit (2T2)h2b8 (-1T2)a6(0T3)b5 submit \
                (-1T2)c7(1T1)c7 submit (-4T2)h2c7 submit ";
    assert_eq!(got, want);
    assert!(g.headers.is_empty());
}

#[test]
fn tree_structure_comments_and_result() {
    let g = parse_file(&Path::new(DATA).join("softmate.5dpgn"));
    assert_eq!(g.result.as_deref(), Some("1-0"));
    assert_eq!(g.headers[0], ("Mode".to_string(), "5D".to_string()));
    // 2w. Bb5 {Beware!} (2b. d5 {The right response}) 2b. c6
    let mut n = &g.root;
    for _ in 0..3 {
        n = &n.children[0];
    }
    assert_eq!(n.text, "Bb5");
    assert_eq!(n.comments, vec!["Beware!".to_string()]);
    assert_eq!(n.children.len(), 2);
    assert_eq!(n.children[0].text, "c6");
    assert_eq!(n.children[1].text, "d5");
    assert_eq!(n.children[1].comments, vec!["The right response".to_string()]);
    // 5. Q>>xf7+~ (~T1) (>L1) {f7-sacrifice!}
    for _ in 0..6 {
        n = &n.children[0];
    }
    assert_eq!(n.text, "Q>>xf7+~ (~T1) (>L1)");
    assert_eq!(n.comments, vec!["f7-sacrifice!".to_string()]);
    let all = all_actions(&g);
    // The variation at move 10 and the main line both reach move 12.
    assert_eq!(all.len(), 2 * 8 + 2 + 5 + 3 + 1);
    // 8. (1T4)Qh5* is a softmate.
    let (s, a, w) = &all.iter().find(|(_, a, _)| lan(a) == "(1T4)d1h5").unwrap();
    assert_eq!(action_to_pgn(s, a), "(1T4)Qd1h5*");
    assert_eq!(action_to_pgn_with_witness(s, a, *w), "(1T4)Qd1h5*");
}

#[test]
fn printer_notation() {
    let g = parse_file(&Path::new(DATA).join("standard.5dpgn"));
    let all = all_actions(&g);
    let printed: Vec<String> = all.iter().map(|(s, a, _)| action_to_pgn(s, a)).collect();
    assert_eq!(printed[0], "(0T1)e2e3");
    assert_eq!(printed[1], "(0T1)Ng8f6");
    assert_eq!(printed[8], "(0T5)Qb3>>x(0T1)f7+~ (>L1)");
    assert_eq!(printed[9], "(1T1)Ke8xf7");
    assert_eq!(printed[13], "(1T3)Qd8h4");
}

#[test]
fn every_action_round_trips() {
    for f in data_files() {
        let g = parse_file(&f);
        for (s, a, w) in all_actions(&g) {
            check_round_trip(&s, a, w);
        }
    }
}

#[test]
fn legal_actions_round_trip() {
    // Up to 60 legal actions from every fourth position of each main line.
    for f in data_files() {
        let g = parse_file(&f);
        let mut s = state_from_setup(&g.setup).unwrap();
        for (i, a) in g.root.mainline().into_iter().enumerate() {
            if i % 4 == 0 {
                let (hc, ss) = HcInfo::build_hc(&s);
                for mvs in hc.search(&s, ss).take(60) {
                    check_round_trip(&s, &Action::from_moveseq(&mvs, &s), None);
                }
            }
            s = s.can_apply_action(a).unwrap();
        }
    }
}

#[test]
fn write_round_trips() {
    for f in data_files() {
        let g = parse_file(&f);
        let main: Vec<Action> = g.root.mainline().into_iter().cloned().collect();
        let mut headers = g.headers.clone();
        headers.push(("Result".into(), "0-1".into()));
        headers.dedup_by(|a, b| a.0 == b.0);
        let text = write_5dpgn(&headers, &g.setup, &main);
        let back = parse_5dpgn(&text, None).unwrap_or_else(|e| panic!("{}: {}\n{}", f.display(), e, text));
        let back_main: Vec<Action> = back.root.mainline().into_iter().cloned().collect();
        assert_eq!(back_main, main, "{}", f.display());
        assert_eq!(setup_headers(&back.setup), setup_headers(&g.setup));
        assert_eq!(back.result.is_some(), headers.iter().any(|(k, v)| k == "Result" && v == "0-1"));
        for (k, v) in &g.headers {
            if !["Board", "Variant", "Mode", "Size", "Timeline", "Promotions", "Result"].contains(&k.as_str()) {
                assert!(back.headers.contains(&(k.clone(), v.clone())), "{}: lost header {}", f.display(), k);
            }
        }
    }
}

#[test]
fn custom_setup_headers() {
    let h = setup_headers(&m7_setup());
    assert_eq!(h[0], ("Board".into(), "Custom - Odd".into()));
    assert_eq!(h.last().unwrap(), &(String::new(), "k7/p*n6/K7/8/8/8/6P*B/8:0:1:w".to_string()));
    let text = write_5dpgn(&[], &m7_setup(), &[]);
    let g = parse_5dpgn(&text, None).unwrap();
    assert_eq!(setup_headers(&g.setup), h);
}

#[test]
fn syntax_variants() {
    // Missing spaces, comments with braces, parentheses and newlines, turn markers 2w./2b.
    let g = parse_5dpgn(
        "[Variant \"Standard - Turn Zero\"]\n1.(0T1)Ng1f3/(0T1)Ng8f6 {a (comment)\n{nested} [x]}\n2w. e3 2b. (0T2)e6\n3.(0T3)Bf1c4/Nc6 4. O-O / a6 1/2-1/2",
        None,
    )
    .unwrap();
    let main = g.root.mainline();
    assert_eq!(main.len(), 8);
    assert_eq!(lan(main[6]), "(0T4)e1g1");
    assert_eq!(g.root.children[0].children[0].comments, vec!["a (comment)\n{nested} [x]".to_string()]);
    assert_eq!(g.result.as_deref(), Some("1/2-1/2"));
    // Queenside castling, promotion with and without '='.
    let g = parse_5dpgn("[Board \"Custom - Odd\"]\n[Size \"8x8\"]\n[r*3k*2r*/8/8/8/8/8/8/R*3K*3:0:1:w]\n1. O-O-O / O-O", None).unwrap();
    let main = g.root.mainline();
    assert_eq!(lan(main[0]), "(0T1)e1c1");
    assert_eq!(lan(main[1]), "(0T1)e8g8");
    let promo = "[Board \"Custom - Odd\"]\n[Promotions \"QN\"]\n[r6k/1P6/8/8/8/8/8/K7:0:1:w]\n1. ";
    let g = parse_5dpgn(&format!("{}bxa8=N", promo), None).unwrap();
    assert_eq!(lan(g.root.mainline()[0]), "(0T1)b7a8N");
    let g = parse_5dpgn(&format!("{}b8N", promo), None).unwrap();
    assert_eq!(lan(g.root.mainline()[0]), "(0T1)b7b8N");
    let g = parse_5dpgn(&format!("{}b8", promo), None).unwrap();
    assert_eq!(lan(g.root.mainline()[0]), "(0T1)b7b8Q");
    assert!(parse_5dpgn(&format!("{}b8=R", promo), None).is_err());
    let s = state_from_setup(&g.setup).unwrap();
    assert_eq!(action_to_pgn(&s, g.root.mainline()[0]), "(0T1)b7b8=Q+");
    // A bare move list defaults to Standard.
    let g = parse_5dpgn("1. e4 / e5", None).unwrap();
    assert_eq!(lan(g.root.mainline()[1]), "(0T1)e7e5");
}

#[test]
fn even_timelines() {
    let g = parse_5dpgn(
        "[Variant \"Standard - Two Timelines\"]\n1. (-0T1)Ng1f3 (+0T1)e2e3 / (+0T1)Ng8f6 (-0T1)Ng8f6\n2. (-0T2)Nf3>>(-0T1)f5 (+0T2)Qd1f3",
        None,
    )
    .unwrap();
    let all = all_actions(&g);
    assert_eq!(all.len(), 3);
    assert_eq!(action_to_pgn(&all[0].0, all[0].1), "(-0T1)Ng1f3 (+0T1)e2e3");
    assert_eq!(action_to_pgn(&all[2].0, all[2].1), "(+0T2)Qd1f3 (-0T2)Nf3>>(-0T1)f5~ (>L+1)");
    for (s, a, w) in &all {
        check_round_trip(s, a, *w);
    }
    // (0T1) does not name a timeline of an even layout.
    assert!(parse_5dpgn("[Variant \"Standard - Two Timelines\"]\n1. (0T1)Ng1f3 (+0T1)e3", None).is_err());
}

#[test]
fn errors_name_the_move() {
    let e = parse_5dpgn("[Board \"Standard\"]\n1. e4 / e5 2. Nd5", None).unwrap_err();
    assert!(e.contains("Nd5") && e.contains("T2"), "{}", e);
    let e = parse_5dpgn("[Board \"Custom - Odd\"]\n[k7/8/8/8/8/8/8/KN3N2:0:1:w]\n1. Nd2", None).unwrap_err();
    assert!(e.contains("Ambiguous") && e.contains("Nd2"), "{}", e);
    assert!(parse_5dpgn("[Board \"Standard\"]\n1. e4 / e5 3. Nf3", None).unwrap_err().contains("non-consecutive"));
    assert!(parse_5dpgn("[Board \"Standard\"]\n1. e4 {open", None).is_err());
    assert!(parse_5dpgn("[Board \"Nonsense\"]\n1. e4", None).unwrap_err().contains("Unknown variant"));
}

/// Walks `$PGN_CORPUS` (default: the C++ engine's test/pgn) and parses, prints and re-parses
/// every game: `cargo test --test pgn -- --ignored --nocapture`.
#[test]
#[ignore]
fn corpus() {
    let root = std::env::var("PGN_CORPUS").unwrap_or_else(|_| r"C:\Users\Jesse\Documents\Coding\Rust\5Dchess_engine\5dchess_engine\test\pgn".into());
    let mut files = Vec::new();
    let mut dirs = vec![PathBuf::from(&root)];
    while let Some(d) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd {
            let p = e.unwrap().path();
            if p.is_dir() {
                dirs.push(p);
            } else if p.extension().is_some_and(|e| e == "5dpgn") {
                files.push(p);
            }
        }
    }
    files.sort();
    let (mut ok, mut actions, mut failed) = (0, 0, Vec::new());
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_else(|_| String::from_utf8_lossy(&std::fs::read(f).unwrap()).into_owned());
        let t0 = std::time::Instant::now();
        let parsed = parse_5dpgn(&text, None);
        let t1 = t0.elapsed();
        if std::env::var("PGN_TIMING").is_ok() {
            eprintln!("{} parse {:?}", f.display(), t1);
        }
        match parsed {
            Ok(g) => {
                ok += 1;
                let all = all_actions(&g);
                actions += all.len();
                for (s, a, w) in &all {
                    let t = std::time::Instant::now();
                    if std::env::var("PGN_TIMING").is_ok() {
                        eprintln!("  start {} moves, {:?}", a.len(), s.get_present());
                    }
                    check_round_trip(s, a, *w);
                    if std::env::var("PGN_TIMING").is_ok() && t.elapsed().as_millis() > 100 {
                        eprintln!("  slow {:?}: {}", t.elapsed(), action_to_pgn_with_witness(s, a, *w));
                    }
                }
                let main: Vec<Action> = g.root.mainline().into_iter().cloned().collect();
                let back = parse_5dpgn(&write_5dpgn(&g.headers, &g.setup, &main), None).unwrap();
                assert_eq!(back.root.mainline().into_iter().cloned().collect::<Vec<_>>(), main, "{}", f.display());
            }
            Err(e) => failed.push(format!("{}: {}", f.display(), e)),
        }
    }
    println!("{} files, {} parsed ({} actions, all round-tripped), {} failed", files.len(), ok, actions, failed.len());
    for f in &failed {
        println!("  {}", f);
    }
    assert!(!files.is_empty(), "no corpus at {}", root);
    let allowed = std::env::var("PGN_CORPUS_MAX_FAILURES").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    assert!(failed.len() <= allowed, "{} corpus files failed", failed.len());
}
