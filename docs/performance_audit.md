# Oracle-preserving performance audit

Updated 2026-10-22. **The benchmark repair is complete; implementation experiments
are underway, one candidate at a time.**

Eleven experiments preserved the oracle. Reduced HC copying was restored and
accepted at the user's direction after its process-cycle retest; ten other
patches remain reverted, including local capture caching at longer budgets.
The fresh-HCE zero-gain shortcut also failed its NPS and process-cycle comparisons.
The runner now optionally records Windows process cycles per node alongside NPS;
actual ETW hardware-counter recording was denied by this session's privileges.
Measured outcomes and uncertainty are recorded in
[performance_results.md](performance_results.md). The queue below retains each
candidate's rationale and required invariants; it is not a list of proven wins.

Candidate source references and diagnostic profiles use the original frozen
baseline `cf055d3d2d13c76b6eb03193b1c16aabfc77fc89`. The repaired benchmark was
validated against `1f109900dd262168feee8b99878fcdc40ae8df90` plus the benchmark
repair in the working tree. Those historical profiles established priorities;
new implementation measurements use frozen baseline 4461ca6, the repaired
v2 benchmark and an explicitly frozen NNUE. No SPRT was run.

The current benchmark settings and verified v2 baselines are recorded below.
The resolved benchmark findings are retained in
`docs/performance_audit/benchmark_history.md` for reference.

## Ranked implementation queue

Priority combines proof simplicity, measured hot-path exposure, and likely scope.
Every row is a candidate requiring its own equivalence check and speed pairs.

| Order | Candidate | Main target | Scope / equivalence risk |
|---:|---|---|---|
| 1 | Checker-source bitmasks | Many-timeline legality | Small; direct Boolean identity |
| 2 | Avoid unnecessary HC copies | Candidate splitting | Small–medium; preserve output sequence |
| 3 | Reuse action batches and move storage | Search and wide actions | Small–medium; preserve ties and recursion ownership |
| 4 | Memoize compound reach answers | Coordinate ordering | Medium; immutable per-context queries |
| 5 | Cache complete local capture gains | Quiescence | Small–medium; exact context and invalidation |
| 6 | Avoid clearing the full edge-reference matrix | `take_point` | Medium; preserve first-edge ownership |
| 7 | Reuse unchanged axis bounds | Best-first heap insertion | Medium; exact bounds, class and sequence |
| 8 | Bulk merge superphysical destination regions | HC construction | Medium; preserve segment and coordinate order |
| 9 | Extract NNUE features once for both views | Evaluation misses | Small; exact integer feature mapping |
| 10 | Tune pure board-cache capacity/associativity | HCE/NNUE | Medium; memory cost and exact-match hits |
| 11 | Cache standard-chess board terms | Single-timeline evaluation | Small; measured headroom is limited |
| 12 | Specialize an exclusively owned TT | Single-thread search | Larger; ownership and layout constraints |

### 1. Replace checking-move lists used only as existence queries

Locations: `src/core/hypercuboid.rs:106`, `:118`, `:1487`, `:1537`.

`lone_checks` and `EnvChecks::checks` store full `Vec<FullMove>` lists. Their
consumers ask only whether a surviving attacker occupies any source square:

```text
any(m in checks: attackers & bit(m.from.xy()) != 0)
    == (attackers & OR(bit(m.from.xy()) for m in checks)) != 0
```

Store a source-square `u64` per cached list. Initially, retain the existing scan
and fold it once; subsequently let a dedicated scan collector OR source bits
directly to avoid storing full moves. An empty list maps to zero. Retain the
existing build invalidation and ascending construction of `env_checks.maybe`.

This predicate is only a preliminary filter. Leave `first_check_after` and its
ordered `first_check` / `first_check_in` scan unchanged. The selected checker,
resulting exclusion slice and next generated action then stay the same.

Validate old-list versus mask predicates over generated HC points, including
multiple checks from one source, captures of sources, fairy pieces and branches.
Then compare ordered action streams and deterministic search traces. No new
legality approximation is needed.

### 2. Stop cloning hypercuboids that cannot produce an output piece

Locations: `src/misc/geometry.rs:145`, `:194`, `:31`;
`src/core/hypercuboid.rs:2323`.

