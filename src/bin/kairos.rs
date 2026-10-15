//! kairos 5DUCI engine executable. Usage: kairos <engine> [options]

use kairos::engine::features::{default_weights, trained_weights, MoveWeights};
use kairos::engine::mcts::*;
use kairos::engine::{uci, Engine};

fn usage() -> ! {
    eprintln!(
        "Usage: kairos <engine> [options]\n\
         Engines: mcts mcts-weighted zero zero-capture zero-capture-check-pw linear linear-trained\n\
         \x20        linear-weighted flat-ucb flat-ucb-weighted monkey\n\
         Options: -s/--seed <u32>  -r/--rollout-max-actions <n>  -wt/--weight-temperature <t>\n\
         \x20        --pw-constant <c>  --pw-alpha <a>"
    );
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 || args[1] == "-h" || args[1] == "--help" {
        usage();
    }
    let name = args[1].as_str();
    let mut seed: Option<u32> = None;
    let mut rmax = DEFAULT_ROLLOUT_MAX_ACTIONS;
    let mut temp = DEFAULT_TEMPERATURE;
    let (mut pwc, mut pwa) = (2.0f64, 0.5f64);
    let mut i = 2;
    while i < args.len() {
        let v = args.get(i + 1).cloned().unwrap_or_else(|| usage());
        match args[i].as_str() {
            "-s" | "--seed" => seed = Some(v.parse().unwrap_or_else(|_| usage())),
            "-r" | "--rollout-max-actions" => rmax = v.parse().unwrap_or_else(|_| usage()),
            "-wt" | "--weight-temperature" => temp = v.parse().unwrap_or_else(|_| usage()),
            "--pw-constant" => pwc = v.parse().unwrap_or_else(|_| usage()),
            "--pw-alpha" => pwa = v.parse().unwrap_or_else(|_| usage()),
            _ => usage(),
        }
        i += 2;
    }
    let roll = |sel: Selection, cutoff: Cutoff, w| {
        Rollout::new(DefaultKind::Rollout { selection: sel, cutoff }, rmax, temp, w, seed)
    };
    let zero = || Rollout::new(DefaultKind::ZeroPosition, 0, temp, default_weights(), seed);
    let engine: Box<dyn Engine> = match name {
        "mcts" => Box::new(Mcts::new(name, TreeKind::Uct, OrderKind::Random, roll(Selection::Random, Cutoff::Zero, default_weights()), seed)),
        "mcts-weighted" => Box::new(Mcts::new(name, TreeKind::Uct, OrderKind::Random, roll(Selection::Weighted, Cutoff::Zero, default_weights()), seed)),
        "zero" => Box::new(Mcts::new(name, TreeKind::Uct, OrderKind::Random, zero(), seed)),
        "zero-capture" => Box::new(Mcts::new(name, TreeKind::Uct, OrderKind::Scored(MoveWeights::capture()), zero(), seed)),
        "zero-capture-check-pw" => Box::new(Mcts::new(
            name,
            TreeKind::Pw { constant: pwc, alpha: pwa },
            OrderKind::Scored(MoveWeights::capture_check()),
            zero(),
            seed,
        )),
        "linear" => Box::new(Mcts::new(name, TreeKind::Uct, OrderKind::Random, roll(Selection::Random, Cutoff::Linear("default"), default_weights()), seed)),
        "linear-trained" => Box::new(Mcts::new(name, TreeKind::Uct, OrderKind::Random, roll(Selection::Random, Cutoff::Linear("trained"), trained_weights()), seed)),
        "linear-weighted" => Box::new(Mcts::new(name, TreeKind::Uct, OrderKind::Random, roll(Selection::Weighted, Cutoff::Linear("default"), default_weights()), seed)),
        "flat-ucb" => Box::new(FlatUcb::new(name, roll(Selection::Random, Cutoff::Zero, default_weights()))),
        "flat-ucb-weighted" => Box::new(FlatUcb::new(name, roll(Selection::Weighted, Cutoff::Zero, default_weights()))),
        "monkey" => Box::new(Monkey::new(seed)),
        "ab" => Box::new(kairos::engine::ab::AlphaBeta::new(Default::default(), Default::default())),
        _ => usage(),
    };
    uci::run(engine);
}
