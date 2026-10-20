# Book evaluation report

Every book position (and every position a book action reaches) searched with 3,000,000 nodes,
one thread, fresh table (`ktools bookeval`), data in `docs/theory/book_evals.tsv` (+ `.edges.tsv`,
`.positions.tsv`). Scores are centipawns from the side to move's view; mate scores are +-(10000-10*n).

## Search depth reached

| statistic | value |
|---|---|
| positions | 4027 |
| depth min / median / mean / max | 1 / 13 / 12.2 / 17 |
| share reaching depth >= 10 | 89.7% |
| share reaching depth >= 11 | 80.6% |
| share reaching depth >= 12 | 69.1% |
| share reaching depth >= 13 | 53.1% |

## Action loss against the parent's best (cp the side to move gives up by playing it)

Loss = parent score - (minus the child score). Noise of two independent 3M-node searches is roughly +-20 cp.

| tag | better than the parent's best (< -25) | -25..25 | 26..50 | 51..100 | 101..200 | > 200 | total |
|---|---|---|---|---|---|---|---|
| play | 119 | 1218 | 209 | 157 | 67 | 41 | 1811 |
| know | 76 | 1089 | 288 | 301 | 213 | 261 | 2228 |

## Played actions by weight

| weight | n | median loss | mean loss | share loss > 50 |
|---|---|---|---|---|
| 2 | 8 | 24 | 21 | 0% |
| 3 | 84 | 14 | 21 | 12% |
| 5 | 39 | 11 | 15 | 3% |
| 10 | 1680 | 5 | 20 | 15% |

## Played actions that lose more than 50 cp against the parent's best (candidates to demote to known)