Both slice-removal implementations clone the entire remaining HC, subtract one
axis, and then discard the clone if that axis became empty. Test
`remaining[i].is_subset_of(fixed)` first. When true, there is no output piece and
the intersection leaves the remaining axis unchanged, so both the clone and
mutation can be skipped. Benchmark the extra subset scan on the non-subset case.

The best-first cursor owns the popped HC and discards it after splitting. Add
consuming removal variants that use that HC as `remaining` instead of first
cloning it. Preserve the existing borrowed APIs where callers retain the input.
Likewise, implement `Hc::clone_from` using its destination buffer; currently the
default clone-from path creates another pooled copy.

Keep exactly the same emitted-piece order, including empty-intersection handling
and coordinates outside the HC. Heap insertion sequence is a tie-breaker. The
word pool already avoids many allocations; this proposal targets copies and
pool traffic, not a nonexistent unpooled allocator problem.

### 3. Retain action batches, root moves and spilled move allocations

Locations: `src/engine/ab.rs:872`, `:2702`, `:2841`, `:3878`;
`src/core/action.rs:94`.

The search allocates `Vec::with_capacity(BATCH)` on each batch iteration and
consumes it. Add reusable batch storage to the owned `NodeBufs`, drain the active
elements, and return the allocation on every exit. The existing owned-buffer
pool is preferable to a global scratch vector: recursion, including singular
search at the same nominal ply, must not overwrite a parent's live batch.

Keep the current 1, 2, 4, ... batch sizes and stable equal-score ordering. Do not
replace stable sorts with score-only unstable sorts. `order_skipped` also takes
`scratch.skipped` and drops the vector rather than returning its capacity; clear
its length before restoring it so the existing empty-scratch semantics survive.

Root reordering currently sorts indices and clones every selected action into
a new vector. Apply the same stable permutation by moving owned entries; actions
longer than three moves otherwise allocate on clone. Preserve treatment of
missing scores and the subsequent kept-mate rotation.

`MoveSeq::assign` can reserve once and bulk-copy a spilled action rather than
repeatedly reconstructing a `Vec` through `push`. Keep the three-move inline layout
and zero-valid representation, which the TT allocation relies upon. Increasing
inline capacity is a separate memory-layout experiment, not this change.

### 4. Cache compound-ray reach by square and movement component

Locations: `src/engine/ab.rs:759`, `:781`;
`src/core/check_position.rs:85`, `:120`, `:329`.

`CheckDetector` caches `SpReach` per source line, but each physical coordinate
again asks `CompoundRay::hits(square, diagonal)`. That loops through up to seven
ray boards, constructs occupancies and performs slider attacks. Several moves
can query the same square/component in the same immutable reach context.

Add lazy known/result bitmasks for those square/component queries, or cache their
combined orthogonal/diagonal component masks. Reuse the current results exactly;
do not approximate reach or substitute a different check detector. Invalidate
with the owning `SpReach` / node context. A lazy version avoids eagerly computing
all 64 squares for a board whose few moves never ask for them.

Compare all square/component answers to the original routine and compare exact
coordinate scores and tie order. Ordering was 1.998 s in the main diagnostic
run, but the fraction specifically spent in repeated `hits` remains unmeasured.

### 5. Cache `capture_gain` at its complete, fixed-window entry point

Locations: `src/engine/ab.rs:2325`; `src/engine/local.rs:22`.

Every qualifying quiet leaf reruns a depth-six board-local capture search for
each mandatory board. A cache can return exactly the same integer gain for an
unchanged board. The observer found 86–90% repeated requests.

Key by complete board contents, side to move and board height. Keep the cache
owned by the engine's material-value configuration, invalidating when
`local_vals` changes; an engine-lifetime generation is another valid option.
Use hash indexing with full key verification. Cache only the full `capture_gain`
result. Internal `search` results depend on alpha, beta and depth and cannot be
cached as unconditional exact values.

Do not change capture order, the 64-move limit, promotion behavior or royal
handling. These are part of the current function's semantics even where a
different rule might appear more natural. The measured section is only 0.433 s
of the 20.046 s instrumented main run; high hit rate is not a huge engine-speed
claim. A cache lookup may cost more than recomputation on trivial quiet boards.

