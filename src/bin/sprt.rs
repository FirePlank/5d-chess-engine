//! SPRT match harness for 5DUCI engines.
//!
//! Plays opening pairs (colors swapped) between a NEW and a BASE engine, referees every action
//! with the kairos rules core, and stops on a pentanomial GSPRT decision (fastchess math).
//!
//! sprt --new "<cmd>" --base "<cmd>" [--book book.g | --book-plies 6]
//!      [--movetime 100 | --tc 10+0.1 | --nodes N | --depth D]
//!      [--concurrency 8] [--max-games 20000] [--min-games 0] [--elo0 0] [--elo1 5]
//!      [--alpha 0.05] [--beta 0.05] [--model normalized|logistic] [--max-actions 160]
//!      [--games-out games.jsonl] [--seed 1] [--compact]
//!
//! Without `--book`, every pair opens from a generated opening: a random 8x8 built-in variant
//! and `--book-plies` actions chosen by a shallow search with noise, the same for both games.
//!
//! `--games-out` appends one JSON object per game; games 2k and 2k+1 form pair k. Its moves
//! start with the opening's `book` actions. Its `evals` are the engines' reported scores on
//! the reported scale (`"eval_scale":275`: internal units * 100 / 275, see
//! `kairos::engine::REPORT_SCALE`; mates as +-20000); records without `eval_scale` are older
//! and hold internal units.

use kairos::core::action::{ExtMove, MoveSeq};
use kairos::core::hypercuboid::{HcCursor, HcInfo};
use kairos::core::promotion::PromotionOptions;
use kairos::core::state::{MateType, State};
use kairos::core::variants::{apply_uci_moves, position_from_uci};
use kairos::engine::ab::{AlphaBeta, MATE};
use kairos::engine::{report_cp, Limits, OptionValue, REPORT_SCALE};
use kairos::misc::ordering::{AxisOrders, HcOrdering};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ------------------------------------------------------------------ statistics

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GameResult {
    Win,
    Draw,
    Loss,
}

#[derive(Clone, Copy, Default, Debug)]
struct Penta {
    ww: usize,
    wd: usize,
    wl: usize,
    dd: usize,
    ld: usize,
    ll: usize,
}

impl Penta {
    fn pairs(&self) -> usize {
        self.ww + self.wd + self.wl + self.dd + self.ld + self.ll
    }
    fn add(&mut self, a: GameResult, b: GameResult) {
        let w = (a == GameResult::Win) as u32 + (b == GameResult::Win) as u32;
        let l = (a == GameResult::Loss) as u32 + (b == GameResult::Loss) as u32;
        let d = 2 - w - l;
        match (w, d, l) {
            (2, _, _) => self.ww += 1,
            (1, 1, _) => self.wd += 1,
            (1, 0, 1) => self.wl += 1,
            (0, 2, _) => self.dd += 1,
            (0, 1, 1) => self.ld += 1,
            _ => self.ll += 1,
        }
    }
    fn score(&self) -> f64 {
        (self.ww as f64 + 0.75 * self.wd as f64 + 0.5 * (self.wl + self.dd) as f64 + 0.25 * self.ld as f64)
            / self.pairs() as f64
    }
    fn variance(&self) -> f64 {
        let s = self.score();
        (self.ww as f64 * (1.0 - s).powi(2)
            + self.wd as f64 * (0.75 - s).powi(2)
            + (self.wl + self.dd) as f64 * (0.5 - s).powi(2)
            + self.ld as f64 * (0.25 - s).powi(2)
            + self.ll as f64 * s.powi(2))
            / self.pairs() as f64
    }
}

fn elo_to_score(e: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf(-e / 400.0))
}
fn score_to_elo(s: f64) -> f64 {
    -400.0 * (1.0 / s.clamp(1e-9, 1.0 - 1e-9) - 1.0).log10()
}
fn regularize(v: usize) -> f64 {
    if v == 0 { 1e-3 } else { v as f64 }
}

