//! The web support code: record loading (SPRT JSON lines, 5DUCI, 5dpgn), the game tree and
//! the JSON command interface the browser uses.

use kairos::web::api::Api;
use kairos::web::json::Json;
use kairos::web::record;
use kairos::web::tree::Status;

fn call(api: &mut Api, req: &str) -> Json {
    let r = Json::parse(&api.call(req, &mut |_| {})).unwrap();
    assert_eq!(r.get("ok").bool(), Some(true), "{} -> {}", req, r);
    r
}

#[test]
fn sprt_records_replay() {
    let text = std::fs::read_to_string("tests/data/sprt_sample.jsonl").unwrap();
    let mut n = 0;
    for line in text.lines().filter(|l| l.starts_with('{')) {
        let j = Json::parse(line).unwrap();
        let mut l = record::load(line, None).unwrap_or_else(|e| panic!("{}: {}", e, line));
        let moves = j.get("moves").str().unwrap();
        let actions: Vec<&str> = moves.split("submit").map(str::trim).filter(|a| !a.is_empty()).collect();
        let main = l.game.mainline();
        assert_eq!(main.len() - 1, actions.len());
        let evals = j.get("evals").arr();
        let book = j.get("book").int().map(|b| b as usize).unwrap_or(actions.len() - evals.len());
        for (i, &id) in main[1..].iter().enumerate() {
            let node = &l.game.nodes[id];
            // Same moves as the record (the tree keeps actions in standard order).
            let mut a: Vec<&str> = node.lan.split_whitespace().collect();
            let mut b: Vec<&str> = actions[i].split_whitespace().collect();
            a.sort();
            b.sort();
            assert_eq!(a, b, "action {}", i + 1);
            assert_eq!(node.book, i < book);
            if i >= book {
                // Old records (no "eval_scale") hold internal units, shown on the reported scale.
                let rec = evals.get(i - book).and_then(|e| e.int()).map(|e| e as i32);
                let want = rec.map(|e| if j.get("eval_scale").is_null() && e.abs() < 20000 { kairos::engine::report_cp(e) } else { e });
                assert_eq!(node.eval, want);
            }
            assert!(!node.san.is_empty());
        }
        let last = *main.last().unwrap();
        let status = l.game.status(last);
        match j.get("reason").str().unwrap() {
            "checkmate" => assert_eq!(status, Status::Checkmate, "{}", line),
            // Records from before the exiled-kings rule call every stalemate a draw.
            "stalemate" => assert!(matches!(status, Status::Stalemate | Status::WhiteExiled | Status::BlackExiled), "{}", line),
            "exiled kings" => assert!(matches!(status, Status::WhiteExiled | Status::BlackExiled), "{}", line),
            _ => {}
        }
        // The start position names its variant.
        assert_ne!(l.meta.get("variant").str(), Some("Invalid"));
        n += 1;
    }
    assert!(n >= 8);
}

#[test]
fn record_eval_scales() {
    // New records are on the reported scale; old ones hold internal units; mates stay.
    let new = r#"{"position":"startpos","book":0,"moves":"(0T1)e2e4 submit (0T1)e7e5 submit (0T2)g1f3 submit","evals":[100,-20000,null],"eval_scale":4800}"#;
    // Written with the earlier 3000 scale: rescaled to the current one.
    let mid = r#"{"position":"startpos","book":0,"moves":"(0T1)e2e4 submit (0T1)e7e5 submit (0T2)g1f3 submit","evals":[160,-20000,null],"eval_scale":3000}"#;
    let old = r#"{"position":"startpos","book":0,"moves":"(0T1)e2e4 submit (0T1)e7e5 submit (0T2)g1f3 submit","evals":[4800,-20000,null]}"#;
    for text in [new, mid, old] {
        let l = record::load(text, None).unwrap();
        let evals: Vec<Option<i32>> = l.game.mainline()[1..].iter().map(|&id| l.game.nodes[id].eval).collect();
        assert_eq!(evals, vec![Some(100), Some(-20000), None]);
    }
}

