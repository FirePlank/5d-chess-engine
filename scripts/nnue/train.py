#!/usr/bin/env python3
"""Train the board NNUE (src/engine/nnue.rs) on records from `ktools nnuedata`.

    python scripts/nnue/train.py --out data/kairos.nnue games/nnue/a games/nnue/b@2 ...

Each dataset is the prefix `ktools nnuedata` wrote (`prefix.rec/.idx/.brd/.ftr`), optionally
weighted (`prefix@w`). The network predicts the evaluation z (internal units, side to move's
view) and is trained so that sigmoid(z / K) matches lam * sigmoid(teacher / K) + (1 - lam) *
result, or just the result where a record has no teacher score. Validation holds out whole
games (every `--val-mod`th game number). `--parity prefix` loads `--init` and prints its
evaluation of the first records of `prefix`, for comparison with `ktools nnuerecs`.
"""
import argparse
import math
import os
import struct
import time
from concurrent.futures import ThreadPoolExecutor

import numpy as np
import torch

NB = 12 * 2 * 64 + 4 * 2 * 64   # board features (nnue::NB)
ROLES = 4                         # nnue::ROLES
ND = 2 * 54 + 2 * 30 + 4          # dense inputs (nnue::ND)
NR = 2 * 12 * 9 * 5 * 2 * 2 + 2 * 12 * 12 * 5 * 2  # relation features (nnue::NR)
MAGIC = b'KNNUE003'
REC = np.dtype([('h', '<f4', 4), ('d', '<i2', ND)])
BRD = np.dtype([('rec', '<u4'), ('role', '<u2'), ('n', '<u2'), ('off', '<u8')])


IDX = np.dtype([('b', '<u8'), ('r', '<u8')])


class Records:
    """Records with their boards, board features and relation features (`ktools nnuedata`)."""

    def batch(self, rows):
        """Tensors of records `rows` (sorted): head, dense, and the boards' features."""
        rows = np.asarray(rows, dtype=np.int64)
        rec = self.rec[rows]
        idx, nidx = self.idx[rows], self.idx[np.minimum(rows + 1, self.n - 1)]
        last = rows + 1 >= self.n
        first = idx['b'].astype(np.int64)
        nxt = np.where(last, len(self.brd), nidx['b'].astype(np.int64))
        r0 = idx['r'].astype(np.int64)
        rn = np.where(last, len(self.rel), nidx['r'].astype(np.int64)) - r0
        if len(self.rel) == 0:
            r0, rn = np.zeros_like(r0), np.zeros_like(r0)
        rel = self.rel[ranges(r0, rn)].astype(np.int64)
        rel_offsets = np.concatenate([[0], np.cumsum(rn)[:-1]]).astype(np.int64)
        counts = nxt - first
        board_rows = ranges(first, counts)
        brd = self.brd[board_rows]
        local = np.repeat(np.arange(len(rows)), counts)
        n = brd['n'].astype(np.int64)
        off = brd['off'].astype(np.int64)
        own = self.ftr[ranges(off, n)].astype(np.int64)
        if self.one_view:
            # The other colour's view (nnue::other_view): own/enemy swapped, ranks mirrored.
            h = np.repeat((brd['role'] >> 8).astype(np.int64), n)
            s = own & 63
            other = (((own >> 6) ^ 1) << 6) + (s & 7) + 8 * (h - 1 - (s >> 3))
        else:
            other = self.ftr[ranges(off + n, n)].astype(np.int64)
        offsets = np.concatenate([[0], np.cumsum(n)[:-1]]).astype(np.int64)
        return {
            'head': torch.from_numpy(np.ascontiguousarray(rec['h'])),
            'dense': torch.from_numpy(rec['d'].astype(np.float32)),
            'own': torch.from_numpy(own), 'other': torch.from_numpy(other),
            'offsets': torch.from_numpy(offsets),
            'board_rec': torch.from_numpy(local), 'role': torch.from_numpy((brd['role'] & 0xff).astype(np.int64)),
            'rel': torch.from_numpy(rel), 'rel_offsets': torch.from_numpy(rel_offsets),
        }