#[allow(clippy::too_many_arguments)]
fn itp(f: impl Fn(f64) -> f64, mut a: f64, mut b: f64, mut fa: f64, mut fb: f64, k1: f64, k2: f64, n0: f64, eps: f64) -> f64 {
    if fa > 0.0 {
        std::mem::swap(&mut a, &mut b);
        std::mem::swap(&mut fa, &mut fb);
    }
    let n_half = ((b - a).abs() / (2.0 * eps)).log2().ceil();
    let n_max = n_half + n0;
    let mut i = 0.0;
    while (b - a).abs() > 2.0 * eps {
        let x_half = (a + b) / 2.0;
        let r = eps * 2f64.powf(n_max - i) - (b - a) / 2.0;
        let delta = k1 * (b - a).powf(k2);
        let x_f = (fb * a - fa * b) / (fb - fa);
        let sigma = (x_half - x_f) / (x_half - x_f).abs();
        let x_t = if delta <= (x_half - x_f).abs() { x_f + sigma * delta } else { x_half };
        let x_itp = if (x_t - x_half).abs() <= r { x_t } else { x_half - sigma * r };
        let fi = f(x_itp);
        if fi == 0.0 {
            a = x_itp;
            b = x_itp;
        } else if fi < 0.0 {
            a = x_itp;
            fa = fi;
        } else {
            b = x_itp;
            fb = fi;
        }
        i += 1.0;
    }
    (a + b) / 2.0
}

fn mle_logistic(scores: &[f64], probs: &[f64], s: f64) -> Vec<f64> {
    let n = scores.len();
    let theta = itp(
        |x| (0..n).map(|i| probs[i] * (scores[i] - s) / (1.0 + x * (scores[i] - s))).sum(),
        -1.0 / (scores[n - 1] - s),
        -1.0 / (scores[0] - s),
        f64::INFINITY,
        f64::NEG_INFINITY,
        0.1,
        2.0,
        0.99,
        1e-3,
    );
    (0..n).map(|i| probs[i] / (1.0 + theta * (scores[i] - s))).collect()
}

fn mean_var(x: &[f64], p: &[f64]) -> (f64, f64) {
    let mu: f64 = x.iter().zip(p).map(|(a, b)| a * b).sum();
    let var = x.iter().zip(p).map(|(a, b)| b * (a - mu) * (a - mu)).sum();
    (mu, var)
}

fn mle_normalized(scores: &[f64], probs: &[f64], mu_ref: f64, t_star: f64) -> Vec<f64> {
    let n = scores.len();
    let mut p = vec![1.0 / n as f64; n];
    for _ in 0..10 {
        let (mu, var) = mean_var(scores, &p);
        let sigma = var.sqrt();
        let phi: Vec<f64> = scores
            .iter()
            .map(|&a| a - mu_ref - 0.5 * t_star * sigma * (1.0 + ((a - mu) / sigma).powi(2)))
            .collect();
        let u = phi.iter().cloned().fold(f64::INFINITY, f64::min);
        let v = phi.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let theta = itp(
            |x| (0..n).map(|i| probs[i] * phi[i] / (1.0 + x * phi[i])).sum(),
            -1.0 / v,
            -1.0 / u,
            f64::INFINITY,
            f64::NEG_INFINITY,
            0.1,
            2.0,
            0.99,
            1e-7,
        );
        let mut max_diff = 0f64;
        for i in 0..n {
            let np = probs[i] / (1.0 + theta * phi[i]);
            max_diff = max_diff.max((np - p[i]).abs());
            p[i] = np;
        }
        if max_diff < 1e-4 {
            break;
        }
    }
    p
}

fn llr(p: &Penta, elo0: f64, elo1: f64, normalized: bool) -> f64 {
    if p.pairs() == 0 {
        return 0.0;
    }
    let c = [regularize(p.ll), regularize(p.ld), regularize(p.dd + p.wl), regularize(p.wd), regularize(p.ww)];
    let total: f64 = c.iter().sum();
    let probs: Vec<f64> = c.iter().map(|x| x / total).collect();
    let scores = [0.0, 0.25, 0.5, 0.75, 1.0];
    let (p0, p1) = if normalized {
        let k = 2f64.sqrt() / (800.0 / 10f64.ln());
        (mle_normalized(&scores, &probs, 0.5, k * elo0), mle_normalized(&scores, &probs, 0.5, k * elo1))
    } else {
        (mle_logistic(&scores, &probs, elo_to_score(elo0)), mle_logistic(&scores, &probs, elo_to_score(elo1)))
    };
    total * (0..5).map(|i| probs[i] * (p1[i].ln() - p0[i].ln())).sum::<f64>()
}

fn erf(x: f64) -> f64 {
    // Abramowitz-Stegun 7.1.26 (|err| < 1.5e-7)
    let t = 1.0 / (1.0 + 0.3275911 * x.abs());
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * (-x * x).exp();
    if x >= 0.0 { y } else { -y }
}

