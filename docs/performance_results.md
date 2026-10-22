# Oracle-preserving performance experiments

2026-10-22, Ryzen 7 5800X / Windows, native release build, rustc
`1.93.0-nightly (646a3f8c1 2025-12-02)`. Initial engine baseline: `4461ca6`.
Embedded NNUE SHA-256:
`6afdc42781243db129fdefd566f374f4fe1278d359b2becf6789a3a4302c8aa0`.

Measurements use `scripts/performance/pairs.py`: 12 sequential AB/BA pairs after
warmup, logical CPU 4, main corpus at 100k nodes per position and timeline corpus
at 10k. Values are paired geometric-mean NPS gains; intervals are estimated 95%
Student-t intervals over pair log ratios. Root preparation is included. No SPRT.

| Candidate | Main gain | Timeline gain | Combined gain (95% interval) | Decision |
|---|---:|---:|---|---|
| Checking-source masks instead of cached move lists | −0.66% | −0.76% | −0.69% (−3.94% to +2.66%) | Reverted; gain not established |
| Fewer HC copies (24-pair confirmation) | +0.42% | −3.03% | −1.14% (−4.48% to +2.32%) | Reverted; gain not confirmed |
| Reuse node action batches | −0.44% | −1.25% | −0.82% (−1.80% to +0.17%) | Reverted; timeline regression |
| Cache compound-reach queries | −4.70% | +0.90% | −2.24% (−4.50% to +0.07%) | Reverted; main-corpus regression |
| Cache complete local capture gains (24-pair confirmation) | +1.98% | +0.59% | +1.44% (−0.72% to +3.65%) | Reverted; gain not established |
| Use graph validity instead of clearing edge references | −2.03% | +2.02% | −0.30% (−2.35% to +1.79%) | Reverted; gain not established |

The source-mask experiment passed 108 tests (two existing ignores), a differential
mask-versus-check-list test, and exact ordered action dumps across the perft,
main and timeline corpora (17,691 output lines, SHA-256
`4f8875730cbc0df30f5737303b04c0a8a806c4dc58e1c2285834f80335074a1f`).
Perft oracle remained `c3d0858de3887e52`. All search reports and oracle fields
matched: main `a769e38ebf311b25` / 900,000 nodes, timelines
`6f555e8ece964abd` / 83,188 nodes.

Local raw results: `target/perf-series/01-source-masks-pairs.json`.
The rejected patch is saved locally as `target/perf-series/01-source-masks.patch`.
These target files are ignored; the table records the outcome without merging
an unproven engine change. Benchmark support and the timeline corpus are retained
for subsequent experiments.

The HC-copy experiment added consuming split APIs, skipped copies whose difference
was empty, and reused clone-from storage. It passed the full suite, 2,048 randomized
ordered split comparisons, the same ordered generation dump and the same search
oracles. Its initial 12 pairs suggested +2.05% (−0.15% to +4.30%), but a separate
24-pair confirmation did not establish a gain, so it was not merged. Raw outputs:
`target/perf-series/02-hc-copy-pairs.json` and `02-hc-copy-confirm.json`; rejected
patch: `target/perf-series/02-hc-copy.patch`.

A six-pair A/A control at above-normal child priority on logical CPU 12 gave
−1.24% (−3.57% to +1.15%), consistent with zero. Later experiments use that
priority/CPU setting, recorded in each raw result, to reduce ordinary desktop
preemption. It does not eliminate measurement noise. Control output:
`target/perf-series/aa-above-normal.json`.

The node-batch experiment passed all 107 tests (two existing ignores) and all
normalized search/oracle comparisons, but its timeline interval was entirely
negative (−2.10% to −0.39%). It was reverted. Raw output:
`target/perf-series/03-batches-pairs.json`; patch:
`target/perf-series/03-batches.patch`. From this experiment onward, both binaries
load the frozen initial NNUE explicitly, since further NNUE training updates are
being committed concurrently. No training files are modified by this work.

The compound-reach cache passed 108 tests (two existing ignores), including
reference queries for every square, all fairy-piece kinds and both colors, and
all search trace comparisons. Its main-corpus interval was negative (−7.79% to
−1.50%), so it was reverted. Raw output:
`target/perf-series/04-reach-cache-pairs.json`; patch:
`target/perf-series/04-reach-cache.patch`.

The local-capture cache passed 109 tests (two existing ignores), including exact
uncached results, forced collisions, promotions, board heights, colors and
different immutable material values. Every search trace matched. The first 12
pairs suggested +1.62% (−1.30% to +4.61%); a separate 24-pair confirmation still
included zero, so the patch was not merged. Raw outputs:
`target/perf-series/05-local-cache-pairs.json` and `05-local-cache-confirm.json`;
patch: `target/perf-series/05-local-cache.patch`.

The edge-reference experiment passed 109 tests (two existing ignores), including
the original dense-reset routine versus scratch reuse on randomized HC restrictions
in both axis orders, and graph validity across 64-bit word/stride boundaries.
Ordered action dumps and perft matched. Search reports matched at node caps 2k/20k
and at uncapped depth 2, in addition to every timed pair. Raw output:
`target/perf-series/06-edge-matrix-pairs.json`; patch:
`target/perf-series/06-edge-matrix.patch`.
