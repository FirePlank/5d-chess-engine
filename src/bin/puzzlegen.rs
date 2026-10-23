//! Puzzle generator: mines the SPRT game records (`games/sprt/games_*.jsonl`) for 5D puzzles.
//!
//! Stages, cheapest first:
//!   0 scan   - each game's recorded evaluations -> candidate positions (no engine)
//!   1 replay - the positions themselves: filters, dedup by position hash
//!   2 verify - one winning idea: the key moves the win depends on (root exclusion)
//!   3 cook   - the forced line, the same rule at every solver turn
//!   4 rate   - 5D difficulty features -> rating and themes
//!
//! In 5D an action plays a move on every board of the present, so two actions that share the
//! decisive move and differ only on another board are the same idea. A move of the best action
//! is a *key move* when every action without it fails (searched with the root actions playing
//! it excluded); a puzzle needs at least one, and a solver's action is right when it plays all
//! the key moves. Positions never repeat (time moves on), so there is no draw-by-repetition
//! puzzle; draws are a fraction of a percent of the games anyway.
//!
//! Scores are the engine's reported centipawns (`engine::report_cp`): +100 is about a 75%
//! expected score in these games (scripts/eval_calibration.py), +250 about 95%.
//!
//!   puzzlegen [--games FILE...] [--out games/puzzles/puzzles.jsonl] [--threads N]
//!             [--max-files N] [--max-games N] [--per-game N] [--dry-run] [--rate-only]
//!             [--export site.jsonl [--shard I/N] [--deep-nodes N]]
//!             [--screen-nodes N] [--verify-nodes N] [--key-nodes N] [--cook-nodes N]
//!             [--defend-nodes N] [--rate-nodes N] [--budget-secs N]
//!
//! Output: one JSON object per line. A run resumes past the candidates in `<out>.progress`.

use kairos::core::action::FullMove;
use kairos::core::bitboard::{marked_pos, pmask};
use kairos::core::board::Board;
use kairos::core::check_position::CheckPosition;
use kairos::core::hypercuboid::{has_physical_check, Entry, HcInfo};
use kairos::core::piece::*;
use kairos::core::promotion::{promotion_letters, PromotionOptions};
use kairos::core::state::State;
use kairos::core::variants::{apply_uci_moves, position_from_uci};
use kairos::engine::ab::AlphaBeta;
use kairos::engine::{Limits, OptionValue};
use kairos::obj;
use kairos::web::json::Json;
use std::collections::{BTreeSet, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// scales and thresholds (reported centipawns)
// ---------------------------------------------------------------------------

/// A mate in n moves is `MATE_CP - n` (negative for being mated).
const MATE_CP: i32 = 100_000;
/// The game records flatten mates to this, with no distance.
const GAME_MATE: i32 = 20_000;
/// Logistic slope of the reported scale: a 75% expected score at +105 (fitted on 74k games).
const WC_K: f64 = 0.010_46;

/// A turning point: this many earlier actions all below `TURN_QUIET_CEIL`, then a mover's
/// score of `TURN_WIN_FLOOR` or more. The recorded searches are shallow (depth 3-6) and swing,
/// so the window is several actions rather than one.
const QUIET_WINDOW: usize = 6;
const TURN_QUIET_CEIL: i32 = 120;
const TURN_WIN_FLOOR: i32 = 250;
/// A move wins past `WON_CP` (about 96%); at or under `HELD_CP` (about 78%) the win is gone. A
/// puzzle's answer must cross that boundary, every other idea stay below it.
const WON_CP: i32 = 300;
const HELD_CP: i32 = 120;
/// The same crossing for a defence: the answer keeps the game playable, the rest lose.
const HOLD_FLOOR: i32 = -120;
const LOST_CEIL: i32 = -300;
/// Missed save: the mover held at least this, and the next score shows them lost by this.
const DEF_HOLD_CP: i32 = 120;
const DEF_LOST_CP: i32 = 300;
/// Below this the position is a mop-up (pieces on the latest boards of the active timelines).
const MIN_PIECES: u32 = 6;
/// The win already in hand (material on the latest boards, solver's view).
const MAX_LEAD: i32 = 2400;
/// Candidates from one game at least this many actions apart.
const MIN_GAP: usize = 4;
/// How many of the solver's turns the start may be moved back to where the forced line begins.
const MAX_REWIND: usize = 4;
/// A mate shot's game ends in the mover's checkmate within this many of their turns.
const MATE_SHOT_WITHIN: usize = 12;
/// Screen: a second idea (sharing no move with the first) this good wins as well, whatever a
/// deeper search finds (well above `HELD_CP`, so shallow noise cannot discard a puzzle).
const SCREEN_SECOND: i32 = 450;
/// Defender replies within this win-chance distance of the best are equally good to show.
const DEFENCE_WINDOW: f64 = 0.10;
/// Caps on counting legal actions (5D positions have thousands).
const ROOT_COUNT_CAP: usize = 4096;
const REPLY_COUNT_CAP: usize = 1024;
/// Caps on the static root order searched for the key action (`shallow_rank`).
const ORDER_CAP: usize = 2048;

fn is_mate_win(s: i32) -> bool {
    s > MATE_CP - 1000
}

fn is_mate_loss(s: i32) -> bool {
    s < -(MATE_CP - 1000)
}

fn wins(s: i32) -> bool {
    s >= WON_CP
}

fn win_chances(s: i32) -> f64 {
    if is_mate_win(s) {
        return 1.0;
    }
    if is_mate_loss(s) {
        return -1.0;
    }
    2.0 / (1.0 + (-WC_K * s as f64).exp()) - 1.0
}

/// The one rule, at every solver turn: the answer lands on the good side of an outcome
/// boundary (win vs not, or playable vs lost) and the best other idea on the bad side.
fn valid(best: i32, runner: Option<i32>) -> bool {
    let w = wins(best);
    let holds = best >= HOLD_FLOOR;
    if !w && !holds {
        return false;
    }
    let Some(r) = runner else { return true };
    (w && !is_mate_win(r) && r <= HELD_CP) || (holds && (is_mate_loss(r) || r <= LOST_CEIL))
}

// ---------------------------------------------------------------------------
// config
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Cfg {
    games: Vec<PathBuf>,
    out: PathBuf,
    threads: usize,
    max_files: usize,
    max_games: usize,
    per_game: usize,
    dry_run: bool,
    rate_only: bool,
    /// Write the site's puzzle file (see `export_site`) from `out` instead of mining.
    export: Option<PathBuf>,
    /// With `export`: only puzzle `i` of every `n` (shard `i` of `n`, counting from 1).
    shard: Option<(usize, usize)>,
    /// With `export`: nodes of the deep check that every solver step still has one idea (0: off).
    deep_nodes: u64,
    screen_nodes: u64,
    verify_nodes: u64,
    key_nodes: u64,
    cook_nodes: u64,
    defend_nodes: u64,
    rate_nodes: u64,
    budget: Duration,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg {
            games: Vec::new(),
            out: PathBuf::from("games/puzzles/puzzles.jsonl"),
            threads: std::thread::available_parallelism().map(|n| n.get().saturating_sub(2).max(1)).unwrap_or(4),
            max_files: usize::MAX,
            max_games: usize::MAX,
            per_game: 6,
            dry_run: false,
            rate_only: false,
            export: None,
            shard: None,
            deep_nodes: 3_000_000,
            screen_nodes: 60_000,
            verify_nodes: 400_000,
            key_nodes: 250_000,
            cook_nodes: 300_000,
            defend_nodes: 150_000,
            rate_nodes: 400_000,
            budget: Duration::from_secs(150),
        }
    }
}

fn parse_args() -> Cfg {
    let mut cfg = Cfg::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_default();
        match a.as_str() {
            "--games" => cfg.games.push(PathBuf::from(val())),
            "--out" => cfg.out = PathBuf::from(val()),
            "--threads" => cfg.threads = val().parse().unwrap_or(cfg.threads),
            "--max-files" => cfg.max_files = val().parse().unwrap_or(usize::MAX),
            "--max-games" => cfg.max_games = val().parse().unwrap_or(usize::MAX),
            "--per-game" => cfg.per_game = val().parse().unwrap_or(cfg.per_game),
            "--dry-run" => cfg.dry_run = true,
            "--rate-only" => cfg.rate_only = true,
            "--export" => cfg.export = Some(PathBuf::from(val())),
            "--deep-nodes" => cfg.deep_nodes = val().parse().unwrap_or(cfg.deep_nodes),
            "--shard" => {
                let v = val();
                cfg.shard = v.split_once('/').and_then(|(i, n)| Some((i.parse().ok()?, n.parse().ok()?))).filter(|&(i, n)| i >= 1 && i <= n);
                if cfg.shard.is_none() {
                    eprintln!("--shard wants I/N with 1 <= I <= N");
                    std::process::exit(2);
                }
            }
            "--screen-nodes" => cfg.screen_nodes = val().parse().unwrap_or(cfg.screen_nodes),
            "--verify-nodes" => cfg.verify_nodes = val().parse().unwrap_or(cfg.verify_nodes),
            "--key-nodes" => cfg.key_nodes = val().parse().unwrap_or(cfg.key_nodes),
            "--cook-nodes" => cfg.cook_nodes = val().parse().unwrap_or(cfg.cook_nodes),
            "--defend-nodes" => cfg.defend_nodes = val().parse().unwrap_or(cfg.defend_nodes),
            "--rate-nodes" => cfg.rate_nodes = val().parse().unwrap_or(cfg.rate_nodes),
            "--budget-secs" => cfg.budget = Duration::from_secs(val().parse().unwrap_or(150)),
            "--help" | "-h" => {
                println!("{}", include_str!("puzzlegen.rs").lines().take_while(|l| l.starts_with("//!")).map(|l| l.trim_start_matches("//!").trim_start_matches(' ')).collect::<Vec<_>>().join("\n"));
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument {other} (--help)");
                std::process::exit(2);
            }
        }
    }
    if cfg.games.is_empty() {
        // Every SPRT game record, newest first.
        let mut files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir("games/sprt")
            .map(|d| {
                d.filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("games_") && n.ends_with(".jsonl")))
                    .map(|p| (p.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), p))
                    .collect()
            })
            .unwrap_or_default();
        files.sort_by(|a, b| b.0.cmp(&a.0));
        cfg.games = files.into_iter().map(|f| f.1).collect();
    }
    cfg.games.truncate(cfg.max_files);
    cfg
}