| line (parent) and action | tag | score after (mover) | parent score | loss | search best at parent | child depth |
|---|---|---|---|---|---|---|
| ... (0T15)dxe5 (0T15)Ne4 (0T16)Q>>xh7~ (>L1) (1T11)Qxf7 (1T12)Qxh8+ (1T12)Kd7 **(1T13)Qa6** | play 10 | -279 | 9970 | 1779 | (1T13)d3b5 | 11 |
| ... (0T15)dxe5 (0T15)Ne4 (0T16)Q>>xh7~ (>L1) (1T11)Qxf7 (1T12)Qxh8+ (1T12)Kd7 **(1T13)Qa6** | play 10 | -279 | 9970 | 1779 | (1T13)d3b5 | 11 |
| ... (0T21)Kd7 (0T22)Ne5+ (0T22)Ke8 (0T23)f4 (0T23)Rf5 (0T24)Ng6 (0T24)Rh5 **(0T25)h4** | play 10 | 42 | 9970 | 1458 | (0T25)g6(0T24)g8 | 13 |
| ... (0T21)Kd7 (0T22)Ne5+ (0T22)Ke8 (0T23)f4 (0T23)Rf5 (0T24)Ng6 (0T24)Rh5 **(0T25)h4** | play 10 | 48 | 9970 | 1452 | (0T25)g6(0T24)g8 | 13 |
| ... (0T14)Kg8 (0T15)Kc1 (0T15)Rf8 (0T16)Qe2 (0T16)c5 (0T17)Qb5 (0T17)a6 **(0T18)Qxb7** | play 10 | -1500 | -192 | 1308 | (0T18)b5(0T9)b5 | 11 |
| ... (0T14)Kg8 (0T15)Kc1 (0T15)Rf8 (0T16)Qe2 (0T16)c5 (0T17)Qb5 (0T17)a6 **(0T18)Qxb7** | play 10 | -1500 | -192 | 1308 | (0T18)b5(0T9)b5 | 11 |
| ... (0T16)Kd7 (0T17)dxe6+ (0T17)fxe6 (0T18)hxg4 (0T18)b6 (0T19)Rh7 (0T19)Kc7 **(0T20)Nxg5** | play 10 | 258 | 9970 | 1242 | (0T20)e4(0T18)e5 | 12 |
| ... (0T16)Kd7 (0T17)dxe6+ (0T17)fxe6 (0T18)hxg4 (0T18)b6 (0T19)Rh7 (0T19)Kc7 **(0T20)Nxg5** | play 10 | 258 | 9970 | 1242 | (0T20)e4(0T18)e5 | 12 |
| ... (0T11)e5 (0T12)Qe1 (0T12)e4 (0T13)Nxe4 (0T13)Qe6 (0T14)Nd6+ (0T14)Ke7 **(0T15)Qxe6+** | play 10 | 270 | 9950 | 1230 | (0T15)e1(0T10)e6 | 13 |
| ... (0T11)e5 (0T12)Qe1 (0T12)e4 (0T13)Nxe4 (0T13)Qe6 (0T14)Nd6+ (0T14)Ke7 **(0T15)Qxe6+** | play 10 | 270 | 9950 | 1230 | (0T15)e1(0T10)e6 | 13 |
| ... (0T15)dxe4 (0T16)Nxe4 (0T16)Kd7 (0T17)d5 (0T17)Na6 (0T18)dxc6+ (0T18)Kc7 **(0T19)Nd6+** | play 10 | 276 | 9960 | 1224 | (0T19)e4(0T17)e5 | 10 |
| ... (0T15)dxe4 (0T16)Nxe4 (0T16)Kd7 (0T17)d5 (0T17)Na6 (0T18)dxc6+ (0T18)Kc7 **(0T19)Nd6+** | play 10 | 276 | 9960 | 1224 | (0T19)e4(0T17)e5 | 10 |
| ... (0T10)Qe7 (0T11)Kd1 (0T11)e5 (0T12)Qe1 (0T12)e4 (0T13)Nxe4 (0T13)Qe6 **(0T14)Nd6+** | play 10 | 342 | 9940 | 1158 | (0T14)e4d6 | 9 |
| ... (0T10)Qe7 (0T11)Kd1 (0T11)e5 (0T12)Qe1 (0T12)e4 (0T13)Nxe4 (0T13)Qe6 **(0T14)Nd6+** | play 10 | 342 | 9940 | 1158 | (0T14)e4d6 | 9 |
| ... (0T15)Rf8 (0T16)Qe2 (0T16)c5 (0T17)Qb5 (0T17)a6 (0T18)Qxb7 (0T18)Nd3+ **(0T19)Kc2** | play 10 | -1500 | -371 | 1129 | (0T19)c1c2 | 13 |
| ... (0T15)Rf8 (0T16)Qe2 (0T16)c5 (0T17)Qb5 (0T17)a6 (0T18)Qxb7 (0T18)Nd3+ **(0T19)Kc2** | play 10 | -1500 | -371 | 1129 | (0T19)c1c2 | 13 |
| ... (0T13)Qe2 (0T13)Nbd7 (0T14)O-O (0T14)e5 (0T15)Q>>xe6+~ (>L1) (1T11)Qe7 **(1T12)Qh5+** | play 10 | 417 | 9960 | 1083 | (1T12)e6c8 | 10 |
| ... (0T13)Qe2 (0T13)Nbd7 (0T14)O-O (0T14)e5 (0T15)Q>>xe6+~ (>L1) (1T11)Qe7 **(1T12)Qh5+** | play 10 | 417 | 9960 | 1083 | (1T12)e6c8 | 10 |
| ... (0T9)Qxd6 (0T10)Nxf7 (0T10)Qe7 (0T11)Nxh8 (0T11)Ng4 (0T12)Qxh7 (0T12)Nxe3+ **(0T13)fxe3** | play 10 | 427 | 9940 | 1073 | (0T13)f2e3 | 8 |
| ... (0T9)Qxd6 (0T10)Nxf7 (0T10)Qe7 (0T11)Nxh8 (0T11)Ng4 (0T12)Qxh7 (0T12)Nxe3+ **(0T13)fxe3** | play 10 | 427 | 9940 | 1073 | (0T13)f2e3 | 8 |
| ... (0T9)Qe7 (0T10)Nd2 (0T10)h6 (0T11)Qe2 (0T11)hxg5 (0T12)Ne4 (0T12)Qd8 **(0T13)Nxg5** | play 10 | -57 | 417 | 474 | (0T13)e2(0T9)e6 | 13 |
| ... (0T9)Qe7 (0T10)Nd2 (0T10)h6 (0T11)Qe2 (0T11)hxg5 (0T12)Ne4 (0T12)Qd8 **(0T13)Nxg5** | play 10 | -57 | 417 | 474 | (0T13)e2(0T9)e6 | 13 |
| ... (0T9)Qe7 (0T10)Nd2 (0T10)Nbd7 (0T11)Qe2 (0T11)e5 (0T12)Ne6+ (0T12)Qxe6 **(0T13)Q>>xe6+~ (>L1)** | play 10 | 83 | 417 | 334 | (0T13)e2(0T9)e6 | 7 |
| ... (0T9)Qe7 (0T10)Nd2 (0T10)Nbd7 (0T11)Qe2 (0T11)e5 (0T12)Ne6+ (0T12)Qxe6 **(0T13)Q>>xe6+~ (>L1)** | play 10 | 83 | 417 | 334 | (0T13)e2(0T9)e6 | 7 |
| ... (0T12)Nd7 (0T13)O-O (0T13)h6 (0T14)Nf3 (0T14)Nf6 (0T15)Qe2 (0T15)Ng4 **(0T16)h3** | play 10 | -203 | 107 | 310 | (0T16)d4d5 | 12 |
| ... (0T12)Nd7 (0T13)O-O (0T13)h6 (0T14)Nf3 (0T14)Nf6 (0T15)Qe2 (0T15)Ng4 **(0T16)h3** | play 10 | -203 | 94 | 297 | (0T16)d4d5 | 12 |
| ... (0T11)Ne4 (0T12)Qe1 (0T12)Nxf2+ (0T13)Qxf2 (0T13)Nd7 (0T14)Qe1 (0T14)Qf6 **(0T15)Nxe6+** | play 10 | 101 | 377 | 276 | (0T15)g5e6 | 10 |
| ... (0T11)Ne4 (0T12)Qe1 (0T12)Nxf2+ (0T13)Qxf2 (0T13)Nd7 (0T14)Qe1 (0T14)Qf6 **(0T15)Nxe6+** | play 10 | 101 | 375 | 274 | (0T15)g5e6 | 10 |
| ... (0T19)Nxf2 (0T20)Rxf2 (0T20)Rxf2 (0T21)Re1 (0T21)Rf6 (0T22)Ne4 (0T22)Kd7 **(0T23)Nxf6+** | play 10 | 175 | 437 | 262 | (0T23)e1g1 | 11 |
| ... (0T19)Nxf2 (0T20)Rxf2 (0T20)Rxf2 (0T21)Re1 (0T21)Rf6 (0T22)Ne4 (0T22)Kd7 **(0T23)Nxf6+** | play 10 | 175 | 437 | 262 | (0T23)e1g1 | 11 |
| ... (0T22)Kd7 (0T23)Nxf6+ (0T23)gxf6 (0T24)Rg1 (0T24)Na6 (0T25)Rg7+ (0T25)Kd6 **(0T26)c4** | play 10 | 29 | 278 | 249 | (0T26)g7(0T23)g7 | 12 |
| ... (0T22)Kd7 (0T23)Nxf6+ (0T23)gxf6 (0T24)Rg1 (0T24)Na6 (0T25)Rg7+ (0T25)Kd6 **(0T26)c4** | play 10 | 29 | 278 | 249 | (0T26)g7(0T23)g7 | 12 |
| ... (0T9)Qxd6 (0T10)Nxf7 (0T10)Qe7 (0T11)Nxh8 (0T11)g6 (0T12)Nxg6 (0T12)Qg7 **(0T13)Nf4** | play 10 | 143 | 387 | 244 | (0T13)d3f5 | 12 |
| ... (0T9)Qxd6 (0T10)Nxf7 (0T10)Qe7 (0T11)Nxh8 (0T11)g6 (0T12)Nxg6 (0T12)Qg7 **(0T13)Nf4** | play 10 | 146 | 387 | 241 | (0T13)d3f5 | 13 |
| ... (0T18)Kc8 (0T19)O-O-O (0T19)Nxf2 (0T20)Rxf2 (0T20)Rxf2 (0T21)Nxe6 (0T21)Re2 **(0T22)Rf1** | play 10 | -386 | -154 | 232 | (0T22)d1g1 | 8 |
| ... (0T18)Kc8 (0T19)O-O-O (0T19)Nxf2 (0T20)Rxf2 (0T20)Rxf2 (0T21)Nxe6 (0T21)Re2 **(0T22)Rf1** | play 10 | -386 | -154 | 232 | (0T22)d1g1 | 8 |
| ... (0T17)Rf2 (0T18)Rf1 (0T18)Qxh2 (0T19)Qd1 (0T19)Qxg2 (0T20)Rxf2 (0T20)Qxf2 **(0T21)Nd2** | play 10 | -314 | -85 | 229 | (0T21)b1d2 | 12 |
| ... (0T17)Rf2 (0T18)Rf1 (0T18)Qxh2 (0T19)Qd1 (0T19)Qxg2 (0T20)Rxf2 (0T20)Qxf2 **(0T21)Nd2** | play 10 | -314 | -85 | 229 | (0T21)b1d2 | 12 |
| ... (0T11)Qf4 (0T12)Nd2 (0T12)Qxg5 (0T13)exd5 (0T13)Qxg2 (0T14)Rf1 (0T14)Nxd5 **(0T15)Ne4** | play 10 | -123 | 98 | 221 | (0T15)d2c4 | 15 |
| ... (0T16)Kd7 (0T17)dxe6+ (0T17)fxe6 (0T18)Nc5+ (0T18)Kc8 (0T19)O-O-O (0T19)Nxf2 **(0T20)Ne4** | play 10 | -77 | 135 | 212 | (0T20)c5e6 | 16 |
| ... (0T16)Kd7 (0T17)dxe6+ (0T17)fxe6 (0T18)Nc5+ (0T18)Kc8 (0T19)O-O-O (0T19)Nxf2 **(0T20)Ne4** | play 10 | -77 | 135 | 212 | (0T20)c5e6 | 16 |
| ... (0T13)Qe2 (0T13)Nbd7 (0T14)O-O (0T14)e5 (0T15)Q>>xe6+~ (>L1) (1T11)Be7 **(1T12)Qh5+** | play 10 | 233 | 417 | 184 | (1T12)e2h5 | 7 |
| ... (0T13)Qe2 (0T13)Nbd7 (0T14)O-O (0T14)e5 (0T15)Q>>xe6+~ (>L1) (1T11)Be7 **(1T12)Qh5+** | play 10 | 233 | 417 | 184 | (1T12)e2h5 | 7 |
| ... (0T4)Nc6 (0T5)d5 (0T5)Nb4 (0T6)d6 (0T6)e6 (0T7)g4 (0T7)Nxg4 **(0T8)Qa4** | play 10 | -203 | -30 | 173 | (0T8)b1d2 | 12 |
| ... (0T8)Bxf4 (0T9)exf4 (0T9)Qe7 (0T10)Nd2 (0T10)h6 (0T11)Qe2 (0T11)hxg5 **(0T12)Ne4** | play 10 | -190 | -18 | 172 | (0T12)e1g1 | 13 |
| ... (0T8)Bxf4 (0T9)exf4 (0T9)Qe7 (0T10)Nd2 (0T10)h6 (0T11)Qe2 (0T11)hxg5 **(0T12)Ne4** | play 10 | -190 | -18 | 172 | (0T12)e1g1 | 13 |
| ... (0T18)Nxe5 (0T19)dxe5 (0T19)Rf4 (0T20)O-O-O+ (0T20)Kc7 (0T21)Rd6 (0T21)Re4 **(0T22)Rxe6** | play 10 | -81 | 86 | 167 | (0T22)d6(0T11)d6 | 13 |
| ... (0T18)Nxe5 (0T19)dxe5 (0T19)Rf4 (0T20)O-O-O+ (0T20)Kc7 (0T21)Rd6 (0T21)Re4 **(0T22)Rxe6** | play 10 | -81 | 86 | 167 | (0T22)d6(0T11)d6 | 13 |
| ... (0T10)Ng4 (0T11)Qe2 (0T11)h5 (0T12)Nd2 (0T12)Ne3+ (0T13)Qxe3 (0T13)e5 **(0T14)Qxe5** | play 10 | 87 | 251 | 164 | (0T14)e3(0T11)e6 | 15 |
| ... (0T10)Kxf7 (0T11)Qf1 (0T11)Ke8 (0T12)g4 (0T12)Qc7 (0T13)Nd2 (0T13)Rf8 **(0T14)Nf3** | play 10 | -34 | 130 | 164 | (0T14)f1g1 | 12 |
| ... (0T10)Ng4 (0T11)Qe2 (0T11)h5 (0T12)Nd2 (0T12)Ne3+ (0T13)Qxe3 (0T13)e5 **(0T14)Qxe5** | play 10 | 87 | 251 | 164 | (0T14)e3(0T11)e6 | 15 |
| ... (0T11)Ng4 (0T12)Qxg4 (0T12)Nd7 (0T13)Nxe6+ (0T13)fxe6 (0T14)Re1 (0T14)Nf6 **(0T15)Qh3** | play 10 | -16 | 146 | 162 | (0T15)g4h4 | 13 |
| ... (0T11)Ng4 (0T12)Qxg4 (0T12)Nd7 (0T13)Nxe6+ (0T13)fxe6 (0T14)Re1 (0T14)Nf6 **(0T15)Qh3** | play 10 | -16 | 146 | 162 | (0T15)g4h4 | 13 |
| ... (0T10)Ne4 (0T11)Ndxe4 (0T11)dxe4 (0T12)Qxe4 (0T12)h6 (0T13)Qe5 (0T13)Nd7 **(0T14)Qe2** | play 10 | -143 | 17 | 160 | (0T14)e5g7 | 13 |
| ... (0T10)Ne4 (0T11)Ndxe4 (0T11)dxe4 (0T12)Qxe4 (0T12)h6 (0T13)Qe5 (0T13)Nd7 **(0T14)Qe2** | play 10 | -143 | 17 | 160 | (0T14)e5g7 | 13 |
| ... (0T10)Qf8 (0T11)Ne5 (0T11)Kd8 (0T12)Qd1 (0T12)g6 (0T13)Qa4 (0T13)b6 **(0T14)Qc2** | play 10 | 39 | 196 | 157 | (0T14)e5(0T12)e6 | 11 |
| ... (0T14)Re8 (0T15)Nf3 (0T15)Kg8 (0T16)Ng5 (0T16)Rxe5+ (0T17)dxe5 (0T17)Qb6 **(0T18)O-O** | play 10 | 37 | 194 | 157 | (0T18)g5(0T16)g6 | 12 |
| ... (0T10)Kxf7 (0T11)Qf1 (0T11)Ke8 (0T12)g4 (0T12)Qc7 (0T13)Nd2 (0T13)Rf8 **(0T14)Nf3** | play 10 | -34 | 122 | 156 | (0T14)f1g1 | 12 |
| ... (0T11)Qf4 (0T12)Nd2 (0T12)Qxg5 (0T13)exd5 (0T13)Qxg2 (0T14)Rf1 (0T14)Nxd5 **(0T15)Ne4** | play 10 | -123 | 33 | 156 | (0T15)d2c4 | 15 |
| ... (0T8)Rf8 (0T9)Qe2 (0T9)h6 (0T10)Nxe6+ (0T10)fxe6 (0T11)Qf3 (0T11)Qe7 **(0T12)Bxd6** | play 10 | -19 | 128 | 147 | (0T12)f3g3 | 13 |
| ... (0T10)Qe7 (0T11)Kd1 (0T11)Ne4 (0T12)Qe1 (0T12)Nxf2+ (0T13)Qxf2 (0T13)h6 **(0T14)Nxe6+** | play 10 | 5 | 148 | 143 | (0T14)f2e2 | 15 |
| ... (0T3)Qb6 (0T4)e3 (0T4)Nc6 (0T5)d5 (0T5)Nb4 (0T6)g4 (0T6)Nxg4 **(0T7)Qa4** | play 10 | -131 | 11 | 142 | (0T7)b1a3 | 12 |
| ... (0T7)Bd6 (0T8)Ng5 (0T8)Bxf4 (0T9)exf4 (0T9)Qc7 (0T10)Nd2 (0T10)Qxf4 **(0T11)Qe2** | play 10 | -22 | 118 | 140 | (0T11)g5f7 | 10 |
| ... (0T5)e6 (0T6)Nxd8+ (0T6)Kxd8 (0T7)a3 (0T7)Na6 (0T8)f4 (0T8)Nc5 **(0T9)g3** | play 10 | -12 | 127 | 139 | (0T9)b1c3 | 8 |
| ... (0T10)Qe7 (0T11)Kd1 (0T11)Ne4 (0T12)Qe1 (0T12)Nxf2+ (0T13)Qxf2 (0T13)h6 **(0T14)Nxe6+** | play 10 | 9 | 148 | 139 | (0T14)f2e2 | 15 |
| ... (0T7)Bd6 (0T8)Ng5 (0T8)Bxf4 (0T9)exf4 (0T9)Qc7 (0T10)Nd2 (0T10)Qxf4 **(0T11)Qe2** | play 10 | -26 | 113 | 139 | (0T11)g5f7 | 10 |
| ... (0T8)Rf8 (0T9)Qe2 (0T9)Bxf4 (0T10)exf4 (0T10)Qe7 (0T11)O-O (0T11)Nh5 **(0T12)Qe1** | play 10 | -12 | 126 | 138 | (0T12)e2h5 | 11 |
| ... (0T8)Rf8 (0T9)Qe2 (0T9)Bxf4 (0T10)exf4 (0T10)Qe7 (0T11)O-O (0T11)Nh5 **(0T12)Qe1** | play 10 | -12 | 126 | 138 | (0T12)e2h5 | 11 |
| ... (0T13)Qf7 (0T14)Nh8 (0T14)Qh5 (0T15)Nf7 (0T15)Qg4 (0T16)Nd6+ (0T16)Ke7 **(0T17)Nxb7** | play 10 | 265 | 402 | 137 | (0T17)d6(0T15)e6 | 11 |
| ... (0T20)Kd8 (0T21)Nc4 (0T21)Kd7 (0T22)Ne5+ (0T22)Ke8 (0T23)f4 (0T23)Rf5 **(0T24)Ng6** | play 10 | -35 | 102 | 137 | (0T24)e5(0T22)e6 | 11 |
| ... (0T20)Kd8 (0T21)Nc4 (0T21)Kd7 (0T22)Ne5+ (0T22)Ke8 (0T23)f4 (0T23)Rf5 **(0T24)Ng6** | play 10 | -35 | 102 | 137 | (0T24)e5(0T22)e6 | 11 |
| ... (0T15)Kg8 (0T16)Ng5 (0T16)Rxe5+ (0T17)dxe5 (0T17)Qb6 (0T18)O-O (0T18)Qb5 **(0T19)Rfe1** | play 10 | -65 | 70 | 135 | (0T19)a1d1 | 10 |
| ... (0T9)Qxd6 (0T10)Nxf7 (0T10)Kxf7 (0T11)Qf1 (0T11)Kg8 (0T12)g4 (0T12)Qc7 **(0T13)g5** | play 10 | -37 | 96 | 133 | (0T13)f1g2 | 10 |
| ... (0T9)Qxd6 (0T10)Nxf7 (0T10)Kxf7 (0T11)Qf1 (0T11)Kg8 (0T12)g4 (0T12)Qc7 **(0T13)g5** | play 10 | -37 | 95 | 132 | (0T13)f1g2 | 10 |
| ... (0T15)Rf8 (0T16)Qe2 (0T16)c5 (0T17)Rf1 (0T17)cxd4 (0T18)Rxf8+ (0T18)Qxf8 **(0T19)cxb4** | play 10 | -339 | -209 | 130 | (0T19)e2d1 | 10 |
| ... (0T15)Rf8 (0T16)Qe2 (0T16)c5 (0T17)Rf1 (0T17)cxd4 (0T18)Rxf8+ (0T18)Qxf8 **(0T19)cxb4** | play 10 | -339 | -209 | 130 | (0T19)e2d1 | 10 |
| ... (0T8)Rf8 (0T9)Qe2 (0T9)h6 (0T10)Nxe6+ (0T10)fxe6 (0T11)Qf3 (0T11)Qe7 **(0T12)Bxd6** | play 10 | 0 | 128 | 128 | (0T12)f3g3 | 13 |
| ... (0T8)Bxf4 (0T9)exf4 (0T9)Qe7 (0T10)Nd2 (0T10)Ne4 (0T11)Ndxe4 (0T11)e5 **(0T12)Ne6+** | play 10 | 84 | 211 | 127 | (0T12)g5e6 | 11 |
| ... (0T12)Qd7 (0T13)Nf3 (0T13)Qd6 (0T14)Ne5 (0T14)Nbd7 (0T15)Qd3 (0T15)Ne4 **(0T16)Qh3** | play 10 | 13 | 140 | 127 | (0T16)d3h3 | 8 |
| ... (0T9)Qxd6 (0T10)Nxf7 (0T10)Kxf7 (0T11)Qf1 (0T11)Ke8 (0T12)g4 (0T12)Qc7 **(0T13)g5** | play 3 | -15 | 111 | 126 | (0T13)f1g2 | 15 |

