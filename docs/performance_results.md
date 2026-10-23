# Oracle-preserving performance experiments

2026-10-22, Ryzen 7 5800X / Windows, native release build, rustc
`1.93.0-nightly (646a3f8c1 2025-12-02)`. Initial engine baseline: `4461ca6`.
Embedded NNUE SHA-256:
`6afdc42781243db129fdefd566f374f4fe1278d359b2becf6789a3a4302c8aa0`.

Initial measurements use `scripts/performance/pairs.py`: 12 sequential AB/BA pairs
after warmup, main corpus at 100k nodes per position and timeline corpus at 10k.
The first two experiments used logical CPU 4; later runs use CPU 12 and above-normal
child priority. The frozen evaluation blend was 50%. Values are paired
geometric-mean NPS gains; intervals are estimated 95%
Student-t intervals over pair log ratios. Root preparation is included. No SPRT.

| Candidate | Main gain | Timeline gain | Combined gain (95% interval) | Decision |
|---|---:|---:|---|---|
| Checking-source masks instead of cached move lists | −0.66% | −0.76% | −0.69% (−3.94% to +2.66%) | Reverted; gain not established |
| Fewer HC copies (24-pair confirmation) | +0.42% | −3.03% | −1.14% (−4.48% to +2.32%) | Initially reverted; later restored at user direction after cycle retest |
| Reuse node action batches | −0.44% | −1.25% | −0.82% (−1.80% to +0.17%) | Reverted; timeline regression |
| Cache compound-reach queries | −4.70% | +0.90% | −2.24% (−4.50% to +0.07%) | Reverted; main-corpus regression |
| Cache complete local capture gains (24-pair confirmation) | +1.98% | +0.59% | +1.44% (−0.72% to +3.65%) | Reverted; gain not established |
| Use graph validity instead of clearing edge references | −2.03% | +2.02% | −0.30% (−2.35% to +1.79%) | Reverted; gain not established |
| Bulk-copy spilled action assignments | −3.28% | −1.78% | −2.67% (−4.09% to −1.24%) | Reverted; regression |
| Double HCE term cache to 8,192 slots | +0.54% | −0.41% | +0.13% (−2.35% to +2.67%) | Reverted; gain not established |
| Extract NNUE features once for both views | +1.19% | −1.75% | −0.13% (−1.72% to +1.48%) | Reverted; gain not established |
| Bulk destination-region merge (24-pair confirmation) | −0.07% | −0.90% | −0.44% (−1.60% to +0.73%) | Reverted; gain not confirmed |
| Fresh HCE proof of zero local gain (blend 65) | −1.54% | −0.33% | −1.02% (−2.30% to +0.27%) | Reverted; no gain in NPS or subsequent process-cycle run |
| Check filter for timelines ending on the opponent's turn + per-departure discovered-check cache (2026-10-23) | +0.34% | −2.59% | −0.40% (−3.04% to +2.30%) | Not merged; mate-suite profile showed 27% of check scans skipped, but the filter's own cost offsets it |

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

Bulk action assignment passed 108 tests (two existing ignores), including inline,
spill, growth and retained-capacity transitions, clones, and zero-filled TT
storage. Ordered generation, perft and all search reports matched, but throughput
regressed. Raw output: `target/perf-series/07-bulk-actions-pairs.json`; patch:
`target/perf-series/07-bulk-actions.patch`.

The larger HCE cache passed 108 tests (two existing ignores), direct-value checks
over all built-in variants and corpus boards, forced collisions, size changes and
upper-half index coverage. Extended checks on five representative positions up
to 16 active timelines matched full results across warm searches, MultiPV 3, two
frozen networks and HCE, including model switches. At skill 4, deterministic
candidate reports/nodes/proof matched; its existing randomized final choice and
chosen score were excluded. The synthetic perft stress search was abandoned
after an excessively long baseline root scan, not counted as a pass. The extra
3.19 MiB per thread did not establish a throughput gain. Raw outputs:
`target/perf-series/08-hce-cache-pairs.json` and `08-oracle-matrix.json`; patch:
`target/perf-series/08-hce-cache.patch`.

The NNUE view transform passed 108 tests (two existing ignores), including cold,
incremental and reverse accumulators against separate extraction of both views
over corpus histories and every built-in variant. All timed search reports and
oracles matched. Raw output: `target/perf-series/09-nnue-views-pairs.json`; patch:
`target/perf-series/09-nnue-views.patch`.

Bulk region merging passed 108 tests (two existing ignores), 1,024 randomized
merge comparisons, the same ordered action dump and perft, and every search
report/oracle comparison. The first 12 pairs suggested +1.68% (−0.31% to +3.72%);
the separate 24-pair confirmation did not reproduce it. Raw outputs:
`target/perf-series/10-region-merge-pairs.json` and `10-region-merge-confirm.json`;
patch: `target/perf-series/10-region-merge.patch`.

