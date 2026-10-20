# Mate-in-one verification: what is wrong with it and the proposed design

Status: sections 3.1-3.4 and 3.6 are implemented (src/engine/ab.rs, `ktools mate1recall`); the site option (3.5) is the
integer coding below; the analysis helper thread is not built: an analysis scans inline before depth 1 (default 1 s, `MateVerify`
sets it). What changed against section 1 is in section 6.

## 1. What the code does today (src/engine/ab.rs)

A root mate in one is looked for by four stages in a chain (`find_best_move_full`, "Root actions"):

| stage | what it covers | its own budget |
|---|---|---|
| `future_mate_in_one` | checking travels from boards the side need not play | `ROOT_PROBE_STEPS` 2048 per travel, `ROOT_FUTURE_MATE_PROBE` 16 actions each |
| `travel_mate` | checking travels (and their completions) from any board | `ROOT_TRAVEL_MATE_COMPLETIONS` 512, `ROOT_PROBE_STEPS` 2048 |
| `root_mate_scan` stage 1 | actions with a precomputed checking entry | `ROOT_CHECKING_SCAN_ACTIONS` 64 / `_STEPS` 512 |
| `root_mate_scan` stage 2 | every action, best-first and natural order alternating | `ROOT_SCAN_ACTIONS` 1024 / `_STEPS` 4096 |
| (new hot fix) stage 3 | the same scan again, past the counts | 2^15 actions / 2^18 steps, 1/12 of the optimum, at most 100 ms |
| `has_evasion` (inside every stage) | proves the checked side has no escape | `TRAVEL_MATE_STEPS` 64 hypercuboids |
| interior nodes | `travel_mate` at plies 1-2 only | `TRAVEL_MATE_STEPS`, 8 completions |

That is 11 unrelated constants for one question ("is there a mate in one?"), the three scan stages repeat each other's work, and
every incident adds a stage or raises a constant:

- missed mate with three boards to play -> raise the scan to 32,768 actions: -88.5 +/- 15.3 Elo (too slow at 10+0.1), reverted;
- then the same scan behind a time share (1/12 of the optimum, <= 100 ms): -1.7 +/- 5.8 Elo, merged as a correctness fix.

Whether the engine sees a mate in one is therefore decided by where the mating action sits in an enumeration order
(it was the 11,575th of the natural order in the reported game) and by the machine speed in the time-share stage.
That is not a guarantee, it is a lottery with good odds.

## 2. Two more incompleteness sources the hot fixes do not touch

1. `has_evasion` treats "ran out of the 64 hypercuboids" as "an evasion exists": `found = next().is_some() || budget == 0 || interrupted()`.
   A real mate has no evasion, so the enumeration must exhaust the whole evasion space; when that space is larger than 64
   hypercuboids (several boards the defender must play, each with many illegal combinations) the mate is silently dropped.
   It never produces a false mate, only a missed one, and nothing reports it.
2. The checking-entry detector (`CheckDetector`) is a static test, so a mate whose check only exists with the other boards'
   moves played (discovered checks, helper captures: the reported game) is found only by the unordered scan.

## 3. Proposed design

One question, one budget, one answer with three states.

### 3.1 `MateScan`: one resumable scan
- A struct (`engine/mate_scan.rs`) owning the cursors and the progress; `run(&mut self, budget) -> Found(action) | Exhausted | OutOfBudget { tried }`
  and it can be called again to continue (analysis keeps going where it stopped).
- One ordering, as a single pipeline: checking travels first, then actions with a checking entry, then the rest best-first with the
  natural-order cursor interleaved (as `scan_for_mate` does today). No repeated passes.
- One budget type `MateBudget { actions: usize, deadline: Option<Instant> }`; the work counters are only a guard against unbounded
  enumeration, the user-visible quantity is time.

### 3.2 Three-valued evasion test
`has_evasion` returns `Yes | No | Unknown` (Unknown = budget hit). A scan candidate (a check with no evasion within the quick budget, which
is rare) that comes back Unknown is re-checked with a larger budget (4096 hypercuboids, then exhaustively when the mode is exhaustive).
This removes the silent false negative in 2.1 at no cost to the common case (candidates are a tiny fraction of the scanned actions).

### 3.3 One user-visible setting: `MateVerify`
UCI option (string): `auto` (default) | `<ms>` | `exhaustive`.

| mode | game | analysis |
|---|---|---|
| `auto` | cheap stages as today + scan share `min(optimum/12, 100 ms)` | cheap stages + 3 s of scan, then keeps scanning on a helper thread while the search runs |
| `<ms>` | that many ms, clamped to the move budget | that many ms (checked from the first iteration) |
| `exhaustive` | refused above a quarter of the move time (a mate in one can wait one more move less than a lost game) | runs until the scan is `Exhausted`, on its own thread |