struct EloEst {
    elo: f64,
    err: f64,
    nelo: f64,
    nerr: f64,
    los: f64,
}

fn estimate(p: &Penta) -> EloEst {
    let n = p.pairs() as f64;
    if n == 0.0 {
        return EloEst { elo: 0.0, err: 0.0, nelo: 0.0, nerr: 0.0, los: 0.5 };
    }
    let s = p.score();
    let var = p.variance();
    let vpp = var / n;
    let ci = 1.959963984540054;
    let (up, lo) = (s + ci * vpp.sqrt(), s - ci * vpp.sqrt());
    let s2n = |x: f64| (x - 0.5) / (2.0 * var).sqrt() * (800.0 / 10f64.ln());
    let (nelo, nerr) = if var <= 0.0 { (0.0, 0.0) } else { (s2n(s), (s2n(up) - s2n(lo)) / 2.0) };
    EloEst {
        elo: if s <= 0.0 { -999.0 } else if s >= 1.0 { 999.0 } else { score_to_elo(s) },
        err: ((score_to_elo(up) - score_to_elo(lo)) / 2.0).min(999.0),
        nelo,
        nerr,
        los: if vpp > 0.0 { (1.0 - erf(-(s - 0.5) / (2.0 * vpp).sqrt())) / 2.0 } else { 0.5 },
    }
}

// ------------------------------------------------------------------ engine process

struct Proc {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<String>,
    name: String,
    /// Last search score reported in an info line (mover's view; mates as +-20000).
    last_score: Option<i32>,
}

/// Score from an engine info line, on the reported scale: "score cp X" or "score mate N"
/// (+-20000). Engines that print no "wdl" (kairos before the reported scale) report internal
/// units, converted here.
fn info_score(line: &str) -> Option<i32> {
    let reported = line.split_whitespace().any(|w| w == "wdl");
    let mut it = line.split_whitespace().skip_while(|w| *w != "score").skip(1);
    match (it.next()?, it.next()?) {
        ("cp", v) => v.parse().ok().map(|x| if reported { x } else { report_cp(x) }),
        ("mate", v) => v.parse::<i32>().ok().map(|n| if n > 0 || v == "+0" { 20000 } else { -20000 }),
        _ => None,
    }
}

impl Proc {
    fn spawn(cmd: &str, name: &str) -> Result<Proc, String> {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let mut child = Command::new(parts[0])
            .args(&parts[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn {}: {}", cmd, e))?;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(l) = line else { break };
                if tx.send(l).is_err() {
                    break;
                }
            }
        });
        let mut p = Proc { child, stdin, rx, name: name.into(), last_score: None };
        p.send("5duci")?;
        p.wait_for("5duciok", Duration::from_secs(30))?;
        p.send("setoption name debug value false")?;
        p.send("isready")?;
        p.wait_for("readyok", Duration::from_secs(30))?;
        Ok(p)
    }
    fn send(&mut self, s: &str) -> Result<(), String> {
        writeln!(self.stdin, "{}", s).and_then(|_| self.stdin.flush()).map_err(|e| format!("{} write: {}", self.name, e))
    }
    fn wait_for(&mut self, prefix: &str, timeout: Duration) -> Result<String, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(l) if l.starts_with(prefix) => return Ok(l),
                Ok(l) if prefix == "bestmove" && l.starts_with("nobestmove") => return Ok(l),
                Ok(l) => {
                    if l.starts_with("info") {
                        if let Some(sc) = info_score(&l) {
                            self.last_score = Some(sc);
                        }
                    }
                    continue;
                }
                Err(RecvTimeoutError::Timeout) => return Err(format!("{} timed out waiting for {}", self.name, prefix)),
                Err(RecvTimeoutError::Disconnected) => return Err(format!("{} exited", self.name)),
            }
        }
    }
}
impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.send("quit");
        std::thread::sleep(Duration::from_millis(5));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ------------------------------------------------------------------ games

#[derive(Clone)]
struct Config {
    new_cmd: String,
    base_cmd: String,
    go: String,
    /// Clock games (`--tc base+inc`, seconds): base and increment in ms.
    clock: Option<(u64, u64)>,
    timeout: Duration,
    max_actions: usize,
}

struct Opening {
    promo: u8,
    position: String,
    moves: String,
    /// Actions in `moves`.
    book: usize,
}