class Data(Records):
    """One `nnuedata` export, memory-mapped."""

    def __init__(self, spec, relations=True):
        prefix, _, w = spec.partition('@')
        self.weight = float(w or 1)
        self.rec = np.memmap(prefix + '.rec', dtype=REC, mode='r')
        self.idx = np.memmap(prefix + '.idx', dtype=IDX, mode='r')
        # A network without relations never reads them (an empty file stands in).
        self.rel = np.memmap(prefix + '.rel', dtype='<u2', mode='r') if relations else np.zeros(0, dtype='<u2')
        self.brd = np.memmap(prefix + '.brd', dtype=BRD, mode='r')
        # One view stored (.ft1, the other computed) or, in older exports, both (.ftr).
        self.one_view = os.path.exists(prefix + '.ft1')
        self.ftr = np.memmap(prefix + ('.ft1' if self.one_view else '.ftr'), dtype='<u2', mode='r')
        self.n = len(self.rec)

    def block(self, a, b):
        """Records a..b read into memory (one sequential read of each file), as `Records`."""
        return Block(self, a, b)


class Block(Records):
    def __init__(self, d, a, b):
        self.weight, self.one_view = d.weight, d.one_view
        self.rec = np.array(d.rec[a:b])
        idx = np.array(d.idx[a:b])
        b0, r0 = int(idx[0]['b']), int(idx[0]['r'])
        b1 = int(d.idx[b]['b']) if b < d.n else len(d.brd)
        r1 = int(d.idx[b]['r']) if b < d.n else len(d.rel)
        if len(d.rel) == 0:
            r0 = r1 = 0
        self.brd = np.array(d.brd[b0:b1])
        f0 = int(self.brd[0]['off']) if len(self.brd) else 0
        f1 = int(self.brd[-1]['off']) + (1 if d.one_view else 2) * int(self.brd[-1]['n']) if len(self.brd) else 0
        self.ftr = np.array(d.ftr[f0:f1])
        self.rel = np.array(d.rel[r0:r1])
        self.brd['off'] -= f0
        self.idx = np.empty(len(idx), dtype=IDX)
        self.idx['b'] = idx['b'] - b0
        self.idx['r'] = idx['r'] - r0 if len(d.rel) else 0
        self.n = b - a


def ranges(starts, counts):
    """The concatenated index ranges [s, s + c)."""
    counts = np.asarray(counts, dtype=np.int64)
    total = int(counts.sum())
    if total == 0:
        return np.zeros(0, dtype=np.int64)
    ends = np.cumsum(counts)
    base = np.repeat(np.asarray(starts, dtype=np.int64) - (ends - counts), counts)
    return base + np.arange(total, dtype=np.int64)


