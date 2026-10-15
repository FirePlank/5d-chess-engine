#!/usr/bin/env python3
"""Pentanomial SPRT math over the harness's JSONL game files (same formulas as src/bin/sprt.rs).

    sprt_merge.py --counts shard.jsonl                     -> "LL,LD,WL+DD,WD,WW"
    sprt_merge.py --from-counts a,b,c,d,e --elo0 0 --elo1 5 -> llr=/pairs=/stop= lines
    sprt_merge.py --out merged.jsonl [--label X] [--old SHA] shard1.jsonl ...  -> Final Summary

Games 2k and 2k+1 of one file form pair k; each shard file numbers its pairs from 0.
"""
import argparse
import json
import math


def itp(f, a, b, f_a, f_b, k1, k2, n0, eps):
    if f_a > 0:
        a, b, f_a, f_b = b, a, f_b, f_a
    n_max = math.ceil(math.log2(abs(b - a) / (2 * eps))) + n0
    i = 0.0
    while abs(b - a) > 2 * eps:
        x_half = (a + b) / 2
        r = eps * 2 ** (n_max - i) - (b - a) / 2
        delta = k1 * abs(b - a) ** k2
        x_f = (f_b * a - f_a * b) / (f_b - f_a)
        sigma = math.copysign(1.0, x_half - x_f)
        x_t = x_f + sigma * delta if delta <= abs(x_half - x_f) else x_half
        x_itp = x_t if abs(x_t - x_half) <= r else x_half - sigma * r
        f_itp = f(x_itp)
        if f_itp == 0:
            a = b = x_itp
        elif f_itp < 0:
            a, f_a = x_itp, f_itp
        else:
            b, f_b = x_itp, f_itp
        i += 1
    return (a + b) / 2


def mle_logistic(scores, probs, s):
    n = len(scores)
    theta = itp(lambda x: sum(probs[i] * (scores[i] - s) / (1 + x * (scores[i] - s)) for i in range(n)),
                -1 / (scores[-1] - s), -1 / (scores[0] - s), math.inf, -math.inf, 0.1, 2.0, 0.99, 1e-3)
    return [probs[i] / (1 + theta * (scores[i] - s)) for i in range(n)]


def mle_normalized(scores, probs, mu_ref, t_star):
    n = len(scores)
    p = [1 / n] * n
    for _ in range(10):
        mu = sum(scores[i] * p[i] for i in range(n))
        sigma = math.sqrt(sum(p[i] * (scores[i] - mu) ** 2 for i in range(n)))
        phi = [scores[i] - mu_ref - 0.5 * t_star * sigma * (1 + ((scores[i] - mu) / sigma) ** 2) for i in range(n)]
        theta = itp(lambda x: sum(probs[i] * phi[i] / (1 + x * phi[i]) for i in range(n)),
                    -1 / max(phi), -1 / min(phi), math.inf, -math.inf, 0.1, 2.0, 0.99, 1e-7)
        new = [probs[i] / (1 + theta * phi[i]) for i in range(n)]
        done = max(abs(new[i] - p[i]) for i in range(n)) < 1e-4
        p = new
        if done:
            break
    return p


def score_to_elo(s):
    s = min(max(s, 1e-9), 1 - 1e-9)
    return -400 * math.log10(1 / s - 1)


def llr(counts, elo0, elo1, model="normalized"):
    if sum(counts) == 0:
        return 0.0
    reg = [c if c > 0 else 1e-3 for c in counts]
    total = sum(reg)
    probs = [c / total for c in reg]
    scores = [0.0, 0.25, 0.5, 0.75, 1.0]
    if model == "normalized":
        k = math.sqrt(2) / (800 / math.log(10))
        p0 = mle_normalized(scores, probs, 0.5, k * elo0)
        p1 = mle_normalized(scores, probs, 0.5, k * elo1)
    else:
        e2s = lambda e: 1 / (1 + 10 ** (-e / 400))
        p0 = mle_logistic(scores, probs, e2s(elo0))
        p1 = mle_logistic(scores, probs, e2s(elo1))
    return total * sum(probs[i] * (math.log(p1[i]) - math.log(p0[i])) for i in range(5))