fn parse_book(path: &str) -> Vec<Opening> {
    let text = std::fs::read_to_string(path).expect("cannot read book");
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("G ") else { continue };
        let Some((promo, rest)) = rest.split_once(' ') else { continue };
        let (pos, moves) = match rest.find(" moves") {
            Some(i) => (rest[..i].to_string(), rest[i + 6..].trim().to_string()),
            None => (rest.to_string(), String::new()),
        };
        let book = moves.matches("submit").count();
        out.push(Opening { promo: promo.parse().unwrap_or(1), position: pos, moves, book });
    }
    out
}

const STD: &str = "r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*";
/// Generated openings start from the built-in 8x8 variants (4x4 games are decided by the
/// opening, so they only add noise).
fn starts() -> Vec<String> {
    vec![
        format!("size 8x8 odd fen [{STD}:0:1:w]"),
        format!("size 8x8 odd fen [{STD}:0:0:b][{STD}:0:1:w]"),
        "size 8x8 odd fen [r*nbycbnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBYCBNR*:0:1:w]".into(),
        "size 8x8 odd fen [r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBK*QBNR*:0:1:w]".into(),
        "size 8x8 odd fen [r*qbnk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*QBNK*BNR*:0:1:w]".into(),
        format!("size 8x8 even fen [{STD}:-0:1:w][{STD}:+0:1:w]"),
    ]
}
/// Each opening action is chosen among this many candidates (the engine's best-ordered half,
/// then random ones), each scored by a search to BOOK_DEPTH; one of those within the margin
/// of the best is played, uniformly at random. Each side's first action takes the wider
/// BOOK_FIRST_MARGIN (it only excludes blunders): opening scores at this depth swing by
/// over a thousand between candidates.
const BOOK_CANDIDATES: usize = 32;
const BOOK_DEPTH: u32 = 2;
const BOOK_MARGIN: i32 = 80;
const BOOK_FIRST_MARGIN: i32 = 600;

fn shuffled_order(w: &HcInfo, mut st: u64) -> HcOrdering {
    st |= 1;
    let u = w.get_universe();
    let orders = (0..u.dimension())
        .map(|n| {
            let mut v: Vec<u32> = u[n].iter().collect();
            for i in (1..v.len()).rev() {
                st ^= st << 13;
                st ^= st >> 7;
                st ^= st << 17;
                v.swap(i, (st % (i as u64 + 1)) as usize);
            }
            v
        })
        .collect::<Vec<_>>();
    HcOrdering::Precomputed(AxisOrders::new(orders))
}

/// The opening of pair `pair`, deterministic in (seed, pair).
fn gen_opening(eng: &mut AlphaBeta, seed: u64, pair: usize, plies: usize) -> Opening {
    let mut st = (seed ^ (pair as u64 + 1).wrapping_mul(0xD1B5_4A32_D192_ED03)).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut rnd = move || {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        st
    };
    let starts = starts();
    let position = starts[(rnd() % starts.len() as u64) as usize].clone();
    let mut s = position_from_uci(&position).expect("start position");
    let stop = AtomicBool::new(false);
    let limits = Limits { depth: Some(BOOK_DEPTH), ..Default::default() };
    let mut moves = String::new();
    let mut book = 0;
    while book < plies {
        let mut cands: Vec<MoveSeq> = eng.node_order(&s, BOOK_CANDIDATES / 2);
        let (w, ss) = HcInfo::build_hc(&s);
        let mut cur = HcCursor::new(ss, shuffled_order(&w, rnd()));
        while cands.len() < BOOK_CANDIDATES {
            match cur.next(&w, &s) {
                Some(a) if !cands.contains(&a) => cands.push(a),
                Some(_) => {}
                None => break,
            }
        }
        if cands.is_empty() {
            break;
        }
        let mut scored: Vec<(i32, State, String)> = Vec::new();
        for a in cands {
            let mut c = s.clone();
            let mut txt = String::new();
            for &m in &a {
                let em = ExtMove::resolve(m, &c);
                txt += &format!("{} ", em);
                assert!(c.apply_ext::<false>(em));
            }
            txt += "submit";
            assert!(c.submit::<false>());
            // Internal units, mates as +-20000 (as the info lines read before they reported
            // the normalized scale and only proven mates).
            let v = match eng.find_best_move(&c, &limits, &stop, &mut |_| {}) {
                Some(_) => -match eng.last_score() {
                    x if x > MATE - 1000 => 20000,
                    x if x < -(MATE - 1000) => -20000,
                    x => x,
                },
                None if c.get_mate_type() == MateType::Checkmate => 30000,
                None => 0,
            };
            scored.push((v, c, txt));
        }
        let best = scored.iter().map(|x| x.0).max().unwrap();
        let margin = if book < 2 { BOOK_FIRST_MARGIN } else { BOOK_MARGIN };
        scored.retain(|x| x.0 >= best - margin);
        let (_, c, txt) = scored.swap_remove((rnd() % scored.len() as u64) as usize);
        s = c;
        moves += &txt;
        moves.push(' ');
        book += 1;
        if !has_legal_action(&s) {
            break;
        }
    }
    Opening { promo: 1, position, moves: moves.trim().to_string(), book }
}

