#!/usr/bin/env python3
"""Train the eval net (src/engine/evalnet.rs) on records from `ktools netdata`.

    python scripts/evalnet/train.py --out data/evalnet.bin games/net/*.bin

Each record is NI + 4 float32: inputs (side to move's view), handcrafted score, engine
score or NaN, result (1/0.5/0 for the side to move), game number. The net predicts a
residual r (centipawns, capped) and is trained so that sigmoid((base + r) / K) matches
lam * sigmoid(teacher / K) + (1 - lam) * result (just the result where there is no score).
Validation holds out whole games.
"""
import argparse
import struct

import numpy as np
import torch

NI = 89


def load(paths):
    parts = []
    for k, p in enumerate(paths):
        a = np.fromfile(p, dtype=np.float32).reshape(-1, NI + 4)
        a[:, NI + 3] += k * 1e6  # game ids unique across files
        parts.append(a)
    return np.concatenate(parts)


def fit_k(base, wdl):
    best = None
    for k in np.geomspace(100, 50000, 60):
        p = 1 / (1 + np.exp(-base / k))
        loss = np.mean((p - wdl) ** 2)
        if best is None or loss < best[0]:
            best = (loss, k)
    return best[1], best[0]


class Net(torch.nn.Module):
    def __init__(self, h1, h2):
        super().__init__()
        self.l1 = torch.nn.Linear(NI, h1)
        self.l2 = torch.nn.Linear(h1, h2)
        self.l3 = torch.nn.Linear(h2, 1)

    def forward(self, x):
        x = torch.clamp(self.l1(x), 0, 1)
        x = torch.clamp(self.l2(x), 0, 1)
        return self.l3(x).squeeze(1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('files', nargs='+')
    ap.add_argument('--out', default='data/evalnet.bin')
    ap.add_argument('--h1', type=int, default=64)
    ap.add_argument('--h2', type=int, default=32)
    ap.add_argument('--epochs', type=int, default=30)
    ap.add_argument('--batch', type=int, default=8192)
    ap.add_argument('--lr', type=float, default=1e-3)
    ap.add_argument('--lam', type=float, default=0.5)
    ap.add_argument('--k', type=float, default=0, help='logistic scale (0 = fit to results)')
    ap.add_argument('--scale', type=float, default=400.0, help='net output to centipawns')
    ap.add_argument('--cap', type=float, default=1000.0)
    ap.add_argument('--seed', type=int, default=1)
    a = ap.parse_args()
    torch.manual_seed(a.seed)
    rng = np.random.default_rng(a.seed)

    d = load(a.files)
    x, base, teach, wdl, game = d[:, :NI], d[:, NI], d[:, NI + 1], d[:, NI + 2], d[:, NI + 3]
    k = a.k
    if k == 0:
        k, loss = fit_k(base, wdl)
        print(f'K fitted to results: {k:.0f} (loss {loss:.5f})')
    has_t = ~np.isnan(teach)
    print(f'{len(d)} records, {has_t.mean():.1%} with engine scores, K {k:.0f}')
    target = wdl.copy()
    tt = 1 / (1 + np.exp(-np.nan_to_num(teach) / k))
    target[has_t] = a.lam * tt[has_t] + (1 - a.lam) * wdl[has_t]

    games = np.unique(game)
    val_games = set(rng.choice(games, size=max(1, len(games) // 10), replace=False).tolist())
    is_val = np.array([g in val_games for g in game])
    mean = x[~is_val].mean(0)
    std = np.maximum(x[~is_val].std(0), 0.05)

    dev = 'cuda' if torch.cuda.is_available() else 'cpu'
    T = lambda v: torch.tensor(v, dtype=torch.float32, device=dev)
    X = T((x - mean) / std)
    B, Y = T(base), T(target)
    tr = torch.tensor(np.nonzero(~is_val)[0], device=dev)
    va = torch.tensor(np.nonzero(is_val)[0], device=dev)

    net = Net(a.h1, a.h2).to(dev)
    torch.nn.init.zeros_(net.l3.weight)
    torch.nn.init.zeros_(net.l3.bias)
    opt = torch.optim.AdamW(net.parameters(), lr=a.lr, weight_decay=1e-5)
    steps = a.epochs * (len(tr) // a.batch + 1)
    sched = torch.optim.lr_scheduler.OneCycleLR(opt, max_lr=a.lr, total_steps=steps, pct_start=0.1)

    def predict(idx):
        r = torch.clamp(net(X[idx]) * a.scale, -a.cap, a.cap)
        return torch.sigmoid((B[idx] + r) / k)

    def evaluate(idx):
        with torch.no_grad():
            tot = 0.0
            for s in range(0, len(idx), 65536):
                j = idx[s:s + 65536]
                tot += torch.sum((predict(j) - Y[j]) ** 2).item()
            return tot / len(idx)

    with torch.no_grad():
        base_val = torch.mean((torch.sigmoid(B[va] / k) - Y[va]) ** 2).item()
    print(f'validation loss without net: {base_val:.5f}')
    for ep in range(1, a.epochs + 1):
        perm = tr[torch.randperm(len(tr), device=dev)]
        for s in range(0, len(perm), a.batch):
            j = perm[s:s + a.batch]
            loss = torch.mean((predict(j) - Y[j]) ** 2)
            opt.zero_grad()
            loss.backward()
            opt.step()
            sched.step()
        print(f'epoch {ep:3d} train {evaluate(tr):.5f} val {evaluate(va):.5f}')

    # Fold normalization into layer 1 and write the blob.
    w1 = net.l1.weight.detach().cpu().numpy() / std[None, :]
    b1 = net.l1.bias.detach().cpu().numpy() - (net.l1.weight.detach().cpu().numpy() * (mean / std)[None, :]).sum(1)
    w2 = net.l2.weight.detach().cpu().numpy()
    b2 = net.l2.bias.detach().cpu().numpy()
    w3 = net.l3.weight.detach().cpu().numpy().reshape(-1)
    b3 = net.l3.bias.detach().cpu().numpy().reshape(-1)
    with open(a.out, 'wb') as f:
        f.write(b'KNET0001')
        f.write(struct.pack('<III', NI, a.h1, a.h2))
        f.write(struct.pack('<ff', a.scale, a.cap))
        for arr in (w1, b1, w2, b2, w3, b3):
            f.write(np.ascontiguousarray(arr, dtype='<f4').tobytes())
    # Reference outputs for the Rust forward pass check.
    with torch.no_grad():
        r = torch.clamp(net(X[:4]) * a.scale, -a.cap, a.cap).cpu().numpy()
    print('first residuals', np.round(r, 1).tolist())
    print(f'wrote {a.out}')


if __name__ == '__main__':
    main()