class Net(torch.nn.Module):
    def __init__(self, acc, emb, hidden, racc, use_dense, dense_mul, out_scale):
        super().__init__()
        self.acc, self.emb, self.hidden, self.racc, self.use_dense = acc, emb, hidden, racc, use_dense
        self.rel = torch.nn.EmbeddingBag(NR, max(racc, 1), mode='sum')  # unused when racc is 0
        self.rb = torch.nn.Parameter(torch.full((max(racc, 1),), 0.1))
        self.ft = torch.nn.EmbeddingBag(NB, acc, mode='sum')
        self.b1 = torch.nn.Parameter(torch.zeros(acc))
        self.l2 = torch.nn.Linear(2 * acc, emb)
        self.lh = torch.nn.Linear(ROLES * emb + ROLES + racc + ND, hidden)
        self.lo = torch.nn.Linear(hidden, 1)
        self.register_buffer('dense_mul', torch.as_tensor(dense_mul, dtype=torch.float32))
        self.out_scale = out_scale
        with torch.no_grad():
            self.ft.weight.normal_(0, 0.02)
            self.rel.weight.normal_(0, 0.05)
            self.b1.fill_(0.1)

    def forward(self, b):
        a_own = torch.clamp(self.ft(b['own'], b['offsets']) + self.b1, 0, 1)
        a_other = torch.clamp(self.ft(b['other'], b['offsets']) + self.b1, 0, 1)
        e = torch.clamp(self.l2(torch.cat([a_own, a_other], 1)), 0, 1)
        n = b['dense'].shape[0]
        slot = b['board_rec'] * ROLES + b['role']
        pooled = torch.zeros(n * ROLES, self.emb, device=e.device).index_add_(0, slot, e)
        counts = torch.zeros(n * ROLES, device=e.device).index_add_(0, slot, torch.ones_like(slot, dtype=torch.float32))
        r = torch.clamp(self.rel(b['rel'], b['rel_offsets']) + self.rb, 0, 1)[:, :self.racc]
        d = b['dense'] * self.dense_mul if self.use_dense else torch.zeros_like(b['dense'])
        x = torch.cat([pooled.view(n, ROLES * self.emb), counts.view(n, ROLES), r, d], 1)
        h = torch.clamp(self.lh(x), 0, 1)
        return self.lo(h).squeeze(1) * self.out_scale

    def clip(self):
        """Keep the weights inside what the engine's integers hold (i8 at scale 64 after the
        first layer; first-layer accumulators inside i16)."""
        with torch.no_grad():
            self.ft.weight.clamp_(-FT_MAX, FT_MAX)
            self.b1.clamp_(-FT_MAX, FT_MAX)
            self.rel.weight.clamp_(-200, 200)
            for w in (self.l2.weight, self.lh.weight):
                w.clamp_(-127 / 64, 127 / 64)

    def quantize(self):
        """The engine's integer model (src/engine/nnue.rs, KNNUE002), as numpy arrays."""
        g = lambda t: t.detach().cpu().double().numpy()
        r = lambda a: np.floor(a + 0.5).astype(np.int64)
        q = {
            'ft_w': r(g(self.ft.weight) * ONE).astype(np.int16),
            'ft_b': r(g(self.b1) * ONE).astype(np.int16),
            'l2_w': np.clip(r(g(self.l2.weight).T * 64), -128, 127).astype(np.int8),
            'l2_b': r(g(self.l2.bias) * ONE * 64).astype(np.int32),
            'rel_w': r(g(self.rel.weight)[:, :self.racc] * ONE).astype(np.int16),
            'rel_b': r(g(self.rb)[:self.racc] * ONE).astype(np.int32),
            'dense_m': r(g(self.dense_mul) * ONE * (1 << FIX)).astype(np.int32),
            'head_w': np.clip(r(g(self.lh.weight).T * 64), -128, 127).astype(np.int8),
            'head_b': r(g(self.lh.bias) * ONE * 64).astype(np.int32),
            'out_w': r(g(self.lo.weight).reshape(-1) * 64).astype(np.int32),
            'out_b': int(r(g(self.lo.bias) * ONE * 64)[0]),
            'out_q': int(r(np.array(self.out_scale / (ONE * 64) * (1 << FIX)))),
        }
        q.update(acc=self.acc, emb=self.emb, hidden=self.hidden, racc=self.racc, use_dense=self.use_dense)
        return q

    def to_bytes(self):
        return qbytes(self.quantize())


FT_MAX = 1.9       # |first-layer weight|: 128 features * 1.9 + bias stays inside i16 at scale ONE
ONE = 127          # nnue::ONE
SHIFT = 6          # nnue::SHIFT
FIX = 16           # nnue::FIX
MAX_FEATURES = 128  # nnue::MAX_BOARD_FEATURES


