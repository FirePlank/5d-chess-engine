# Search audit against Stockfish, Apeiron and Peras (2026-10-22)

Four read-only comparisons of `src/engine/ab.rs`, `nnue.rs`, `hce.rs`, `time.rs` against
Stockfish (`infinite-chess-engine/stockfish/src`), Apeiron (`infinite-chess-engine/src`) and
Peras (`HydroChess/src`). Line numbers refer to the working tree on 2026-10-22 (ab.rs has the
uncommitted bucketed TT). Items marked (v) were checked by hand against the code.

## A. Bugs

1. (v) `self.excluded` (singular verification) stays set for the whole verification subtree:
   every descendant puts the excluded action in `pre_tried` (never searches it if legal there)
   and no singular test runs below (ab.rs ~2693-2714). References keep it per node.
2. (v) No guard against two null moves in a row (ab.rs ~2657). SF/Apeiron forbid it.
3. (v) A node whose only "action" is a TT action that fails validation returns static eval,
   even when it is checkmate or stalemate (ab.rs ~3068-3074). The same path makes a singular
   test with one legal action depend on the static eval (SF/Apeiron return alpha).
4. Fail-low nodes store nothing when any action was capped/pruned (ab.rs ~3081); SF/Apeiron
   store an upper bound after pruning. Then IIR fires at those nodes again next iteration.
5. Exact is stored at PV nodes even with skipped actions (inconsistent with item 4).
6. When kairos does store a fail-low, it overwrites the TT action with a fail-soft noise move;
   SF/Apeiron keep the old move unless one raised alpha.
7. (v) History table has no colour: white and black moves share entries (ab.rs:809).
8. `checks_left` is dead: the "checking actions are never cut by the cap" rule never applies.
9. `quiet` / LMR test uses a sum of per-board scores (`action_score < 10_000`): a capture plus
   a bad quiet can count as quiet, many good quiets as non-quiet.
10. qsearch at depth limit in check returns static eval (references: alpha); `ply >= MAX_PLY`
    returns static eval in check; qsearch `stand + gain` is not clamped (sum over boards).
11. `ex::K_DEF` and `ex::DEF_BOARD` are the same loop twice (duplicated feature, hce.rs:573-578).
12. Falling-eval time factor uses internal units: saturates at ~2 reported cp (ab.rs ~4055).
13. TT (new buckets): age weighted 1 per search (references: 8); `2*(bound==Exact)` term dead;
    a new proof can be dropped by the keep branch; proven mates never evicted; lock races can
    leave two entries for one key.

## B. Missing techniques (present in SF and/or Apeiron/Peras)

- Mate distance pruning; razoring; ProbCut; quiet-move futility; SEE pruning; history pruning.
- Improving / opponent-worsening; correction history; TT eval refinement; static eval in TT.
- Null move: verification search, eval-based R (kairos R = 2 + d/4 vs 7 + d/3), cutNode.
- RFP only at depth <= 2 (SF < 19, Apeiron < 14), no improving term.
- LMP is a flat cap (20/32/44/56) that stops growing at depth 4, applies to captures and checks.
- LMR: no history/statScore, cutNode, ttPv, improving terms; no deeper/shallower re-search.
- Singular: no double/negative extensions, no multi-cut; starts at depth 4 (refs 6).
- TT cutoff: no one-ply slack for fail-low, no cutNode rule, no history update on TT cut.
- qsearch: no real capture search (capture_gain per board, physical only), no TT, QDEPTH 2,
  QCAP 4 evasions.
- History: no malus, no gravity (halving at 20000, reads clamped to ±5000), no capture history,
  no continuation/pawn/low-ply/countermove history; any tactical action updates nothing.
- Ordering: killers before good captures; no SEE good/bad capture split; check bonus ignores
  SEE.
- Aspiration: re-centres on the previous score, no fail-high depth reduction, constants coarse.
- Root: reordered by null-window fail-soft bounds; no root TT store; no hashfull/prefetch.
- Time: no best-move-change depth factor, effort from one iteration only, no movestogo.
- NNUE: no accumulator stack / Finny analogue; scalar add/sub rows; HCE + residual net + NNUE all
  computed at every eval; `board_chess` not cached; `capture_gain` not cached.

## C. Matches

Aborted-iteration handling, PV-node TT (no cut), exact integer NNUE inference with reference
tests, sparse affine on non-zero inputs, eval cache per side to move.

## D. Status (2026-10-23)

Bug fixes: 4000-game non-regression SPRTs on GitHub (10+0.1, mixed.g), merged when Elo + error >= 0.
Techniques: 8775-game SPRTs, merged when Elo - error > 0 (removals as non-regressions).

| Item | Outcome |
| --- | --- |
| A1 excluded action per node | merged e27b350 (-0.6 +/- 6.9) |
| A2 double null | merged 64e4b23 (+0.4 +/- 7.6) |
| A3 terminal with stale TT action | merged 727c91e (+1.8 +/- 7.5) |
| A4 fail-low upper bound after pruning | rejected: always storing the upper bound hid the k7/p*n6 puzzle mate |
| A5 exact with skipped actions | merged 593c8f0 as a lower bound |
| A6 fail-low keeps the TT action | merged 2eb8d4d, reverted 7bb8125: hid the puzzle mate |
| A7 history colour | merged 1a1eafd (+0.2 +/- 9.3) |
| A8 dead checks_left | removed eed4fca (behavior-identical) |
| A9 quiet by summed score | merged 2aad730 (-1.6 +/- 7.2) |
| A10 qsearch clamp / in-check leaf | clamp merged 604fc14 (+3.9 +/- 6.8); fail-low in check hid the puzzle mate, kept static eval; MAX_PLY in check: maxply_check queued |
| A11 K_DEF / DEF_BOARD | computed once 42a5bd5 (behavior-identical) |
| A12 falling-eval units | merged 7fc2ea4 (-3.3 +/- 9.0) |
| A13 TT ageing / proofs / locks | buckets 850269b, follow-ups ff150fc (-4.1 +/- 7.2), lock waits 91266f8 |
| Mate distance pruning | merged d8cc8fa (-1.0 +/- 5.1, by request) |
| History gravity and malus | merged 26786d2 (+4.8 +/- 6.7, by request) |
| Improving RFP at every depth | rejected: suite 254 vs 261 mates |
| TT eval refinement | skipped: built on the rejected improving code |
| Null reduction by depth and excess | rejected (-4.6 +/- 6.5) |
| LMR history term | rejected (+2.3 +/- 6.3) |
| Quiet futility | rejected (+0.7 +/- 6.6) |
| Singular multi-cut / negative ext. | rejected (-5.8 +/- 6.7) |
| Razoring | rejected: lost 33 suite mates |
| Aspiration re-centring | rejected (-9.2 +/- 8.0, local SPRT) |
| Sacrifice rule removal | merged 24962cb (+4.3 +/- 5.9) |
| Capturing travels at the root | v1 -13.7; v2 (searched last) merged 3c3853b (+0.2 +/- 5.2) |
| NNUE: board cache by key, chess terms memoized | 1549b8f, 9ad5aea (speed, oracle-identical) |
| NNUE past boards retrained | merged e2c166c (+25.2 +/- 9.5) |
| Queued | maxply_check, singular_double, corr_history, capture_history, order_traits, time_stable (GitHub); policy_net, lmp_grow, tt_cut_slack, lmr_research (local) |
