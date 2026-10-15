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
            args.get(2).map(|s| s.as_str()).unwrap_or("games/sprt/games_timeline_scaled_300.jsonl"),
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(100),
            &args[4.min(args.len())..],
            &mut out,
        ),
        "materank" => materank(
            args.get(2).map(|s| s.as_str()).unwrap_or("games/sprt/games_timeline_scaled_300.jsonl"),
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(300),
            &args[4.min(args.len())..],
            &mut out,
        ),
        "rootgen" => {
            use kairos::engine::ab::AlphaBeta;
            use kairos::engine::{Engine, OptionValue};
            let path = args.get(2).map(|s| s.as_str()).unwrap_or("games/bench_ab.g");
            let text = std::fs::read_to_string(path).expect("positions");
            for (k, line) in text.lines().filter(|l| l.starts_with("G ")).enumerate() {
                let s = parse_g_line(line).unwrap();
                for bf in [false, true] {
                    let mut e = AlphaBeta::new(Default::default(), Default::default());
                    e.set_option("best_first", &OptionValue::Bool(bf));
                    let t = Instant::now();
                    let n = e.node_order(&s, 4000).len();
                    let (w, _) = HcInfo::build_hc(&s);
                    write!(out, "pos {} dim {} bf={} actions {} in {:.1}ms | ", k, w.dimension(), bf, n, t.elapsed().as_secs_f64() * 1e3).unwrap();
                }
                writeln!(out).unwrap();
            }
        }
        "depthstat" => {
            use kairos::engine::ab::AlphaBeta;
            use kairos::engine::{Engine, Limits};
            let path = args.get(2).map(|s| s.as_str()).unwrap_or("games/sprt/games_timeline_scaled_300.jsonl");
            let ms: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(100);
            let positions = mate_positions(path, 60);
            let stop = std::sync::atomic::AtomicBool::new(false);
            let mut hist = std::collections::BTreeMap::new();
            for p in &positions {
                let mut e = AlphaBeta::new(Default::default(), Default::default());
                let mut last = String::new();
                e.find_best_move(p, &Limits { movetime: Some(ms), ..Default::default() }, &stop, &mut |m| last = m);
                let d: u32 = last.split_whitespace().nth(1).and_then(|x| x.parse().ok()).unwrap_or(0);
                let roots: u32 = last.split_whitespace().skip_while(|w| *w != "roots").nth(1).and_then(|x| x.parse().ok()).unwrap_or(0);
                *hist.entry(d).or_insert(0) += 1;
                writeln!(out, "depth {} roots {} nodes {}", d, roots, last.split_whitespace().skip_while(|w| *w != "nodes").nth(1).unwrap_or("?")).unwrap();
            }
            writeln!(out, "{:?}", hist).unwrap();
        }
        "netdata" => netdata(&args[2..], &mut out),
        "netcheck" => {
            use kairos::engine::evalnet::{Net, NI};
            let net = Net::from_bytes(&std::fs::read(&args[2]).unwrap()).expect("net");
            let raw = std::fs::read(&args[3]).unwrap();
            let f: Vec<f32> = raw.chunks_exact(4).take(4 * (NI + 4)).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
            for r in f.chunks_exact(NI + 4) {
                let x: [f32; NI] = r[..NI].try_into().unwrap();
                write!(out, "{} ", net.residual(&x)).unwrap();
            }
            writeln!(out).unwrap();
        }
        "abbench" => abbench(
            args.get(2).map(|s| s.as_str()).unwrap_or("data/bench.g"),
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(4),
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
            use kairos::core::hypercuboid::{HcCursor, SearchMode};
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
                for variant in 0..5u32 {
                    let (w, ss) = HcInfo::build_hc(&s);
                    let (order, mode) = match variant {
                        0 => (kairos::misc::ordering::HcOrdering::Natural, SearchMode::Balanced),
                        1 => (kairos::engine::ab::AlphaBeta::new(Default::default(), Default::default()).ordering(&s, &w, true), SearchMode::Balanced),
                        v => (kairos::misc::ordering::HcOrdering::random(w.get_universe(), &mut kairos::misc::rng::Mt19937::new(v)),
                              if v == 3 { SearchMode::Iterative } else { SearchMode::Balanced }),
                    };
                    let mut cur = HcCursor::new(ss, order, mode);
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
            use kairos::core::hypercuboid::{HcCursor, SearchMode};
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                let s = parse_g_line(&line).unwrap();
                let (w, ss) = HcInfo::build_hc(&s);
                let natural = w.search(&s, ss.clone()).count();
                let ab = kairos::engine::ab::AlphaBeta::new(Default::default(), Default::default());
                let mut cur = HcCursor::new(ss, ab.ordering(&s, &w, true), SearchMode::Balanced);
                let mut n = 0;
                while cur.next(&w, &s).is_some() {
                    n += 1;
                }
                writeln!(out, "natural {} scored {} dim {} new_axis {}", natural, n, w.dimension(), w.first_branch_axis()).unwrap();
                for mode in [SearchMode::Balanced, SearchMode::Iterative, SearchMode::Stable] {
                    let mut counts = Vec::new();
                    for seed in 1..=6u32 {
                        let mut rng = kairos::misc::rng::Mt19937::new(seed);
                        let (w2, ss2) = HcInfo::build_hc(&s);
                        let order = kairos::misc::ordering::HcOrdering::random(w2.get_universe(), &mut rng);
                        let mut cur = HcCursor::new(ss2, order, mode);
                        let mut k = 0;
                        while cur.next(&w2, &s).is_some() {
                            k += 1;
                        }
                        counts.push(k);
                    }
                    writeln!(out, "  random {:?}: {:?}", mode, counts).unwrap();
                }
            }
        }
        "bench" => bench(args.get(2).map(|s| s.as_str()).unwrap_or("data/bench.g"), &mut out),
        _ => {
            eprintln!("usage: ktools dump [cap] < positions.g | ktools perft <depth> [cap] < positions.g");
        }
    }
}

