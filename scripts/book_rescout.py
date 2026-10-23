"""Classify every book action from a `ktools bookeval` run (book_weight_proposal.md's rule) and
compare with an earlier run's resolved classes.

    python scripts/book_rescout.py docs/theory/book_evals_v2.tsv docs/theory/book_eval_resolved.tsv out.tsv

loss = parent score - the mover's score after the action (cp, mates clamped to +-1500); sigma by
the lower of the two search depths. play: loss - sigma > 50 demote, loss + sigma <= 50 keep, else
uncertain; know: loss + sigma <= 50 promote, loss - sigma > 50 stays known, else uncertain.
"""
import csv
import sys
from collections import Counter

SIGMA = {16: 6, 15: 9, 14: 11, 13: 18, 12: 34, 11: 43, 10: 80}


def sigma(d):
    return 6 if d >= 16 else SIGMA.get(d, 100)


def clamp(sc, kind):
    v = int(sc)
    if kind == 'mate' or abs(v) >= 5000:
        return 1500 if v > 0 else -1500
    return max(-1500, min(1500, v))


def main():
    evals_path, old_path, out_path = sys.argv[1:4]
    ev = {}
    for r in csv.DictReader(open(evals_path, encoding='utf-8'), delimiter='\t'):
        if r['score']:
            ev[r['key']] = (clamp(r['score'], r['kind']), int(r['depth']), r['best'])
    old = {}
    for r in csv.DictReader(open(old_path, encoding='utf-8'), delimiter='\t'):
        old[(r['parent'], r['child'])] = r['class']
    rows = []
    for r in csv.DictReader(open(evals_path + '.edges.tsv', encoding='utf-8'), delimiter='\t'):
        p, c = r['parent'], r['child']
        if p not in ev or c not in ev:
            continue
        ps, pd, best = ev[p]
        cs, cd, _ = ev[c]
        mover = -cs
        loss = ps - mover
        s = sigma(min(pd, cd))
        tag = r['tag']
        if tag == 'play':
            cls = 'play->know' if loss - s > 50 else 'keep play' if loss + s <= 50 else 'play uncertain'
        else:
            cls = 'know->play' if loss + s <= 50 else 'keep know' if loss - s > 50 else 'know uncertain'
        line = r['parent_line']
        rows.append(dict(parent=p, child=c, tag=tag, weight=r['weight'], pgn=r['pgn'], mover_score=mover, parent_score=ps,
                         loss=loss, sigma=s, depth=min(pd, cd), parent_best=best, cls=cls, old=old.get((p, c), ''),
                         plies=len(line.split()) + 1, parent_line=line))
    with open(out_path, 'w', encoding='utf-8', newline='') as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()), delimiter='\t')
        w.writeheader()
        w.writerows(rows)
    print(f'{len(rows)} actions; depth min/median {min(r["depth"] for r in rows)}/{sorted(r["depth"] for r in rows)[len(rows) // 2]}')
    print('new classes:', dict(Counter(r['cls'] for r in rows)))
    for o in ('play uncertain', 'know uncertain'):
        sub = [r for r in rows if r['old'] == o]
        print(f'old {o} ({len(sub)}): now', dict(Counter(r['cls'] for r in sub)))
    sub = [r for r in rows if r['old'] == 'keep play' and r['cls'] == 'play->know']
    print(f'old keep play now play->know: {len(sub)}')


if __name__ == '__main__':
    main()
