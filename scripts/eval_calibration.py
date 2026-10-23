"""How reported scores map to game results in SPRT games: for each action's reported score (cp,
mover's view) the mover's eventual game score, binned, and the logistic fitted to it.

    python scripts/eval_calibration.py games/sprt/games_tech_*.jsonl [--min-action 6] [--max-cp 800]

Prints the empirical expected score per bin, the fitted scale (the cp at which the expected score is
75%), and the same split by game phase and by active timelines (from the move text).
"""
import argparse
import glob
import json
import math

import numpy as np


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('files', nargs='+')
    ap.add_argument('--min-action', type=int, default=6, help='skip the first actions (book, opening)')
    ap.add_argument('--max-cp', type=int, default=800)
    a = ap.parse_args()
    files = sorted({f for p in a.files for f in glob.glob(p)})
    cps, ys, acts, games = [], [], [], 0
    for f in files:
        for line in open(f, encoding='utf-8'):
            try:
                g = json.loads(line)
            except json.JSONDecodeError:
                continue
            if g.get('eval_scale') != 4800 or 'evals' not in g:
                continue
            games += 1
            s_new = g['score']
            for i, e in enumerate(g['evals']):
                if e is None or i < a.min_action or abs(e) > a.max_cp:
                    continue
                mover_is_new = (i % 2 == 0) == g['new_first']
                y = s_new if mover_is_new else 1 - s_new
                cps.append(e)
                ys.append(y)
                acts.append(i)
    cp, y, act = np.array(cps, float), np.array(ys, float), np.array(acts)
    print(f'{games} games, {len(cp)} scored actions from {len(files)} files')

    def fit(c, t):
        # Logistic E = 1 / (1 + exp(-c / s)) through the origin, s by Newton on the log loss.
        k = 1 / 100.0
        for _ in range(50):
            p = 1 / (1 + np.exp(-k * c))
            g = np.sum((p - t) * c)
            h = np.sum(p * (1 - p) * c * c)
            k -= g / h
        return k

    k = fit(cp, y)
    print(f'fitted: expected score 75% at {math.log(3) / k:.0f} cp (engine assumes 100), '
          f'90% at {math.log(9) / k:.0f} cp (assumes ~200)')
    edges = [-800, -400, -250, -150, -100, -60, -30, -10, 10, 30, 60, 100, 150, 250, 400, 800]
    print('\n cp bin        n   expected   model(4800)  fitted')
    for lo, hi in zip(edges, edges[1:]):
        m = (cp >= lo) & (cp < hi)
        if m.sum() < 50:
            continue
        mid = cp[m].mean()
        model = 1 / (1 + math.exp(-mid * 4800 / 100 / 4350))
        print(f'[{lo:5},{hi:5}) {m.sum():7}   {y[m].mean():.3f}      {model:.3f}       {1 / (1 + math.exp(-k * mid)):.3f}')
    print('\nby game phase (action index):')
    for lo, hi in [(6, 20), (20, 40), (40, 70), (70, 1000)]:
        m = (act >= lo) & (act < hi)
        if m.sum() > 1000:
            kk = fit(cp[m], y[m])
            print(f'  actions {lo:3}-{hi:4}: n {m.sum():7}, 75% at {math.log(3) / kk:.0f} cp')


if __name__ == '__main__':
    main()