#[test]
fn uci_and_ktools_lines() {
    let l = record::load("position startpos moves (0T1)e2e4 submit (0T1)e7e5 submit", None).unwrap();
    assert_eq!(l.game.mainline().len(), 3);
    assert_eq!(l.meta.get("variant").str(), Some("Standard - Turn Zero"));
    let g = "G 1 size 8x8 odd fen [k7/p*n6/K7/8/8/8/6P*B/8:0:1:w] moves (0T1)h2g1 submit (0T1)a8b8 submit";
    let l = record::load(g, None).unwrap();
    assert_eq!(l.game.mainline().len(), 3);
    assert_eq!(l.meta.get("variant").str(), Some("Custom"));
    // A bare move list uses the given start position.
    let l = record::load("(0T1)e2e4 submit", Some("size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w]")).unwrap();
    assert_eq!(l.meta.get("variant").str(), Some("Standard"));
    assert!(record::load("position startpos moves (0T1)e2e5 submit", None).is_err());
}

#[test]
fn position_shorthands() {
    // A bare board FEN: one board on L0 at T1, sized from the FEN.
    let l = record::load("k7/p*n6/K7/8/8/8/6P*B/8", None).unwrap();
    assert_eq!(l.game.start, "size 8x8 odd fen [k7/p*n6/K7/8/8/8/6P*B/8:0:1:w]");
    assert_eq!(l.game.mainline().len(), 1);
    let l = record::load("nbrk/3p*/P*3/KRBN b", None).unwrap();
    assert_eq!(l.game.start, "size 4x4 odd fen [nbrk/3p*/P*3/KRBN:0:1:b]");
    assert!(l.game.nodes[0].state.get_present().1);
    // Board tags without a size prefix, and variant names.
    let l = record::load("[k7/p*n6/K7/8/8/8/6P*B/8:0:1:w]", None).unwrap();
    assert_eq!(l.game.mainline().len(), 1);
    assert_eq!(record::load("Standard - Two Timelines", None).unwrap().meta.get("variant").str(), Some("Standard - Two Timelines"));
    // As the start position of a bare move list or a headerless 5dpgn.
    let pgn = std::fs::read_to_string("tests/data/pgn/expert_m7.5dpgn").unwrap();
    assert_eq!(record::load(&pgn, Some("k7/p*n6/K7/8/8/8/6P*B/8")).unwrap().game.mainline().len(), 14);
    assert!(record::load("(0T1)h2g1 submit", Some("k7/p*n6/K7/8/8/8/6P*B/8")).is_ok());
    // Not positions.
    assert!(record::as_position("k7/p*n6/K7 x").is_none());
    assert!(record::as_position("k7/p*n6/K9").is_none());
    assert!(record::as_position("1. e3 / Nf6").is_none());
}

#[test]
fn expert_pgn_matches_its_uci_line() {
    let pgn = std::fs::read_to_string("tests/data/pgn/expert_m7.5dpgn").unwrap();
    let pos = "size 8x8 odd fen [k7/p*n6/K7/8/8/8/6P*B/8:0:1:w]";
    let a = record::load(&pgn, Some(pos)).unwrap();
    let uci = "(0T1)h2g1 submit (0T1)a8b8 submit (0T2)g1d4 submit (0T2)b8c8 submit (0T3)d4c3 submit (0T3)b7(0T1)c7 submit (0T4)c3(0T1)c6 submit (0T4)c8(0T3)b8 (1T1)a8(0T2)a8 submit (1T2)c6(0T1)c6 submit (2T1)a8(1T2)a8 submit (2T2)h2b8 (-1T2)a6(0T3)b5 submit (-1T2)c7(1T1)c7 submit (-4T2)h2c7 submit";
    let b = record::load(&format!("position {} moves {}", pos, uci), None).unwrap();
    let (ma, mb) = (a.game.mainline(), b.game.mainline());
    assert_eq!(ma.len(), mb.len());
    for (x, y) in ma.iter().zip(&mb) {
        assert_eq!(a.game.nodes[*x].moves, b.game.nodes[*y].moves);
    }
    let mut g = a.game;
    assert_eq!(g.status(*ma.last().unwrap()), Status::Checkmate);
}

#[test]
fn pgn_variations_become_branches() {
    let pgn = std::fs::read_to_string("tests/data/pgn/softmate.5dpgn").unwrap();
    let l = record::load(&pgn, None).unwrap();
    assert!(l.game.nodes.iter().any(|n| n.children.len() > 1), "variations kept");
    // "2b. d5" uncovers check from Bb5: that variation is dropped with a warning.
    assert_eq!(l.meta.get("warnings").arr().len(), 1, "{}", l.meta);
}

