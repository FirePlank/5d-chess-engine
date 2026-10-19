"""Fit the standard-chess terms (src/engine/chess.rs) to Peras' centipawn evaluation.

  ktools chessfeat <labels.tsv> feat.bin 12 600000
  python scripts/chess/fit_chess.py feat.bin [--out src/engine/chess_tab.rs] [--k 400] [--l2 1e-6]
        [--drop MAT,...]   (groups left out of the fit: their tables are zero)
        [--dropterms ROOK_TRAPPED,...]   (single terms left out)

The features are raw term counts (white minus black); the middlegame and endgame coefficients of
every term are fitted together (count * phase/24 and count * (24-phase)/24) by minimizing
(sigmoid(pred/K) - sigmoid(peras/K))^2 on positions with |cp| <= 1500. Tables are written in
eighths of a centipawn."""
import argparse
import sys

import numpy as np
from scipy.optimize import minimize

ap = argparse.ArgumentParser()
ap.add_argument('feat')
ap.add_argument('--out', default='')
ap.add_argument('--k', type=float, default=400.0)
ap.add_argument('--l2', type=float, default=1e-9)
ap.add_argument('--ncount', type=int, default=0)
ap.add_argument('--drop', default='')
ap.add_argument('--dropterms', default='')
ap.add_argument('--maxit', type=int, default=400)
ap.add_argument('--init', default='')
a = ap.parse_args()

# Term layout, as chess.rs (kept in sync by reading its constants).
import re
src = open('src/engine/chess.rs', encoding='utf-8').read()
sizes = {}
names = re.findall(r'pub const (\w+): usize = ([^;]+);', src[src.index('pub mod t {'):src.index('const fn group_table')])
vals = {}
for n, e in names:
    e = e.strip()
    e = re.sub(r'\b([A-Z_0-9]+)\b', lambda m: str(vals[m.group(1)]) if m.group(1) in vals else m.group(1), e)
    vals[n] = eval(e)
NT = vals['COUNT']
GROUPS = {'MAT': (0, vals['PSQT']), 'PSQT': (vals['PSQT'], vals['PASSED']), 'PAWN': (vals['PASSED'], vals['BPAIR']),
          'PIECE': (vals['BPAIR'], vals['THREAT_M']), 'THREAT': (vals['THREAT_M'], vals['SHIELD1']),
          'KING': (vals['SHIELD1'], vals['MOB']), 'MOB': (vals['MOB'], NT)}

raw = np.fromfile(a.feat, dtype=np.uint8)
rec = 8 + 2 * NT
n = raw.size // rec
raw = raw[:n * rec].reshape(n, rec)
phase = raw[:, 0:2].copy().view('<i2')[:, 0].astype(np.float32)
stm = raw[:, 2:4].copy().view('<i2')[:, 0].astype(np.int32)
cp = raw[:, 4:8].copy().view('<i4')[:, 0].astype(np.float32)
X = raw[:, 8:].copy().view('<i2').astype(np.float32)
del raw
ywhite = np.where(stm == 1, -cp, cp)
keep = np.abs(ywhite) <= 1500
X, phase, ywhite, stm = X[keep], phase[keep], ywhite[keep], stm[keep]
n = len(ywhite)
sgn = np.where(stm == 1, -1.0, 1.0).astype(np.float32)
print(f'{n} positions, {NT} terms', file=sys.stderr)

active = np.ones(NT, bool)
for g in a.drop.split(','):
    if g:
        lo, hi = GROUPS[g]
        active[lo:hi] = False
for tname in a.dropterms.split(','):
    if tname:
        active[vals[tname]] = False
