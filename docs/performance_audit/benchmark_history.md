# Historical benchmark findings

This records the original 2026-10-21–22 audit of
`cf055d3d2d13c76b6eb03193b1c16aabfc77fc89`. The three benchmark repairs below
were implemented on 2026-10-22. The original proposed remedies are retained as
historical context, not outstanding work. Current usage and v2 validation are in
`docs/performance_audit.md`; the regular profiler attribution cleanup remains
separately listed there.

The live checkout changed during the original audit, so its diagnostic builds
used a frozen git archive. Subsequent benchmark-repair tests used the newer
`1f10990` engine plus the working-tree repair. The profiles were not rerun.

## Original findings — resolved

### 1. Fixed nodes do not currently mean fixed work

`src/engine/ab.rs:2042` (`root_mate_budget`) assigns the default analysis root scan
a **1,000 ms deadline**, even when `Limits` contains only depth and node limits.
`abbench` never overrides `MateVerify`. The root scan performs work outside the
ordinary node budget. A faster binary can examine different root-scan actions,
populate different memo entries, or discover a mate the slower run misses.

This showed up on the **same frozen executable**: two 20k-node runs both returned
stock oracle `e82287ff973edc82` and 123,808 reported nodes, but their `take_point`
counts were **155,246 versus 154,430**, and `find_checks` counts were
**100,841 versus 100,285**. Equal stock oracle did not mean equal work.

Add an explicitly benchmark-only counted root-mate budget: fixed action, step,
and recheck limits with no deadline. Apply the same settings to base and candidate
and record them in the result. Do not change the production `Auto` policy as part
of a speed patch. `MateVerify=exhaustive` already removes the deadline without a
clock, but can be impractical on difficult positions and is not a bounded runner.
`MateVerify=0` is **Auto**, not disabled.

Keep a separate default-Auto wall-clock benchmark for actual root latency. Its
results are useful, but cannot certify equal work.

### 2. The NPS numerator is stale

`src/bin/ktools.rs:1487` extracts `nodes` from the last info string. Info is emitted
after completed depths (`src/engine/ab.rs:3895`), while a node cap can stop partway
through the next depth. Nodes from that partial depth are missing from the
numerator but present in elapsed time.

On the 20k corpus, stock `abbench` reported **123,808**, while
`AlphaBeta::last_search_nodes()` reported **180,000**. That understates nodes by
31.2%. The independent probe produced the same expanded search trace hash
`0299ba183744930d` on two runs, although this does not remove the root-scan issue.

Use `last_search_nodes()` for total work. Preserve completed-depth node counts as
additional oracle fields. Report root preparation time, search time and total
wall time separately where practical; label NPS that includes preparation and
engine setup accordingly. Positions with zero search nodes need latency reporting.

### 3. Strengthen the equivalence signature

Stock `abbench` hashes only the returned move and the last reported node count.
Stock `bench` hashes capped per-position leaf counts; it is depth 2, cap 50, not
an exhaustive legal-action or search trace oracle.

For each position compare actual total nodes, returned action, final internal
score, mate-proof flag, completed depth, and every completed-depth score/PV/node
record. Strip time and NPS fields. For generation changes also compare ordered
actions, rejected-region behavior where relevant, and budget exhaustion points.
Do not just compare unordered legal-move sets: selective search depends on order.

The saved `docs/performance_audit/probe.rs` demonstrates the richer public-API
signature and actual node counting. It still uses the existing mate policy; it
is **not** a finished deterministic benchmark replacement.

## Reproducing the original audit

The stock baseline hashes observed were:

```text
ktools bench data/bench.g
oracle c3d0858de3887e52 | perft leaves 21813

AB_NODES=20000 ktools abbench data/bench_ab.g 20
aboracle e82287ff973edc82 | reported nodes 123808

probe data/bench_ab.g 20000 20
trace 0299ba183744930d | actual nodes 180000
```

The `AB_NODES=...` line above denotes an environment setting; in PowerShell use
`$env:AB_NODES='20000'`. The probe hashes more fields and is a different oracle.

Create a fresh archive with `git archive cf055d3 --format=zip --output=...` and
extract it into a folder named `instrumented`. Run the saved
`docs/performance_audit/instrument.py <that-folder>` once, then build its `ktools`
with `--release --features prof` into a separate target directory. This script
adds diagnostic timings and observer-only cache counters; it never substitutes
cached results. Events 0/1 are chess hit/miss; 2/3 are local-gain hit/miss.

Run `abbench` at depth ceiling 20 with `AB_NODES=200000` on the archived main
corpus and `AB_NODES=2000` on the saved timeline fixture. Use the archived NNUE
and residual files, not whichever versions are now in the live checkout.

For the probe, copy `docs/performance_audit/probe.rs` to `src/bin/auditprobe.rs`
**inside an isolated baseline archive**, then `cargo build --release --bin
auditprobe`. It accepts `file.g [node-cap] [depth] [MateVerify-value|inventory]`.
The `inventory` mode reports timeline and stored-board counts without search.
Standalone linking against a release rlib also works with matching CPU/panic
settings and `-C lto=fat`; these release rlibs contain LLVM bitcode.

The frozen builds, stock benchmarks, richer probe repetitions and diagnostic
profiles succeeded during the original audit. The later benchmark repair passed
the full test suite and the v2 A/A checks documented in the current audit.
No candidate speedup A/B test has been run.
