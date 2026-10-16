//! Texel tuning of the feature evaluation on harness game files (JSONL).
//!
//! tune [--epochs N] [--lr X] [--threads T] [--out weights.txt] [--freeze name,name,...]
//!      games1.jsonl games2.jsonl ...
//!
//! `--freeze` keeps the named features at their prior weight (to compare feature sets on the
//! same data and holdout).

use kairos::core::state::State;
use kairos::core::variants::{apply_uci_moves, position_from_uci};
use kairos::engine::hce::{self, Weights, NF};
use std::sync::{Arc, Mutex};

/// Features stored compactly (they are small counts).
type Packed = [i16; NF];

struct Sample {
    f: Packed,
    r: f32,
}

fn pack(f: &hce::Features) -> Packed {
    std::array::from_fn(|i| f[i].clamp(i16::MIN as i32, i16::MAX as i32) as i16)
}

#[inline]
fn linear(f: &Packed, w: &Weights) -> f32 {
    let mut acc = 0f32;
    for i in 0..NF {
        acc += w[i] * f[i] as f32;
    }
    acc
}

fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{}\":", key);
    let i = line.find(&pat)? + pat.len();
    let rest = line[i..].trim_start();
    if let Some(r) = rest.strip_prefix('"') {
        let mut end = 0;
        let b = r.as_bytes();
        while end < b.len() {
            if b[end] == b'\\' {
                end += 2;
                continue;
            }
            if b[end] == b'"' {
                break;
            }
            end += 1;
        }
        Some(&r[..end])
    } else {
        let end = rest.find(|c| c == ',' || c == '}').unwrap_or(rest.len());
        Some(rest[..end].trim())
    }
}

/// Quiet positions of one game, labelled with the game result from white's side.
/// No capture or promotion available to the side to move anywhere.
fn quiet(s: &State) -> bool {
    let player = s.get_present().1;
    let mut scratch = kairos::core::multiverse::MoveList::new();
    let (l_min, l_max) = s.get_lines_range();
    (l_min..=l_max).all(|l| {
        let (t, c) = s.get_timeline_end(l);
        c != player || !s.multiverse().has_tactical_move(l, t, player, &mut scratch)
    })
}

