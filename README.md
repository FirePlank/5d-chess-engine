# Kairos

A Rust engine for [5D Chess With Multiverse Time Travel](https://www.5dchesswithmultiversetimetravel.com/).

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL%20v3-blue.svg)](LICENSE)

## Features

- **Rules**: Full 5D rules on boards up to 8x8: time travel, branching timelines, the present, odd/even timeline layouts, all fairy pieces (princess, unicorn, dragon, brawn, royal queen, common king), castling, en passant and promotion.
- **Legal actions**: A hypercuboid search enumerates legal turns lazily, so checkmate detection and move generation stay tractable with many timelines.
- **Search**: Iterative deepening PVS over whole turns with a transposition table, killer and history ordering, late move pruning and reductions and a tactical quiescence search. Turns are enumerated best-first by the summed scores of their per-board moves, with checking moves (including checks across time and timelines) ordered first.
- **Evaluation**: Linear evaluation over every timeline's latest board (material, timeline advantage, king safety including pieces near a royal from neighbouring timelines, mobility, pawn structure) with weights fitted to game results.
- **Protocol**: Speaks 5DUCI, so it plugs into any 5DUCI interface or match runner.
- **Opening book**: Turn Zero theory from the community guides (`docs/theory/`), the T0 Openings Guide's recommended structures and trap refutations (Turn Zero only; other variants are never in the book), in `data/book_theory.txt` (built by `ktools bookgen` from the annotated 5dpgn lines of `data/book_theory_src.txt`). With the 5DUCI option `OwnBook` (off by default) a book position is answered at once with a weighted random theory move.

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

Clocks: `go wtime <ms> btime <ms> [winc <ms> binc <ms>]`, with an optional delay (the clock waits before running down; unused delay is lost): `wdelay/bdelay <ms>` per action and `wtldelay/btldelay <ms>` per active timeline of the position. The 5D tournament clock (5 min + 3 s per active timeline) is `go wtime 300000 btime 300000 wtldelay 3000 btldelay 3000`. The JSON API's `search` takes the same as `timeLeft`, `increment`, `delay` and `delayPerTimeline` (ms).

MultiPV: `setoption name MultiPV value K` (default 1, 1 to 256) reports the best K root actions, each with an exact score and its own line (fewer when the position has fewer root actions). After each completed depth there is one info line per line, best first, with `multipv k` (1-based) right after the depth:

```text
info depth 4 multipv 1 score cp 34 wdl 592 0 408 nodes 1374 nps 171729 time 8 roots 20 pv (0T1)g1f3 submit (0T1)g8f6 submit ...
info depth 4 multipv 2 score cp 26 wdl 571 0 429 nodes 1374 nps 171729 time 8 roots 20 pv (0T1)b1c3 submit (0T1)b8c6 submit ...
info depth 4 multipv 3 score cp 6 wdl 517 0 483 nodes 1374 nps 171729 time 8 roots 20 pv (0T1)e2e4 submit (0T1)g8f6 submit ...
```

A depth's lines share its `nodes`, `nps`, `time` and `roots`; `score` reads as in a plain search (`cp X wdl W D L` or `mate N wdl ...`, side to move's view). With K = 1 the search and its output are the plain ones: no `multipv` tag. With `Threads` above 1, a final group of K lines follows when a helper's result wins the vote (its depth may be one more than the last group's); `bestmove` is the last group's line 1 action, unless a later, interrupted depth changed it (as in a plain search). The JSON API's `search` takes `"multiPV": K` (else the `set_option` value, else 1); its streamed info lines are the same text, and the browser's `parseInfo` (web/static/js/kairos.js) returns the tag as `multipv` (absent on a plain search). The Analysis page keeps line 1 as the shown entry and all lines of that depth in its `lines` array (index k - 1).

## Testing

```bash
# Unit tests and perft
cargo test --release

# Perft of positions in a file (one "G ..." line per position)
./target/release/ktools perft 3 < data/perft.g

# Node oracles (must not change for pure speed work)
./target/release/ktools bench
AB_NODES=20000 ./target/release/ktools abbench

# Tactics regression suite (data/tactics.g, from data/tactics_src.txt): solved/unsolved and
# time to solve per position, here 2 s per position on 1 thread
./target/release/ktools tactics data/tactics.g 2000 1
```

