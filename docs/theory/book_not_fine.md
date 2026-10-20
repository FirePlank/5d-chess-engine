# Book moves the engine says are not fine

Played book actions (weight > 0) that lose more than 50 cp against the best move at their position, clear of the search noise
(first pass plus the deeper pass on the uncertain ones). loss = cp the side to move gives up by playing it; after = score for that side after it.

158 actions: 6 in plain single-timeline opening lines, 83 distinct deeper in long or multi-timeline lines (150 of them under the Bf4 Bf5 e3 g5 sacrificial main line). Everything is in `book_eval_resolved.tsv` (class `play->know`).

Reading it: a flagged move is only compared with the best move at its own position, so a good reply to an opponent's known mistake is
never flagged for that mistake; but a reply that wins by less than the engine's best (e.g. +215 where the engine has a forced mate) shows a big
"loss" because a mate counts as 1,500. In the deeper table 15 of the 89 distinct moves still leave the mover at +150 or better after the move (11 of them
where the engine sees a forced mate at that position): those are still good replies, not mistakes, and should not be demoted. 66 land between
-150 and +150 and 8 are clearly losing (after <= -150).

## Opening lines

| line | weight | loss | after | opening |
|---|---|---|---|---|
| 1. Nf3 Nf6 2. d4 c5 3. dxc5 Qa5+ | 10 | 80 | -23 | Sarp's Counterattack |
| 1. Nf3 Nf6 2. e3 Ng4 3. Ng5 f5 4. Qxg4 | 10 | 111 | -21 | e3 vs f2/f7 Sac: 1. e3 Nf6 2. Nf3 Ng4 |
| 1. e3 Nf6 2. Bb5 c6 3. c3 cxb5 4. Qb3 Qa5 | 5 | 92 | -29 |  |
| 1. e3 Nf6 2. Bb5 c6 3. c3 cxb5 4. Qb3 Qb6 | 10 | 78 | -15 |  |
| 1. Nf3 Nf6 2. d4 c5 3. c3 Qb6 4. e3 Nc6 5. Bc4 | 3 | 66 | -39 | Sarp's Counterattack: Mainline: 4. e3 Nc6 |
| 1. Nf3 Nf6 2. d4 c5 3. c3 Qb6 4. e3 Nc6 5. d5 Nb4 6. g4 Nxg4 7. Qa4 | 10 | 142 | -131 | Sarp's Counterattack: Mainline: Dreamer's Variation: Monte Carlo's Gambit |

## Deeper lines (biggest loss first, last 6 plies shown)