### 6. Clear only the edge references that were written

Locations: `src/core/hypercuboid.rs:1008`; `src/misc/graph.rs:39`.

`PointScratch` already retains storage, but every `take_point` clears/resizes
`edge_refs` to `dim * dim` INVALID values. Most coordinates may be null and the
actual graph sparse. The graph's already-cleared adjacency bitsets can provide
current-edge validity: first-edge ownership checks graph membership, and every
matching edge has both references written in the current call. This avoids
clearing references or adding stamp/touched-entry storage. Handle dimension/stride
changes explicitly. Do not read an unwritten or stale reference.

The reverse-edge INVALID test preserves the **first** pairing claimed by an
endpoint. Keep that exact ownership, graph edges, vertex order and blossom
matching algorithm. A different equally valid matching is not oracle-identical.
This is a candidate, not a guaranteed win: at small dimensions a dense fill can
beat stamp checks and bookkeeping.

### 7. Avoid recomputing every axis bound for every HC child

Location: `src/core/hypercuboid.rs:2062` (`BfHeap::push`).

Each split child differs from its parent on a small number of axes, yet insertion
rescans every axis, including subset and singleton tests and searches through
coordinate order. Reuse unchanged per-axis maxima or a sequentially updated
running bound while splitting. As a simpler first patch, compute `single()` once
per axis rather than multiple times and avoid rebuilding equivalent metadata.

Keep the exact `i64` bound and `(-class, bound, seq)` heap key. Empty child HCs
must still be discarded before advancing sequence. Deferred branch ordering
requires a version/invalidation boundary; do not cache maxima before the final
scores exist. Any shortcut that changes `need_branch` timing or counted cursor
steps needs a stronger proof than just legal-action equivalence.

### 8. Append, sort and coalesce destination regions once

Locations: `src/core/multiverse.rs:128`, `:750`, `:819`.

`region_insert` binary-searches and inserts into the middle of a vector for each
destination. Compound sliders revisit destination boards across components and
directions. For sorted-merge segments, append `(destination, bitboard)` pairs,
sort once by the identical `Vec4` ordering, then OR adjacent equal destinations.
The resulting list must be byte-for-byte equivalent in order and contents.

Respect segment boundaries: bishop generation has a pure-slide prefix followed
by a separately sorted compound segment; other pieces have their own generation
orders. Do not globally sort all pieces' move lists. Compare complete ordered
`MoveList`s over randomized positions, short histories and every fairy piece.
Small move lists may favor the current insertion method, so benchmark both sizes.

### 9. Use the existing `other_view` feature transform during NNUE updates

Locations: `src/engine/nnue.rs:107`, `:417`, `:426`.

`refresh_board` extracts each square's features separately for both views;
`update_board` decodes each changed old/new square twice. Extract one view and
derive the other through the existing `other_view(f, h)` mapping. Preserve the
per-square piece/unmoved-feature order and the existing wrapping integer adds
and subtracts. Do not assume eight ranks: the mapping takes board height.

Compare complete feature lists and accumulators against both-view extraction,
including walls, fairy pieces, unmoved flags, promotion, and make/unmake. Then
require integer evaluator equality for cold refresh and incremental paths.
The measured FT section is 0.448 s; only part of that is feature extraction.

### 10. Experiment with exact board-cache sizes and associativity

Locations: `src/engine/hce.rs:751`, `src/engine/nnue.rs:580`.

The term cache is 4,096 slots; NNUE is 8,192 slots, both direct-mapped with exact
board comparison. Count hits, collisions, predecessor-update availability and
memory per thread before trying larger or two-way tables. A pure cache hit must
return exactly what a miss recomputes, so this can be speed-only.

Keep height/width context and model invalidation correct. For the frozen NNUE,
the two-view accumulator storage alone is 4 MiB per thread; doubling it is not
free. Include SMP memory/cache pressure in measurements. Changing TT capacity,
indexing or replacement is **not** the same experiment: those affect the search.

### 11. Cache standard-chess terms, but keep expectations small

Locations: `src/engine/hce.rs:842`; `src/engine/chess.rs:541`.