Throughput measured on the reported position: ~250k actions/s on one thread, so a 18M-action position (the 7 s case of the other
report) takes about 70 s exhaustively: fine for analysis on a helper thread, never inline in a game.

### 3.4 Verification report
The engine states the outcome in the UCI `info`: `string mate1 exhausted` (no mate in one exists, proven), `string mate1 found`,
or `string mate1 partial <actions> <ms>`. The website can show "no mate in one (verified)" / "partially verified (x of y actions)" and
offer the setting. In analysis the helper keeps going, so the report updates.

### 3.5 Website
In the analysis panel: "Mate verification: Auto / 1 s / 10 s / Exhaustive" (sets `MateVerify`); in game settings nothing but
Auto (bounded). The option is persisted with the other engine options (`set_option` already round-trips through the worker).

### 3.6 Recall as a measured quantity, not an anecdote
- `ktools mate1corpus <games.jsonl>`: every checkmate game gives a position (before the winner's last action) that has a mate in one by
  construction; plus the reported positions. ~99% of the games end in mate, so the corpus is tens of thousands of positions for free.
- `ktools mate1recall <corpus> [budget]`: runs the engine at depth 1 with the given `MateVerify` and reports recall (the engine's action
  mates), the time spent per position and the percentage `Exhausted`. Acceptance: recall 100% with `exhaustive`, and for `auto` the
  recall and p99 time are reported with every change to this code. Hot fixes then get a number before they are merged.
- The exact solver (`matex`) stays the oracle; the SPRT is only the regression guard for time (the -88 Elo lesson).

## 4. Work breakdown (engine side)

1. `MateBudget` + `MateScan` (pure refactor of `root_mate_scan` / `scan_for_mate` / the stages; the first cut keeps today's order and
   counts, so the oracle search output and the node counts stay identical in `auto` game mode).
2. Three-valued `has_evasion` + recheck of Unknown candidates.
3. `MateVerify` option, the game/analysis policy, the `info string mate1 ...` report.
4. Helper-thread scan for analysis (uses the existing Lazy SMP helper pool; the main thread polls a shared slot).
5. `mate1corpus` / `mate1recall` and a regression test over the reported positions.
6. Non-regression SPRT for game `auto`; the site change (separate repo) after.

## 5. Decisions needed

- Default analysis budget for `auto` (proposed 3 s before the background scan takes over) and the game cap (kept at 100 ms / 1/12).
- Whether `exhaustive` may be offered in games at all (proposed: no).
- The site option text and where it lives (analysis panel only).

## 6. As implemented

- `MateVerify` UCI option (`auto` | milliseconds | `exhaustive`; integer coding for the web API: 0 auto, negative exhaustive, positive ms).
  Game (a clock): auto = the first 1,024 actions, then 1/12 of the optimum (<= 100 ms); ms and exhaustive are capped at a quarter of the
  optimum. Analysis (no clock): auto = 1 s past the first 1,024 actions; exhaustive = no limit.
- One `MateBudget` {free, actions, steps, deadline} and one `scan_for_mate` pass: the former stage 2 and the time-share stage 3 are a single
  enumeration (no second pass repeating the first 1,024 actions). The scan returns `Found | Exhausted | OutOfBudget { tried }`; either cursor
  finishing its space without running out of hypercuboids proves there is no mate in one, so small positions stop at once (the full scan used
  to run both enumerations to their end).
- `is_mate` / three-valued `evasion`: an escape search that runs out of its 64 hypercuboids is looked at again with 4,096 (root, game) or
  65,536 (analysis) hypercuboids instead of being counted as an escape; interior nodes keep the old behavior (recheck 0).
- An analysis is told the outcome: `info string mate1 none (every action tried)` or `info string mate1 partial (N actions, M ms)`.
- `ktools mate1recall <games.jsonl|positions.g>` measures recall and time (MATE_VERIFY, M1_CLOCK, M1_STRIDE, M1_DEPTH env).
  Local corpus (405 checkmate-game positions, every third game): recall 100% before and after; analysis time p99 0.68 s -> 0.21 s, max 3.2 s ->
  0.5 s, total 16.4 s -> 5.0 s; the 7 reported hard positions (`tests/data`, `games/mate1_tests.g`): 7/7 in auto, 1000 ms and exhaustive.
  `abbench` oracle at depth 9: identical node counts and best moves before and after.
- Interior nodes: `travel_mate` as before, plus an optional scan of a few checking-entry actions at plies <= `INTERIOR_SCAN_PLY` (0 = off
  by default; tested separately by SPRT).
