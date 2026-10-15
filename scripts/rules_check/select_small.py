# Select G lines whose position (from a `ktools rules ... nomate` dump) has few playable boards
# and few legal-move combinations: usage select.py <rules> <max_playable> <max_product> [every]
import sys
path, maxp, maxprod = sys.argv[1], int(sys.argv[2]), float(sys.argv[3])
every = int(sys.argv[4]) if len(sys.argv) > 4 else 1
out, g, T, per = [], None, None, {}
def flush():
    if g is None: return
    pl = len(per)
    prod = 1.0
    for v in per.values(): prod *= (v + 1)
    if 0 < pl <= maxp and prod <= maxprod: out.append(g)
n = 0
for line in open(path, encoding='utf-8'):
    if line.startswith('G '):
        flush(); g = line.rstrip('\n'); per = {}
    elif line.startswith('m '):
        l = line[2:].split(',')[0]
        per[l] = per.get(l, 0) + 1
flush()
for i, l in enumerate(out):
    if i % every == 0: print(l)
