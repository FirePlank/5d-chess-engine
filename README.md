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

A depth's lines share its `nodes`, `nps`, `time` and `roots`; `score` reads as in a plain search (`cp X wdl W D L` or `mate N wdl ...`, side to move's view). With K = 1 the search and its output are the plain ones: no `multipv` tag. With `Threads` above 1, a final group of K lines follows when a helper's result wins the vote (its depth may be one more than the last group's); `bestmove` is the last group's line 1 action, unless a later, interrupted depth changed it (as in a plain search). The JSON API's `search` takes `"multiPV": K` (else the `set_option` value, else 1); its streamed info lines are the same text, and 5D Chess Lab's `parseInfo` (static/js/kairos.js) returns the tag as `multipv` (absent on a plain search). The Analysis page keeps line 1 as the shown entry and all lines of that depth in its `lines` array (index k - 1).

## Testing

Strength: `setoption name Skill Level value N` selects levels 1–8 (default 8).
The JSON `search` field is `"skillLevel": N`; omitted searches default to full strength,
unless a persistent `Skill Level` option is set. Level 8 runs the unchanged full search.
Levels 1–7 separate board-local chess competence from multiverse understanding. Material,
development, pawn structure and local king safety retain their evaluation weights, while
the remaining evaluation (including the learned residual) is blended in at 8–100% as levels
rise. Search depth/node budgets increase by level; moves across time or timelines receive
an additional depth reduction at levels 1–6, while ordinary moves do not. Four to eight candidate
lines leave more of the budget for replies. A local capture search accounts for recaptures
and material already won before filtering obvious giveaways, allowing winning mates and
substantial evaluated compensation (trusted more at higher levels). Bounded score sampling adds
small inaccuracies without treating a large advantage as permission to throw away pieces.
This models stronger ordinary-chess play than 5D planning, not calibrated ratings such as
"1200 standard / 600 5D". `OwnBook` remains optional at every level; interfaces should disable
pondering for limited levels. The ladder targets increasing strength, but equal Elo gaps
and comparisons with the previous policy require paired matches at a fixed time control.

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

## 5D Chess Lab

Kairos runs in the browser at **[5D Chess Lab](https://fireplank.github.io/5d-chess-lab/)**: an analysis board, a board editor and play against the engine. The site lives in its own repository, [FirePlank/5d-chess-lab](https://github.com/FirePlank/5d-chess-lab), and builds this crate (compiled to WebAssembly) as a git dependency; it picks up every engine change on `main` within the hour (at once when this repository's `SITE_DISPATCH_TOKEN` secret is set, see `.github/workflows/notify-site.yml`), and moves to it only once its checks pass.

The `kairos::web` module is the part of the engine it uses: 5dpgn and 5DFEN, the game records, the analysis game tree and the JSON command interface (`web::api`). Changes there show up on the site.

## License

This project is licensed under the GNU General Public License v3.0 - see [LICENSE](LICENSE) for details.
