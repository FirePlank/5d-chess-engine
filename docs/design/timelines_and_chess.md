# Many timelines and plain chess: diagnosis and plan

Two complaints, one root: Kairos thinks about every timeline and every piece the same way,
where a strong human thinks about the few that matter and plays sound chess on the rest.

## What the data says (game of 2026-10-18, Turn Zero, Kairos Black, 2 s per move)

| timelines | NPS | depth |
|---|---|---|
| 1 | ~450k | 11-12 |
| 3 | ~130k | 7 |
| 8 | 30-60k | 4-5 |
| 12 | 3-5k | 3 (one position: depth 1 never finished in 10 s) |

Profile of the late positions (8-12 timelines, `ktools gameprobe`, `--features prof`):

- ~31 candidate points are tried per legal action found; each costs ~4-8 us in `take_point`
  alone. Rejections: jump-order conflicts 40%, checks 34%, wasted moves 14%, present 7%.
- Half of all time is quiescence **check evasions** (~150-200 us per in-check node, up to
  10 ms in the pre-mate position: 125k jump-order rejections for 1k legal evasions). With
  1000 root actions whose replies are checks, depth 1 alone needed > 10 s.
- The rest is main-node generation (~120 us per action).

So the depth collapse is per-node cost (NPS falls 100x) far more than branching: the LMP
caps already bound the actions searched per node.

Plain chess (Peras, depth 14, on the same game's single-timeline phase): 5...g5 (+0.02 ->
+1.93 for White after 6.Nxg5), 7...Rxg5 (the exchange: +4.5 after 8.Bxg5). Kairos rated
Black +0.18 at move 11, where Peras says White +4.2. The tuned weights explain it: rook 945,
knight 846, bishop 1554, pawn 484, queen 2465, against timeline slots 2333, king terms ~1000-2500.
Self-play at 10+0.1 is decided by mates, so outcome-tuning learned that material barely
matters; a human converts material.

## How a strong human handles many timelines

1. **Triage.** Most boards are quiet. A board is hot when a royal is attacked or nearly so,
   when captures or checks are available, or when a travel can reach it.
2. **Default moves on quiet boards.** A quiet board gets one sensible move, chosen quickly. It
   is not searched in combination with everything else.
3. **Calculate only where it matters.** Lines are calculated on the hot boards and on the
   travels between boards. Moves on different boards commute and barely interact, so values
   add up.
4. **Remember assessments.** An unchanged board is not re-thought. A board seen before, even
   on another timeline, is recognised.
5. **Chess first, then 5D.** Every board is a chess position: material, king safety, pawn
   structure. 5D adds timelines, the present, travel threats and softmate on top of that.

## The plan

### A. Search that scales with timelines

A1. **Focused evasions.** In check, the evasion space is first searched with every board that
    is not involved in a check fixed to its default move. Involved boards are the ones holding
    the royal, the checker or a blocking square. The full space comes only when the focused
    space has no evasion. A mate proof still needs the full space.
    Bounded by a step budget (done: `QEVASION_STEPS`).
A2. **Focused actions at interior nodes.** Each axis gets a *heat*: checks, captures, an
    attacked royal, travel arrivals or departures. Deviating from the default on a quiet
    axis costs a large ordering penalty. The best-first cursor then spends its LMP budget on
    alternatives on hot boards instead of permutations of quiet moves (pos 49 of the game:
    17 of the first 25 actions were quiet pawn and piece shuffles on one board). Non-PV nodes
    fix quiet axes to their default outright; PV nodes and the root keep everything.
A3. **Cheaper generation.**
    - `take_point` allocates and clears dim^2 tables per call: make them persistent and sized
      once.
    - Jump-order problems should cut wider slices (all arrivals onto that board, not one
      coordinate).
    - When the chosen point moves no travel and no royal can be attacked across timelines, a
      product of per-board-legal moves is legal. Skip the full check scan (a "lone"-style
      fast path for several boards).
A4. **Per-board caches across the tree.** Board-local facts (legal moves with their local
    scores, local eval terms, safety) are keyed by board hash. Sibling and cousin nodes share
    almost all boards, so most nodes recompute only the boards an action changed.
A5. **Purposeful travels below the root.** Travels that capture, check, defend a royal or
    create a needed timeline are kept; quiet travels at depth >= 2 from the leaves get the
    quiet-axis treatment. The full space stays the fallback for mate proofs.

Targets (`ktools gameprobe` on the game corpus): 8-12 timelines at >= 50k NPS and depth >= 6
in 2 s; no position below depth 3; all known mates (`ktools matecheck`) still found.

### B. Plain chess competence

B1. **Peras as the chess teacher.** Peras (the user's NNUE chess engine, GPL-3.0 like Kairos)
    scores every single-timeline position of the training games, and every present board as a
    chess position. Those scores become extra teacher labels for the eval net, so material,
    structure and endgames are valued the way chess values them, with the 5D terms on top.
B2. **Material sanity suite.** Single-timeline positions from Kairos games where its move
    loses >= 1.5 pawns by Peras at depth 14 and no 5D compensation exists (no travel tactic
    found by a deep Kairos search). Track the blunder rate per engine version; it must fall.
B3. **Validate at longer time controls.** Eval changes go through 10+0.1 SPRT as usual, but a
    material change is also run at 30+0.3, closer to the match. Self-play at bullet speeds
    undervalues material; the user decides when the two disagree.
B4. **Book.** Keep extending the Standard and T0 theory (London, the expert lines) so the
    opening never needs the eval's judgement of gambits.

### Order of work

1. A1 (focused evasions) + A3 table reuse: the largest measured cost, no Elo risk at few
   timelines. Speed oracle plus gameprobe; SPRT as a non-regression test.
2. B2 suite and B1 labels (Peras runs locally from `target/peras/`).
3. A2 heat ordering (SPRT, Elo-seeking).
4. A4 caches, A5 travel pruning (SPRT each).