/// Node oracle + speed benchmark: capped perft and fixed-iteration seeded MCTS searches.
/// The checksum must not change for pure speed optimizations.
fn bench(path: &str, out: &mut impl Write) {
    use kairos::engine::features::default_weights;
    use kairos::engine::mcts::*;
    use kairos::engine::{Engine, Limits};
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
    let perft_secs = t0.elapsed().as_secs_f64();
    let stop = std::sync::atomic::AtomicBool::new(false);
    let t1 = Instant::now();
    let mut iters = 0u64;
    for (k, s) in positions.iter().enumerate() {
        for name in ["mcts", "linear"] {
            let roll = Rollout::new(
                DefaultKind::Rollout { selection: Selection::Random, cutoff: if name == "mcts" { Cutoff::Zero } else { Cutoff::Linear("default") } },
                60,
                DEFAULT_TEMPERATURE,
                default_weights(),
                Some(k as u32 + 11),
            );
            let mut e = Mcts::new(name, TreeKind::Uct, OrderKind::Random, roll, Some(k as u32 + 11));
            let limits = Limits { depth: Some(6), ..Default::default() };
            let mut infos = Vec::new();
            let best = e.find_best_move(s, &limits, &stop, &mut |m| infos.push(m));
            best.map(|a| a.to_string()).hash(&mut h);
            for m in &infos {
                if let Some(p) = m.find("mcts_score") {
                    m[p..].hash(&mut h);
                }
                if let Some(p) = m.find("tree_nodes=") {
                    m[p..].split_whitespace().next().hash(&mut h);
                }
            }
            iters += 60;
        }
        if std::env::var("BENCH_VERBOSE").is_ok() {
            let (lo, hi) = s.get_lines_range();
            eprintln!("pos {} lines {}..{} present {:?} cumulative {:.2}s", k, lo, hi, s.get_present(), t1.elapsed().as_secs_f64());
        }
    }
    let mcts_secs = t1.elapsed().as_secs_f64();
    writeln!(
        out,
        "oracle {:016x} | perft leaves {} in {:.3}s ({:.0}/s) | mcts {} iters in {:.3}s ({:.1} it/s) | total {:.3}s",
        h.finish(),
        leaves,
        perft_secs,
        leaves as f64 / perft_secs,
        iters,
        mcts_secs,
        iters as f64 / mcts_secs,
        perft_secs + mcts_secs
    )
    .unwrap();
    write!(out, "{}", kairos::misc::prof::report()).unwrap();
}