// ---------------------------------------------------------------------------
// stage 0: scan the recorded evaluations
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Source {
    /// The game turned here: quiet for `QUIET_WINDOW` actions, then decisive.
    TurningPoint,
    /// A mate the mover saw that their previous search had not.
    MateShot,
    /// The mover was holding, then the next score showed them lost: the position before
    /// their action asks for the only defence.
    MissedSave,
}

impl Source {
    fn name(self) -> &'static str {
        match self {
            Source::TurningPoint => "turning_point",
            Source::MateShot => "mate_shot",
            Source::MissedSave => "missed_save",
        }
    }
}

struct Game {
    file: String,
    start: String,
    /// One entry per action: its moves, space-separated, without "submit".
    actions: Vec<String>,
}

struct Cand {
    game: Arc<Game>,
    /// The puzzle position is the one before this action.
    ply: usize,
    source: Source,
    scan_eval: i32,
}

/// Candidate plies of one game record (a line of a `games_*.jsonl`).
/// A game and its candidates: (ply, source, the recorded score there).
type Scanned = (Game, Vec<(usize, Source, i32)>);

fn scan_game(line: &str, file: &str, per_game: usize) -> Option<Scanned> {
    let j = Json::parse(line).ok()?;
    // Scores in reported centipawns only (records before `eval_scale` used internal units).
    if j.get("eval_scale").int() != Some(4800) {
        return None;
    }
    let start = j.get("position").str()?.to_string();
    let actions: Vec<String> = j
        .get("moves")
        .str()?
        .split("submit")
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty())
        .collect();
    let evals: Vec<Option<i32>> = j.get("evals").arr().iter().map(|e| e.int().map(|v| v as i32)).collect();
    // The opening book's actions come first and carry no score: score i is action off + i's.
    let off = actions.len().checked_sub(evals.len())?;
    // How the game ended for action i's mover (1 won, 0 lost): the recorded scores come from
    // shallow searches and swing, so a candidate must agree with the result.
    let new_score = j.get("score").num()?;
    let new_first = j.get("new_first").bool()?;
    let result = |i: usize| if (off + i).is_multiple_of(2) == new_first { new_score } else { 1.0 - new_score };
    let mated_at_end = j.get("reason").str() == Some("checkmate");
    let n = evals.len();
    if n < QUIET_WINDOW + 4 {
        return None;
    }
    // evals[i]: the score of action i's mover, from their side, in the position before it.
    // Seen from action i's mover, action j's score is the same or negated.
    let view = |i: usize, j: usize| evals[j].map(|e| if (i - j).is_multiple_of(2) { e } else { -e });
    let last = n - 1; // the final action ends the game: no puzzle there
    let mut scored: Vec<(i32, usize, Source, i32)> = Vec::new();

    for i in QUIET_WINDOW..last {
        let Some(s) = evals[i] else { continue };
        // Turning point: the earliest decisive score after a quiet stretch.
        // The mover's next search must agree (shallow scores swing).
        let next_own = if i + 2 < n { evals[i + 2] } else { Some(s) };
        if s >= TURN_WIN_FLOOR && result(i) == 1.0 && next_own.is_some_and(|v| v >= TURN_WIN_FLOOR) && (i - QUIET_WINDOW..i).all(|j| view(i, j).is_some_and(|v| v < TURN_QUIET_CEIL && v.abs() < GAME_MATE)) {
            scored.push((10_000 - i as i32, i, Source::TurningPoint, s));
        }
        // Mate shot: a mate that is not the mover's previous mate counted down.
        // A mate the mover's next search still sees, and the game ends in their mate soon after.
        if s >= GAME_MATE
            && result(i) == 1.0
            && mated_at_end
            && n - i <= 2 * MATE_SHOT_WITHIN
            && (i + 2 >= n || evals[i + 2] == Some(GAME_MATE))
            && evals[i - 2] != Some(GAME_MATE)
        {
            let surprise = if evals[i - 2].is_some_and(|p| p < TURN_WIN_FLOOR) { 100 } else { 50 };
            scored.push((surprise * 10 + 9_000 - i as i32, i, Source::MateShot, s));
        }
        // Missed save: holding (also on the mover's two previous searches), then lost.
        if (-DEF_HOLD_CP..TURN_WIN_FLOOR).contains(&s)
            && result(i) == 0.0
            && view(i, i + 1).is_some_and(|a| a <= -DEF_LOST_CP)
            && (1..=2).all(|k| view(i, i - 2 * k).is_some_and(|v| v >= -DEF_HOLD_CP))
        {
            scored.push((5_000 - i as i32, i, Source::MissedSave, s));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out: Vec<(usize, Source, i32)> = Vec::new();
    for (_, i, src, s) in scored {
        if out.len() >= per_game {
            break;
        }
        if out.iter().all(|c| c.0.abs_diff(off + i) >= MIN_GAP) {
            out.push((off + i, src, s));
        }
    }
    if out.is_empty() {
        return None;
    }
    let keep = out.iter().map(|c| c.0).max().unwrap_or(0) + 1;
    let mut actions = actions;
    actions.truncate(keep);
    Some((Game { file: file.to_string(), start, actions }, out))
}

fn scan(cfg: &Cfg) -> Vec<Cand> {
    let files: Vec<PathBuf> = cfg.games.clone();
    let next = AtomicUsize::new(0);
    let games_seen = AtomicUsize::new(0);
    let out: Mutex<Vec<Cand>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..cfg.threads {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                let Some(path) = files.get(k) else { break };
                let Ok(f) = std::fs::File::open(path) else { continue };
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                let mut local = Vec::new();
                for line in BufReader::new(f).lines().map_while(Result::ok) {
                    if games_seen.fetch_add(1, Ordering::Relaxed) >= cfg.max_games {
                        break;
                    }
                    if let Some((g, cands)) = scan_game(&line, &name, cfg.per_game) {
                        let g = Arc::new(g);
                        for (ply, source, scan_eval) in cands {
                            local.push(Cand { game: Arc::clone(&g), ply, source, scan_eval });
                        }
                    }
                }
                out.lock().unwrap().extend(local);
            });
        }
    });
    let n = games_seen.load(Ordering::Relaxed).min(cfg.max_games);
    let cands = out.into_inner().unwrap();
    println!("stage 0: {} games in {} files -> {} candidate positions", n, files.len(), cands.len());
    cands
}

// ---------------------------------------------------------------------------
// stage 1: positions
// ---------------------------------------------------------------------------

/// The position before action `ply` of `g`.
fn state_at(g: &Game, ply: usize) -> Option<State> {
    let mut s = position_from_uci(&g.start).ok()?;
    let moves: String = g.actions[..ply].iter().map(|a| format!("{a} submit ")).collect();
    apply_uci_moves(&mut s, &moves).ok()?;
    Some(s)
}

/// Rough material value (royals 0).
fn value(p: Piece) -> i32 {
    match p {
        PAWN_W | BRAWN_W => 100,
        KNIGHT_W | COMMON_KING_W => 300,
        BISHOP_W => 320,
        UNICORN_W | DRAGON_W => 350,
        ROOK_W => 500,
        PRINCESS_W => 750,
        QUEEN_W => 900,
        _ => 0,
    }
}

/// The latest board of every timeline.
fn latest_boards(s: &State) -> Vec<(i32, Board)> {
    let (lo, hi) = s.get_lines_range();
    (lo..=hi)
        .filter_map(|l| {
            let (t, c) = s.get_timeline_end(l);
            s.get_board_ptr(l, t, c).map(|b| (l, *b))
        })
        .collect()
}

fn pieces_of(b: &Board, side: bool) -> impl Iterator<Item = i32> + '_ {
    marked_pos(b.hostile(!side))
}

/// Material of `side` minus the other's, on the latest boards of every timeline.
fn material_lead(s: &State, side: bool) -> i32 {
    latest_boards(s)
        .iter()
        .map(|(_, b)| pieces_of(b, side).map(|sq| value(b.kind(sq))).sum::<i32>() - pieces_of(b, !side).map(|sq| value(b.kind(sq))).sum::<i32>())
        .sum()
}

/// Pieces on the latest boards of the active timelines.
fn active_pieces(s: &State) -> u32 {
    let (amin, amax) = s.get_active_range();
    latest_boards(s).iter().filter(|(l, _)| *l >= amin && *l <= amax).map(|(_, b)| (b.occupied() & !b.wall()).count_ones()).sum()
}

/// Legal actions of `s`, at most `cap`.
fn count_actions(s: &State, cap: usize) -> usize {
    let (info, ss) = HcInfo::build_hc(s);
    info.search(s, ss).take(cap).count()
}

/// Whether the side to move is checkmated (no legal action, in check).
fn mated(s: &State) -> Option<bool> {
    if count_actions(s, 1) > 0 {
        return None;
    }
    Some(s.has_phantom_check())
}

// ---------------------------------------------------------------------------
// the engine
// ---------------------------------------------------------------------------

/// One root line: its score (reported cp; mates `MATE_CP - n`), and the root action's moves.
#[derive(Clone, Debug)]
struct Line {
    score: i32,
    action: String,
}

struct Searched {
    lines: Vec<Line>,
    /// The best action at each completed depth.
    by_depth: Vec<(i32, String)>,
}

struct Engine {
    ab: AlphaBeta,
}

impl Engine {
    fn new() -> Self {
        let mut ab = AlphaBeta::new();
        ab.set_option("Hash", &OptionValue::Int(16));
        ab.set_option("Threads", &OptionValue::Int(1));
        Engine { ab }
    }