def qbytes(q):
    out = bytearray(MAGIC)
    out += struct.pack('<9I', NB, q['acc'], q['emb'], ROLES, ND, q['hidden'], NR, q['racc'], int(q['use_dense']))
    for k in ('ft_w', 'ft_b', 'l2_w', 'l2_b', 'rel_w', 'rel_b', 'dense_m', 'head_w', 'head_b', 'out_w'):
        out += np.ascontiguousarray(q[k]).astype(q[k].dtype.newbyteorder('<')).tobytes()
    out += struct.pack('<2i', q['out_b'], q['out_q'])
    return bytes(out)


def shr_round(x, bits):
    return (x + (1 << (bits - 1))) >> bits


def qforward(q, b):
    """The engine's integer evaluation of batch `b` (numpy, exact): what Rust computes."""
    acc, emb, hid = q['acc'], q['emb'], q['hidden']
    ftw = q['ft_w'].reshape(NB, acc).astype(np.int64)
    own, other, offsets = b['own'].numpy(), b['other'].numpy(), b['offsets'].numpy()
    nb = len(offsets)
    bounds = np.append(offsets, len(own))
    out_e = np.zeros((nb, emb), dtype=np.int64)
    for k in range(nb):
        a = []
        for feats in (own[bounds[k]:bounds[k + 1]], other[bounds[k]:bounds[k + 1]]):
            s = q['ft_b'].astype(np.int64) + ftw[feats].sum(0)
            assert np.all(np.abs(s) <= 32767), 'accumulator past i16'
            a.append(np.clip(s, 0, ONE))
        a = np.concatenate(a)
        s = q['l2_b'].astype(np.int64) + a @ q['l2_w'].reshape(2 * acc, emb).astype(np.int64)
        out_e[k] = np.clip(s >> SHIFT, 0, ONE)
    n = b['dense'].shape[0]
    rec, role = b['board_rec'].numpy(), b['role'].numpy()
    pooled = np.zeros((n, ROLES, emb), dtype=np.int64)
    counts = np.zeros((n, ROLES), dtype=np.int64)
    np.add.at(pooled, (rec, role), out_e)
    np.add.at(counts, (rec, role), 1)
    racc = q['racc']
    relw = q['rel_w'].reshape(NR, racc).astype(np.int64)
    rel, ro = b['rel'].numpy(), np.append(b['rel_offsets'].numpy(), len(b['rel']))
    ract = np.stack([np.clip(q['rel_b'].astype(np.int64) + relw[rel[ro[i]:ro[i + 1]]].sum(0), 0, ONE) for i in range(n)]) if racc else np.zeros((n, 0), dtype=np.int64)
    d = b['dense'].numpy().astype(np.int64)
    if not q['use_dense']:
        d = np.zeros_like(d)
    dq = np.where(d != 0, np.clip(shr_round(d * q['dense_m'].astype(np.int64), FIX), -32767, 32767), 0)
    x = np.concatenate([np.minimum(pooled.reshape(n, ROLES * emb), 32767), np.minimum(counts * ONE, 32767), ract, dq], 1)
    s = q['head_b'].astype(np.int64) + x @ q['head_w'].reshape(-1, hid).astype(np.int64)
    h = np.clip(s >> SHIFT, 0, ONE)
    o = q['out_b'] + h @ q['out_w'].astype(np.int64)
    return shr_round(o * q['out_q'], FIX)


def save(net, out):
    """The engine's file, and the float checkpoint (`out`.pt) to resume or check from."""
    open(out, 'wb').write(net.to_bytes())
    torch.save({'acc': net.acc, 'emb': net.emb, 'hidden': net.hidden, 'racc': net.racc, 'use_dense': net.use_dense, 'out_scale': net.out_scale,
                'state': net.state_dict()}, out + '.pt')


def load(path):
    c = torch.load(path, map_location='cpu')
    net = Net(c['acc'], c['emb'], c['hidden'], c['racc'], c['use_dense'], np.zeros(ND), c['out_scale'])
    net.load_state_dict(c['state'])
    return net


def to(b, dev):
    return {k: v.to(dev, non_blocking=True) for k, v in b.items()}


