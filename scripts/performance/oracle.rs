//! Link this same probe against each frozen release library, then compare stdout byte for
//! byte. Exercises cold/warm searches, MultiPV, skill, and model changes without clock limits.
use kairos::core::perft::parse_g_line;
use kairos::engine::{ab::AlphaBeta, Limits, OptionValue};
use std::sync::atomic::AtomicBool;

fn report(line: &str) -> String {
    if line.starts_with("depth ") {
        let mut words = line.split_whitespace();
        let mut kept = Vec::new();
        while let Some(word) = words.next() {
            match word {
                "time" | "nps" => { words.next(); }
                "pv" => { kept.push(word); kept.extend(words); break; }
                _ => kept.push(word),
            }
        }
        return kept.join(" ");
    }
    if line.starts_with("string mate1 partial (")
        && let Some((status, elapsed)) = line.rsplit_once(", ")
        && elapsed.strip_suffix(" ms)").is_some_and(|n| n.parse::<u128>().is_ok())
    {
        return format!("{})", status);
    }
    line.to_string()
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(args.len() >= 7, "oracle CORPUS DEPTH NODE_CAP MULTIPV SKILL MODEL [MODEL...]; cap 0 = uncapped");
    let text = std::fs::read_to_string(&args[1]).unwrap();
    let depth = args[2].parse().unwrap();
    let nodes: u64 = args[3].parse().unwrap();
    let skill: i64 = args[5].parse().unwrap();
    let limits = Limits { depth: Some(depth), nodes: (nodes != 0).then_some(nodes), ..Default::default() };
    let stop = AtomicBool::new(false);
    for (pos, line) in text.lines().filter(|l| l.starts_with("G ")).enumerate() {
        let state = parse_g_line(line).unwrap();
        let mut e = AlphaBeta::new();
        for (key, value) in [("MateVerify", OptionValue::Str("counted".into())),
                             ("MultiPV", OptionValue::Int(args[4].parse().unwrap())),
                             ("Skill Level", OptionValue::Int(skill))] {
            assert!(e.set_option(key, &value));
        }
        if let Ok(blend) = std::env::var("NNUE_BLEND") {
            assert!(e.set_option("EvalBlend", &OptionValue::Int(blend.parse().unwrap())));
        }
        for (model, path) in args[6..].iter().enumerate() {
            assert!(e.set_option("EvalFile", &OptionValue::Str(path.clone())));
            for warm in 0..2 {
                println!("pos {pos} model {model} warm {warm}");
                let best = e.find_best_move(&state, &limits, &stop, &mut |m| println!("{}", report(&m)));
                if skill == 8 {
                    println!("nodes {} score {} proven {} best {}", e.last_search_nodes(), e.last_score(),
                             e.last_proven(), best.map_or_else(|| "none".into(), |m| m.to_string()));
                } else {
                    // Skill uses an unseeded random final pick; its searched candidate scores,
                    // PVs and nodes above remain deterministic. Do not compare the random pick.
                    println!("nodes {} proven {} randomized selection", e.last_search_nodes(), e.last_proven());
                }
            }
        }
    }
}
