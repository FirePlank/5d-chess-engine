# Book tuning: every change

Applied by `data/book_tuning.txt` on top of `data/book_theory_src.txt` (`ktools bookgen`). Rule (docs/theory/book_weight_proposal.md): loss L = cp the mover gives up against the best move at that position, noise s by search depth;
played -> known when L - s > 50 (applied in lines deeper than 8 actions only) (never when the move leaves the mover at +150 or better, and a position left without a played action just leaves the book there); known -> played when L + s <= 50 (NOT applied, see below; not mate lines, not when it leaves the mover below -100);
a played action with L > 35 gets weight min(its old weight, max(1, round(10 * exp(-max(0, L - 5) / 25)))) (the experts' weights are only ever lowered), a promoted one gets the formula's; first moves (ply 1) are left alone. Positions of the second (12M-node) pass use its scores.

4039 book actions evaluated; 329 changed: 126 demoted, 203 re-weighted (applied); 859 promotions and 5 early-line demotions are listed but not applied.

## Re-weighted played actions (old weight -> new weight: count)

| old | new | actions |
|---|---|---|
| 3 | 1 | 2 |
| 5 | 1 | 2 |
| 5 | 2 | 1 |
| 10 | 1 | 128 |
| 10 | 2 | 51 |
| 10 | 3 | 19 |

## Demoted: played -> known (126)

| line and move | | loss | after | opening |
|---|---|---|---|---|
| 1. Nf3 Nf6 2. d4 c5 3. c3 Qb6 4. e3 Nc6 5. d5 Nb4 6. g4 Nxg4 7. Qa4 | demoted | 142 | -131 |  |
| 1. Nf3 Nf6 2. d4 c5 3. c3 Qb6 4. e3 Nc6 5. d5 Nb4 6. d6 e6 7. g4 Nxg4 8. Qa4 | demoted | 173 | -203 |  |
| 1. Nf3 Nf6 2. d4 c5 3. c3 Qb6 4. e3 Nc6 5. d5 Nb4 6. d6 e6 7. g4 Nxg4 8. Qa4 Nxf2 | demoted | 116 | +87 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qb6 10. O-O | demoted | 93 | +35 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qb6 10. O-O | demoted | 93 | +35 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Nd2 Qxf4 11. Qe2 | demoted | 168 | -22 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Nd2 Qxf4 11. Qe2 | demoted | 139 | -26 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Qxh7 | demoted | 115 | -3 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Qxh7 | demoted | 99 | -3 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Nxe6+ fxe6 11. Qe2 | demoted | 90 | -82 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Nxe6+ fxe6 11. Qe2 | demoted | 88 | -82 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 h6 11. Nxf7 | demoted | 74 | -41 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 h6 11. Nxf7 | demoted | 74 | -41 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 h6 11. Qe2 hxg5 12. Ne4 | demoted | 164 | -179 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 h6 11. Qe2 hxg5 12. Ne4 | demoted | 157 | -179 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. O-O Nh5 12. Qe1 | demoted | 138 | -12 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. O-O Nh5 12. Qe1 | demoted | 138 | -12 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 e5 12. Ne6+ | demoted | 127 | +84 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Na6 12. Qc2 | demoted | 93 | +37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Na6 12. Qc2 | demoted | 84 | +46 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ng4 11. Qe2 h5 12. Nc4 | demoted | 62 | -11 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. O-O Ne4 12. Qe1 | demoted | 62 | +24 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 h6 11. Qe2 hxg5 12. Ne4 Qd8 13. Nxg5 | demoted | 474 | -57 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 h6 11. Qe2 hxg5 12. Ne4 Qd8 13. Nxg5 | demoted | 474 | -57 |  |
| … Nd2 Nbd7 Qe2 e5 Ne6+ Qxe6 [Q>>xe6+~ (>L1)] (25 plies deep) | demoted | 334 | +83 |  |
| … Nd2 Nbd7 Qe2 e5 Ne6+ Qxe6 [Q>>xe6+~ (>L1)] (25 plies deep) | demoted | 334 | +83 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 g6 12. Nxg6 Qg7 13. Nf4 | demoted | 244 | +143 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 g6 12. Nxg6 Qg7 13. Nf4 | demoted | 241 | +146 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 e5 12. Ne6+ Qxe6 13. Nc5 | demoted | 175 | +80 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Kg8 12. g4 Qc7 13. g5 | demoted | 133 | -37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Kg8 12. g4 Qc7 13. g5 | demoted | 132 | -37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 | demoted | 126 | -15 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 | demoted | 117 | -4 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 | demoted | 99 | +17 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 e5 12. Ne6+ Qxe6 13. Nc5 | demoted | 96 | +67 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Nbd7 12. Qe1 Ng4 13. Nxe6+ | demoted | 95 | -68 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 | demoted | 81 | +31 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Nbd7 12. Qe1 Ng4 13. Nxe6+ | demoted | 76 | -68 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Ne5 Kd8 12. Qe2 g6 13. Rf1 | demoted | 71 | +85 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. Nxf7 Kxf7 10. exf4 Ng4 11. Qe2 h5 12. Nd2 Ne3+ 13. Qxe3 e5 14. Qxe5 | demoted | 164 | +87 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. Nd2 Rf8 14. Nf3 | demoted | 164 | -34 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. Nxf7 Kxf7 10. exf4 Ng4 11. Qe2 h5 12. Nd2 Ne3+ 13. Qxe3 e5 14. Qxe5 | demoted | 164 | +87 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Qe5 Nd7 14. Qe2 | demoted | 160 | -143 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Qe5 Nd7 14. Qe2 | demoted | 160 | -143 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Ne5 Kd8 12. Qd1 g6 13. Qa4 b6 14. Qc2 | demoted | 157 | +39 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. Nd2 Rf8 14. Nf3 | demoted | 156 | -34 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 h6 14. Nxe6+ | demoted | 143 | +5 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 h6 14. Nxe6+ | demoted | 139 | +9 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Ne5 Kd8 12. Qd1 g6 13. Qa4 b6 14. Qc2 | demoted | 126 | +76 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Qe5 hxg5 14. h4 | demoted | 122 | -108 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Qe5 hxg5 14. h4 | demoted | 122 | -108 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 9. Nxf7 Kxf7 10. Bg5 Ke8 11. Qc2 Rf8 12. Bxf6 Qb6 13. Bg5 Qa6 14. Bd8+ | demoted | 119 | -16 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 9. Nxf7 Kxf7 10. Bg5 Ke8 11. Qc2 Rf8 12. Bxf6 Qb6 13. Bg5 Qa6 14. Bd8+ | demoted | 112 | -16 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 | demoted | 112 | +3 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 | demoted | 100 | +3 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 | demoted | 88 | -6 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Kd8 12. Nd2 Qf8 13. Ng6 hxg6 14. Nc4 | demoted | 85 | +38 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Kd8 12. Nd2 Qf8 13. Ng6 hxg6 14. Nc4 | demoted | 84 | +38 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 | demoted | 76 | -6 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Kd8 12. Qd1 b6 13. Ng6 hxg6 14. Qd3 | demoted | 72 | +28 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Kd8 12. Qd1 b6 13. Ng6 hxg6 14. Qd3 | demoted | 72 | +28 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 Nbd7 14. Qh3 | demoted | 66 | +36 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 Rf8 14. Qh3 | demoted | 64 | +45 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 Nd7 14. Qe1 Qf6 15. Nxe6+ | demoted | 265 | +112 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 Nd7 14. Qe1 Qf6 15. Nxe6+ | demoted | 263 | +112 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Nxd5 15. Ne4 | demoted | 215 | -123 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Nxd5 15. Ne4 | demoted | 202 | -123 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Qe1 e4 15. Ne6+ | demoted | 188 | +100 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Qe1 e4 15. Ne6+ | demoted | 188 | +100 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ng4 12. Qxg4 Nd7 13. Nxe6+ fxe6 14. Re1 Nf6 15. Qh3 | demoted | 162 | -16 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ng4 12. Qxg4 Nd7 13. Nxe6+ fxe6 14. Re1 Nf6 15. Qh3 | demoted | 162 | -16 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Kd8 14. dxe6 Re8 15. Ne4 | demoted | 125 | -117 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Kd8 14. dxe6 Re8 15. Ne4 | demoted | 125 | -117 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Qd5 13. Qe2 Nd7 14. Qh5 Qf5 15. O-O | demoted | 111 | +120 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Kd8 14. Nc4 Qxd5 15. Nd6+ | demoted | 109 | -74 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Kd8 14. Nc4 Qxd5 15. Nd6+ | demoted | 109 | -74 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Qd5 13. Qe2 Nd7 14. Qh5 Qf5 15. O-O | demoted | 101 | +130 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 Kd8 15. Nxf7+ | demoted | 97 | +71 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Nf6 15. Qe2 | demoted | 96 | -22 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Nf6 15. Qe2 | demoted | 96 | -22 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 | demoted | 92 | +28 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 | demoted | 92 | +28 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 e4 15. Rxe4 | demoted | 74 | +39 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Ng4 16. h3 | demoted | 343 | -203 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Ng4 16. h3 | demoted | 297 | -203 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 h6 14. Nxe6+ fxe6 15. Qe1 Qd6 16. Qe2 | demoted | 141 | +69 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 h6 14. Nxe6+ fxe6 15. Qe1 Qd6 16. Qe2 | demoted | 141 | +69 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Qc2 e5 13. Ng6 Qf7 14. Qf5 O-O-O 15. Nxe5 Qe7 16. Nxc6+ | demoted | 65 | +100 |  |
| … Qa6 Bd8+ Kxd8 Q>>xg6+~ (>L1) Kd7 [Qf7*] (32 plies deep) | demoted | 319 | +78 |  |
| … Qa6 Bd8+ Kxd8 Q>>xg6+~ (>L1) Kd7 [Qf7*] (32 plies deep) | demoted | 319 | +78 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Qb5 a6 18. Qxb7 | demoted | 1308 | -1500 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Qb5 a6 18. Qxb7 | demoted | 1308 | -1500 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. Nxf7 Kxf7 10. exf4 Ng4 11. Qe2 h5 12. Nd2 Ne3+ 13. Qxe3 e5 14. Qxe5 Re8 15. Nf3 Kg8 16. Ng5 Rxe5+ 17. dxe5 Qb6 18. O-O | demoted | 157 | +37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Qxg5 15. fxe6 O-O-O 16. exd7+ Rxd7 17. c4 Qg6 18. Qe3 | demoted | 121 | +78 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. Nxf7 Kxf7 10. exf4 Ng4 11. Qe2 h5 12. Nd2 Ne3+ 13. Qxe3 e5 14. Qxe5 Re8 15. Nf3 Kg8 16. Ng5 Rxe5+ 17. dxe5 Qb6 18. O-O | demoted | 120 | +59 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Qxg5 15. fxe6 O-O-O 16. exd7+ Rxd7 17. c4 Qg6 18. Qe3 | demoted | 106 | +90 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 Qxf7 15. f5 Qxf5 16. Qxf5 exf5 17. Rae1+ Kf7 18. Re6 | demoted | 64 | -62 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 Qxf7 15. f5 Qxf5 16. Qxf5 exf5 17. Rae1+ Kf7 18. Re6 | demoted | 64 | -62 |  |
| … Ne4 Q>>xh7~ (>L1) Qxf7 Qxh8+ Kd7 [Qa6] (36 plies deep) | demoted | 1779 | -279 |  |
| … Ne4 Q>>xh7~ (>L1) Qxf7 Qxh8+ Kd7 [Qa6] (36 plies deep) | demoted | 1779 | -279 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Qb5 a6 18. Qxb7 Nd3+ 19. Kc2 | demoted | 1129 | -1500 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Qb5 a6 18. Qxb7 Nd3+ 19. Kc2 | demoted | 1129 | -1500 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. Nxf7 Kxf7 10. exf4 Ng4 11. Qe2 h5 12. Nd2 Ne3+ 13. Qxe3 e5 14. Qxe5 Re8 15. Nf3 Kg8 16. Ng5 Rxe5+ 17. dxe5 Qb6 18. O-O Qb5 19. Rfe1 | demoted | 140 | -80 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. Nxf7 Kxf7 10. exf4 Ng4 11. Qe2 h5 12. Nd2 Ne3+ 13. Qxe3 e5 14. Qxe5 Re8 15. Nf3 Kg8 16. Ng5 Rxe5+ 17. dxe5 Qb6 18. O-O Qb5 19. Rfe1 | demoted | 135 | -65 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 h5 19. Rxg4 | demoted | 113 | -37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 h5 19. Rxg4 | demoted | 113 | -37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Qd1 | demoted | 107 | -101 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Qd1 | demoted | 97 | -91 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Ne4 | demoted | 212 | -77 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Ne4 | demoted | 212 | -77 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Qd1 Qxg2 20. Rxf2 Qxf2 21. Nd2 | demoted | 244 | -314 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Qd1 Qxg2 20. Rxf2 Qxf2 21. Nd2 | demoted | 244 | -314 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Ne4 Nd3+ 21. Kc2 | demoted | 94 | +61 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Ne4 Nd3+ 21. Kc2 | demoted | 94 | +61 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Rxf2 Rxf2 21. Nxe6 Re2 22. Rf1 | demoted | 232 | -386 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Rxf2 Rxf2 21. Nxe6 Re2 22. Rf1 | demoted | 232 | -386 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Ne5+ Nxe5 19. dxe5 Rf4 20. O-O-O+ Kc7 21. Rd6 Re4 22. Rxe6 | demoted | 165 | -81 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Ne5+ Nxe5 19. dxe5 Rf4 20. O-O-O+ Kc7 21. Rd6 Re4 22. Rxe6 | demoted | 165 | -81 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 19. Nf3 Re4 20. Ng5 Rxe3+ 21. Kd2 Nd7 22. Kxe3 | demoted | 90 | +2 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 19. Nf3 Re4 20. Ng5 Rxe3+ 21. Kd2 Nd7 22. Kxe3 | demoted | 90 | +2 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 h5 19. Rxg4 hxg4 20. Ne5+ Kd8 21. Nc4 Kd7 22. Ne5+ Ke8 23. f4 Rf5 24. Ng6 | demoted | 137 | -35 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 h5 19. Rxg4 hxg4 20. Ne5+ Kd8 21. Nc4 Kd7 22. Ne5+ Ke8 23. f4 Rf5 24. Ng6 | demoted | 137 | -35 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 h5 19. Rxg4 hxg4 20. Ne5+ Kd8 21. Nc4 Kd7 22. Ne5+ Ke8 23. f4 Rf5 24. Ng6 Rh5 25. h4 | demoted | 1458 | +42 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 h5 19. Rxg4 hxg4 20. Ne5+ Kd8 21. Nc4 Kd7 22. Ne5+ Ke8 23. f4 Rf5 24. Ng6 Rh5 25. h4 | demoted | 1452 | +48 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Rxf2 Rxf2 21. Re1 Rf6 22. Ne4 Kd7 23. Nxf6+ gxf6 24. Rg1 Na6 25. Rg7+ Kd6 26. c4 | demoted | 249 | +29 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Rxf2 Rxf2 21. Re1 Rf6 22. Ne4 Kd7 23. Nxf6+ gxf6 24. Rg1 Na6 25. Rg7+ Kd6 26. c4 | demoted | 249 | +29 |  |

## Not applied: promotions, known -> played (859), shallowest lines first

| line and move | | loss | after | opening |
|---|---|---|---|---|
| 1. c4 b5 | weight 10 | -10 | +0 | Defended Pawn: English Opening |
| 1. e4 d5 | weight 10 | -3 | -5 | Atonal Attack: 1...d5 (losing for Black) |
| 1. e3 e5 | weight 8 | 11 | -10 |  |
| 1. c4 Ne6 | weight 7 | 14 | -24 | Defended Pawn: English Opening |
| 1. e3 e6 | weight 6 | 16 | -15 |  |
| 1. c4 g6 | weight 6 | 16 | -26 | Defended Pawn: English Opening |
| 1. e4 e5 | weight 5 | 21 | -29 | Atonal Attack |
| 1. e3 e6 | weight 5 | 21 | -18 |  |
| 1. c4 c5 | weight 5 | 24 | -34 | Defended Pawn: Ryan's Opening: Too Slow (1...c5) |
| 1. e3 c6 | weight 4 | 27 | -26 |  |
| 1. e4 e6 | weight 4 | 29 | -37 | Atonal Attack |
| 1. c3 Nf6 2. Qb3 | weight 10 | 1 | -8 |  |
| 1. e3 e6 2. Qf3 | weight 9 | 8 | +10 |  |
| 1. c3 Nf6 2. Qb3 | weight 8 | 12 | -18 |  |
| 1. e3 Nf6 2. Bb5 | weight 6 | 16 | -22 |  |
| 1. e3 e6 2. Qf3 | weight 6 | 16 | -1 |  |
| 1. c3 Nf6 2. Qa4 | weight 6 | 16 | -23 |  |
| 1. e3 Nf6 2. g4 | weight 6 | 18 | -24 |  |
| 1. e3 Nf6 2. Qf3 | weight 6 | 18 | -24 |  |
| 1. c3 Nf6 2. Qa4 | weight 5 | 20 | -26 |  |
| 1. e3 Nf6 2. Bb5 | weight 5 | 23 | -23 |  |
| 1. e3 Nf6 2. c3 | weight 5 | 24 | -30 |  |
| 1. e3 Nf6 2. g4 | weight 4 | 25 | -25 |  |
| 1. e3 Nf6 2. Qf3 | weight 4 | 27 | -27 |  |
| 1. e3 Nf6 2. Bc4 | weight 4 | 28 | -34 |  |
| 1. e3 Nf6 2. c3 | weight 4 | 30 | -30 |  |
| 1. Nf3 Nf6 2. c3 | weight 3 | 34 | -12 |  |
| 1. c4 c5 2. Nf3 Nf6 | weight 10 | -1 | -27 |  |
| 1. Nf3 Nf6 2. Nd4 d5 | weight 9 | 8 | +38 | Mongolian Attack |
| 1. e3 e6 2. Qf3 Qf6 | weight 9 | 8 | -18 |  |
| 1. Nf3 Nf6 2. Na3 c6 | weight 9 | 9 | -4 | Digolian Attack: 2...c6 |
| 1. e4 Nf6 2. d4 Nxe4 | weight 7 | 15 | -19 | Atonal Attack: 2...Nxe4 (blunder) |
| 1. c4 Nf6 2. Nf3 g6 | weight 4 | 27 | -28 |  |
| 1. e4 Nf6 2. d4 c6 | weight 4 | 28 | -32 | Atonal Attack: 1...Nf6 2. d4 |
| 1. Nf3 Nf6 2. e3 Ng4 | weight 3 | 36 | -29 | e3 vs f2/f7 Sac: 1. e3 Nf6 2. Nf3 Ng4 |
| 1. e3 Nf6 2. g4 d5 3. g5 | weight 10 | -13 | -17 |  |
| 1. c4 Nf6 2. Nf3 Ne6 3. d4 | weight 10 | -7 | +20 | Defended Pawn: Ryan's Opening: 3. d4 |
| 1. Nf3 Nf6 2. d4 d5 3. Nc3 | weight 10 | -3 | +22 | Classical f7 Defense (London Structures) |
| 1. e3 Nf6 2. g4 g5 3. c3 | weight 9 | 7 | -22 |  |
| 1. e3 Nf6 2. g4 d5 3. g5 | weight 9 | 9 | -27 |  |
| 1. e3 e6 2. Qf3 Qf6 3. Nc3 | weight 8 | 10 | +8 |  |
| 1. e3 Nf6 2. g4 g5 3. c3 | weight 8 | 12 | -37 |  |
| 1. Nf3 Nf6 2. c3 d5 3. e4 | weight 8 | 12 | -18 |  |
| 1. Nf3 Nf6 2. c4 e6 3. c5 | weight 8 | 12 | +13 | Asymmetrical English: 2...e6 |
| 1. Nf3 Nf6 2. Nd4 e6 3. e3 | weight 6 | 18 | -34 | Mongolian Attack: 2...e6 |
| 1. Nf3 Nf6 2. e3 e6 3. c3 | weight 5 | 23 | -5 | e3 Structure: 2...e6 |
| 1. e4 Nf6 2. d4 e6 3. c3 | weight 5 | 23 | -10 | Atonal Attack: 2...e6 |
| 1. e3 e6 2. Qf3 Qf6 3. Qxf6 | weight 4 | 25 | -8 |  |
| 1. c3 Nf6 2. Qa4 c6 3. a3 | weight 4 | 27 | -33 |  |
| 1. c3 Nf6 2. Qa4 c6 3. a3 | weight 4 | 29 | -41 |  |
| 1. Nf3 Nf6 2. Nd4 c5 3. Nf5 | weight 4 | 30 | -61 | Mongolian Attack: 2...c5: The Zigzag |
| 1. Nf3 Nf6 2. e3 d5 3. Nd4 | weight 4 | 30 | -41 | e3 Structure: 2...d5 |
| 1. Nf3 Nf6 2. d4 d5 3. Bd2 | weight 4 | 31 | -12 | Classical f7 Defense (London Structures) |
| 1. e3 Nf6 2. Bb5 c6 3. c3 | weight 3 | 33 | -58 |  |
| 1. e3 Nf6 2. Bb5 Nc6 3. c3 | weight 3 | 33 | -48 |  |
| 1. e3 Nf6 2. Bb5 c6 3. c3 | weight 3 | 35 | -58 |  |
| 1. e3 Nf6 2. Bb5 Nc6 3. c3 | weight 3 | 37 | -55 |  |
| 1. e3 Nf6 2. Bb5 e6 3. c3 | weight 3 | 39 | -42 |  |
| 1. e4 Nf6 2. d4 Nxe4 3. Qf3 Nf6 | weight 10 | -92 | +55 |  |
| 1. Nf3 Nf6 2. Nd4 d5 3. e3 c6 | weight 10 | 0 | +41 | Mongolian Attack: Classical f7 Defense |
| 1. Nf3 Nf6 2. Na3 c6 3. Nc4 d5 | weight 10 | 0 | +5 |  |
| 1. Nf3 Nf6 2. e3 c5 3. b3 Nc6 | weight 10 | 1 | -6 | e3 Structure: Symmetrical English Invitational: 3. b3 |
| 1. e3 e6 2. Qf3 Qf6 3. Qxf6 Nxf6 | weight 10 | 3 | +5 |  |
| 1. Nf3 Nf6 2. d4 d5 3. Nc3 Nc6 | weight 10 | 5 | -27 | Classical f7 Defense: 3. Nc3 (not recommended) |
| 1. b4 Nf6 2. Nf3 Ne6 3. Bb2 g6 | weight 9 | 7 | -21 |  |
| 1. e3 e6 2. Qf3 Qf6 3. Nc3 Nc6 | weight 9 | 8 | -16 |  |
| 1. c4 c5 2. Nf3 Nf6 3. b3 Ne6 | weight 8 | 12 | -32 |  |
| 1. Nf3 Nf6 2. Na3 d5 3. Nb5 c6 | weight 7 | 13 | +15 | Digolian Attack: 2...d5: 3. Nb5 c6 |
| 1. Nf3 Nf6 2. e3 e6 3. b3 Ng4 | weight 5 | 21 | -25 | e3 vs f2/f7 Sac: 1. e3 Nf6 2. Nf3 e6 3. b3 |
| 1. e3 Nf6 2. b4 e6 3. Bb2 h5 | weight 5 | 21 | -29 | Magic Wall: Mainline (2...e6) |
| 1. Nf3 Nf6 2. Nd4 c5 3. c3 cxd4 | weight 5 | 24 | -21 | Mongolian Attack: 2...c5: 3. c3 (f7 sac approach) |
| 1. e3 Nf6 2. b4 e6 3. Bb2 g6 | weight 5 | 24 | -32 | Magic Wall: Mainline (2...e6) |
| 1. e3 Nf6 2. b4 e6 3. Bb2 Ne4 | weight 4 | 26 | -34 | Magic Wall: Mainline (2...e6) |
| 1. c4 Nf6 2. Nf3 Ne6 3. b3 g6 | weight 4 | 31 | -24 |  |
| 1. Nf3 Nf6 2. d4 d5 3. Bd2 Bd7 | weight 3 | 35 | -23 |  |
| 1. e3 Nf6 2. Bb5 c6 3. c3 cxb5 4. Qb3 | weight 10 | -14 | -63 |  |
| 1. e3 Nf6 2. g4 d5 3. g5 Ne4 4. Qf3 | weight 10 | -1 | -9 |  |
| 1. e3 Nf6 2. g4 g5 3. c3 c6 4. Qb3 | weight 10 | -1 | -24 |  |
| 1. Nf3 Nf6 2. e3 d5 3. Bd3 c6 4. b3 | weight 10 | 1 | +0 |  |
| 1. Nf3 Nf6 2. Nd4 c5 3. Nf5 g6 4. Nd6+ | weight 10 | 2 | -49 | Mongolian Attack: The Zigzag: 3...g6 The Zigzag Gambit |
| 1. e3 e6 2. Qf3 Qf6 3. Nc3 Nc6 4. Nb5 | weight 10 | 3 | +13 |  |
| 1. Nf3 Nf6 2. c4 c5 3. Nc3 Nc6 4. d3 | weight 9 | 8 | +3 | Symmetrical English Four Knights Variation |
| 1. c3 Nf6 2. Qa4 c6 3. a3 d5 4. Qb3 | weight 9 | 8 | -53 |  |
| 1. c3 Nf6 2. Qa4 c6 3. a3 d5 4. Qb3 | weight 9 | 9 | -52 |  |
| 1. e3 e6 2. Qf3 Qf6 3. Qxf6 Nxf6 4. f4 | weight 9 | 9 | -14 |  |
| 1. e3 Nf6 2. g4 d5 3. g5 Ne4 4. Qf3 | weight 8 | 10 | -9 |  |
| 1. e3 Nf6 2. g4 g5 3. c3 c6 4. Qb3 | weight 8 | 12 | -37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Nbd2 | weight 6 | 18 | +3 | Classical f7 Defense: Recommended Structure |
| 1. e3 Nf6 2. Bb5 c6 3. c3 cxb5 4. Qb3 | weight 5 | 22 | -61 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. e3 | weight 3 | 35 | -14 | Classical f7 Defense: Recommended Structure |
| 1. c4 Nf6 2. Nf3 Ne6 3. d4 b5 4. b3 | weight 3 | 38 | -28 |  |
| 1. e3 Nf6 2. b4 e6 3. Bb2 Ne4 4. Qh5 g6 | weight 10 | -7 | -21 |  |
| 1. Nf3 Nf6 2. Na3 d5 3. Nb5 c6 4. Nbd4 Bg4 | weight 10 | 0 | +35 | Digolian Attack: 2...d5: Trap line (4...Bg4) |
| 1. Nf3 Nf6 2. Na3 c6 3. Nc4 d5 4. Nce5 Be6 | weight 10 | 4 | +10 | Digolian Attack: 2...c6: 4...Be6 |
| 1. Nf3 Nf6 2. d4 c5 3. c3 Qb6 4. e3 e6 | weight 10 | 5 | -20 | Sarp's Counterattack: Mainline: 4. e3 |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bg5 h6 | weight 10 | 6 | -20 |  |
| 1. Nf3 Nf6 2. e3 Ng4 3. Ng5 f5 4. Qxg4 fxg4 | weight 10 | 6 | +15 |  |
| 1. e3 Nf6 2. b4 d5 3. Bb2 c6 4. Bxf6 h5 | weight 9 | 7 | -54 |  |
| 1. e3 Nf6 2. b4 e6 3. Bb2 h5 4. Qf3 Qe7 | weight 9 | 8 | -17 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Nbd2 Nbd7 | weight 8 | 10 | -13 | Classical f7 Defense: Recommended Structure: Nbd2 |
| 1. e3 Nf6 2. b4 e6 3. Bb2 Nd5 4. Qh5 g6 | weight 8 | 10 | -69 |  |
| 1. c4 Nf6 2. Nf3 Ne6 3. b3 g6 4. Bb2 Bg7 | weight 8 | 10 | -21 |  |
| 1. Nf3 Nf6 2. d4 c5 3. c3 Qb6 4. dxc5 Qe6 | weight 8 | 12 | -49 | Sarp's Counterattack: Mainline: The Senatus Gambit |
| 1. c4 c5 2. Nf3 Nf6 3. b3 Ne6 4. Bb2 g6 | weight 8 | 12 | -38 |  |
| 1. Nf3 Nf6 2. Na3 c6 3. Nc4 d5 4. Nce5 Bf5 | weight 5 | 20 | -6 | Digolian Attack: 2...c6: 4...Bf5 |
| 1. e3 e6 2. Qf3 Qf6 3. Qxf6 Nxf6 4. f4 Ng4 | weight 5 | 22 | -8 |  |
| 1. Nf3 Nf6 2. Na3 c6 3. Nc4 d5 4. Nce5 Bg4 | weight 4 | 27 | -13 | Digolian Attack: 2...c6: 4...Bg4 |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. e3 e6 | weight 4 | 29 | -15 | Classical f7 Defense: Recommended Structure: e3 |
| 1. c4 Nf6 2. Nf3 Ne6 3. d4 b5 4. b3 Qb6 5. Bd2 | weight 10 | 6 | -29 |  |
| 1. Nf3 Nf6 2. c4 c5 3. Nc3 Nc6 4. d3 b5 5. cxb5 | weight 9 | 9 | +17 |  |
| 1. e3 e6 2. Qf3 Qf6 3. Qxf6 Nxf6 4. f4 Ng4 5. h3 | weight 4 | 25 | -17 |  |
| 1. Nf3 Nf6 2. e3 d5 3. Bd3 c6 4. Ng5 Bg4 5. Qxg4 Nxg4 | weight 10 | -89 | +28 |  |
| 1. Nf3 Nf6 2. Nd4 d5 3. e3 c6 4. Bd3 Bg4 5. Qxg4 Nxg4 | weight 10 | -20 | -25 |  |
| 1. e3 Nf6 2. b4 d5 3. Bb2 c6 4. Bxf6 h5 5. Bxe7+ Qxe7 | weight 10 | -10 | -2 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bg5 Bg4 5. e3 Ne4 | weight 10 | 0 | -6 |  |
| 1. c4 c5 2. Nf3 Nf6 3. b3 Ne6 4. Bb2 g6 5. Nc3 Bg7 | weight 10 | 0 | -37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bg5 Bg4 5. e3 Ne4 | weight 10 | 1 | -6 |  |
| 1. Nf3 Nf6 2. Na3 c6 3. Nc4 d5 4. Nce5 Bg4 5. Nxg4 Nxg4 | weight 10 | 2 | -1 |  |
| 1. Nf3 Nf6 2. Na3 d5 3. Nb5 c6 4. Nbd4 Bg4 5. e3 e6 | weight 10 | 3 | +31 |  |
| 1. Nf3 Nf6 2. e3 Ng4 3. Ng5 f5 4. Qxg4 fxg4 5. Nf7 e6 | weight 8 | 10 | +16 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bg5 h6 5. Bh4 g5 | weight 7 | 13 | -13 | Classical f7 Defense: Recommended Structure: Bg5: Bh4 g5 |
| 1. Nf3 Nf6 2. d4 c5 3. c3 Qb6 4. e3 e6 5. Nbd2 cxd4 | weight 6 | 16 | -37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 | weight 6 | 19 | -42 |  |
| 1. Nf3 Nf6 2. e3 c5 3. b3 Nc6 4. Bb2 b6 5. Ng5 e6 | weight 5 | 20 | -94 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 | weight 5 | 23 | -42 |  |
| 1. e3 Nf6 2. b4 e6 3. Bb2 h5 4. Qf3 Qe7 5. a3 Ng4 | weight 4 | 26 | -47 |  |
| 1. Nf3 Nf6 2. c4 c5 3. Nc3 Nc6 4. d3 b5 5. cxb5 Nd4 6. e3 | weight 10 | 0 | +22 |  |
| 1. Nf3 Nf6 2. e3 d5 3. Bd3 c6 4. b3 Bg4 5. h3 Bh5 6. g4 | weight 10 | 5 | -11 |  |
| 1. e3 e6 2. Qf3 Qf6 3. Qxf6 Nxf6 4. f4 Ng4 5. h3 Nf2 6. Kxf2 | weight 8 | 12 | +41 |  |
| 1. Nf3 Nf6 2. d4 c5 3. c3 Qb6 4. e3 Nc6 5. d5 Nb4 6. g4 | weight 5 | 23 | +1 | Sarp's Counterattack: Mainline: Dreamer's Variation: Monte Carlo's Gambit |
| 1. Nf3 Nf6 2. e3 Ng4 3. Ng5 f5 4. Qxg4 fxg4 5. Nf7 e6 6. Nxd8+ Kxd8 | weight 10 | -22 | -55 |  |
| 1. Nf3 Nf6 2. d4 c5 3. c3 Qb6 4. e3 e6 5. Nbd2 cxd4 6. Nxd4 Nc6 | weight 10 | -16 | -15 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 | weight 10 | -8 | -37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 | weight 10 | -5 | -28 |  |
| 1. c4 c5 2. Nf3 Nf6 3. b3 Ne6 4. Bb2 g6 5. Nc3 Bg7 6. Nd5 Nxd5 | weight 10 | -2 | -40 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bg5 Bg4 5. e3 Ne4 6. h3 Bxf3 | weight 10 | 1 | +0 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bg5 Bg4 5. e3 Ne4 6. h3 Bxf3 | weight 10 | 4 | -3 |  |
| 1. Nf3 Nf6 2. d4 c5 3. c3 Qb6 4. e3 Nc6 5. d5 Nb4 6. g4 Nbxd5 | weight 9 | 9 | -10 | Sarp's Counterattack: Mainline: Dreamer's Variation: Monte Carlo's Gambit |
| 1. Nf3 Nf6 2. d4 c5 3. c3 Qb6 4. e3 Nc6 5. d5 Nb4 6. g4 Nxg4 | weight 8 | 10 | -11 | Sarp's Counterattack: Mainline: Dreamer's Variation: Monte Carlo's Gambit |
| 1. Nf3 Nf6 2. c4 c5 3. Nc3 Nc6 4. d3 b5 5. cxb5 Nd4 6. e3 Nxb5 7. Nxb5 | weight 10 | 3 | +51 |  |
| 1. Nf3 Nf6 2. d4 c5 3. c3 Qb6 4. e3 Nc6 5. Nbd2 cxd4 6. Nxd4 Nxd4 7. cxd4 | weight 6 | 17 | -11 |  |
| 1. Nf3 Nf6 2. c4 c5 3. Nc3 Nc6 4. d3 b5 5. cxb5 Nd4 6. e3 Nxb5 7. Qb3 | weight 4 | 31 | +23 |  |
| 1. e3 Nf6 2. b4 d5 3. Bb2 c6 4. Bxf6 h5 5. Bxe7+ Qxe7 6. Qf3 f5 7. Qg3 g5 | weight 10 | -63 | +18 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 | weight 10 | -9 | -27 |  |
| 1. Nf3 Nf6 2. e3 Ng4 3. Ng5 f5 4. Qxg4 fxg4 5. Nf7 e6 6. Nxd8+ Kxd8 7. a3 Na6 | weight 10 | -6 | +1 |  |
| 1. Nf3 Nf6 2. Na3 d5 3. Nb5 c6 4. Nbd4 Bg4 5. e3 e6 6. h3 Bh5 7. Nxe6+ fxe6 | weight 10 | 3 | -78 |  |
| 1. c4 c5 2. Nf3 Nf6 3. b3 Ne6 4. Bb2 g6 5. Nc3 Bg7 6. Nd5 Nxd5 7. cxd5 Nd4 | weight 9 | 8 | -48 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 | weight 9 | 9 | -45 |  |
| 1. Nf3 Nf6 2. c4 c5 3. Nc3 Nc6 4. d3 b5 5. cxb5 Nd4 6. e3 Nxb5 7. Nxb5 Qa5+ 8. Bd2 | weight 9 | 9 | +60 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 | weight 10 | -44 | +7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 | weight 10 | -44 | +7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 | weight 10 | -6 | -31 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 | weight 10 | -6 | -31 |  |
| 1. Nf3 Nf6 2. Na3 d5 3. Nb5 c6 4. Nbd4 Bg4 5. e3 e6 6. h3 Bh5 7. Nxe6+ fxe6 8. Ne5 Bxd1+ | weight 10 | -1 | -82 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 | weight 10 | 5 | -42 |  |
| 1. c4 c5 2. Nf3 Nf6 3. b3 Ne6 4. Bb2 g6 5. Nc3 Bg7 6. Nd5 Nxd5 7. cxd5 Nd4 8. e3 Nxf3+ | weight 9 | 7 | -33 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 | weight 5 | 24 | -61 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 | weight 3 | 32 | -69 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 | weight 10 | -20 | -20 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 9. Bxd6 Qxd6 | weight 10 | -17 | -29 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 | weight 10 | -17 | -20 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 9. Bxd6 Qxd6 | weight 10 | -17 | -29 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 | weight 10 | -7 | +21 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 | weight 10 | -1 | +15 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 h6 | weight 9 | 8 | -39 |  |
| 1. c4 c5 2. Nf3 Nf6 3. b3 Ne6 4. Bb2 g6 5. Nc3 Bg7 6. Nd5 Nxd5 7. cxd5 Nd4 8. e3 Nxf3+ 9. gxf3 Bxb2 | weight 9 | 8 | -51 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 h6 | weight 7 | 14 | -52 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 | weight 6 | 16 | -54 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 9. Nxf7 Kxf7 | weight 6 | 17 | -30 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 9. Nxf7 Kxf7 | weight 6 | 17 | -30 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 | weight 6 | 18 | -56 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 | weight 5 | 23 | -54 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Be7 | weight 3 | 35 | -73 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Be7 | weight 3 | 39 | -70 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Qe2 | weight 10 | -1 | +50 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Qe3 | weight 10 | 0 | +49 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 9. Bxd6 Qxd6 10. Qe2 | weight 10 | 2 | +27 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 9. Bxd6 Qxd6 10. Qe2 | weight 10 | 2 | +27 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Qe3 | weight 10 | 3 | +65 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Qe2 | weight 6 | 18 | +50 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 h6 | weight 10 | -23 | +13 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Nxe6+ fxe6 | weight 10 | -21 | -8 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Qe2 Ne4 | weight 10 | -20 | -30 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 h6 | weight 10 | -19 | -33 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Ne4 | weight 10 | -12 | -59 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 | weight 10 | -10 | +25 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Qe3 Qe7 | weight 10 | -10 | -55 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 | weight 10 | -10 | +25 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ng4 | weight 10 | -9 | -43 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Nxe6+ fxe6 | weight 10 | -8 | -6 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Qe2 Ne4 | weight 10 | -7 | -43 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ng4 | weight 10 | -1 | -9 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 | weight 10 | 0 | -23 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Nh5 | weight 10 | 4 | -56 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 h6 | weight 10 | 5 | +13 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 | weight 10 | 5 | -30 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 9. Nxf7 Kxf7 10. Bg5 Nbd7 | weight 10 | 5 | -56 |  |
| 1. c4 c5 2. Nf3 Nf6 3. b3 Ne6 4. Bb2 g6 5. Nc3 Bg7 6. Nd5 Nxd5 7. cxd5 Nd4 8. e3 Nxf3+ 9. gxf3 Bxb2 10. Qxb2 f6 | weight 10 | 5 | -40 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Qe3 Qe7 | weight 10 | 6 | -55 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Qe3 Ne4 | weight 10 | 6 | -71 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 h6 10. Nxe6+ fxe6 | weight 7 | 13 | -35 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 h6 10. Nxe6+ fxe6 | weight 7 | 13 | -35 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Ne4 | weight 7 | 15 | -81 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 9. Nxf7 Kxf7 10. Bg5 Ke8 | weight 7 | 15 | -66 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 9. Nxf7 Kxf7 10. Bg5 Na6 | weight 7 | 15 | -66 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Qe3 Ne4 | weight 6 | 16 | -65 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 | weight 5 | 20 | -91 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Nbd7 | weight 5 | 20 | -30 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 h6 | weight 5 | 21 | -33 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ng4 | weight 4 | 27 | -9 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ng4 | weight 4 | 31 | -43 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Nd2 | weight 10 | -1 | +92 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. Nd2 | weight 10 | -1 | +24 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. Nd2 | weight 10 | 4 | +26 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. O-O | weight 7 | 14 | +77 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Nxh8 | weight 4 | 31 | +65 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. O-O | weight 4 | 31 | +53 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 | weight 10 | -27 | -70 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 h6 | weight 10 | -27 | -26 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 h6 11. Qe2 hxg5 | weight 10 | -26 | +22 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Nd2 h6 | weight 10 | -26 | -66 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 h6 11. Nxf7 Qxf7 | weight 10 | -19 | +60 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 h6 11. Nxf7 Qxf7 | weight 10 | -19 | +60 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 | weight 10 | -18 | -35 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 9. Nxf7 Kxf7 10. Bg5 Ke8 11. Qc2 Rf8 | weight 10 | -18 | -80 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 | weight 10 | -14 | -61 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 | weight 10 | -14 | -76 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 9. Nxf7 Kxf7 10. Bg5 Ke8 11. Qc2 Rf8 | weight 10 | -13 | -85 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. O-O h6 | weight 10 | -12 | -65 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 | weight 10 | -12 | -18 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 | weight 10 | -12 | -18 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Qe3 Ne4 11. Nxe4 dxe4 | weight 10 | -11 | -60 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 h6 11. Qe2 hxg5 | weight 10 | -10 | +15 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Nbd7 | weight 10 | -7 | -46 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 h6 | weight 10 | -6 | -28 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 | weight 10 | -4 | -93 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 dxe4 | weight 10 | -3 | -27 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ng4 11. Qe2 h5 | weight 10 | -3 | -44 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 dxe4 | weight 10 | -3 | -27 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 | weight 10 | -1 | -74 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 | weight 10 | 1 | -35 |  |
| 1. c4 c5 2. Nf3 Nf6 3. b3 Ne6 4. Bb2 g6 5. Nc3 Bg7 6. Nd5 Nxd5 7. cxd5 Nd4 8. e3 Nxf3+ 9. gxf3 Bxb2 10. Qxb2 f6 11. e4 Qe5 | weight 10 | 2 | -32 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 | weight 10 | 3 | -90 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ng4 11. Qe2 h5 | weight 10 | 4 | -51 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Qe3 Ne4 11. Nxe4 dxe4 | weight 10 | 4 | -65 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 h6 10. Nxe6+ fxe6 11. Qf3 Bxf4 | weight 10 | 4 | -14 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 h6 10. Nxe6+ fxe6 11. Qf3 Bxf4 | weight 10 | 5 | -2 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qc2 Qc7 | weight 9 | 7 | -100 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. O-O Ne4 | weight 9 | 9 | -86 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. O-O h6 | weight 8 | 10 | -63 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Kf8 | weight 8 | 11 | +62 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Kf8 | weight 8 | 11 | +62 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Nbd7 | weight 8 | 12 | -46 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Nd2 h6 | weight 8 | 12 | -75 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 | weight 6 | 16 | +57 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 | weight 6 | 16 | +57 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Nxe4 | weight 6 | 19 | -49 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Nxe4 | weight 6 | 19 | -49 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 e5 | weight 5 | 22 | -75 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 | weight 4 | 31 | +42 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 | weight 4 | 31 | +42 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. O-O Ne4 | weight 3 | 33 | -86 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 h6 10. Nxe6+ fxe6 11. Qf3 Qe7 12. Qg3 | weight 10 | -56 | +184 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ng4 12. Qxg4 | weight 10 | -24 | +79 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Ng6 | weight 10 | -23 | +93 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Ng6 | weight 10 | -17 | +93 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Kd8 12. Nd2 | weight 10 | -10 | +117 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Na3 | weight 10 | -8 | +78 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Kd8 12. Nd2 | weight 10 | 1 | +114 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 | weight 10 | 2 | -59 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 | weight 10 | 2 | -59 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Na3 | weight 7 | 13 | +63 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Ne5 Kd8 12. Qe2 | weight 6 | 19 | +103 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Qd1 | weight 4 | 28 | +42 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 | weight 10 | -56 | +14 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 | weight 10 | -50 | +14 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 g6 9. Nxf7 Kxf7 10. Bg5 Ke8 11. Qc2 Rf8 12. Bxf6 Qb6 | weight 10 | -36 | -86 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Nbd7 12. Qe1 Ng4 | weight 10 | -31 | -8 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Ng6 Qf7 | weight 10 | -21 | -72 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Qxh7 Nxh7 12. Ne5 Qxf2+ | weight 10 | -19 | -7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Nbd7 | weight 10 | -19 | -23 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Qxh7 Nxh7 12. Ne5 Qxf2+ | weight 10 | -19 | -7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Na6 12. Qc2 Nb4 | weight 10 | -16 | -30 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ng4 12. Qxg4 Nd7 | weight 10 | -14 | -65 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Nbd7 | weight 10 | -13 | -23 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Ng6 Qf7 | weight 10 | -12 | -81 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Nbd7 12. Qe1 Ng4 | weight 10 | -12 | -27 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 | weight 10 | -9 | -85 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 | weight 10 | -9 | -85 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Na6 12. Qc2 Nb4 | weight 10 | -7 | -30 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ng4 12. Qe1 Nd7 | weight 10 | -7 | -27 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 h6 10. Nxe6+ fxe6 11. Qf3 Qe7 12. Bxd6 Qxd6 | weight 10 | -6 | +6 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 | weight 10 | -3 | -49 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Nxe4 12. Nxe4 dxe4 | weight 10 | -2 | -49 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 | weight 10 | 0 | -60 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Qd5 | weight 10 | 0 | -94 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 h6 10. Nxe6+ fxe6 11. Qf3 Bxf4 12. exf4 Qe7 | weight 10 | 0 | -21 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Qd5 | weight 10 | 0 | -94 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ng4 12. Qxg4 Nd7 | weight 10 | 0 | -65 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ng4 12. Qe1 Nd7 | weight 10 | 1 | -8 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Nxe4 12. Nxe4 dxe4 | weight 10 | 2 | -49 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 h6 10. Nxe6+ fxe6 11. Qf3 Bxf4 12. exf4 Qe7 | weight 10 | 5 | -27 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 h6 11. Nxf7 Qxf7 12. Qe2 Ne4 | weight 10 | 6 | -47 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 h6 11. Nxf7 Qxf7 12. Qe2 Ne4 | weight 10 | 6 | -47 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 | weight 9 | 7 | -67 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 | weight 9 | 7 | -59 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Qc2 Nc5 | weight 9 | 9 | -90 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. O-O Ne4 12. Qe1 Nxg5 | weight 9 | 9 | -33 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Qc2 Nc5 | weight 9 | 9 | -90 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Na6 | weight 8 | 10 | -70 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 h6 10. Nxe6+ fxe6 11. Qf3 Qe7 12. Bxd6 Qxd6 | weight 7 | 13 | +6 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f4 Nb4 | weight 7 | 14 | +38 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. O-O Ne4 12. Qe1 Nxg5 | weight 7 | 14 | -39 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Na6 | weight 6 | 18 | -70 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qc7 10. Qe3 Ne4 11. Nxe4 dxe4 12. Qg3 g6 | weight 6 | 19 | -79 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f4 Nb4 | weight 5 | 21 | +38 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Nxe4 12. Nxe4 Qf4 | weight 5 | 24 | -75 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. Qg2 | weight 10 | -9 | +125 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. Qg2 | weight 10 | -7 | +119 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 e5 12. Ne6+ Qxe6 13. Nd6+ | weight 10 | 0 | +163 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Nxe4 12. Nxe4 Qf4 13. Ned2 | weight 7 | 15 | +60 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd1 | weight 6 | 18 | -40 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd1 | weight 6 | 18 | -40 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Qd5 13. Qxd5 | weight 4 | 31 | +63 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Qd5 13. Qxd5 | weight 4 | 31 | +63 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Kd8 12. Qd1 b6 13. Ng6 | weight 3 | 33 | +97 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f4 Qc7 13. f5 | weight 3 | 36 | -6 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f4 Qc7 13. f5 | weight 3 | 37 | -6 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 Ng4 | weight 10 | -59 | +28 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Nbd7 12. Qe1 Ng4 13. Nxe6+ Qxe6 | weight 10 | -34 | +102 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Nbd7 12. Qe1 Ng4 13. Nxe6+ Qxe6 | weight 10 | -34 | +102 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O Nf6 | weight 10 | -18 | -63 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f4 Qc7 13. f5 Qb6 | weight 10 | -15 | +21 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Nxe4 12. Nxe4 Qf4 13. Nbd2 dxe4 | weight 10 | -14 | -85 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Ng4 | weight 10 | -13 | +17 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. O-O Ne4 12. Qe1 Nxg5 13. fxg5 Qxg5 | weight 10 | -12 | -5 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Ne5 Kd8 12. Qe2 g6 13. Rf1 Ne4 | weight 10 | -9 | -76 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Qxh7 Nxh7 12. Ne5 Qxf2+ 13. Kxf2 Rf8+ | weight 10 | -8 | -14 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Nxe4 12. Nxe4 Qf4 13. Nbd2 dxe4 | weight 10 | -8 | -85 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Qxh7 Nxh7 12. Ne5 Qxf2+ 13. Kxf2 Rf8+ | weight 10 | -8 | -14 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Qe5 hxg5 | weight 10 | -7 | -14 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Qe5 hxg5 | weight 10 | -7 | -14 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f4 Qc7 13. f5 Qb6 | weight 10 | -7 | +13 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 Qf6 | weight 10 | -7 | -52 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 | weight 10 | -7 | -1 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 | weight 10 | -6 | -70 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Qe5 Nd7 | weight 10 | -4 | -17 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Qe5 Nd7 | weight 10 | -4 | -17 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Nf3 Nd7 | weight 10 | -3 | -59 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Ne5 Kd8 12. Qe2 g6 13. Rf1 Ne4 | weight 10 | -3 | -76 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Nf3 Nd7 | weight 10 | -2 | -59 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Ng6 Qf7 13. Nh8 Qh5 | weight 10 | 0 | -47 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Qd5 13. Qxd5 cxd5 | weight 10 | 1 | -64 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 | weight 10 | 1 | -82 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Qd5 13. Qxd5 cxd5 | weight 10 | 1 | -64 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Kd8 12. Qd1 b6 13. Ng6 hxg6 | weight 10 | 3 | -100 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd1 Nd3+ | weight 10 | 4 | +36 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd1 Nd3+ | weight 10 | 4 | +36 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Kd8 12. Qd1 b6 13. Ng6 hxg6 | weight 10 | 5 | -100 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Nbd7 13. exd5 O-O-O | weight 10 | 5 | -92 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Nbd7 13. exd5 O-O-O | weight 10 | 5 | -92 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Nxe4 12. Nxe4 Qf4 13. Ned2 Nd7 | weight 10 | 6 | -66 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Ng4 | weight 9 | 7 | +8 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 | weight 9 | 7 | -1 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Nxe4 12. Nxe4 Qf4 13. Ned2 Nd7 | weight 9 | 8 | -63 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 | weight 9 | 8 | -16 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Kd8 | weight 9 | 8 | -16 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 Ng4 | weight 9 | 9 | -26 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. O-O Ne4 12. Qe1 Nxg5 13. fxg5 Qxg5 | weight 5 | 21 | -32 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Ng6 Qf7 13. Nh8 Qh5 | weight 5 | 22 | -47 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 | weight 4 | 25 | -33 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 | weight 3 | 39 | -33 |  |
| … Qe2 hxg5 Ne4 Qd7 Q>>xe6+~ (>L1) [Qe7] (27 plies deep) | weight 10 | -60 | +71 |  |
| … Qe2 hxg5 Ne4 Qd7 Q>>xe6+~ (>L1) [Qe7] (27 plies deep) | weight 10 | -60 | +71 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 e5 12. Ne6+ Qxe6 13. Nd6+ Qxd6 14. Qh3 | weight 10 | -36 | +199 |  |
| … e4 dxe4 g3 Qd5 Q>>xe6+~ (>L1) [Qe7] (27 plies deep) | weight 10 | -17 | +51 |  |
| … e4 dxe4 g3 Qd5 Q>>xe6+~ (>L1) [Qe7] (27 plies deep) | weight 10 | -17 | +51 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. O-O-O | weight 10 | -15 | +31 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O Nf6 14. Qe2 | weight 10 | -11 | +74 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 h6 10. Nxe6+ fxe6 11. Qf3 Bxf4 12. exf4 Qe7 13. Qe2 e5 14. O-O | weight 10 | -10 | +123 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Qe5 hxg5 14. Qxg7 | weight 10 | -7 | +21 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Qe5 hxg5 14. Qxg7 | weight 10 | -7 | +21 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Kd8 14. Nc4 | weight 10 | -3 | +19 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Kd8 14. Nc4 | weight 10 | -3 | +19 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Qc2 e5 13. Ng6 Qf7 14. Ne7+ | weight 10 | -1 | +115 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. O-O-O | weight 10 | 0 | +31 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 | weight 9 | 7 | +63 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 | weight 5 | 24 | +58 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 | weight 10 | -25 | -61 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 | weight 10 | -25 | -61 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. g4 e5 13. g5 e4 14. gxf6 Qxf6 | weight 10 | -15 | -72 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Kd8 14. dxe6 Re8 | weight 10 | -15 | -8 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Kd8 14. dxe6 Re8 | weight 10 | -15 | -8 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 Ng4 14. Qg1 Nxe3+ | weight 10 | -11 | -37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. O-O-O Qg4 | weight 10 | -10 | -21 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. O-O-O Qg4 | weight 10 | -10 | -21 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 Qxf7 | weight 10 | -8 | -50 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 Qxf7 | weight 10 | -7 | -56 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 | weight 10 | -5 | -17 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 hxg5 | weight 10 | -4 | +10 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Ng4 14. Qg1 Nxe3+ | weight 10 | -3 | +13 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Ng4 14. Qg1 Nxe3+ | weight 10 | -2 | +13 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 | weight 10 | -1 | -21 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 Nbd7 14. Qh3 Qe7 | weight 10 | -1 | -35 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 h6 14. Nxe6+ fxe6 | weight 10 | -1 | -8 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 Nbd7 14. Qh3 Qe7 | weight 10 | -1 | -35 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Qd5 13. O-O Qxe4+ 14. Nxe4 O-O | weight 10 | 0 | -86 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. Nxf7 Kxf7 10. exf4 Ng4 11. Qe2 h5 12. Nd2 Ne3+ 13. Qxe3 e5 14. Qxe5 Re8 | weight 10 | 0 | -87 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 | weight 10 | 0 | -3 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Qd5 13. O-O Qxe4+ 14. Nxe4 O-O | weight 10 | 0 | -86 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. Nxf7 Kxf7 10. exf4 Ng4 11. Qe2 h5 12. Nd2 Ne3+ 13. Qxe3 e5 14. Qxe5 Re8 | weight 10 | 0 | -87 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 | weight 10 | 0 | -3 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. g4 e5 13. g5 e4 14. gxf6 Qxf6 | weight 10 | 1 | -89 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Ne5 Kd8 12. Qe2 g6 13. Rf1 Ne4 14. Qg4 Qf6 | weight 10 | 4 | -66 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qf8 11. Ne5 Kd8 12. Qe2 g6 13. Rf1 Ne4 14. Qg4 Qf6 | weight 10 | 4 | -66 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Kd8 12. Nd2 Qf8 13. Ng6 hxg6 14. Nc4 dxc4 | weight 10 | 5 | -43 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Kd8 12. Nd2 Qf8 13. Ng6 hxg6 14. Nc4 dxc4 | weight 10 | 5 | -43 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Qe5 Nd7 14. Qe2 hxg5 | weight 10 | 6 | +137 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Qe5 Nd7 14. Qe2 hxg5 | weight 10 | 6 | +137 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 hxg5 | weight 9 | 7 | -1 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 | weight 7 | 13 | -11 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 | weight 7 | 13 | -11 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 | weight 7 | 14 | +18 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 | weight 7 | 14 | +18 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Nf3 Nd7 14. Qe2 Nf6 | weight 5 | 20 | -100 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 Ng4 14. Qg1 Nxe3+ | weight 5 | 21 | -30 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 Rf8 14. Qh3 Qd7 | weight 4 | 26 | -71 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 Rf8 14. Qh3 Qd7 | weight 4 | 26 | -71 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 e4 | weight 3 | 34 | -94 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Nxd5 15. Nc4 | weight 10 | -21 | +100 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. O-O-O Qg4 15. Qxg4 | weight 10 | -16 | +37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. O-O-O Qg4 15. Qxg4 | weight 10 | -16 | +37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. e5 | weight 10 | -14 | +7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. e5 | weight 10 | -14 | +7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 Kd8 15. Rxe5 | weight 10 | -10 | +172 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. O-O-O Qg6 15. dxe6 | weight 10 | 0 | +42 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. O-O-O Qg6 15. dxe6 | weight 10 | 3 | +42 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Nxd5 15. Nc4 | weight 10 | 5 | +87 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 Kd8 15. Rxe5 | weight 10 | 6 | +162 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Nf3 Nd7 14. Qe2 O-O 15. Ne5 | weight 9 | 7 | +125 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Nf3 Nd7 14. Qe2 O-O 15. Ne5 | weight 7 | 14 | +111 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Qh3 | weight 6 | 17 | +4 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Qh3 | weight 5 | 20 | -3 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Qd5 13. O-O Qxe4+ 14. Nxe4 O-O 15. Nd6 | weight 4 | 29 | +57 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Qd5 13. O-O Qxe4+ 14. Nxe4 O-O 15. Nd6 | weight 4 | 29 | +57 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Qc2 e5 13. Ng6 Qf7 14. Ne7+ Kxe7 15. dxe5 Ne4 | weight 10 | -166 | +7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Qc2 e5 13. Ng6 Qf7 14. Ne7+ Kxe7 15. dxe5 Ne4 | weight 10 | -160 | +7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Nf6 15. Qe2 hxg5 | weight 10 | -39 | +61 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Nf6 15. Qe2 hxg5 | weight 10 | -39 | +61 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. Nd2 Rf8 14. Nf3 Nbd7 15. Ng5 Qd6 | weight 10 | -30 | -3 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 | weight 10 | -21 | -36 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 | weight 10 | -21 | -36 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 | weight 10 | -15 | +52 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 | weight 10 | -15 | +52 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. Nd2 Rf8 14. Nf3 Nbd7 15. Ng5 Qd6 | weight 10 | -14 | -3 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Nxd5 15. Ne4 Ne3+ | weight 10 | -14 | +137 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 cxd5 15. Nge4 dxe4 | weight 10 | -14 | +34 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Nxd5 15. Ne4 Ne3+ | weight 10 | -14 | +137 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 cxd5 15. Nge4 dxe4 | weight 10 | -14 | +34 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 Kd8 15. Nxf7+ Rxf7 | weight 10 | -13 | -84 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 e4 15. Rxe4 dxe4 | weight 10 | -12 | -68 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 Kd8 15. Nxf7+ Qxf7 | weight 10 | -12 | -85 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Nxd5 15. Nc4 Ne3+ | weight 10 | -10 | -90 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. Nxf7 Kxf7 10. exf4 Ng4 11. Qe2 h5 12. Nd2 Ne3+ 13. Qxe3 e5 14. Qxe5 Re8 15. Nf3 Kg8 | weight 10 | -4 | -93 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. Nxf7 Kxf7 10. exf4 Ng4 11. Qe2 h5 12. Nd2 Ne3+ 13. Qxe3 e5 14. Qxe5 Re8 15. Nf3 Kg8 | weight 10 | -4 | -93 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 | weight 10 | -3 | -72 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 Qxf7 15. f5 Qxf5 | weight 10 | -3 | -36 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 | weight 10 | -3 | -1 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 | weight 10 | -3 | -25 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 f5 15. Qxf5 exf5 | weight 10 | 0 | +6 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Qf4+ Qxf4 | weight 10 | 1 | +67 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Qf4+ Qxf4 | weight 10 | 1 | +67 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Nxd5 15. Nc4 Ne3+ | weight 10 | 1 | -88 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qd7 10. Nd2 h6 11. Nxf7 Qxf7 12. Qe2 Qd7 13. Nf3 Qd6 14. Ne5 Nbd7 15. Qd3 Ne4 | weight 10 | 2 | -58 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Kd8 14. Nc4 Qxd5 15. Nd6+ Qxd6 | weight 10 | 2 | +72 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Kd8 14. Nc4 Qxd5 15. Nd6+ Qxd6 | weight 10 | 2 | +72 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 h6 15. exd5 cxd5 | weight 10 | 4 | -59 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 | weight 10 | 4 | -32 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 | weight 10 | 4 | -1 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 | weight 10 | 4 | -32 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 h6 15. exd5 cxd5 | weight 10 | 4 | -59 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 | weight 10 | 4 | -32 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 | weight 9 | 7 | +8 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 | weight 9 | 7 | +8 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ng4 12. Qxg4 Nd7 13. Nxe6+ fxe6 14. Re1 Nf6 15. Qh3 O-O-O | weight 8 | 10 | +6 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ng4 12. Qxg4 Nd7 13. Nxe6+ fxe6 14. Re1 Nf6 15. Qh3 O-O-O | weight 8 | 10 | +6 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 Qxf7 15. f5 Qxf5 | weight 8 | 11 | -44 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 Nbd7 14. Qh3 Qe7 15. gxf6 Nxf6 | weight 8 | 12 | -46 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nh5 | weight 8 | 12 | -53 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 Nbd7 14. Qh3 Qe7 15. gxf6 Nxf6 | weight 8 | 12 | -46 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nh5 | weight 8 | 12 | -53 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 | weight 7 | 13 | -79 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Ke7 | weight 7 | 14 | -7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. fxg7 Nd7 | weight 7 | 14 | -39 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Ke7 | weight 7 | 14 | -7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. fxg7 Nd7 | weight 7 | 15 | -40 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Na6 | weight 6 | 16 | -9 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Na6 | weight 6 | 16 | -9 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 e4 15. Rxe4 dxe4 | weight 6 | 17 | -56 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f4 Nb4 13. Kd2 Ne4+ 14. Kc1 Nxc3+ 15. Nxc3 Kg8 | weight 6 | 18 | +39 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f4 Nb4 13. Kd2 Ne4+ 14. Kc1 Nxc3+ 15. Nxc3 Kg8 | weight 6 | 18 | +39 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Ng4 | weight 6 | 19 | -94 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 Rf8 14. Qh3 Qd7 15. gxf6 g6 | weight 5 | 20 | -80 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 Kd8 15. Nxf7+ Qxf7 | weight 4 | 25 | -96 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 dxe4 | weight 4 | 30 | +7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nfd7 | weight 4 | 30 | -71 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 dxe4 | weight 4 | 30 | +7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qd8 13. g5 Rf8 14. Qh3 Qd7 15. gxf6 g6 | weight 4 | 30 | -80 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nfd7 | weight 4 | 30 | -71 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 | weight 2 | 41 | -82 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 | weight 2 | 41 | -82 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 dxe4 16. cxb4 | weight 10 | -19 | +12 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 dxe4 16. cxb4 | weight 10 | -19 | +12 |  |
| … Q>>xe6+~ (>L1) Qe7 Qc8+ Qd8 Qxd8+ [Kxd8] (31 plies deep) | weight 10 | -13 | +55 |  |
| … Q>>xe6+~ (>L1) Qe7 Qc8+ Qd8 Qxd8+ [Kxd8] (31 plies deep) | weight 10 | -13 | +55 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Qc2 e5 13. Ng6 Qf7 14. Qf5 O-O-O 15. Nxe5 Qe7 16. Nd2 | weight 10 | -3 | +168 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. O-O-O Qg4 15. Qxg4 Nxg4 16. Ne4 | weight 10 | -1 | +37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. O-O-O Qg4 15. Qxg4 Nxg4 16. Ne4 | weight 10 | -1 | +37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Nxd5 15. Nc4 Ne3+ 16. fxe3 | weight 10 | 0 | +88 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Nxd5 15. Nc4 Ne3+ 16. fxe3 | weight 10 | 2 | +88 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. O-O-O Qg6 15. dxe6 Ne4 16. exf7+ | weight 10 | 4 | +50 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. O-O-O Qg6 15. dxe6 Ne4 16. exf7+ | weight 10 | 4 | +50 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Qe7 11. Nxh8 Nbd7 12. Qc2 e5 13. Ng6 Qf7 14. Qf5 O-O-O 15. Nxe5 Qe7 16. Nd2 | weight 9 | 8 | +157 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qd1 Rf2 | weight 10 | -32 | +100 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qd1 Rf2 | weight 10 | -32 | +100 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 | weight 10 | -19 | -18 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 | weight 10 | -19 | -18 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 16. exf6 hxg5 | weight 10 | -18 | -59 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 16. exf6 hxg5 | weight 10 | -18 | -59 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. fxg7 Nd7 16. gxf8=Q+ Nxf8 | weight 10 | -17 | -26 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 cxd5 15. Nge4 dxe4 16. Nxe4 Na6 | weight 10 | -13 | +37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 cxd5 15. Nge4 dxe4 16. Nxe4 Na6 | weight 10 | -13 | +37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nh5 16. Nc4 dxc4 | weight 10 | -12 | -18 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nh5 16. Nc4 dxc4 | weight 10 | -12 | -18 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 Qxf7 15. f5 Qxf5 16. Qxf5 exf5 | weight 10 | -10 | +1 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 Qxf7 15. f5 Qxf5 16. Qxf5 exf5 | weight 10 | -10 | +1 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 h6 13. Nf3 Nd7 14. Qe2 Nf6 15. Ne5 Nd5 16. Nxc6 Ne3+ | weight 10 | -9 | -95 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Na6 16. Nd6+ Ke7 | weight 10 | -8 | +20 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Na6 16. Nd6+ Ke7 | weight 10 | -8 | +20 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ | weight 10 | -7 | -2 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qg6 | weight 10 | -7 | +11 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qg6 | weight 10 | -7 | +11 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Kd7 16. dxe6+ fxe6 | weight 10 | -6 | -33 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 h6 15. exd5 exd5 16. Nh7 Nf6 | weight 10 | -6 | -98 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Kd7 16. dxe6+ fxe6 | weight 10 | -6 | -33 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 h6 15. exd5 exd5 16. Nh7 Nf6 | weight 10 | -6 | -98 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ng4 12. Qxg4 Nd7 13. Nxe6+ fxe6 14. Re1 Nf6 15. Qh3 O-O-O 16. Rxe6 Qd7 | weight 10 | -5 | +21 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ | weight 10 | -5 | -2 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ng4 12. Qxg4 Nd7 13. Nxe6+ fxe6 14. Re1 Nf6 15. Qh3 O-O-O 16. Rxe6 Qd7 | weight 10 | -5 | +21 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Nd7 | weight 10 | -3 | -6 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Kd8 14. dxe6 Re8 15. Ne4 Rxe6 16. Nxg5 Rxe2+ | weight 10 | -3 | -67 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 h6 15. exd5 cxd5 16. Nge4 dxe4 | weight 10 | -3 | -58 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Kd8 14. dxe6 Re8 15. Ne4 Rxe6 16. Nxg5 Rxe2+ | weight 10 | -3 | -67 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 h6 15. exd5 cxd5 16. Nge4 dxe4 | weight 10 | -3 | -58 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Nd7 | weight 10 | -2 | -5 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Rf7 | weight 10 | -1 | +43 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 | weight 10 | -1 | +5 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Rf7 | weight 10 | -1 | +43 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 | weight 10 | -1 | +5 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 e4 15. Rxe4 dxe4 16. Qe3 Nd7 | weight 10 | 1 | -62 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f4 Nb4 13. Kd2 Ne4+ 14. Kc1 Nd3+ 15. Qxd3 Kg8 16. Qe2 Rf8 | weight 10 | 2 | +93 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 dxe4 16. cxb4 Rf8 | weight 10 | 2 | -14 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 dxe4 16. cxb4 Rf8 | weight 10 | 2 | -14 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f4 Nb4 13. Kd2 Ne4+ 14. Kc1 Nd3+ 15. Qxd3 Kg8 16. f5 Rf8 | weight 10 | 4 | +52 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. O-O-O Qg6 15. dxe6 Ne4 16. exf7+ Kd8 | weight 10 | 4 | -54 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nfd7 16. Nxh7 Rh8 | weight 10 | 4 | -58 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 16. g3 e5 | weight 10 | 4 | -81 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f4 Nb4 13. Kd2 Ne4+ 14. Kc1 Nd3+ 15. Qxd3 Kg8 16. f5 Rf8 | weight 10 | 4 | +52 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. O-O-O Qg6 15. dxe6 Ne4 16. exf7+ Kd8 | weight 10 | 4 | -54 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nfd7 16. Nxh7 Rh8 | weight 10 | 4 | -58 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 f5 15. Qxf5 exf5 16. Rfe1 Qe4 | weight 10 | 5 | +31 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 f5 15. Qxf5 exf5 16. Rfe1 Qe4 | weight 10 | 5 | +31 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 h6 14. Nxe6+ fxe6 15. Qe1 Qd6 16. Qe2 g6 | weight 10 | 6 | -75 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nfd7 16. Nc4 dxc4 | weight 10 | 6 | -38 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 h6 14. Nxe6+ fxe6 15. Qe1 Qd6 16. Qe2 g6 | weight 10 | 6 | -75 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nfd7 16. Nc4 dxc4 | weight 10 | 6 | -38 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 16. g3 e5 | weight 9 | 7 | -81 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f4 Nb4 13. Kd2 Ne4+ 14. Kc1 Nd3+ 15. Qxd3 Kg8 16. Qe2 Rf8 | weight 9 | 7 | +88 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. fxg7 Nd7 16. gxf8=Q+ Nxf8 | weight 8 | 11 | -48 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 | weight 8 | 11 | -22 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 | weight 8 | 11 | -22 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 Kd8 15. Nxf7+ Qxf7 16. Qh4+ Qf6 | weight 7 | 13 | -93 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Kd7 16. Na5 Kc8 | weight 6 | 16 | +31 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Kd7 16. Na5 Kc8 | weight 6 | 16 | +31 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ | weight 3 | 36 | +6 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ | weight 3 | 36 | +6 |  |
| … Qe7 Qc8+ Qd8 Ne6+ fxe6 Qg6+ [hxg6] (33 plies deep) | weight 10 | -50 | +316 |  |
| … Qe7 Qc8+ Qd8 Ne6+ fxe6 Qg6+ [hxg6] (33 plies deep) | weight 10 | -50 | +316 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. dxe6 | weight 10 | -31 | -17 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. dxe6 | weight 10 | -31 | -17 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. dxc6 | weight 10 | -12 | -36 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. dxc6 | weight 10 | -12 | -36 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Kd7 16. dxe6+ fxe6 17. f3 | weight 10 | -6 | +39 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Kd7 16. dxe6+ fxe6 17. f3 | weight 10 | -6 | +39 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. O-O-O | weight 10 | 1 | +17 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. O-O-O | weight 10 | 1 | +17 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. a3 | weight 10 | 2 | -75 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. Nd6+ | weight 9 | 7 | -55 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. Nd6+ | weight 9 | 7 | -55 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. a3 | weight 8 | 11 | -84 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. e5 | weight 4 | 26 | -99 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. e5 | weight 4 | 26 | -99 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 e4 15. Rxe4 dxe4 16. Qe3 Nd7 17. Qh3 h6 | weight 10 | -73 | -98 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 e4 15. Rxe4 dxe4 16. Qe3 Nd7 17. Qh3 h6 | weight 10 | -64 | -98 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 | weight 10 | -38 | -35 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 | weight 10 | -38 | +23 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 | weight 10 | -38 | -35 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 | weight 10 | -38 | +23 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. dxe6 O-O-O | weight 10 | -29 | +46 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. dxe6 O-O-O | weight 10 | -29 | +46 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 Kd8 15. Nxf7+ Qxf7 16. Qh4+ Qf6 17. Qg4 Qf5 | weight 10 | -19 | -37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Qb5 a6 | weight 10 | -16 | +192 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. fxg7 Nd7 16. gxf8=Q+ Nxf8 17. f4 Qh5 | weight 10 | -14 | -58 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Qb5 a6 | weight 10 | -12 | +192 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. fxg7 Nd7 16. gxf8=Q+ Nxf8 17. f4 Qh5 | weight 10 | -12 | -58 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Kd7 16. dxe6+ fxe6 17. Ne5+ Nxe5 | weight 10 | -10 | +19 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Kd7 16. dxe6+ fxe6 17. Ne5+ Nxe5 | weight 10 | -10 | +19 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 | weight 10 | -5 | +23 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qg6 17. Nd2 Nd7 | weight 10 | -4 | +4 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Kd7 16. dxe6+ fxe6 17. Na5 Kc8 | weight 10 | -4 | +14 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Kd8 14. dxe6 Re8 15. Ne4 Rxe6 16. Nxg5 Rxe2+ 17. Kxe2 Nbd7 | weight 10 | -2 | -70 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Kd8 14. dxe6 Re8 15. Ne4 Rxe6 16. Nxg5 Rxe2+ 17. Kxe2 Nbd7 | weight 10 | -2 | -70 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qh4 Rf6 | weight 10 | -1 | +48 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 | weight 10 | -1 | -47 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 | weight 10 | -1 | -47 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 | weight 10 | 0 | +5 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nh5 16. Nc4 dxc4 17. Ne4 Ke7 | weight 10 | 0 | -28 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nh5 16. Nc4 dxc4 17. Ne4 Ke7 | weight 10 | 0 | -28 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 | weight 10 | 1 | -51 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 | weight 10 | 1 | +4 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 | weight 10 | 1 | -51 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Kd7 16. dxe6+ fxe6 17. Na5 Kc8 | weight 10 | 2 | +14 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 Qxf7 15. f5 Qxf5 16. Qxf5 exf5 17. Rae1+ Kf7 | weight 10 | 3 | -2 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 Qxf7 15. f5 Qxf5 16. Qxf5 exf5 17. Rae1+ Kf7 | weight 10 | 3 | -2 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 16. exf6 hxg5 17. fxg7 Rg8 | weight 10 | 4 | -83 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qh4 Rf6 | weight 10 | 4 | +43 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 16. exf6 hxg5 17. fxg7 Rg8 | weight 10 | 4 | -83 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Kd7 16. dxe6+ fxe6 17. f3 Nf6 | weight 9 | 7 | -46 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 Kd8 15. Nxf7+ Qxf7 16. Qh4+ Qf6 17. Qg4 Qf5 | weight 9 | 7 | -37 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Kd7 16. dxe6+ fxe6 17. f3 Nf6 | weight 9 | 7 | -46 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 16. g3 e5 17. Qxe5 O-O+ | weight 9 | 9 | -91 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 h6 15. exd5 exd5 16. Nh7 Nf6 17. Nxf8+ Kxf8 | weight 9 | 9 | -83 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 16. g3 e5 17. Qxe5 O-O+ | weight 9 | 9 | -91 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 h6 15. exd5 exd5 16. Nh7 Nf6 17. Nxf8+ Kxf8 | weight 9 | 9 | -83 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qg6 17. Nd2 Nd7 | weight 8 | 10 | +4 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 | weight 8 | 11 | +23 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. Nd6+ Ke7 | weight 7 | 14 | +41 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. Nd6+ Ke7 | weight 7 | 14 | +41 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nfd7 16. Nc4 dxc4 17. Ne4 Ke7 | weight 7 | 15 | -48 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nfd7 16. Nc4 dxc4 17. Ne4 Ke7 | weight 7 | 15 | -48 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Rf7 17. cxb4 Qf8 | weight 5 | 24 | +12 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Rf7 17. cxb4 Qf8 | weight 5 | 24 | +12 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ | weight 4 | 31 | -38 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 h6 15. exd5 exd5 16. Nge4 dxe4 17. Nxe4 Kd7 | weight 3 | 32 | -97 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 h6 15. exd5 exd5 16. Nge4 dxe4 17. Nxe4 Kd7 | weight 3 | 32 | -97 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Qxg5 15. fxe6 O-O-O 16. exd7+ Rxd7 17. Rae1 Qd5 18. Qe3 | weight 7 | 15 | +175 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. O-O-O | weight 6 | 16 | +35 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. O-O-O | weight 6 | 16 | +35 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Ne5+ | weight 4 | 27 | +24 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Ne5+ | weight 4 | 27 | +24 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Qxg5 15. fxe6 O-O-O 16. exd7+ Rxd7 17. Rae1 Qd5 18. Qg4 | weight 4 | 30 | +160 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Rf1 dxe4 18. Rxf8+ Qxf8 | weight 10 | -27 | +122 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Rf1 dxe4 18. Rxf8+ Qxf8 | weight 10 | -27 | +122 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 | weight 10 | -22 | +23 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 | weight 10 | -22 | +23 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 h6 15. exd5 cxd5 16. Nge4 dxe4 17. Nxe4 Kd7 18. Nc5+ Kc8 | weight 10 | -16 | -94 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 h6 15. exd5 cxd5 16. Nge4 dxe4 17. Nxe4 Kd7 18. Nc5+ Kc8 | weight 10 | -16 | -94 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Qxg5 15. fxe6 O-O-O 16. exd7+ Rxd7 17. Rae1 f5 18. f4+ fxe4 | weight 10 | -11 | -53 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Qxg5 15. fxe6 O-O-O 16. exd7+ Rxd7 17. Rae1 f5 18. f4+ fxe4 | weight 10 | -10 | -53 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. Ne4 Rd8 | weight 10 | -8 | +34 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. Ne4 Rd8 | weight 10 | -8 | +34 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. h3 cxd5 18. Na5 Kc8 | weight 10 | -4 | -17 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nh5 16. Nc4 dxc4 17. Ne4 Nf4 18. Nd6+ Ke7 | weight 10 | -4 | -99 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. h3 cxd5 18. Na5 Kc8 | weight 10 | -4 | -17 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nh5 16. Nc4 dxc4 17. Ne4 Nf4 18. Nd6+ Ke7 | weight 10 | -4 | -99 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 16. exf6 hxg5 17. fxg7 Rg8 18. h4 g4 | weight 10 | -4 | -75 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 18. O-O Na6 | weight 10 | -3 | -12 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 18. O-O Na6 | weight 10 | -3 | -12 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 16. exf6 hxg5 17. fxg7 Rg8 18. h4 g4 | weight 10 | -2 | -77 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 | weight 10 | -1 | +2 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 | weight 10 | -1 | +2 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qg6 17. Nd2 Nd7 18. Nf3 Rxf4 | weight 10 | 0 | -15 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 18. O-O Nd7 | weight 10 | 0 | -15 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qg6 17. Nd2 Nd7 18. Nf3 Rxf4 | weight 10 | 0 | -15 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 18. O-O Nd7 | weight 10 | 0 | -15 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. dxc6 bxc6 18. h3 Nf6 | weight 10 | 1 | -1 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. dxc6 bxc6 18. h3 Nf6 | weight 10 | 1 | -1 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 | weight 10 | 2 | -52 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 h6 15. exd5 exd5 16. Nge4 dxe4 17. Nxe4 Kd7 18. Nc5+ Kc8 | weight 10 | 3 | -68 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 h6 15. exd5 exd5 16. Nge4 dxe4 17. Nxe4 Kd7 18. Nc5+ Kc8 | weight 10 | 3 | -68 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 16. g3 e5 17. Qxe5 O-O+ 18. Qxe7 Nxe7 | weight 10 | 4 | -88 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Qd1 Qf8 | weight 10 | 4 | +28 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. d5 c5 18. Nxc5+ Kc8 | weight 10 | 4 | +4 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 16. g3 e5 17. Qxe5 O-O+ 18. Qxe7 Nxe7 | weight 10 | 4 | -88 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Qd1 Qf8 | weight 10 | 4 | +28 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. d5 c5 18. Nxc5+ Kc8 | weight 10 | 4 | +4 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. dxe6 O-O-O 18. e7 Rfe8 | weight 10 | 5 | +84 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. dxe6 O-O-O 18. e7 Rfe8 | weight 10 | 5 | +84 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 | weight 10 | 6 | -50 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 Qxf7 15. f5 Qxf5 16. Qxf5 exf5 17. Rae1+ Kf7 18. Re6 Kxe6 | weight 9 | 8 | +54 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 16. exf6 hxg5 17. fxg7 Rg8 18. h4 gxh4 | weight 9 | 8 | -87 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 Qxf7 15. f5 Qxf5 16. Qxf5 exf5 17. Rae1+ Kf7 18. Re6 Kxe6 | weight 9 | 8 | +54 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 16. exf6 hxg5 17. fxg7 Rg8 18. h4 gxh4 | weight 9 | 8 | -87 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 h5 | weight 8 | 10 | -76 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Ne5+ Nxe5 | weight 8 | 10 | -34 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 h5 | weight 8 | 10 | -76 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Ne5+ Nxe5 | weight 8 | 10 | -34 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. Nxf7 Kxf7 10. exf4 Ng4 11. Qe2 h5 12. Nd2 Ne3+ 13. Qxe3 e5 14. Qxe5 Re8 15. Nf3 Kg8 16. Ng5 Rxe5+ 17. dxe5 Qb6 18. O-O Qb5 | weight 8 | 11 | -70 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 | weight 8 | 12 | -39 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 | weight 8 | 12 | -39 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f4 Nb4 13. Kd2 Ne4+ 14. Kc1 Nd3+ 15. Qxd3 Kg8 16. Qe2 Qf8 17. Qh5 Nxc3+ 18. Nxc3 Qb4 | weight 7 | 13 | -83 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Rf7 17. cxb4 Qf8 18. Nd2 Rf2 | weight 6 | 19 | +7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Rf1 dxe4 18. cxb4 cxd4 | weight 5 | 20 | +9 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Rf1 dxe4 18. cxb4 cxd4 | weight 5 | 20 | +9 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 b5 | weight 5 | 23 | -89 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 b5 | weight 5 | 23 | -89 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. fxg7 Nd7 16. gxf8=Q+ Nxf8 17. f4 Qh5 18. h4 Qg4 | weight 4 | 25 | -96 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 18. O-O Nf6 | weight 4 | 26 | -41 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 18. O-O Nf6 | weight 4 | 26 | -41 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Kd7 16. dxe6+ fxe6 17. Ne5+ Nxe5 18. dxe5 Kc7 | weight 3 | 33 | +7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qg4 14. Qxg4 Nxg4 15. Nc4 Kd7 16. dxe6+ fxe6 17. Ne5+ Nxe5 18. dxe5 Kc7 | weight 3 | 33 | +7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Rf1 dxe4 18. cxb4 cxb4 | weight 3 | 36 | -7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Rf1 dxe4 18. cxb4 cxb4 | weight 3 | 36 | -7 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. g3 | weight 10 | -83 | +60 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. g3 | weight 10 | -50 | +27 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 e4 15. Rxe4 dxe4 16. Qe3 f5 17. Qh3 g6 18. Qxh7 Qd6 19. Qxb7 | weight 10 | -13 | +196 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Bxf4 10. exf4 Qe7 11. Kd1 Ne4 12. Qe1 Nxf2+ 13. Qxf2 e5 14. Re1 e4 15. Rxe4 dxe4 16. Qe3 f5 17. Qh3 g6 18. Qxh7 Qd6 19. Qxb7 | weight 10 | -13 | +196 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. O-O-O | weight 10 | -12 | +10 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. O-O-O | weight 10 | -12 | +10 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 16. g3 e5 17. Qxe5 O-O+ 18. Qxe7 Nxe7 19. Rfe1 | weight 10 | 1 | +87 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 16. g3 e5 17. Qxe5 O-O+ 18. Qxe7 Nxe7 19. Rfe1 | weight 10 | 1 | +87 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Nd2 | weight 4 | 25 | -19 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Nd2 | weight 4 | 25 | -19 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Rf1 cxd4 18. Rxf8+ Qxf8 19. cxb4 Qxb4 | weight 10 | -88 | +427 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Rf1 cxd4 18. Rxf8+ Qxf8 19. cxb4 Qxb4 | weight 10 | -88 | +427 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 16. exf6 hxg5 17. fxg7 Rg8 18. h4 gxh4 19. Rxh4 Nd7 | weight 10 | -22 | -69 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. g3 Ne2+ | weight 10 | -22 | -38 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nh5 16. Nc4 dxc4 17. Ne4 Nf4 18. Nd6+ Ke7 19. Nxb7 Nxg2+ | weight 10 | -18 | -96 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 Nh5 16. Nc4 dxc4 17. Ne4 Nf4 18. Nd6+ Ke7 19. Nxb7 Nxg2+ | weight 10 | -18 | -96 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 h5 19. Rxg4 hxg4 | weight 10 | -7 | +44 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. dxe6 O-O-O 18. e7 Rfe8 19. exd8=Q+ Kxd8 | weight 10 | -7 | +96 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 h5 19. Rxg4 hxg4 | weight 10 | -7 | +44 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. dxe6 O-O-O 18. e7 Rfe8 19. exd8=Q+ Kxd8 | weight 10 | -7 | +96 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Qxg5 15. fxe6 O-O-O 16. exd7+ Rxd7 17. Rae1 f5 18. f4+ fxe4 19. fxg5 Re8 | weight 10 | -3 | -60 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 19. Nf3 Re4 | weight 10 | -3 | -39 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Qxg5 15. fxe6 O-O-O 16. exd7+ Rxd7 17. Rae1 f5 18. f4+ fxe4 19. fxg5 Re8 | weight 10 | -3 | -60 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 16. exf6 hxg5 17. fxg7 Rg8 18. h4 g4 19. h5 Rxg7 | weight 10 | -2 | -74 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 18. O-O Na6 19. Rfe1 Nxc5 | weight 10 | -2 | -10 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 18. O-O Na6 19. Rfe1 Nxc5 | weight 10 | -2 | -10 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. d5 c5 18. Nxc5+ Kc8 19. O-O Nd7 | weight 10 | -1 | -2 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. d5 c5 18. Nxc5+ Kc8 19. O-O Nd7 | weight 10 | -1 | -2 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 16. exf6 hxg5 17. fxg7 Rg8 18. h4 gxh4 19. Rxh4 Nd7 | weight 10 | 0 | -91 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 18. O-O Nf6 19. Rfe1 Nbd7 | weight 10 | 4 | -41 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 18. O-O Nf6 19. Rfe1 Nbd7 | weight 10 | 4 | -41 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Qd1 Qxg2 | weight 10 | 5 | +86 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. O-O-O Nf6 | weight 10 | 6 | -16 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. O-O-O Nf6 | weight 10 | 6 | -16 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Ne5+ Nxe5 19. dxe5 Kc7 | weight 9 | 7 | -29 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Ne5+ Nxe5 19. dxe5 Kc7 | weight 9 | 7 | -29 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. Rhe1 Nd7 | weight 9 | 8 | -17 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. Rhe1 Nd7 | weight 9 | 8 | -17 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 16. exf6 hxg5 17. fxg7 Rg8 18. h4 g4 19. h5 Rxg7 | weight 9 | 8 | -74 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 19. Nf3 Re4 | weight 9 | 9 | -38 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Qd1 Qf8 19. Nd2 Rxd2 | weight 8 | 10 | +21 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Qd1 Qf8 19. Nd2 Rxd2 | weight 8 | 10 | +21 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. dxc6 bxc6 18. h3 O-O-O 19. hxg4 f5 | weight 8 | 11 | -8 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. g3 Ne2+ | weight 8 | 11 | -38 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Na6 17. dxc6 bxc6 18. h3 O-O-O 19. hxg4 f5 | weight 8 | 11 | -8 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Qd1 Qxg2 | weight 7 | 15 | +86 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Nd2 Qxg2 | weight 5 | 20 | -1 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. Nf3 Nf6 | weight 5 | 20 | -10 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Nd2 Qxg2 | weight 5 | 20 | -1 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. Nf3 Nf6 | weight 5 | 20 | -10 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Ne5+ Nxe5 19. dxe5 Rf4 | weight 4 | 25 | -47 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Ne5+ Nxe5 19. dxe5 Rf4 | weight 4 | 25 | -47 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Rf1 dxe4 18. cxb4 cxb4 19. Rxf8+ Qxf8 | weight 3 | 32 | -38 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 c5 17. Rf1 dxe4 18. cxb4 cxb4 19. Rxf8+ Qxf8 | weight 3 | 32 | -38 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Qd1 Qxg2 20. Rxf2 | weight 10 | -39 | -47 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Qd1 Qxg2 20. Rxf2 | weight 10 | -29 | -57 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. O-O-O Nf6 20. Nf3 | weight 10 | 2 | +14 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. O-O-O Nf6 20. Nf3 | weight 10 | 2 | +14 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. Rhe1 Nd7 20. Nxe6 | weight 5 | 21 | -4 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. Rhe1 Nd7 20. Nxe6 | weight 5 | 21 | -4 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Qd1 Qxg2 20. Rxf2 Qxf2 | weight 10 | -23 | +70 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Qd1 Qxg2 20. Rxf2 Qxf2 | weight 10 | -13 | +70 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 Qxf7 15. f5 Qxf5 16. Qxf5 exf5 17. Rae1+ Kf7 18. Re6 Kxe6 19. Re1+ Kf7 20. Re2 Rhe8 | weight 10 | -9 | +50 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nxf7 Qxf7 15. f5 Qxf5 16. Qxf5 exf5 17. Rae1+ Kf7 18. Re6 Kxe6 19. Re1+ Kf7 20. Re2 Rhe8 | weight 10 | -9 | +50 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 19. Nf3 Re4 20. Ng5 Nd7 | weight 10 | -7 | -42 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 19. Nf3 Re4 20. Ng5 Nd7 | weight 10 | -7 | -42 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Qd1 Qf8 19. Nd2 Rxd2 20. Qg4 Qf7 | weight 10 | -6 | +24 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 18. O-O Nf6 19. Rfe1 Nbd7 20. Re7 Nxc5 | weight 10 | -6 | -24 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 18. O-O Nf6 19. Rfe1 Nbd7 20. Re7 Nxc5 | weight 10 | -6 | -24 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. d5 c5 18. Nxc5+ Kc8 19. O-O Nd7 20. Ne4 Kb8 | weight 10 | -4 | -6 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. d5 c5 18. Nxc5+ Kc8 19. O-O Nd7 20. Ne4 Kb8 | weight 10 | -4 | -6 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 16. exf6 hxg5 17. fxg7 Rg8 18. h4 g4 19. h5 Rxg7 20. h6 Rh7 | weight 10 | -3 | -62 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. h3 Nf6 15. e5 h6 16. exf6 hxg5 17. fxg7 Rg8 18. h4 g4 19. h5 Rxg7 20. h6 Rh7 | weight 10 | -3 | -62 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 18. O-O Na6 19. Rfe1 Nxc5 20. Re7 Nd3 | weight 10 | -2 | +48 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Qd1 Qf8 19. Nd2 Rxd2 20. Qg4 Qf7 | weight 10 | -2 | +24 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qg4 13. Qxg4 Nxg4 14. exd5 exd5 15. Nde4 dxe4 16. Nxe4 Kd7 17. Nc5+ Kc8 18. O-O Na6 19. Rfe1 Nxc5 20. Re7 Nd3 | weight 10 | -2 | +48 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. Rhe1 Na6 20. Ne4 Rd8 | weight 10 | 0 | +9 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. Rhe1 Na6 20. Ne4 Rd8 | weight 10 | 0 | +9 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. O-O-O Nf6 20. Rhg1 Kf8 | weight 10 | 2 | -16 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 16. g3 e5 17. Qxe5 O-O+ 18. Qxe7 Nxe7 19. Nh4 g5 20. Rfe1 gxh4 | weight 10 | 5 | -88 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 16. g3 e5 17. Qxe5 O-O+ 18. Qxe7 Nxe7 19. Nh4 g5 20. Rfe1 gxh4 | weight 10 | 5 | -88 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. O-O-O Nf6 20. Rhg1 Kf8 | weight 10 | 6 | -20 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Qd1 Qxg2 20. Nd2 Re2 | weight 9 | 7 | +87 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Rf1 Qxh2 19. Qd1 Qxg2 20. Nd2 Re2 | weight 9 | 7 | +87 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. Nf3 Nf6 20. O-O-O Kf8 | weight 9 | 8 | -22 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. Nf3 Nf6 20. O-O-O Kf8 | weight 9 | 8 | -22 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Qxg5 15. fxe6 O-O-O 16. exd7+ Rxd7 17. Rae1 f5 18. f4+ fxe4 19. fxg5 Re8 20. g6 Rd6 | weight 9 | 9 | -44 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Ne5+ Nxe5 19. dxe5 Rf4 20. O-O-O+ Kc7 | weight 9 | 9 | -64 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Qxg5 15. fxe6 O-O-O 16. exd7+ Rxd7 17. Rae1 f5 18. f4+ fxe4 19. fxg5 Re8 20. g6 Rd6 | weight 9 | 9 | -44 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Ne5+ Nxe5 19. dxe5 Rf4 20. O-O-O+ Kc7 | weight 9 | 9 | -64 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. Nf3 Nf6 20. O-O-O Ne4 | weight 7 | 13 | -27 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. Nf3 Nf6 20. O-O-O Ne4 | weight 7 | 13 | -27 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 19. Nf3 Re4 20. Ng5 Rxe3+ | weight 6 | 18 | -67 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 19. Nf3 Re4 20. Ng5 Rxe3+ | weight 6 | 18 | -67 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. Rhe1 Nd7 20. Ne4 Kb8 | weight 5 | 22 | +16 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. Rhe1 Nd7 20. Ne4 Kb8 | weight 5 | 22 | +16 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Rxf2 Rxf2 | weight 5 | 23 | -80 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Rxf2 Rxf2 | weight 5 | 23 | -80 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Ne5+ Nxe5 19. dxe5 Kc7 20. O-O-O Nd7 | weight 4 | 28 | -53 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Ne5+ Nxe5 19. dxe5 Kc7 20. O-O-O Nd7 | weight 4 | 28 | -53 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. Nf3 Nf6 20. O-O-O Kd7 | weight 3 | 38 | -52 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. Nf3 Nf6 20. O-O-O Kd7 | weight 3 | 38 | -52 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 h5 19. Rxg4 hxg4 20. Ne5+ Ke8 21. Nc4 Kd7 | weight 10 | -178 | +50 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 h5 19. Rxg4 hxg4 20. Ne5+ Ke8 21. Nc4 Kd7 | weight 10 | -156 | +50 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Rxf2 Rxf2 21. Re1 Rf6 | weight 10 | -70 | -22 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Rxf2 Rxf2 21. Re1 Rf6 | weight 10 | -50 | -22 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 h5 19. Rxg4 hxg4 20. Ne5+ Kd8 21. Nc4 Kd7 | weight 10 | -13 | +50 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Nc4 Kd7 17. dxe6+ fxe6 18. Rg1 h5 19. Rxg4 hxg4 20. Ne5+ Kd8 21. Nc4 Kd7 | weight 10 | -13 | +50 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 19. Nf3 Re4 20. Ng5 Nd7 21. Nxe4 dxe4 | weight 10 | -8 | -26 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 19. Nf3 Re4 20. Ng5 Nd7 21. Nxe4 dxe4 | weight 10 | -8 | -26 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Qd1 Qf8 19. Nd2 Rxd2 20. Qg4 Rc2+ 21. Kxc2 Qf2+ | weight 10 | -2 | -98 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Qxg5 15. fxe6 O-O-O 16. exd7+ Rxd7 17. Rae1 f5 18. f4+ fxe4 19. fxg5 Re8 20. g6 Rd6 21. Rf7 Rxg6 | weight 10 | -1 | -23 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Na6 12. f3 Nb4 13. Kd2 Ne4+ 14. fxe4+ Kg8 15. Kc1 Rf8 16. Qe2 Nd3+ 17. Qxd3 Rf2 18. Qd1 Qf8 19. Nd2 Rxd2 20. Qg4 Rc2+ 21. Kxc2 Qf2+ | weight 10 | -1 | -98 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 16. g3 e5 17. Qxe5 O-O+ 18. Qxe7 Nxe7 19. Nh4 g5 20. Rfe1 gxh4 21. Rxe7 Rfe8 | weight 10 | 0 | -88 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 16. g3 e5 17. Qxe5 O-O+ 18. Qxe7 Nxe7 19. Nh4 g5 20. Rfe1 gxh4 21. Rxe7 Rfe8 | weight 10 | 0 | -88 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. f5 Qxg5 15. fxe6 O-O-O 16. exd7+ Rxd7 17. Rae1 f5 18. f4+ fxe4 19. fxg5 Re8 20. g6 Rd6 21. Rf7 Rxg6 | weight 10 | 3 | -27 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. Nf3 Nf6 20. O-O-O Kd7 21. Ne5+ Rxe5 | weight 10 | 3 | -51 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. Nf3 Nf6 20. O-O-O Kd7 21. Ne5+ Rxe5 | weight 10 | 3 | -51 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 19. Nf3 Re4 20. Ng5 Rxe3+ 21. Kd2 Nd7 | weight 8 | 10 | -92 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 19. Nf3 Re4 20. Ng5 Rxe3+ 21. Kd2 Nd7 | weight 8 | 10 | -92 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Rxf2 Rxf2 21. Ne4 Kd7 | weight 7 | 13 | -84 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Rxf2 Rxf2 21. Ne4 Kd7 | weight 7 | 13 | -84 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. Rhe1 Nd7 20. Ne4 Kb8 21. Nd6 Nb6 | weight 4 | 27 | +11 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. Rhe1 Nd7 20. Ne4 Kb8 21. Nd6 Nb6 | weight 3 | 36 | +11 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. Nf3 Nf6 20. O-O-O Kd7 21. Ne5+ Rxe5 22. fxe5 Ne4 | weight 10 | 0 | -53 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. Qh3 Qf7 15. gxf6 Qxf6 16. f4 Qf5 17. Qxf5 Rxf5 18. Nd2 Nd7 19. Nf3 Nf6 20. O-O-O Kd7 21. Ne5+ Rxe5 22. fxe5 Ne4 | weight 10 | 0 | -53 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 16. g3 e5 17. Qxe5 O-O+ 18. Qxe7 Nxe7 19. Nh4 g5 20. Rfe1 gxh4 21. Rxe7 Rfe8 22. Rae1 Rxe7 | weight 9 | 7 | -90 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Bxf4 9. exf4 Qe7 10. Nd2 Ne4 11. Ndxe4 dxe4 12. Qxe4 Nd7 13. O-O h6 14. Nf3 Nf6 15. Qe2 Nd5 16. g3 e5 17. Qxe5 O-O+ 18. Qxe7 Nxe7 19. Nh4 g5 20. Rfe1 gxh4 21. Rxe7 Rfe8 22. Rae1 Rxe7 | weight 9 | 7 | -90 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 19. Nf3 Re4 20. Ng5 Nd7 21. Nxe4 dxe4 22. O-O-O Nf6 | weight 7 | 15 | -43 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Qe7 9. Bxd6 Qxd6 10. Nxf7 Kxf7 11. Qf1 Ke8 12. g4 Qc7 13. g5 Rf8 14. gxf6 Qf7 15. Nd2 Qxf6 16. f4 Qh4+ 17. Qf2 Rxf4+ 18. Qxh4 Rxh4 19. Nf3 Re4 20. Ng5 Nd7 21. Nxe4 dxe4 22. O-O-O Nf6 | weight 7 | 15 | -43 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. Rhe1 Nd7 20. g3 Nxc5 21. dxc5 Nd5 22. Rxe6 Kc7 23. Rxd5 cxd5 | weight 6 | 19 | -54 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qh5 14. Qxh5 Nxh5 15. Ne4 Kd7 16. dxe6+ fxe6 17. Nc5+ Kc8 18. O-O-O Nf4 19. Rhe1 Nd7 20. g3 Nxc5 21. dxc5 Nd5 22. Rxe6 Kc7 23. Rxd5 cxd5 | weight 6 | 19 | -54 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Rxf2 Rxf2 21. Ne4 Kd7 22. Nc5+ Kc8 23. Ne4 Kd7 | weight 5 | 24 | -78 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Rxf2 Rxf2 21. Ne4 Kd7 22. Nc5+ Kc8 23. Ne4 Kd7 | weight 5 | 24 | -78 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Rxf2 Rxf2 21. Re1 Rf6 22. Ne4 Kd7 23. Nxf6+ gxf6 24. Rg1 Na6 25. Rg7+ Kd6 26. c4 c5 27. Rf7 Rg8 | weight 7 | 13 | -32 |  |
| 1. Nf3 Nf6 2. d4 d5 3. c3 c6 4. Bf4 Bf5 5. e3 e6 6. Bd3 Bxd3 7. Qxd3 Bd6 8. Ng5 Rf8 9. Qe2 Qe7 10. Bxd6 Qxd6 11. e4 Qf4 12. Nd2 Qxg5 13. exd5 Qxg2 14. Rf1 Qg4 15. Qxg4 Nxg4 16. Ne4 Kd7 17. dxe6+ fxe6 18. Nc5+ Kc8 19. O-O-O Nxf2 20. Rxf2 Rxf2 21. Re1 Rf6 22. Ne4 Kd7 23. Nxf6+ gxf6 24. Rg1 Na6 25. Rg7+ Kd6 26. c4 c5 27. Rf7 Rg8 | weight 7 | 13 | -32 |  |

## Not applied: demotions in lines up to 8 actions deep (5), where the guides disagree with the engine

| line and move | | loss | after | opening |
|---|---|---|---|---|
| 1. Nf3 Nf6 2. d4 c5 3. dxc5 Qa5+ | weight 10 now | 80 | -23 | Sarp's Counterattack: 3. dxc5 Queen's Line |
| 1. Nf3 Nf6 2. e3 Ng4 3. Ng5 f5 4. Qxg4 | weight 10 now | 111 | -21 |  |
| 1. e3 Nf6 2. Bb5 c6 3. c3 cxb5 4. Qb3 Qa5 | weight 5 now | 92 | -29 |  |
| 1. e3 Nf6 2. Bb5 c6 3. c3 cxb5 4. Qb3 Qb6 | weight 10 now | 78 | -15 |  |
| 1. Nf3 Nf6 2. d4 c5 3. c3 Qb6 4. e3 Nc6 5. Bc4 | weight 3 now | 66 | -39 | Sarp's Counterattack: Mainline: 4. e3 Nc6 |