`TermCache` covers `BoardTerms`, but `sides_with` separately calls `board_chess`
on each active board whenever the chess terms are enabled. Cache `BoardChess`
by full board and dimensions, preferably lazily in board-term storage so it does
not add chess work to disabled multi-timeline positions. Keep `thr` selection by
board side to move outside the cache, as it is now.

`CHESS_MAX_LINES = 1`: these terms are not computed in ordinary many-timeline
positions. The observer hit rate and measured time argue against making this
the first optimization. Compare cache-hit and forced-miss outputs exactly.

### 12. Remove TT locking only under proven exclusive ownership

Locations: `src/engine/ab.rs:339`, `:351`.

Every TT probe/store uses an atomic exchange even with one search thread. A
specialized exclusively owned single-thread table could avoid those locked
instructions while preserving entry selection, replacement and contents.

This is a larger architectural candidate. `Threads=1` alone does not establish
Rust aliasing/exclusive ownership if helper handles retain table references.
Prove exclusivity for the whole search and retain the existing synchronized path
otherwise. Keep the same slot count, index mapping, `MoveSeq` layout and memory
initialization assumptions. Do not replace the SMP table with lossy lockless
entries under the speed-only label. Ordinary lazy-SMP runs are scheduling-dependent
and cannot provide an exact multi-thread search oracle.

## Benchmark status and verified v2 baselines

`ktools abbench` now uses one thread and `MateVerify=counted`. The main root mate
scan is bounded by 1,024 actions, 262,144 hypercuboids per enumeration, and 4,096
steps per escape recheck, with no local wall-clock deadline. The existing quick
scan and travel-probe count limits also remain in force. Normal gameplay still
defaults to `MateVerify=auto`.

The v2 oracle hashes actual total nodes, the returned action, final internal
score, mate-proof status, and normalized search reports including every completed
depth's score, PV and nodes. Clock/NPS fields and the root diagnostic's elapsed
milliseconds are excluded. NPS uses time inside `find_best_move`, including root
preparation; `wall` additionally includes engine setup and result handling.
`BENCH_VERBOSE=1` provides per-position signatures and normalized reports.

Repeated release runs matched for all three validation workloads:

| Workload | Depth ceiling | Node cap per position | Actual total nodes | V2 oracle |
|---|---:|---:|---:|---|
| `data/bench_ab.g` | 20 | 20,000 | 180,000 | `675116deb4e173ea` |
| `docs/performance_audit/timeline_sample.g` | 20 | 2,000 | 18,000 | `b00c871af5ef5dda` |
| `data/bench_ab.g` | 2 | None | 15,753 | `763f7d8280d6431f` |

These are new baselines; the old `e82287ff973edc82` checksum is incompatible.
The perft oracle remains `c3d0858de3887e52` (21,813 leaves). The full test suite
passed: 107 tests passed, with two existing tests ignored. Regression tests cover
actual node accounting before the first completed depth, cross-process trace
repeatability, signature sensitivity, time-field removal and the counted budget.

The exact release outputs are saved in
`docs/performance_audit/benchmark_v2_results.txt`. Timing differences between
these A/A runs are ordinary measurement noise, not evidence of an engine speedup.
The network hashes used for validation match those in the historical measurements.

## Historical measurements and their limits

These profiles were collected before the benchmark repair, from the frozen
`cf055d3d2d13c76b6eb03193b1c16aabfc77fc89` source. They still support the
candidate ranking, but have not been rerun with the counted v2 benchmark.

Host: Ryzen 7 5800X, Windows x86-64 MSVC; rustc
`1.93.0-nightly (646a3f8c1 2025-12-02)`. Repository settings already use native CPU,
optimization level 3, fat LTO, one codegen unit, and aborting release panics.
The frozen embedded NNUE is 128 accumulator / 32 embedding / 32 hidden units,
with dense inputs and no relation accumulator; default blend is 50%.

Network SHA-256: `d6487f1eafbf30d01d6687cb274411592fd3a08d2516ff8f1952a46b1e796979`.
Residual network SHA-256: `44fe897a958dfddf89363b6c5c24226b5e7a8933007a924cfdc8d406b975dfe0`.

Two diagnostic workloads:

