//! JSON command interface over a [`Game`] and the engine, as used by the browser (through the
//! WASM build). One request in, one response out: `{"cmd": ..., ...}` → `{"ok": true, ...}` or
//! `{"ok": false, "error": ...}`.

use super::json::Json;
use super::record::{self, VARIANTS};
use super::tree::{parse_lan_moves, parse_move_json, v4, Game, ROOT};
use crate::core::action::{Action, ExtMove};
use crate::core::variants::position_from_uci;
use crate::engine::ab::AlphaBeta;
use crate::engine::Limits;
use crate::obj;
use std::sync::atomic::AtomicBool;

#[derive(Default)]
pub struct Api {
    game: Option<Game>,
    engine: Option<AlphaBeta>,
    hash_mb: Option<usize>,
    /// Engine options (`Threads`, ...) applied to the engine now and when it is created.
    options: Vec<(String, i64)>,
}

fn err(e: impl Into<String>) -> Json {
    obj![("ok", false), ("error", e.into())]
}

fn ok(mut fields: Json) -> Json {
    if let Json::Obj(kv) = &mut fields {
        kv.insert(0, ("ok".into(), true.into()));
    }
    fields
}

fn moves_of(j: &Json) -> Result<Vec<ExtMove>, String> {
    j.arr().iter().map(|m| parse_move_json(m).ok_or_else(|| "bad move object".to_string())).collect()
}

/// A start position from a variant name, `startpos` or a 5DUCI position argument.
pub fn resolve_position(p: &str) -> Result<String, String> {
    let p = p.trim();
    if let Some(v) = record::as_position(p) {
        return Ok(v);
    }
    position_from_uci(p)?;
    Ok(p.to_string())
}

impl Api {
    pub fn new() -> Api {
        Api::default()
    }

    fn game(&mut self) -> Result<&mut Game, String> {
        self.game.as_mut().ok_or_else(|| "no game loaded".to_string())
    }

    /// Handle one request; `info` receives engine progress lines during `search`.
    pub fn call(&mut self, req: &str, info: &mut dyn FnMut(&str)) -> String {
        let r = match Json::parse(req) {
            Ok(j) => self.dispatch(&j, info).unwrap_or_else(err),
            Err(e) => err(e),
        };
        r.to_string()
    }

    fn node_arg(&self, j: &Json) -> usize {
        j.get("node").int().map(|n| n as usize).unwrap_or(ROOT)
    }

