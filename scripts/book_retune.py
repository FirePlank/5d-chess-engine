"""Update data/book_tuning.txt from a re-scout (scripts/book_rescout.py output), with the rule of
book_weight_proposal.md as first applied (472a83a): only actions deeper than 8 in their line;
a played action clearly more than 50 cp worse than the best (loss - sigma > 50) becomes known
unless it leaves the mover at +150 or better or a mate is involved; a played action losing more
than 35 cp gets weight min(current, round(10 * exp(-(loss - 5) / 25))). Existing entries are kept
(their "was" too) unless the new measurement changes the action again.

    python scripts/book_retune.py docs/theory/book_rescout_v2.tsv data/book_tuning.txt
"""
import csv
import math
import sys


def main():
    rescout, tuning_path = sys.argv[1:3]
    lines = open(tuning_path, encoding='utf-8').read().splitlines()
    head = [l for l in lines if l.startswith('#') or not l.strip()]
    entries = {}
    for l in lines:
        if l.startswith('#') or not l.strip():
            continue
        k, tag, w, rest = l.split(' ', 3)
        lan = rest.split(' ; ')[0]
        entries[(k, lan)] = l
    lan_of = {}
    for r in csv.DictReader(open(rescout.replace('book_rescout_v2.tsv', 'book_evals_v2.tsv.edges.tsv'), encoding='utf-8'), delimiter='\t'):
        lan_of[(r['parent'], r['child'])] = r['lan']
    seen, demoted, lowered = set(), [], []
    for r in csv.DictReader(open(rescout, encoding='utf-8'), delimiter='\t'):
        if r['tag'] != 'play' or int(r['plies']) <= 8:
            continue
        lan = lan_of[(r['parent'], r['child'])]
        key = (r['parent'], lan)
        if key in seen:
            continue
        seen.add(key)
        loss, s, after = int(r['loss']), int(r['sigma']), int(r['mover_score'])
        mate = abs(int(r['parent_score'])) >= 1500 or abs(after) >= 1500
        w = int(r['weight'])
        old = entries.get(key)
        was = old.split(' | ')[1] if old else f'was play {w}'
        if r['cls'] == 'play->know' and after < 150 and not mate:
            entries[key] = f"{r['parent']} know 0 {lan} ; {r['pgn']} | {was} | demoted (re-scout 2026-10-23): loses {loss} cp (+-{s}) against the best move"
            demoted.append(r)
        elif loss > 35:
            nw = max(1, round(10 * math.exp(-(loss - 5) / 25)))
            if nw < w:
                entries[key] = f"{r['parent']} play {nw} {lan} ; {r['pgn']} | {was} | weight {w} -> {nw} (re-scout 2026-10-23, loss {loss} cp)"
                lowered.append(r)
    out = head + sorted(entries.values())
    open(tuning_path, 'w', encoding='utf-8', newline='\n').write('\n'.join(out) + '\n')
    print(f'{len(demoted)} demoted, {len(lowered)} weights lowered, {len(entries)} entries')


if __name__ == '__main__':
    main()
