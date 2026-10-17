#!/usr/bin/env python3
"""Sac gauntlet report: how often the candidate (NEW) loses BY the f7/f2 travel sac.

Reads the --games-out JSONL of `scripts/sac_gauntlet.sh` (NEW = candidate, BASE = the sac-bot,
kairos with `SacBot=true`, which plays an f-sac signature travel whenever one is legal). A sac
is a sac-bot action with a travel back in time landing on the defender's first two ranks; a
loss by the sac is a game with such a sac that the sac-bot won.

usage: python scripts/sac_gauntlet.py games.jsonl [more.jsonl ...]
"""
import json
import re
import sys
from collections import defaultdict

TRAVEL = re.compile(r'\((-?\d+)T(\d+)\)[a-h]\d\((-?\d+)T(\d+)\)([a-h])(\d)')


def main(paths):
    games = [json.loads(l) for p in paths for l in open(p) if l.strip()]
    per = defaultdict(lambda: [0, 0.0, 0, 0])  # games, NEW score, sac games, losses by sac
    for g in games:
        acts = [a.strip() for a in g['moves'].split('submit') if a.strip()]
        book = g['book']
        # White is to move after the book when it has an even number of actions (both starts
        # have White to move); NEW plays that side when new_first.
        new_white = (book % 2 == 0) == g['new_first']
        bot_white = not new_white
        sac = False
        for k, a in enumerate(acts[book:]):
            if (k % 2 == 0) == g['new_first']:
                continue
            for m in TRAVEL.finditer(a):
                t_from, t_to, rank = int(m.group(2)), int(m.group(4)), int(m.group(6))
                if t_to < t_from and (rank >= 7 if bot_white else rank <= 2):
                    sac = True
        key = ('T0 ' if ':0:0:b' in g['position'] else 'Std ') + ' '.join(acts[:book])[-70:]
        for k in (key, 'ALL'):
            e = per[k]
            e[0] += 1
            e[1] += g['score']
            e[2] += sac
            e[3] += sac and g['score'] == 0
    for k, (n, s, sg, lost) in sorted(per.items(), key=lambda x: (x[0] != 'ALL', x[0])):
        print(f"{n:5d} games  score {s / n:.3f}  sacs {sg:4d}  lost by sac {lost:4d} ({100 * lost / n:.1f}%)  {k}")


if __name__ == '__main__':
    main(sys.argv[1:])