| Workload | Positions | Node cap per position | Actual search nodes | Instrumented elapsed |
|---|---:|---:|---:|---:|
| Tracked `data/bench_ab.g`, depth ceiling 20 | 11 | 200,000 | 1,800,000 | 20.046 s |
| Selected timeline positions, depth ceiling 20 | 10 | 2,000 | 18,000 | 6.460 s |

Two main-corpus positions and one timeline position return without search nodes.
The timeline fixture has 1, 3, 6, 5, 8, 11, 13, 16, 10, and 12 active timelines,
and 8–204 stored boards. It is saved in `docs/performance_audit/timeline_sample.g`.
Its comments identify the original zero-based indices in the local `games/tl_bench.g`.
These are diagnostic samples, not a representative playing-strength suite.

Selected section times after correcting the HC build timer boundaries:

| Section | Main corpus | Timeline sample | Interpretation |
|---|---:|---:|---|
| Static evaluation | 5.985 s | 0.081 s | Includes HCE and NNUE below |
| HCE counts | 2.602 s | 0.045 s | Includes royal-5D and chess terms |
| NNUE board lookup, update, embedding and pooling | 1.431 s | 0.020 s | Includes FT and L2 work |
| NNUE head | 0.739 s | 0.006 s | Integer head, already AVX2 |
| Move ordering | 1.998 s | 0.309 s | Includes physical and travel check scoring |
| HC rebuild | 2.159 s | 0.164 s | All construction stages |
| Candidate `take_point` | 0.828 s | 0.752 s | 1,390,250 / 473,744 calls |
| Candidate legality (`jump+present`) | 1.446 s | 0.977 s | Includes check scan below |
| Candidate check scan | 1.135 s | 0.747 s | Preserve the first check found |
| Invalid-region removal (`space_ops`) | 0.099 s | 0.197 s | Does not cover all successful point splits |
| Heap bound computation | 0.298 s | 0.105 s | Also nested inside some space operations |
| Board-local capture gain | 0.433 s | 0.006 s | Not counted as AB nodes internally |
| Standard-chess board terms | 0.185 s | <0.001 s | Only enabled with one active timeline |

**Do not sum these rows.** They overlap. Instrumentation adds clock and atomic
overhead; the added diagnostic cache probes also cost time. Other work was active
on the desktop. These results support prioritization, not release-speed claims or
precise exclusive CPU percentages. In particular, the small-node timeline run
includes substantial root preprocessing. Evaluate root latency separately from
steady-state search throughput.

An observer-only, direct-mapped 4,096-slot cache, checking the full board and
context but still executing the original function, found:

| Pure function | Main corpus repeated requests | Timeline sample repeated requests |
|---|---:|---:|
| `local::capture_gain` | 544,177 / 630,354 = **86.3%** | 5,692 / 6,331 = **89.9%** |
| `chess::board_chess` | 115,760 / 585,314 = **19.8%** | 24 / 779 = **3.1%** |

These observer caches retained entries across the corpus; material values were
fixed throughout. Rates are opportunities, not the benefit of a real cache after
lookup cost, eviction, initialization and invalidation. Chess-term caching is a
lower priority than its uncached implementation initially suggests.

Raw outputs are in `docs/performance_audit/evidence.txt`. The temporary diagnostic
build changed the benchmark hash contents, so its hashes are not comparable to
the stock `aboracle`. The original detailed runs also hashed diagnostic messages;
the saved instrumentation script now restricts trace hashing to depth reports.

## Remaining profiler attribution cleanup

`HcInfo::rebuild` repeatedly shadows `_prof`; shadowing does not drop the previous
guard. `drop(())` drops nothing. Consequently `hc_status` spans nearly the whole
build, `hc_movegen` includes later stages, and `hc_physical` includes jumps.
`BfHeap::push` also records into the `tt_validate` bucket. `state_clone` actually
times `AlphaBeta::make`, and several declared buckets have no active call sites.

Use explicit blocks or named guards with explicit drops, rename the misleading
bucket, and separate heap bounds. The temporary instrumentation did this for the
build stages and heap bounds. Never use `--features prof` for acceptance NPS pairs.

This cleanup is still open in the regular `prof` feature. It does not block
unprofiled v2 equivalence checks or NPS pairs. The timings above came from the
separately corrected diagnostic build, not the uncorrected regular profiler.