/// Alpha-beta node oracle: fixed-depth searches; checksum covers node counts and best moves.
fn abbench(path: &str, depth: u32, out: &mut impl Write) {
    use kairos::engine::ab::AlphaBeta;
    use kairos::engine::{Engine, Limits};
    use std::hash::{Hash, Hasher};
    let text = std::fs::read_to_string(path).expect("bench positions");
    let positions: Vec<State> = text.lines().filter(|l| l.starts_with("G ")).map(|l| parse_g_line(l).unwrap()).collect();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let stop = std::sync::atomic::AtomicBool::new(false);
    let t0 = Instant::now();
    let mut total_nodes = 0u64;
    for (k, s) in positions.iter().enumerate() {
        let mut e = AlphaBeta::new(Default::default(), Default::default());
        if let Ok(opts) = std::env::var("AB_OPTS") {
            use kairos::engine::OptionValue;
            for o in opts.split_whitespace() {
                if let Some((k, v)) = o.split_once('=') {
                    e.set_option(k, &OptionValue::parse(v));
                }
            }
        }
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
fn blunders(path: &str, max_games: usize, opts: &[String], out: &mut impl Write) {
    use kairos::core::variants::{apply_uci_moves, position_from_uci};
    use kairos::engine::ab::AlphaBeta;
    use kairos::engine::{Engine, Limits, OptionValue};
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
                                    let mut e = AlphaBeta::new(Default::default(), Default::default());
                                    for o in opts {
                                        if let Some((k, v)) = o.split_once('=') {
                                            e.set_option(k, &OptionValue::parse(v));
                                        }
                                    }
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
fn materank(path: &str, max_games: usize, opts: &[String], out: &mut impl Write) {
    use kairos::engine::ab::AlphaBeta;
    use kairos::engine::{Engine, OptionValue};
    let positions = mate_positions(path, max_games);
    let ranks: Vec<Option<usize>> = std::thread::scope(|sc| {
        let hs: Vec<_> = positions
            .chunks(positions.len().div_ceil(14).max(1))
            .map(|chunk| {
                sc.spawn(move || {
                    let mut e = AlphaBeta::new(Default::default(), Default::default());
                    for o in opts {
                        if let Some((k, v)) = o.split_once('=') {
                            e.set_option(k, &OptionValue::parse(v));
                        }
                    }
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

/// Eval-net training records from harness JSONL games: for every position where an engine
/// chose an action, the net inputs, the handcrafted score, the engine's reported score and
/// the game result, all from the side to move's view, and the game number.
///   ktools netdata out.bin games1.jsonl games2.jsonl ...
/// Record: NI + 4 little-endian f32 (inputs, base, teacher or NaN, wdl, game).
fn netdata(args: &[String], out: &mut impl Write) {
    use kairos::core::variants::{apply_uci_moves, position_from_uci};
    use kairos::engine::evalnet::{inputs, NI};
    use kairos::engine::hce::{evaluate, tuned_weights};
    let out_path = &args[0];
    let mut lines: Vec<String> = Vec::new();
    for f in &args[1..] {
        let text = std::fs::read_to_string(f).expect("games");
        lines.extend(text.lines().filter(|l| !l.trim().is_empty()).map(String::from));
    }
    let threads = 14;
    // Base score: the evaluation the search uses (the tuned feature HCE).
    let ep = tuned_weights();
    let chunks: Vec<Vec<f32>> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads)
            .map(|tid| {
                let (lines, ep) = (&lines, &ep);
                sc.spawn(move || {
                    let mut rec: Vec<f32> = Vec::new();
                    let mut g = tid;
                    while g < lines.len() {
                        let line = &lines[g];
                        g += threads;
                        let (Some(pos), Some(moves), Some(score)) =
                            (json_field(line, "position"), json_field(line, "moves"), json_field(line, "score"))
                        else {
                            continue;
                        };
                        let Ok(score) = score.parse::<f32>() else { continue };
                        let new_first = json_field(line, "new_first") == Some("true");
                        // Older files have no scores: label with the result only.
                        let evs: Vec<Option<i32>> = match line.find("\"evals\":") {
                            Some(e) => {
                                let t = &line[e + 8..];
                                let t = t[t.find('[').unwrap_or(0) + 1..t.find(']').unwrap_or(0)].to_string();
                                t.split(',').map(|x| x.trim().parse().ok()).collect()
                            }
                            None => vec![None; json_field(line, "actions").and_then(|a| a.parse().ok()).unwrap_or(0)],
                        };
                        let acts: Vec<&str> = moves.split(" submit").map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
                        let n_eng = evs.len();
                        if n_eng == 0 || acts.len() < n_eng {
                            continue;
                        }
                        let opening = acts.len() - n_eng;
                        let Ok(mut s) = position_from_uci(pos) else { continue };
                        let mut ok = true;
                        let mut first_player = None;
                        let mut pending: Vec<(usize, Vec<f32>, bool)> = Vec::new();
                        for (i, a) in acts.iter().enumerate() {
                            if i == opening {
                                first_player = Some(s.get_present().1);
                            }
                            if i >= opening {
                                let base = evaluate(&s, ep);
                                let x = inputs(&s, base);
                                let mut r = x.to_vec();
                                r.push(base as f32);
                                let t = evs[i - opening];
                                r.push(match t {
                                    Some(v) if v.abs() < 20000 => v as f32,
                                    _ => f32::NAN,
                                });
                                pending.push((i, r, s.get_present().1));
                            }
                            if apply_uci_moves(&mut s, &format!("{} submit", a)).is_err() {
                                ok = false;
                                break;
                            }
                        }
                        let Some(first) = first_player else { continue };
                        if !ok {
                            continue;
                        }
                        // NEW's colour and the result for white.
                        let new_black = if new_first { first } else { !first };
                        let white = if score == 0.5 { 0.5 } else if (score == 1.0) != new_black { 1.0 } else { 0.0 };
                        for (_, mut r, stm_black) in pending {
                            r.push(if stm_black { 1.0 - white } else { white });
                            r.push(g as f32 - threads as f32);
                            rec.extend(r);
                        }
                    }
                    rec
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut bytes: Vec<u8> = Vec::new();
    let mut n = 0usize;
    for c in &chunks {
        n += c.len() / (NI + 4);
        for v in c {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
    }
    std::fs::write(out_path, &bytes).expect("write records");
    writeln!(out, "{} records ({} inputs + 4) from {} games -> {}", n, NI, lines.len(), out_path).unwrap();
}
