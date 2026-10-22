"""Sequential AB/BA search benchmarks, with oracle and normalized-trace checks.

python scripts/performance/pairs.py base.exe new.exe results.json --pairs 12 --cpu 4
Repeat --suite FILE:NODES to override the two default suites. No third-party packages.
"""
import argparse
import ctypes
import hashlib
import json
import math
import os
from pathlib import Path
import re
import statistics
import subprocess


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def pin(cpu):
    if os.name == "nt":
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.GetCurrentProcess.restype = ctypes.c_void_p
        kernel.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
        if not kernel.SetProcessAffinityMask(kernel.GetCurrentProcess(), 1 << cpu):
            raise ctypes.WinError(ctypes.get_last_error())
    else:
        os.sched_setaffinity(0, {cpu})


def summarize(values):
    logs = [math.log(a / b) for a, b in values]
    mean = statistics.mean(logs)
    # Two-sided Student t 95% critical values, df 1..30. Larger samples use the
    # df=30 value conservatively. Pairs, not positions, are the sampling units.
    critical = [12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306,
                2.262, 2.228, 2.201, 2.179, 2.160, 2.145, 2.131, 2.120,
                2.110, 2.101, 2.093, 2.086, 2.080, 2.074, 2.069, 2.064,
                2.060, 2.056, 2.052, 2.048, 2.045, 2.042]
    result = {"pairs": len(logs), "nps_gain_percent": 100 * math.expm1(mean)}
    if len(logs) > 1:
        half = critical[min(len(logs) - 2, 29)] * statistics.stdev(logs) / math.sqrt(len(logs))
        result["ci95_percent"] = [100 * math.expm1(mean - half), 100 * math.expm1(mean + half)]
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("base", type=Path)
    p.add_argument("candidate", type=Path)
    p.add_argument("output", type=Path)
    p.add_argument("--pairs", type=int, default=12)
    p.add_argument("--cpu", type=int)
    p.add_argument("--priority", choices=["normal", "above-normal"], default="normal",
                   help="child benchmark priority (above-normal is Windows only)")
    p.add_argument("--nnue-file", type=Path, help="load this frozen network in both binaries")
    p.add_argument("--suite", action="append", help="position-file:node-cap")
    args = p.parse_args()
    if args.pairs < 2:
        p.error("at least two pairs are required")
    if args.cpu is not None:
        pin(args.cpu)
    if args.priority == "above-normal" and os.name != "nt":
        p.error("above-normal priority is supported only on Windows")
    creationflags = subprocess.ABOVE_NORMAL_PRIORITY_CLASS if args.priority == "above-normal" else 0
    suites = [x.rsplit(":", 1) for x in (args.suite or ["data/bench_ab.g:100000", "data/bench_timeline.g:10000"])]
    suites = [(str(Path(path).resolve()), int(nodes)) for path, nodes in suites]
    binaries = {"base": str(args.base.resolve()), "candidate": str(args.candidate.resolve())}
    env = dict(os.environ, BENCH_VERBOSE="1")
    if args.nnue_file is not None:
        env["NNUE_FILE"] = str(args.nnue_file.resolve())
    eval_file = env.get("NNUE_FILE")
    result = {"binaries": {k: {"path": v, "sha256": digest(v)} for k, v in binaries.items()},
              "cpu": args.cpu, "priority": args.priority, "suites": [{"path": path, "nodes": nodes, "sha256": digest(path)} for path, nodes in suites],
              "evaluation_env": {k: env.get(k) for k in ["NNUE_FILE", "NNUE_BLEND"]},
              "evaluation_file_sha256": digest(eval_file) if eval_file and eval_file.lower() not in {"none", "<empty>"} else None,
              "workspace_evaluation_files": {str(path): digest(path) for path in [Path("data/kairos.nnue"), Path("data/evalnet.bin")]},
              "runs": []}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    expected = {}

    def save():
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")

    def run(binary, suite, pair):
        path, nodes = suites[suite]
        run_env = dict(env, AB_NODES=str(nodes))
        out = subprocess.run([binaries[binary], "abbench", path, "20"], env=run_env, capture_output=True, text=True, check=True,
                             creationflags=creationflags)
        if len(out.stdout.splitlines()) != 1:
            raise RuntimeError("use an uninstrumented release build (unexpected output)")
        m = re.fullmatch(r"aboracle ([0-9a-f]+) \| v2 depth 20 \| nodes (\d+) in ([\d.]+)s .*", out.stdout.strip())
        if not m:
            raise RuntimeError("unrecognized abbench v2 output: " + out.stdout)
        trace = re.sub(r" time [\d.]+s best ", " best ", out.stderr)
        record = {"binary": binary, "suite": suite, "pair": pair, "oracle": m[1], "nodes": int(m[2]),
                  "seconds": float(m[3]), "stdout": out.stdout, "trace": trace}
        result["runs"].append(record)
        signature = (m[1], int(m[2]), trace)
        if suite not in expected:
            expected[suite] = signature
        elif expected[suite] != signature:
            result["error"] = f"oracle/trace mismatch: pair {pair}, suite {suite}, {binary}"
            save()
            raise RuntimeError(result["error"])
        return record["seconds"]

    # One warmup per binary/suite; checked for equivalence but excluded from statistics.
    for suite in range(len(suites)):
        for binary in binaries:
            run(binary, suite, -1)
    save()
    times = [[] for _ in suites]
    combined = []
    for pair in range(args.pairs):
        totals = {"base": 0.0, "candidate": 0.0}
        for suite in range(len(suites)):
            t = {binary: run(binary, suite, pair) for binary in (["base", "candidate"] if pair % 2 == 0 else ["candidate", "base"])}
            times[suite].append((t["base"], t["candidate"]))
            for binary, seconds in t.items():
                totals[binary] += seconds
        combined.append((totals["base"], totals["candidate"]))
        result["summary"] = {"combined": summarize(combined), "suites": [summarize(t) for t in times]}
        save()
        print(json.dumps({"pair": pair + 1, **result["summary"]}), flush=True)


if __name__ == "__main__":
    main()
