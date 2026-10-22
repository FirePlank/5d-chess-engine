# Oracle-preserving speed comparisons

Build a frozen baseline and a candidate with the same compiler, native CPU flags,
release profile and embedded networks. Run the full tests and compare ordered
generation output when changing generation, then run:

```powershell
python scripts/performance/pairs.py base.exe candidate.exe target/pairs.json --pairs 12 --cpu 4
```

`--cpu` is an optional logical CPU index. The runner pins itself and its children;
it fails if the requested affinity cannot be applied. Avoid competing builds or
training during measurements. Both binaries run sequentially on the same CPU.
On Windows, `--priority above-normal` gives only the child benchmarks higher
scheduling priority to reduce interference from ordinary desktop applications;
it does not change any existing process's priority. The default is normal.
Each pair alternates AB/BA order. Warmup runs are checked but excluded from timing
statistics. The defaults are `data/bench_ab.g` at 100,000 nodes per position and
`data/bench_timeline.g` at 10,000. Override them with repeated
`--suite path/to/positions.g:node-cap` options.
Progress is saved between completed pairs, keeping JSON file writes out of the
interval between the base and candidate runs. A mismatch is saved immediately.

Every run must match the baseline's v2 checksum, actual nodes and normalized
per-position reports. Any mismatch stops the run and saves the failure. The
runner expects an uninstrumented `abbench` v2 binary. `NNUE_FILE` and `NNUE_BLEND`
are inherited and recorded; use identical evaluation settings for both binaries.
The JSON also records executable and corpus SHA-256 hashes and the current
evaluation-file hashes. When building frozen snapshots, verify their embedded
files match those recorded hashes.
If the embedded network is being updated concurrently, pass `--nnue-file` with a
frozen copy. It loads that same file in both binaries and records its SHA-256,
independently of the workspace's current network and either executable's embedded
version.
Use `--nnue-blend 50` (or the intended common value) as well when defaults differ.
The runner saves raw per-position stderr alongside the normalized comparison
trace, retaining position timings for diagnosis.

Positive `nps_gain_percent` means the candidate is faster. Results use geometric
means of paired elapsed-time ratios, with an approximate 95% Student-t interval
over pair log ratios. The combined result adds each suite's search time within a
pair before forming the ratio. Inspect each suite as well as the combined result.
Twelve pairs are a starting point; a small or noisy gain may remain inconclusive.
Keep the raw results, and establish a gain before accepting a speed patch.

On Windows, add `--process-cycles` to collect
[QueryProcessCycleTime](https://learn.microsoft.com/en-us/windows/win32/api/realtimeapiset/nf-realtimeapiset-queryprocesscycletime)
after each child exits. This needs no elevation and does not instrument the engine.
Each run records the count and cycles per searched node. A separate paired summary
reports `nodes_per_process_cycle_gain_percent`; positive means fewer cycles for
the same work. Warmups remain excluded, and combined comparisons sum cycles across
suites before taking ratios. The count covers the whole process, including model
loading, initialization, output, and all user/kernel threads; NPS covers the
engine's reported benchmark interval. Do not treat their scopes as identical.

This API reports Windows process cycle accounting, **not** the hardware PMU
`TotalCycles` event. Windows cycle accounting can depend on the platform's
[timer and CPU frequency behavior](https://learn.microsoft.com/en-us/windows/win32/api/realtimeapiset/nf-realtimeapiset-querythreadcycletime#remarks);
it is an additional measure, not proof of a change in
hardware cycles or instructions. Actual hardware counters can be captured with
[ETW PMU recording](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/recording-pmu-events)
and require suitable tracing privileges. Keep hardware counter results separate
from process accounting and wall-clock results. On the audit machine, `xperf
-pmcsources` exposes `TotalCycles` and `InstructionRetired`, but this task runs
without an administrator token.

`oracle.rs` provides broader deterministic checks using the same source linked
against each frozen release library. Example for one library (PowerShell):

```powershell
$deps = 'target/base/release/deps'
$lib = (Get-ChildItem $deps -Filter 'libkairos*.rlib' | Select-Object -First 1).FullName
rustc --edition=2024 -C opt-level=3 -C target-cpu=native -C panic=abort -C lto=fat -C codegen-units=1 scripts/performance/oracle.rs --extern "kairos=$lib" -L "dependency=$deps" -o target/oracle-base.exe
```

Repeat for the candidate library. Run both probes with identical arguments:
`CORPUS DEPTH NODE_CAP MULTIPV SKILL MODEL [MODEL...]`, then compare stdout byte
for byte. A zero cap means uncapped; `<empty>` selects HCE. Each model is loaded
in sequence into the same engine, with a cold search, a warm search, and an
explicit resumed search one ply deeper to exercise retained root order and model
invalidation. Use explicit model files to avoid differing
embedded versions; set the same `NNUE_BLEND` environment value for both probes.
These are equivalence checks, not NPS measurements. Finish
compilation and validation before starting timing pairs.
Below skill 8, the engine randomly selects a final move from its searched
candidates. The probe compares their deterministic reports, node count and proof
status; it omits the randomized final move and its score. Full-strength results
include all final fields. Choose tractable positions for extended MultiPV/model
checks; synthetic perft stress positions can have extremely expensive root scans
even at a small search node cap.