    /// Search `s` to `nodes` with `multipv` lines, the root actions playing any of `exclude` left
    /// out. No lines: no action to play (all excluded, or none legal).
    fn search(&mut self, s: &State, nodes: u64, multipv: usize, exclude: &[FullMove]) -> Searched {
        self.ab.set_option("MultiPV", &OptionValue::Int(multipv as i64));
        self.ab.set_root_exclude(exclude);
        self.ab.new_game();
        let stop = AtomicBool::new(false);
        let mut infos: Vec<String> = Vec::new();
        let limits = Limits { nodes: Some(nodes), ..Default::default() };
        let best = self.ab.find_best_move(s, &limits, &stop, &mut |l| infos.push(l));
        self.ab.set_root_exclude(&[]);
        let mut lines: Vec<(i32, usize, Line)> = Vec::new();
        let mut by_depth: Vec<(i32, String)> = Vec::new();
        for info in &infos {
            if let Some((depth, k, line)) = parse_info(info) {
                if k <= 1 {
                    by_depth.push((depth, line.action.clone()));
                }
                lines.push((depth, k.max(1), line));
            }
        }
        // The deepest depth's lines, by MultiPV index.
        let top = lines.iter().map(|l| l.0).max().unwrap_or(0);
        let mut last: Vec<(usize, Line)> = lines.into_iter().filter(|l| l.0 == top).map(|l| (l.1, l.2)).collect();
        last.sort_by_key(|l| l.0);
        last.dedup_by_key(|l| l.0);
        let mut out: Vec<Line> = last.into_iter().map(|l| l.1).collect();
        // A forced or book action reports no line: the engine's answer is the line.
        if out.is_empty()
            && let Some(a) = best
        {
            let action = a.moves().iter().map(|m| m.to_string()).collect::<Vec<_>>().join(" ");
            out.push(Line { score: score_of(self.ab.last_score(), self.ab.last_proven()), action });
        }
        Searched { lines: out, by_depth }
    }
}

fn score_of(internal: i32, proven: bool) -> i32 {
    let mate = kairos::engine::ab::MATE;
    if proven && internal.abs() > mate - 1000 {
        let n = (mate - internal.abs() + 1) / 2;
        return if internal > 0 { MATE_CP - n } else { -(MATE_CP - n) };
    }
    kairos::engine::report_cp(internal)
}

/// (depth, MultiPV index (0: none), line) of an `info` line with a score and a pv.
fn parse_info(info: &str) -> Option<(i32, usize, Line)> {
    let t: Vec<&str> = info.split_whitespace().collect();
    let at = |k: &str| t.iter().position(|w| *w == k);
    let depth: i32 = t.get(at("depth")? + 1)?.parse().ok()?;
    let k: usize = at("multipv").and_then(|i| t.get(i + 1)?.parse().ok()).unwrap_or(0);
    let si = at("score")?;
    let score = match (*t.get(si + 1)?, t.get(si + 2)?) {
        ("cp", v) => v.parse().ok()?,
        ("mate", v) => {
            let n: i32 = v.parse().ok()?;
            if n > 0 || *v == "+0" { MATE_CP - n } else { -(MATE_CP + n) }
        }
        _ => return None,
    };
    let pi = at("pv")?;
    let action: Vec<&str> = t[pi + 1..].iter().take_while(|w| **w != "submit").copied().collect();
    if action.is_empty() {
        return None;
    }
    Some((depth, k, Line { score, action: action.join(" ") }))
}

/// A move of an action's text ("(0T5)b3(0T1)f7", "(0T2)h7h8N").
fn parse_move(tok: &str) -> Option<FullMove> {
    let core = tok.trim_end_matches(|c: char| c.is_ascii_alphabetic() && c.is_ascii_uppercase());
    FullMove::parse(core).ok()
}

fn action_moves(action: &str) -> Vec<(String, FullMove)> {
    action.split_whitespace().filter_map(|t| parse_move(t).map(|m| (t.to_string(), m))).collect()
}

/// `s` after `action` (and its submit), if legal.
fn play(s: &State, action: &str) -> Option<State> {
    let mut t = s.clone();
    apply_uci_moves(&mut t, &format!("{action} submit")).ok()?;
    // The mover may not leave a royal capturable.
    if t.royal_capturable(t.get_present().1) {
        return None;
    }
    Some(t)
}

/// The key moves of `best` (the moves every other idea lacks) and the best score without one,
/// or None when some other idea wins as well.
fn key_moves(e: &mut Engine, s: &State, best: &Line, nodes: u64) -> Option<(Vec<String>, Option<i32>)> {
    let moves = action_moves(&best.action);
    if moves.is_empty() {
        return None;
    }
    let mut keys = Vec::new();
    let mut runner: Option<i32> = None;
    for (tok, fm) in &moves {
        let r = e.search(s, nodes, 1, &[*fm]);
        let without = r.lines.first().map(|l| l.score);
        if std::env::var_os("PUZZLEGEN_DEBUG").is_some() {
            eprintln!("  key? {} best {} | without it {:?}: {:?}", tok, best.score, without, r.lines.first().map(|l| &l.action));
        }
        // Without this move the side to move has no action at all, or one that fails.
        let fails = match without {
            None => true,
            Some(w) => !valid_pair(best.score, w),
        };
        if fails {
            keys.push(tok.clone());
            runner = Some(runner.map_or(without.unwrap_or(-MATE_CP), |r: i32| r.max(without.unwrap_or(-MATE_CP))));
        }
    }
    if keys.is_empty() {
        return None;
    }
    Some((keys, runner))
}

/// Whether `other` keeps what `best` achieves (a second idea): both win, or both hold.
fn valid_pair(best: i32, other: i32) -> bool {
    !valid(best, Some(other))
}

/// Whether `action` plays every one of `keys`.
fn plays_keys(action: &str, keys: &[String]) -> bool {
    let a: Vec<FullMove> = action_moves(action).into_iter().map(|m| m.1).collect();
    keys.iter().all(|k| parse_move(k).is_some_and(|m| a.contains(&m)))
}

// ---------------------------------------------------------------------------
// stages 2-3: verify and cook
// ---------------------------------------------------------------------------

mod rej {
    use std::sync::atomic::{AtomicUsize, Ordering};
    pub const BAD_POSITION: usize = 0;
    pub const SCREENED: usize = 1;
    pub const WEAK: usize = 2;
    pub const NOT_ONLY: usize = 3;
    pub const COOK_FAILED: usize = 4;
    pub const TRIVIAL: usize = 5;
    pub const UNSOUND: usize = 6;
    pub const OK: usize = 7;
    pub const N: usize = 8;
    pub static COUNTS: [AtomicUsize; N] = [const { AtomicUsize::new(0) }; N];
    pub static SRC_SEEN: [AtomicUsize; 3] = [const { AtomicUsize::new(0) }; 3];
    pub static SRC_OK: [AtomicUsize; 3] = [const { AtomicUsize::new(0) }; 3];
    pub fn hit(k: usize) {
        COUNTS[k].fetch_add(1, Ordering::Relaxed);
    }
    pub fn report() {
        const NAMES: [&str; N] = [
            "position filtered",
            "screened out (shallow)",
            "best move neither wins nor holds",
            "another idea also works",
            "cook produced nothing",
            "trivial recapture",
            "failed soundness check",
            "accepted",
        ];
        println!("\ncandidate outcomes:");
        for (i, n) in NAMES.iter().enumerate() {
            println!("  {:<34}{:>8}", n, COUNTS[i].load(Ordering::Relaxed));
        }
        println!("  by source (seen / kept):");
        for (i, n) in ["turning_point", "mate_shot", "missed_save"].iter().enumerate() {
            let ok = SRC_OK[i].load(Ordering::Relaxed);
            let seen = SRC_SEEN[i].load(Ordering::Relaxed);
            println!("    {n:<16}{seen:>8}{ok:>8}  {:>5.1}%", if seen > 0 { 100.0 * ok as f64 / seen as f64 } else { 0.0 });
        }
    }
}

/// A solver turn of the forced line: the action shown and its key moves.
struct Turn {
    action: String,
    keys: Vec<String>,
}

struct Cooked {
    /// Solver and defender actions alternately, the solver's first.
    line: Vec<String>,
    keys: Vec<Vec<String>>,
    final_score: i32,
    ends_in_mate: bool,
    /// Win-chance gap between the answer and the best other idea at the start.
    root_margin: f64,
    /// Legal defender actions at each defender turn (capped).
    defender_replies: Vec<usize>,
}

/// The solver's verified turn in `s`, or None where the line stops being forced. `win`: the
/// puzzle is a win, so every turn must keep winning (else every turn must hold). A mate in one
/// with several mating actions ends the line with no key moves: any mate is the answer.
fn solver_turn(e: &mut Engine, s: &State, win: bool, nodes: u64, key_nodes: u64) -> Option<(Turn, i32, Option<i32>)> {
    let r = e.search(s, nodes, 1, &[]);
    let best = r.lines.first()?.clone();
    if !valid(best.score, None) || win != wins(best.score) {
        return None;
    }
    match key_moves(e, s, &best, key_nodes) {
        Some((keys, runner)) if valid(best.score, runner) => Some((Turn { action: best.action, keys }, best.score, runner)),
        _ if best.score == MATE_CP - 1 => Some((Turn { action: best.action, keys: Vec::new() }, best.score, None)),
        _ => None,
    }
}

