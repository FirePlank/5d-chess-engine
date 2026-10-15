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
        "abbench" => abbench(
            args.get(2).map(|s| s.as_str()).unwrap_or("data/bench.g"),
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(4),
            &mut out,
        ),
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
        let limits = Limits { depth: Some(depth), ..Default::default() };
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
