# The sac gauntlet's openings (scripts/sac_gauntlet.sh): the Turn Zero and Standard starts
# and the f7 guide's lines and the lost Turn Zero game from data/tactics.g, before the sac.
G 1 size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:0:b] [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w] moves
G 1 size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w] moves
# f7 defence Std: 1...Nf6 against 1. e3
G 1 size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w] moves (0T1)e2e3 submit
# f7 defence Std: 1...Nf6 against 1. c3
G 1 size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w] moves (0T1)c2c3 submit
# f7 defence Std: 1...Nf6 against 1. e4
G 1 size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w] moves (0T1)e2e4 submit
# f7 defence Std: Q to a5/b6/c7 after 4. Qb3 (Bb5 line)
G 1 size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w] moves (0T1)e2e3 submit (0T1)g8f6 submit (0T2)f1b5 submit (0T2)c7c6 submit (0T3)c2c3 submit (0T3)c6b5 submit (0T4)d1b3 submit
# f7 defence Std: Q to a5/b6/c7 after 4. Qb3 (g4 line)
G 1 size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w] moves (0T1)e2e3 submit (0T1)g8f6 submit (0T2)g2g4 submit (0T2)g7g5 submit (0T3)c2c3 submit (0T3)c7c6 submit (0T4)d1b3 submit
# f7 defence Std: 2...c6 against 2. Qa4
G 1 size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w] moves (0T1)c2c3 submit (0T1)g8f6 submit (0T2)d1a4 submit
# f7 defence Std: 3...d5 against 3. a3
G 1 size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w] moves (0T1)c2c3 submit (0T1)g8f6 submit (0T2)d1a4 submit (0T2)c7c6 submit (0T3)a2a3 submit
# f7 defence Std: classic game, 2...f5 against 2. Qf3
G 1 size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w] moves (0T1)e2e3 submit (0T1)e7e6 submit (0T2)d1f3 submit
# f7 defence T0 game: 5...Ne4
G 1 size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:0:b] [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w] moves (0T1)g1f3 submit (0T1)g8f6 submit (0T2)d2d4 submit (0T2)d7d5 submit (0T3)c2c3 submit (0T3)c7c6 submit (0T4)c1g5 submit (0T4)c8g4 submit (0T5)e2e3 submit
# f7 defence T0 game: 7...Nxg5
G 1 size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:0:b] [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*:0:1:w] moves (0T1)g1f3 submit (0T1)g8f6 submit (0T2)d2d4 submit (0T2)d7d5 submit (0T3)c2c3 submit (0T3)c7c6 submit (0T4)c1g5 submit (0T4)c8g4 submit (0T5)e2e3 submit (0T5)f6e4 submit (0T6)h2h3 submit (0T6)g4f3 submit (0T7)d1f3 submit
