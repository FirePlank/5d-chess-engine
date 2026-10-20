# Book weights and "recommended" bounds: proposal from the evaluation data

Source: `book_evals.tsv` (3,000,000 nodes per position, one thread, 4083 positions) and the derived
`book_eval_actions.tsv` (one row per book action: the mover's score after it, the parent's best,
`loss` = how much the action gives up against the parent's best, clamped to +-1500 so a mate counts
as 1500). Nothing here is applied to the book yet.

## 1. How much to trust a loss: the noise depends on the depth reached

An action that IS the engine's best move at its parent should have loss 0. Its measured loss
(1,567 such actions) shows the noise of comparing two independent searches:

| lower of parent / child depth | n | sd of the loss (cp) |
|---|---|---|
| 16 | 18 | 6 |
| 15 | 111 | 9 |
| 14 | 275 | 11 |
| 13 | 362 | 18 |
| 12 | 302 | 34 |
| 11 | 204 | 43 |
| 10 | 161 | 81 |
| <= 9 | ~130 | 90-130 |

So one global threshold is wrong: +-50 cp is a safe call at depth >= 14 (sd ~10) and a coin flip at depth
11 (sd 43). The proposal uses a per-action uncertainty `sigma(depth)` = 6, 9, 11, 18, 34, 43, 80, 100 for
depth 16+, 15, 14, 13, 12, 11, 10, <= 9. Only 47% of the positions reached depth 13.

## 2. The decision rule

Let L = loss (cp), s = sigma(min depth). Your threshold is 50 cp.

| current tag | rule | actions (non-mate + mate) |
|---|---|---|
| play | L - s > 50: demote to known ("not recommended") | 133 |
| play | L + s <= 50: keep | 1,070 |
| play | otherwise: uncertain, search deeper | 608 |
| know | L + s <= 50: promote to playable | 942 |
| know | L - s > 50: stays known | 540 |
| know | otherwise: uncertain, search deeper | 746 |

1,354 actions (1,960 positions) are uncertain. Resolving them needs a second pass at 4x the nodes
(12M, about +2 depth, sd roughly halves): ~56 s per position, ~3.8 h on 8 threads for all of them,
or ~1.7 h for the ~900 closest to the line (|L - 50| < 1.5 s). I recommend the second pass before
changing any tag: the first pass is enough for the demotions below but not for promotions.

## 3. Weights

Weights are a softmax on the loss: `weight = max(1, round(10 * exp(-max(0, L - 5) / 25)))`

| loss (cp) | 0-5 | 10 | 15 | 20 | 25 | 30 | 40 | 50 | 60+ |
|---|---|---|---|---|---|---|---|---|---|
| weight | 10 | 8 | 7 | 5 | 4 | 4 | 2 | 2 | 1 (and "not recommended" above 50) |

Today 1,680 of the 1,811 played actions have weight 10 whatever their loss (median loss 5 cp, but 15%
lose more than 50 and 6% more than 100); weights 2/3/5 are used by 131 actions only. The formula keeps
the strong moves at 10 and spreads the rest by how much they actually give up.

Three tiers instead of two make the site display honest:

| tier | loss | engine plays it | site shows |
|---|---|---|---|
| recommended | L <= 25 | weight 4-10 | green |
| playable | 25 < L <= 50 | weight 1-4 | yellow ("fine") |
| not recommended | L > 50 (clearly: L - s > 50) | never | red / "known" |

## 4. What it would change (first pass, certain cases only)

Early lines (at most 8 actions deep), clearly worse than 50 cp, currently played:

- 1.e3 Nf6 2.Bb5 c6 3.c3 cxb5 4.Qb3: 4...Qa5 (w5, loss 92) and 4...Qb6 (w10, loss 78)
- 1.Nf3 Nf6 2.d4 c5 3.dxc5 Qa5+ (w10, loss 80)
- 1.Nf3 Nf6 2.d4 c5 3.c3 Qb6 4.e3 Nc6 5.Bc4 (w3, loss 66)
- 20 more demotions are mate-related (the engine finds a mate the book line deviates from).

Known now but clearly fine (candidates to promote, 123 within 8 actions): White's first moves 1.d4, 1.Nc3, 1.c4
(4-5 cp from 1.Nf3), Black's 1...b5 vs 1.c4, 1.e4 d5, many replies in the Digolian, Mongolian and Magic Wall
lines, e.g. 1.Nf3 Nf6 2.Na3 c6 3.Nc4 d5 (loss 0), 1.e3 e6 2.Qf3 Qf6 (3 cp).

## 5. Limits of this data, so the rules above are not over-read

- The engine does not see the human f7-sac theory beyond its depth: an action can be best for the engine and
  still a known mistake (that is why some `know` lines exist). Demotions are safe to apply on the numbers;
  promotions of first moves and of lines the guides call bad should be checked by you.
- 162 of the promotion candidates leave the mover below -100 cp after the move: the move is the best of a
  bad lot, so the earlier move that led there is the one to demote. Do not promote those on loss alone; add
  the guard `mover_score >= -100`.
- 15 promotion candidates are forced-mate lines; keep them known (they are traps, not repertoire).
- A line is only used when every earlier move of it is played, so a promotion deep in an unreachable line changes
  nothing until its parents are playable.
- Scores come from the HCE + net eval at depth 10-17; a better net moves them. The data files should be
  regenerated (`ktools bookeval`, ~2 h) after any net change that wins more than a few Elo.

## 6. Proposed procedure

1. Second pass at 12M nodes on the uncertain positions (closest first, ~1.7 h).
2. Apply: demote (L - s > 50), promote (L + s <= 50, guards above, your review of first moves), re-weight with the
   formula, add the three-tier flag to the book format (`m rec|ok|know`) so the site can colour it.
3. `ktools bookeval` and a test that checks no played action has L - s > 50 and every position keeps at least
   one played action.

## 7. Second pass at 12M nodes (done)

872 positions (those within half a noise spread of the 50 cp line whose first search stopped at depth <= 13) were searched again with 12,000,000
nodes (`book_evals_deep.tsv`, 2 h 5 min). Median depth 11 -> 13 (min 8, max 17). The score of a position moved by a median of 12 cp
(25/75/90%: 4 / 29 / 47 cp), which is the size of the first pass's noise, so it was worth doing.

Re-applying the rule of section 2 with the deeper scores (`book_eval_resolved.tsv`, every action with its class):

| class | first pass | with the deep pass |
|---|---|---|
| play stays (clearly fine) | 1,070 | 1,184 |
| play -> know (clearly worse) | 133 | 158 |
| play, still uncertain | 608 | 469 |
| know -> play (clearly fine) | 942 | 1,066 |
| know stays (clearly worse) | 540 | 609 |
| know, still uncertain | 746 | 553 |

Of the 1,354 uncertain actions 390 were resolved (23 played ones to demote, 136 played ones to keep, 140 known ones to promote, 91 known ones to
keep); 1,022 are still uncertain: the extra depth (+2) only halves the noise at best, and many of those positions sit right on the line, where no
amount of searching helps because the move is simply about 50 cp worse. Options: (a) leave the uncertain ones as they are (the safe default: a played
line keeps its tag, a known one stays known); (b) a third pass at 48M nodes for the ~450 positions still at depth <= 13 (about 4 h); (c) treat the
cp near the line as a tier of its own ("playable", weight 1-2), which is what section 3 does anyway. I recommend (a)+(c): apply the clear
demotions/promotions and the weight formula now, leave the uncertain ones where they are.
