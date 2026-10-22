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
Each pair alternates AB/BA order. Warmup runs are checked but excluded from timing
statistics. The defaults are `data/bench_ab.g` at 100,000 nodes per position and
`data/bench_timeline.g` at 10,000. Override them with repeated
`--suite path/to/positions.g:node-cap` options.

Every run must match the baseline's v2 checksum, actual nodes and normalized
per-position reports. Any mismatch stops the run and saves the failure. The
runner expects an uninstrumented `abbench` v2 binary. `NNUE_FILE` and `NNUE_BLEND`
are inherited and recorded; use identical evaluation settings for both binaries.
The JSON also records executable and corpus SHA-256 hashes and the current
evaluation-file hashes. When building frozen snapshots, verify their embedded
files match those recorded hashes.

Positive `nps_gain_percent` means the candidate is faster. Results use geometric
means of paired elapsed-time ratios, with an approximate 95% Student-t interval
over pair log ratios. The combined result adds each suite's search time within a
pair before forming the ratio. Inspect each suite as well as the combined result.
Twelve pairs are a starting point; a small or noisy gain may remain inconclusive.
Keep the raw results, and establish a gain before accepting a speed patch.