fn cook(e: &mut Engine, start: &State, solver: bool, win: bool, cfg: &Cfg, deadline: Instant) -> Option<Cooked> {
    let mut s = start.clone();
    let mut line: Vec<String> = Vec::new();
    let mut keys: Vec<Vec<String>> = Vec::new();
    let mut had_choice: Vec<bool> = Vec::new();
    let mut defender_replies = Vec::new();
    let mut root_margin = 1.0f64;
    let mut final_score = 0;
    let mut ends_in_mate = false;
    let mut seen: HashSet<u64> = HashSet::new();
    seen.insert(s.hash());
    while Instant::now() < deadline {
        if let Some(in_check) = mated(&s) {
            ends_in_mate = in_check && s.get_present().1 != solver;
            break;
        }
        if s.get_present().1 == solver {
            let Some((turn, score, runner)) = solver_turn(e, &s, win, cfg.cook_nodes, cfg.key_nodes) else { break };
            // The first turn is the puzzle itself: several mates in one is no puzzle.
            if line.is_empty() && turn.keys.is_empty() {
                return None;
            }
            if line.is_empty() {
                root_margin = runner.map_or(2.0, |r| (win_chances(score) - win_chances(r)).clamp(0.0, 2.0));
            }
            final_score = score;
            had_choice.push(count_actions(&s, 2) > 1);
            let Some(next) = play(&s, &turn.action) else { break };
            line.push(turn.action);
            keys.push(turn.keys);
            s = next;
        } else {
            defender_replies.push(count_actions(&s, REPLY_COUNT_CAP));
            let r = e.search(&s, cfg.defend_nodes, 3, &[]);
            let Some(top) = r.lines.first().cloned() else { break };
            // Among replies as good as the best, show one that leaves the solver a single idea.
            let mut choice = top.action.clone();
            for l in &r.lines {
                if win_chances(top.score) - win_chances(l.score) > DEFENCE_WINDOW {
                    break;
                }
                let Some(after) = play(&s, &l.action) else { continue };
                if mated(&after).is_some() || solver_turn(e, &after, win, cfg.cook_nodes / 2, cfg.key_nodes / 2).is_some() {
                    choice = l.action.clone();
                    break;
                }
            }
            let Some(next) = play(&s, &choice) else { break };
            line.push(choice);
            s = next;
        }
        if !seen.insert(s.hash()) {
            return None;
        }
    }
    if !ends_in_mate && let Some(true) = mated(&s) {
        ends_in_mate = s.get_present().1 != solver;
    }
    // An odd length (the solver plays last), without a trailing solver action that had no
    // alternative; a mating action is never filler.
    loop {
        if !line.is_empty() && line.len().is_multiple_of(2) {
            line.pop();
            ends_in_mate = false;
            continue;
        }
        if !ends_in_mate && line.len() > 1 && !had_choice.get((line.len() - 1) / 2).copied().unwrap_or(true) {
            line.pop();
            keys.pop();
            line.pop();
            continue;
        }
        break;
    }
    keys.truncate(line.len().div_ceil(2));
    if line.is_empty() {
        return None;
    }
    Some(Cooked { line, keys, final_score, ends_in_mate, root_margin, defender_replies })
}

/// Walk the start back while the solver's earlier turn is already the only winning idea.
fn rewind(e: &mut Engine, c: &Cand, win: bool, cfg: &Cfg, deadline: Instant) -> usize {
    let mut ply = c.ply;
    for _ in 0..MAX_REWIND {
        if ply < 2 || Instant::now() >= deadline {
            break;
        }
        let Some(s) = state_at(&c.game, ply - 2) else { break };
        if solver_turn(e, &s, win, cfg.verify_nodes, cfg.key_nodes).is_none_or(|t| t.0.keys.is_empty()) {
            break;
        }
        ply -= 2;
    }
    ply
}

/// Moves of `action` that capture, with the square (line, xy) they land on.
fn captures(s: &State, action: &str) -> Vec<(i32, i32)> {
    let player = s.get_present().1;
    action_moves(action)
        .into_iter()
        .filter(|(_, m)| s.get_board_ptr(m.to.l(), m.to.t(), player).is_some_and(|b| b.hostile(player) & pmask(m.to.xy()) != 0))
        .map(|(_, m)| (m.to.l(), m.to.xy()))
        .collect()
}

/// The answer is only "take it back": the opponent's last action captured on a square, the key
/// move recaptures there, and it is the whole puzzle or the only piece that can.
fn trivial_recapture(g: &Game, ply: usize, start: &State, key: &[String], plies: usize) -> bool {
    if ply == 0 || key.len() != 1 {
        return false;
    }
    let Some(prev) = state_at(g, ply - 1) else { return false };
    let taken = captures(&prev, &g.actions[ply - 1]);
    let Some(m) = parse_move(&key[0]) else { return false };
    if !taken.contains(&(m.to.l(), m.to.xy())) || m.from.tl() != m.to.tl() {
        return false;
    }
    let player = start.get_present().1;
    let Some(b) = start.get_board_ptr(m.to.l(), m.to.t(), player) else { return false };
    let takers = (b.attacks_to(m.to.xy()) & b.hostile(!player)).count_ones();
    takers < 2 || plies == 1
}

/// Every action of the line is legal where it is played, and a claimed mate is one.
fn line_is_sound(start: &State, line: &[String], ends_in_mate: bool) -> bool {
    let mut s = start.clone();
    for a in line {
        match play(&s, a) {
            Some(n) => s = n,
            None => return false,
        }
    }
    !ends_in_mate || mated(&s) == Some(true)
}

// ---------------------------------------------------------------------------
// stage 4: 5D features, rating, themes
// ---------------------------------------------------------------------------

/// How a move moves through the multiverse.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Kind {
    /// On its own board.
    Physical,
    /// To another timeline's latest board (no new timeline).
    Jump,
    /// To a past board: a new timeline branches off.
    Branch,
}

fn kind_of(s: &State, m: &FullMove) -> Kind {
    if m.from.tl() == m.to.tl() {
        Kind::Physical
    } else if s.get_timeline_end(m.to.l()) == (m.to.t(), s.get_present().1) {
        Kind::Jump
    } else {
        Kind::Branch
    }
}

/// How hard one key move is to FIND (0 writes itself, ~1.2 has to be seen); the 5D reading
/// cost is `visibility`'s.
fn find_difficulty(s: &State, m: &FullMove, gives_check: bool, prev_capture: Option<(i32, i32)>, choices: usize) -> f64 {
    if choices <= 1 {
        return 0.0;
    }
    let player = s.get_present().1;
    let Some(src) = s.get_board_ptr(m.from.l(), m.from.t(), player) else { return 1.0 };
    let Some(dst) = s.get_board_ptr(m.to.l(), m.to.t(), player) else { return 1.0 };
    let mover = src.kind(m.from.xy());
    let victim = if dst.hostile(player) & pmask(m.to.xy()) != 0 { dst.kind(m.to.xy()) } else { NO_PIECE };
    if victim != NO_PIECE && prev_capture == Some((m.to.l(), m.to.xy())) && m.from.tl() == m.to.tl() {
        return 0.15;
    }
    if victim != NO_PIECE {
        let (vv, av) = (value(victim), value(mover).max(100));
        let defended = dst.attacks_to(m.to.xy()) & dst.hostile(player) != 0;
        if !defended {
            return if vv >= 300 { 0.20 } else { 0.40 };
        }
        if vv > av + 100 {
            return 0.55;
        }
        if vv + 100 < av {
            return if gives_check { 0.65 } else { 1.20 };
        }
        return 0.85;
    }
    if gives_check { 0.60 } else { 1.0 }
}

/// The 5D reading cost of a move: a travel is the move a player does not look for, more so
/// the further back in time or across timelines it reaches, and most when it branches.
fn visibility(m: &FullMove, kind: Kind) -> f64 {
    let (dx, dy) = ((m.to.x() - m.from.x()).abs(), (m.to.y() - m.from.y()).abs());
    let (dt, dl) = ((m.to.t() - m.from.t()).abs(), (m.to.l() - m.from.l()).abs());
    let board_reach = ((dx.max(dy).max(1) as f64).log2() / 3.0).clamp(0.0, 1.0);
    let multiverse_reach = (((dt + dl).max(1) as f64).log2() / 3.0).clamp(0.0, 1.0);
    match kind {
        Kind::Physical => 1.0 + 0.30 * board_reach,
        Kind::Jump => 1.30 + 0.20 * multiverse_reach,
        Kind::Branch => 1.55 + 0.30 * multiverse_reach,
    }
}

/// The board a move's piece stands on after the action (the latest board of its line, or of
/// the timeline it created).
fn landing_board(before: &State, after: &State, m: &FullMove) -> Option<Board> {
    let mover = before.get_present().1;
    let (lo0, hi0) = before.get_lines_range();
    let (lo1, hi1) = after.get_lines_range();
    let mut lines: Vec<i32> = vec![m.to.l()];
    lines.extend((lo1..lo0).chain(hi0 + 1..=hi1));
    for l in lines {
        let (t, c) = after.get_timeline_end(l);
        if let Some(b) = after.get_board_ptr(l, t, c)
            && b.hostile(!mover) & pmask(m.to.xy()) != 0
            && (l == m.to.l() || l < lo0 || l > hi0)
        {
            return Some(*b);
        }
    }
    None
}

/// A double attack by the piece on `at` of `b` (it belongs to `side`): two or more targets
/// worth taking (royal, more valuable, or undefended), weighted by worth and how far apart.
fn fork_strength(b: &Board, at: i32, side: bool, move_reach: f64) -> f64 {
    let mine = value(b.kind(at)).max(100);
    let mut targets: Vec<(i32, i32)> = Vec::new();
    for sq in marked_pos(b.hostile(side)) {
        if b.attacks_to(sq) & pmask(at) == 0 {
            continue;
        }
        let v = if b.royal() & pmask(sq) != 0 { 1000 } else { value(b.kind(sq)) };
        let defended = b.attacks_to(sq) & b.hostile(!side) != 0;
        if b.royal() & pmask(sq) != 0 || !defended || v > mine {
            targets.push((sq, v));
        }
    }
    if targets.len() < 2 {
        return 0.0;
    }
    targets.sort_by_key(|t| std::cmp::Reverse(t.1));
    let worth = ((targets[0].1 + targets[1].1) as f64 / 1800.0).clamp(0.0, 1.0);
    let apart = targets.iter().flat_map(|a| targets.iter().map(move |c| ((a.0 & 7) - (c.0 & 7)).abs().max((a.0 >> 3) - (c.0 >> 3)).abs())).max().unwrap_or(0);
    worth * (0.45 + 0.25 * move_reach + 0.30 * (apart as f64 / 7.0).clamp(0.0, 1.0))
}