After concurrent commit `84895d2` changed the default blend from 50% to 65% and
the network, local capture caching was retested with both explicitly frozen:
network SHA-256 `b8aa980a730d77be7ce14283becd0586586f97f33cd236f1a2d7a46b8ebb1fee`,
blend 65. Twelve longer pairs used 500k/50k node caps (4,500,000/385,938 actual
nodes), giving main +1.80%, timelines −0.77%, combined +0.57% (−0.37% to +1.51%).
All traces matched, but there was still no established combined gain, so the
cache was reverted again. This baseline executable's explicit evaluation options
reproduce the new configuration; engine changes in `84895d2` are only defaults.
Raw output: `target/perf-series/05-local-current-long.json`; current-source patch:
`target/perf-series/05-local-current.patch`.

The fresh-HCE zero-gain shortcut reused existing attack maps to prove there were
no local captures or quiet promotions. Its proof was consumed immediately after
a fresh evaluation and cleared on evaluation-cache hits. It passed 111 tests
(two existing ignores), exhaustive single-piece/target checks, feature equality,
and extended oracle checks across MultiPV, warm searches, two frozen networks,
model switches, HCE, and blends 65/100. At skill 4 the deterministic reports,
nodes and proof matched; the existing randomized final selection was excluded.
The extended matrix output hashes are recorded in
`target/perf-series/11-oracle-matrix.json`.

Both binaries loaded the third frozen network above at blend 65. All 12 NPS
pairs matched main `b880243d945d51cd` / 900,000 nodes and timeline
`9849dd738e5d0d59` / 83,213 nodes, including every normalized report. Combined
NPS was −1.02% (−2.30% to +0.27%). A second, separately reported 12-pair run
collected Windows process cycles as well: main NPS −2.09%, timelines −0.83%,
combined −1.58% (−3.46% to +0.34%). Nodes per accounted process cycle were
−2.05%, −0.75%, and −1.52% combined (−3.39% to +0.39%), respectively.
All oracle comparisons passed in that run too. No gain was established, so the
engine patch was reverted. Raw outputs: `target/perf-series/11-local-zero-pairs.json`
and `11-local-zero-cycles.json`; saved patch: `11-local-zero.patch`.

Cycle collection uses `QueryProcessCycleTime` after process exit, with no engine
instrumentation. It includes initialization, model loading, output, and all
process threads in user/kernel mode, while NPS uses the engine's search interval.
It is Windows process cycle accounting, not the PMU `TotalCycles` event or retired
instructions. A four-pair identical-binary control gave NPS +0.47% (−5.07% to
+6.34%) and nodes per accounted cycle +0.52% (−4.84% to +6.18%); both are consistent
with zero. This short control showed no clear precision advantage over NPS.
Raw control: `target/perf-series/aa-process-cycles.json`.

`xperf -pmcsources` exposes `TotalCycles` and `InstructionRetired` on this host,
but starting a strict ETW PMC recording returned `Access is denied (0x5)`.
The current token is not an administrator and has no `SeSystemProfilePrivilege`.
No hardware-counter results are claimed; no tracing session was left running.
The runner retains optional process-cycle counts, paired intervals, explicit
blend selection and raw position timings. `scripts/performance/oracle.rs` retains
the broader deterministic equivalence checks for future candidates.

A quick process-cycle retest screened the three earlier candidates with the
strongest initial positive results. Each used six AB/BA pairs after warmup, CPU
12, above-normal priority, the frozen third network at blend 65, and the default
100k/10k node caps. The saved executables' SHA-256 hashes matched their earlier
result records; no rebuilding or engine source changes were needed. Every one
of the 84 runs, including warmups, matched its suite's checksum, node count and
normalized reports (main `b880243d945d51cd` / 900,000 nodes; timeline
`9849dd738e5d0d59` / 83,213 nodes).

| Quick retest | Combined NPS gain | Process cycles/node change (95% interval; negative is better) |
|---|---:|---|
| Local-capture cache | −0.55% | +0.54% (−3.62% to +4.89%) |
| Reduced HC copying | +1.22% | −1.14% (−3.01% to +0.77%) |
| Bulk region merging | −0.30% | +0.31% (−1.44% to +2.08%) |

Reduced HC copying was positive on both suites: main NPS +1.47%, timelines
+0.86%; nodes per accounted process cycle +1.40% and +0.80%. It is the most
promising of this quick screen, but its combined interval still spans zero.
All three combined intervals span zero, and process accounting showed no clear
precision advantage over NPS here. These are separate short-run estimates;
no engine patch was restored or accepted from this screen.
Raw outputs: `target/perf-series/05-local-cycle-retest.json`,
`02-hc-cycle-retest.json`, and `10-region-cycle-retest.json`. Converted cycle/node
intervals and per-suite summaries are saved in `quick-cycle-retests-summary.json`.