def estimate(counts):
    n = sum(counts)
    if n == 0:
        return 0, 0, 0, 0
    vals = [0, 0.25, 0.5, 0.75, 1.0]
    s = sum(v * c for v, c in zip(vals, counts)) / n
    var = sum(c * (v - s) ** 2 for v, c in zip(vals, counts)) / n
    ci = 1.959963984540054 * math.sqrt(var / n)
    elo, err = score_to_elo(s), (score_to_elo(s + ci) - score_to_elo(s - ci)) / 2
    if var <= 0:
        return elo, err, 0, 0
    s2n = lambda x: (x - 0.5) / math.sqrt(2 * var) * (800 / math.log(10))
    return elo, err, s2n(s), (s2n(s + ci) - s2n(s - ci)) / 2


def read_games(path):
    games = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                games.append(json.loads(line))
    return games


def pair_counts(games):
    pairs = {}
    for g in games:
        pairs.setdefault(g["pair"], []).append(g["score"])
    c = [0, 0, 0, 0, 0]
    for scores in pairs.values():
        if len(scores) == 2:
            c[int(round(2 * sum(scores)))] += 1
    return c


def bounds(alpha=0.05, beta=0.05):
    return math.log(beta / (1 - alpha)), math.log((1 - beta) / alpha)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--counts")
    ap.add_argument("--from-counts")
    ap.add_argument("--elo0", type=float, default=0.0)
    ap.add_argument("--elo1", type=float, default=5.0)
    ap.add_argument("--model", default="normalized")
    ap.add_argument("--out")
    ap.add_argument("--label", default="new")
    ap.add_argument("--old", default="old")
    ap.add_argument("files", nargs="*")
    a = ap.parse_args()
    lo, hi = bounds()
    if a.counts:
        try:
            print(",".join(map(str, pair_counts(read_games(a.counts)))))
        except FileNotFoundError:
            print("0,0,0,0,0")
        return
    if a.from_counts:
        c = [int(x) for x in a.from_counts.split(",")]
        l = llr(c, a.elo0, a.elo1, a.model)
        elo, err, nelo, nerr = estimate(c)
        print(f"llr={l:.3f}\npairs={sum(c)}\nstop={'true' if l >= hi or l <= lo else 'false'}")
        print(f"elo={elo:+.1f}+-{err:.1f} nelo={nelo:+.1f}+-{nerr:.1f} penta={c}")
        return
    merged, counts, reasons, wdl = [], [0] * 5, {}, [0, 0, 0]
    for path in a.files:
        games = read_games(path)
        for g in games:
            reasons[g["reason"].split(" (")[0]] = reasons.get(g["reason"].split(" (")[0], 0) + 1
            wdl[{1.0: 0, 0.5: 1, 0.0: 2}[float(g["score"])]] += 1
        counts = [x + y for x, y in zip(counts, pair_counts(games))]
        merged += games
    if a.out:
        with open(a.out, "w") as f:
            for g in merged:
                f.write(json.dumps(g) + "\n")
    l = llr(counts, a.elo0, a.elo1, a.model)
    elo, err, nelo, nerr = estimate(counts)
    verdict = "H1 ACCEPTED" if l >= hi else "H0 ACCEPTED" if l <= lo else "INCONCLUSIVE"
    print("==== Final Summary ====")
    print(f"Test: {a.label} vs {a.old}")
    print(f"Elo: {elo:+.1f} +/- {err:.1f}, nElo: {nelo:+.1f} +/- {nerr:.1f}")
    print(f"Record: {wdl[0]}W - {wdl[2]}L - {wdl[1]}D ({sum(wdl)} total)")
    print(f"Pentanomial [LL LD WL+DD WD WW]: {counts}")
    print(f"LLR: {l:+.3f} [{lo:.3f}, {hi:.3f}] ({a.elo0}, {a.elo1}) => {verdict}")
    print(f"Termination: {reasons}")


if __name__ == "__main__":
    main()
