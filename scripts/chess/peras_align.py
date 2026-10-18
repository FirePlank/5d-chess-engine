"""Per-record Peras labels aligned with netdata records.

python -I scripts/chess/peras_align.py <labels.tsv> <sidecar.fen>...
For each <x>.fen writes <x>.peras.npy: float32 [n, 2] = (Peras expected score for the side to
move, Peras cp for the side to move, mates as +-3000); NaN where there is no label (several
timelines, non-standard pieces or boards, illegal as chess)."""
import sys

import numpy as np

sys.path.insert(0, 'scripts/chess')
from peras_label import to_fen  # noqa: E402

lab = {}
for line in open(sys.argv[1]):
    fen, e, sc = line.rstrip('\n').split('\t')
    cp = (3000.0 if not sc.startswith('M-') else -3000.0) if sc.startswith('M') else float(sc)
    lab[fen] = (float(e), cp)
print(len(lab), 'labels', flush=True)
memo = {}
for path in sys.argv[2:]:
    with open(path) as fh:
        lines = fh.read().split('\n')
    if lines and lines[-1] == '':
        lines.pop()
    out = np.full((len(lines), 2), np.nan, dtype=np.float32)
    for i, line in enumerate(lines):
        if line == '-':
            continue
        v = memo.get(line)
        if v is None:
            b, side = line.rsplit('|', 1)
            fen = to_fen(b, side)
            v = lab.get(fen, (np.nan, np.nan)) if fen else (np.nan, np.nan)
            memo[line] = v
        out[i] = v
    np.save(path[:-4] + '.peras.npy', out)
    print(path, len(lines), 'records,', int(np.sum(~np.isnan(out[:, 0]))), 'labelled', flush=True)