fn has_legal_action(s: &State) -> bool {
    let (w, ss) = HcInfo::build_hc(s);
    w.search(s, ss).next().is_some()
}

/// Apply an engine's action; Err(reason) if it is illegal.
fn referee_apply(s: &mut State, action: &[String]) -> Result<(), String> {
    let player = s.get_present().1;
    for tok in action {
        let mv = ExtMove::parse(tok)?;
        let p = mv.fm.from;
        if s.get_board_ptr(p.l(), p.t(), player).is_none() || s.get_timeline_end(p.l()) != (p.t(), player) {
            return Err(format!("move {} is not from a playable board", tok));
        }
        let n = s.normalize_promotion(mv).ok_or_else(|| format!("bad promotion {}", tok))?;
        if !s.apply_ext::<false>(n) {
            return Err(format!("illegal move {}", tok));
        }
    }
    if !s.submit::<false>() {
        return Err("action does not move the present".into());
    }
    if s.first_check(s.get_present().1).is_some() {
        return Err("action leaves a royal piece in check".into());
    }
    Ok(())
}

struct GameRecord {
    result: GameResult, // from NEW's perspective
    reason: String,
    actions: usize,
    log: String,
    /// Each engine action's reported score, from the mover's view (reported scale).
    evals: Vec<Option<i32>>,
}

/// `new_white`: whether NEW plays the side to move first in the opening position.
fn play_game(cfg: &Config, procs: &mut [Option<Proc>; 2], op: &Opening, new_first: bool) -> Result<GameRecord, String> {
    let mut s = position_from_uci(&op.position)?;
    s.set_promotion_options(PromotionOptions(op.promo));
    apply_uci_moves(&mut s, &op.moves)?;
    let mut history = op.moves.clone();
    for p in procs.iter_mut().flatten() {
        p.send("5ducinewgame")?;
        p.send("isready")?;
        p.wait_for("readyok", Duration::from_secs(30))?;
    }
    let first_player = s.get_present().1;
    // Remaining clock (ms) of NEW and BASE in clock games.
    let mut clock = [cfg.clock.map(|c| c.0).unwrap_or(0); 2];
    let mut n = 0usize;
    let mut evals: Vec<Option<i32>> = Vec::new();
    let finish = |res_for_mover_new: GameResult, reason: String, n: usize, history: &str, evals: &[Option<i32>]| GameRecord {
        result: res_for_mover_new,
        reason,
        actions: n,
        log: history.to_string(),
        evals: evals.to_vec(),
    };
    loop {
        let player = s.get_present().1;
        // 0 = NEW, 1 = BASE
        let side = if (player == first_player) == new_first { 0 } else { 1 };
        let new_result = |mover_wins: bool| -> GameResult {
            if (side == 0) == mover_wins { GameResult::Win } else { GameResult::Loss }
        };
        if !has_legal_action(&s) {
            let r = match s.get_mate_type() {
                MateType::Checkmate => (new_result(false), "checkmate"),
                _ => (GameResult::Draw, "stalemate"),
            };
            return Ok(finish(r.0, r.1.into(), n, &history, &evals));
        }
        if n >= cfg.max_actions {
            return Ok(finish(GameResult::Draw, "action limit".into(), n, &history, &evals));
        }
        let proc = procs[side].as_mut().unwrap();
        proc.send(&format!("position {} moves {}", op.position, history))?;
        proc.last_score = None;
        let t0 = Instant::now();
        match cfg.clock {
            Some((_, inc)) => {
                // `player` is the side to move (false = white); each engine keeps its own clock.
                let (w, b) = if player { (clock[1 - side], clock[side]) } else { (clock[side], clock[1 - side]) };
                proc.send(&format!("go wtime {} btime {} winc {} binc {}", w, b, inc, inc))?;
            }
            None => proc.send(&cfg.go)?,
        }
        // A clock game allows one second of grace past the flag before the engine is dropped.
        let timeout = cfg.clock.map(|_| Duration::from_millis(clock[side] + 1000)).unwrap_or(cfg.timeout);
        let reply = proc.wait_for("bestmove", timeout)?;
        if let Some((_, inc)) = cfg.clock {
            let used = t0.elapsed().as_millis() as u64;
            if used > clock[side] {
                return Ok(finish(new_result(false), "time forfeit".into(), n, &history, &evals));
            }
            clock[side] = clock[side] - used + inc;
        }
        evals.push(proc.last_score);
        if reply.starts_with("nobestmove") {
            return Ok(finish(new_result(false), "nobestmove with legal actions".into(), n, &history, &evals));
        }
        let toks: Vec<String> = reply.split_whitespace().skip(1).map(|x| x.to_string()).collect();
        if let Err(e) = referee_apply(&mut s, &toks) {
            return Ok(finish(new_result(false), format!("illegal action ({}): {}", e, toks.join(" ")), n, &history, &evals));
        }
        for t in &toks {
            history.push(' ');
            history += t;
        }
        history += " submit";
        n += 1;
    }
}