/// Checks the side to move faces in `s`, by plane: (on the board, through time or timelines).
fn check_planes(s: &State) -> (bool, bool) {
    let stm = s.get_present().1;
    let checks = CheckPosition::for_phantom(s).checks(!stm, true);
    let physical = checks.iter().any(|m| m.from.tl() == m.to.tl());
    let superphysical = checks.iter().any(|m| m.from.tl() != m.to.tl());
    (physical, superphysical)
}

/// Candidate moves (one board's move, physical or a travel) the side to move has in `s`, and
/// how many of them capture or give check on their own board.
fn semimoves(s: &State) -> (usize, usize) {
    let (info, _) = HcInfo::build_hc(s);
    let defender = !s.get_present().1;
    let fba = info.first_branch_axis();
    let (mut all, mut forcing) = (0usize, 0usize);
    for n in 0..info.dimension().min(fba + 1) {
        for i in 1..info.axis_len(n) {
            match info.entry(n, i as u32) {
                Entry::Physical { b, victim, .. } => {
                    all += 1;
                    if *victim != NO_PIECE || has_physical_check(b, defender) {
                        forcing += 1;
                    }
                }
                Entry::Arriving { victim, .. } => {
                    all += 1;
                    if *victim != NO_PIECE {
                        forcing += 1;
                    }
                }
                _ => {}
            }
        }
    }
    (all, forcing)
}

/// Everything about a puzzle's difficulty, stored with it so `--rate-only` re-rates exactly.
#[derive(Clone, Default, Debug)]
struct Features {
    plies: usize,
    ends_in_mate: bool,
    mate_in: i32,
    /// Summed per-key-move find difficulty, weighted by 5D visibility and choice.
    calc_load: f64,
    /// Share of solver actions that check or capture.
    forcing: f64,
    shallow_rank: usize,
    depth_to_find: usize,
    margin: f64,
    mean_replies: f64,
    quiet_key: bool,
    sacrifice: i32,
    /// Active timelines and all timelines at the start; boards the solver must play.
    active_timelines: usize,
    timelines: usize,
    present_boards: usize,
    /// The key action's moves: how many are key, and the most demanding kind among them.
    key_count: usize,
    key_kind: usize,
    /// Solver turns whose key moves include a travel, and timelines the line creates.
    travel_turns: usize,
    branches_created: usize,
    /// Share of solver checks that come only from another board (through time or timelines).
    superphysical_checks: f64,
    /// The mate is delivered across boards.
    superphysical_mate: bool,
    /// Pieces on the boards the key moves start from or land on.
    relevant: usize,
    pieces: u32,
    /// Legal actions and candidate moves at the start (capped), and forcing candidates.
    root_actions: usize,
    candidates: usize,
    forcing_candidates: usize,
    /// Boards the solution's key moves touch, and their widest separation in (time, timeline).
    boards_touched: usize,
    span: i32,
    fork_peak: f64,
    mean_choice: f64,
}

/// The search-free features of `line` (with its key moves) from `start`.
fn board_features(start: &State, line: &[String], keys: &[Vec<String>]) -> Option<Features> {
    let solver = start.get_present().1;
    let mut f = Features {
        plies: line.len(),
        active_timelines: start.active_timelines(),
        timelines: {
            let (lo, hi) = start.get_lines_range();
            (hi - lo + 1) as usize
        },
        present_boards: {
            let present = start.get_present();
            let (amin, amax) = start.get_active_range();
            (amin..=amax).filter(|&l| start.get_timeline_end(l) == present).count()
        },
        pieces: active_pieces(start),
        ..Default::default()
    };
    let (cands, forcing_cands) = semimoves(start);
    f.candidates = cands;
    f.forcing_candidates = forcing_cands;
    f.root_actions = count_actions(start, ROOT_COUNT_CAP);
    let base_lead = material_lead(start, solver);
    let mut low_lead = base_lead;
    let mut s = start.clone();
    let mut prev_capture: Option<(i32, i32)> = None;
    let mut touched: BTreeSet<(i32, i32)> = BTreeSet::new();
    let mut relevant_lines: BTreeSet<i32> = BTreeSet::new();
    let (mut solver_turns, mut forcing_turns, mut check_turns, mut sp_check_turns) = (0usize, 0usize, 0usize, 0usize);
    let mut fork_by_turn: Vec<f64> = Vec::new();
    let mut choice_sum = 0.0;
    for (k, action) in line.iter().enumerate() {
        let after = play(&s, action)?;
        if k % 2 == 0 {
            let turn_keys = keys.get(k / 2)?;
            let moves: Vec<FullMove> = turn_keys.iter().filter_map(|t| parse_move(t)).collect();
            let (choices, _) = semimoves(&s);
            let choice = ((choices.max(1) as f64).log2() / 7.0).clamp(0.0, 1.0);
            choice_sum += choice;
            // A check only from another board (through time or across timelines): the one a
            // player does not see coming. Checks onto past boards are everywhere in 5D, so one
            // next to an ordinary check on the board does not count.
            let (phys_check, sp_any) = check_planes(&after);
            let sp_check = sp_any && !phys_check;
            let gives_check = after.has_phantom_check();
            let caps = captures(&s, action);
            if gives_check || !caps.is_empty() {
                forcing_turns += 1;
            }
            if gives_check {
                check_turns += 1;
                if sp_check {
                    sp_check_turns += 1;
                }
            }
            let mut turn_load = 0.0;
            let mut traveled = false;
            for m in &moves {
                let kind = kind_of(&s, m);
                f.key_kind = f.key_kind.max(kind as usize);
                traveled |= kind != Kind::Physical;
                turn_load += find_difficulty(&s, m, gives_check, prev_capture, choices) * visibility(m, kind);
                touched.insert((m.from.l(), m.from.t()));
                touched.insert((m.to.l(), m.to.t()));
                relevant_lines.insert(m.from.l());
                relevant_lines.insert(m.to.l());
                if k == 0 {
                    let cap = s.get_board_ptr(m.to.l(), m.to.t(), solver).is_some_and(|b| b.hostile(solver) & pmask(m.to.xy()) != 0);
                    f.quiet_key |= !cap && !gives_check;
                }
                if let Some(b) = landing_board(&s, &after, m) {
                    let reach = ((((m.to.x() - m.from.x()).abs().max((m.to.y() - m.from.y()).abs()) + (m.to.t() - m.from.t()).abs() + (m.to.l() - m.from.l()).abs()).max(1) as f64).log2() / 3.0).clamp(0.0, 1.0);
                    fork_by_turn.push(fork_strength(&b, m.to.xy(), solver, reach));
                }
            }
            // A combination over several boards: each extra key move is another idea to see,
            // and seeing that they work together.
            turn_load += 0.45 * moves.len().saturating_sub(1) as f64;
            f.calc_load += turn_load * (0.25 + 0.75 * choice);
            if traveled {
                f.travel_turns += 1;
            }
            f.key_count = f.key_count.max(moves.len());
            solver_turns += 1;
        }
        let (lo0, hi0) = s.get_lines_range();
        let (lo1, hi1) = after.get_lines_range();
        if k % 2 == 0 {
            f.branches_created += ((hi1 - lo1) - (hi0 - lo0)).max(0) as usize;
        }
        prev_capture = captures(&s, action).first().copied();
        s = after;
        if k % 2 == 0 {
            low_lead = low_lead.min(material_lead(&s, solver));
        }
    }
    if solver_turns == 0 {
        return None;
    }
    f.ends_in_mate = mated(&s) == Some(true) && s.get_present().1 != solver;
    if f.ends_in_mate {
        let (_, sp) = check_planes(&s);
        let (phys, _) = check_planes(&s);
        f.superphysical_mate = sp && !phys;
    }
    f.forcing = forcing_turns as f64 / solver_turns as f64;
    f.superphysical_checks = if check_turns > 0 { sp_check_turns as f64 / check_turns as f64 } else { 0.0 };
    f.sacrifice = (base_lead - low_lead).max(0);
    f.mean_choice = choice_sum / solver_turns as f64;
    f.boards_touched = touched.len();
    let tv: Vec<(i32, i32)> = touched.iter().copied().collect();
    f.span = tv.iter().flat_map(|a| tv.iter().map(move |b| (a.0 - b.0).abs().max((a.1 - b.1).abs()))).max().unwrap_or(0);
    f.relevant = latest_boards(start)
        .iter()
        .filter(|(l, _)| relevant_lines.contains(l))
        .map(|(_, b)| (b.occupied() & !b.wall()).count_ones() as usize)
        .sum();
    // A fork foreseen from the first move is harder than one met on arrival.
    let nf = fork_by_turn.len();
    f.fork_peak = fork_by_turn
        .iter()
        .enumerate()
        .map(|(i, v)| v * (0.55 + 0.45 * if nf < 2 { 1.0 } else { i as f64 / (nf - 1) as f64 }))
        .fold(0.0, f64::max);
    f.mate_in = if f.ends_in_mate { line.len().div_ceil(2) as i32 } else { 0 };
    Some(f)
}

fn n(v: f64, hi: f64) -> f64 {
    (v / hi).clamp(0.0, 1.0)
}

