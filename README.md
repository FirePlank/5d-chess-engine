# Kairos

A Rust engine for [5D Chess With Multiverse Time Travel](https://www.5dchesswithmultiversetimetravel.com/).

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL%20v3-blue.svg)](LICENSE)

## Features

- **Rules**: Full 5D rules on boards up to 8x8: time travel, branching timelines, the present, odd/even timeline layouts, all fairy pieces (princess, unicorn, dragon, brawn, royal queen, common king), castling, en passant and promotion.
- **Legal actions**: A hypercuboid search enumerates legal turns lazily, so checkmate detection and move generation stay tractable with many timelines.
- **Search**: Iterative deepening PVS over whole turns with a transposition table, killer and history ordering, late move pruning and reductions and a tactical quiescence search. Turns are enumerated best-first by the summed scores of their per-board moves, with checking moves (including checks across time and timelines) ordered first.
- **Evaluation**: Handcrafted evaluation over every timeline's latest board, with timeline advantage weighted by the material still able to use it.
- **Protocol**: Speaks 5DUCI, so it plugs into any 5DUCI interface or match runner.

## Quick Start

```bash
cargo build --release
./target/release/kairos ab
```

Then talk 5DUCI on stdin:

```text
5duci
isready
position startpos
go movetime 1000
```

The engine answers with `bestmove <moves>`; the GUI applies them and sends `submit`.

Other engines are built into the same binary (`kairos mcts`, `kairos linear`, `kairos flat-ucb`, ...) for comparison; run `kairos --help` for the list.

## Testing

```bash
# Unit tests and perft
cargo test --release

# Perft of positions in a file (one "G ..." line per position)
./target/release/ktools perft 3 < data/perft.g
```

Strength is measured with the SPRT harness in `src/bin/sprt.rs` (pentanomial GSPRT, colour-swapped opening pairs from `data/book.g`):

```bash
./target/release/sprt --new "target/release/kairos ab" --base "baseline/kairos ab" \
  --movetime 100 --concurrency 12 --elo0 0 --elo1 5
```

Larger tests run sharded on GitHub Actions with `scripts/sprt_run.sh`, which pushes a `test:` commit to the `sprt` branch and stops the run once the LLR crosses a bound.

## License

This project is licensed under the GNU General Public License v3.0 - see [LICENSE](LICENSE) for details.
