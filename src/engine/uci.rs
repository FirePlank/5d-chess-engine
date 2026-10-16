//! 5DUCI protocol loop (engine side).

use super::ab::AlphaBeta;
use super::{Limits, OptionValue};
use crate::core::state::State;
use crate::core::variants::{apply_uci_moves, position_from_uci, startpos};
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

fn out(line: &str) {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{}", line);
    let _ = lock.flush();
}

/// The limits of a `go` command (its arguments in `toks`) for the side to move (`black`):
/// `movetime`, `depth`, `nodes`; a clock: `wtime/btime`, `winc/binc`, and a delay clock's
/// `wdelay/bdelay` (fixed per action) and `wtldelay/btldelay` (per active timeline), all ms.
/// The 5D tournament clock (5 min, 3 s delay per active timeline) is
/// `go wtime 300000 btime 300000 wtldelay 3000 btldelay 3000`. Unknown tokens are skipped.
pub fn parse_go(toks: &[&str], black: bool) -> Limits {
    let mut limits = Limits::default();
    let mut k = 0;
    while k < toks.len() {
        let v = toks.get(k + 1).and_then(|x| x.parse::<u64>().ok());
        match toks[k] {
            "movetime" => limits.movetime = v,
            "depth" => limits.depth = v.map(|d| d as u32),
            "nodes" => limits.nodes = v,
            "wtime" if !black => limits.time_left = v,
            "btime" if black => limits.time_left = v,
            "winc" if !black => limits.increment = v,
            "binc" if black => limits.increment = v,
            "wdelay" if !black => limits.delay = v,
            "bdelay" if black => limits.delay = v,
            "wtldelay" if !black => limits.delay_per_timeline = v,
            "btldelay" if black => limits.delay_per_timeline = v,
            "wtime" | "btime" | "winc" | "binc" | "wdelay" | "bdelay" | "wtldelay" | "btldelay" => {}
            _ => {
                k += 1;
                continue;
            }
        }
        k += 2;
    }
    limits
}

pub fn run(engine: AlphaBeta) {
    let engine = Arc::new(Mutex::new(engine));
    let stop = Arc::new(AtomicBool::new(false));
    let busy = Arc::new(AtomicBool::new(false));
    let debug = Arc::new(AtomicBool::new(false));
    let mut state: Option<State> = None;
    let mut worker: Option<JoinHandle<()>> = None;
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        if let Some(w) = worker.take() {
            if busy.load(Ordering::SeqCst) {
                worker = Some(w);
            } else {
                let _ = w.join();
            }
        }
        let mut it = line.split_whitespace();
        let cmd = it.next().unwrap_or("");
        match cmd {
            "5duci" => {
                out("id name kairos");
                out("id author kairos");
                out("option name Hash type spin default 64 min 1 max 65536");
                out(&format!("option name Threads type spin default 1 min 1 max {}", super::ab::MAX_THREADS));
                out("option name OwnBook type check default false");
                out("5duciok");
            }
            "isready" => out("readyok"),
            "5ducinewgame" => {
                if busy.load(Ordering::SeqCst) {
                    out("info engine is busy, please run 'stop' first");
                } else {
                    state = None;
                    engine.lock().unwrap().new_game();
                }
            }
            "position" => {
                if busy.load(Ordering::SeqCst) {
                    out("info engine is busy, please run 'stop' first");
                    continue;
                }
                let rest = line["position".len()..].trim();
                let (pos, moves) = match rest.find(" moves") {
                    Some(i) => (&rest[..i], &rest[i + 6..]),
                    None => match rest.find(" move ") {
                        Some(i) => (&rest[..i], &rest[i + 5..]),
                        None => (rest, ""),
                    },
                };
                let base = if pos.trim() == "startpos" { Ok(startpos()) } else { position_from_uci(pos) };
                match base {
                    Ok(mut s) => match apply_uci_moves(&mut s, moves) {
                        Ok(()) => state = Some(s),
                        Err(e) => out(&format!("info {}", e)),
                    },
                    Err(e) => out(&format!("info position error: {}", e)),
                }
            }
            "go" => {
                if busy.load(Ordering::SeqCst) {
                    out("info search already running");
                    continue;
                }
                let Some(s) = state.clone() else {
                    out("nobestmove");
                    continue;
                };
                let toks: Vec<&str> = it.collect();
                let limits = parse_go(&toks, s.get_present().1);
                stop.store(false, Ordering::SeqCst);
                busy.store(true, Ordering::SeqCst);
                let (engine, stop, busy) = (engine.clone(), stop.clone(), busy.clone());
                worker = Some(std::thread::Builder::new().stack_size(64 << 20).spawn(move || {
                    let mut eng = engine.lock().unwrap();
                    let best = eng.find_best_move(&s, &limits, &stop, &mut |msg| out(&format!("info {}", msg)));
                    let reply = match best {
                        Some(act) => {
                            let mut text = String::from("bestmove");
                            let mut os = s.clone();
                            for em in act.moves() {
                                text.push(' ');
                                text += &em.lan(&os);
                                if let Some(n) = os.normalize_promotion(*em) {
                                    os.apply_ext::<false>(n);
                                }
                            }
                            text
                        }
                        None => "nobestmove".to_string(),
                    };
                    busy.store(false, Ordering::SeqCst);
                    out(&reply);
                }).expect("spawn search thread"));
            }
            "stop" => stop.store(true, Ordering::SeqCst),
            "setoption" => {
                let rest = line["setoption".len()..].trim();
                let Some(rest) = rest.strip_prefix("name ") else { continue };
                let (key, value) = match rest.find(" value ") {
                    Some(i) => (rest[..i].trim(), OptionValue::parse(rest[i + 7..].trim())),
                    None => (rest.trim(), OptionValue::None),
                };
                if key == "debug" {
                    debug.store(!matches!(value, OptionValue::Bool(false)), Ordering::SeqCst);
                    continue;
                }
                let ok = engine.lock().unwrap().set_option(key, &value);
                if !ok && debug.load(Ordering::SeqCst) {
                    out(&format!("info [debug] unknown option {}", key));
                }
            }
            "print" => {
                if let Some(s) = &state {
                    out(&s.show_fen());
                }
            }
            "quit" => {
                stop.store(true, Ordering::SeqCst);
                if let Some(w) = worker.take() {
                    let _ = w.join();
                }
                out("bye");
                return;
            }
            _ => {
                if debug.load(Ordering::SeqCst) {
                    out(&format!("info [debug] unknown command: {}", line));
                }
            }
        }
    }
    stop.store(true, Ordering::SeqCst);
    if let Some(w) = worker.take() {
        let _ = w.join();
    }
}

#[cfg(test)]
mod tests {
    use super::parse_go;

    #[test]
    fn go_delay_params() {
        let toks: Vec<&str> = "wtime 300000 btime 290000 winc 0 binc 0 wdelay 500 bdelay 700 wtldelay 3000 btldelay 2000".split(' ').collect();
        let w = parse_go(&toks, false);
        assert_eq!((w.time_left, w.increment, w.delay, w.delay_per_timeline), (Some(300000), Some(0), Some(500), Some(3000)));
        let b = parse_go(&toks, true);
        assert_eq!((b.time_left, b.increment, b.delay, b.delay_per_timeline), (Some(290000), Some(0), Some(700), Some(2000)));
        let m = parse_go(&["movetime", "100", "foo", "nodes", "5"], false);
        assert_eq!((m.movetime, m.nodes, m.delay), (Some(100), Some(5), None));
    }
}