Strength is measured with the SPRT harness in `src/bin/sprt.rs` (pentanomial GSPRT, colour-swapped pairs from openings generated per pair (a random 8x8 variant plus 6 noisy shallow-search actions), 10+0.1 clock by default):

```bash
./target/release/sprt --new target/release/kairos --base baseline/kairos \
  --movetime 100 --concurrency 12 --elo0 0 --elo1 5
```

Larger tests run sharded on GitHub Actions with `scripts/sprt_run.sh`, which pushes a `test:` commit to the `sprt` branch and stops the run once the LLR crosses a bound.

## Website: viewer, analysis board and play

```bash
rustup target add wasm32-wasip1-threads   # once
cargo wasm                                # the browser engine (again after engine changes)
cargo run --release --bin kairos-web
```

Then open <http://127.0.0.1:8080/>. `cargo wasm` (an alias in `.cargo/config.toml`) builds `web/wasm` into `web/wasm/target/wasm32-wasip1-threads/release/kairos_wasm.wasm`, which the server serves as `/kairos.wasm`; the server warns at start if it is missing or older than the sources.

- **Analysis**: paste or drop a game (SPRT `--games-out` JSON lines, 5dpgn, a 5DUCI `position … moves …` line or a board FEN; Ctrl+V loads from the clipboard) and step through it: every timeline and turn on one zoomable board, arrows for the last action and for checks, the present, book actions marked, the recorded evaluations as a graph, and the opening book's theory moves for the position (click one to play it). Kairos analyses the position in the browser: live depth, score and principal variation, click a PV action to play it. Moves can be made on the board (select on every board you want to play, then submit), building variations.
- **Play**: play Kairos on any built-in variant, as either side, with a clock or a fixed think time; with "Opening book" on (the default) Kairos plays theory moves from its opening book at once. The server runs the engine; one person plays at a time, everyone else watches the live game and can queue for the next one. Finished games are appended to `games/web/played.jsonl`.
- **Games**: browse the record files under `games/` (JSONL and 5dpgn) with filters for variant, result, ending and length, and open any game in the viewer.

There is one browser build (`wasm32-wasip1-threads`, like Apeiron's single build): it runs on shared memory, and when the page is cross-origin isolated (kairos-web sends COOP/COEP) the analysis engine also runs its Lazy SMP helper threads in Web Workers; otherwise it searches on one thread. Threads and hash size are in the engine settings (gear) on the Analysis page.

Options: `--port 8080`, `--host 0.0.0.0` (listen on every interface), `--games <dir>` (repeatable; default `games/`), `--threads <n>` and `--hash <MB>` (the play engine), `--archive <file>`.

The site is plain HTML/CSS/ES modules in `web/static`, with no build step. It also runs with no server at all: the published site, <https://fireplank.github.io/5d-chess-engine/>, is rebuilt by `.github/workflows/pages.yml` on every push to main that touches the engine or the site. `scripts/build_pages.sh [dir]` (after `cargo wasm`) assembles it: `web/static`, the engine as `kairos.wasm`, and the game records in `web/games` (a small curated set) with an index. Without a server, Play runs the game on the in-browser engine (each visitor plays their own Kairos, using the analysis engine settings; no queue), the game browser lists the bundled files and any JSONL/5dpgn file opened from the computer, and `coi-serviceworker.js` adds the COOP/COEP headers static hosts cannot send, so the engine still gets its threads (one thread where service workers are unavailable).

Tests: `cargo test` covers the record loader and 5dpgn; after `cargo wasm`, `node tests/web/records.mjs games/sprt/*.jsonl` replays SPRT records through the browser build and cross-checks actions, book/eval tags and results, and `node tests/web/pgn_corpus.mjs <dir>` loads every 5dpgn file under a directory and views each main-line position.

## License

This project is licensed under the GNU General Public License v3.0 - see [LICENSE](LICENSE) for details.