static QUIET_ONLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn game_samples(line: &str, out: &mut Vec<Sample>) -> Option<()> {
    let position = field(line, "position")?;
    let moves = field(line, "moves")?;
    let score: f32 = field(line, "score")?.parse().ok()?;
    let new_first = field(line, "new_first")? == "true";
    let actions: usize = field(line, "actions")?.parse().ok()?;
    let mut s: State = position_from_uci(position).ok()?;
    let toks: Vec<&str> = moves.split_whitespace().collect();
    let total = toks.iter().filter(|t| **t == "submit").count();
    let opening = total.saturating_sub(actions);
    // Replay, collecting positions after each submitted action.
    let mut snaps: Vec<State> = Vec::new();
    let mut first_player = None;
    let mut done = 0;
    let mut cur = String::new();
    if opening == 0 {
        first_player = Some(s.get_present().1);
    }
    for t in toks {
        if t == "submit" {
            apply_uci_moves(&mut s, &cur).ok()?;
            apply_uci_moves(&mut s, "submit").ok()?;
            cur.clear();
            done += 1;
            if done == opening {
                first_player = Some(s.get_present().1);
            }
            if done >= 2 {
                snaps.push(s.clone());
            }
        } else {
            cur.push(' ');
            cur.push_str(t);
        }
    }
    let first = first_player?;
    let new_color = if new_first { first } else { !first };
    let white_result = if score == 0.5 {
        0.5
    } else if (score == 1.0) == !new_color {
        1.0
    } else {
        0.0
    };
    if std::env::var("TUNE_TRACE").is_ok() {
        let w = hce::default_weights();
        let ev: Vec<i32> = snaps.iter().map(|st| hce::linear(&hce::features(st), &w) as i32).collect();
        eprintln!("white_result {} evals {:?}", white_result, ev);
        if let Some(st) = snaps.last() {
            let (lo, hi) = st.get_lines_range();
            let (alo, ahi) = st.get_active_range();
            for l in lo..=hi {
                let (t, c) = st.get_timeline_end(l);
                let b = st.board(l, t, c);
                eprintln!("  L{} T{} {} active {} {}", l, t, if c { 'b' } else { 'w' }, l >= alo && l <= ahi, b.get_fen(false));
            }
            eprintln!("  present {:?} feats {:?}", st.get_present(), hce::features(st));
        }
    }
    for st in snaps {
        if st.has_phantom_check() {
            continue;
        }
        if QUIET_ONLY.load(std::sync::atomic::Ordering::Relaxed) && !quiet(&st) {
            continue;
        }
        out.push(Sample { f: pack(&hce::features(&st)), r: white_result });
    }
    Some(())
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

fn loss(samples: &[Sample], w: &Weights, k: f64) -> f64 {
    let mut acc = 0.0;
    for s in samples {
        let p = sigmoid(k * linear(&s.f, w) as f64);
        acc += (s.r as f64 - p).powi(2);
    }
    acc / samples.len().max(1) as f64
}

fn arg<T: std::str::FromStr>(args: &[String], key: &str, def: T) -> T {
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(def)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let epochs: usize = arg(&args, "--epochs", 3000);
    let lr: f64 = arg(&args, "--lr", 2.0);
    let threads: usize = arg(&args, "--threads", 14);
    let out_path: String = arg(&args, "--out", "games/tuned_weights.txt".to_string());
    QUIET_ONLY.store(args.iter().any(|a| a == "--quiet"), std::sync::atomic::Ordering::Relaxed);
    let files: Vec<String> = args[1..]
        .iter()
        .enumerate()
        .filter(|(i, a)| a.ends_with(".jsonl") && !args[*i].starts_with("--"))
        .map(|(_, a)| a.clone())
        .collect();
    let mut lines: Vec<String> = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).expect("read games");
        lines.extend(text.lines().filter(|l| !l.trim().is_empty()).map(String::from));
    }
    eprintln!("{} games from {} files", lines.len(), files.len());
    let lines = Arc::new(lines);
    let collected: Arc<Mutex<Vec<(usize, Vec<Sample>)>>> = Arc::new(Mutex::new(Vec::new()));
    let t0 = std::time::Instant::now();
    let mut hs = Vec::new();
    for tid in 0..threads {
        let (lines, collected) = (lines.clone(), collected.clone());
        hs.push(std::thread::spawn(move || {
            let mut mine = Vec::new();
            let mut i = tid;
            while i < lines.len() {
                let _ = game_samples(&lines[i], &mut mine);
                i += threads;
            }
            collected.lock().unwrap().push((tid, mine));
        }));
    }
    for h in hs {
        h.join().unwrap();
    }
    // In thread order, so the shuffle and holdout below are reproducible.
    let mut parts = std::mem::take(&mut *collected.lock().unwrap());
    parts.sort_by_key(|p| p.0);
    let mut all: Vec<Sample> = parts.into_iter().flat_map(|p| p.1).collect();
    // Deterministic shuffle, then 10% holdout.
    let mut st = 0x2545F4914F6CDD1Du64;
    for i in (1..all.len()).rev() {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        all.swap(i, (st % (i as u64 + 1)) as usize);
    }
    let hold = all.len() / 10;
    let (valid, train) = all.split_at(hold);
    eprintln!("{} positions ({} train, {} holdout) in {:.1}s", all.len(), train.len(), valid.len(), t0.elapsed().as_secs_f64());

    let mut w = hce::default_weights();
    // --init file: start from (name value) lines, times --init-scale; unnamed features keep
    // their default.
    let init: String = arg(&args, "--init", String::new());
    if !init.is_empty() {
        let sc: f32 = arg(&args, "--init-scale", 1.0);
        for line in std::fs::read_to_string(&init).expect("init weights").lines() {
            let mut it = line.split_whitespace();
            if let (Some(n), Some(v)) = (it.next(), it.next().and_then(|v| v.parse::<f32>().ok())) {
                if let Some(i) = hce::FEATURE_NAMES.iter().position(|x| *x == n) {
                    w[i] = v * sc;
                }
            }
        }
    }
    if args.iter().any(|a| a == "--featdiag") {
        // Per-feature correlation with the result and mean result by sign.
        let n = all.len() as f64;
        let mr = all.iter().map(|s| s.r as f64).sum::<f64>() / n;
        for i in 0..NF {
            let mf = all.iter().map(|s| s.f[i] as f64).sum::<f64>() / n;
            let (mut cov, mut vf, mut vr) = (0.0, 0.0, 0.0);
            let (mut pos, mut np, mut neg, mut nn) = (0.0, 0usize, 0.0, 0usize);
            for s in all.iter() {
                let (df, dr) = (s.f[i] as f64 - mf, s.r as f64 - mr);
                cov += df * dr;
                vf += df * df;
                vr += dr * dr;
                if s.f[i] > 0 {
                    pos += s.r as f64;
                    np += 1;
                } else if s.f[i] < 0 {
                    neg += s.r as f64;
                    nn += 1;
                }
            }
            let corr = cov / (vf.sqrt() * vr.sqrt()).max(1e-12);
            eprintln!(
                "{:<16} corr {:+.4}  f>0: {:.3} (n {:7})  f<0: {:.3} (n {:7})",
                hce::FEATURE_NAMES[i],
                corr,
                pos / np.max(1) as f64,
                np,
                neg / nn.max(1) as f64,
                nn
            );
        }
        return;
    }
    if args.iter().any(|a| a == "--diag") {
        // Mean result by default-eval bucket (white's view), to check labels and scale.
        let mut bk: std::collections::BTreeMap<i32, (f64, usize)> = Default::default();
        for s in all.iter() {
            let e = linear(&s.f, &w) as i32;
            let b = (e.clamp(-2000, 2000) / 200) * 200;
            let x = bk.entry(b).or_default();
            x.0 += s.r as f64;
            x.1 += 1;
        }
        for (b, (r, n)) in bk {
            eprintln!("eval {:6}: n {:8}  mean result {:.3}", b, n, r / n as f64);
        }
        let draws = all.iter().filter(|s| s.r == 0.5).count();
        eprintln!("draw positions {:.3}", draws as f64 / all.len() as f64);
        return;
    }
    // Logistic scale per centipawn (fixed: with free weights it only sets their units).
    let k: f64 = arg(&args, "--k", 0.004);
    let l2: f64 = arg(&args, "--l2", 1e-7);
    // Features that are almost never non-zero keep their prior weight.
    // Features non-zero in fewer than this fraction of positions keep their prior weight.
    let min_nz: f64 = arg(&args, "--min-nz", 0.005);
    let forced_arg: String = arg(&args, "--freeze", String::new());
    let forced: Vec<&str> = forced_arg.split(',').filter(|x| !x.is_empty()).collect();
    let mut frozen = [false; NF];
    for i in 0..NF {
        let nz = train.iter().filter(|s| s.f[i] != 0).count();
        frozen[i] = (nz as f64) < min_nz * train.len() as f64 || forced.contains(&hce::FEATURE_NAMES[i]);
        if frozen[i] {
            eprintln!("frozen {} ({} non-zero)", hce::FEATURE_NAMES[i], nz);
        }
    }
    let prior = w;
    eprintln!("K = {:.6} (per cp); start loss train {:.6} holdout {:.6}", k, loss(train, &w, k), loss(valid, &w, k));

    // Feature scales for a better-conditioned Adam step.
    let mut scale = [1f64; NF];
    for i in 0..NF {
        let m = train.iter().map(|s| (s.f[i] as f64).abs()).sum::<f64>() / train.len() as f64;
        scale[i] = m.max(1e-3);
    }
    let (mut m1, mut m2) = ([0f64; NF], [0f64; NF]);
    let (b1, b2, eps) = (0.9f64, 0.999f64, 1e-8f64);
    let mut wd: [f64; NF] = std::array::from_fn(|i| w[i] as f64);
    let chunks: Vec<&[Sample]> = train.chunks(train.len().div_ceil(threads)).collect();
    for epoch in 1..=epochs {
        let wf: Weights = std::array::from_fn(|i| wd[i] as f32);
        let grads: Vec<[f64; NF]> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunks
                .iter()
                .map(|ch| {
                    let wf = wf;
                    sc.spawn(move || {
                        let mut g = [0f64; NF];
                        for s in ch.iter() {
                            let p = sigmoid(k * linear(&s.f, &wf) as f64);
                            let d = 2.0 * (p - s.r as f64) * p * (1.0 - p) * k;
                            for i in 0..NF {
                                g[i] += d * s.f[i] as f64;
                            }
                        }
                        g
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let n = train.len() as f64;
        for i in 0..NF {
            if frozen[i] {
                continue;
            }
            let g = grads.iter().map(|g| g[i]).sum::<f64>() / n + 2.0 * l2 * (wd[i] - prior[i] as f64);
            m1[i] = b1 * m1[i] + (1.0 - b1) * g;
            m2[i] = b2 * m2[i] + (1.0 - b2) * g * g;
            let mh = m1[i] / (1.0 - b1.powi(epoch as i32));
            let vh = m2[i] / (1.0 - b2.powi(epoch as i32));
            wd[i] -= lr / scale[i].sqrt() * mh / (vh.sqrt() + eps);
        }
        if epoch % 250 == 0 || epoch == epochs {
            let wf: Weights = std::array::from_fn(|i| wd[i] as f32);
            eprintln!("epoch {:5} train {:.6} holdout {:.6}", epoch, loss(train, &wf, k), loss(valid, &wf, k));
        }
    }
    w = std::array::from_fn(|i| wd[i] as f32);
    let mut text = String::new();
    for i in 0..NF {
        text += &format!("{:<16} {:10.2}\n", hce::FEATURE_NAMES[i], w[i]);
    }
    print!("{}", text);
    std::fs::write(&out_path, &text).ok();
}
