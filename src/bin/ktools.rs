//! Command-line utilities: perft, legal-action dumps and node-count benchmarks.
//! Positions are read as "G <promotions> <5duci position> moves <lan|submit>*" lines.

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

fn coord(v: kairos::core::vec4::Vec4) -> String {
    format!("{},{},{},{}", v.l(), v.t(), v.x(), v.y())
}

fn move_string(m: &kairos::core::action::FullMove) -> String {
    format!("{}>{}", coord(m.from), coord(m.to))
}

/// The `rules` mode record of one position (see `ktools rules`).
fn rules_dump(s: &State, cap: usize, mate: bool, out: &mut impl Write) {
    let m = s.multiverse();
    let (present, player) = s.get_present();
    let (lmin, lmax) = s.get_lines_range();
    let (amin, amax) = s.get_active_range();
    let (sx, sy) = s.get_board_size();
    writeln!(
        out,
        "I player {} present {} lines {} {} active {} {} even {} size {} {} promo {}",
        player as u8, present, lmin, lmax, amin, amax, m.layout().is_even() as u8, sx, sy, s.get_promotion_options().0
    )
    .unwrap();
    for (l, t, c, fen) in m.get_boards(true) {
        writeln!(out, "B {} {} {} {}", l, t, c as u8, fen).unwrap();
    }
    let st = s.get_timeline_status();
    let join = |v: &[i32]| v.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(",");
    writeln!(out, "T mandatory {} optional {}", join(&st.mandatory), join(&st.optional)).unwrap();
    let mut moves = Vec::new();
    for l in st.playable() {
        for p in s.get_all_pieces(&[l]) {
            let mut to = Vec::new();
            s.gen_piece_move(p, &mut to);
            for q in to {
                moves.push(format!("{}>{}", coord(p), coord(q)));
            }
        }
    }
    moves.sort();
    writeln!(out, "N {}", moves.len()).unwrap();
    for mv in &moves {
        writeln!(out, "m {}", mv).unwrap();
    }
    let mut checks: Vec<String> = s.phantom().find_checks(!player).iter().map(move_string).collect();
    checks.sort();
    for c in &checks {
        writeln!(out, "k {}", c).unwrap();
    }
    writeln!(out, "C {}", s.has_phantom_check() as u8).unwrap();
    if !mate {
        return;
    }
    let mate = s.get_mate_type();
    let (w, ss) = HcInfo::build_hc(s);
    let acts: Vec<String> =
        w.search(s, ss).take(cap).map(|a| a.iter().map(move_string).collect::<Vec<_>>().join(" ")).collect();
    writeln!(out, "M {} {}", mate_name(mate), acts.len()).unwrap();
    for a in &acts {
        writeln!(out, "a {}", a).unwrap();
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
        "gameprobe" => {
            // gameprobe <file.g> [ms] : per position, the timelines (all / active), the search's
            // last report (depth, score, nodes, nps, roots) and the action, with `ms` per move.
            use kairos::engine::ab::AlphaBeta;
            use kairos::engine::Limits;
            let path = args.get(2).expect("gameprobe <file.g> [ms]");
            let ms: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1000);
            let text = std::fs::read_to_string(path).expect("positions");
            let stop = std::sync::atomic::AtomicBool::new(false);
            for (k, line) in text.lines().filter(|l| l.starts_with("G ")).enumerate() {
                let s = parse_g_line(line).unwrap();
                let (l0, l1) = s.get_lines_range();
                let mut e = AlphaBeta::new();
                // GAMEPROBE_THREADS: search threads (Lazy SMP).
                if let Some(t) = std::env::var("GAMEPROBE_THREADS").ok().and_then(|v| v.parse::<i64>().ok()) {
                    e.set_option("Threads", &kairos::engine::OptionValue::Int(t));
                }
                let mut last = String::new();
                let t = Instant::now();
                let best = e.find_best_move(&s, &Limits { movetime: Some(ms), ..Default::default() }, &stop, &mut |m| last = m);
                let (w, _) = HcInfo::build_hc(&s);
                let (mut phys, mut arr, mut dep) = (0, 0, 0);
                let u = w.get_universe();
                for n in 0..w.first_branch_axis() {
                    for i in u[n].iter() {
                        match w.entry(n, i) {
                            kairos::core::hypercuboid::Entry::Physical { .. } => phys += 1,
                            kairos::core::hypercuboid::Entry::Arriving { .. } => arr += 1,
                            kairos::core::hypercuboid::Entry::Departing { .. } => dep += 1,
                            _ => {}
                        }
                    }
                }
                let br = if w.first_branch_axis() < w.dimension() { u[w.first_branch_axis()].iter().count() } else { 0 };
                let secs = t.elapsed().as_secs_f64();
                writeln!(out, "pos {} {} lines {} active {} dim {} phys {} arr {} dep {} branch {} | {:.2}s total {} tnps {:.0} | {} | {}", k, if s.player() { "b" } else { "w" },
                    l1 - l0 + 1, s.active_timelines(), w.dimension(), phys, arr, dep, br, secs, e.last_search_nodes(), e.last_search_nodes() as f64 / secs,
                    last.split(" pv ").next().unwrap_or(""), best.map(|a| a.to_string()).unwrap_or_default()).unwrap();
                out.flush().unwrap();
            }
            write!(out, "{}", kairos::misc::prof::report()).unwrap();
        }
        "nodeorder" => {
            // nodeorder <file.g> <k> [n] : the first n actions an interior node of the k-th
            // position would search (ORDER_NO_FUTURE=1: without optional boards, as interior nodes).
            use kairos::engine::ab::AlphaBeta;
            let text = std::fs::read_to_string(args.get(2).expect("nodeorder <file.g> <k> [n]")).expect("positions");
            let k: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
            let n: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(30);
            let line = text.lines().filter(|l| l.starts_with("G ")).nth(k).expect("position");
            let s = parse_g_line(line).unwrap();
            for (i, a) in AlphaBeta::new().node_order(&s, n).iter().enumerate() {
                writeln!(out, "{:>3} {}", i, a.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(" ")).unwrap();
            }
        }
        "tlcorpus" => {
            // tlcorpus <per_bucket> <games.jsonl>... : G lines of positions from the games, up to
            // per_bucket for each count of active timelines (1, 2, 3, 4-5, 6-7, 8-10, 11+), taking
            // every 7th position of each game. Prints "G ..." with the bucket as a comment before.
            use kairos::core::variants::{apply_uci_moves, position_from_uci};
            let per: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20);
            let bucket = |n: usize| match n { 0 | 1 => 0, 2 => 1, 3 => 2, 4 | 5 => 3, 6 | 7 => 4, 8..=10 => 5, _ => 6 };
            let mut counts = [0usize; 7];
            'files: for path in &args[3..] {
                let text = std::fs::read_to_string(path).expect("games");
                for line in text.lines() {
                    if counts.iter().all(|&c| c >= per) {
                        break 'files;
                    }
                    let (Some(pos), Some(moves)) = (json_field(line, "position"), json_field(line, "moves")) else { continue };
                    let acts: Vec<&str> = moves.split(" submit").map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
                    let Ok(mut s) = position_from_uci(pos) else { continue };
                    // TLC_MAX_EVAL: only positions the next mover scored within this (reported
                    // scale, from the game's "evals"; mates and unknown scores are left out).
                    let max_eval: Option<i64> = std::env::var("TLC_MAX_EVAL").ok().and_then(|v| v.parse().ok());
                    let book_len: usize = json_field(line, "book").and_then(|b| b.parse().ok()).unwrap_or(0);
                    let evals: Vec<Option<i64>> = line
                        .find("\"evals\":")
                        .and_then(|i| line[i..].find('[').map(|j| i + j + 1))
                        .map(|i| {
                            let rest = &line[i..];
                            rest[..rest.find(']').unwrap_or(0)].split(',').map(|x| x.trim().parse().ok()).collect()
                        })
                        .unwrap_or_default();
                    for (i, a) in acts.iter().enumerate() {
                        if apply_uci_moves(&mut s, &format!("{} submit", a)).is_err() {
                            break;
                        }
                        if let Some(m) = max_eval {
                            let e = (i + 1).checked_sub(book_len).and_then(|k| evals.get(k).copied().flatten());
                            if e.is_none_or(|e| e.abs() > m) {
                                continue;
                            }
                        }
                        let b = bucket(s.active_timelines());
                        // TLC_MIN_LEFT: only positions with at least this many actions still to come.
                        let left: usize = std::env::var("TLC_MIN_LEFT").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
                        if (i + 1) % 7 == 0 && counts[b] < per && i + left < acts.len() {
                            counts[b] += 1;
                            writeln!(out, "# tl {}", s.active_timelines()).unwrap();
                            writeln!(out, "G 1 {} moves {} submit", pos, acts[..=i].join(" submit ")).unwrap();
                        }
                    }
                }
            }
            eprintln!("buckets {:?}", counts);
        }
        "inactivebook" => {
            // inactivebook <max> <games.jsonl>... : G lines of game positions with a timeline
            // outside the active range, at most one per game.
            use kairos::core::variants::{apply_uci_moves, position_from_uci};
            let max: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(500);
            let mut n = 0usize;
            let mut games = 0usize;
            'files: for path in &args[3..] {
                let text = std::fs::read_to_string(path).expect("games");
                for line in text.lines() {
                    if n >= max {
                        break 'files;
                    }
                    let (Some(pos), Some(moves)) = (json_field(line, "position"), json_field(line, "moves")) else { continue };
                    games += 1;
                    let acts: Vec<&str> = moves.split(" submit").map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
                    let Ok(mut s) = position_from_uci(pos) else { continue };
                    for (i, a) in acts.iter().enumerate() {
                        if apply_uci_moves(&mut s, &format!("{} submit", a)).is_err() {
                            break;
                        }
                        let (lo, hi) = s.get_lines_range();
                        let (amin, amax) = s.get_active_range();
                        if (lo < amin || hi > amax) && (i + 1) % 5 == 0 && i + 6 < acts.len() {
                            n += 1;
                            writeln!(out, "G 1 {} moves {} submit", pos, acts[..=i].join(" submit ")).unwrap();
                            break;
                        }
                    }
                }
            }
            eprintln!("{} positions with an inactive timeline in {} games", n, games);
        }
        "chesspos" => {
            // chesspos <max> <games.jsonl>... : the single-timeline positions of the games and the
            // move played there, as "<board fen>|<side>|<move>|<file>:<game>:<action>" (the board
            // is the present one; actions with a travel or another board are left out).
            use kairos::core::variants::{apply_uci_moves, position_from_uci};
            let max: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10000);
            let mut n = 0;
            'files: for path in &args[3..] {
                let text = std::fs::read_to_string(path).expect("games");
                for (g, line) in text.lines().enumerate() {
                    let (Some(pos), Some(moves)) = (json_field(line, "position"), json_field(line, "moves")) else { continue };
                    let book: usize = json_field(line, "book").and_then(|b| b.parse().ok()).unwrap_or(0);
                    let acts: Vec<&str> = moves.split(" submit").map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
                    let Ok(mut s) = position_from_uci(pos) else { continue };
                    for (i, a) in acts.iter().enumerate() {
                        let (lo, hi) = s.get_lines_range();
                        if lo == hi && i >= book && !a.contains(' ') && a.matches('(').count() == 1 {
                            let (t, c) = s.get_timeline_end(lo);
                            if let Some((_, _, _, fen)) = s.multiverse().get_boards(true).into_iter().find(|b| b.0 == lo && b.1 == t && b.2 == c) {
                                let mv = &a[a.find(')').unwrap() + 1..];
                                writeln!(out, "{}|{}|{}|{}:{}:{}", fen, if c { "b" } else { "w" }, mv, path, g, i).unwrap();
                                n += 1;
                                if n >= max {
                                    break 'files;
                                }
                            }
                        }
                        if apply_uci_moves(&mut s, &format!("{} submit", a)).is_err() {
                            break;
                        }
                    }
                }
            }
        }
        "chessfeat" => {
            // chessfeat <labels.tsv> <out.bin> [every=1] [max=1000000]: standard-chess term counts
            // (white minus black) of labelled positions, for scripts/chess/fit_chess.py.
            // Record: i16 phase, i16 side (1 black), i32 cp, then `chess::t::COUNT` i16.
            use kairos::core::board::Board;
            use kairos::engine::chess::{analyze, terms, threats, t, CountSink};
            let every: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(1);
            let max: usize = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(1_000_000);
            let text = std::fs::read_to_string(&args[2]).expect("labels");
            let mut w = std::io::BufWriter::new(std::fs::File::create(&args[3]).expect("out"));
            let mut n = 0usize;
            for (li, line) in text.lines().enumerate() {
                if li % every != 0 {
                    continue;
                }
                let mut it = line.split('\t');
                let (Some(fen), _, Some(cp)) = (it.next(), it.next(), it.next()) else { continue };
                let Ok(cp) = cp.parse::<i32>() else { continue };
                let mut ff = fen.split(' ');
                let (Some(place), Some(side)) = (ff.next(), ff.next()) else { continue };
                let Ok(b) = Board::from_fen(place, 8, 8) else { continue };
                let Some(info) = analyze(&b, 8, 8, true) else { continue };
                let black = side == "b";
                if info.in_check(black as usize) {
                    continue;
                }
                let mut v = [0i32; t::COUNT];
                for c in 0..2 {
                    let mut sink = CountSink([0; t::COUNT]);
                    terms(&info, c, &mut sink);
                    threats(&info, c, if (c == 1) == black { t::THREAT_M } else { t::THREAT_W }, &mut sink);
                    for (vi, &x) in v.iter_mut().zip(sink.0.iter()).take(t::COUNT) {
                        *vi += if c == 0 { x } else { -x };
                    }
                }
                w.write_all(&(info.phase as i16).to_le_bytes()).unwrap();
                w.write_all(&(black as i16).to_le_bytes()).unwrap();
                w.write_all(&cp.to_le_bytes()).unwrap();
                for x in v {
                    w.write_all(&(x.clamp(-32000, 32000) as i16).to_le_bytes()).unwrap();
                }
                n += 1;
                if n >= max {
                    break;
                }
            }
            writeln!(out, "{} records of {} terms", n, t::COUNT).unwrap();
        }
        "chessbench" => {
            // chessbench <labels.tsv> [n=20000] [reps=20]: ns per board of the standard-chess terms
            // next to the rest of `hce::board_terms`.
            use kairos::core::board::Board;
            let n: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(20000);
            let reps: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(20);
            let text = std::fs::read_to_string(&args[2]).expect("labels");
            let boards: Vec<Board> =
                text.lines().step_by(97).filter_map(|l| Board::from_fen(l.split(' ').next()?, 8, 8).ok()).take(n).collect();
            let mut acc = 0i64;
            let t0 = Instant::now();
            for _ in 0..reps {
                for b in &boards {
                    let c = kairos::engine::chess::board_chess(std::hint::black_box(b), 8, 8);
                    acc += c.v[0][1] as i64 + c.thr[1][0] as i64;
                }
            }
            let ns = t0.elapsed().as_nanos() as f64 / (reps * boards.len()) as f64;
            let t1 = Instant::now();
            for _ in 0..reps {
                for b in &boards {
                    acc += std::mem::size_of_val(&kairos::engine::hce::board_terms(std::hint::black_box(b), 8, 8)) as i64;
                }
            }
            let ns2 = t1.elapsed().as_nanos() as f64 / (reps * boards.len()) as f64;
            writeln!(out, "{} boards: chess {:.0} ns/board, rest of board_terms {:.0} ns/board (checksum {})", boards.len(), ns, ns2, acc).unwrap();
        }
        "legalcheck" => {
            // legalcheck <file.g>... : every legal action of each position, enumerated with and
            // without the problem memos (best-first in the engine's order, and depth-first in
            // natural order); prints positions where the sets differ.
            use kairos::core::hypercuboid::NO_PROBLEM_MEMO;
            use kairos::engine::ab::AlphaBeta;
            use std::sync::atomic::Ordering;
            let (mut n, mut checked, mut bad) = (0, 0, 0);
            for path in &args[2..] {
                let text = std::fs::read_to_string(path).expect("positions");
                for line in text.lines().filter(|l| l.starts_with("G ")) {
                    let Ok(s) = parse_g_line(line) else { continue };
                    n += 1;
                    let in_check = s.has_phantom_check();
                    checked += in_check as usize;
                    let all = |memo: bool| -> (Vec<String>, Vec<String>) {
                        NO_PROBLEM_MEMO.store(!memo, Ordering::Relaxed);
                        let mut a: Vec<String> = AlphaBeta::new().node_order(&s, 200_000).iter().map(|x| format!("{:?}", x)).collect();
                        let (w, ss) = HcInfo::build_hc(&s);
                        let mut b: Vec<String> = w.search(&s, ss).take(200_000).map(|x| format!("{:?}", x)).collect();
                        a.sort();
                        b.sort();
                        (a, b)
                    };
                    let (a0, b0) = all(false);
                    let (a1, b1) = all(true);
                    if a0 != a1 || b0 != b1 || a0 != b0 {
                        bad += 1;
                        writeln!(out, "DIFF check {} | bf {} vs {} | dfs {} vs {} | {}", in_check, a0.len(), a1.len(), b0.len(), b1.len(), &line[..line.len().min(120)]).unwrap();
                    }
                }
            }
            NO_PROBLEM_MEMO.store(false, Ordering::Relaxed);
            writeln!(out, "positions {} in check {} differing {}", n, checked, bad).unwrap();
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
        "puzzles" => puzzles(
            args.get(2).map(|s| s.as_str()).unwrap_or("data/puzzles.g"),
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(15000),
            &mut out,
        ),
        "matex" => {
            // matex <n> : exact (no pruning) mate in n for the side to move, per G line on stdin.
            let n: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2);
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                let s = parse_g_line(&line).unwrap();
                let t0 = Instant::now();
                let r = exact_mate(&s, n);
                let txt = r.as_ref().map(|a| {
                    let act = kairos::core::action::Action::from_moveseq(a, &s);
                    let mut os = s.clone();
                    act.moves().iter().map(|em| { let t = em.lan(&os); if let Some(m) = os.normalize_promotion(*em) { os.apply_ext::<true>(m); } t }).collect::<Vec<_>>().join(" ")
                });
                writeln!(out, "mate in {}: {:?} ({:.1}s)", n, txt, t0.elapsed().as_secs_f64()).unwrap();
                out.flush().unwrap();
            }
        }
        "mate2" => {
            // Exact (full-width) mate-in-1/2 check: every white first action that forces mate
            // next turn, with each black reply and the white action that mates it.
            // `mate2 1` checks mate in 1 only.
            let only1 = args.get(2).map(|s| s.as_str()) == Some("1");
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                let s = parse_g_line(&line).unwrap();
                let txt = |st: &State, a: &MoveSeq| {
                    let act = kairos::core::action::Action::from_moveseq(a, st);
                    let mut os = st.clone();
                    act.moves().iter().map(|em| { let t = em.lan(&os); if let Some(n) = os.normalize_promotion(*em) { os.apply_ext::<true>(n); } t }).collect::<Vec<_>>().join(" ")
                };
                let (w, ss) = HcInfo::build_hc(&s);
                let firsts: Vec<MoveSeq> = w.search(&s, ss).collect();
                writeln!(out, "{} white actions", firsts.len()).unwrap();
                for a in &firsts {
                    let Some(q) = apply_seq(&s, a) else { continue };
                    if is_mated(&q) {
                        writeln!(out, "MATE IN 1: {}", txt(&s, a)).unwrap();
                        continue;
                    }
                    if only1 {
                        continue;
                    }
                    let (wq, ssq) = HcInfo::build_hc(&q);
                    let replies: Vec<MoveSeq> = wq.search(&q, ssq).collect();
                    if replies.is_empty() {
                        continue; // stalemate
                    }
                    let mut mates = Vec::new();
                    let mut ok = true;
                    for r in &replies {
                        let Some(q2) = apply_seq(&q, r) else { ok = false; break };
                        let (w2, ss2) = HcInfo::build_hc(&q2);
                        match w2.search(&q2, ss2).find(|c| apply_seq(&q2, c).map(|q3| is_mated(&q3)).unwrap_or(false)) {
                            Some(c) => mates.push((txt(&q, r), txt(&q2, &c))),
                            None => { ok = false; break; }
                        }
                    }
                    if ok {
                        writeln!(out, "MATE IN 2: {}  ({} black replies)", txt(&s, a), replies.len()).unwrap();
                        for (r, c) in mates {
                            writeln!(out, "    {:<28} -> {}", r, c).unwrap();
                        }
                    }
                }
            }
        }
        "evalpos" => {
            // evalpos <file.g> [ms] : static evaluation breakdown (white's view) and a short
            // search of each G line.
            use kairos::engine::hce;
            let ms: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1000);
            let text = std::fs::read_to_string(&args[2]).expect("positions");
            for line in text.lines().filter(|l| l.starts_with("G ")) {
                let s = parse_g_line(line).unwrap();
                let w = hce::tuned_weights();
                let sd = hce::sides(&s);
                let f = hce::features_from(&sd, &s, false);
                let black = s.get_present().1;
                let base = hce::score(&f, &w, black);
                let res = kairos::engine::evalnet::embedded()
                    .map(|n| n.residual(&kairos::engine::evalnet::inputs_from(&sd, &s, hce::score(&f, &w, black))))
                    .unwrap_or(0);
                let sg = if black { -1 } else { 1 };
                writeln!(out, "present {:?}: hce {} net residual {} total {} (white's view)", s.get_present(), sg * base, sg * res, sg * (base + res)).unwrap();
                let mut terms: Vec<(f32, usize)> =
                    (0..hce::NF).filter(|&i| f[i] != 0).map(|i| (w[i] * f[i] as f32, i)).collect();
                terms.sort_by(|a, b| b.0.abs().total_cmp(&a.0.abs()));
                for (v, i) in terms.iter().take(14) {
                    write!(out, " {}={}({:+.0})", hce::FEATURE_NAMES[*i], f[*i], v).unwrap();
                }
                writeln!(out).unwrap();
                write!(out, " 5D (white, black):").unwrap();
                for i in hce::si::TRAVEL_PAST..hce::si::COUNT {
                    let (a, b) = (sd.side[0][i], sd.side[1][i]);
                    if a != 0 || b != 0 {
                        write!(out, " {}=({},{})", hce::FEATURE_NAMES[hce::fi::DANGER_5D + i - hce::si::TRAVEL_PAST], a, b).unwrap();
                    }
                }
                writeln!(out).unwrap();
                if ms > 0 {
                    let mut e = kairos::engine::ab::AlphaBeta::new();
                    let stop = std::sync::atomic::AtomicBool::new(false);
                    let depth = std::env::var("EVALPOS_DEPTH").ok().and_then(|d| d.parse().ok());
                    let limits = kairos::engine::Limits { movetime: Some(ms), depth, ..Default::default() };
                    let mut last = String::new();
                    let verbose = std::env::var_os("EVALPOS_INFO").is_some();
                    let best = e.find_best_move(&s, &limits, &stop, &mut |m| {
                        if verbose {
                            eprintln!("    {}", m);
                        }
                        last = m
                    });
                    writeln!(out, "  search {}ms: {} | {}", ms, best.map(|a| a.to_string()).unwrap_or_default(), last).unwrap();
                }
            }
        }
        "netdata" => netdata(&args[2..], &mut out),
        "nnuedata" => nnuedata(&args[2..], &mut out),
        "nnueeval" => nnueeval(&args[2..], &mut out),
        "nnuerandom" => nnuerandom(&args[2..], &mut out),
        "nnuerecs" => nnuerecs(&args[2..], &mut out),
        "nnuerel" => nnuerel(&args[2..], &mut out),
        "labelpick" => labelpick(&args[2..], &mut out),
        "labelrun" => labelrun(&args[2..], &mut out),
        "netcheck" => {
            // Residuals of the first records of a netdata file under a net blob.
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
        "futurestats" => {
            // How often engine actions move on boards other than the mandatory ones (boards
            // ahead of the present or on inactive timelines), by kind of move.
            use kairos::core::variants::{apply_uci_moves, position_from_uci};
            let (mut actions, mut with_future) = (0usize, 0usize);
            let (mut phys, mut dep, mut arr) = (0usize, 0usize, 0usize);
            let mut optional_boards = 0usize;
            for f in &args[2..] {
                for line in std::fs::read_to_string(f).expect("games").lines() {
                    let (Some(pos), Some(moves), Some(n)) = (json_field(line, "position"), json_field(line, "moves"), json_field(line, "actions")) else { continue };
                    let n: usize = n.parse().unwrap_or(0);
                    let acts: Vec<&str> = moves.split(" submit").map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
                    let Ok(mut s) = position_from_uci(pos) else { continue };
                    for (i, a) in acts.iter().enumerate() {
                        if i + n >= acts.len() {
                            let present = s.get_present();
                            let (amin, amax) = s.get_active_range();
                            let mandatory = |l: i32| l >= amin && l <= amax && s.get_timeline_end(l) == present;
                            let ts = s.get_timeline_status();
                            optional_boards += ts.optional.len();
                            actions += 1;
                            let mut fut = false;
                            for tok in a.split_whitespace() {
                                let Ok(em) = kairos::core::action::ExtMove::parse(tok) else { continue };
                                let (fl, tl) = (em.fm.from.l(), em.fm.to.l());
                                let from_future = !mandatory(fl);
                                let same_board = em.fm.from.tl() == em.fm.to.tl();
                                let onto_future = !same_board && s.get_timeline_end(tl) == (em.fm.to.t(), present.1) && !mandatory(tl);
                                if from_future && same_board { phys += 1; fut = true; }
                                if from_future && !same_board { dep += 1; fut = true; }
                                if onto_future { arr += 1; fut = true; }
                            }
                            with_future += fut as usize;
                        }
                        if apply_uci_moves(&mut s, &format!("{} submit", a)).is_err() {
                            break;
                        }
                    }
                }
            }
            writeln!(out, "engine actions {} | with a move on a non-mandatory board {} ({:.2}%) | physical {} departures {} arrivals {} | optional boards per action {:.2}",
                actions, with_future, 100.0 * with_future as f64 / actions.max(1) as f64, phys, dep, arr, optional_boards as f64 / actions.max(1) as f64).unwrap();
        }
        "bookeval" => bookeval(&args[2..], &mut out),
        "mate1recall" => mate1recall(&args[2..], &mut out),
        "matescan" => matescan(&args[2..], &mut out),
        "abbench" => abbench(
            args.get(2).map(|s| s.as_str()).unwrap_or("data/bench_ab.g"),
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(20),
            &mut out,
        ),
        "overrunpos" => overrunpos(&args[2..], &mut out),
        "firstact" => firstact(args.get(2).map(|s| s.as_str()).unwrap_or("games/ov_hard.g"), args.get(3).and_then(|s| s.parse().ok()).unwrap_or(20000), &mut out),
        "overrun" => overrun(
            args.get(2).map(|s| s.as_str()).expect("overrun <games.jsonl> [threads] [ms] [games] [step]"),
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(8),
            args.get(4).and_then(|s| s.parse().ok()).unwrap_or(100),
            args.get(5).and_then(|s| s.parse().ok()).unwrap_or(1000),
            args.get(6).and_then(|s| s.parse().ok()).unwrap_or(1),
            &mut out,
        ),
        "smpbench" => smpbench(
            args.get(2).map(|s| s.as_str()).unwrap_or("data/bench_ab.g"),
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1000),
            args.get(4).map(|s| s.split(',').filter_map(|t| t.parse().ok()).collect()).unwrap_or_else(|| vec![1, 2, 4, 8]),
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
                    let mut psc = kairos::core::hypercuboid::PointScratch::default();
                    let pt = if w.take_point(&mut hc, &order, &mut psc) { Some(psc.point) } else { None };
                    let Some(pt) = pt else {
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
                        else if let Some(p) = w.test_wasted(&s, &pt, &hc) { ("test_wasted", Some(p)) }
                        else if let Some(p) = w.find_checks(&s, &pt, &hc, &mut Default::default()) { ("find_checks", Some(p)) } else { ("ok", None) };
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
            // HC search (several orderings/modes) vs exhaustive naive search (its actions
            // without wasted moves).
            use kairos::core::hypercuboid::HcCursor;
            use kairos::core::naive::{naive_actions_budget, wastes_moves};
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
                let want: std::collections::BTreeSet<String> = naive.iter().filter(|a| !wastes_moves(&s, a)).map(seq_string).collect();
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
        "rules" => {
            // rules [cap] [nomate] : per-position rules dump (boards, moves, checks, mate type,
            // up to `cap` actions) for cross-checking other implementations.
            // Coordinates are "l,t,x,y" in internal timeline numbering (y = 0 is rank 1).
            let cap: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            let mate = args.get(3).map(|s| s.as_str()) != Some("nomate");
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                writeln!(out, "{}", line).unwrap();
                match parse_g_line(&line) {
                    Ok(s) => rules_dump(&s, cap, mate, &mut out),
                    Err(e) => writeln!(out, "E {}", e).unwrap(),
                }
                out.flush().unwrap();
            }
        }
        "randplay" => {
            // randplay <plies> <games> <seed> [branchy] : random legal games from each start
            // position, printing a "G" line for every position reached ("branchy" prefers, out
            // of a few random actions, one that time travels the most).
            use kairos::core::action::ExtMove;
            use kairos::core::hypercuboid::HcCursor;
            let plies: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);
            let games: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
            let mut rng_state: u64 = args.get(4).and_then(|s| s.parse::<u64>().ok()).unwrap_or(1).wrapping_mul(0x9E3779B97F4A7C15) | 1;
            let mut next_u32 = move || {
                rng_state ^= rng_state << 13;
                rng_state ^= rng_state >> 7;
                rng_state ^= rng_state << 17;
                (rng_state >> 32) as u32
            };
            let branchy = args.get(5).map(|s| s.as_str()) == Some("branchy");
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                for _ in 0..games {
                    let mut s = parse_g_line(&line).unwrap();
                    let mut hist = if line.contains(" moves") { line.clone() } else { format!("{} moves", line) };
                    for _ in 0..plies {
                        writeln!(out, "{}", hist).unwrap();
                        let (w, ss) = HcInfo::build_hc(&s);
                        let order = shuffled_order(&w, next_u32() as u64);
                        let mut cur = HcCursor::new(ss, order);
                        let a = if branchy {
                            // Up to 200 legal actions; pick at random among those that time travel.
                            let mut acts = Vec::new();
                            while acts.len() < 200 {
                                match cur.next(&w, &s) {
                                    Some(a) => acts.push(a),
                                    None => break,
                                }
                            }
                            let jumping: Vec<usize> =
                                (0..acts.len()).filter(|&i| acts[i].iter().any(|m| m.from.tl() != m.to.tl())).collect();
                            if acts.is_empty() {
                                break;
                            }
                            let i = if jumping.is_empty() {
                                next_u32() as usize % acts.len()
                            } else {
                                jumping[next_u32() as usize % jumping.len()]
                            };
                            acts.swap_remove(i)
                        } else {
                            match cur.next(&w, &s) {
                                Some(a) => a,
                                None => break,
                            }
                        };
                        for &m in &a {
                            let em = ExtMove::resolve(m, &s);
                            hist += &format!(" {}", em);
                            assert!(s.apply_ext::<false>(em));
                        }
                        hist += " submit";
                        assert!(s.submit::<false>());
                    }
                }
                out.flush().unwrap();
            }
        }
        "replay" => {
            // replay : parse each "G" line (applying its moves) and print the ones that fail.
            let (mut ok, mut bad) = (0, 0);
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                match parse_g_line(&line) {
                    Ok(_) => ok += 1,
                    Err(e) => {
                        bad += 1;
                        writeln!(out, "E {}
{}", e, line).unwrap();
                    }
                }
            }
            writeln!(out, "replayed {} failed {}", ok, bad).unwrap();
        }
        "playcheck" => {
            // playcheck : for each "G" game line, replay its moves and check at every submit that the
            // action just played is legal and that the position before it was not reported as mate.
            use kairos::core::action::{ExtMove, FullMove};
            use kairos::core::naive::is_legal_action;
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                let (head, moves) = match line.find(" moves") {
                    Some(i) => (&line[..i], &line[i + 6..]),
                    None => (&line[..], ""),
                };
                let mut s = match parse_g_line(head) {
                    Ok(s) => s,
                    Err(e) => {
                        writeln!(out, "E {}", e).unwrap();
                        continue;
                    }
                };
                let (mut ply, mut problems) = (0, Vec::new());
                let mut act: Vec<FullMove> = Vec::new();
                let mut before = s.clone();
                for tok in moves.split_whitespace() {
                    if tok == "submit" {
                        let mate = before.get_mate_type();
                        if mate != MateType::None {
                            problems.push(format!("ply {} mate {} before a played action", ply, mate_name(mate)));
                        }
                        if !is_legal_action(&before, &act) {
                            problems.push(format!("ply {} action not legal: {}", ply, seq_string(&MoveSeq::from(&act[..]))));
                        }
                        if !s.submit::<false>() {
                            problems.push(format!("ply {} cannot submit", ply));
                            break;
                        }
                        ply += 1;
                        act.clear();
                        before = s.clone();
                    } else {
                        let mv = match ExtMove::parse(tok) {
                            Ok(m) => m,
                            Err(e) => {
                                problems.push(format!("ply {} {}", ply, e));
                                break;
                            }
                        };
                        match s.normalize_promotion(mv) {
                            Some(n) if s.apply_ext::<false>(n) => act.push(mv.fm),
                            _ => {
                                problems.push(format!("ply {} cannot apply {}", ply, tok));
                                break;
                            }
                        }
                    }
                }
                let end = if act.is_empty() { mate_name(s.get_mate_type()) } else { "partial" };
                writeln!(out, "R plies {} end {} problems {}", ply, end, problems.len()).unwrap();
                for p in &problems {
                    writeln!(out, "  {}", p).unwrap();
                }
                out.flush().unwrap();
            }
        }
        "hcverify" => {
            // hcverify [cap] [seeds] : checks that every action generator the engine uses yields
            // exactly the hypercuboid search's legal actions. Above `cap` actions, only the first
            // `cap` of each generator are checked for legality.
            let cap: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(5000);
            let seeds: u64 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(2);
            let (mut positions, mut complete, mut actions, mut bad, mut repeating) = (0u64, 0u64, 0u64, 0u64, 0u64);
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                let s = parse_g_line(&line).unwrap();
                positions += 1;
                match hcverify(&s, cap, seeds) {
                    Ok((n, full, repeats)) => {
                        actions += n as u64;
                        complete += full as u64;
                        if let Some(r) = repeats {
                            repeating += 1;
                            writeln!(out, "REPEATS {}\n  {}", r, line).unwrap();
                        }
                    }
                    Err(e) => {
                        bad += 1;
                        writeln!(out, "MISMATCH {}\n  {}", e, line).unwrap();
                        out.flush().unwrap();
                    }
                }
            }
            writeln!(out, "hcverify positions {} (complete sets {}) actions {} mismatches {} with repeated actions {}", positions, complete, actions, bad, repeating)
                .unwrap();
        }
        "bookgen" => {
            // bookgen [source] : the opening book file (data/book_theory.txt) from its annotated
            // 5dpgn source.
            let path = args.get(2).map(|s| s.as_str()).unwrap_or("data/book_theory_src.txt");
            let src = std::fs::read_to_string(path).expect("book source");
            // The tuning layer (data/book_tuning.txt) when it exists, or the file named by the third argument.
            let tuning = std::fs::read_to_string(args.get(3).map(|s| s.as_str()).unwrap_or("data/book_tuning.txt")).unwrap_or_default();
            match kairos::engine::book::generate_tuned(&src, &tuning) {
                Ok(text) => {
                    write!(out, "{}", text).unwrap();
                    let b = kairos::engine::book::Book::parse(&text).expect("generated book parses");
                    let (p, m) = b.size();
                    eprintln!("book: {} positions, {} actions", p, m);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
        "tacticsgen" => {
            // tacticsgen [source] : the tactics suite (data/tactics.g) from its 5dpgn source.
            let path = args.get(2).map(|s| s.as_str()).unwrap_or("data/tactics_src.txt");
            let src = std::fs::read_to_string(path).expect("tactics source");
            let mut notes = Vec::new();
            match kairos::misc::tactics::generate_with_notes(&src, &mut notes) {
                Ok(text) => {
                    write!(out, "{}", text).unwrap();
                    let n = kairos::misc::tactics::parse(&text).expect("generated suite parses").len();
                    for note in &notes {
                        eprintln!("note: {}", note);
                    }
                    eprintln!("tactics: {} positions", n);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
        "tactics" => tactics(
            args.get(2).map(|s| s.as_str()).unwrap_or("data/tactics.g"),
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(2000),
            args.get(4).and_then(|s| s.parse().ok()).unwrap_or(1),
            &mut out,
        ),
        "bookcover" => {
            // bookcover <file>: for each 5dpgn line (one game per line) how far the opening book
            // knows it: the actions that are book moves ("play" or "know") and the first one
            // that is not (its position, if known, still lists the book's other actions).
            use kairos::core::variants::state_from_setup;
            use kairos::engine::book::Book;
            use kairos::web::pgn::{action_to_pgn_short, parse_5dpgn};
            let book = Book::theory();
            let text = std::fs::read_to_string(args.get(2).expect("file")).expect("read");
            for (n, line) in text.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let g = match parse_5dpgn(line, None) {
                    Ok(g) => g,
                    Err(e) => {
                        writeln!(out, "L{} PARSE ERROR: {}", n + 1, e).unwrap();
                        continue;
                    }
                };
                let Ok(mut s) = state_from_setup(&g.setup) else { continue };
                let (mut plies, mut known, mut first_miss) = (0, 0, None);
                for a in g.root.mainline() {
                    plies += 1;
                    let txt = action_to_pgn_short(&s, a);
                    let lan = kairos::web::tree::action_lan(&s, a);
                    let in_book = book.probe(&s).is_some_and(|ms| ms.iter().any(|m| m.lan == lan));
                    if in_book && first_miss.is_none() {
                        known += 1;
                    } else if first_miss.is_none() {
                        let pos_known = book.probe(&s).is_some();
                        first_miss = Some(format!("ply {} {} ({})", plies, txt, if pos_known { "position in book, move new" } else { "position not in book" }));
                    }
                    match s.can_apply_action(a) {
                        Some(next) => s = next,
                        None => break,
                    }
                }
                match first_miss {
                    None => writeln!(out, "L{} COVERED {}/{} ", n + 1, known, plies).unwrap(),
                    Some(m) => writeln!(out, "L{} {}/{} first new: {}", n + 1, known, plies, m).unwrap(),
                }
            }
        }
        "pgn2g" => {
            // pgn2g [all] <file.5dpgn>... : the "G" line of each game's main-line end ("all":
            // of every position along the main line).
            use kairos::core::variants::state_from_setup;
            use kairos::web::pgn::parse_5dpgn;
            use kairos::web::record::setup_to_uci;
            let all = args.get(2).map(|s| s.as_str()) == Some("all");
            for f in &args[2 + all as usize..] {
                let text = std::fs::read(f).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
                let g = match parse_5dpgn(&text, None) {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("{}: {}", f, e);
                        continue;
                    }
                };
                let Ok(mut s) = state_from_setup(&g.setup) else { continue };
                let mut line = format!("G {} {} moves", g.setup.promotions.0, setup_to_uci(&g.setup));
                for a in g.root.mainline() {
                    if all {
                        writeln!(out, "{}", line).unwrap();
                    }
                    for m in a.moves() {
                        line += &format!(" {}", m);
                    }
                    line += " submit";
                    s = s.can_apply_action(a).expect("main-line action applies");
                }
                writeln!(out, "{}", line).unwrap();
            }
        }
        "subdump" => {
            // subdump <spaces> <seed> [volume] : dumps the actions found in `spaces` random
            // sub-universes of positions too big to enumerate, so two builds compare part by part.
            // Output: "S <k>" then "a <wastes 0|1> <action>|<action without wasted moves>" lines.
            use kairos::core::hypercuboid::{Entry, HcCursor};
            use kairos::core::naive::wastes_moves;
            use kairos::misc::geometry::{Hc, SearchSpace};
            use kairos::misc::integer_set::IntegerSet;
            let spaces: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(4);
            let seed: u64 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(1);
            let volume: usize = args.get(4).and_then(|v| v.parse().ok()).unwrap_or(20000);
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if !line.starts_with("G ") {
                    continue;
                }
                let s = parse_g_line(&line).unwrap();
                writeln!(out, "{}", line).unwrap();
                let (w, _) = HcInfo::build_hc(&s);
                let u = w.get_universe();
                let (dim, na) = (w.dimension(), w.first_branch_axis());
                let mut st = (s.hash() ^ seed.wrapping_mul(0xD6E8FEB86659FD93)) | 1;
                let mut rnd = move |n: u64| {
                    st ^= st << 13;
                    st ^= st >> 7;
                    st ^= st << 17;
                    st % n.max(1)
                };
                for k in 0..spaces {
                    // Random non-null coordinates per axis (the first branch axis always gets
                    // one), axes in random order while the volume allows.
                    let mut sets: Vec<IntegerSet> = (0..dim).map(|_| IntegerSet::singleton(0)).collect();
                    let mut axes: Vec<usize> = (0..dim).collect();
                    for i in (1..axes.len()).rev() {
                        axes.swap(i, rnd(i as u64 + 1) as usize);
                    }
                    let mut vol = 1usize;
                    for &n in &axes {
                        let cands: Vec<u32> = u[n].iter().filter(|&i| i != 0).collect();
                        let want = if n == na { 1 + rnd(3) } else if n > na { rnd(3) } else { rnd(5) } as usize;
                        let mut c = want.min(cands.len());
                        while c > 0 && vol * (c + 1) > volume {
                            c -= 1;
                        }
                        for _ in 0..c {
                            sets[n].insert(cands[rnd(cands.len() as u64) as usize]);
                        }
                        vol *= sets[n].len();
                    }
                    // The departure of every arrival kept.
                    for n in 0..dim {
                        let deps: Vec<(usize, u32)> = sets[n]
                            .iter()
                            .filter_map(|i| match w.entry(n, i) {
                                Entry::Arriving { idx, fa, .. } => Some((*fa as usize, *idx)),
                                _ => None,
                            })
                            .collect();
                        for (fa, idx) in deps {
                            sets[fa].insert(idx);
                        }
                    }
                    // The branch axes in use form a prefix: one hypercuboid per count.
                    let mut hcs = Vec::new();
                    for j in 0..=dim - na {
                        let mut axes = sets.clone();
                        for (b, set) in axes[na..].iter_mut().enumerate() {
                            if b < j {
                                set.erase(0);
                            } else {
                                *set = IntegerSet::singleton(0);
                            }
                        }
                        if axes.iter().all(|a| !a.is_empty()) {
                            hcs.push(Hc::new(axes));
                        }
                    }
                    writeln!(out, "S {}", k).unwrap();
                    let mut cur = HcCursor::new(SearchSpace::from_vec(hcs), shuffled_order(&w, seed + k as u64));
                    while let Some(a) = cur.next(&w, &s) {
                        let wasted = wastes_moves(&s, &a);
                        let reduced: Vec<String> = without_wasted(&s, &a).iter().map(|m| m.to_string()).collect();
                        writeln!(out, "a {} {}|{}", wasted as u8, seq_string(&a), reduced.join(" ")).unwrap();
                    }
                }
                out.flush().unwrap();
            }
        }
        "wastecheck" => {
            // wastecheck [cap] : checks the hypercuboid search against a `ktools dump` that still
            // includes wasted-move actions: same actions once those are dropped, same mate type.
            // Truncated dumps are checked one way; positions with `cap`+ actions are skipped.
            use kairos::core::action::FullMove;
            use kairos::core::naive::{is_legal_action, wastes_moves};
            use std::collections::BTreeSet;
            let cap: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(200_000);
            let (mut positions, mut cut, mut skipped, mut old_n, mut wasted_n, mut new_n, mut bad) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
            let parse_seq = |a: &str| -> Vec<FullMove> { a.split_whitespace().map(|t| FullMove::parse(t).unwrap()).collect() };
            let mut check = |g: &str, mate: &str, dumped: &[String], out: &mut dyn Write| {
                let s = parse_g_line(g).unwrap();
                positions += 1;
                let (w, ss) = HcInfo::build_hc(&s);
                let mut got: BTreeSet<String> = BTreeSet::new();
                let mut fail: Vec<String> = Vec::new();
                for a in w.search(&s, ss) {
                    if got.len() >= cap {
                        skipped += 1;
                        return;
                    }
                    let txt = seq_string(&a);
                    if !got.insert(txt.clone()) {
                        fail.push(format!("repeated {}", txt));
                    }
                }
                let full = dumped.len() < cap;
                cut += !full as u64;
                let mut want: BTreeSet<String> = BTreeSet::new();
                for d in dumped {
                    let mvs = parse_seq(d);
                    old_n += 1;
                    if !wastes_moves(&s, &mvs) {
                        want.insert(d.clone());
                        continue;
                    }
                    wasted_n += 1;
                    let keep: Vec<String> = without_wasted(&s, &mvs).iter().map(|m| m.to_string()).collect();
                    let reduced = keep.join(" ");
                    if !got.contains(&reduced) {
                        fail.push(format!("{} without its wasted moves ({}) not yielded", d, reduced));
                    }
                }
                for a in want.difference(&got).take(3) {
                    fail.push(format!("missing {}", a));
                }
                if full {
                    for a in got.difference(&want).take(3) {
                        fail.push(format!("extra {}", a));
                    }
                } else if let Some(a) = got.iter().find(|a| {
                    let mvs = parse_seq(a);
                    !is_legal_action(&s, &mvs) || wastes_moves(&s, &mvs)
                }) {
                    fail.push(format!("yielded {} is illegal or wastes a move", a));
                }
                new_n += got.len() as u64;
                let m = mate_name(s.get_mate_type());
                if m != mate {
                    fail.push(format!("mate type {} was {}", m, mate));
                }
                if got.is_empty() != dumped.is_empty() {
                    fail.push(format!("{} actions, {} dumped", got.len(), dumped.len()));
                }
                if !fail.is_empty() {
                    bad += 1;
                    writeln!(out, "MISMATCH {}\n  {}", fail.iter().take(4).cloned().collect::<Vec<_>>().join("\n  "), g).unwrap();
                    out.flush().unwrap();
                }
            };
            // (G line, mate type, dumped actions) of the position being read.
            let mut cur: Option<(String, String, Vec<String>)> = None;
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                if line.starts_with("G ") {
                    if let Some((g, m, d)) = cur.take() {
                        check(&g, &m, &d, &mut out);
                    }
                    cur = Some((line, String::new(), Vec::new()));
                } else if let Some((_, m, d)) = cur.as_mut() {
                    if let Some(r) = line.strip_prefix("M ") {
                        *m = r.split(' ').next().unwrap_or("").to_string();
                    } else if let Some(a) = line.strip_prefix("a ") {
                        d.push(a.to_string());
                    }
                }
            }
            if let Some((g, m, d)) = cur.take() {
                check(&g, &m, &d, &mut out);
            }
            writeln!(
                out,
                "wastecheck positions {} (dumps cut short {}, skipped {}) dumped actions {} with wasted moves {} yielded {} mismatches {}",
                positions, cut, skipped, old_n, wasted_n, new_n, bad
            )
            .unwrap();
        }
        "bench" => bench(args.get(2).map(|s| s.as_str()).unwrap_or("data/bench.g"), &mut out),
        "matecheck" => matecheck(&args[2..], &mut out),
        "mrank" => {
            // mrank <file.g> [cap] : ranks of the mating actions (first ten) among the first
            // `cap` actions of each G line in the order an interior node tries them.
            let text = std::fs::read_to_string(&args[2]).expect("positions");
            let cap: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(20000);
            let e = kairos::engine::ab::AlphaBeta::new();
            for line in text.lines().filter(|l| l.starts_with("G ")) {
                let s = parse_g_line(line).unwrap();
                let order = e.node_order(&s, cap);
                let ranks: Vec<usize> = order.iter().enumerate().filter(|(_, a)| apply_seq(&s, a).is_some_and(|c| is_mated(&c))).map(|(i, _)| i).collect();
                writeln!(out, "actions {} | mating ranks {:?}", order.len(), &ranks[..ranks.len().min(10)]).unwrap();
                if let Ok(v) = std::env::var("MRANK_V") {
                    // The first `v` actions (and the first mating one): checking or not.
                    let n: usize = v.parse().unwrap_or(10);
                    let checks = order.iter().filter(|a| apply_seq(&s, a).is_some_and(|c| c.has_phantom_check())).count();
                    writeln!(out, "  checking actions among them: {}", checks).unwrap();
                    for (i, a) in order.iter().enumerate().filter(|(i, _)| *i < n || ranks.first() == Some(i)) {
                        let c = apply_seq(&s, a).is_some_and(|c| c.has_phantom_check());
                        writeln!(out, "  {:>5} {} {}", i, if c { "+" } else { " " }, seq_string(a)).unwrap();
                    }
                }
            }
        }
        _ => {
            eprintln!(
                "usage: ktools <mode> ...\n  \
                 perft <depth> [cap] | dump [cap] | naive | validate [limit] | ordercheck [cap] | slicecheck | explain <moves>\n  \
                 \x20 (positions as G lines on stdin)\n  \
                 bench [file]            perft oracle\n  \
                 abbench [file] [depth]  alpha-beta node oracle (AB_NODES caps nodes per position)\n  \
                 puzzles [file] [ms]     time to find each mate in data/puzzles.g
                   matecheck <cases> [nodes] [max] [threads] [records_out]   mate consistency along played lines
                   mrank <file.g> [cap]   ranks of the mating actions in the interior node order
                   tactics [file] [ms] [threads]   solved/unsolved and time to solve per position of data/tactics.g
                   bookgen [src] | tacticsgen [src]   build data/book_theory.txt / data/tactics.g from their sources
                   blunders <games.jsonl> [n] | materank <games.jsonl> [n] | depthstat <games.jsonl> [ms] | rootgen [file]
                   overrunpos <file.g>... [threads=1,4] [ms=100,1000] [clock=2000+100,30000+300]   worst time overrun
                   firstact [file.g] [cap ms]   time to the first legal action by enumeration strategy"
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
        // NNUE_FILE: search with that network's evaluation (`EvalFile`).
        if let Ok(f) = std::env::var("NNUE_FILE") {
            assert!(e.set_option("EvalFile", &kairos::engine::OptionValue::Str(f)), "NNUE_FILE");
        }
        // NNUE_BLEND: the network's share in percent (`EvalBlend`).
        if let Some(b) = std::env::var("NNUE_BLEND").ok().and_then(|b| b.parse::<i64>().ok()) {
            assert!(e.set_option("EvalBlend", &kairos::engine::OptionValue::Int(b)), "NNUE_BLEND");
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

/// Lazy SMP scaling: per thread count, total nodes/s and mean depth reached in `ms` per
/// position, then the time to reach (with depth as the limit) the depth one thread reaches.
fn smpbench(path: &str, ms: u64, threads: Vec<usize>, out: &mut impl Write) {
    use kairos::engine::ab::AlphaBeta;
    use kairos::engine::{Limits, OptionValue};
    let text = std::fs::read_to_string(path).expect("bench positions");
    let positions: Vec<State> = text.lines().filter(|l| l.starts_with("G ")).map(|l| parse_g_line(l).unwrap()).collect();
    let stop = std::sync::atomic::AtomicBool::new(false);
    let field = |line: &str, key: &str| -> u64 { line.split_whitespace().skip_while(|w| *w != key).nth(1).and_then(|x| x.parse().ok()).unwrap_or(0) };
    let hash: i64 = std::env::var("SMP_HASH").ok().and_then(|v| v.parse().ok()).unwrap_or(256);
    let engine = |t: usize| {
        let mut e = AlphaBeta::new();
        e.set_option("Hash", &OptionValue::Int(hash));
        e.set_option("Threads", &OptionValue::Int(t as i64));
        e
    };
    // Depth one thread reaches per position: the time-to-depth target.
    let mut target: Vec<u32> = Vec::new();
    let mut base_ttd = 0.0;
    for &t in &threads {
        let (mut nodes, mut depth_sum, mut secs) = (0u64, 0u64, 0.0);
        let mut reached = Vec::new();
        for s in &positions {
            let mut e = engine(t);
            let mut last = String::new();
            let t0 = Instant::now();
            e.find_best_move(s, &Limits { movetime: Some(ms), ..Default::default() }, &stop, &mut |m| last = m);
            secs += t0.elapsed().as_secs_f64();
            nodes += e.last_search_nodes();
            depth_sum += field(&last, "depth");
            reached.push(field(&last, "depth") as u32);
        }
        if target.is_empty() {
            target = reached;
        }
        let mut ttd = 0.0;
        for (s, &d) in positions.iter().zip(&target) {
            let mut e = engine(t);
            let t0 = Instant::now();
            e.find_best_move(s, &Limits { depth: Some(d.max(1)), ..Default::default() }, &stop, &mut |_| {});
            ttd += t0.elapsed().as_secs_f64();
        }
        if base_ttd == 0.0 {
            base_ttd = ttd;
        }
        writeln!(
            out,
            "threads {:>2} | {:>9.0} nps | mean depth {:.2} | time to depth {:.2}s (x{:.2})",
            t,
            nodes as f64 / secs,
            depth_sum as f64 / positions.len() as f64,
            ttd,
            base_ttd / ttd
        )
        .unwrap();
        out.flush().unwrap();
    }
}

/// Time overruns: searches every `step`-th position of the games in `path` for `ms` with
/// `threads` threads (one engine throughout, as in a match) and reports the searches that
/// returned late.
fn overrun(path: &str, threads: usize, ms: u64, max_games: usize, step: usize, out: &mut impl Write) {
    use kairos::core::variants::{apply_uci_moves, position_from_uci};
    use kairos::engine::ab::AlphaBeta;
    use kairos::engine::{Limits, OptionValue};
    let text = std::fs::read_to_string(path).expect("games");
    let stop = std::sync::atomic::AtomicBool::new(false);
    let mut e = AlphaBeta::new();
    e.set_option("Threads", &OptionValue::Int(threads as i64));
    let (mut searches, mut worst, mut late) = (0usize, 0.0f64, 0usize);
    for (g, line) in text.lines().take(max_games).enumerate() {
        let (Some(pos), Some(moves)) = (json_field(line, "position"), json_field(line, "moves")) else { continue };
        let Ok(mut s) = position_from_uci(pos) else { continue };
        let acts: Vec<&str> = moves.split(" submit").map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
        for (k, a) in acts.iter().enumerate() {
            if k % step == 0 {
                let t = Instant::now();
                let mut last = String::new();
                e.find_best_move(&s, &Limits { movetime: Some(ms), ..Default::default() }, &stop, &mut |m| last = m);
                let over = t.elapsed().as_secs_f64() * 1e3 - ms as f64;
                searches += 1;
                worst = worst.max(over);
                if over > 30.0 {
                    late += 1;
                    let (lo, hi) = s.get_lines_range();
                    writeln!(out, "game {} action {}: {:.0} ms late, {} timelines | {}", g, k, over, hi - lo + 1, last.chars().take(90).collect::<String>()).unwrap();
                }
            }
            if apply_uci_moves(&mut s, &format!("{} submit", a)).is_err() {
                break;
            }
        }
    }
    writeln!(out, "{} searches, {} late by over 30 ms, worst {:.0} ms", searches, late, worst).unwrap();
}

fn json_field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{}\":", key);
    let i = line.find(&pat)? + pat.len();
    let rest = line[i..].trim_start();
    if let Some(r) = rest.strip_prefix('"') {
        Some(&r[..r.find('"')?])
    } else {
        let end = rest.find([',', '}']).unwrap_or(rest.len());
        Some(rest[..end].trim())
    }
}

fn apply_seq(s: &State, a: &[kairos::core::action::FullMove]) -> Option<State> {
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

/// Exact (full-width, no pruning) test whether the side to move forces checkmate within `n` of
/// its own actions; returns the first mating action found.
fn exact_mate(s: &State, n: u32) -> Option<MoveSeq> {
    let (w, ss) = HcInfo::build_hc(s);
    for a in w.search(s, ss) {
        let Some(q) = apply_seq(s, &a) else { continue };
        if is_mated(&q) {
            return Some(a);
        }
        if n <= 1 {
            continue;
        }
        let (wq, ssq) = HcInfo::build_hc(&q);
        let mut any = false;
        let mut all = true;
        for r in wq.search(&q, ssq) {
            any = true;
            let Some(q2) = apply_seq(&q, &r) else { all = false; break };
            if exact_mate(&q2, n - 1).is_none() {
                all = false;
                break;
            }
        }
        if any && all {
            return Some(a);
        }
    }
    None
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
/// The moves of the action `mvs` (playable in `s`) that are not wasted (see
/// `hypercuboid::wasted_moves`), in order.
fn without_wasted(s: &State, mvs: &[kairos::core::action::FullMove]) -> Vec<kairos::core::action::FullMove> {
    let player = s.get_present().1;
    let mut c = s.clone();
    let mut branching = Vec::new();
    for &m in mvs {
        branching.push(c.get_timeline_end(m.to.l()) != (m.to.t(), player));
        c.apply_move::<true>(m);
    }
    let wasted = kairos::core::hypercuboid::wasted_moves(&c, mvs, |i| branching[i]);
    mvs.iter().enumerate().filter(|(i, _)| !wasted.contains(i)).map(|(_, m)| *m).collect()
}

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
    kairos::misc::ordering::HcOrdering::Precomputed(kairos::misc::ordering::AxisOrders::new(orders))
}

/// Mate puzzles: "# mate <n> <name>" comment lines, each followed by a G line. Reports the
/// time and depth at which the search first reports the mate, within `ms` per puzzle.
fn puzzles(path: &str, ms: u64, out: &mut impl Write) {
    use kairos::engine::ab::AlphaBeta;
    use kairos::engine::Limits;
    let text = std::fs::read_to_string(path).expect("puzzles");
    let mut label = String::new();
    let mut solved = 0;
    let mut total = 0;
    for line in text.lines() {
        if let Some(l) = line.strip_prefix("# mate ") {
            label = l.to_string();
            continue;
        }
        if !line.starts_with("G ") {
            continue;
        }
        total += 1;
        let want: u32 = label.split_whitespace().next().and_then(|n| n.parse().ok()).unwrap_or(0);
        let s = parse_g_line(line).unwrap();
        let stop = std::sync::atomic::AtomicBool::new(false);
        let mut e = AlphaBeta::new();
        let t0 = Instant::now();
        let mut found: Option<(f64, String)> = None;
        let mut last = String::new();
        e.find_best_move(&s, &Limits { movetime: Some(ms), ..Default::default() }, &stop, &mut |m| {
            if found.is_none() && m.contains("score mate ") && !m.contains("score mate -") {
                found = Some((t0.elapsed().as_secs_f64(), m.clone()));
            }
            last = m;
        });
        let depth = |m: &str| m.split_whitespace().nth(1).unwrap_or("?").to_string();
        match found {
            Some((t, m)) => {
                solved += 1;
                let n = m.split_whitespace().skip_while(|w| *w != "mate").nth(1).unwrap_or("?");
                writeln!(out, "mate {:<3} {:<45} found mate {} at depth {} in {:.2}s", want, label, n, depth(&m), t).unwrap();
            }
            None => writeln!(out, "mate {:<3} {:<45} not found in {:.1}s (reached depth {})", want, label, ms as f64 / 1000.0, depth(&last)).unwrap(),
        }
        out.flush().unwrap();
    }
    writeln!(out, "solved {}/{}", solved, total).unwrap();
}

/// Tactics regression suite (`kairos::misc::tactics`): searches each position for `ms` and
/// reports whether the played action is correct, and from when every reported line agreed.
fn tactics(path: &str, ms: u64, threads: usize, out: &mut impl Write) {
    use kairos::core::action::Action;
    use kairos::engine::ab::AlphaBeta;
    use kairos::engine::{Limits, OptionValue};
    use kairos::web::tree::parse_lan_moves;
    let text = std::fs::read_to_string(path).expect("tactics suite");
    let suite = match kairos::misc::tactics::parse(&text) {
        Ok(s) => s,
        Err(e) => {
            writeln!(out, "{}", e).unwrap();
            std::process::exit(1);
        }
    };
    let (mut solved, mut time_sum) = (0, 0.0);
    let t_all = Instant::now();
    for t in &suite {
        let mut e = AlphaBeta::new();
        e.set_option("Threads", &OptionValue::Int(threads as i64));
        let stop = std::sync::atomic::AtomicBool::new(false);
        let t0 = Instant::now();
        // Time of the first info line of the current run of lines starting with a correct action.
        let mut since: Option<f64> = None;
        let mut last = String::new();
        // A proven mate as fast as the known one also solves a `mate <n>` position.
        let fast_mate = |line: &str| -> bool {
            let mut w = line.split_whitespace().skip_while(|w| *w != "score").skip(1);
            w.next() == Some("mate") && w.next().and_then(|k| k.parse::<i64>().ok()).is_some_and(|k| k > 0 && t.mate.is_some_and(|n| k <= n as i64))
        };
        let first_action = |line: &str| -> Option<Action> {
            let pv = line.split(" pv ").nth(1)?;
            let first = pv.split("submit").next()?.trim();
            parse_lan_moves(first).ok().filter(|m| !m.is_empty()).map(|m| Action::from_vector(&m, &t.state))
        };
        let best = e.find_best_move(&t.state, &Limits { movetime: Some(ms), ..Default::default() }, &stop, &mut |m| {
            match first_action(&m) {
                Some(a) if t.solved_by(&a) || fast_mate(&m) => {
                    since.get_or_insert(t0.elapsed().as_secs_f64());
                }
                Some(_) => since = None,
                None => {}
            }
            last = m;
        });
        let secs = t0.elapsed().as_secs_f64();
        let depth = last.split_whitespace().nth(1).unwrap_or("?").to_string();
        let score = last.split_whitespace().skip_while(|w| *w != "score").take(3).collect::<Vec<_>>().join(" ");
        match best {
            Some(a) if t.solved_by(&a) || (fast_mate(&last) && first_action(&last).as_ref() == Some(&a)) => {
                let at = since.unwrap_or(secs);
                solved += 1;
                time_sum += at;
                writeln!(out, "solved   {:>6.2}s  depth {:>2}  {:<16} {}", at, depth, score, t.name).unwrap();
            }
            other => {
                let played = other.map(|a| kairos::web::tree::action_lan(&t.state, &a)).unwrap_or_else(|| "none".into());
                let wanted = if t.avoid { format!("not {}", t.best_lan.join(" | ")) } else { t.best_lan.join(" | ") };
                writeln!(out, "UNSOLVED {:>6.2}s  depth {:>2}  {:<16} {}  (played {}, wanted {})", secs, depth, score, t.name, played, wanted).unwrap();
            }
        }
        out.flush().unwrap();
    }
    writeln!(
        out,
        "solved {}/{} | {} ms per position, {} thread(s) | mean time to solve {:.2}s | total {:.1}s",
        solved,
        suite.len(),
        ms,
        threads,
        time_sum / solved.max(1) as f64,
        t_all.elapsed().as_secs_f64()
    )
    .unwrap();
}

/// Replay one harness game record (`sprt` JSONL), calling `f` with each position an engine
/// played from, that engine's reported score of it (internal units, None when unscored, or
/// when the record says the score's line did not begin with the action played) and the action
/// it played (5DUCI moves, without `submit`). Returns the
/// result for white (1, 0.5, 0), or None for a game without a usable result or that does not
/// replay (engine failures, missing fields, illegal moves).
fn replay_game(line: &str, mut f: impl FnMut(&State, Option<i32>, &str)) -> Option<f32> {
    use kairos::core::variants::{apply_uci_moves, position_from_uci};
    // A game lost to an engine failure has no real result.
    if json_field(line, "reason").is_some_and(|r| r.starts_with("protocol")) {
        return None;
    }
    let (pos, moves, score) = (json_field(line, "position")?, json_field(line, "moves")?, json_field(line, "score")?);
    let score = score.parse::<f32>().ok()?;
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
    // Scores on the reported scale ("eval_scale": internal * 100 / scale) back to internal
    // units; older records hold internal units. (Mates, +-20000 on either scale, still land
    // outside the kept range.)
    let mut evs: Vec<Option<i32>> = match json_field(line, "eval_scale").and_then(|x| x.parse::<f64>().ok()) {
        Some(scale) => evs.into_iter().map(|e| e.map(|v| (v as f64 * scale / 100.0).round() as i32)).collect(),
        None => evs,
    };
    // A score of another action (the clock cut a change of mind short) is no label for this one.
    if let Some(e) = line.find("\"pv_played\":[") {
        let t = &line[e + 13..];
        for (ev, played) in evs.iter_mut().zip(t[..t.find(']').unwrap_or(0)].split(',')) {
            if played.trim() == "0" {
                *ev = None;
            }
        }
    }
    let acts: Vec<&str> = moves.split(" submit").map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
    let n_eng = evs.len();
    if n_eng == 0 || acts.len() < n_eng {
        return None;
    }
    let opening = acts.len() - n_eng;
    let mut s = position_from_uci(pos).ok()?;
    let mut first_player = None;
    for (i, a) in acts.iter().enumerate() {
        if i == opening {
            first_player = Some(s.get_present().1);
        }
        if i >= opening {
            f(&s, evs[i - opening], a);
        }
        apply_uci_moves(&mut s, &format!("{} submit", a)).ok()?;
    }
    // NEW's colour and the result for white.
    let first = first_player?;
    let new_black = if new_first { first } else { !first };
    Some(if score == 0.5 { 0.5 } else if (score == 1.0) != new_black { 1.0 } else { 0.0 })
}

/// `nnuedata out games.jsonl...`: NNUE training records (`engine::nnue`, read by
/// scripts/nnue/train.py), in four files next to `out`:
/// - `out.rec`: per record, f32 teacher score (internal units, side to move's view; NaN when
///   none or a mate), result for the side to move (NaN for an action-limit game), game number
///   and ply, then the `nnue::ND`
///   dense inputs as i16 (they are counts);
/// - `out.idx`: per record, u64 index of its first row in `out.brd` (its rows follow) and u64
///   index of its first relation feature in `out.rel`;
/// - `out.brd`: per latest board, u32 record, u16 role (low byte) and board height (high byte),
///   u16 feature count n, u64 offset in `out.ft1` (u16 units) of its n features from the side to
///   move's view (the other view is `nnue::other_view` of each);
/// - `out.ft1`: u16 board features; `out.rel`: u16 relation features (`nnue::relation_features`).
///   (Older exports have `out.ftr` instead, with both views stored, and no height.)
///
/// NETDATA_NO_CHECK=1 leaves out positions with the side to move in check (test sets).
/// NNUE_QUIET=1 keeps only quiet positions, as the search evaluates them statically: the side to
/// move not in check, with no capture quiescence would credit (`local::capture_gain` on its
/// boards of the present), and its played action neither capturing nor checking; the teacher's
/// score of the others includes tactics the search resolves itself.
/// NNUE_LABELS=file (`labelrun` output, for the same games file): only the relabelled positions,
/// with their new scores (or, with NNUE_LABELS_KEEP=1, their old ones, for comparison).
fn nnuedata(args: &[String], out: &mut impl Write) {
    use kairos::engine::{hce, nnue};
    use std::io::BufWriter;
    struct Sample {
        head: [f32; 4],
        dense: [i16; nnue::ND],
        boards: Vec<(u8, i32, Vec<u16>)>,
        rel: Vec<u16>,
    }
    let base = &args[0];
    let mut lines: Vec<String> = Vec::new();
    for f in &args[1..] {
        let text = std::fs::read_to_string(f).expect("games");
        lines.extend(text.lines().filter(|l| !l.trim().is_empty()).map(String::from));
    }
    let threads = 14;
    let no_check = std::env::var_os("NETDATA_NO_CHECK").is_some();
    let keep_teacher = std::env::var_os("NNUE_LABELS_KEEP").is_some();
    let quiet = std::env::var_os("NNUE_QUIET").is_some();
    let local_vals = kairos::engine::local::values_from(&hce::tuned_weights()[hce::fi::MAT_A..hce::fi::MAT_A + 10]);
    let labels: Option<std::collections::HashMap<(usize, usize), f32>> = std::env::var("NNUE_LABELS").ok().map(|p| {
        std::fs::read_to_string(p)
            .expect("labels")
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                let line: usize = f.first()?.parse().ok()?;
                let ply: usize = f.get(1)?.parse().ok()?;
                let score: i32 = f.get(2)?.parse().ok()?;
                let mate = f.get(4) == Some(&"1");
                Some(((line, ply), if mate || score.abs() >= 20000 { f32::NAN } else { score as f32 }))
            })
            .collect()
    });
    let open = |ext: &str| BufWriter::with_capacity(1 << 20, std::fs::File::create(format!("{}.{}", base, ext)).expect("create output"));
    let (mut rec, mut idx, mut brd, mut ftr, mut relf) = (open("rec"), open("idx"), open("brd"), open("ft1"), open("rel"));
    let (mut n_rec, mut n_brd, mut n_ftr, mut n_rel) = (0u64, 0u64, 0u64, 0u64);
    let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<Sample>>(256);
    std::thread::scope(|sc| {
        for tid in 0..threads {
            let (lines, tx, labels, local_vals) = (&lines, tx.clone(), &labels, &local_vals);
            sc.spawn(move || {
                let mut g = tid;
                while g < lines.len() {
                    let line = &lines[g];
                    let game = g;
                    g += threads;
                    let mut samples: Vec<(Sample, bool)> = Vec::new();
                    let mut ply = 0;
                    let Some(white) = replay_game(line, |s, t, played| {
                        ply += 1;
                        if (no_check || quiet) && s.has_phantom_check() {
                            return;
                        }
                        if quiet {
                            let present = s.get_present();
                            let (amin, amax) = s.get_active_range();
                            let h = s.get_board_size().1;
                            let gain: i32 = (amin..=amax)
                                .filter(|&l| s.get_timeline_end(l) == present)
                                .map(|l| kairos::engine::local::capture_gain(s.board(l, present.0, present.1), present.1, h, local_vals))
                                .sum();
                            if gain > 0 || tactical(s, played) {
                                return;
                            }
                        }
                        let relabelled = match labels {
                            Some(m) => match m.get(&(game, ply)) {
                                Some(&v) => Some(v),
                                None => return,
                            },
                            None => None,
                        };
                        let stm = s.get_present().1;
                        let h = s.get_board_size().1;
                        let sd = hce::sides(s);
                        let teacher = match (relabelled, t) {
                            (Some(v), _) if !keep_teacher => v,
                            (_, Some(v)) if v.abs() < 20000 => v as f32,
                            _ => f32::NAN,
                        };
                        let (head, dense) = ([teacher, 0.0, game as f32, ply as f32], nnue::dense(&sd, s));
                        let mut fb = [0u16; nnue::MAX_BOARD_FEATURES];
                        let boards = nnue::boards(s)
                            .map(|bi| {
                                let n = nnue::board_features(bi.board, h, stm, &mut fb);
                                (bi.role, h, fb[..n].to_vec())
                            })
                            .collect();
                        let mut rel = Vec::new();
                        nnue::relation_features(s, &mut rel);
                        samples.push((Sample { head, dense, boards, rel }, stm));
                    }) else {
                        continue;
                    };
                    // An action-limit draw is a game cut off, not a result: scores only.
                    let censored = json_field(line, "reason").is_some_and(|r| r.starts_with("action limit"));
                    let done: Vec<Sample> = samples
                        .into_iter()
                        .map(|(mut x, black)| {
                            x.head[1] = if censored { f32::NAN } else if black { 1.0 - white } else { white };
                            x
                        })
                        .collect();
                    if tx.send(done).is_err() {
                        return;
                    }
                }
            });
        }
        drop(tx);
        for batch in rx {
            for x in batch {
                for v in &x.head {
                    rec.write_all(&v.to_le_bytes()).unwrap();
                }
                for v in &x.dense {
                    rec.write_all(&v.to_le_bytes()).unwrap();
                }
                idx.write_all(&n_brd.to_le_bytes()).unwrap();
                idx.write_all(&n_rel.to_le_bytes()).unwrap();
                for f in &x.rel {
                    relf.write_all(&f.to_le_bytes()).unwrap();
                }
                n_rel += x.rel.len() as u64;
                for (role, h, own) in &x.boards {
                    brd.write_all(&(n_rec as u32).to_le_bytes()).unwrap();
                    brd.write_all(&(*role as u16 | (*h as u16) << 8).to_le_bytes()).unwrap();
                    brd.write_all(&(own.len() as u16).to_le_bytes()).unwrap();
                    brd.write_all(&n_ftr.to_le_bytes()).unwrap();
                    for f in own {
                        ftr.write_all(&f.to_le_bytes()).unwrap();
                    }
                    n_ftr += own.len() as u64;
                    n_brd += 1;
                }
                n_rec += 1;
            }
        }
    });
    for w in [&mut rec, &mut idx, &mut brd, &mut ftr, &mut relf] {
        w.flush().expect("write records");
    }
    writeln!(out, "{} records, {} boards, {} board features, {} relation features ({} dense inputs) from {} games -> {}.*", n_rec, n_brd, n_ftr, n_rel, nnue::ND, lines.len(), base).unwrap();
}

/// Whether action `played` (5DUCI moves) of the side to move in `s` captures or gives check.
fn tactical(s: &State, played: &str) -> bool {
    use kairos::core::action::ExtMove;
    let c = s.get_present().1;
    let captures = played.split_whitespace().filter_map(|t| ExtMove::parse(t).ok()).any(|m| {
        let to = m.fm.to;
        s.get_board_ptr(to.l(), to.t(), c).is_some_and(|b| b.hostile(c) & kairos::core::bitboard::pmask(to.xy()) != 0)
    });
    if captures {
        return true;
    }
    let mut after = s.clone();
    kairos::core::variants::apply_uci_moves(&mut after, &format!("{} submit", played)).is_ok() && after.has_phantom_check()
}

/// `nnueeval model.nnue file.g`: the NNUE evaluation of each position (internal units, side to
/// move's view), for checking the trainer's export against the engine.
fn nnueeval(args: &[String], out: &mut impl Write) {
    use kairos::engine::{hce, nnue};
    let m = nnue::Model::from_bytes(&std::fs::read(&args[0]).expect("model")).expect("model");
    let mut cache = nnue::Cache::new();
    let text = std::fs::read_to_string(&args[1]).expect("positions");
    for line in text.lines().filter(|l| l.starts_with("G ")) {
        let s = parse_g_line(line).unwrap();
        writeln!(out, "{}", m.evaluate(&s, Some(&hce::sides(&s)), &mut cache)).unwrap();
    }
}

/// `nnuerecs model.nnue prefix [n]`: the NNUE evaluation of the first `n` records `nnuedata`
/// wrote to `prefix.*`, one per line (the trainer's `--parity` prints its own for comparison).
fn nnuerecs(args: &[String], out: &mut impl Write) {
    use kairos::engine::nnue::{Model, ND};
    let m = Model::from_bytes(&std::fs::read(&args[0]).expect("model")).expect("model");
    let read = |ext: &str| std::fs::read(format!("{}.{}", args[1], ext)).expect("records");
    // One view stored (`.ft1`), or both (older exports, `.ftr`).
    let one_view = std::path::Path::new(&format!("{}.ft1", args[1])).exists();
    let (rec, idx, brd, ftr, rel) = (read("rec"), read("idx"), read("brd"), read(if one_view { "ft1" } else { "ftr" }), read("rel"));
    let u = |b: &[u8], at: usize, n: usize| b[at..at + n].iter().rev().fold(0u64, |a, &x| a << 8 | x as u64);
    let rec_len = 16 + 2 * ND;
    let total = rec.len() / rec_len;
    let n: usize = args.get(2).and_then(|x| x.parse().ok()).unwrap_or(20).min(total);
    let boards_total = brd.len() / 16;
    let feat = |i: u64| u(&ftr, 2 * i as usize, 2) as u16;
    for r in 0..n {
        let mut d = [0i16; ND];
        for (i, v) in d.iter_mut().enumerate() {
            *v = u(&rec, r * rec_len + 16 + 2 * i, 2) as u16 as i16;
        }
        let first = u(&idx, 16 * r, 8) as usize;
        let end = if r + 1 < total { u(&idx, 16 * (r + 1), 8) as usize } else { boards_total };
        let r0 = u(&idx, 16 * r + 8, 8) as usize;
        let r1 = if r + 1 < total { u(&idx, 16 * (r + 1) + 8, 8) as usize } else { rel.len() / 2 };
        let relations: Vec<u16> = (r0..r1).map(|i| u(&rel, 2 * i, 2) as u16).collect();
        let mut feats: Vec<(u8, Vec<u16>, Vec<u16>)> = Vec::new();
        for b in first..end {
            let (rh, k, off) = (u(&brd, 16 * b + 4, 2), u(&brd, 16 * b + 6, 2), u(&brd, 16 * b + 8, 8));
            let own: Vec<u16> = (0..k).map(|j| feat(off + j)).collect();
            let other = if one_view { own.iter().map(|&f| kairos::engine::nnue::other_view(f, (rh >> 8) as i32)).collect() } else { (0..k).map(|j| feat(off + k + j)).collect() };
            feats.push(((rh & 0xff) as u8, own, other));
        }
        let refs: Vec<(u8, &[u16], &[u16])> = feats.iter().map(|(r, o, t)| (*r, &o[..], &t[..])).collect();
        writeln!(out, "{}", m.evaluate_features(&refs, &relations, &d)).unwrap();
    }
}

/// `nnuerel file.g [reps]`: the cost of the NNUE relation features against the handcrafted
/// evaluation's counting pass (`hce::sides`), per position, and how many features positions have.
fn nnuerel(args: &[String], out: &mut impl Write) {
    use kairos::engine::{hce, nnue};
    let text = std::fs::read_to_string(&args[0]).expect("positions");
    let reps: usize = args.get(1).and_then(|x| x.parse().ok()).unwrap_or(200);
    let states: Vec<State> = text.lines().filter(|l| l.starts_with("G ")).filter_map(|l| parse_g_line(l).ok()).collect();
    let mut f = Vec::new();
    let (mut n, mut most) = (0, 0);
    let t = Instant::now();
    for _ in 0..reps {
        for s in &states {
            f.clear();
            nnue::relation_features(s, &mut f);
            n += f.len();
            most = most.max(f.len());
        }
    }
    let rel = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let mut sink = 0i64;
    for _ in 0..reps {
        for s in &states {
            sink += hce::sides(s).active as i64;
        }
    }
    let sides = t.elapsed().as_secs_f64();
    // The time-travel move generation alone.
    let t = Instant::now();
    let mut ml = kairos::core::multiverse::MoveList::new();
    for _ in 0..reps {
        for s in &states {
            let (lo, hi) = s.get_lines_range();
            for l in lo..=hi {
                let (tt, c) = s.get_timeline_end(l);
                let b = s.board(l, tt, c);
                for sq in kairos::core::bitboard::marked_pos(b.hostile(!c) & !b.wall()) {
                    ml.clear();
                    s.multiverse().gen_superphysical_moves(kairos::core::vec4::Vec4::from_xy(sq, kairos::core::vec4::Vec4::new(0, 0, tt, l)), c, &mut ml);
                    sink += ml.len() as i64;
                }
            }
        }
    }
    let movegen = t.elapsed().as_secs_f64();
    let calls = (reps * states.len()) as f64;
    writeln!(out, "move generation alone {:.2} us", movegen * 1e6 / calls).unwrap();
    writeln!(out, "{} positions: relations {:.2} us ({:.1} features, most {}), hce sides {:.2} us ({})", states.len(), rel * 1e6 / calls, n as f64 / calls, most, sides * 1e6 / calls, sink % 2).unwrap();
}

/// `labelpick out.jsonl per_game min_weight games.jsonl...`: positions to relabel by deeper
/// search, a few per game (`per_game`), favouring positions with more active timelines and more
/// moves through time onto royals and pieces (`nnue::relation_features`): weight 1, plus the
/// active timelines past the first (up to 4), plus half the relations (up to 8); only positions
/// of weight `min_weight` or more. Writes each game with any picked
/// positions as its record plus `"label_plies":[..]` (1-based engine positions, as `nnuedata`'s
/// ply).
fn labelpick(args: &[String], out: &mut impl Write) {
    use kairos::engine::nnue;
    let per_game: usize = args[1].parse().expect("per_game");
    let min_weight: f64 = args[2].parse().expect("min_weight");
    let mut lines: Vec<String> = Vec::new();
    for f in &args[3..] {
        lines.extend(std::fs::read_to_string(f).expect("games").lines().filter(|l| !l.trim().is_empty()).map(String::from));
    }
    let threads = 14;
    let picked: Vec<Vec<(usize, Vec<usize>)>> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads)
            .map(|tid| {
                let lines = &lines;
                sc.spawn(move || {
                    let mut res = Vec::new();
                    let mut g = tid;
                    while g < lines.len() {
                        let (line, game) = (&lines[g], g);
                        g += threads;
                        // (weight, ply) of each engine position.
                        let mut cands: Vec<(f64, usize)> = Vec::new();
                        let mut rel = Vec::new();
                        if replay_game(line, |s, _, _| {
                            rel.clear();
                            nnue::relation_features(s, &mut rel);
                            let active = { let (a, b) = s.get_active_range(); (b - a + 1) as f64 };
                            let ply = cands.len() + 1;
                            cands.push((1.0 + (active - 1.0).min(4.0) + (rel.len() as f64).min(8.0) / 2.0, ply));
                        })
                        .is_none()
                        {
                            continue;
                        }
                        // Weighted sampling without replacement (exponential keys), seeded by game.
                        let mut z = (game as u64 + 1).wrapping_mul(0x9e3779b97f4a7c15);
                        let mut keyed: Vec<(f64, usize)> = cands
                            .iter()
                            .filter(|c| c.0 >= min_weight)
                            .map(|&(w, ply)| {
                                z ^= z >> 31;
                                z = z.wrapping_mul(0xbf58476d1ce4e5b9).wrapping_add(0x94d049bb133111eb);
                                let u = ((z >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
                                (u.ln() / w, ply)
                            })
                            .collect();
                        keyed.sort_by(|a, b| b.0.total_cmp(&a.0));
                        let mut plies: Vec<usize> = keyed.iter().take(per_game).map(|x| x.1).collect();
                        plies.sort();
                        if !plies.is_empty() {
                            res.push((game, plies));
                        }
                    }
                    res
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut all: Vec<(usize, Vec<usize>)> = picked.into_iter().flatten().collect();
    all.sort();
    let mut text = String::new();
    let mut n = 0;
    for (g, plies) in &all {
        let line = lines[*g].trim_end();
        let line = line.strip_suffix('}').expect("a JSON object");
        n += plies.len();
        text.push_str(&format!("{},\"label_plies\":[{}]}}\n", line, plies.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",")));
    }
    std::fs::write(&args[0], text).expect("write");
    writeln!(out, "{} positions from {} games -> {}", n, all.len(), args[0]).unwrap();
}

/// `labelrun picks.jsonl out.tsv nodes [shard shards threads max_seconds]`: search each picked
/// position of `labelpick`'s games (those with line % shards == shard) to `nodes` nodes, one
/// search per thread, a fresh small table per game; no game is started after `max_seconds`.
/// Each label is written (and flushed) as soon as its search ends, so a job cut short keeps them. Writes per position: line, ply, score (internal units,
/// side to move's view), depth, 1 for a proven mate.
fn labelrun(args: &[String], out: &mut impl Write) {
    use kairos::engine::ab::AlphaBeta;
    use kairos::engine::{Limits, OptionValue};
    use std::sync::atomic::AtomicBool;
    let text = std::fs::read_to_string(&args[0]).expect("picks");
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let nodes: u64 = args[2].parse().expect("nodes");
    let n = |i: usize, d: usize| args.get(i).and_then(|x| x.parse().ok()).unwrap_or(d);
    let (shard, shards) = (n(3, 0), n(4, 1));
    let threads = n(5, std::thread::available_parallelism().map_or(1, |x| x.get()));
    let max_seconds = n(6, 0) as f64;
    let mine: Vec<usize> = (0..lines.len()).filter(|i| i % shards == shard).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let t0 = Instant::now();
    let sink = std::sync::Mutex::new(std::fs::File::create(&args[1]).expect("create labels"));
    let done = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads)
            .map(|_| {
                let (lines, mine, next, sink, done) = (&lines, &mine, &next, &sink, &done);
                sc.spawn(move || {
                    let stop = AtomicBool::new(false);
                    loop {
                        if max_seconds > 0.0 && t0.elapsed().as_secs_f64() > max_seconds {
                            break;
                        }
                        let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(&li) = mine.get(k) else { break };
                        let line = lines[li];
                        let Some(e) = line.find("\"label_plies\":[") else { continue };
                        let t = &line[e + 15..];
                        let plies: Vec<usize> = t[..t.find(']').unwrap_or(0)].split(',').filter_map(|x| x.trim().parse().ok()).collect();
                        let mut e = AlphaBeta::new();
                        e.set_option("Hash", &OptionValue::Int(16));
                        let mut ply = 0;
                        let mut states = Vec::new();
                        let _ = replay_game(line, |s, _, _| {
                            ply += 1;
                            if plies.contains(&ply) {
                                states.push((ply, s.clone()));
                            }
                        });
                        for (ply, s) in states {
                            let mut last = String::new();
                            let limits = Limits { nodes: Some(nodes), ..Default::default() };
                            if e.find_best_move(&s, &limits, &stop, &mut |m| last = m).is_none() {
                                continue;
                            }
                            let depth: i32 = last.split_whitespace().skip_while(|w| *w != "depth").nth(1).and_then(|d| d.parse().ok()).unwrap_or(0);
                            let row = format!("{}\t{}\t{}\t{}\t{}\n", li, ply, e.last_score(), depth, e.last_proven() as u8);
                            let mut f = sink.lock().unwrap();
                            f.write_all(row.as_bytes()).expect("write labels");
                            f.flush().expect("write labels");
                            done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
    });
    let (secs, n) = (t0.elapsed().as_secs_f64(), done.into_inner());
    writeln!(out, "{} positions of {} games in {:.1}s ({:.2} s per position per thread, {} threads)", n, mine.len(), secs, secs * threads as f64 / n.max(1) as f64, threads).unwrap();
}

/// `nnuerandom out.nnue [acc emb hidden racc dense]`: a model with random weights, to measure
/// the evaluation's cost before any training (dense 0: without the handcrafted counts).
fn nnuerandom(args: &[String], out: &mut impl Write) {
    let n = |i: usize, d: usize| args.get(i).and_then(|x| x.parse().ok()).unwrap_or(d);
    let m = kairos::engine::nnue::Model::random(n(1, 128), n(2, 32), n(3, 32), n(4, 64), n(5, 1) != 0, 1);
    std::fs::write(&args[0], m.to_bytes()).expect("write model");
    writeln!(out, "random model {}x{}x{} relations {} dense {} -> {}", m.acc, m.emb, m.hidden, m.racc, m.use_dense, args[0]).unwrap();
}

/// `netdata out.bin games.jsonl...`: eval-net records (NI + 4 f32: inputs, base, teacher or NaN,
/// wdl, game; side to move's view). NETDATA_FEN=<path> also writes single-timeline FENs.
/// NETDATA_NO_CHECK drops in-check positions; use it for held-out sets, as check leaks the result.
fn netdata(args: &[String], out: &mut impl Write) {
    use kairos::engine::evalnet::{inputs, NI};
    use kairos::engine::hce::{evaluate, tuned_weights};
    let out_path = &args[0];
    let mut lines: Vec<String> = Vec::new();
    for f in &args[1..] {
        let text = std::fs::read_to_string(f).expect("games");
        lines.extend(text.lines().filter(|l| !l.trim().is_empty()).map(String::from));
    }
    let threads = 14;
    let no_check = std::env::var_os("NETDATA_NO_CHECK").is_some();
    let fen_path = std::env::var("NETDATA_FEN").ok();
    let want_fen = fen_path.is_some();
    // Base score: the evaluation the search uses (the tuned feature HCE).
    let ep = tuned_weights();
    let chunks: Vec<(Vec<f32>, Vec<String>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads)
            .map(|tid| {
                let (lines, ep) = (&lines, &ep);
                sc.spawn(move || {
                    let mut rec: Vec<f32> = Vec::new();
                    let mut fens: Vec<String> = Vec::new();
                    let mut g = tid;
                    while g < lines.len() {
                        let line = &lines[g];
                        g += threads;
                        let mut pending: Vec<(String, Vec<f32>, bool)> = Vec::new();
                        let Some(white) = replay_game(line, |s, t, _| {
                            if no_check && s.has_phantom_check() {
                                return;
                            }
                            let base = evaluate(s, ep);
                            let x = inputs(s, base);
                            let mut r = x.to_vec();
                            r.push(base as f32);
                            r.push(match t {
                                Some(v) if v.abs() < 20000 => v as f32,
                                _ => f32::NAN,
                            });
                            let mut fen = String::from("-");
                            if want_fen {
                                let (lo, hi) = s.get_lines_range();
                                if lo == hi {
                                    let (t, c) = s.get_timeline_end(lo);
                                    fen = format!("{}|{}", s.board(lo, t, c).get_fen(true), if c { "b" } else { "w" });
                                }
                            }
                            pending.push((fen, r, s.get_present().1));
                        }) else {
                            continue;
                        };
                        for (fen, mut r, stm_black) in pending {
                            r.push(if stm_black { 1.0 - white } else { white });
                            r.push(g as f32 - threads as f32);
                            rec.extend(r);
                            if want_fen {
                                fens.push(fen);
                            }
                        }
                    }
                    (rec, fens)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut bytes: Vec<u8> = Vec::new();
    let mut n = 0usize;
    for (c, _) in &chunks {
        n += c.len() / (NI + 4);
        for v in c {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
    }
    std::fs::write(out_path, &bytes).expect("write records");
    if let Some(p) = &fen_path {
        let mut text = String::new();
        for (_, f) in &chunks {
            for line in f {
                text.push_str(line);
                text.push('\n');
            }
        }
        std::fs::write(p, text).expect("write fens");
    }
    writeln!(out, "{} records ({} inputs + 4) from {} games -> {}", n, NI, lines.len(), out_path).unwrap();
}

/// One position of `ktools hcverify`: (reference action count, whether it is complete, the
/// first generator that repeated an action).
fn hcverify(s: &State, cap: usize, seeds: u64) -> Result<(usize, bool, Option<String>), String> {
    use kairos::core::hypercuboid::{BestFirstCursor, HcCursor};
    use kairos::core::naive::is_legal_action;
    use kairos::engine::ab::AlphaBeta;
    use kairos::misc::ordering::AxisOrders;
    use std::collections::BTreeSet;
    let (w, ss) = HcInfo::build_hc(s);
    let mut reference: BTreeSet<String> = BTreeSet::new();
    for a in w.search(s, ss.clone()) {
        reference.insert(seq_string(&a));
        if reference.len() > cap {
            break;
        }
    }
    let full = reference.len() <= cap;
    let mut repeats: Option<String> = None;
    let mut compare = |name: &str, acts: &[MoveSeq]| -> Result<(), String> {
        let got: BTreeSet<String> = acts.iter().map(seq_string).collect();
        if got.len() != acts.len() && repeats.is_none() {
            let mut seen = BTreeSet::new();
            let dups: Vec<String> = acts.iter().map(seq_string).filter(|a| !seen.insert(a.clone())).take(3).collect();
            repeats = Some(format!("{}: {} actions but {} distinct (repeated {:?})", name, acts.len(), got.len(), dups));
        }
        if full {
            if got != reference {
                let extra: Vec<&String> = got.difference(&reference).take(3).collect();
                let missing: Vec<&String> = reference.difference(&got).take(3).collect();
                return Err(format!("{}: {} actions vs hc {} (extra {:?} missing {:?})", name, got.len(), reference.len(), extra, missing));
            }
        } else {
            if got.len() < cap {
                return Err(format!("{}: only {} actions, hc has more than {}", name, got.len(), cap));
            }
            if let Some(a) = acts.iter().take(cap).find(|a| !is_legal_action(s, a)) {
                return Err(format!("{}: illegal action {}", name, seq_string(a)));
            }
        }
        Ok(())
    };
    // Mate status: no action exactly when the hypercuboid search has none.
    let mate = s.get_mate_type();
    if reference.is_empty() != matches!(mate, MateType::Checkmate | MateType::Stalemate) {
        return Err(format!("mate type {} but hc has {} actions", mate_name(mate), reference.len()));
    }
    // Generators are run to the end (allowing for repeats) or to `cap` distinct actions.
    let limit = 4 * reference.len().min(cap) + 16;
    let take = |next: &mut dyn FnMut() -> Option<MoveSeq>| -> Vec<MoveSeq> {
        let mut acts = Vec::new();
        let mut seen = BTreeSet::new();
        while acts.len() < limit && seen.len() < cap {
            match next() {
                Some(a) => {
                    seen.insert(seq_string(&a));
                    acts.push(a);
                }
                None => break,
            }
        }
        acts
    };
    // The engine's interior-node enumeration.
    let engine = AlphaBeta::new();
    compare("best-first (engine order)", &engine.node_order(s, limit))?;
    // Best-first under random scores (orders sorted by them, as the cursor requires).
    let mut st = (s.hash() | 1).wrapping_mul(0x9E3779B97F4A7C15);
    let mut rnd = move || {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        st
    };
    for seed in 0..seeds {
        let (w2, ss2) = HcInfo::build_hc(s);
        let u = w2.get_universe();
        let mut cur = BestFirstCursor::default();
        {
            let (orders, scores) = cur.ordering_mut();
            let mut lists = Vec::new();
            scores.clear();
            for n in 0..u.dimension() {
                let len = u[n].max().map(|i| i as usize + 1).unwrap_or(0);
                // Few distinct values, so ties are common.
                let sc: Vec<i32> = (0..len).map(|_| (rnd() % 7) as i32 - 3).collect();
                let mut v: Vec<u32> = u[n].iter().collect();
                v.sort_by_key(|&i| (std::cmp::Reverse(sc[i as usize]), i));
                lists.push(v);
                scores.push(sc);
            }
            *orders = AxisOrders::new(lists);
        }
        cur.restart(&w2, ss2);
        let acts = take(&mut || cur.next(&w2, s));
        compare(&format!("best-first (random order {})", seed), &acts)?;
    }
    // The evasion cursor under the engine ordering.
    let (w3, ss3) = HcInfo::build_hc(s);
    let mut cur = HcCursor::new(ss3, engine.ordering(s, &w3));
    let acts = take(&mut || cur.next(&w3, s));
    compare("depth-first (engine order)", &acts)?;
    Ok((reference.len(), full, repeats))
}

/// `overrunpos <file.g>... [threads=1,4] [ms=100,1000] [clock=2000+100,30000+300]`: reports
/// how far each search runs past its hard time limit, and searches returning no or illegal
/// actions. `clock` is time left + increment in ms; 30 s past the limit counts as hung.
fn overrunpos(args: &[String], out: &mut impl Write) {
    use kairos::engine::ab::AlphaBeta;
    use kairos::engine::time::TimePlan;
    use kairos::engine::{Limits, OptionValue};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let mut threads: Vec<usize> = vec![1, 4];
    let mut limits: Vec<(String, Limits)> = Vec::new();
    let mut items: Vec<(String, State)> = Vec::new();
    let list = |v: &str| v.split(',').map(|x| x.to_string()).collect::<Vec<_>>();
    let (mut mss, mut clocks) = (list("100,1000"), list("2000+100,30000+300"));
    for a in args {
        if let Some(v) = a.strip_prefix("threads=") {
            threads = v.split(',').filter_map(|x| x.parse().ok()).collect();
        } else if let Some(v) = a.strip_prefix("ms=") {
            mss = list(v);
        } else if let Some(v) = a.strip_prefix("clock=") {
            clocks = list(v);
        } else {
            let text = std::fs::read_to_string(a).expect("positions");
            let mut label = String::new();
            for line in text.lines() {
                if let Some(l) = line.strip_prefix("# ") {
                    label = l.to_string();
                } else if line.starts_with("G ") {
                    let s = parse_g_line(line).unwrap();
                    let name = if label.is_empty() { format!("{} #{}", a, items.len()) } else { std::mem::take(&mut label) };
                    items.push((name, s));
                }
            }
        }
    }
    for m in mss.iter().filter(|m| !m.is_empty()) {
        limits.push((format!("movetime {}", m), Limits { movetime: m.parse().ok(), ..Default::default() }));
    }
    for c in clocks.iter().filter(|c| !c.is_empty()) {
        let (t, i) = c.split_once('+').unwrap_or((c, "0"));
        limits.push((format!("clock {}", c), Limits { time_left: t.parse().ok(), increment: i.parse().ok(), ..Default::default() }));
    }
    let mut worst_all = 0.0f64;
    for &th in &threads {
        for (lname, lim) in &limits {
            let (mut worst, mut late, mut bad, mut n) = (f64::MIN, 0, 0, 0);
            let mut worst_at = String::new();
            for (name, s) in &items {
                let boards = {
                    let present = s.get_present();
                    let (amin, amax) = s.get_active_range();
                    (amin..=amax).filter(|&l| s.get_timeline_end(l) == present).count()
                };
                let hard = TimePlan::new(lim, boards, s.active_timelines()).map(|p| p.maximum).unwrap_or(0) as f64;
                let stop = Arc::new(AtomicBool::new(false));
                let (tx, rx) = std::sync::mpsc::channel();
                let (s2, lim2, stop2) = (s.clone(), lim.clone(), stop.clone());
                std::thread::spawn(move || {
                    let mut e = AlphaBeta::new();
                    e.set_option("Threads", &OptionValue::Int(th as i64));
                    let t0 = Instant::now();
                    let a = e.find_best_move(&s2, &lim2, &stop2, &mut |_| {});
                    let legal = a.as_ref().map(|a| s2.can_apply_action(a).is_some());
                    let _ = tx.send((t0.elapsed().as_secs_f64() * 1e3, legal));
                });
                n += 1;
                let (ms, legal) = match rx.recv_timeout(std::time::Duration::from_millis(hard as u64 + 30_000)) {
                    Ok(r) => r,
                    Err(_) => {
                        stop.store(true, Ordering::Relaxed);
                        (f64::INFINITY, None)
                    }
                };
                let over = ms - hard;
                if over > worst {
                    worst = over;
                    worst_at = name.clone();
                }
                let note = match legal {
                    Some(true) => "",
                    Some(false) => " ILLEGAL ACTION",
                    None if ms.is_finite() => " no action",
                    None => " HUNG",
                };
                if !note.is_empty() && note != " no action" {
                    bad += 1;
                }
                if over > 50.0 || !note.is_empty() {
                    late += (over > 50.0) as usize;
                    writeln!(out, "  threads {} {} | {} | limit {:.0} ms, took {:.0} ms ({:+.0}){}", th, lname, name, hard, ms, over, note).unwrap();
                }
                out.flush().unwrap();
            }
            worst_all = worst_all.max(worst);
            writeln!(out, "threads {} {}: {} searches, {} over by more than 50 ms, {} hung/illegal, worst {:+.0} ms ({})", th, lname, n, late, bad, worst, worst_at).unwrap();
            out.flush().unwrap();
        }
    }
    writeln!(out, "worst overrun {:+.0} ms", worst_all).unwrap();
    out.flush().unwrap();
    std::process::exit(0);
}

/// Time to the first legal action under several enumeration strategies (each capped).
///   ktools firstact <file.g> [cap ms]
fn firstact(path: &str, cap: u64, out: &mut impl Write) {
    use kairos::core::hypercuboid::{interrupt_on, interrupted, HcCursor};
    let text = std::fs::read_to_string(path).expect("positions");
    let e = kairos::engine::ab::AlphaBeta::new();
    let stop = std::sync::atomic::AtomicBool::new(false);
    let deadline = || Some(Instant::now() + std::time::Duration::from_millis(cap));
    for line in text.lines().filter(|l| l.starts_with("G ")) {
        let s = parse_g_line(line).unwrap();
        let t = Instant::now();
        let (w, ss) = HcInfo::build_hc(&s);
        writeln!(out, "dim {} playable {} hcs {} in_check {} build {:.1}ms", w.dimension(), w.first_branch_axis(), ss.len(), s.has_phantom_check(), t.elapsed().as_secs_f64() * 1e3).unwrap();
        out.flush().unwrap();
        {
            let _g = interrupt_on(&stop, deadline());
            let t = Instant::now();
            let a = w.search(&s, ss.clone()).next();
            writeln!(out, "  natural, full: {} in {:.0}ms (interrupted {})", a.is_some(), t.elapsed().as_secs_f64() * 1e3, interrupted()).unwrap();
            out.flush().unwrap();
        }
        {
            let _g = interrupt_on(&stop, deadline());
            let t = Instant::now();
            let mut cur = HcCursor::new(ss.clone(), e.ordering(&s, &w));
            let a = cur.next(&w, &s);
            writeln!(out, "  engine order depth-first, full: {} in {:.0}ms (interrupted {})", a.is_some(), t.elapsed().as_secs_f64() * 1e3, interrupted()).unwrap();
            out.flush().unwrap();
        }
        {
            let _g = interrupt_on(&stop, deadline());
            let t = Instant::now();
            let a = e.node_order(&s, 1);
            writeln!(out, "  best-first, full: {} in {:.0}ms (interrupted {})", !a.is_empty(), t.elapsed().as_secs_f64() * 1e3, interrupted()).unwrap();
            out.flush().unwrap();
        }
    }
}

/// Run the calling process below normal priority (long diagnostic runs share the machine).
fn lower_priority() {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn SetPriorityClass(process: *mut std::ffi::c_void, class: u32) -> i32;
        }
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x4000;
        // SAFETY: plain Win32 call on the current process' pseudo-handle.
        unsafe { SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS) };
    }
}

/// One `matecheck` case: a start position, its G line, and (replaying a record) the actions to
/// play instead of the engines' own.
struct McCase {
    id: String,
    start: State,
    g: String,
    forced: Option<Vec<String>>,
}

/// One searched ply of a `matecheck` line.
struct McPly {
    /// Side to move (true: black).
    black: bool,
    /// The engine's score (internal units, side to move's view), whether a mate score is
    /// proven, and the depth of its last report.
    score: i32,
    proven: bool,
    depth: i32,
    /// The mate (in moves, < 0: mated) of the last depth reported, when it was one.
    reported: Option<i32>,
    /// The action played (as moves for a G line, and in LAN).
    uci: String,
    lan: String,
}

/// Mate distance in moves of an internal score: > 0 the side to move mates, < 0 it is mated.
fn mc_mate(score: i32) -> Option<i32> {
    use kairos::engine::ab::MATE;
    if score > MATE - 1000 {
        Some((MATE - score + 1) / 2)
    } else if score < -(MATE - 1000) {
        Some(-((MATE + score + 1) / 2))
    } else {
        None
    }
}

/// The G line of `s` (its moves continued by `acts`).
fn mc_g_line(base: &str, acts: &[&str]) -> String {
    let mut g = base.to_string();
    if !acts.is_empty() {
        if !g.contains(" moves") {
            g += " moves";
        }
        for a in acts {
            g += &format!(" {} submit", a);
        }
    }
    g
}

/// Cases from `path`: G lines (one start each), `R <id>\t<G line>\t<action>;<action>...`
/// records (replayed with these actions), or harness JSONL games (each game's first claimed
/// mate: the position two actions before the first `evals` entry with a mate score).
fn mc_cases(path: &str, max: usize) -> Vec<McCase> {
    use kairos::core::variants::{apply_uci_moves, position_from_uci};
    let text = std::fs::read_to_string(path).expect("cases");
    let mut cases = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if cases.len() >= max {
            break;
        }
        if line.starts_with("G ") {
            let Ok(s) = parse_g_line(line) else { continue };
            cases.push(McCase { id: format!("{}:{}", path.rsplit(['/', '\\']).next().unwrap_or(path), n + 1), start: s, g: line.to_string(), forced: None });
        } else if let Some(r) = line.strip_prefix("R ") {
            let mut f = r.split('\t');
            let (Some(id), Some(g), acts) = (f.next(), f.next(), f.next().unwrap_or("")) else { continue };
            let Ok(s) = parse_g_line(g) else { continue };
            let forced = acts.split(';').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
            cases.push(McCase { id: id.to_string(), start: s, g: g.to_string(), forced: Some(forced) });
        } else if line.starts_with('{') {
            let (Some(pos), Some(moves)) = (json_field(line, "position"), json_field(line, "moves")) else { continue };
            let Some(e) = line.find("\"evals\":") else { continue };
            let Some(list) = line[e + 8..].trim_start().strip_prefix('[') else { continue };
            let evals: Vec<Option<i32>> = list.split(']').next().unwrap_or("").split(',').map(|x| x.trim().parse().ok()).collect();
            let mate = if json_field(line, "eval_scale").is_some() { 20000 } else { kairos::engine::ab::MATE - 1000 };
            let acts: Vec<&str> = moves.split(" submit").map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
            // The evals are of the actions after the opening's.
            let book: usize = json_field(line, "book").and_then(|b| b.parse().ok()).unwrap_or(0);
            if evals.len() + book != acts.len() {
                continue;
            }
            let Some(claim) = evals.iter().position(|v| v.is_some_and(|v| v >= mate)).map(|i| i + book) else { continue };
            let start = claim.saturating_sub(2);
            let Ok(mut s) = position_from_uci(pos) else { continue };
            if !acts[..start].iter().all(|a| apply_uci_moves(&mut s, &format!("{} submit", a)).is_ok()) {
                continue;
            }
            let g = mc_g_line(&format!("G {} {}", s.get_promotion_options().0, pos), &acts[..start]);
            let game = json_field(line, "game").unwrap_or("?");
            cases.push(McCase { id: format!("{}:{}#{}@{}", path.rsplit(['/', '\\']).next().unwrap_or(path), n + 1, game, start), start: s, g, forced: None });
        }
    }
    cases
}

/// Plays one case with one engine per side (or along the record), searching every position.
/// Stops at mate, at `max_plies`, if no mate is claimed in 4 plies, or soon after a claimed
/// mate is overdue.
fn mc_play(case: &McCase, nodes: u64, max_plies: usize, depth_limit: u32, max_ms: u64) -> (Vec<McPly>, bool) {
    use kairos::engine::ab::AlphaBeta;
    use std::sync::atomic::{AtomicBool, Ordering};
    let mut engines = [AlphaBeta::new(), AlphaBeta::new()];
    let mut s = case.start.clone();
    let mut plies: Vec<McPly> = Vec::new();
    // Ply by which the first claimed mate should be over.
    let mut due: Option<usize> = None;
    for j in 0..max_plies {
        // A ply gets `max_ms` at most (finding a first action, or the root's actions, can take
        // minutes in big positions): then the search is stopped.
        let stop = AtomicBool::new(false);
        let done = AtomicBool::new(false);
        let r = std::thread::scope(|sc| {
            sc.spawn(|| {
                let t = Instant::now();
                while !done.load(Ordering::Relaxed) {
                    if t.elapsed().as_millis() as u64 >= max_ms {
                        stop.store(true, Ordering::Relaxed);
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            });
            let r = mc_ply(case, j, &s, &mut engines, &stop, nodes, depth_limit);
            done.store(true, Ordering::Relaxed);
            r
        });
        let Some((ply, next)) = r else { break };
        if due.is_none()
            && case.forced.is_none()
            && let Some(m) = mc_mate(ply.score).filter(|&m| m > 0)
        {
            due = Some(j + 2 * m as usize - 2);
        }
        plies.push(ply);
        s = next;
        if let Some(f) = &case.forced {
            if j + 1 >= f.len() {
                break;
            }
        } else if due.is_some_and(|d| j + 1 > d + 4) || (due.is_none() && j + 1 >= 4) {
            break;
        }
    }
    let mated = s.get_mate_type() == MateType::Checkmate;
    (plies, mated)
}

/// One ply of `mc_play`: the search of `s` and the position after the action played (None: the
/// game is over, or the record's action does not apply).
#[allow(clippy::too_many_arguments)]
fn mc_ply(
    case: &McCase,
    j: usize,
    s: &State,
    engines: &mut [kairos::engine::ab::AlphaBeta; 2],
    stop: &std::sync::atomic::AtomicBool,
    nodes: u64,
    depth_limit: u32,
) -> Option<(McPly, State)> {
    use kairos::engine::Limits;
    {
        let _g = kairos::core::hypercuboid::interrupt_on(stop, None);
        let (w, ss) = HcInfo::build_hc(s);
        w.search(s, ss).next()?;
    }
    let forced = match &case.forced {
        Some(f) => Some(f.get(j)?.as_str()),
        None => None,
    };
    let black = s.get_present().1;
    let e = &mut engines[black as usize];
    let mut last = String::new();
    let limits = Limits { nodes: Some(nodes).filter(|&n| n > 0), depth: Some(depth_limit), ..Default::default() };
    let best = e.find_best_move(s, &limits, stop, &mut |m| last = m)?;
    let depth: i32 = last.split_whitespace().nth(1).and_then(|d| d.parse().ok()).unwrap_or(0);
    let reported: Option<i32> = last.split_whitespace().skip_while(|w| *w != "score").nth(2).filter(|_| last.contains(" score mate ")).and_then(|m| m.parse().ok());
    let (score, proven) = (e.last_score(), e.last_proven());
    if let Some(uci) = forced {
        let mut c = s.clone();
        kairos::core::variants::apply_uci_moves(&mut c, &format!("{} submit", uci)).ok()?;
        return Some((McPly { black, score, proven, depth, reported, uci: uci.to_string(), lan: uci.to_string() }, c));
    }
    let uci = best.moves().iter().map(|m| m.to_string()).collect::<Vec<_>>().join(" ");
    let lan = kairos::web::tree::action_lan(s, &best);
    let next = s.can_apply_action(&best)?;
    Some((McPly { black, score, proven, depth, reported, uci, lan }, next))
}

/// Inconsistencies of a played line (see `matecheck`), as (ply, kind, detail).
/// (ply, kind, detail) of one inconsistency.
type McFlag = (usize, &'static str, String);

fn mc_flags(plies: &[McPly], mated: bool) -> (Option<(usize, i32)>, Vec<McFlag>) {
    let mut flags = Vec::new();
    for (j, p) in plies.iter().enumerate() {
        if let Some(m) = p.reported.filter(|&m| mc_mate(p.score) != Some(m)) {
            flags.push((j, "DROPPED", format!("depth {} reported mate {}, the action played scored {}", p.depth, m, mc_text(p.score))));
        }
    }
    let Some(k) = plies.iter().position(|p| mc_mate(p.score).is_some_and(|m| m > 0)) else {
        return (None, flags);
    };
    let n = mc_mate(plies[k].score).unwrap();
    let winner = plies[k].black;
    // Before the claim: the loser's search let the mate in although it searched deep enough
    // to see it, and the winner's search one move earlier saw no mate that deep.
    if k >= 1 && plies[k - 1].black != winner && mc_mate(plies[k - 1].score).is_none_or(|m| m > 0) && plies[k - 1].depth >= 2 * n {
        flags.push((k - 1, "WALKED", format!("loser depth {} saw {}, then mate {}", plies[k - 1].depth, mc_text(plies[k - 1].score), n)));
    }
    if k >= 2 && plies[k - 2].black == winner && plies[k - 2].depth > 2 * n {
        flags.push((k - 2, "EARLY", format!("winner depth {} saw {}, then mate {} after the reply", plies[k - 2].depth, mc_text(plies[k - 2].score), n)));
    }
    let mut bound = n;
    for (j, p) in plies.iter().enumerate().skip(k + 1) {
        let m = mc_mate(p.score);
        if bound < 1 || (p.black == winner && bound < 2) {
            // Past the claimed mate (see UNREALIZED).
            break;
        }
        if p.black == winner {
            let want = bound - 1;
            match m {
                Some(m) if m > 0 => {
                    if m > want {
                        flags.push((j, "LONGER", format!("mate {} where mate <= {}", m, want)));
                    }
                    bound = m;
                }
                _ => {
                    let kind = if p.depth >= 2 * want - 1 { "LOST" } else { "FADED" };
                    flags.push((j, kind, format!("{} at depth {} where mate <= {}", mc_text(p.score), p.depth, want)));
                    bound = want;
                }
            }
        } else if m.is_none_or(|m| m > 0) {
            let kind = if m.is_some() { "REFUTED" } else if p.depth >= 2 * bound { "BLIND" } else { "UNSEEN" };
            flags.push((j, kind, format!("loser {} at depth {} where mated in <= {}", mc_text(p.score), p.depth, bound)));
        }
    }
    let end = plies.len();
    // The mating action is due at this ply.
    let due = k + 2 * n as usize - 2;
    if !mated || plies.last().is_some_and(|p| p.black != winner) {
        flags.push((end, "UNREALIZED", format!("no mate by ply {} (due at {})", end, due)));
    } else if end - 1 > due {
        flags.push((end, "SLOW", format!("mated at ply {} (due at {})", end - 1, due)));
    }
    (Some((k, n)), flags)
}

fn mc_text(score: i32) -> String {
    match mc_mate(score) {
        Some(m) => format!("M{}", m),
        None => format!("cp{}", kairos::engine::report_cp(score)),
    }
}

/// `matecheck <cases> [nodes] [max_cases] [threads] [records_out]`: plays out each case and
/// checks that a claimed mate in N becomes mate in N-1 or less and the loser sees it coming.
/// Cases: G lines, records or JSONL games. Env: MC_DEPTH, MC_MAX_PLIES, MC_MS.
fn matecheck(args: &[String], out: &mut impl Write) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    lower_priority();
    let path = args.first().map(|s| s.as_str()).unwrap_or("data/matecheck.g");
    let nodes: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(40_000);
    let max: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
    let threads: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(4).max(1);
    let records = args.get(4);
    let max_plies: usize = std::env::var("MC_MAX_PLIES").ok().and_then(|v| v.parse().ok()).unwrap_or(40);
    let depth_limit: u32 = std::env::var("MC_DEPTH").ok().and_then(|v| v.parse().ok()).unwrap_or(64);
    let max_ms: u64 = std::env::var("MC_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(60_000);
    let cases = mc_cases(path, max);
    let next = AtomicUsize::new(0);
    let t0 = Instant::now();
    let mut counts: std::collections::BTreeMap<&'static str, usize> = std::collections::BTreeMap::new();
    let (mut claimed, mut clean, mut proven_false, mut total) = (0, 0, 0, 0);
    let mut rec = records.map(|p| std::io::BufWriter::new(std::fs::File::create(p).expect("records file")));
    std::thread::scope(|sc| {
        let (tx, rx) = std::sync::mpsc::channel::<(usize, Vec<McPly>, bool)>();
        for _ in 0..threads {
            let (cases, next, tx) = (&cases, &next, tx.clone());
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn_scoped(sc, move || loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(c) = cases.get(i) else { break };
                    let (plies, mated) = mc_play(c, nodes, max_plies, depth_limit, max_ms);
                    if tx.send((i, plies, mated)).is_err() {
                        break;
                    }
                })
                .unwrap();
        }
        drop(tx);
        // Reported as they finish.
        for (i, plies, mated) in rx {
            total += 1;
            let c = &cases[i];
            let (claim, flags) = mc_flags(&plies, mated);
            let line: Vec<String> = plies
                .iter()
                .map(|p| {
                    let was = match p.reported.filter(|&m| Some(m) != mc_mate(p.score)) {
                        Some(m) => format!("(M{})", m),
                        None => String::new(),
                    };
                    format!("{}{}/{}{}", mc_text(p.score), if p.proven && mc_mate(p.score).is_some() { "!" } else { "" }, p.depth, was)
                })
                .collect();
            writeln!(out, "case {} | {}{}", c.id, line.join(" "), if mated { " #" } else { "" }).unwrap();
            let kinds: std::collections::BTreeSet<&str> = flags.iter().map(|f| f.1).collect();
            for kd in &kinds {
                *counts.entry(kd).or_insert(0) += 1;
            }
            if let Some((k, n)) = claim {
                claimed += 1;
                if flags.iter().all(|f| f.1 == "EARLY") {
                    clean += 1;
                }
                // A proven mate the line then failed to deliver would be a bug in the proof.
                if plies[k].proven && kinds.contains("UNREALIZED") {
                    proven_false += 1;
                }
                writeln!(out, "  claim ply {} mate {} ({}){}", k, n, if plies[k].black { "black" } else { "white" }, if plies[k].proven { " proven" } else { "" }).unwrap();
            }
            for (j, kind, detail) in &flags {
                let act = plies.get(*j).map(|p| format!(" [played {}]", p.lan)).unwrap_or_default();
                writeln!(out, "  {:<10} ply {:>2} {}{}", kind, j, detail, act).unwrap();
            }
            if let Some(w) = rec.as_mut() {
                writeln!(w, "R {}\t{}\t{}", c.id, c.g, plies.iter().map(|p| p.uci.as_str()).collect::<Vec<_>>().join(";")).unwrap();
                w.flush().unwrap();
            }
            out.flush().unwrap();
        }
    });
    writeln!(out, "cases {} | mate claimed {} | consistent {} | flagged cases by kind {:?} | proven claims undelivered {} | {} nodes/search, {:.0}s", total, claimed, clean, counts, proven_false, nodes, t0.elapsed().as_secs_f64()).unwrap();
}

/// `bookeval [nodes] [threads] [out.tsv] [max_secs] [depth_cap] [book_file]`: searches every
/// book position once with a fixed node budget and writes its score, best action and PV.
/// Also writes `<out>.edges.tsv` and `<out>.positions.tsv`; resumes past rows already written.
fn bookeval(args: &[String], out: &mut impl Write) {
    use kairos::engine::ab::AlphaBeta;
    use kairos::engine::{Limits, OptionValue};
    use kairos::web::tree::{parse_lan_moves, play_action};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    let nodes: u64 = args.first().and_then(|s| s.parse().ok()).unwrap_or(2_000_000);
    let threads: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(8);
    let path = args.get(2).cloned().unwrap_or_else(|| "docs/theory/book_evals.tsv".to_string());
    let max_secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(600);
    let depth_cap: u32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(30);
    let book_file = args.get(5).cloned().unwrap_or_else(|| "data/book_theory.txt".to_string());
    let text = std::fs::read_to_string(&book_file).expect("book file");
    // Positions by frontier key (the book's own key), the first state seen; edges between them.
    let mut states: HashMap<u64, State> = HashMap::new();
    let mut order: Vec<u64> = Vec::new();
    struct Edge {
        parent: u64,
        child: u64,
        tag: String,
        weight: String,
        lan: String,
        pgn: String,
    }
    let mut edges: Vec<Edge> = Vec::new();
    let mut names: HashMap<u64, String> = HashMap::new();
    // The source comment block printed before a position (the guides' notes on the line it starts).
    let mut notes: HashMap<u64, String> = HashMap::new();
    let mut pending: Vec<String> = Vec::new();
    let mut cur: Option<(u64, State)> = None;
    // The distinct histories (full hashes) that reach one frontier position: the book keys by the boards in
    // play, so these are merged, but the search of the first one does not tell about the others.
    let mut histories: HashMap<u64, std::collections::HashSet<u64>> = HashMap::new();
    let mut add = |s: &State, states: &mut HashMap<u64, State>, order: &mut Vec<u64>| -> u64 {
        let k = s.frontier_hash();
        histories.entry(k).or_default().insert(s.hash());
        if let std::collections::hash_map::Entry::Vacant(e) = states.entry(k) {
            e.insert(s.clone());
            order.push(k);
        }
        k
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            pending.clear();
        } else if let Some(c) = line.strip_prefix('#') {
            pending.push(c.trim().to_string());
        }
        if let Some(g) = line.strip_prefix("P ") {
            let s = parse_g_line(g).expect("book position");
            let k = add(&s, &mut states, &mut order);
            if !pending.is_empty() {
                notes.entry(k).or_insert_with(|| pending.join(" "));
            }
            pending.clear();
            cur = Some((k, s));
        } else if let Some(n) = line.strip_prefix("N ") {
            let (k, _) = cur.as_ref().expect("name after position");
            names.entry(*k).or_insert_with(|| n.trim().to_string());
        } else if let Some(m) = line.strip_prefix("m ") {
            let (k, s) = cur.as_ref().expect("move after position");
            let (head, pgn) = m.split_once(';').unwrap_or((m, ""));
            let mut it = head.split_whitespace();
            let tag = it.next().unwrap_or("").to_string();
            let weight = it.next().unwrap_or("0").to_string();
            let lan = it.collect::<Vec<_>>().join(" ");
            let moves = parse_lan_moves(&lan).expect("lan");
            let child = play_action(s, &moves).expect("legal book action");
            let ck = add(&child, &mut states, &mut order);
            if !edges.iter().any(|e| e.parent == *k && e.lan == lan) {
                edges.push(Edge { parent: *k, child: ck, tag, weight, lan, pgn: pgn.trim().to_string() });
            }
        }
    }
    // A line (PGN actions) from a root to every position, by breadth-first search.
    let mut children: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, e) in edges.iter().enumerate() {
        children.entry(e.parent).or_default().push(i);
    }
    let is_child: std::collections::HashSet<u64> = edges.iter().map(|e| e.child).collect();
    let mut line_to: HashMap<u64, String> = HashMap::new();
    let mut queue = std::collections::VecDeque::new();
    for &k in &order {
        if !is_child.contains(&k) {
            line_to.insert(k, String::new());
            queue.push_back(k);
        }
    }
    while let Some(k) = queue.pop_front() {
        let here = line_to[&k].clone();
        for &i in children.get(&k).map(|v| v.as_slice()).unwrap_or(&[]) {
            let e = &edges[i];
            if let std::collections::hash_map::Entry::Vacant(v) = line_to.entry(e.child) {
                v.insert(if here.is_empty() { e.pgn.clone() } else { format!("{} {}", here, e.pgn) });
                queue.push_back(e.child);
            }
        }
    }
    let turn_of = |k: u64| -> (i32, bool) {
        let (t, c) = states[&k].get_present();
        (t, c)
    };
    {
        let mut f = std::fs::File::create(format!("{}.edges.tsv", path)).expect("edges file");
        writeln!(f, "parent\tchild\ttag\tweight\tlan\tpgn\tparent_line").unwrap();
        for e in &edges {
            writeln!(f, "{:016x}\t{:016x}\t{}\t{}\t{}\t{}\t{}", e.parent, e.child, e.tag, e.weight, e.lan, e.pgn, line_to.get(&e.parent).cloned().unwrap_or_default()).unwrap();
        }
        let mut f = std::fs::File::create(format!("{}.positions.tsv", path)).expect("positions file");
        writeln!(f, "key\tturn\tside\tname\tline\tnote\thistories").unwrap();
        for &k in &order {
            let (t, c) = turn_of(k);
            writeln!(f, "{:016x}\t{}\t{}\t{}\t{}\t{}\t{}", k, t, if c { "b" } else { "w" }, names.get(&k).cloned().unwrap_or_default(), line_to.get(&k).cloned().unwrap_or_default(), notes.get(&k).cloned().unwrap_or_default().replace('\t', " "), histories.get(&k).map_or(1, |h| h.len())).unwrap();
        }
    }
    let done: std::collections::HashSet<String> = std::fs::read_to_string(&path)
        .map(|t| t.lines().filter_map(|l| l.split('\t').next().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let mut todo: Vec<u64> = order.iter().copied().filter(|k| !done.contains(&format!("{:016x}", k))).collect();
    todo.sort_by_key(|&k| turn_of(k));
    if let Ok(n) = std::env::var("BOOKEVAL_LIMIT") {
        todo.truncate(n.parse().unwrap_or(usize::MAX));
    }
    writeln!(out, "{} positions, {} actions, {} to search ({} nodes each, {} threads)", order.len(), edges.len(), todo.len(), nodes, threads).unwrap();
    out.flush().ok();
    let file = {
        let new = !std::path::Path::new(&path).exists();
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).expect("out file");
        if new {
            writeln!(f, "key\tturn\tside\tdepth\tkind\tscore\tbest\tnodes\tsecs\tpv").unwrap();
        }
        Arc::new(Mutex::new(f))
    };
    let total = todo.len();
    let todo = Arc::new(todo);
    let states = Arc::new(states);
    let next = Arc::new(AtomicUsize::new(0));
    let t0 = Instant::now();
    let mut handles = Vec::new();
    for _ in 0..threads {
        let (todo, states, next, file) = (todo.clone(), states.clone(), next.clone(), file.clone());
        handles.push(
            std::thread::Builder::new()
                .stack_size(256 << 20)
                .spawn(move || {
                    let stop = AtomicBool::new(false);
                    let mut e = AlphaBeta::new();
                    e.set_option("Hash", &OptionValue::Int(64));
                    e.set_option("Threads", &OptionValue::Int(1));
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= todo.len() {
                            break;
                        }
                        let k = todo[i];
                        let s = &states[&k];
                        e.new_game();
                        let mut last = String::new();
                        let t = Instant::now();
                        let limits = Limits { depth: Some(depth_cap), nodes: Some(nodes), movetime: Some(max_secs * 1000), ..Default::default() };
                        let best = e.find_best_move(s, &limits, &stop, &mut |m| {
                            if m.contains("score ") && m.starts_with("depth ") {
                                last = m;
                            }
                        });
                        let toks: Vec<&str> = last.split_whitespace().collect();
                        let after = |key: &str, n: usize| -> String { toks.iter().position(|w| *w == key).and_then(|p| toks.get(p + n)).map(|s| s.to_string()).unwrap_or_default() };
                        let kind = after("score", 1);
                        let score = after("score", 2);
                        let pv: Vec<&str> = toks.iter().skip_while(|w| **w != "pv").skip(1).take(6).copied().collect();
                        let (tt, c) = s.get_present();
                        let line = format!(
                            "{:016x}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{}",
                            k,
                            tt,
                            if c { "b" } else { "w" },
                            after("depth", 1),
                            kind,
                            score,
                            best.map(|a| kairos::web::tree::action_lan(s, &a)).unwrap_or_default(),
                            after("nodes", 1),
                            t.elapsed().as_secs_f64(),
                            pv.join(" ")
                        );
                        let mut f = file.lock().unwrap();
                        writeln!(f, "{}", line).unwrap();
                        f.flush().ok();
                        if (i + 1) % 50 == 0 {
                            eprintln!("{}/{} {:.0}s", i + 1, total, t0.elapsed().as_secs_f64());
                        }
                    }
                })
                .expect("spawn"),
        );
    }
    for h in handles {
        h.join().expect("worker");
    }
    writeln!(out, "done in {:.0}s", t0.elapsed().as_secs_f64()).unwrap();
}

/// `mate1recall <games.jsonl|positions.g>... [-n max] [-t threads]`: how often a depth-1 search
/// finds the mate in one before each checkmate game's final action, and how long it takes.
/// Env: MATE_VERIFY (engine `MateVerify`), M1_DEPTH, M1_CLOCK (ms), M1_STRIDE.
fn mate1recall(args: &[String], out: &mut impl Write) {
    use kairos::core::variants::{apply_uci_moves, position_from_uci};
    use kairos::engine::ab::AlphaBeta;
    use kairos::engine::{Limits, OptionValue};
    let mut files = Vec::new();
    let (mut max, mut threads) = (usize::MAX, 8usize);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-n" => max = it.next().and_then(|v| v.parse().ok()).unwrap_or(max),
            "-t" => threads = it.next().and_then(|v| v.parse().ok()).unwrap_or(threads),
            f => files.push(f.to_string()),
        }
    }
    let stride: usize = std::env::var("M1_STRIDE").ok().and_then(|v| v.parse().ok()).unwrap_or(1).max(1);
    let k0: usize = std::env::var("M1_K").ok().and_then(|v| v.parse().ok()).unwrap_or(1).max(1);
    let depth: u32 = std::env::var("M1_DEPTH").ok().and_then(|v| v.parse().ok()).unwrap_or(if k0 == 1 { 1 } else { 2 * k0 as u32 });
    let verify = std::env::var("MATE_VERIFY").ok();
    let clock: Option<u64> = std::env::var("M1_CLOCK").ok().and_then(|v| v.parse().ok());
    let k: usize = std::env::var("M1_K").ok().and_then(|v| v.parse().ok()).unwrap_or(1).max(1);
    let mut positions: Vec<State> = Vec::new();
    let mut n = 0usize;
    'files: for f in &files {
        let text = std::fs::read_to_string(f).expect("games file");
        // A `.g` file lists positions (G lines) that have a mate in one.
        if f.ends_with(".g") {
            for line in text.lines().filter(|l| l.starts_with("G ")) {
                if positions.len() < max {
                    positions.push(parse_g_line(line).expect("G line"));
                }
            }
            continue;
        }
        for line in text.lines() {
            if positions.len() >= max {
                break 'files;
            }
            if json_field(line, "reason") != Some("checkmate") {
                continue;
            }
            n += 1;
            if !n.is_multiple_of(stride) {
                continue;
            }
            let (Some(pos), Some(moves)) = (json_field(line, "position"), json_field(line, "moves")) else { continue };
            let acts: Vec<&str> = moves.split(" submit").map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
            if acts.len() < 3 {
                continue;
            }
            let Ok(mut s) = position_from_uci(pos) else { continue };
            let mut ok = true;
            // The position `k` own actions before the final mate (the winner to move).
            let keep = 2 * k - 1;
            if acts.len() < keep + 1 {
                continue;
            }
            for a in &acts[..acts.len() - keep] {
                if apply_uci_moves(&mut s, &format!("{} submit", a)).is_err() {
                    ok = false;
                    break;
                }
            }
            // For k > 1 only positions with a forced mate in k count (the exact solver decides).
            if ok && !is_mated(&s) && (k == 1 || exact_mate(&s, k as u32).is_some()) {
                positions.push(s);
            }
        }
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::<(usize, bool, f64, String)>::new());
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| {
                let stop = std::sync::atomic::AtomicBool::new(false);
                let mut e = AlphaBeta::new();
                e.set_option("Hash", &OptionValue::Int(16));
                e.set_option("Threads", &OptionValue::Int(1));
                if let Some(v) = &verify {
                    e.set_option("MateVerify", &OptionValue::Str(v.clone()));
                }
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(p) = positions.get(i) else { break };
                    e.new_game();
                    let t = Instant::now();
                    let mut last = String::new();
                    let limits = match clock {
                        Some(ms) => Limits { depth: Some(depth), time_left: Some(ms), increment: Some(100), ..Default::default() },
                        None => Limits { depth: Some(depth), ..Default::default() },
                    };
                    let a = e.find_best_move(p, &limits, &stop, &mut |m| last = m);
                    let secs = t.elapsed().as_secs_f64();
                    let mated = if k == 1 {
                        a.and_then(|a| p.can_apply_action(&a)).is_some_and(|q| is_mated(&q))
                    } else {
                        // the last reported score is a mate in at most k
                        last.split_whitespace().skip_while(|w| *w != "score").nth(1) == Some("mate")
                            && last.split_whitespace().skip_while(|w| *w != "score").nth(2).and_then(|n| n.parse::<i32>().ok()).is_some_and(|n| n > 0 && n as usize <= k)
                    };
                    results.lock().unwrap().push((i, mated, secs, last));
                }
            });
        }
    });
    let mut r = results.into_inner().unwrap();
    r.sort_by_key(|x| x.0);
    let missed: Vec<&(usize, bool, f64, String)> = r.iter().filter(|x| !x.1).collect();
    let mut times: Vec<f64> = r.iter().map(|x| x.2).collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f64| times.get(((times.len() as f64 - 1.0) * p) as usize).copied().unwrap_or(0.0);
    writeln!(out, "positions {} | mate played {} ({:.2}%) | missed {} | time p50 {:.3}s p99 {:.3}s max {:.3}s total {:.1}s", r.len(), r.len() - missed.len(), 100.0 * (r.len() - missed.len()) as f64 / r.len().max(1) as f64, missed.len(), q(0.5), q(0.99), q(1.0), times.iter().sum::<f64>()).unwrap();
    for m in missed.iter().take(20) {
        writeln!(out, "missed #{} ({:.3}s): {}", m.0, m.2, m.3.chars().take(120).collect::<String>()).unwrap();
    }
}

/// `matescan <file.g>... [-n max] [-t threads]`: runs the root mate-in-one scan (depth 1) on each
/// position and reports whether it played a mate, what the scan reported, and the time.
/// Env as for `mate1recall`, plus M1_PLY (for jsonl games) and M1_SAVE=<path> for mated lines.
fn matescan(args: &[String], out: &mut impl Write) {
    use kairos::core::variants::{apply_uci_moves, position_from_uci};
    use kairos::engine::ab::AlphaBeta;
    use kairos::engine::{Limits, OptionValue};
    let mut files = Vec::new();
    let (mut max, mut threads) = (usize::MAX, 8usize);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-n" => max = it.next().and_then(|v| v.parse().ok()).unwrap_or(max),
            "-t" => threads = it.next().and_then(|v| v.parse().ok()).unwrap_or(threads),
            f => files.push(f.to_string()),
        }
    }
    let verify = std::env::var("MATE_VERIFY").ok();
    let clock: Option<u64> = std::env::var("M1_CLOCK").ok().and_then(|v| v.parse().ok());
    if std::env::var_os("MREGION_VERIFY").is_some() {
        kairos::engine::ab::REGION_VERIFY.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let back: usize = std::env::var("M1_PLY").ok().and_then(|v| v.parse().ok()).unwrap_or(1).max(1);
    let stride: usize = std::env::var("M1_STRIDE").ok().and_then(|v| v.parse().ok()).unwrap_or(1).max(1);
    let save = std::env::var("M1_SAVE").ok();
    let mut positions: Vec<State> = Vec::new();
    let mut sources: Vec<String> = Vec::new();
    let mut n = 0usize;
    'files: for f in &files {
        let text = std::fs::read_to_string(f).expect("positions file");
        if f.ends_with(".g") {
            for line in text.lines().filter(|l| l.starts_with("G ")) {
                if positions.len() >= max {
                    break 'files;
                }
                positions.push(parse_g_line(line).expect("G line"));
                sources.push(line.to_string());
            }
            continue;
        }
        for line in text.lines() {
            if positions.len() >= max {
                break 'files;
            }
            n += 1;
            if !n.is_multiple_of(stride) {
                continue;
            }
            let (Some(pos), Some(moves)) = (json_field(line, "position"), json_field(line, "moves")) else { continue };
            let acts: Vec<&str> = moves.split(" submit").map(|x| x.trim()).filter(|x| !x.is_empty()).collect();
            if acts.len() < back + 2 {
                continue;
            }
            let Ok(mut s) = position_from_uci(pos) else { continue };
            if acts[..acts.len() - back].iter().all(|a| apply_uci_moves(&mut s, &format!("{} submit", a)).is_ok()) && !is_mated(&s) {
                positions.push(s);
                sources.push(line.to_string());
            }
        }
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    // (index, mate played, scan line, seconds)
    let results = std::sync::Mutex::new(Vec::<(usize, bool, String, f64)>::new());
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| {
                let stop = std::sync::atomic::AtomicBool::new(false);
                let mut e = AlphaBeta::new();
                e.set_option("Hash", &OptionValue::Int(16));
                e.set_option("Threads", &OptionValue::Int(1));
                if let Some(v) = &verify {
                    e.set_option("MateVerify", &OptionValue::Str(v.clone()));
                }
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(p) = positions.get(i) else { break };
                    e.new_game();
                    let mut scan = String::new();
                    let limits = match clock {
                        Some(ms) => Limits { depth: Some(1), time_left: Some(ms), increment: Some(100), ..Default::default() },
                        None => Limits { depth: Some(1), ..Default::default() },
                    };
                    let t = Instant::now();
                    let a = e.find_best_move(p, &limits, &stop, &mut |m| {
                        if let Some(k) = m.find("mate1 ") {
                            scan = m[k + 6..].to_string();
                        }
                    });
                    let secs = t.elapsed().as_secs_f64();
                    let mated = a.and_then(|a| p.can_apply_action(&a)).is_some_and(|q| is_mated(&q));
                    results.lock().unwrap().push((i, mated, scan, secs));
                }
            });
        }
    });
    let mut r = results.into_inner().unwrap();
    r.sort_by_key(|x| x.0);
    for x in &r {
        writeln!(out, "#{} {} {:.4}s {}", x.0, if x.1 { "MATE" } else { "-" }, x.3, x.2).unwrap();
    }
    if let Some(path) = &save {
        let lines: Vec<&str> = r.iter().filter(|x| x.1).map(|x| sources[x.0].as_str()).collect();
        std::fs::write(path, lines.join("\n") + "\n").expect("M1_SAVE file");
    }
    let mates = r.iter().filter(|x| x.1).count();
    let none = r.iter().filter(|x| x.2.starts_with("none")).count();
    let partial = r.iter().filter(|x| x.2.starts_with("partial")).count();
    let unresolved: usize = r.iter().filter_map(|x| x.2.split(", ").find(|w| w.ends_with("unresolved")).and_then(|w| w.split(' ').next()?.parse::<usize>().ok())).sum();
    let mut times: Vec<f64> = r.iter().map(|x| x.3).collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f64| times.get(((times.len() as f64 - 1.0) * p) as usize).copied().unwrap_or(0.0);
    write!(out, "{}", kairos::misc::prof::report()).unwrap();
    if kairos::engine::ab::REGION_VERIFY.load(std::sync::atomic::Ordering::Relaxed) {
        let st = |i: usize| kairos::engine::ab::REGION_STATS[i].load(std::sync::atomic::Ordering::Relaxed);
        writeln!(out, "regions {} | actions checked {} | broken {}", st(0), st(1), st(2)).unwrap();
    }
    writeln!(out, "positions {} | mate {} | none {} | partial {} (unresolved checks {}) | time p50 {:.4}s p90 {:.4}s p99 {:.4}s max {:.4}s total {:.2}s", r.len(), mates, none, partial, unresolved, q(0.5), q(0.9), q(0.99), q(1.0), times.iter().sum::<f64>()).unwrap();
}
