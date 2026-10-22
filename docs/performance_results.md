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