## Acceptance process without SPRT

1. Freeze source, both networks, toolchain, build flags, corpus, engine options,
   TT size and thread count. Use the repaired v2 `abbench` with its counted root
   mate budget and actual node accounting on both binaries.
2. Implement one candidate, with a differential check of the precise function
   it replaces. Run relevant existing tests and ordered move/perft checks where
   generation changes. Exercise quiet, checked, branch-heavy, optional/inactive,
   deep-history, fairy, small-board, castling, en-passant and promotion cases.
3. Require equal v2 oracles and normalized per-position reports at several
   fixed node budgets and uncapped shallow depths on one thread. Include MultiPV
   and cold/warm cache paths; include limited skill and model/weight changes when
   caches touch those paths.
   An A/A mismatch means the harness is not ready for candidate acceptance.
4. Build base and candidate without profiler instrumentation. Warm both. Run
   sequential paired `A B`, then `B A`, alternating order; never benchmark them
   concurrently. Use actual node totals. Retain per-position elapsed times and
   report small/many-timeline strata separately, not only the aggregate.
5. Use paired elapsed-time ratios (or NPS ratios when work is equal), with enough
   repetitions to distinguish the gain from noise. About 12 pairs is a starting
   point, not a statistical guarantee. Report geometric-mean gain and uncertainty;
   reject inconclusive small wins or regressions in important strata. Keep CPU
   affinity/power conditions consistent and avoid competing training/build jobs.
6. Commit the individually passing speed patch with its baseline/candidate IDs,
   model hashes, equivalence outputs, pair results and meaningful limitations.
   No SPRT is needed for an established behavior-preserving transformation.

Run a separate timed smoke test for responsiveness, default root preprocessing,
and SMP resource use. Its expected time-dependent search differences do not
replace deterministic equivalence checks.

## Changes deliberately excluded from this queue

- Different action caps, batch sizes, pruning, reductions, mate budgets, check
  priorities or history behavior. Those change the selective search.
- Wider exclusion slices or a new matching algorithm merely because they return
  the same legal set. They can change enumeration order and step-budget usage.
- Dropping history from board/state hashes, changing TT replacement or table
  indexing, or skipping validation of remembered moves.
- HCE/NNUE blend changes, retraining, disabling dense inputs, changed rounding,
  float reassociation or introducing FMA into the residual network. Preserve
  existing scalar summation order and integer overflow/clamp behavior.
- Less frequent stop/deadline checks sold as strictly identical behavior.
- Recommending make/unmake, PEXT, AVX2, per-ply scratch, branch deferral, or physical
  move caching as new features: they are already implemented.

For the browser build, the software-PEXT slider fallback is a separate plausible
target (`src/core/bitboard.rs:266`). Magic lookup tables or a different exact
attack algorithm could preserve every attack bitboard, but native BMI2 NPS pairs
would not measure that change. Benchmark WASM separately if this target matters.

## Running the repaired benchmark

PowerShell, from the repository root:

```powershell
cargo build --release --bin ktools
$env:AB_NODES = '20000'
.\target\release\ktools.exe abbench data/bench_ab.g 20

$env:AB_NODES = '2000'
.\target\release\ktools.exe abbench docs/performance_audit/timeline_sample.g 20

Remove-Item Env:AB_NODES
.\target\release\ktools.exe abbench data/bench_ab.g 2
```

Use the same corpus, depth, node cap, evaluation files, blend, toolchain and build
flags for both binaries. `NNUE_FILE` and `NNUE_BLEND` override the embedded
evaluation. Repeat an A/A run before measuring a candidate, then follow the
interleaved-pair procedure above. Tests are run with `cargo test`.

The capped perft `bench` remains a useful additional check. For move-generation
changes, also compare ordered action streams and the affected budget/exclusion
behavior; matching capped leaf counts alone does not establish search identity.

## Reproducing the historical profiles

The archived profiling and probe instructions, original hashes and benchmark
failure evidence are in `docs/performance_audit/benchmark_history.md`. The saved
`instrument.py` and `probe.rs` target the original `cf055d3` archive and are useful
for reproducing that evidence. Use the repaired `ktools abbench` for new speed
patch acceptance.
