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
                let mut limits = Limits::default();
                let toks: Vec<&str> = it.collect();
                let mut k = 0;
                let player_black = s.get_present().1;
                while k < toks.len() {
                    let v = toks.get(k + 1).and_then(|x| x.parse::<u64>().ok());
                    match toks[k] {
                        "movetime" => limits.movetime = v,
                        "depth" => limits.depth = v.map(|d| d as u32),
                        "nodes" => limits.nodes = v,
                        "wtime" if !player_black => limits.time_left = v,
                        "btime" if player_black => limits.time_left = v,
                        "winc" if !player_black => limits.increment = v,
                        "binc" if player_black => limits.increment = v,
                        _ => {
                            k += 1;
                            continue;
                        }
                    }
                    k += 2;
                }
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