(top 80 of 265 actions; all in `book_eval_actions.tsv`)

## Known actions within 50 cp of the parent's best (candidates to promote to played)

| line (parent) and action | tag | score after (mover) | parent score | loss | search best at parent | child depth |
|---|---|---|---|---|---|---|
| ... (0T12)Nd2 (0T12)Qxg5 (0T13)exd5 (0T13)Nxd5 (0T14)Ne4 (0T14)Qe7 (0T15)Nd6+ **(0T15)Qxd6** | know 0 | -395 | -9940 | -1105 | (0T15)e7d6 | 11 |
| ... (0T12)Nd2 (0T12)Qxg5 (0T13)exd5 (0T13)Nxd5 (0T14)Ne4 (0T14)Qe7 (0T15)Nd6+ **(0T15)Qxd6** | know 0 | -395 | -9940 | -1105 | (0T15)e7d6 | 11 |
| ... (0T18)Rf1 (0T18)Qxh2 (0T19)Qd1 (0T19)Qxg2 (0T20)Nd2 (0T20)Rxd2 (0T21)Rf7+ **(0T21)Kxf7** | know 0 | -417 | -9930 | -1083 | (0T21)g8f7 | 12 |
| ... (0T18)Rf1 (0T18)Qxh2 (0T19)Qd1 (0T19)Qxg2 (0T20)Nd2 (0T20)Rxd2 (0T21)Rf7+ **(0T21)Kxf7** | know 0 | -417 | -9930 | -1083 | (0T21)g8f7 | 12 |
| ... (0T18)Rg1 (0T18)h5 (0T19)Rxg4 (0T19)hxg4 (0T20)Ne5+ (0T20)Ke8 (0T21)Nc4 **(0T21)Kd7** | know 0 | 50 | -128 | -178 | (0T21)e8d7 | 11 |
| ... (0T8)Ng5 (0T8)Rf8 (0T9)Qe2 (0T9)Bxf4 (0T10)exf4 (0T10)Ne4 (0T11)Nxe6+ **(0T11)fxe6** | know 0 | -209 | -369 | -160 | (0T11)f7e6 | 11 |
| ... (0T8)Ng5 (0T8)Rf8 (0T9)Qe2 (0T9)Bxf4 (0T10)exf4 (0T10)Ne4 (0T11)Nxe6+ **(0T11)fxe6** | know 0 | -209 | -369 | -160 | (0T11)f7e6 | 11 |
| ... (0T18)Rg1 (0T18)h5 (0T19)Rxg4 (0T19)hxg4 (0T20)Ne5+ (0T20)Ke8 (0T21)Nc4 **(0T21)Kd7** | know 0 | 50 | -106 | -156 | (0T21)e8d7 | 11 |
| ... (0T12)Qc2 (0T12)e5 (0T13)Ng6 (0T13)Qf7 (0T14)Ne7+ (0T14)Kxe7 (0T15)dxe5 **(0T15)Ne4** | know 0 | 7 | -130 | -137 | (0T15)d7c5 | 12 |
| ... (0T12)Qc2 (0T12)e5 (0T13)Ng6 (0T13)Qf7 (0T14)Ne7+ (0T14)Kxe7 (0T15)dxe5 **(0T15)Ne4** | know 0 | 7 | -130 | -137 | (0T15)e7e8 | 12 |
| ... (0T9)Qe7 (0T10)Nd2 (0T10)Ne4 (0T11)Ndxe4 (0T11)e5 (0T12)Ne6+ (0T12)Qxe6 **(0T13)Nd6+** | know 0 | 229 | 109 | -120 | (0T13)e4g5 | 12 |
| ... (0T9)exf4 (0T9)Qe7 (0T10)Nd2 (0T10)Ne4 (0T11)Ndxe4 (0T11)e5 (0T12)Ne6+ **(0T12)Qxe6** | know 0 | -109 | -202 | -93 | (0T12)e7e6 | 10 |
| ... (0T16)Qe2 (0T16)c5 (0T17)Rf1 (0T17)cxd4 (0T18)Rxf8+ (0T18)Qxf8 (0T19)cxb4 **(0T19)Qxb4** | know 0 | 427 | 339 | -88 | (0T19)f8b4 | 10 |
| ... (0T16)Qe2 (0T16)c5 (0T17)Rf1 (0T17)cxd4 (0T18)Rxf8+ (0T18)Qxf8 (0T19)cxb4 **(0T19)Qxb4** | know 0 | 427 | 339 | -88 | (0T19)f8b4 | 10 |
| ... (0T2)e3 (0T2)d5 (0T3)Bd3 (0T3)c6 (0T4)Ng5 (0T4)Bg4 (0T5)Qxg4 **(0T5)Nxg4** | know 0 | 28 | -53 | -81 | (0T5)f6g4 | 10 |
| ... (0T10)h6 (0T11)Qe2 (0T11)hxg5 (0T12)Ne4 (0T12)Qd7 (0T13)Q>>xe6+~ (>L1) **(1T9)Qe7** | know 0 | 91 | 11 | -80 | (1T9)d8e7 | 10 |
| ... (0T10)h6 (0T11)Qe2 (0T11)hxg5 (0T12)Ne4 (0T12)Qd7 (0T13)Q>>xe6+~ (>L1) **(1T9)Qe7** | know 0 | 91 | 11 | -80 | (1T9)d8e7 | 10 |
| ... (0T18)Nc5+ (0T18)Kc8 (0T19)O-O-O (0T19)Nxf2 (0T20)Rxf2 (0T20)Rxf2 (0T21)Re1 **(0T21)Rf6** | know 0 | -22 | -92 | -70 | (0T21)b8a6 | 9 |
| ... (0T3)e6 (0T4)Bd2 (0T4)Bxc5 (0T5)g3 (0T5)Bxf2+ (0T6)Kxf2 (0T6)Ne4+ **(0T7)Ke1** | know 0 | -198 | -264 | -66 | (0T7)f2e1 | 12 |
| ... (0T10)Nd2 (0T10)Ne4 (0T11)Ndxe4 (0T11)e5 (0T12)Ne6+ (0T12)Qxe6 (0T13)Nd6+ **(0T13)Qxd6** | know 0 | -163 | -229 | -66 | (0T13)e8f8 | 10 |
| ... (0T22)Ne4 (0T22)Kd7 (0T23)Nxf6+ (0T23)gxf6 (0T24)Rg1 (0T24)Na6 (0T25)Rg7+ **(0T25)Kd6** | know 0 | -278 | -341 | -63 | (0T25)d7d6 | 7 |
| ... (0T22)Ne4 (0T22)Kd7 (0T23)Nxf6+ (0T23)gxf6 (0T24)Rg1 (0T24)Na6 (0T25)Rg7+ **(0T25)Kd6** | know 0 | -278 | -341 | -63 | (0T25)d7d6 | 7 |
| ... (0T17)d5 (0T17)Na6 (0T18)dxc6+ (0T18)Kc7 (0T19)Nd6+ (0T19)Kxd6 (0T20)O-O-O+ **(0T20)Kxc6** | know 0 | -312 | -374 | -62 | (0T20)d6c6 | 10 |
| ... (0T17)d5 (0T17)Na6 (0T18)dxc6+ (0T18)Kc7 (0T19)Nd6+ (0T19)Kxd6 (0T20)O-O-O+ **(0T20)Kxc6** | know 0 | -312 | -374 | -62 | (0T20)d6c6 | 10 |
| ... (0T10)Nxf7 (0T10)Kxf7 (0T11)Qf1 (0T11)Ke8 (0T12)g4 (0T12)Qd8 (0T13)g5 **(0T13)Ng4** | know 0 | 28 | -31 | -59 | (0T13)f6g4 | 13 |
| ... (0T8)Rf8 (0T9)Qe2 (0T9)h6 (0T10)Nxe6+ (0T10)fxe6 (0T11)Qf3 (0T11)Qe7 **(0T12)Qg3** | know 0 | 184 | 128 | -56 | (0T12)f3g3 | 12 |
| ... (>L1) (1T9)Qe7 (1T10)Qc8+ (1T10)Qd8 (1T11)Ne6+ (1T11)fxe6 (1T12)Qg6+ **(1T12)hxg6** | know 0 | 316 | 266 | -50 | (1T12)h7g6 | 8 |
| ... (0T18)Nc5+ (0T18)Kc8 (0T19)O-O-O (0T19)Nxf2 (0T20)Rxf2 (0T20)Rxf2 (0T21)Re1 **(0T21)Rf6** | know 0 | -22 | -72 | -50 | (0T21)f2f6 | 9 |
| ... (>L1) (1T9)Qe7 (1T10)Qc8+ (1T10)Qd8 (1T11)Ne6+ (1T11)fxe6 (1T12)Qg6+ **(1T12)hxg6** | know 0 | 316 | 266 | -50 | (1T12)h7g6 | 8 |
| ... (0T8)Rf8 (0T9)Qe2 (0T9)h6 (0T10)Nxe6+ (0T10)fxe6 (0T11)Qf3 (0T11)Qe7 **(0T12)Qg3** | know 0 | 177 | 128 | -49 | (0T12)f3g3 | 12 |
| ... (0T11)Ndxe4 (0T11)e5 (0T12)Ne6+ (0T12)Qxe6 (0T13)Nd6+ (0T13)Qxd6 (0T14)Qh3 **(0T14)Qd7** | know 0 | -151 | -199 | -48 | (0T14)e8g8 | 12 |
| ... (0T3)Ng5 (0T3)f5 (0T4)Qxg4 (0T4)fxg4 (0T5)Nf7 (0T5)e6 (0T6)Nxd8+ **(0T6)Kxd8** | know 0 | -30 | -77 | -47 | (0T6)e8d8 | 12 |
| ... (0T12)Qe1 (0T12)Nxf2+ (0T13)Qxf2 (0T13)e5 (0T14)Qe1 (0T14)e4 (0T15)Ne6+ **(0T15)Qxe6** | know 0 | -75 | -121 | -46 | (0T15)e7e6 | 10 |
| ... (0T12)Qe1 (0T12)Nxf2+ (0T13)Qxf2 (0T13)e5 (0T14)Qe1 (0T14)e4 (0T15)Ne6+ **(0T15)Qxe6** | know 0 | -75 | -121 | -46 | (0T15)e7e6 | 10 |
| ... (0T9)Qe2 (0T9)Qe7 (0T10)Bxd6 (0T10)Qxd6 (0T11)e4 (0T11)Qf4 (0T12)Nd2 **(0T12)Qxg5** | know 0 | 14 | -31 | -45 | (0T12)b8a6 | 10 |
| ... (0T13)exd5 (0T13)Qh5 (0T14)Qxh5 (0T14)Nxh5 (0T15)Ne4 (0T15)Kd7 (0T16)Nc5+ **(0T16)Kc8** | know 0 | -7 | -52 | -45 | (0T16)d7e8 | 11 |
| ... (0T13)exd5 (0T13)Qh5 (0T14)Qxh5 (0T14)Nxh5 (0T15)Ne4 (0T15)Kd7 (0T16)Nc5+ **(0T16)Kc8** | know 0 | -7 | -52 | -45 | (0T16)d7e8 | 11 |
| ... (0T5)e3 (0T5)e6 (0T6)Bd3 (0T6)Bxd3 (0T7)Qxd3 (0T7)Bd6 (0T8)Ng5 **(0T8)Bxf4** | know 0 | 7 | -37 | -44 | (0T8)d8c7 | 13 |
| ... (0T5)e3 (0T5)e6 (0T6)Bd3 (0T6)Bxd3 (0T7)Qxd3 (0T7)Bd6 (0T8)Ng5 **(0T8)Bxf4** | know 0 | 7 | -37 | -44 | (0T8)d8c7 | 13 |
| ... (0T9)Qe2 (0T9)Qe7 (0T10)Bxd6 (0T10)Qxd6 (0T11)e4 (0T11)Qf4 (0T12)Nd2 **(0T12)Qxg5** | know 0 | 14 | -30 | -44 | (0T12)b8a6 | 10 |
| ... (0T8)Ng5 (0T8)g6 (0T9)Nxf7 (0T9)Kxf7 (0T10)Bg5 (0T10)Qb6 (0T11)Bxf6+ **(0T11)Kxf6** | know 0 | -270 | -311 | -41 | (0T11)f7f6 | 11 |
| ... (0T8)Ng5 (0T8)Rf8 (0T9)Qe2 (0T9)Bxf4 (0T10)exf4 (0T10)Qe7 (0T11)Kd1 **(0T11)Ne4** | know 0 | -6 | -47 | -41 | (0T11)g7g6 | 10 |
| ... (0T8)Ng5 (0T8)g6 (0T9)Nxf7 (0T9)Kxf7 (0T10)Bg5 (0T10)Qb6 (0T11)Bxf6+ **(0T11)Kxf6** | know 0 | -270 | -311 | -41 | (0T11)f7f6 | 11 |
| ... (0T4)Bxf6 (0T4)h5 (0T5)Bxe7+ (0T5)Qxe7 (0T6)Qf3 (0T6)f5 (0T7)Qg3 **(0T7)g5** | know 0 | -4 | -45 | -41 | (0T7)g7g5 | 11 |
| ... (0T16)Nc4 (0T16)Kd7 (0T17)dxe6+ (0T17)fxe6 (0T18)Rg1 (0T18)h5 (0T19)Rxg4 **(0T19)hxg4** | know 0 | 41 | 1 | -40 | (0T19)h5g4 | 11 |
| ... (0T16)Nc4 (0T16)Kd7 (0T17)dxe6+ (0T17)fxe6 (0T18)Rg1 (0T18)h5 (0T19)Rxg4 **(0T19)hxg4** | know 0 | 41 | 1 | -40 | (0T19)h5g4 | 11 |
| ... (0T12)Qxe4 (0T12)Nd7 (0T13)O-O (0T13)h6 (0T14)f5 (0T14)Nf6 (0T15)Qe2 **(0T15)hxg5** | know 0 | 61 | 22 | -39 | (0T15)h6g5 | 12 |
| ... (0T12)Qxe4 (0T12)Nd7 (0T13)O-O (0T13)h6 (0T14)f5 (0T14)Nf6 (0T15)Qe2 **(0T15)hxg5** | know 0 | 61 | 22 | -39 | (0T15)h6g5 | 12 |
| ... (0T14)Rf1 (0T14)Qg4 (0T15)Qxg4 (0T15)Nxg4 (0T16)Ne4 (0T16)Kd7 (0T17)dxe6+ **(0T17)fxe6** | know 0 | -35 | -73 | -38 | (0T17)f7e6 | 12 |
| ... (0T15)Kd7 (0T16)dxe6+ (0T16)fxe6 (0T17)Nc5+ (0T17)Kc8 (0T18)O-O-O (0T18)Nf4 **(0T19)g3** | know 0 | 15 | -23 | -38 | (0T19)g2g3 | 11 |
| ... (0T14)exd5 (0T14)exd5 (0T15)Nde4 (0T15)dxe4 (0T16)Nxe4 (0T16)Kd7 (0T17)Nc5+ **(0T17)Kc8** | know 0 | 23 | -15 | -38 | (0T17)d7c8 | 11 |
| ... (0T14)Rf1 (0T14)Qg4 (0T15)Qxg4 (0T15)Nxg4 (0T16)Ne4 (0T16)Kd7 (0T17)dxe6+ **(0T17)fxe6** | know 0 | -35 | -73 | -38 | (0T17)f7e6 | 12 |
| ... (0T15)Kd7 (0T16)dxe6+ (0T16)fxe6 (0T17)Nc5+ (0T17)Kc8 (0T18)O-O-O (0T18)Nf4 **(0T19)g3** | know 0 | 15 | -23 | -38 | (0T19)g2g3 | 11 |
| ... (0T14)exd5 (0T14)exd5 (0T15)Nde4 (0T15)dxe4 (0T16)Nxe4 (0T16)Kd7 (0T17)Nc5+ **(0T17)Kc8** | know 0 | 23 | -15 | -38 | (0T17)d7c8 | 11 |
| ... (0T11)Ndxe4 (0T11)e5 (0T12)Ne6+ (0T12)Qxe6 (0T13)Nd6+ (0T13)Qxd6 (0T14)Qh3 **(0T14)Qd7** | know 0 | -151 | -188 | -37 | (0T14)e8g8 | 12 |
| ... (0T10)Ne4 (0T11)Ndxe4 (0T11)e5 (0T12)Ne6+ (0T12)Qxe6 (0T13)Nd6+ (0T13)Qxd6 **(0T14)Qh3** | know 0 | 199 | 163 | -36 | (0T14)d3f5 | 12 |
| ... (0T10)exf4 (0T10)Qe7 (0T11)Kd1 (0T11)Nbd7 (0T12)Qe1 (0T12)Ng4 (0T13)Nxe6+ **(0T13)Qxe6** | know 0 | 102 | 68 | -34 | (0T13)e7e6 | 10 |
| ... (0T15)Nxe5 (0T15)Qe7 (0T16)Nd2 (0T16)Ne4 (0T17)Nxd7 (0T17)Rxd7 (0T18)Nxe4 **(0T18)dxe4** | know 0 | -150 | -184 | -34 | (0T18)d5e4 | 12 |
| ... (0T10)exf4 (0T10)Qe7 (0T11)Kd1 (0T11)Nbd7 (0T12)Qe1 (0T12)Ng4 (0T13)Nxe6+ **(0T13)Qxe6** | know 0 | 102 | 68 | -34 | (0T13)e7e6 | 10 |
| ... (0T15)Ne4 (0T15)Kd7 (0T16)dxe6+ (0T16)fxe6 (0T17)Nc5+ (0T17)Kc8 (0T18)O-O-O **(0T18)Nf4** | know 0 | 23 | -10 | -33 | (0T18)h5f4 | 11 |
| ... (0T15)Ne4 (0T15)Kd7 (0T16)dxe6+ (0T16)fxe6 (0T17)Nc5+ (0T17)Kc8 (0T18)O-O-O **(0T18)Nf4** | know 0 | 23 | -10 | -33 | (0T18)h5f4 | 11 |
| ... (0T13)Kd2 (0T13)Ne4+ (0T14)fxe4+ (0T14)Kg8 (0T15)Kc1 (0T15)Rf8 (0T16)Qd1 **(0T16)Rf2** | know 0 | 100 | 68 | -32 | (0T16)c6c5 | 13 |
| ... (0T12)h6 (0T13)Nf3 (0T13)Nd7 (0T14)Qe2 (0T14)Nf6 (0T15)Ne5 (0T15)Nd5 **(0T16)h4** | know 0 | 139 | 107 | -32 | (0T16)e1g1 | 11 |
| ... (0T13)Kd2 (0T13)Ne4+ (0T14)fxe4+ (0T14)Kg8 (0T15)Kc1 (0T15)Rf8 (0T16)Qd1 **(0T16)Rf2** | know 0 | 100 | 68 | -32 | (0T16)c6c5 | 13 |
| ... (0T9)Qe2 (0T9)Bxf4 (0T10)exf4 (0T10)Qe7 (0T11)Kd1 (0T11)Nbd7 (0T12)Qe1 **(0T12)Ng4** | know 0 | -8 | -39 | -31 | (0T12)f6g4 | 13 |
| ... (0T15)Nxe5 (0T15)Qe7 (0T16)Nd2 (0T16)Ne4 (0T17)Nxd7 (0T17)Rxd7 (0T18)Nxe4 **(0T18)dxe4** | know 0 | -150 | -180 | -30 | (0T18)d5e4 | 12 |
| ... (0T12)g4 (0T12)Qc7 (0T13)Nd2 (0T13)Rf8 (0T14)Nf3 (0T14)Nbd7 (0T15)Ng5 **(0T15)Qd6** | know 0 | -3 | -33 | -30 | (0T15)c7d6 | 14 |
| ... (0T11)Qf1 (0T11)Na6 (0T12)f4 (0T12)Nb4 (0T13)Kd2 (0T13)Ne4+ (0T14)Kc1 **(0T14)Nc2+** | know 0 | 132 | 103 | -29 | (0T14)b4c2 | 10 |
| ... (0T11)Qf1 (0T11)Na6 (0T12)f4 (0T12)Nb4 (0T13)Kd2 (0T13)Ne4+ (0T14)Kc1 **(0T14)Nc2+** | know 0 | 132 | 103 | -29 | (0T14)b4c2 | 10 |
| ... (0T6)Bd3 (0T6)Bxd3 (0T7)Qxd3 (0T7)Bd6 (0T8)Ng5 (0T8)Bxf4 (0T9)exf4 **(0T9)Qe7** | know 0 | 21 | -7 | -28 | (0T9)f6g4 | 13 |
| ... (0T8)Rf8 (0T9)Qe2 (0T9)Bxf4 (0T10)exf4 (0T10)Qe7 (0T11)Kd1 (0T11)Ng4 **(0T12)Qxg4** | know 0 | 79 | 51 | -28 | (0T12)b1d2 | 14 |
| ... (0T13)Qxg2 (0T14)Rf1 (0T14)Qg4 (0T15)Qxg4 (0T15)Nxg4 (0T16)Ne4 (0T16)Na6 **(0T17)dxe6** | know 0 | -17 | -44 | -27 | (0T17)d5e6 | 12 |
| ... (0T13)Qxg2 (0T14)Rf1 (0T14)Qg4 (0T15)Qxg4 (0T15)Nxg4 (0T16)Ne4 (0T16)Na6 **(0T17)dxe6** | know 0 | -17 | -44 | -27 | (0T17)d5e6 | 12 |
| ... (0T6)Nf6 (0T7)B>>xf7+~ (>L1) (1T5)Kxf7 (1T6)Qxg4 (1T6)Qc8 (1T7)Qxc8 **(0T7)Q>xc8** | know 0 | -337 | -363 | -26 | (0T7)f6(0T6)f4 | 6 |
| ... (0T9)Bxd6 (0T9)Qxd6 (0T10)Nxf7 (0T10)Qe7 (0T11)Nxh8 (0T11)g6 (0T12)Nxg6 **(0T12)Qf7** | know 0 | -128 | -154 | -26 | (0T12)e7d6 | 12 |
| ... (0T8)Ng5 (0T8)Rf8 (0T9)Qe2 (0T9)Bxf4 (0T10)exf4 (0T10)Qe7 (0T11)Nd2 **(0T11)h6** | know 0 | -66 | -92 | -26 | (0T11)b8d7 | 12 |
| ... (0T11)Ndxe4 (0T11)dxe4 (0T12)Qxe4 (0T12)Nd7 (0T13)O-O (0T13)h6 (0T14)Nf3 **(0T14)Nf6** | know 0 | -61 | -86 | -25 | (0T14)d7f6 | 13 |
| ... (0T11)Ndxe4 (0T11)dxe4 (0T12)Qxe4 (0T12)Nd7 (0T13)O-O (0T13)h6 (0T14)Nf3 **(0T14)Nf6** | know 0 | -61 | -86 | -25 | (0T14)d7f6 | 13 |
| ... (0T10)Ne4 (0T11)Ndxe4 (0T11)e5 (0T12)Ne6+ (0T12)Qxe6 (0T13)Nd6+ (0T13)Qxd6 **(0T14)Qh3** | know 0 | 188 | 163 | -25 | (0T14)d3f5 | 10 |
| ... (0T10)Nxf7 (0T10)Qe7 (0T11)Nxh8 (0T11)Kd8 (0T12)Qd1 (0T12)g6 (0T13)Qa4 **(0T13)b6** | know 0 | -134 | -158 | -24 | (0T13)b7b6 | 13 |

(top 80 of 1453 actions; all in `book_eval_actions.tsv`)

## Mates

0 actions lead to a forced mate (score after for the mover is a mate): 0 for the mover, 0 against.

