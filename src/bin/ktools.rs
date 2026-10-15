//! Command-line utilities: perft, legal-action dumps and node-count benchmarks.
//!
//! Positions are read as "G <promotions> <5duci position> moves <lan|submit>*" lines
//! (the format emitted by `oracle lines`).

use kairos::core::action::MoveSeq;
use kairos::core::hypercuboid::HcInfo;
use kairos::core::perft::{parse_g_line, perft};
use kairos::core::state::{MateType, State};
use std::io::{BufRead, Write};
use std::time::Instant;

fn seq_string(mv: &MoveSeq) -> String {
    mv.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(" ")
}

fn mate_name(m: MateType) -> &'static str {
    match m {
        MateType::None => "none",
        MateType::Checkmate => "checkmate",
        MateType::Softmate => "softmate",
        MateType::Stalemate => "stalemate",
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("help");
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    match mode {
        "dump" => {
            let cap: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2000);
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                let s = match parse_g_line(&line) {
                    Ok(s) => s,
                    Err(e) => {
                        writeln!(out, "{}\nE {}", line, e).unwrap();
                        continue;
                    }
                };
                let (w, ss) = HcInfo::build_hc(&s);
                let acts: Vec<String> = w.search(&s, ss).take(cap).map(|m| seq_string(&m)).collect();
                writeln!(out, "{}", line).unwrap();
                writeln!(out, "M {} {}", mate_name(s.get_mate_type()), acts.len()).unwrap();
                for a in &acts {
                    writeln!(out, "a {}", a).unwrap();
                }
            }
        }
        "perft" => {
            let depth: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
            let cap: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                let s = parse_g_line(&line).expect("bad position");
                let t0 = Instant::now();
                let n = perft(&s, depth, cap);
                writeln!(out, "{} {}", n, t0.elapsed().as_secs_f64()).unwrap();
                out.flush().unwrap();
            }
        }
        "blunders" => blunders(
            args.get(2).map(|s| s.as_str()).unwrap_or("games/sprt/games_hce_v8.jsonl"),
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(100),
            &mut out,
        ),
        "materank" => materank(
            args.get(2).map(|s| s.as_str()).unwrap_or("games/sprt/games_hce_v8.jsonl"),
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(300),
            &mut out,
        ),
        "rootgen" => {
            use kairos::engine::ab::AlphaBeta;
            let path = args.get(2).map(|s| s.as_str()).unwrap_or("games/bench_ab.g");
            let text = std::fs::read_to_string(path).expect("positions");
            for (k, line) in text.lines().filter(|l| l.starts_with("G ")).enumerate() {
                let s = parse_g_line(line).unwrap();
                let e = AlphaBeta::new();
                let t = Instant::now();
                let n = e.node_order(&s, 4000).len();
                let (w, _) = HcInfo::build_hc(&s);
                writeln!(out, "pos {} dim {} actions {} in {:.1}ms", k, w.dimension(), n, t.elapsed().as_secs_f64() * 1e3).unwrap();
            }
        }
        "depthstat" => {
            use kairos::engine::ab::AlphaBeta;
            use kairos::engine::Limits;
            let path = args.get(2).map(|s| s.as_str()).unwrap_or("games/sprt/games_timeline_scaled_300.jsonl");
            let ms: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(100);
            let positions = mate_positions(path, 60);
            let stop = std::sync::atomic::AtomicBool::new(false);
            let mut hist = std::collections::BTreeMap::new();
            for p in &positions {
                let mut e = AlphaBeta::new();
                let mut last = String::new();
                e.find_best_move(p, &Limits { movetime: Some(ms), ..Default::default() }, &stop, &mut |m| last = m);
                let d: u32 = last.split_whitespace().nth(1).and_then(|x| x.parse().ok()).unwrap_or(0);
                let roots: u32 = last.split_whitespace().skip_while(|w| *w != "roots").nth(1).and_then(|x| x.parse().ok()).unwrap_or(0);
                *hist.entry(d).or_insert(0) += 1;
                writeln!(out, "depth {} roots {} nodes {}", d, roots, last.split_whitespace().skip_while(|w| *w != "nodes").nth(1).unwrap_or("?")).unwrap();
            }
            writeln!(out, "{:?}", hist).unwrap();
        }
        "abbench" => abbench(
            args.get(2).map(|s| s.as_str()).unwrap_or("data/bench_ab.g"),
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(20),
            &mut out,
        ),
        "naive" => {
            use kairos::core::naive::{is_legal_action, naive_actions};
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                let s = parse_g_line(&line).unwrap();
                let naive: std::collections::BTreeSet<String> = naive_actions(&s, 1_000_000).iter().map(seq_string).collect();
                let legal_naive = naive_actions(&s, 1_000_000).iter().filter(|a| is_legal_action(&s, a)).count();
                let (w, ss) = HcInfo::build_hc(&s);
                let hc: Vec<MoveSeq> = w.search(&s, ss).collect();
                let hc_legal = hc.iter().filter(|a| is_legal_action(&s, a)).count();
                let hcs: std::collections::BTreeSet<String> = hc.iter().map(seq_string).collect();
                writeln!(out, "naive {} (validated legal {}) | hc {} (legal {}) | hc-only {} naive-only {}",
                    naive.len(), legal_naive, hcs.len(), hc_legal, hcs.difference(&naive).count(), naive.difference(&hcs).count()).unwrap();
                if std::env::var("SHOW").is_ok() {
                    for a in naive.difference(&hcs).take(10) {
                        writeln!(out, "  naive-only {}", a).unwrap();
                    }
                }
            }
        }
        "slicecheck" => {
            use kairos::core::hypercuboid::Entry;
            use kairos::core::naive::naive_actions;
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                let s = parse_g_line(&line).unwrap();
                let (w, mut ss) = HcInfo::build_hc(&s);
                let dim = w.dimension();
                let nb = w.first_branch_axis();
                // Map every legal action to its point.
                let mut targets: Vec<(String, Vec<u32>)> = Vec::new();
                for act in naive_actions(&s, 100000) {
                    let mut pt = vec![0u32; dim];
                    let mut ok = true;
                    let mut branch_k = 0;
                    for m in &act {
                        let phys = m.from.tl() == m.to.tl();
                        let nonbranch = !phys && s.get_timeline_end(m.to.l()) == (m.to.t(), s.get_present().1)
                            && !act.iter().any(|o| o.from.l() == m.to.l() && o != m);
                        let find = |n: usize, f: &dyn Fn(&Entry) -> bool| (0..w.axis_len(n) as u32).find(|&i| f(w.entry(n, i)));
                        if phys {
                            let n = w.line_to_axis(m.from.l()).unwrap();
                            match find(n, &|e| matches!(e, Entry::Physical { m: x, .. } if x == m)) { Some(i) => pt[n] = i, None => ok = false }
                        } else {
                            let nf = w.line_to_axis(m.from.l()).unwrap();
                            match find(nf, &|e| matches!(e, Entry::Departing { from, .. } if *from == m.from)) { Some(i) => pt[nf] = i, None => ok = false }
                            let na = if nonbranch { w.line_to_axis(m.to.l()).unwrap() } else { branch_k += 1; nb + branch_k - 1 };
                            match find(na, &|e| matches!(e, Entry::Arriving { m: x, .. } if x == m)) { Some(i) => pt[na] = i, None => ok = false }
                        }
                    }
                    if ok { targets.push((seq_string(&act), pt)); } else { writeln!(out, "unmapped {}", seq_string(&act)).unwrap(); }
                }
                writeln!(out, "{} targets mapped, dim {} new_axis {}", targets.len(), dim, nb).unwrap();
                let order = kairos::misc::ordering::HcOrdering::Natural;
                let mut yielded = std::collections::HashSet::new();
                let mut alive: Vec<bool> = targets.iter().map(|t| ss.contains(&t.1)).collect();
                writeln!(out, "initially contained: {}", alive.iter().filter(|&&a| a).count()).unwrap();
                let mut step = 0;
                while !ss.is_empty() && step < 100000 {
                    step += 1;
                    let mut hc = ss.pop_back().unwrap();
                    let before = ss.clone();
                    let hc0 = hc.clone();
                    let Some(pt) = w.take_point(&mut hc, &order) else {
                        for (k, t) in targets.iter().enumerate() {
                            if alive[k] && hc0.contains(&t.1) && !before.contains(&t.1) { writeln!(out, "step {} take_point dropped {}", step, t.0).unwrap(); alive[k] = false; }
                        }
                        continue;
                    };
                    for (k, t) in targets.iter().enumerate() {
                        if alive[k] && hc0.contains(&t.1) && !hc.contains(&t.1) && !before.contains(&t.1) { writeln!(out, "step {} take_point pruning dropped {}", step, t.0).unwrap(); alive[k] = false; }
                    }
                    let (kind, prob) = if let Some(p) = w.jump_order_consistent(&s, &pt, &hc) { ("jump_order", Some(p)) }
                        else if let Some(p) = w.test_present(&s, &pt, &hc) { ("test_present", Some(p)) }
                        else if let Some(p) = w.find_checks(&s, &pt, &hc) { ("find_checks", Some(p)) } else { ("ok", None) };
                    match prob {
                        None => { yielded.insert(pt.clone()); ss.concat(hc.remove_point(&pt)); }
                        Some(problem) => {
                            ss.push_back(hc.clone());
                            ss.remove_slice_backwards(&problem, true, 10, 1);
                            for (k, t) in targets.iter().enumerate() {
                                if alive[k] && !ss.contains(&t.1) && !yielded.contains(&t.1) && problem.contains(&t.1) {
                                    writeln!(out, "step {} {} slice dropped {} (point {:?}; slice {:?})", step, kind, t.0, pt, problem.fixed).unwrap();
                                    alive[k] = false;
                                }
                            }
                        }
                    }
                    for (k, t) in targets.iter().enumerate() {
                        if alive[k] && !ss.contains(&t.1) && !yielded.contains(&t.1) { writeln!(out, "step {} lost (other) {}", step, t.0).unwrap(); alive[k] = false; }
                    }
                }
                writeln!(out, "yielded {} points in {} steps", yielded.len(), step).unwrap();
            }
        }
        "explain" => {
            // explain <action moves...> : timeline geometry before/after an action
            let act: Vec<kairos::core::action::FullMove> =
                args[2..].iter().map(|t| kairos::core::action::FullMove::parse(t).unwrap()).collect();
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                let s = parse_g_line(&line).unwrap();
                let show = |s: &kairos::core::state::State, out: &mut dyn Write| {
                    let (a, b) = s.get_lines_range();
                    writeln!(out, "  present {:?} apparent {:?} lines {}..{} active {:?}", s.get_present(), s.apparent_present(), a, b, s.get_active_range()).unwrap();
                    for l in a..=b {
                        writeln!(out, "    L{} start {:?} end {:?}", l, s.get_timeline_start(l), s.get_timeline_end(l)).unwrap();
                    }
                };
                show(&s, &mut out);
                let ts = s.get_timeline_status();
                writeln!(out, "  mandatory {:?} optional {:?} unplayable {:?}", ts.mandatory, ts.optional, ts.unplayable).unwrap();
                let mut c = s.clone();
                for &m in &act {
                    let ok = c.apply_move::<false>(m);
                    writeln!(out, "  apply {} -> {} ; present {:?} apparent {:?} active {:?}", m, ok, c.get_present(), c.apparent_present(), c.get_active_range()).unwrap();
                }
                let sub = c.submit::<false>();
                writeln!(out, "  submit {} ; check {:?}", sub, c.first_check(c.get_present().1)).unwrap();
                show(&c, &mut out);
            }
        }
        "validate" => {
            // HC search (several orderings/modes) vs exhaustive naive search.
            use kairos::core::hypercuboid::HcCursor;
            use kairos::core::naive::naive_actions_budget;
            let limit: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(20000);
            let (mut checked, mut skipped, mut bad) = (0, 0, 0);
            for (idx, line) in stdin.lock().lines().enumerate() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                let s = parse_g_line(&line).unwrap();
                let Some(naive) = naive_actions_budget(&s, limit + 1, 200_000) else {
                    skipped += 1;
                    continue;
                };
                if naive.len() > limit {
                    skipped += 1;
                    continue;
                }
                let want: std::collections::BTreeSet<String> = naive.iter().map(seq_string).collect();
                checked += 1;
                let mut results = Vec::new();
                for variant in 0..5u64 {
                    let (w, ss) = HcInfo::build_hc(&s);
                    if variant == 2 {
                        // The engine's best-first enumeration.
                        let acts = kairos::engine::ab::AlphaBeta::new().node_order(&s, limit * 2 + 1);
                        results.push(acts.iter().map(seq_string).collect());
                        continue;
                    }
                    let order = match variant {
                        0 => kairos::misc::ordering::HcOrdering::Natural,
                        1 => kairos::engine::ab::AlphaBeta::new().ordering(&s, &w),
                        v => shuffled_order(&w, v),
                    };
                    let mut cur = HcCursor::new(ss, order);
                    let mut got = std::collections::BTreeSet::new();
                    while let Some(a) = cur.next(&w, &s) {
                        got.insert(seq_string(&a));
                        if got.len() > limit * 2 { break; }
                    }
                    results.push(got);
                }
                if results.iter().any(|g| *g != want) {
                    bad += 1;
                    let counts: Vec<usize> = results.iter().map(|g| g.len()).collect();
                    let extra: Vec<usize> = results.iter().map(|g| g.difference(&want).count()).collect();
                    writeln!(out, "MISMATCH pos {} naive {} hc {:?} extra {:?}
{}", idx, want.len(), counts, extra, line).unwrap();
                    out.flush().unwrap();
                }
            }
            writeln!(out, "checked {} skipped {} mismatched {}", checked, skipped, bad).unwrap();
        }
        "ordercheck" => {
            // Legal-action counts under the natural, engine and shuffled orders must agree.
            use kairos::core::hypercuboid::HcCursor;
            let cap: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(20000);
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                let s = parse_g_line(&line).unwrap();
                let (w, ss) = HcInfo::build_hc(&s);
                let natural = w.search(&s, ss).take(cap).count();
                let best_first = kairos::engine::ab::AlphaBeta::new().node_order(&s, cap).len();
                let mut shuffled = Vec::new();
                for seed in 1..=6u64 {
                    let (w2, ss2) = HcInfo::build_hc(&s);
                    let mut cur = HcCursor::new(ss2, shuffled_order(&w2, seed));
                    let mut k = 0;
                    while k < cap && cur.next(&w2, &s).is_some() {
                        k += 1;
                    }
                    shuffled.push(k);
                }
                writeln!(out, "natural {} best-first {} shuffled {:?} dim {} new_axis {}", natural, best_first, shuffled, w.dimension(), w.first_branch_axis()).unwrap();
            }
        }
        "bench" => bench(args.get(2).map(|s| s.as_str()).unwrap_or("data/bench.g"), &mut out),
        _ => {
            eprintln!(
                "usage: ktools <mode> ...\n  \
                 perft <depth> [cap] | dump [cap] | naive | validate [limit] | ordercheck [cap] | slicecheck | explain <moves>\n  \
                 \x20 (positions as G lines on stdin)\n  \
                 bench [file]            perft oracle\n  \
                 abbench [file] [depth]  alpha-beta node oracle (AB_NODES caps nodes per position)\n  \
                 blunders <games.jsonl> [n] | materank <games.jsonl> [n] | depthstat <games.jsonl> [ms] | rootgen [file]"
            );
        }
    }
}