cols = np.flatnonzero(active)
mg_w = (phase / 24.0)[:, None]
eg_w = ((24.0 - phase) / 24.0)[:, None]
Xa = X[:, cols]
D = np.concatenate([Xa * mg_w, Xa * eg_w, (sgn * phase / 24.0)[:, None], (sgn * (24 - phase) / 24.0)[:, None]], axis=1).astype(np.float32)
del X, Xa
m = len(cols)
sc = np.sqrt((D.astype(np.float64) ** 2).mean(axis=0)) + 1e-6
D = (D / sc.astype(np.float32)).astype(np.float32)
rng = np.random.default_rng(1)
perm = rng.permutation(n)
hold = perm[: n // 10]
train = perm[n // 10:]
K = a.k
yt = 1.0 / (1.0 + np.exp(-ywhite / K))


def loss_grad(w, idx):
    pred = D[idx] @ w.astype(np.float32)
    p = 1.0 / (1.0 + np.exp(-np.clip(pred / K, -30, 30)))
    d = p - yt[idx]
    l = float(np.mean(d * d))
    gp = (2.0 * d * p * (1 - p) / K).astype(np.float32)
    g = (D[idx].T @ gp) / len(idx)
    return l, g.astype(np.float64)


def f(w):
    l, g = loss_grad(w, train)
    wc = w / sc          # centipawns
    l += a.l2 * float(wc @ wc)
    g = g + 2 * a.l2 * wc / sc
    return 1000 * l, 1000 * g


w0 = np.zeros(2 * m + 2)
# start from material at conventional values
for k, v in enumerate([100, 320, 330, 500, 900]):
    if active[k]:
        j = int(np.searchsorted(cols, k))
        w0[j] = v * sc[j]
        w0[m + j] = v * sc[m + j]
if a.init:
    pass
base_l = loss_grad(np.zeros(2 * m + 2), hold)[0]
print(f'zero-eval holdout loss {base_l:.6f}', file=sys.stderr)
print(f'material-only start holdout loss {loss_grad(w0, hold)[0]:.6f}', file=sys.stderr)
res = minimize(f, w0, jac=True, method='L-BFGS-B', options={'maxiter': a.maxit, 'maxfun': a.maxit * 2, 'maxls': 40, 'ftol': 1e-13, 'gtol': 1e-9})
w = res.x
w_s = w
w = w / sc
print(f'fit: train {loss_grad(w_s, train)[0]:.6f} holdout {loss_grad(w_s, hold)[0]:.6f} ({res.nit} it, {res.message})', file=sys.stderr)
pred = D[hold] @ w_s.astype(np.float32)
print(f'holdout corr {np.corrcoef(pred, ywhite[hold])[0, 1]:.4f}, rmse(clipped 600) {np.sqrt(np.mean((np.clip(pred, -600, 600) - np.clip(ywhite[hold], -600, 600)) ** 2)):.1f} cp', file=sys.stderr)
print(f'tempo mg/eg {w[2 * m]:.1f} {w[2 * m + 1]:.1f}', file=sys.stderr)

tab = np.zeros((NT, 2))
tab[cols, 0] = w[:m]
tab[cols, 1] = w[m:2 * m]
np.save(a.out.replace('.rs', '.npy') if a.out else 'chess_tab.npy', tab)
if a.out:
    t8 = np.clip(np.round(tab * 8), -32000, 32000).astype(int)
    rows = ',\n'.join('    [%d, %d]' % (x, y) for x, y in t8)
    open(a.out, 'w').write('//! Fitted coefficients of `chess` (middlegame, endgame; eighths of a centipawn).\n//! Generated by scripts/chess/fit_chess.py.\n\npub static TAB: [[i16; 2]; super::chess::t::COUNT] = [\n' + rows + ',\n];\n')
    print('wrote', a.out, file=sys.stderr)
# summary
for g, (lo, hi) in GROUPS.items():
    print(f'{g}: mean |mg| {np.abs(tab[lo:hi, 0]).mean():.1f} mean |eg| {np.abs(tab[lo:hi, 1]).mean():.1f}', file=sys.stderr)
print('material mg', np.round(tab[0:5, 0], 0), 'eg', np.round(tab[0:5, 1], 0), file=sys.stderr)
