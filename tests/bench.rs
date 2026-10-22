//! The benchmark must count interrupted iterations and be repeatable across processes.
use std::process::{Command, Output};

fn abbench(file: &str, nodes: u64) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_ktools"))
        .args(["abbench", file, "20"])
        .env("AB_NODES", nodes.to_string())
        .env("BENCH_VERBOSE", "1")
        .env_remove("NNUE_FILE")
        .env_remove("NNUE_BLEND")
        .output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    output
}

#[test]
fn benchmark_counts_nodes_before_the_first_depth_completes() {
    let output = abbench("data/startpos.g", 1);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stdout.contains("| nodes 1 in "), "{stdout}");
    assert!(stdout.contains("| mate counted | node-cap 1"), "{stdout}");
    assert!(stderr.contains("depth 0 nodes 1 "), "{stderr}");
}

#[test]
fn benchmark_repeats_the_same_multiverse_search_trace() {
    let a = abbench("data/bench_ab.g", 2000);
    let b = abbench("data/bench_ab.g", 2000);
    let a_out = String::from_utf8(a.stdout).unwrap();
    let b_out = String::from_utf8(b.stdout).unwrap();
    assert_eq!(a_out.split(" | ").next(), b_out.split(" | ").next());
    // The verbose trace retains all deterministic root-scan and depth information.
    // Only each per-position summary's elapsed time may differ.
    let normalize = |output: Vec<u8>| String::from_utf8(output).unwrap().lines().map(|line| {
        if let Some((prefix, rest)) = line.split_once(" time ") {
            let (_, best) = rest.split_once(" best ").unwrap();
            format!("{prefix} best {best}")
        } else {
            line.to_string()
        }
    }).collect::<Vec<_>>();
    assert_eq!(normalize(a.stderr), normalize(b.stderr));
}