/// Move generation oracle + speed benchmark: capped perft over the bench positions. The
/// checksum must not change for pure speed optimizations.
fn bench(path: &str, out: &mut impl Write) {
    use std::hash::{Hash, Hasher};
    let text = std::fs::read_to_string(path).expect("bench positions");
    let positions: Vec<State> = text.lines().filter(|l| l.starts_with("G ")).map(|l| parse_g_line(l).unwrap()).collect();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let t0 = Instant::now();
    let mut leaves = 0u64;
    for s in &positions {
        let n = perft(s, 2, 50);
        n.hash(&mut h);
        leaves += n;
    }
    let secs = t0.elapsed().as_secs_f64();
    writeln!(out, "oracle {:016x} | perft leaves {} in {:.3}s ({:.0}/s)", h.finish(), leaves, secs, leaves as f64 / secs).unwrap();
    write!(out, "{}", kairos::misc::prof::report()).unwrap();
}

/// Alpha-beta node oracle: fixed-depth searches; checksum covers node counts and best moves.
fn abbench(path: &str, depth: u32, out: &mut impl Write) {
    use kairos::engine::ab::AlphaBeta;
    use kairos::engine::Limits;
    use std::hash::{Hash, Hasher};
    let text = std::fs::read_to_string(path).expect("bench positions");
    let positions: Vec<State> = text.lines().filter(|l| l.starts_with("G ")).map(|l| parse_g_line(l).unwrap()).collect();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let stop = std::sync::atomic::AtomicBool::new(false);
    let t0 = Instant::now();
    let mut total_nodes = 0u64;
    for (k, s) in positions.iter().enumerate() {
        let mut e = AlphaBeta::new();
        let node_cap = std::env::var("AB_NODES").ok().and_then(|v| v.parse().ok());
        let limits = Limits { depth: Some(depth), nodes: node_cap, ..Default::default() };
        let mut last = String::new();
        let t = Instant::now();
        let best = e.find_best_move(s, &limits, &stop, &mut |m| last = m);
        best.map(|a| a.to_string()).hash(&mut h);
        let nodes: u64 = last.split_whitespace().skip_while(|w| *w != "nodes").nth(1).and_then(|x| x.parse().ok()).unwrap_or(0);
        nodes.hash(&mut h);
        total_nodes += nodes;
        if std::env::var("BENCH_VERBOSE").is_ok() {
            eprintln!("pos {} nodes {} {:.2}s", k, nodes, t.elapsed().as_secs_f64());
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    writeln!(out, "aboracle {:016x} | depth {} | nodes {} in {:.3}s ({:.0} nps)", h.finish(), depth, total_nodes, secs, total_nodes as f64 / secs).unwrap();
    write!(out, "{}", kairos::misc::prof::report()).unwrap();
}

fn json_field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{}\":", key);
    let i = line.find(&pat)? + pat.len();
    let rest = line[i..].trim_start();
    if let Some(r) = rest.strip_prefix('"') {
        Some(&r[..r.find('"')?])
    } else {
        let end = rest.find(|c| c == ',' || c == '}').unwrap_or(rest.len());
        Some(rest[..end].trim())
    }
}

