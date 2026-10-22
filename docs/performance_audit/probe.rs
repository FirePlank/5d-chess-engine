use kairos::core::perft::parse_g_line;
use kairos::engine::{ab::AlphaBeta, Limits, OptionValue};
use std::{hash::{Hash, Hasher}, sync::atomic::AtomicBool, time::Instant};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let input = std::fs::read_to_string(&args[1]).unwrap();
    let cap: u64 = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(20000);
    let depth: u32 = args.get(3).map(|s| s.parse().unwrap()).unwrap_or(20);
    let positions: Vec<_> = input.lines().filter(|s| s.starts_with("G ")).map(|l| parse_g_line(l).unwrap()).collect();
    if args.get(4).is_some_and(|s| s == "inventory") {
        for (i, s) in positions.iter().enumerate() {
            let (lo, hi) = s.get_lines_range();
            let boards: usize = (lo..=hi).map(|l| s.multiverse().timeline(l).boards.len()).sum();
            println!("pos {i} lines {} active {} boards {boards}", hi-lo+1, s.active_timelines());
        }
        return;
    }
    let stop = AtomicBool::new(false);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let wall = Instant::now();
    let mut total = 0;
    for (i, s) in positions.iter().enumerate() {
        let mut e = AlphaBeta::new();
        if let Some(v) = args.get(4) { e.set_option("MateVerify", &OptionValue::Str(v.clone())); }
        let mut last = String::new();
        let t = Instant::now();
        let best = e.find_best_move(s, &Limits {depth: Some(depth), nodes: Some(cap), ..Default::default()}, &stop, &mut |m| {
            if m.starts_with("depth ") {
                let w: Vec<_> = m.split_whitespace().collect();
                let mut clean = Vec::new();
                let mut j = 0;
                while j < w.len() {
                    if w[j] == "nps" || w[j] == "time" { j += 2; } else {clean.push(w[j]); j += 1;}
                }
                let line = clean.join(" ");
                line.hash(&mut h);
                last = line;
            }
        });
        let nodes = e.last_search_nodes();
        let best = best.map(|a| a.to_string());
        best.hash(&mut h); nodes.hash(&mut h); e.last_score().hash(&mut h); e.last_proven().hash(&mut h);
        total += nodes;
        let (lo, hi) = s.get_lines_range();
        println!("pos {i} lines {} active {} actual {nodes} score {} proven {} seconds {:.6} last [{last}] best {:?}", hi-lo+1, s.active_timelines(), e.last_score(), e.last_proven(), t.elapsed().as_secs_f64(), best);
    }
    let elapsed = wall.elapsed().as_secs_f64();
    println!("trace {:016x} actual_nodes {total} seconds {elapsed:.6} nps {:.0}", h.finish(), total as f64/elapsed);
    print!("{}", kairos::misc::prof::report());
}