// ------------------------------------------------------------------ main

fn arg(args: &[String], key: &str) -> Option<String> {
    let prefix = format!("{}=", key);
    if let Some(v) = args.iter().find_map(|a| a.strip_prefix(&prefix)) {
        return Some(v.to_string());
    }
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let new_cmd = arg(&args, "--new").expect("--new <cmd>");
    let base_cmd = arg(&args, "--base").expect("--base <cmd>");
    let book: Vec<Opening> = arg(&args, "--book").map(|p| parse_book(&p)).unwrap_or_default();
    let book_plies: usize = arg(&args, "--book-plies").and_then(|v| v.parse().ok()).unwrap_or(6);
    let conc: usize = arg(&args, "--concurrency").and_then(|v| v.parse().ok()).unwrap_or(8);
    let max_games: usize = arg(&args, "--max-games")
        .or_else(|| arg(&args, "--games"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(20000);
    let min_games: usize = arg(&args, "--min-games").and_then(|v| v.parse().ok()).unwrap_or(0);
    let compact = args.iter().any(|a| a == "--compact");
    let elo0: f64 = arg(&args, "--elo0").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let elo1: f64 = arg(&args, "--elo1").and_then(|v| v.parse().ok()).unwrap_or(5.0);
    let alpha: f64 = arg(&args, "--alpha").and_then(|v| v.parse().ok()).unwrap_or(0.05);
    let beta: f64 = arg(&args, "--beta").and_then(|v| v.parse().ok()).unwrap_or(0.05);
    let normalized = arg(&args, "--model").map(|m| m != "logistic").unwrap_or(true);
    let max_actions: usize = arg(&args, "--max-actions").and_then(|v| v.parse().ok()).unwrap_or(160);
    let seed: u64 = arg(&args, "--seed").and_then(|v| v.parse().ok()).unwrap_or(1);
    // --tc base+inc in seconds, e.g. 10+0.1.
    let clock = arg(&args, "--tc").map(|tc| {
        let (b, i) = tc.split_once('+').unwrap_or((tc.as_str(), "0"));
        let ms = |x: &str| (x.parse::<f64>().expect("--tc base+inc") * 1000.0).round() as u64;
        (ms(b), ms(i))
    });
    let (go, budget_ms) = if let Some((b, i)) = clock {
        (format!("tc {}+{} ms", b, i), b + i)
    } else if let Some(n) = arg(&args, "--nodes") {
        (format!("go nodes {}", n), 30000u64)
    } else if let Some(d) = arg(&args, "--depth") {
        (format!("go depth {}", d), 60000)
    } else {
        let mt: u64 = arg(&args, "--movetime").and_then(|v| v.parse().ok()).unwrap_or(100);
        (format!("go movetime {}", mt), mt)
    };
    let cfg = Config {
        new_cmd: new_cmd.clone(),
        base_cmd: base_cmd.clone(),
        go: go.clone(),
        clock,
        timeout: Duration::from_millis(budget_ms * 4 + 10000),
        max_actions,
    };
    let lower = (beta / (1.0 - alpha)).ln();
    let upper = ((1.0 - beta) / alpha).ln();
    println!("SPRT  NEW: {}\n      BASE: {}", new_cmd, base_cmd);
    println!(
        "      {} | bounds [{}, {}] {} | alpha={} beta={} | LLR bounds [{:.2}, {:.2}] | {} | {} workers",
        go,
        elo0,
        elo1,
        if normalized { "nElo" } else { "Elo" },
        alpha,
        beta,
        lower,
        upper,
        if book.is_empty() { format!("generated openings, {} actions", book_plies) } else { format!("{} openings x2", book.len()) },
        conc
    );
    let log_path = arg(&args, "--log");
    let log = Arc::new(Mutex::new(log_path.map(|p| std::fs::File::create(p).expect("log file"))));
    let jsonl = Arc::new(Mutex::new(
        arg(&args, "--games-out").map(|p| std::fs::OpenOptions::new().create(true).append(true).open(p).expect("games file")),
    ));
    // Deterministic opening order from the seed.
    let mut order: Vec<usize> = (0..book.len()).collect();
    let mut st = seed.wrapping_mul(0x9E3779B97F4A7C15) | 1;
    for i in (1..order.len()).rev() {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        order.swap(i, (st % (i as u64 + 1)) as usize);
    }
    let book = Arc::new(book);
    let order = Arc::new(order);
    let next_pair = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));
    struct Shared {
        penta: Penta,
        wdl: [usize; 3],
        errors: usize,
        reasons: std::collections::BTreeMap<String, usize>,
        lengths: usize,
    }
    let shared = Arc::new(Mutex::new(Shared {
        penta: Penta::default(),
        wdl: [0; 3],
        errors: 0,
        reasons: Default::default(),
        lengths: 0,
    }));
    let start = Instant::now();
    let mut handles = Vec::new();
    for w in 0..conc {
        let (cfg, book, order, next_pair, done, shared, log, jsonl) = (
            cfg.clone(),
            book.clone(),
            order.clone(),
            next_pair.clone(),
            done.clone(),
            shared.clone(),
            log.clone(),
            jsonl.clone(),
        );
        handles.push(std::thread::spawn(move || {
            let mut procs: [Option<Proc>; 2] = [None, None];
            let mut book_engine = AlphaBeta::new();
            book_engine.set_option("Hash", &OptionValue::Int(8));
            while !done.load(Ordering::SeqCst) {
                let k = next_pair.fetch_add(1, Ordering::SeqCst);
                if 2 * k >= max_games {
                    break;
                }
                let generated;
                let op = if book.is_empty() {
                    generated = gen_opening(&mut book_engine, seed, k, book_plies);
                    &generated
                } else {
                    &book[order[k % order.len()]]
                };
                let mut results = Vec::new();
                for new_first in [true, false] {
                    let mut attempt = 0;
                    loop {
                        if procs[0].is_none() {
                            procs[0] = Proc::spawn(&cfg.new_cmd, "NEW").ok();
                        }
                        if procs[1].is_none() {
                            procs[1] = Proc::spawn(&cfg.base_cmd, "BASE").ok();
                        }
                        if procs[0].is_none() || procs[1].is_none() {
                            eprintln!("worker {}: engine failed to start", w);
                            done.store(true, Ordering::SeqCst);
                            return;
                        }
                        match play_game(&cfg, &mut procs, op, new_first) {
                            Ok(rec) => {
                                results.push(rec);
                                break;
                            }
                            Err(e) => {
                                // Protocol failure: the failing engine loses if identifiable.
                                let who = if e.starts_with("NEW") { 0 } else if e.starts_with("BASE") { 1 } else { 2 };
                                procs = [None, None];
                                shared.lock().unwrap().errors += 1;
                                eprintln!("worker {}: {}", w, e);
                                attempt += 1;
                                if who < 2 || attempt >= 2 {
                                    let result = match who {
                                        0 => GameResult::Loss,
                                        1 => GameResult::Win,
                                        _ => GameResult::Draw,
                                    };
                                    results.push(GameRecord { result, reason: format!("protocol: {}", e), actions: 0, log: String::new(), evals: Vec::new() });
                                    break;
                                }
                            }
                        }
                    }
                }
                let mut sh = shared.lock().unwrap();
                sh.penta.add(results[0].result, results[1].result);
                for (gi, r) in results.iter().enumerate() {
                    sh.wdl[match r.result {
                        GameResult::Win => 0,
                        GameResult::Draw => 1,
                        GameResult::Loss => 2,
                    }] += 1;
                    *sh.reasons.entry(r.reason.split(':').next().unwrap_or("").split(" (").next().unwrap_or("").to_string()).or_insert(0) += 1;
                    sh.lengths += r.actions;
                    if let Some(f) = jsonl.lock().unwrap().as_mut() {
                        let score = match r.result {
                            GameResult::Win => 1.0,
                            GameResult::Draw => 0.5,
                            GameResult::Loss => 0.0,
                        };
                        let esc = |x: &str| x.replace('\\', "\\\\").replace('"', "\\\"");
                        let _ = writeln!(
                            f,
                            "{{\"game\":{},\"pair\":{},\"new_first\":{},\"score\":{},\"reason\":\"{}\",\"actions\":{},\"position\":\"{}\",\"book\":{},\"moves\":\"{}\",\"eval_scale\":{},\"evals\":[{}]}}",
                            2 * k + gi,
                            k,
                            gi == 0,
                            score,
                            esc(&r.reason),
                            r.actions,
                            esc(&op.position),
                            op.book,
                            esc(r.log.trim()),
                            REPORT_SCALE,
                            r.evals.iter().map(|e| e.map(|v| v.to_string()).unwrap_or("null".into())).collect::<Vec<_>>().join(",")
                        );
                        let _ = f.flush();
                    }
                    if let Some(f) = log.lock().unwrap().as_mut() {
                        let _ = writeln!(
                            f,
                            "pair={} game={} new_first={} result={:?} reason=\"{}\" actions={} position={} moves {}",
                            k,
                            gi,
                            gi == 0,
                            r.result,
                            r.reason,
                            r.actions,
                            op.position,
                            r.log
                        );
                    }
                }
                let l = llr(&sh.penta, elo0, elo1, normalized);
                let e = estimate(&sh.penta);
                let games = sh.wdl.iter().sum::<usize>();
                if !compact || games % 50 == 0 {
                println!(
                    "games {:>5} | W {:>4} D {:>4} L {:>4} | penta [{} {} {} {} {}] | Elo {:+.1} ±{:.1} nElo {:+.1} ±{:.1} LOS {:.1}% | LLR {:+.2} [{:.2},{:.2}] | avg len {:.0} | {:.0}s",
                    games,
                    sh.wdl[0],
                    sh.wdl[1],
                    sh.wdl[2],
                    sh.penta.ll,
                    sh.penta.ld,
                    sh.penta.wl + sh.penta.dd,
                    sh.penta.wd,
                    sh.penta.ww,
                    e.elo,
                    e.err,
                    e.nelo,
                    e.nerr,
                    100.0 * e.los,
                    l,
                    lower,
                    upper,
                    sh.lengths as f64 / games.max(1) as f64,
                    start.elapsed().as_secs_f64()
                );
                }
                if (l >= upper || l <= lower) && games >= min_games {
                    done.store(true, Ordering::SeqCst);
                }
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    let sh = shared.lock().unwrap();
    let l = llr(&sh.penta, elo0, elo1, normalized);
    let e = estimate(&sh.penta);
    let games = sh.wdl.iter().sum::<usize>();
    println!("\n==== Final Summary ====");
    println!("NEW:  {}\nBASE: {}\n{}", new_cmd, base_cmd, go);
    println!(
        "Games: {} (W {} / D {} / L {}), pairs {}, protocol errors {}",
        games,
        sh.wdl[0],
        sh.wdl[1],
        sh.wdl[2],
        sh.penta.pairs(),
        sh.errors
    );
    println!(
        "Pentanomial [LL LD WL+DD WD WW]: [{} {} {} {} {}]",
        sh.penta.ll,
        sh.penta.ld,
        sh.penta.wl + sh.penta.dd,
        sh.penta.wd,
        sh.penta.ww
    );
    println!("Elo: {:+.1} ± {:.1} (95%), nElo: {:+.1} ± {:.1}, LOS: {:.2}%", e.elo, e.err, e.nelo, e.nerr, 100.0 * e.los);
    println!(
        "LLR: {:+.3} [{:.3}, {:.3}] => {}",
        l,
        lower,
        upper,
        if l >= upper {
            "H1 ACCEPTED (NEW is stronger)"
        } else if l <= lower {
            "H0 ACCEPTED (no gain)"
        } else {
            "INCONCLUSIVE"
        }
    );
    println!("Termination reasons: {:?}", sh.reasons);
    println!("Elapsed: {:.0}s", start.elapsed().as_secs_f64());
}
