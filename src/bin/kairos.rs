//! kairos: a 5D chess engine speaking 5DUCI on stdin/stdout.

fn main() {
    if std::env::args().len() > 1 {
        eprintln!("Usage: kairos   (then talk 5DUCI on stdin)");
        std::process::exit(2);
    }
    kairos::engine::uci::run(kairos::engine::ab::AlphaBeta::new());
}