#[test]
fn api_build_an_action() {
    let mut api = Api::new();
    call(&mut api, r#"{"cmd":"new","position":"Standard - Two Timelines"}"#);
    let v = call(&mut api, r#"{"cmd":"view","node":0,"draft":[]}"#);
    let v = v.get("view");
    assert_eq!(v.get("timelines").arr().len(), 2);
    let ls: Vec<i64> = v.get("timelines").arr().iter().map(|t| t.get("l").int().unwrap()).collect();
    // e2-e4 on both boards: the first alone cannot be submitted.
    let mv = |l: i64| format!(r#"{{"from":[{l},1,4,1],"to":[{l},1,4,3]}}"#);
    let t = call(&mut api, &format!(r#"{{"cmd":"targets","node":0,"draft":[],"from":[{},1,4,1]}}"#, ls[0]));
    assert_eq!(t.get("targets").arr().len(), 2);
    let r = call(&mut api, &format!(r#"{{"cmd":"check_move","node":0,"draft":[],"move":{}}}"#, mv(ls[0])));
    assert_eq!(r.get("view").get("canSubmit").bool(), Some(false));
    let r = call(&mut api, &format!(r#"{{"cmd":"check_move","node":0,"draft":[{}],"move":{}}}"#, mv(ls[0]), mv(ls[1])));
    assert_eq!(r.get("view").get("canSubmit").bool(), Some(true));
    let r = call(&mut api, &format!(r#"{{"cmd":"play","node":0,"moves":[{},{}]}}"#, mv(ls[0]), mv(ls[1])));
    let node = r.get("node").int().unwrap();
    assert_eq!(node, 1);
    // Playing the same action again finds the same node.
    let r = call(&mut api, &format!(r#"{{"cmd":"play","node":0,"moves":[{},{}]}}"#, mv(ls[1]), mv(ls[0])));
    assert_eq!(r.get("node").int(), Some(1));
    let e = call(&mut api, r#"{"cmd":"export"}"#);
    let pgn = e.get("pgn").str().unwrap().to_string();
    // The export loads back to the same action.
    let r = call(&mut api, &format!(r#"{{"cmd":"load","text":{}}}"#, Json::Str(pgn)));
    assert_eq!(r.get("mainline").arr().len(), 2);
    let uci = e.get("uci").str().unwrap();
    assert!(uci.contains(" moves ") && uci.ends_with("submit"), "{}", uci);
}

#[test]
fn api_search_resumes_a_deeper_search() {
    let mut api = Api::new();
    let pos = r#""position":"startpos moves (0T1)e2e4 submit (0T1)e7e5 submit""#;
    let depths = |api: &mut Api, req: &str| {
        let mut d = Vec::new();
        let r = Json::parse(&api.call(req, &mut |l| d.push(l.split_whitespace().nth(1).unwrap().parse::<i32>().unwrap()))).unwrap();
        assert_eq!(r.get("ok").bool(), Some(true), "{}", r);
        d
    };
    assert_eq!(depths(&mut api, &format!(r#"{{"cmd":"search",{},"depth":4,"movetime":60000}}"#, pos))[0], 1);
    // Deeper on the same position: depths 5 and 6 only.
    assert_eq!(depths(&mut api, &format!(r#"{{"cmd":"search",{},"depth":6,"movetime":60000,"resume":true}}"#, pos)), vec![5, 6]);
    // A different position starts from depth 1 even when asked to resume.
    let other = r#""position":"startpos moves (0T1)d2d4 submit""#;
    assert_eq!(depths(&mut api, &format!(r#"{{"cmd":"search",{},"depth":3,"movetime":60000,"resume":true}}"#, other))[0], 1);
}

#[test]
fn api_line_san_and_play_line() {
    let mut api = Api::new();
    call(&mut api, r#"{"cmd":"new","position":"Standard"}"#);
    let r = call(&mut api, r#"{"cmd":"line_san","node":0,"line":"(0T1)e2e4 submit (0T1)e7e5 submit (0T2)g1f3 submit"}"#);
    let acts = r.get("actions").arr();
    assert_eq!(acts.len(), 3);
    assert!(acts[2].get("san").str().unwrap().contains("f3"));
    let r = call(&mut api, r#"{"cmd":"play_line","node":0,"line":"(0T1)e2e4 submit (0T1)e7e5 submit (0T2)g1f3 submit","count":2}"#);
    assert_eq!(r.get("node").int(), Some(2));
    let s = call(&mut api, r#"{"cmd":"search","position":"startpos","depth":2,"movetime":5000}"#);
    assert!(s.get("bestmove").str().is_some());
}