    fn dispatch(&mut self, j: &Json, info: &mut dyn FnMut(&str)) -> Result<Json, String> {
        let cmd = j.get("cmd").str().unwrap_or("");
        match cmd {
            "variants" => {
                let list: Vec<Json> = VARIANTS
                    .iter()
                    .map(|v| obj![("name", *v), ("position", record::variant_position(v).unwrap_or_default())])
                    .collect();
                Ok(ok(obj![("variants", Json::Arr(list))]))
            }
            // The variant a record's start position is (the game browser's variant column).
            "variant" => {
                let pos = j.get("position").str().unwrap_or("startpos");
                let name = match position_from_uci(pos) {
                    Ok(s) => record::variant_of(&s).unwrap_or("Custom"),
                    Err(_) => "Invalid",
                };
                Ok(ok(obj![("variant", name)]))
            }
            "load" => {
                let text = j.get("text").str().ok_or("missing text")?;
                let l = record::load(text, j.get("position").str().filter(|s| !s.trim().is_empty()))?;
                let tree = l.game.tree_json();
                let mainline = l.game.mainline();
                self.game = Some(l.game);
                Ok(ok(obj![("meta", l.meta), ("tree", tree), ("mainline", mainline)]))
            }
            "new" => {
                let pos = resolve_position(j.get("position").str().unwrap_or("Standard - Turn Zero"))?;
                let g = Game::new(&pos)?;
                let tree = g.tree_json();
                let variant = record::variant_of(&g.nodes[ROOT].state);
                self.game = Some(g);
                Ok(ok(obj![("tree", tree), ("meta", obj![("format", "new"), ("variant", variant.unwrap_or("Custom"))])]))
            }
            // Check a board-editor position without touching the loaded game: whether it
            // loads, its view, and whether the side to move can capture a royal at once.
            "inspect" => {
                let text = j.get("text").str().ok_or("missing text")?;
                let mut l = match record::load(text, None) {
                    Ok(l) => l,
                    Err(e) => return Ok(ok(obj![("valid", false), ("error", e)])),
                };
                let end = *l.game.mainline().last().unwrap_or(&ROOT);
                let view = l.game.view(end, &[])?;
                let s = &l.game.nodes[end].state;
                let player = s.get_present().1;
                let capture = s.first_check(player).map(|fm| obj![("from", v4(fm.from)), ("to", v4(fm.to)), ("c", player as i32)]);
                Ok(ok(obj![("valid", true), ("view", view), ("meta", l.meta), ("capture", capture)]))
            }
            "tree" => Ok(ok(obj![("tree", self.game()?.tree_json())])),
            "view" => {
                let id = self.node_arg(j);
                let draft = moves_of(j.get("draft"))?;
                Ok(ok(obj![("view", self.game()?.view(id, &draft)?)]))
            }
            "targets" => {
                let id = self.node_arg(j);
                let draft = moves_of(j.get("draft"))?;
                let from = parse_move_json(&obj![("from", j.get("from").clone()), ("to", j.get("from").clone())])
                    .ok_or("bad from")?
                    .fm
                    .from;
                let color = j.get("color").int().map(|c| c != 0);
                let t = self.game()?.piece_targets(id, &draft, from, color)?;
                Ok(ok(obj![("targets", Json::Arr(t.into_iter().map(v4).collect()))]))
            }
            "check_move" => {
                // Validate one more move of a draft; returns the draft position's view.
                let id = self.node_arg(j);
                let mut draft = moves_of(j.get("draft"))?;
                draft.push(parse_move_json(j.get("move")).ok_or("bad move")?);
                let g = self.game()?;
                g.draft_state(id, &draft)?;
                Ok(ok(obj![("view", g.view(id, &draft)?)]))
            }
            "play" => {
                let id = self.node_arg(j);
                let moves = if let Some(lan) = j.get("lan").str() { parse_lan_moves(lan)? } else { moves_of(j.get("moves"))? };
                let g = self.game()?;
                let n = g.add_child(id, &moves)?;
                Ok(ok(obj![("node", n), ("tree", g.tree_json())]))
            }
            "play_line" => {
                // A sequence of actions (`lan ... submit lan ... submit`), e.g. an engine PV.
                let mut id = self.node_arg(j);
                let line = j.get("line").str().ok_or("missing line")?;
                let count = j.get("count").int().map(|c| c as usize).unwrap_or(usize::MAX);
                let g = self.game()?;
                for (k, act) in line.split("submit").filter(|a| !a.trim().is_empty()).enumerate() {
                    if k >= count {
                        break;
                    }
                    id = g.add_child(id, &parse_lan_moves(act)?)?;
                }
                Ok(ok(obj![("node", id), ("tree", g.tree_json())]))
            }
            "line_san" => {
                // 5dpgn text of each action of a line played from node.
                let id = self.node_arg(j);
                let line = j.get("line").str().ok_or("missing line")?;
                let g = self.game()?;
                let mut s = g.node(id)?.state.clone();
                let mut out = Vec::new();
                for act in line.split("submit").filter(|a| !a.trim().is_empty()) {
                    let Ok(moves) = parse_lan_moves(act) else { break };
                    let a = Action::from_vector(&moves, &s);
                    let Ok(next) = super::tree::play_action(&s, a.moves()) else { break };
                    let (san, full) = super::tree::action_san_pair(&s, &a);
                    out.push(obj![("san", san), ("sanFull", full), ("lone", super::tree::lone_board(&s)), ("lan", act.trim()), ("player", s.get_present().1)]);
                    s = next;
                }
                Ok(ok(obj![("actions", Json::Arr(out))]))
            }
            "delete" => {
                let id = self.node_arg(j);
                let g = self.game()?;
                let p = g.delete(id)?;
                Ok(ok(obj![("node", p), ("tree", g.tree_json())]))
            }
            "promote" => {
                let id = self.node_arg(j);
                let g = self.game()?;
                // `fork`: only among its siblings ("Promote variation"); else to the main line.
                if j.get("fork").bool() == Some(true) {
                    g.promote_at_fork(id);
                } else {
                    g.promote(id);
                }
                Ok(ok(obj![("tree", g.tree_json())]))
            }
            "comment" => {
                let id = self.node_arg(j);
                let text = j.get("text").str().unwrap_or("").trim().to_string();
                let g = self.game()?;
                g.node(id)?;
                g.nodes[id].comments = if text.is_empty() { Vec::new() } else { vec![text] };
                Ok(ok(obj![("tree", g.tree_json())]))
            }
            "export" => {
                // The path to `node` (default: the main line) as 5dpgn and as a 5DUCI line.
                let g = self.game()?;
                let path = match j.get("node").int() {
                    Some(n) => {
                        let mut p = g.path(n as usize);
                        let mut c = n as usize;
                        while let Some(&f) = g.nodes[c].children.first() {
                            p.push(f);
                            c = f;
                        }
                        p
                    }
                    None => g.mainline(),
                };
                let mut actions = Vec::new();
                for w in path.windows(2) {
                    actions.push(Action::from_vector(&g.nodes[w[1]].moves, &g.nodes[w[0]].state));
                }
                let setup = record::setup_of_state(&g.nodes[ROOT].state);
                let headers: Vec<(String, String)> = j
                    .get("headers")
                    .arr()
                    .iter()
                    .filter_map(|h| Some((h.arr().first()?.str()?.to_string(), h.arr().get(1)?.str()?.to_string())))
                    .collect();
                let mate = super::tree::searchable(&g.nodes[*path.last().unwrap()].state);
                let opts = super::pgn::PgnOptions { short: false, mate };
                let pgn = super::pgn::write_5dpgn_opts(&headers, &setup, &actions, opts);
                let last = *path.last().unwrap();
                Ok(ok(obj![("pgn", pgn), ("uci", g.position_command(last))]))
            }
            "search" => self.search(j, info),
            "book" => {
                // The opening book's actions for a node of the game (or a 5DUCI `position`):
                // [{lan, pgn, play, weight}], `play` ones first by weight, then the known ones.
                let book = crate::engine::book::Book::theory();
                let (s, opening) = match j.get("position").str() {
                    Some(p) => {
                        let s = Self::uci_state(p)?;
                        let opening = book.opening_name(&s).map(|name| obj![("name", name)]).unwrap_or(Json::Null);
                        (s, opening)
                    }
                    None => {
                        let id = self.node_arg(j);
                        let game = self.game()?;
                        let s = game.node(id)?.state.clone();
                        let mut opening = Json::Null;
                        // The current branch's last recognized opening survives leaving theory.
                        let mut at = Some(id);
                        while let Some(node_id) = at {
                            let node = game.node(node_id)?;
                            if let Some(name) = book.opening_name(&node.state) {
                                opening = obj![("name", name), ("node", node_id), ("ply", node.ply)];
                                break;
                            }
                            at = node.parent;
                        }
                        (s, opening)
                    }
                };
                let trans = book.transpositions(&s);
                let mut moves: Vec<&crate::engine::book::BookMove> =
                    book.probe(&s).map(|v| v.iter().collect()).unwrap_or_default();
                moves.extend(trans.iter());
                moves.sort_by_key(|m| (!m.play, std::cmp::Reverse(m.weight)));
                let list: Vec<Json> =
                    moves.iter().map(|m| obj![("lan", m.lan.as_str()), ("pgn", m.pgn.as_str()), ("play", m.play), ("weight", m.weight as i64)]).collect();
                Ok(ok(obj![("moves", Json::Arr(list)), ("opening", opening)]))
            }
            "set_hash" => {
                let mb = j.get("mb").int().unwrap_or(32).max(1) as usize;
                self.hash_mb = Some(mb);
                self.engine = None;
                Ok(ok(obj![]))
            }
            "set_option" => {
                let name = j.get("name").str().ok_or("missing name")?.to_string();
                let value = j.get("value").int().ok_or("missing value")?;
                if let Some(e) = self.engine.as_mut() {
                    e.set_option(&name, &crate::engine::OptionValue::Int(value));
                }
                self.options.retain(|(n, _)| *n != name);
                self.options.push((name, value));
                Ok(ok(obj![]))
            }
            _ => Err(format!("unknown command {:?}", cmd)),
        }
    }