def target(head, k, lam):
    """The training target and which records have one: the teacher's expected score mixed with
    the result; either alone where the other is missing (NaN: no score, or a game cut off by
    the action limit); none where both are."""
    teacher, result = head[:, 0], head[:, 1]
    q = torch.sigmoid(teacher / k)
    no_t, no_r = torch.isnan(teacher), torch.isnan(result)
    y = torch.where(no_t, result, torch.where(no_r, q, lam * q + (1 - lam) * result))
    return torch.nan_to_num(y), ~(no_t & no_r)


BLOCK = 1 << 20  # records read at once (and shuffled among)


def blocks_of(d):
    return [(a, min(a + BLOCK, d.n)) for a in range(0, d.n, BLOCK)]


def is_val(rec, val_mod):
    return (rec['h'][:, 2].astype(np.int64) % val_mod) == 0


def dense_scale(data, n=200000):
    """1 / spread of each dense input over the first block's records of each dataset."""
    x = np.concatenate([np.asarray(d.rec['d'][:min(n, d.n)], dtype=np.float32) for d in data])
    return 1.0 / np.maximum(x.std(0), 1.0)


def prefetched(plan, load):
    """`load(item)` for each item of `plan`, the next one read in the background meanwhile."""
    with ThreadPoolExecutor(1) as ex:
        nxt = ex.submit(load, plan[0]) if plan else None
        for k in range(len(plan)):
            cur = nxt.result()
            nxt = ex.submit(load, plan[k + 1]) if k + 1 < len(plan) else None
            yield cur


def held_out(data, val_mod, limit):
    """The validation records (whole games) of every dataset, at most `limit`, as in-memory
    batches."""
    out, n = [], 0
    plan = [(d, a, b) for d in data for a, b in blocks_of(d)]
    for blk in prefetched(plan, lambda x: x[0].block(x[1], x[2])):
        rows = np.nonzero(is_val(blk.rec, val_mod))[0]
        for j in range(0, len(rows), 16384):
            if n >= limit:
                return out
            out.append(blk.batch(rows[j:j + 16384]))
            n += len(rows[j:j + 16384])
    return out


def whole(d, size=16384):
    """Every record of `d` as in-memory batches."""
    out = []
    for a, b in blocks_of(d):
        blk = d.block(a, b)
        out += [blk.batch(np.arange(j, min(j + size, blk.n))) for j in range(0, blk.n, size)]
    return out


