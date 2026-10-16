# Kairos

A Rust engine for [5D Chess With Multiverse Time Travel](https://www.5dchesswithmultiversetimetravel.com/).

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL%20v3-blue.svg)](LICENSE)

## Features

- **Rules**: Full 5D rules on boards up to 8x8: time travel, branching timelines, the present, odd/even timeline layouts, all fairy pieces (princess, unicorn, dragon, brawn, royal queen, common king), castling, en passant and promotion.
- **Legal actions**: A hypercuboid search enumerates legal turns lazily, so checkmate detection and move generation stay tractable with many timelines.
- **Search**: Iterative deepening PVS over whole turns with a transposition table, killer and history ordering, late move pruning and reductions and a tactical quiescence search. Turns are enumerated best-first by the summed scores of their per-board moves, with checking moves (including checks across time and timelines) ordered first.
- **Evaluation**: Linear evaluation over every timeline's latest board (material, timeline advantage, king safety including pieces near a royal from neighbouring timelines, mobility, pawn structure) with weights fitted to game results.
- **Protocol**: Speaks 5DUCI, so it plugs into any 5DUCI interface or match runner.

## Quick Start

```bash
cargo build --release
./target/release/kairos
```

Then talk 5DUCI on stdin:

```text
5duci
isready
position startpos
go movetime 1000
```

The engine answers with `bestmove <moves>`; the GUI applies them and sends `submit`.

## Testing

```bash
# Unit tests and perft
cargo test --release

# Perft of positions in a file (one "G ..." line per position)
./target/release/ktools perft 3 < data/perft.g

# Node oracles (must not change for pure speed work)
./target/release/ktools bench
AB_NODES=20000 ./target/release/ktools abbench
```

Strength is measured with the SPRT harness in `src/bin/sprt.rs` (pentanomial GSPRT, colour-swapped pairs from openings generated per pair (a random 8x8 variant plus 6 noisy shallow-search actions), 10+0.1 clock by default):

```bash
./target/release/sprt --new target/release/kairos --base baseline/kairos \
  --movetime 100 --concurrency 12 --elo0 0 --elo1 5
```

Larger tests run sharded on GitHub Actions with `scripts/sprt_run.sh`, which pushes a `test:` commit to the `sprt` branch and stops the run once the LLR crosses a bound.

## Website: viewer, analysis board and play

```bash
cargo run --release --bin kairos-web
```

Then open <http://127.0.0.1:8080/>. On its first start the server builds the browser engine (`web/wasm` for `wasm32-wasip1`, into `web/static/kairos.wasm`; it rebuilds when the crate sources change). Install the target once with `rustup target add wasm32-wasip1`.

- **Analysis**: paste or drop a game (SPRT `--games-out` JSON lines, 5dpgn, or a 5DUCI `position … moves …` line) and step through it: every timeline and turn on one zoomable board, arrows for the last action and for checks, the present, book actions marked, the recorded evaluations as a graph. Kairos analyses the position in the browser (a WASM build in a Web Worker): live depth, score and principal variation, click a PV action to play it. Moves can be made on the board (select on every board you want to play, then submit), building variations.
- **Play**: play Kairos on any built-in variant, as either side, with a clock or a fixed think time. The server runs the engine; one person plays at a time, everyone else watches the live game and can queue for the next one. Finished games are appended to `games/web/played.jsonl`.
- **Games**: browse the record files under `games/` (JSONL and 5dpgn) with filters for variant, result, ending and length, and open any game in the viewer.

Options: `--port 8080`, `--host 0.0.0.0` (listen on every interface), `--games <dir>` (repeatable; default `games/`), `--hash <MB>` (engine hash for play), `--archive <file>`, `--no-wasm-build`.

The site is plain HTML/CSS/ES modules in `web/static`, with no build step. Without the server (any static file host that sends `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`), the viewer and analysis work; play and the game browser need `kairos-web`.

Tests: `cargo test` covers the record loader and 5dpgn; `node tests/web/records.mjs games/sprt/*.jsonl` replays SPRT records through the WASM build and cross-checks actions, book/eval tags and results.

## License

This project is licensed under the GNU General Public License v3.0 - see [LICENSE](LICENSE) for details.