At the user's direction, reduced HC copying is now accepted despite the remaining
measurement uncertainty. It removes the redundant clone of an owned HC before
splitting, skips slice differences known to be empty, and reuses clone-from
storage. A fresh release build on current main passed all 108 tests (two existing
ignores), the exact 17,691-line ordered action dump and perft oracle above, and
all full-corpus search checks in `target/perf-series/12-hc-accepted-oracle.json`.
The latter two-pair run validates the rebuilt executable; the six-pair retest
above remains the adoption evidence, not a proven confidence-bound speed claim.
Subsequent experiments compare against this HC-enabled baseline, with the same
frozen network and blend 65.

Remaining audit items are tested against the accepted HC implementation. Each
candidate uses six sequential AB/BA pairs, the same frozen third network and blend
65, CPU 12 and above-normal priority, with full oracle/trace comparisons. At the
user's direction, the process-cycle point estimate breaks ties when the NPS
interval includes zero; acceptance on that basis retains measurement uncertainty.
Concurrent work is changing NNUE architecture, so these builds use source frozen
at `327251d` plus accepted performance patches. This isolates each experiment.
Process counts remain Windows accounting, not PMU hardware cycles.

| Remaining candidate | Combined NPS gain | Process cycles/node change (95%; negative better) | Decision |
|---|---:|---|---|
| Root/skipped storage reuse | -2.10% | +2.07% (-1.49% to +5.75%) | Reverted |
| Axis universe/subset proof | -1.05% | +0.99% (-0.09% to +2.08%) | Reverted |
| Double NNUE board cache | -0.03% | +0.04% (-3.20% to +3.38%) | Reverted |
| Cache standard-chess terms | -0.01% | +0.02% (-1.20% to +1.26%) | Reverted |
| Exclusively owned single-thread TT | -0.34% | +0.42% (-2.44% to +3.37%) | Reverted |
<!-- remaining-experiments-details -->

Root/skipped storage reuse: All 109 tests passed (two existing ignores), including stable tie/missing-score permutations and heap ownership. MultiPV, HCE/model switches, warm searches, skill 4 and explicit resume matched the HC baseline. Timeline NPS and cycle throughput intervals were negative. Raw results: `target/perf-series/13-root-storage-pairs.json`.

Axis universe/subset proof: All 109 tests passed (two existing ignores), including original-bound comparisons over 768 contexts with shared/natural orders, ties, empties, outside coordinates and word boundaries. Ordered action dump, perft and extended search/resume oracles matched. Both combined estimates were negative. Raw results: `target/perf-series/14-axis-bounds-pairs.json`.

Double NNUE board cache: All 109 tests passed (two existing ignores), including direct/cached values over corpus positions and every variant, forced key/dimension mismatches and cache clears. Model switches, MultiPV, skill and resumed-search oracles matched. NPS was -0.03%; accounted-cycle throughput was -0.04%, providing no reason to retain the extra cache memory. Raw results: `target/perf-series/15-nnue-cache-pairs.json`.

Cache standard-chess terms: All 110 tests passed (two existing ignores), including forced slot collisions, dimensions, standalone requests, full Sides equality and lazy allocation on multi-timeline positions. Extended model/MultiPV/skill/resume oracles matched. NPS (-0.005%) and accounted-cycle throughput (-0.018%) were effectively flat, with no cycle advantage to retain the bookkeeping. Raw results: `target/perf-series/16-chess-cache-pairs.json`.

Exclusively owned single-thread TT: All 111 tests passed (two existing ignores) on frozen source and current workspace, including exact shared/owned replacement, spills under four-thread contention, reentrant probes, Arc ownership, thread changes and hash replacement. MultiPV/model/HCE/skill/resume search oracles matched. Combined NPS and accounted-cycle throughput point estimates were both negative. Raw results: `target/perf-series/17-owned-tt-pairs.json`.

All five remaining experiments matched the suite oracle and every normalized
report in all 140 runs, including warmups. Each also matched the extended probe
with MultiPV 3, HCE/network switches, warm searches, an explicit resume one ply
deeper, and deterministic skill-4 reports. The probe now performs this explicit
third resume pass; the previous two passes exercised cold/warm state only.
The final source after reverting these five candidates passes 108 tests with two
existing ignores. All twelve audit areas have now had measured attempts. Only
reduced HC copying was retained in this campaign, at the user's direction; the
other variants did not show a combined process-cycle advantage in their screens.
The saved patches and complete measurements remain under `target/perf-series/`.
