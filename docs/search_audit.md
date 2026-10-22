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