def measure(net, batches, k, lam, dev):
    """The training loss (mixed target) and, against game results alone, the log loss and
    squared error: those do not depend on the mix, so they compare recipes."""
    net.eval()
    tot, w, ll, se, nr = 0.0, 0, 0.0, 0.0, 0
    with torch.no_grad():
        for b in batches:
            b = to(b, dev)
            p = torch.sigmoid(net(b) / k)
            y, ok = target(b['head'], k, lam)
            tot += float((((p - y) ** 2) * ok).sum())
            w += int(ok.sum())
            r = b['head'][:, 1]
            has = ~torch.isnan(r)
            pr, rr = p[has].clamp(1e-6, 1 - 1e-6), r[has]
            ll += float(-(rr * torch.log(pr) + (1 - rr) * torch.log(1 - pr)).sum())
            se += float(((pr - rr) ** 2).sum())
            nr += int(has.sum())
    net.train()
    return tot / max(w, 1), ll / max(nr, 1), se / max(nr, 1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('data', nargs='*')
    ap.add_argument('--out', default='games/nnue/kairos.nnue')
    ap.add_argument('--init', help='start from this checkpoint (the .pt saved next to --out)')
    ap.add_argument('--acc', type=int, default=128)
    ap.add_argument('--emb', type=int, default=32)
    ap.add_argument('--hidden', type=int, default=32)
    ap.add_argument('--racc', type=int, default=64, help='relation accumulator width')
    ap.add_argument('--no-dense', action='store_true', help='without the handcrafted counts (a pure NNUE)')
    ap.add_argument('--k', type=float, default=4350.0, help='internal units per logistic unit')
    ap.add_argument('--lam', type=float, default=0.9, help='teacher share of the target')
    ap.add_argument('--epochs', type=int, default=10)
    ap.add_argument('--batch', type=int, default=4096)
    ap.add_argument('--lr', type=float, default=1e-3)
    ap.add_argument('--val-mod', type=int, default=20)
    ap.add_argument('--val-limit', type=int, default=500000, help='most validation records')
    ap.add_argument('--test', help='a held-out export (prefix) measured each epoch against game results')
    ap.add_argument('--out-scale', type=float, default=1000.0)
    ap.add_argument('--seed', type=int, default=1)
    ap.add_argument('--device', help='cuda or cpu (default: cuda when available)')
    ap.add_argument('--parity', help='print the integer evaluation of --init (a .pt) of the first records of this prefix')
    ap.add_argument('--parity-n', type=int, default=20)
    args = ap.parse_args()
    dev = args.device or ('cuda' if torch.cuda.is_available() else 'cpu')
    if args.parity:
        net = load(args.init)
        d = Data(args.parity)
        for v in qforward(net.quantize(), d.batch(np.arange(min(args.parity_n, d.n)))):
            print(int(v))
        return
    torch.manual_seed(args.seed)
    rng = np.random.default_rng(args.seed)
    data = [Data(x, relations=args.racc > 0) for x in args.data]
    val = held_out(data, args.val_mod, args.val_limit)
    test = whole(Data(args.test, relations=args.racc > 0)) if args.test else []
    print('records', sum(d.n for d in data), 'validation', sum(len(b['head']) for b in val), 'test', sum(len(b['head']) for b in test), flush=True)
    if args.init:
        net = load(args.init)
    else:
        net = Net(args.acc, args.emb, args.hidden, args.racc, not args.no_dense, dense_scale(data), args.out_scale)
    net.to(dev)
    opt = torch.optim.AdamW(net.parameters(), lr=args.lr, weight_decay=1e-5)
    steps = args.epochs * sum(math.ceil(d.n / args.batch) for d in data)
    sched = torch.optim.lr_scheduler.OneCycleLR(opt, max_lr=args.lr, total_steps=max(steps, 1), pct_start=0.05)

    def report(ep, extra=''):
        v = measure(net, val, args.k, args.lam, dev)
        line = f'epoch {ep}{extra} val {v[0]:.6f} val_result_ll {v[1]:.5f}'
        if test:
            t = measure(net, test, args.k, args.lam, dev)
            line += f' test_result_ll {t[1]:.5f} test_result_mse {t[2]:.5f}'
        return v[0], line

    best, line = report(0)
    print(line, flush=True)
    save(net, args.out)
    for ep in range(1, args.epochs + 1):
        tot, n, t0 = 0.0, 0, time.time()
        plan = [(d, a, b) for d in data for a, b in blocks_of(d)]
        rng.shuffle(plan)
        for blk in prefetched(plan, lambda x: x[0].block(x[1], x[2])):
            rows = rng.permutation(np.nonzero(~is_val(blk.rec, args.val_mod))[0])
            for j in range(0, len(rows), args.batch):
                r = np.sort(rows[j:j + args.batch])
                b = to(blk.batch(r), dev)
                p = torch.sigmoid(net(b) / args.k)
                y, ok = target(b['head'], args.k, args.lam)
                loss = blk.weight * (((p - y) ** 2) * ok).sum() / ok.sum().clamp(min=1)
                opt.zero_grad(set_to_none=True)
                loss.backward()
                opt.step()
                net.clip()
                if sched.last_epoch + 1 < steps:
                    sched.step()
                tot += float(loss.detach()) * len(r)
                n += len(r)
        rate = n / max(time.time() - t0, 1e-9)
        v, line = report(ep, f' train {tot / max(n, 1):.6f} ({rate:.0f}/s)')
        if v < best:
            best = v
            save(net, args.out)
            line += ' saved'
        print(line, flush=True)


if __name__ == '__main__':
    main()