| line | weight | loss | after | opening |
|---|---|---|---|---|
| … Ne4 16. Q>>xh7~ (>L1) 17. Qxf7 Qxh8+ 18. Kd7 Qa6 (36 plies deep) | 10 | 1779 (mate) | -279 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 22. Ne5+ Ke8 23. f4 Rf5 24. Ng6 Rh5 25. h4 (49 plies deep) | 10 | 1458 (mate) | 42 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 15. Kc1 Rf8 16. Qe2 c5 17. Qb5 a6 18. Qxb7 (35 plies deep) | 10 | 1308 (mate) | -1500 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 12. Qe1 Nxf2+ 13. Qxf2 Nd7 14. Qe1 Ne5 15. Nxe6+ (29 plies deep) | 10 | 1285 (mate) | 215 | Classical f7 Defense: Recommended Structure: Bf4 |
| … Nbd7 14. O-O e5 15. Q>>xe6+~ (>L1) 16. Be7 Qh5+ (32 plies deep) | 10 | 1267 (mate) | 233 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 17. dxe6+ fxe6 18. hxg4 b6 19. Rh7 Kc7 20. Nxg5 (39 plies deep) | 10 | 1242 (mate) | 258 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 12. Qe1 e4 13. Nxe4 Qe6 14. Nd6+ Ke7 15. Qxe6+ (29 plies deep) | 10 | 1230 (mate) | 270 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 11. Kd1 e5 12. Qe1 e4 13. Nxe4 Qe6 14. Nd6+ (27 plies deep) | 10 | 1158 (mate) | 342 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 16. Qe2 c5 17. Qb5 a6 18. Qxb7 Nd3+ 19. Kc2 (37 plies deep) | 10 | 1129 (mate) | -1500 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Ne4 10. Nxe6+ (19 plies deep) | 10 | 1113 (mate) | 387 | Classical f7 Defense: Recommended Structure: Bf4 |
| … Nbd7 14. O-O e5 15. Q>>xe6+~ (>L1) 16. Qe7 Qh5+ (32 plies deep) | 10 | 1083 (mate) | 417 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 16. Nxe4 Kd7 17. d5 Na6 18. dxc6+ Kc7 19. Nd6+ (37 plies deep) | 10 | 1074 (mate) | 426 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 10. Nxf7 Qe7 11. Nxh8 Ng4 12. Qxh7 Nxe3+ 13. fxe3 (25 plies deep) | 10 | 1073 (mate) | 427 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 10. Nd2 h6 11. Qe2 hxg5 12. Ne4 Qd8 13. Nxg5 (25 plies deep) | 10 | 474 | -57 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 13. O-O h6 14. Nf3 Nf6 15. Qe2 Ng4 16. h3 (31 plies deep) | 10 | 343 | -203 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 10. Nd2 Nbd7 11. Qe2 e5 12. Ne6+ Qxe6 13. Q>>xe6+~ (>L1) (25 plies deep) | 10 | 334 | 83 | Classical f7 Defense: Recommended Structure: Bf4 |
| … Qa6 14. Bd8+ Kxd8 15. Q>>xg6+~ (>L1) 16. Kd7 Qf7* (32 plies deep) | 10 | 319 | 78 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 12. Qe1 Nxf2+ 13. Qxf2 Nd7 14. Qe1 Qf6 15. Nxe6+ (29 plies deep) | 10 | 265 | 112 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 23. Nxf6+ gxf6 24. Rg1 Na6 25. Rg7+ Kd6 26. c4 (51 plies deep) | 10 | 249 | 29 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 18. Rf1 Qxh2 19. Qd1 Qxg2 20. Rxf2 Qxf2 21. Nd2 (41 plies deep) | 10 | 244 | -314 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 10. Nxf7 Qe7 11. Nxh8 g6 12. Nxg6 Qg7 13. Nf4 (25 plies deep) | 10 | 244 | 143 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 19. O-O-O Nxf2 20. Rxf2 Rxf2 21. Nxe6 Re2 22. Rf1 (43 plies deep) | 10 | 232 | -386 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Nxd5 15. Ne4 (29 plies deep) | 10 | 215 | -123 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Ne4 (39 plies deep) | 10 | 212 | -77 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 20. Rxf2 Rxf2 21. Re1 Rf6 22. Ne4 Kd7 23. Nxf6+ (45 plies deep) | 10 | 202 | 235 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Qe1 e4 15. Ne6+ (29 plies deep) | 10 | 188 | 100 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 10. Nd2 Ne4 11. Ndxe4 e5 12. Ne6+ Qxe6 13. Nc5 (25 plies deep) | 10 | 175 | 80 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 5. d5 Nb4 6. d6 e6 7. g4 Nxg4 8. Qa4 (15 plies deep) | 10 | 173 | -203 | Sarp's Counterattack: Mainline: Dreamer's Variation |
| … 8. Ng5 Bxf4 9. exf4 Qc7 10. Nd2 Qxf4 11. Qe2 (21 plies deep) | 10 | 168 | -22 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 19. dxe5 Rf4 20. O-O-O+ Kc7 21. Rd6 Re4 22. Rxe6 (43 plies deep) | 10 | 165 | -81 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 11. Qe2 h5 12. Nd2 Ne3+ 13. Qxe3 e5 14. Qxe5 (27 plies deep) | 10 | 164 | 87 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 11. Qf1 Ke8 12. g4 Qc7 13. Nd2 Rf8 14. Nf3 (27 plies deep) | 10 | 164 | -34 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 9. exf4 Qe7 10. Nd2 h6 11. Qe2 hxg5 12. Ne4 (23 plies deep) | 10 | 164 | -179 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 12. Qxg4 Nd7 13. Nxe6+ fxe6 14. Re1 Nf6 15. Qh3 (29 plies deep) | 10 | 162 | -16 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 11. Ndxe4 dxe4 12. Qxe4 h6 13. Qe5 Nd7 14. Qe2 (27 plies deep) | 10 | 160 | -143 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 11. Ne5 Kd8 12. Qd1 g6 13. Qa4 b6 14. Qc2 (27 plies deep) | 10 | 157 | 39 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 15. Nf3 Kg8 16. Ng5 Rxe5+ 17. dxe5 Qb6 18. O-O (35 plies deep) | 10 | 157 | 37 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 14. Nh8 Qh5 15. Nf7 Qg4 16. Nd6+ Ke7 17. Nxb7 (33 plies deep) | 10 | 152 | 265 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 h6 14. Nxe6+ (27 plies deep) | 10 | 143 | 5 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 13. Qxf2 h6 14. Nxe6+ fxe6 15. Qe1 Qd6 16. Qe2 (31 plies deep) | 10 | 141 | 69 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 16. Ng5 Rxe5+ 17. dxe5 Qb6 18. O-O Qb5 19. Rfe1 (37 plies deep) | 10 | 140 | -80 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 9. Qe2 Bxf4 10. exf4 Qe7 11. O-O Nh5 12. Qe1 (23 plies deep) | 10 | 138 | -12 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 21. Nc4 Kd7 22. Ne5+ Ke8 23. f4 Rf5 24. Ng6 (47 plies deep) | 10 | 137 | -35 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 10. Nxf7 Kxf7 11. Qf1 Kg8 12. g4 Qc7 13. g5 (25 plies deep) | 10 | 133 | -37 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 e5 12. Ne6+ (23 plies deep) | 10 | 127 | 84 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 (25 plies deep) | 3 | 126 | -15 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 12. Nd2 Qxg5 13. exd5 Kd8 14. dxe6 Re8 15. Ne4 (29 plies deep) | 10 | 125 | -117 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 11. Ndxe4 dxe4 12. Qxe4 h6 13. Qe5 hxg5 14. h4 (27 plies deep) | 10 | 122 | -108 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 15. fxe6 O-O-O 16. exd7+ Rxd7 17. c4 Qg6 18. Qe3 (35 plies deep) | 10 | 121 | 78 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 11. Qc2 Rf8 12. Bxf6 Qb6 13. Bg5 Qa6 14. Bd8+ (27 plies deep) | 10 | 119 | -16 | Classical f7 Defense: Recommended Structure: Bf4 |
| … Nb4 6. d6 e6 7. g4 Nxg4 8. Qa4 Nxf2 (16 plies deep) | 10 | 116 | 87 | Sarp's Counterattack: Mainline: Dreamer's Variation |
| … 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Qxh7 (21 plies deep) | 10 | 115 | -3 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 h5 19. Rxg4 (37 plies deep) | 10 | 113 | -37 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 (27 plies deep) | 3 | 112 | 3 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 12. Qxe4 Qd5 13. Qe2 Nd7 14. Qh5 Qf5 15. O-O (29 plies deep) | 10 | 111 | 120 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 12. Nd2 Qxg5 13. exd5 Kd8 14. Nc4 Qxd5 15. Nd6+ (29 plies deep) | 10 | 109 | -74 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Qd1 (37 plies deep) | 10 | 107 | -101 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 15. Nde4 dxe4 16. Nxe4 Ke7 17. Nc5 Ke8 18. Nxb7 (35 plies deep) | 10 | 106 | 159 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 (25 plies deep) | 10 | 99 | 17 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 10. Bg5 Qb6 11. Bxf6+ Kxf6 12. Qe2 Bf4 13. exf4 (25 plies deep) | 10 | 98 | 238 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 Kd8 15. Nxf7+ (29 plies deep) | 10 | 97 | 71 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 12. Qxe4 Nd7 13. O-O h6 14. f5 Nf6 15. Qe2 (29 plies deep) | 10 | 96 | -22 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 10. exf4 Qe7 11. Kd1 Nbd7 12. Qe1 Ng4 13. Nxe6+ (25 plies deep) | 10 | 95 | -68 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Ne4 Nd3+ 21. Kc2 (41 plies deep) | 10 | 94 | 61 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qb6 10. O-O (19 plies deep) | 10 | 93 | 35 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Na6 12. Qc2 (23 plies deep) | 10 | 93 | 37 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 (29 plies deep) | 10 | 92 | 28 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 8. Ng5 Bxf4 9. exf4 Qc7 10. Nxe6+ fxe6 11. Qe2 (21 plies deep) | 10 | 90 | -82 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 19. Nf3 Re4 20. Ng5 Rxe3+ 21. Kd2 Nd7 22. Kxe3 (43 plies deep) | 10 | 90 | 2 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 (27 plies deep) | 3 | 88 | -6 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 9. Nxf7 Kxf7 10. exf4 Ng4 11. Qe2 h5 12. Nd2 (23 plies deep) | 10 | 88 | 167 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 11. Nxh8 Kd8 12. Nd2 Qf8 13. Ng6 hxg6 14. Nc4 (27 plies deep) | 10 | 85 | 38 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 12. Qd1 b6 13. Na3 Na6 14. Qc2 Nb4 15. Nf7+ (29 plies deep) | 10 | 77 | 192 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 h6 11. Nxf7 (21 plies deep) | 10 | 74 | -41 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 e4 15. Rxe4 (29 plies deep) | 3 | 74 | 39 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 11. Nxh8 Kd8 12. Qd1 b6 13. Ng6 hxg6 14. Qd3 (27 plies deep) | 10 | 72 | 28 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 10. Nxf7 Qf8 11. Ne5 Kd8 12. Qe2 g6 13. Rf1 (25 plies deep) | 10 | 71 | 85 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 11. Qf1 Ke8 12. g4 Qd8 13. g5 Nbd7 14. Qh3 (27 plies deep) | 10 | 66 | 36 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 13. Ng6 Qf7 14. Qf5 O-O-O 15. Nxe5 Qe7 16. Nxc6+ (31 plies deep) | 10 | 65 | 100 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 15. f5 Qxf5 16. Qxf5 exf5 17. Rae1+ Kf7 18. Re6 (35 plies deep) | 10 | 64 | -62 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 11. Qf1 Ke8 12. g4 Qd8 13. g5 Rf8 14. Qh3 (27 plies deep) | 10 | 64 | 45 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 9. exf4 Qd7 10. Nd2 Ng4 11. Qe2 h5 12. Nc4 (23 plies deep) | 10 | 62 | -11 | Classical f7 Defense: Recommended Structure: Bf4 |
| … 9. Qe2 Bxf4 10. exf4 Qe7 11. O-O Ne4 12. Qe1 (23 plies deep) | 10 | 62 | 24 | Classical f7 Defense: Recommended Structure: Bf4 |