/// Absolute rating in Elo-like points. The chess terms (how much there is to calculate, how
/// hidden the answer is, how busy the boards are) follow the infinite-chess generator; on top,
/// 5D's own: how many timelines the solver must read, whether the answer travels (through time
/// or across timelines, worst when it branches), whether it is a combination over several
/// boards, and whether its checks come from another board.
fn rating(f: &Features) -> i32 {
    let forcing = f.forcing.clamp(0.0, 1.0);
    let mate_tail = ((f.mate_in as usize * 2).saturating_sub(1).saturating_sub(f.plies) as f64 * 0.2).min(2.0);
    let calc = n(f.calc_load + mate_tail, 5.5);
    // Engine order puts hundreds of 5D actions ahead of a hidden one: log-scaled.
    let obscure = (0.60 * n(((f.shallow_rank + 1) as f64).log2(), 9.0) + 0.40 * n(f.depth_to_find.saturating_sub(2) as f64, 8.0)) * (0.50 + 0.50 * calc);
    // Boards to hold in mind: pieces that bear on the tactic, and timelines.
    let complex = (0.55 * n(f.relevant as f64, 60.0) + 0.45 * n(f.timelines.saturating_sub(1) as f64, 6.0)) * (0.35 + 0.65 * calc);
    // Timelines to read at all, before any calculation: every active one is a board to scan.
    let timelines = n((f.active_timelines.max(1) as f64).log2(), 3.0);
    // The answer itself: a travel (most when it branches), a combination over boards, a check
    // or mate that comes from another board, timelines created along the way.
    let travel = match f.key_kind {
        2 => 1.0,
        1 => 0.65,
        _ => 0.0,
    };
    let travel_share = if f.plies > 0 { f.travel_turns as f64 / f.plies.div_ceil(2) as f64 } else { 0.0 };
    let combo = n(f.key_count.saturating_sub(1) as f64, 2.0);
    let across = 0.6 * f.superphysical_checks + 0.4 * if f.superphysical_mate { 1.0 } else { 0.0 };
    let branching = n(f.branches_created as f64, 3.0);
    let precision = n(((f.mean_replies.max(1.0)).log2()) / 9.0, 1.0) * f.mean_choice;
    let margin_edge = 1.0 - n(f.margin, 1.4);
    let cands = forcing * f.forcing_candidates as f64 + (1.0 - forcing) * f.candidates as f64;
    let width = n((cands.max(1.0)).log2() / 8.0, 1.0) * calc;
    let spread = n(((f.span + 1) as f64).log2(), 3.0) * (0.4 + 0.6 * calc);
    let r = 520.0
        + 1400.0 * calc * (1.0 - 0.45 * forcing)
        + 400.0 * obscure
        + 280.0 * complex
        + 380.0 * timelines
        + 260.0 * travel * (0.6 + 0.4 * travel_share)
        + 200.0 * combo
        + 160.0 * across
        + 100.0 * branching
        + 140.0 * spread
        + 280.0 * f.fork_peak
        + 140.0 * precision
        + 50.0 * margin_edge
        + 150.0 * if f.quiet_key { 1.0 } else { 0.0 }
        + 120.0 * n(f.sacrifice as f64, 900.0)
        + 60.0 * width
        - 100.0 * if f.ends_in_mate { 1.0 } else { 0.0 };
    ((r / 10.0).round() as i32 * 10).clamp(400, 3200)
}

fn themes(start: &State, line: &[String], keys: &[Vec<String>], f: &Features, win: bool, final_score: i32) -> BTreeSet<String> {
    let mut t: BTreeSet<String> = BTreeSet::new();
    let solver = start.get_present().1;
    if f.ends_in_mate {
        t.insert("mate".into());
        t.insert(match f.mate_in {
            1 => "mateIn1".into(),
            2 => "mateIn2".into(),
            3 => "mateIn3".into(),
            4 => "mateIn4".into(),
            _ => "mateIn5plus".into(),
        });
        t.insert(if f.superphysical_mate { "crossBoardMate" } else { "boardMate" }.into());
    } else if !win {
        t.insert("defensiveMove".into());
        t.insert("onlyMove".into());
    } else if f.mate_in > 0 {
        t.insert("matingAttack".into());
    } else if final_score > 600 {
        t.insert("crushing".into());
    } else {
        t.insert("advantage".into());
    }
    t.insert(
        match line.len() {
            1 => "oneMove",
            3 => "short",
            5 | 7 => "long",
            _ => "veryLong",
        }
        .into(),
    );
    t.insert(
        match f.active_timelines {
            0 | 1 => "oneTimeline",
            2 => "twoTimelines",
            _ => "manyTimelines",
        }
        .into(),
    );
    if f.quiet_key {
        t.insert("quietMove".into());
    }
    if f.sacrifice >= 200 {
        t.insert("sacrifice".into());
        if f.sacrifice >= 800 {
            t.insert("bigSacrifice".into());
        }
    }
    if f.fork_peak > 0.0 {
        t.insert("fork".into());
    }
    if f.key_count >= 2 {
        t.insert("multiBoard".into());
    }
    if f.branches_created > 0 {
        t.insert("newTimeline".into());
    }
    if f.superphysical_checks > 0.0 {
        t.insert("crossBoardCheck".into());
    }
    let mut s = start.clone();
    for (k, action) in line.iter().enumerate() {
        if k % 2 == 0 {
            for tok in keys.get(k / 2).into_iter().flatten() {
                let Some(m) = parse_move(tok) else { continue };
                match kind_of(&s, &m) {
                    Kind::Branch => {
                        t.insert("timeTravel".into());
                        t.insert("branchingTravel".into());
                    }
                    Kind::Jump => {
                        t.insert(if m.from.t() == m.to.t() { "timelineJump" } else { "timeTravel" }.into());
                    }
                    Kind::Physical => {}
                }
                if let Some(b) = s.get_board_ptr(m.to.l(), m.to.t(), solver)
                    && b.hostile(solver) & pmask(m.to.xy()) != 0
                {
                    t.insert("capture".into());
                    if b.attacks_to(m.to.xy()) & b.hostile(solver) == 0 {
                        t.insert("hangingPiece".into());
                    }
                }
                if tok.ends_with(|c: char| c.is_ascii_uppercase()) {
                    t.insert("promotion".into());
                    if !tok.ends_with('Q') {
                        t.insert("underPromotion".into());
                    }
                }
            }
        }
        let Some(next) = play(&s, action) else { break };
        if k % 2 == 0 && next.has_phantom_check() {
            t.insert("check".into());
        }
        s = next;
    }
    t
}

// ---------------------------------------------------------------------------
// the pipeline
// ---------------------------------------------------------------------------

/// The 5DUCI position of `s` on its own (every board), when it reads back to the same position.
fn standalone(s: &State) -> Option<String> {
    let (w, h) = s.get_board_size();
    let layout = if s.multiverse().layout().is_even() { "even" } else { "odd" };
    let promo = s.get_promotion_options();
    let promotions = if promo == PromotionOptions::QUEEN { String::new() } else { format!("promotions {} ", promotion_letters(promo)) };
    let boards: String = s.show_fen().lines().collect();
    let text = format!("size {w}x{h} {layout} {promotions}fen {boards}");
    let back = position_from_uci(&text).ok()?;
    (back.hash() == s.hash() && back.get_present() == s.get_present()).then_some(text)
}

fn solve(e: &mut Engine, c: &Cand, cfg: &Cfg) -> Option<Json> {
    let deadline = Instant::now() + cfg.budget;
    let s = state_at(&c.game, c.ply)?;
    let solver = s.get_present().1;
    if active_pieces(&s) < MIN_PIECES || count_actions(&s, 2) < 2 || material_lead(&s, solver) > MAX_LEAD {
        rej::hit(rej::BAD_POSITION);
        return None;
    }
    // Screen: a cheap search that only rejects on what deeper ones cannot overturn: a clearly
    // lost position, or a second line sharing no move with the first that wins big as well.
    let sc = e.search(&s, cfg.screen_nodes, 3, &[]);
    let Some(top) = sc.lines.first() else {
        rej::hit(rej::SCREENED);
        return None;
    };
    let top_moves: Vec<FullMove> = action_moves(&top.action).into_iter().map(|m| m.1).collect();
    let other_wins = sc.lines[1..].iter().any(|l| {
        let disjoint = action_moves(&l.action).iter().all(|m| !top_moves.contains(&m.1));
        disjoint && ((is_mate_win(top.score) && is_mate_win(l.score)) || l.score >= SCREEN_SECOND)
    });
    if top.score < LOST_CEIL - 200 || other_wins {
        rej::hit(rej::SCREENED);
        return None;
    }
    // Verify: one winning (or saving) idea.
    let v = e.search(&s, cfg.verify_nodes, 1, &[]);
    let Some(best) = v.lines.first().cloned() else {
        rej::hit(rej::SCREENED);
        return None;
    };
    if !valid(best.score, None) {
        rej::hit(rej::WEAK);
        return None;
    }
    if std::env::var_os("PUZZLEGEN_DEBUG").is_some() {
        eprintln!("candidate {} {} best {} {}", c.game.file, c.ply, best.score, best.action);
    }
    let Some((_, runner)) = key_moves(e, &s, &best, cfg.key_nodes) else {
        rej::hit(rej::NOT_ONLY);
        return None;
    };
    if !valid(best.score, runner) {
        rej::hit(rej::NOT_ONLY);
        return None;
    }
    let win = wins(best.score);
    let ply = rewind(e, c, win, cfg, deadline);
    let start = state_at(&c.game, ply)?;
    let Some(cooked) = cook(e, &start, solver, win, cfg, deadline) else {
        rej::hit(rej::COOK_FAILED);
        return None;
    };
    if trivial_recapture(&c.game, ply, &start, &cooked.keys[0], cooked.line.len()) {
        rej::hit(rej::TRIVIAL);
        return None;
    }
    if !line_is_sound(&start, &cooked.line, cooked.ends_in_mate) {
        rej::hit(rej::UNSOUND);
        return None;
    }
    let mut f = board_features(&start, &cooked.line, &cooked.keys)?;
    // How hidden the answer is: its place in the engine's static order of the start's actions,
    // and the depth from which the search keeps choosing it.
    let key0 = &cooked.keys[0];
    let key_moves0: Vec<FullMove> = key0.iter().filter_map(|k| parse_move(k)).collect();
    let order = e.ab.node_order(&start, ORDER_CAP);
    f.shallow_rank = order.iter().position(|a| key_moves0.iter().all(|m| a.contains(m))).unwrap_or(ORDER_CAP);
    let r = e.search(&start, cfg.rate_nodes, 1, &[]);
    let last_wrong = r.by_depth.iter().filter(|(_, a)| !plays_keys(a, key0)).map(|(d, _)| *d).max().unwrap_or(0);
    f.depth_to_find = last_wrong.max(0) as usize + 1;
    f.margin = cooked.root_margin;
    f.mean_replies = if cooked.defender_replies.is_empty() { 0.0 } else { cooked.defender_replies.iter().sum::<usize>() as f64 / cooked.defender_replies.len() as f64 };
    // A forced mate the line stops short of (two ways to go on): its length still counts.
    if !f.ends_in_mate && is_mate_win(cooked.final_score) {
        f.mate_in = (cooked.keys.len() as i32 - 1) + (MATE_CP - cooked.final_score);
    }
    let th = themes(&start, &cooked.line, &cooked.keys, &f, win, cooked.final_score);
    rej::hit(rej::OK);
    let id = format!("{:016x}", start.hash());
    let game_moves: String = c.game.actions[..ply].iter().map(|a| format!("{a} submit")).collect::<Vec<_>>().join(" ");
    Some(obj![
        ("id", id),
        ("rating", rating(&f)),
        ("rating_deviation", 500),
        ("side", if solver { "black" } else { "white" }),
        ("position", standalone(&start).unwrap_or_default()),
        ("game_start", c.game.start.clone()),
        ("game_moves", game_moves),
        ("solution", cooked.line.clone()),
        ("key_moves", Json::Arr(cooked.keys.iter().map(|k| Json::from(k.clone())).collect())),
        ("themes", th.into_iter().collect::<Vec<_>>()),
        ("source", c.source.name()),
        ("outcome", if win { "win" } else { "hold" }),
        ("final_eval", cooked.final_score),
        ("scan_eval", c.scan_eval),
        ("corpus", c.game.file.clone()),
        ("features", features_json(&f)),
    ])
}

