#!/usr/bin/env python3
"""Compare an NNUE with the handcrafted evaluation + residual net on the same held-out positions.

    ktools nnuerecs games/nnue/x.nnue games/nnue/t_rec8 99999999 > nn.txt
    python scripts/nnue/compare.py --nnue nn.txt --records games/nnue/t_rec8 \\
        --netdata nc_rec8.bin --net data/evalnet.bin

`--records` is the `nnuedata` export the NNUE scores came from (engine-exact, `nnuerecs`) and
`--netdata` the `netdata` export of the same games with the same filter (NETDATA_NO_CHECK=1).
Records are matched by game and order; games cut off by the action limit (no result) are left
out of both. Reported per evaluator: AUC of the score against the result (scale-free), and the
log loss and squared error of sigmoid(score / K) against the result at the K that suits that
evaluator best, and at K = 4350.
"""
import argparse
import struct
import sys

import numpy as np

sys.path.insert(0, __file__.rsplit('/', 1)[0] if '/' in __file__ else '.')
from train import Data  # noqa: E402

NI = 113


def residual_net(path):
    b = open(path, 'rb').read()
    assert b[:8] == b'KNET0001'
    n_in, h1, h2 = struct.unpack('<III', b[8:20])
    assert n_in == NI
    scale, cap = struct.unpack('<ff', b[20:28])
    a = np.frombuffer(b[28:], dtype='<f4')
    p = 0

    def take(n):
        nonlocal p
        r = a[p:p + n]
        p += n
        return r
    w1, b1 = take(h1 * NI).reshape(h1, NI), take(h1)
    w2, b2 = take(h2 * h1).reshape(h2, h1), take(h2)
    w3, b3 = take(h2), take(1)[0]

    def f(x):
        out = []
        for s in range(0, len(x), 200000):
            xx = x[s:s + 200000].astype(np.float32)
            a1 = np.clip(xx @ w1.T + b1, 0, 1)
            a2 = np.clip(a1 @ w2.T + b2, 0, 1)
            out.append(np.clip((a2 @ w3 + b3) * scale, -cap, cap))
        return np.concatenate(out)
    return f


def auc(score, y):
    m = y != 0.5
    s, yy = score[m], y[m]
    order = np.argsort(s, kind='mergesort')
    ranks = np.empty(len(s))
    ranks[order] = np.arange(1, len(s) + 1)
    pos = yy == 1
    n1, n0 = pos.sum(), (~pos).sum()
    return (ranks[pos].sum() - n1 * (n1 + 1) / 2) / (n1 * n0)


def losses(ev, y, k):
    p = np.clip(1 / (1 + np.exp(-np.clip(ev, -60000, 60000) / k)), 1e-6, 1 - 1e-6)
    return -np.mean(y * np.log(p) + (1 - y) * np.log(1 - p)), np.mean((p - y) ** 2)


def report(name, ev, y):
    ks = np.geomspace(1000, 30000, 120)
    lls = [losses(ev, y, k)[0] for k in ks]
    kb = ks[int(np.argmin(lls))]
    ll, mse = losses(ev, y, kb)
    ll0, _ = losses(ev, y, 4350.0)
    print(f'{name:10s} auc {auc(ev, y):.5f}  ll {ll:.5f} mse {mse:.5f} (best K {kb:.0f})  ll@4350 {ll0:.5f}')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--nnue', nargs='+', required=True, help='nnuerecs outputs (one score per record)')
    ap.add_argument('--records', required=True)
    ap.add_argument('--netdata', required=True)
    ap.add_argument('--net', required=True)
    args = ap.parse_args()
    h = np.asarray(Data(args.records).rec['h'])
    game, ply, res = h[:, 2].astype(np.int64), h[:, 3].astype(np.int64), h[:, 1]
    order = np.lexsort((ply, game))
    nd = np.fromfile(args.netdata, dtype=np.float32).reshape(-1, NI + 4)
    ngame = nd[:, NI + 3].astype(np.int64)
    norder = np.argsort(ngame, kind='stable')
    if len(nd) != len(h) or not np.array_equal(np.sort(game), ngame[norder]):
        sys.exit(f'record sets differ: {len(h)} nnue records, {len(nd)} netdata records')
    inc = (nd[:, NI] + residual_net(args.net)(nd[:, :NI]))[norder]
    y = res[order]
    keep = ~np.isnan(y)
    print(f'{keep.sum()} positions with a result ({(~keep).sum()} of action-limit games left out)')
    report('hce+net', inc[keep], y[keep])
    for path in args.nnue:
        ev = np.loadtxt(path)[order]
        report(path.rsplit('/', 1)[-1][:10], ev[keep], y[keep])


if __name__ == '__main__':
    main()