fn apply_seq(s: &State, a: &MoveSeq) -> Option<State> {
    let mut c = s.clone();
    for fm in a {
        if !c.apply_move::<false>(*fm) {
            return None;
        }
    }
    if c.submit::<false>() { Some(c) } else { None }
}

fn is_mated(s: &State) -> bool {
    let (w, ss) = HcInfo::build_hc(s);
    w.search(s, ss).next().is_none() && s.has_phantom_check()
}

/// Does the side to move have a mating action among its first `cap` actions?
fn has_mate_in_1(s: &State, cap: usize) -> bool {
    let (w, ss) = HcInfo::build_hc(s);
    w.search(s, ss).take(cap).any(|a| apply_seq(s, &a).map(|c| is_mated(&c)).unwrap_or(false))
}

/// For checkmate games: replays to the loser's last decision and asks whether alpha-beta with
/// various node budgets still plays into a mate in one.
fn blunders(path: &str, max_games: usize, out: &mut impl Write) {
    use kairos::core::variants::{apply_uci_moves, position_from_uci};
    use kairos::engine::ab::AlphaBeta;
    use kairos::engine::Limits;
    let no_safety = std::env::var("NO_SAFETY").is_ok();
    let text = std::fs::read_to_string(path).expect("games");
    let mut positions: Vec<State> = Vec::new();
    for line in text.lines() {
        if positions.len() >= max_games {
            break;
        }
        if json_field(line, "reason") != Some("checkmate") {
            continue;
        }
        let (Some(pos), Some(moves)) = (json_field(line, "position"), json_field(line, "moves")) else { continue };
        let acts: Vec<&str> = moves.split(" submit").map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
        if acts.len() < 4 {
            continue;
        }
        let Ok(mut s) = position_from_uci(pos) else { continue };
        let mut ok = true;
        for a in &acts[..acts.len() - 2] {
            if apply_uci_moves(&mut s, &format!("{} submit", a)).is_err() {
                ok = false;
                break;
            }
        }
        if ok {
            positions.push(s);
        }
    }
    let budgets: Vec<u64> = std::env::var("BUDGETS").ok().map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or(vec![5_000, 50_000, 500_000]);
    let budgets = &budgets;
    let results: Vec<Vec<Option<bool>>> = std::thread::scope(|sc| {
        let hs: Vec<_> = positions
            .chunks(positions.len().div_ceil(14).max(1))
            .map(|chunk| {
                sc.spawn(move || {
                    let stop = std::sync::atomic::AtomicBool::new(false);
                    chunk
                        .iter()
                        .map(|p| {
                            budgets
                                .iter()
                                .map(|&n| {
                                    let mut e = AlphaBeta::new();
                                    let limits = Limits { depth: Some(60), nodes: Some(n), ..Default::default() };
                                    let a = e.find_best_move(p, &limits, &stop, &mut |_| {})?;
                                    let q = p.can_apply_action(&a)?;
                                    Some(has_mate_in_1(&q, 3000))
                                })
                                .chain(std::iter::once({
                                    // Is any action safe from mate in one? (first 300 actions)
                                    if no_safety {
                                        Some(true)
                                    } else {
                                        let (w, ss) = HcInfo::build_hc(p);
                                        let safe = w.search(p, ss).take(300).any(|a| {
                                            apply_seq(p, &a).map(|q| !has_mate_in_1(&q, 3000)).unwrap_or(false)
                                        });
                                        Some(safe)
                                    }
                                }))
                                .collect()
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let lost = results.iter().filter(|v| v[budgets.len()] == Some(false)).count();
    writeln!(out, "positions with no mate-in-1-safe action (first 300): {}/{}", lost, results.len()).unwrap();
    for (k, &n) in budgets.iter().enumerate() {
        let avoidable = results.iter().filter(|v| v[k] == Some(true) && v[budgets.len()] == Some(true)).count();
        writeln!(out, "nodes {:7}: avoidable mate-in-1 blunders {}", n, avoidable).unwrap();
        let r: Vec<Option<bool>> = results.iter().map(|v| v[k]).collect();
        let blund = r.iter().filter(|x| **x == Some(true)).count();
        let none = r.iter().filter(|x| x.is_none()).count();
        writeln!(out, "nodes {:7}: still mated-in-1 {}/{} (no action {})", n, blund, r.len(), none).unwrap();
    }
}

/// Positions just before a game-ending mate (winner to move), from harness JSONL.
fn mate_positions(path: &str, max_games: usize) -> Vec<State> {
    use kairos::core::variants::{apply_uci_moves, position_from_uci};
    let text = std::fs::read_to_string(path).expect("games");
    let mut positions = Vec::new();
    for line in text.lines() {
        if positions.len() >= max_games {
            break;
        }
        if json_field(line, "reason") != Some("checkmate") {
            continue;
        }
        let (Some(pos), Some(moves)) = (json_field(line, "position"), json_field(line, "moves")) else { continue };
        let acts: Vec<&str> = moves.split(" submit").map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
        let Ok(mut s) = position_from_uci(pos) else { continue };
        if acts[..acts.len() - 1].iter().all(|a| apply_uci_moves(&mut s, &format!("{} submit", a)).is_ok()) {
            positions.push(s);
        }
    }
    positions
}

/// Rank of the first mating action in the alpha-beta node ordering, for positions where the
/// side to move mated in the game. `opts` are `key=value` engine options.
fn materank(path: &str, max_games: usize, out: &mut impl Write) {
    use kairos::engine::ab::AlphaBeta;

    let positions = mate_positions(path, max_games);
    let ranks: Vec<Option<usize>> = std::thread::scope(|sc| {
        let hs: Vec<_> = positions
            .chunks(positions.len().div_ceil(14).max(1))
            .map(|chunk| {
                sc.spawn(move || {
                    let e = AlphaBeta::new();
                    chunk
                        .iter()
                        .map(|p| e.node_order(p, 2000).iter().position(|a| apply_seq(p, a).map(|c| is_mated(&c)).unwrap_or(false)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let n = ranks.len();
    let within = |k: usize| ranks.iter().filter(|r| r.map(|x| x < k).unwrap_or(false)).count();
    writeln!(
        out,
        "positions {} | first mate within 1: {} 4: {} 16: {} 48: {} 200: {} 2000: {} | not found {}",
        n,
        within(1),
        within(4),
        within(16),
        within(48),
        within(200),
        within(2000),
        ranks.iter().filter(|r| r.is_none()).count()
    )
    .unwrap();
}


/// A random per-axis order (for checking that the action set does not depend on order).
fn shuffled_order(w: &HcInfo, seed: u64) -> kairos::misc::ordering::HcOrdering {
    let mut st = seed.wrapping_mul(0x9E3779B97F4A7C15) | 1;
    let u = w.get_universe();
    let orders = (0..u.dimension())
        .map(|n| {
            let mut v: Vec<u32> = u[n].iter().collect();
            for i in (1..v.len()).rev() {
                st ^= st << 13;
                st ^= st >> 7;
                st ^= st << 17;
                v.swap(i, (st % (i as u64 + 1)) as usize);
            }
            v
        })
        .collect();
    kairos::misc::ordering::HcOrdering::Precomputed(orders)
}
