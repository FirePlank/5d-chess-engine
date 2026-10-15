# Diff summary: classify the differing moves of a compare.mjs DIFF_JSON dump.
# Classify differing moves by piece letter and 4D displacement (dx, dy, dt, dl).
import json, sys, collections
d = json.load(open(sys.argv[1]))
for kind, lst in d.items():
    if not kind.startswith(('moves', 'checks')): continue
    c = collections.Counter()
    for e in lst:
        for side, key in (('K', 'onlyKairos'), ('O', 'onlyOther')):
            for m in e['detail'].get(key, []):
                if ' ' not in m: c[(side, '?', m)] += 1; continue
                p, mk = m.split(' ')
                a, b = mk.split('>')
                l1, t1, x1, y1 = map(int, a.split(',')); l2, t2, x2, y2 = map(int, b.split(','))
                c[(side, p.upper().rstrip('*') + ('*' if p.endswith('*') else ''), (x2 - x1, y2 - y1, t2 - t1, l2 - l1), 'white' if p[0].isupper() else 'black')] += 1
    print(kind)
    for k, v in sorted(c.items(), key=lambda kv: -kv[1])[:40]: print('   ', v, k)