fn features_json(f: &Features) -> Json {
    obj![
        ("plies", f.plies),
        ("ends_in_mate", f.ends_in_mate),
        ("mate_in", f.mate_in),
        ("calc_load", (f.calc_load * 100.0).round() / 100.0),
        ("forcing", (f.forcing * 100.0).round() / 100.0),
        ("shallow_rank", f.shallow_rank),
        ("depth_to_find", f.depth_to_find),
        ("margin", (f.margin * 1000.0).round() / 1000.0),
        ("mean_replies", (f.mean_replies * 10.0).round() / 10.0),
        ("quiet_key", f.quiet_key),
        ("sacrifice", f.sacrifice),
        ("active_timelines", f.active_timelines),
        ("timelines", f.timelines),
        ("present_boards", f.present_boards),
        ("key_count", f.key_count),
        ("key_kind", ["physical", "jump", "branch"][f.key_kind.min(2)]),
        ("travel_turns", f.travel_turns),
        ("branches_created", f.branches_created),
        ("superphysical_checks", (f.superphysical_checks * 100.0).round() / 100.0),
        ("superphysical_mate", f.superphysical_mate),
        ("relevant", f.relevant),
        ("pieces", f.pieces as usize),
        ("root_actions", f.root_actions),
        ("candidates", f.candidates),
        ("forcing_candidates", f.forcing_candidates),
        ("boards_touched", f.boards_touched),
        ("span", f.span),
        ("fork_peak", (f.fork_peak * 1000.0).round() / 1000.0),
        ("mean_choice", (f.mean_choice * 1000.0).round() / 1000.0),
    ]
}

fn features_from_json(j: &Json) -> Features {
    let i = |k: &str| j.get(k).int().unwrap_or(0);
    let fl = |k: &str| j.get(k).num().unwrap_or(0.0);
    let b = |k: &str| j.get(k).bool().unwrap_or(false);
    Features {
        plies: i("plies") as usize,
        ends_in_mate: b("ends_in_mate"),
        mate_in: i("mate_in") as i32,
        calc_load: fl("calc_load"),
        forcing: fl("forcing"),
        shallow_rank: i("shallow_rank") as usize,
        depth_to_find: i("depth_to_find") as usize,
        margin: fl("margin"),
        mean_replies: fl("mean_replies"),
        quiet_key: b("quiet_key"),
        sacrifice: i("sacrifice") as i32,
        active_timelines: i("active_timelines") as usize,
        timelines: i("timelines") as usize,
        present_boards: i("present_boards") as usize,
        key_count: i("key_count") as usize,
        key_kind: match j.get("key_kind").str() {
            Some("branch") => 2,
            Some("jump") => 1,
            _ => 0,
        },
        travel_turns: i("travel_turns") as usize,
        branches_created: i("branches_created") as usize,
        superphysical_checks: fl("superphysical_checks"),
        superphysical_mate: b("superphysical_mate"),
        relevant: i("relevant") as usize,
        pieces: i("pieces") as u32,
        root_actions: i("root_actions") as usize,
        candidates: i("candidates") as usize,
        forcing_candidates: i("forcing_candidates") as usize,
        boards_touched: i("boards_touched") as usize,
        span: i("span") as i32,
        fork_peak: fl("fork_peak"),
        mean_choice: fl("mean_choice"),
    }
}

fn progress_path(out: &Path) -> PathBuf {
    let mut p = out.as_os_str().to_owned();
    p.push(".progress");
    PathBuf::from(p)
}

/// Re-rate every stored puzzle from its features, drop duplicate positions (keeping the
/// longest line), and rewrite the file easiest first.
fn finalize(out: &Path) -> Vec<Json> {
    let Ok(text) = std::fs::read_to_string(out) else { return Vec::new() };
    let mut puzzles: Vec<Json> = text.lines().filter_map(|l| Json::parse(l).ok()).collect();
    for p in puzzles.iter_mut() {
        let r = rating(&features_from_json(p.get("features")));
        if let Json::Obj(kv) = p
            && let Some(e) = kv.iter_mut().find(|e| e.0 == "rating")
        {
            e.1 = Json::from(r);
        }
    }
    puzzles.sort_by(|a, b| {
        a.get("id").str().cmp(&b.get("id").str()).then(b.get("solution").arr().len().cmp(&a.get("solution").arr().len()))
    });
    puzzles.dedup_by(|a, b| a.get("id").str() == b.get("id").str());
    puzzles.sort_by_key(|p| p.get("rating").int().unwrap_or(0));
    let mut body = String::new();
    for p in &puzzles {
        p.write(&mut body);
        body.push('\n');
    }
    let tmp = out.with_extension("jsonl.tmp");
    if std::fs::write(&tmp, &body).is_ok() && std::fs::rename(&tmp, out).is_err() {
        let _ = std::fs::write(out, &body);
        let _ = std::fs::remove_file(&tmp);
    }
    puzzles
}

fn report(puzzles: &[Json], out: &Path) {
    let mut bands = [0usize; 6];
    let mut themes: std::collections::BTreeMap<String, usize> = Default::default();
    let mut mates = 0;
    for p in puzzles {
        let r = p.get("rating").int().unwrap_or(0);
        bands[match r {
            r if r < 1000 => 0,
            r if r < 1400 => 1,
            r if r < 1800 => 2,
            r if r < 2200 => 3,
            r if r < 2600 => 4,
            _ => 5,
        }] += 1;
        for t in p.get("themes").arr() {
            if let Some(t) = t.str() {
                *themes.entry(t.to_string()).or_default() += 1;
            }
        }
        if p.get("features").get("ends_in_mate").bool() == Some(true) {
            mates += 1;
        }
    }
    println!("\n{} puzzles in {} ({} mates)", puzzles.len(), out.display(), mates);
    println!(
        "  rating bands: <1000 {} | 1000-1399 {} | 1400-1799 {} | 1800-2199 {} | 2200-2599 {} | 2600+ {}",
        bands[0], bands[1], bands[2], bands[3], bands[4], bands[5]
    );
    let mut ts: Vec<_> = themes.into_iter().collect();
    ts.sort_by_key(|t| std::cmp::Reverse(t.1));
    println!("  themes:");
    for (t, n) in ts.iter().take(30) {
        println!("    {t:<22}{n:>6}");
    }
}

// ---------------------------------------------------------------------------
// site export: equivalent answers and the replies to them
// ---------------------------------------------------------------------------

/// Most other actions playing a step's key moves that are tried as equivalent answers.
const VARIANT_CAP: usize = 24;

/// The solver's score after playing into `child` (the defender to move): `MATE_CP` when the
/// defender is mated, `None` when stalemated.
fn after_score(e: &mut Engine, child: &State, nodes: u64) -> Option<i32> {
    match mated(child) {
        Some(true) => return Some(MATE_CP),
        Some(false) => return None,
        None => {}
    }
    e.search(child, nodes, 1, &[]).lines.first().map(|l| -l.score)
}

/// Puzzles dropped by the deep check.
static DEEP_FAILED: AtomicUsize = AtomicUsize::new(0);

/// The deep check of a solver step, `nodes` a search: the position still wins (or holds) as
/// before, and without any one key move no other idea does - the claim the mining made with
/// much smaller searches.
fn deep_step_ok(e: &mut Engine, s: &State, win: bool, keys: &[FullMove], nodes: u64) -> bool {
    let Some(best) = e.search(s, nodes, 1, &[]).lines.first().cloned() else { return false };
    if !valid(best.score, None) || win != wins(best.score) {
        return false;
    }
    keys.iter().all(|k| valid(best.score, e.search(s, nodes, 1, &[*k]).lines.first().map(|l| l.score)))
}

/// Whether an answer scoring `mine` keeps what the puzzle's own answer (`reference`) does.
fn keeps_outcome(mine: i32, reference: i32, win: bool) -> bool {
    if win {
        mine >= reference || (!is_mate_win(reference) && wins(mine))
    } else {
        mine >= HOLD_FLOOR
    }
}

fn texts(a: &kairos::core::action::MoveSeq) -> String {
    a.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(" ")
}