    /// The position of a 5DUCI `position` argument (with optional `moves`).
    fn uci_state(pos: &str) -> Result<crate::core::state::State, String> {
        let (p, moves) = match pos.find(" moves") {
            Some(i) => (&pos[..i], &pos[i + 6..]),
            None => (pos, ""),
        };
        let mut s = if p.trim() == "startpos" { crate::core::variants::startpos() } else { position_from_uci(p)? };
        crate::core::variants::apply_uci_moves(&mut s, moves)?;
        Ok(s)
    }

    /// Run the engine on a 5DUCI position, answering from the opening book first if `book`.
    /// Takes search limits, clock fields (`timeLeft`, `increment`, `delay`, `delayPerTimeline`)
    /// and `multiPV` (see README).
    fn search(&mut self, j: &Json, info: &mut dyn FnMut(&str)) -> Result<Json, String> {
        let s = Self::uci_state(j.get("position").str().ok_or("missing position")?)?;
        let limits = Limits {
            depth: j.get("depth").int().map(|d| d as u32),
            movetime: j.get("movetime").int().map(|d| d as u64),
            nodes: j.get("nodes").int().map(|d| d as u64),
            time_left: j.get("timeLeft").int().map(|d| d as u64),
            increment: j.get("increment").int().map(|d| d as u64),
            delay: j.get("delay").int().map(|d| d as u64),
            delay_per_timeline: j.get("delayPerTimeline").int().map(|d| d as u64),
            resume: j.get("resume").bool() == Some(true),
        };
        let hash = self.hash_mb;
        let options = &self.options;
        let eng = self.engine.get_or_insert_with(|| {
            let mut e = AlphaBeta::new();
            if let Some(mb) = hash {
                e.set_option("Hash", &crate::engine::OptionValue::Int(mb as i64));
            }
            for (name, value) in options {
                e.set_option(name, &crate::engine::OptionValue::Int(*value));
            }
            e
        });
        if j.get("newGame").bool() == Some(true) {
            eng.new_game();
        }
        eng.set_option("OwnBook", &crate::engine::OptionValue::Bool(j.get("book").bool() == Some(true)));
        // `multiPV`: this search's, else the `set_option` value, else 1.
        let multipv = j.get("multiPV").int().or_else(|| options.iter().find(|(n, _)| n == "MultiPV").map(|o| o.1)).unwrap_or(1);
        eng.set_option("MultiPV", &crate::engine::OptionValue::Int(multipv));
        let skill = j.get("skillLevel").int().or_else(|| options.iter().find(|(n, _)| n == "Skill Level").map(|o| o.1)).unwrap_or(8);
        eng.set_option("Skill Level", &crate::engine::OptionValue::Int(skill));
        let stop = AtomicBool::new(false);
        let best = eng.find_best_move(&s, &limits, &stop, &mut |m| info(&m));
        let bestmove = best.map(|a| super::tree::action_lan(&s, &a));
        Ok(ok(obj![("bestmove", bestmove)]))
    }
}