/// A stored puzzle as the site plays it. Besides the line, every solver step says how its
/// answers are judged and what the defender plays back:
///   - `accept`: "keys" when every action that plays all the key moves was found to keep the
///     outcome (any such action is right), "list" when some did not (only `canonical` and the
///     listed `variants` are right when `complete`, else the site's engine must judge), and
///     "mates" for a mate in one with several mating actions (any mating action).
///   - `variants`: other right actions found, each with the defender's best reply to it, and
///     whether the next step's key moves still apply after that reply (`same_next`; if not,
///     the site judges the following step with the engine).
fn site_puzzle(e: &mut Engine, p: &Json, cfg: &Cfg) -> Option<Json> {
    let mut cur = position_from_uci(p.get("position").str()?).ok()?;
    let win = p.get("outcome").str()? == "win";
    let line: Vec<String> = p.get("solution").arr().iter().filter_map(|x| x.str().map(String::from)).collect();
    let keys: Vec<Vec<String>> = p
        .get("key_moves")
        .arr()
        .iter()
        .map(|k| k.arr().iter().filter_map(|x| x.str().map(String::from)).collect())
        .collect();
    let nodes = cfg.key_nodes * 2;
    let mut steps: Vec<Json> = Vec::new();
    for (i, ks) in keys.iter().enumerate() {
        let canon = line.get(2 * i)?.clone();
        let reply = line.get(2 * i + 1).cloned();
        let child = play(&cur, &canon)?;
        let mut step = vec![("canonical", Json::from(canon.clone())), ("keys", Json::Arr(ks.iter().map(|k| Json::from(k.clone())).collect()))];
        if let Some(r) = &reply {
            step.push(("reply", Json::from(r.clone())));
        }
        if ks.is_empty() {
            step.push(("accept", Json::from("mates")));
            steps.push(Json::Obj(step.into_iter().map(|(k, v)| (k.to_string(), v)).collect()));
            break;
        }
        let kmoves: Vec<FullMove> = ks.iter().filter_map(|k| parse_move(k)).collect();
        if cfg.deep_nodes > 0 && !deep_step_ok(e, &cur, win, &kmoves, cfg.deep_nodes) {
            DEEP_FAILED.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        let reference = after_score(e, &child, nodes)?;
        let all: Vec<_> = e.ab.node_order(&cur, ROOT_COUNT_CAP).into_iter().filter(|a| kmoves.iter().all(|m| a.contains(m))).collect();
        let mut variants: Vec<Json> = Vec::new();
        let (mut tried, mut bad) = (0usize, 0usize);
        for a in all.iter().map(texts).filter(|t| *t != canon) {
            if tried == VARIANT_CAP {
                break;
            }
            tried += 1;
            let Some(vc) = play(&cur, &a) else {
                bad += 1;
                continue;
            };
            if !after_score(e, &vc, nodes).is_some_and(|m| keeps_outcome(m, reference, win)) {
                bad += 1;
                continue;
            }
            let mut v = vec![("action", Json::from(a.clone()))];
            if mated(&vc).is_none() {
                let top = e.search(&vc, cfg.defend_nodes, 1, &[]).lines.first().map(|l| l.action.clone());
                if let Some(r) = top {
                    if let (Some(next), Some(nk)) = (play(&vc, &r), keys.get(i + 1)) {
                        let same = solver_turn(e, &next, win, cfg.cook_nodes / 2, cfg.key_nodes / 2)
                            .is_some_and(|t| t.0.keys.len() == nk.len() && nk.iter().all(|k| t.0.keys.contains(k)));
                        v.push(("same_next", Json::from(same)));
                    }
                    v.push(("reply", Json::from(r)));
                }
            }
            variants.push(Json::Obj(v.into_iter().map(|(k, v)| (k.to_string(), v)).collect()));
        }
        let complete = tried == all.len().saturating_sub(1);
        step.push(("accept", Json::from(if bad == 0 { "keys" } else { "list" })));
        step.push(("complete", Json::from(complete)));
        step.push(("examined", Json::from(tried)));
        step.push(("rejected", Json::from(bad)));
        step.push(("variants", Json::Arr(variants)));
        steps.push(Json::Obj(step.into_iter().map(|(k, v)| (k.to_string(), v)).collect()));
        let Some(r) = reply else { break };
        cur = play(&child, &r)?;
    }
    Some(obj![
        ("id", p.get("id").clone()),
        ("rating", p.get("rating").clone()),
        ("rating_deviation", p.get("rating_deviation").clone()),
        ("side", p.get("side").clone()),
        ("outcome", p.get("outcome").clone()),
        ("themes", p.get("themes").clone()),
        ("position", p.get("position").clone()),
        ("mate_in", p.get("features").get("mate_in").clone()),
        ("ends_in_mate", p.get("features").get("ends_in_mate").clone()),
        ("solution", p.get("solution").clone()),
        ("verified_nodes", cfg.deep_nodes as usize),
        ("steps", Json::Arr(steps)),
    ])
}

/// Write `site` (one puzzle per line) from the stored puzzles.
fn export_site(cfg: &Cfg, site: &Path) {
    let text = std::fs::read_to_string(&cfg.out).unwrap_or_default();
    let mut puzzles: Vec<Json> = text.lines().filter_map(|l| Json::parse(l).ok()).collect();
    if let Some((i, n)) = cfg.shard {
        let mut k = 0;
        puzzles.retain(|_| {
            k += 1;
            (k - 1) % n == i - 1
        });
    }
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, Json)>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..cfg.threads {
            sc.spawn(|| {
                let mut e = Engine::new();
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(p) = puzzles.get(k) else { break };
                    if let Some(j) = site_puzzle(&mut e, p, cfg) {
                        done.lock().unwrap().push((k, j));
                    }
                    if (k + 1).is_multiple_of(25) {
                        println!("  {} of {} exported", k + 1, puzzles.len());
                    }
                }
            });
        }
    });
    let mut done = done.into_inner().unwrap();
    done.sort_by_key(|d| d.0);
    let mut body = String::new();
    let (mut keys_ok, mut partial) = (0usize, 0usize);
    for (_, j) in &done {
        let all_keys = j.get("steps").arr().iter().all(|s| s.get("accept").str() != Some("list"));
        if all_keys {
            keys_ok += 1;
        } else {
            partial += 1;
        }
        j.write(&mut body);
        body.push('\n');
    }
    let _ = std::fs::write(site, body);
    println!("{} of {} puzzles exported to {} ({} judged by key moves alone, {} with a list of answers; {} failed the deep check)", done.len(), puzzles.len(), site.display(), keys_ok, partial, DEEP_FAILED.load(Ordering::Relaxed));
}

fn main() {
    let cfg = parse_args();
    if let Some(dir) = cfg.out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if cfg.rate_only {
        let p = finalize(&cfg.out);
        report(&p, &cfg.out);
        return;
    }
    if let Some(site) = &cfg.export {
        export_site(&cfg, site);
        return;
    }
    let mut cands = scan(&cfg);
    // One candidate per position.
    let mut seen: HashSet<u64> = HashSet::new();
    let done: HashSet<u64> = std::fs::read_to_string(progress_path(&cfg.out))
        .map(|t| t.lines().filter_map(|l| u64::from_str_radix(l.trim(), 16).ok()).collect())
        .unwrap_or_default();
    let mut keyed: Vec<(u64, Cand)> = Vec::new();
    for c in cands.drain(..) {
        let Some(s) = state_at(&c.game, c.ply) else { continue };
        let h = s.hash();
        if seen.insert(h) && !done.contains(&h) {
            keyed.push((h, c));
        }
    }
    // Mate shots first (they survive most often), then turning points, then missed saves.
    keyed.sort_by_key(|(_, c)| match c.source {
        Source::MateShot => 0,
        Source::TurningPoint => 1,
        Source::MissedSave => 2,
    });
    println!("stage 1: {} unique positions to search ({} already done)", keyed.len(), done.len());
    if cfg.dry_run {
        let mut by: std::collections::BTreeMap<&str, usize> = Default::default();
        for (_, c) in &keyed {
            *by.entry(c.source.name()).or_default() += 1;
        }
        println!("  {by:?}");
        return;
    }
    let (tx, rx) = mpsc::channel::<(u64, Option<Json>)>();
    let out_path = cfg.out.clone();
    let writer = std::thread::spawn(move || {
        let mut out = std::fs::OpenOptions::new().create(true).append(true).open(&out_path).expect("output file");
        let mut prog = std::fs::OpenOptions::new().create(true).append(true).open(progress_path(&out_path)).expect("progress file");
        let (mut seen, mut found) = (0usize, 0usize);
        let t0 = Instant::now();
        for (h, p) in rx {
            seen += 1;
            if let Some(p) = p {
                let mut line = String::new();
                p.write(&mut line);
                let _ = writeln!(out, "{line}");
                let _ = out.flush();
                found += 1;
            }
            let _ = writeln!(prog, "{h:016x}");
            let _ = prog.flush();
            if seen % 50 == 0 {
                println!("  {seen} searched, {found} puzzles ({:.0}s)", t0.elapsed().as_secs_f64());
            }
        }
    });
    let next = AtomicUsize::new(0);
    let keyed = &keyed;
    std::thread::scope(|sc| {
        for _ in 0..cfg.threads {
            let tx = tx.clone();
            let cfg = &cfg;
            let next = &next;
            sc.spawn(move || {
                let mut e = Engine::new();
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some((h, c)) = keyed.get(k) else { break };
                    let p = solve(&mut e, c, cfg);
                    let i = c.source as usize;
                    rej::SRC_SEEN[i].fetch_add(1, Ordering::Relaxed);
                    if p.is_some() {
                        rej::SRC_OK[i].fetch_add(1, Ordering::Relaxed);
                    }
                    let _ = tx.send((*h, p));
                }
            });
        }
    });
    drop(tx);
    let _ = writer.join();
    rej::report();
    let p = finalize(&cfg.out);
    report(&p, &cfg.out);
}
