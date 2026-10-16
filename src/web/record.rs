//! Game records: SPRT `--games-out` JSON lines, 5DUCI move lists and 5dpgn, loaded into a
//! [`Game`] tree plus metadata.

use super::json::Json;
use super::pgn;
use super::tree::{parse_lan_moves, Game, ROOT};
use crate::core::multiverse::TimelineLayout;
use crate::core::state::State;
use crate::core::variants::{default_variant, position_from_uci, state_from_setup, Sign, VariantSetup};
use crate::obj;

/// Built-in variants, in menu order.
pub const VARIANTS: [&str; 7] = [
    "Standard",
    "Standard - Turn Zero",
    "Standard - Reversed Royalty",
    "Standard - Half Reflected",
    "Standard - Defended Pawn",
    "Standard - Two Timelines",
    "Very Small - Open",
];

/// The 5DUCI `position` argument for a setup.
pub fn setup_to_uci(v: &VariantSetup) -> String {
    let mut s = format!("size {}x{} {} fen", v.size_x, v.size_y, if v.even { "even" } else { "odd" });
    let layout = if v.even { TimelineLayout::even() } else { TimelineLayout::odd() };
    for b in &v.boards {
        // Internal negative lines are written mirrored with a '-' sign.
        let (sign, l) = match b.sign {
            Sign::Nil if b.l < 0 => ("-", layout.mirror(b.l)),
            Sign::Nil => ("", b.l),
            Sign::Positive => ("+", b.l),
            Sign::Negative => ("-", b.l),
        };
        s += &format!(" [{}:{}{}:{}:{}]", b.fen, sign, l, b.t, if b.c { "b" } else { "w" });
    }
    s
}

/// 5DUCI `position` argument of a built-in variant.
pub fn variant_position(name: &str) -> Option<String> {
    default_variant(name).map(|v| setup_to_uci(&v))
}

fn same_position(a: &State, b: &State) -> bool {
    a.get_board_size() == b.get_board_size() && a.hash() == b.hash() && a.multiverse().layout() == b.multiverse().layout()
}

/// The built-in variant whose start position is `s`, if any.
pub fn variant_of(s: &State) -> Option<&'static str> {
    VARIANTS.iter().copied().find(|name| {
        default_variant(name).and_then(|v| state_from_setup(&v).ok()).is_some_and(|v| same_position(&v, s))
    })
}

pub struct Loaded {
    pub game: Game,
    pub meta: Json,
}

/// Width of one FEN row (squares), or None if it is not a FEN row.
fn fen_row_width(row: &str) -> Option<i32> {
    let mut w = 0;
    for ch in row.chars() {
        match ch {
            '1'..='8' => w += ch as i32 - '0' as i32,
            '*' => {}
            c if c.is_ascii_alphabetic() => w += 1,
            _ => return None,
        }
    }
    Some(w)
}

/// A start position written in a shorthand the 5DUCI `position` command does not take, as a
/// 5DUCI position argument:
/// - a built-in variant name (`Standard - Two Timelines`);
/// - a bare board FEN, optionally followed by `w`/`b` (`k7/p*n6/K7/8/8/8/6P*B/8`): one board
///   on timeline 0 at turn 1 for the side to move, sized from the FEN;
/// - board tags without a size prefix (`[fen:0:1:w] [fen:-1:1:w]`), as an 8x8 odd layout.
pub fn as_position(text: &str) -> Option<String> {
    let t = text.trim();
    if let Some(v) = variant_position(t) {
        return Some(v);
    }
    if t.starts_with('[') {
        let tags: Vec<&str> = t.split(']').map(str::trim).filter(|s| !s.is_empty()).collect();
        let boards = tags.iter().all(|s| s.starts_with('[') && s.contains(':') && !s.contains('"'));
        return (boards && position_from_uci(&format!("fen {}", t)).is_ok()).then(|| format!("size 8x8 odd fen {}", t));
    }
    let mut it = t.split_whitespace();
    let fen = it.next()?;
    let c = match it.next() {
        None | Some("w" | "W") => "w",
        Some("b" | "B") => "b",
        _ => return None,
    };
    if it.next().is_some() || !fen.contains('/') {
        return None;
    }
    let rows: Vec<&str> = fen.split('/').collect();
    let w = fen_row_width(rows[0])?;
    if !(1..=8).contains(&w) || rows.len() > 8 || rows.iter().any(|r| fen_row_width(r) != Some(w)) {
        return None;
    }
    let p = format!("size {}x{} odd fen [{}:0:1:{}]", w, rows.len(), fen, c);
    position_from_uci(&p).is_ok().then_some(p)
}

/// What kind of record `text` holds.
fn detect(text: &str) -> &'static str {
    let t = text.trim_start();
    if t.starts_with('{') {
        "json"
    } else if t.starts_with("position ")
        || t.starts_with("G ")
        || t.starts_with("startpos")
        || t.starts_with("size ")
        || t.starts_with("fen ")
        || t.split_whitespace().any(|w| w == "submit")
    {
        "uci"
    } else {
        "pgn"
    }
}

/// Load one game. `default_position` (5DUCI) is used by formats that do not name their start
/// position (bare move lists, 5dpgn without headers); otherwise Standard.
pub fn load(text: &str, default_position: Option<&str>) -> Result<Loaded, String> {
    if let Some(p) = as_position(text) {
        return load_uci(&p, None);
    }
    let normalized = default_position.map(|p| as_position(p).unwrap_or_else(|| p.trim().to_string()));
    let default_position = normalized.as_deref();
    match detect(text) {
        "json" => load_json(&Json::parse(text.trim())?),
        "uci" => load_uci(text, default_position),
        _ => load_pgn(text, default_position),
    }
}

/// Replay `actions` (LAN tokens per action) from the root as the main line.
fn replay(game: &mut Game, actions: &[Vec<crate::core::action::ExtMove>]) -> Result<Vec<usize>, String> {
    let mut ids = Vec::with_capacity(actions.len());
    let mut cur = ROOT;
    for (i, a) in actions.iter().enumerate() {
        cur = game.add_child(cur, a).map_err(|e| format!("action {} ({}): {}", i + 1, a.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(" "), e))?;
        ids.push(cur);
    }
    Ok(ids)
}

/// Split a `moves` string into actions at `submit` tokens.
fn split_actions(moves: &str) -> Result<Vec<Vec<crate::core::action::ExtMove>>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for tok in moves.split_whitespace() {
        if tok == "submit" {
            out.push(parse_lan_moves(&cur)?);
            cur.clear();
        } else {
            cur.push(' ');
            cur += tok;
        }
    }
    if !cur.trim().is_empty() {
        out.push(parse_lan_moves(&cur)?);
    }
    Ok(out)
}

fn result_text(white_score: f64) -> &'static str {
    if white_score > 0.75 {
        "1-0"
    } else if white_score < 0.25 {
        "0-1"
    } else {
        "1/2-1/2"
    }
}

pub fn load_json(j: &Json) -> Result<Loaded, String> {
    let position = j.get("position").str().unwrap_or("startpos");
    let mut game = Game::new(position)?;
    let moves = j.get("moves").str().ok_or("record has no \"moves\" string")?;
    let actions = split_actions(moves)?;
    let ids = replay(&mut game, &actions)?;
    let evals = j.get("evals").arr();
    let book = match j.get("book").int() {
        Some(b) => b.max(0) as usize,
        None if !evals.is_empty() => ids.len().saturating_sub(evals.len()),
        None => 0,
    };
    // Evals are kept on the engine's reported scale (+100 = about a 75% expected score);
    // records without "eval_scale" are older and hold internal units. Mates are +-20000.
    let scaled = j.get("eval_scale").int().is_some();
    let report = |e: i32| if scaled || e.abs() >= 20000 { e } else { crate::engine::report_cp(e) };
    for (i, &id) in ids.iter().enumerate() {
        let n = &mut game.nodes[id];
        n.book = i < book;
        if i >= book {
            n.eval = evals.get(i - book).and_then(|e| e.int()).map(|e| report(e as i32));
        }
    }
    let new_first = j.get("new_first").bool();
    let score = j.get("score").num();
    let white_score = match (score, new_first) {
        (Some(s), Some(nf)) => Some(if nf { s } else { 1.0 - s }),
        (Some(s), None) => Some(s),
        _ => None,
    };
    let (white, black) = match new_first {
        Some(true) => ("New", "Base"),
        Some(false) => ("Base", "New"),
        None => ("White", "Black"),
    };
    let white = j.get("white").str().unwrap_or(white).to_string();
    let black = j.get("black").str().unwrap_or(black).to_string();
    let variant = variant_of(&game.nodes[ROOT].state);
    let meta = obj![
        ("format", "sprt"),
        ("variant", variant.unwrap_or("Custom")),
        ("white", white),
        ("black", black),
        ("result", white_score.map(result_text)),
        ("reason", j.get("reason").str()),
        ("game", j.get("game").int()),
        ("pair", j.get("pair").int()),
        ("newFirst", new_first),
        ("book", book),
        ("actions", ids.len()),
        ("hasEvals", !evals.is_empty())
    ];
    Ok(Loaded { game, meta })
}

/// `position <pos> moves ...`, `G <n> <pos> moves ...`, `<pos> moves ...` or just moves.
pub fn load_uci(text: &str, default_position: Option<&str>) -> Result<Loaded, String> {
    let mut t = text.trim();
    t = t.strip_prefix("position ").unwrap_or(t).trim();
    if let Some(rest) = t.strip_prefix("G ") {
        // ktools position files: `G <id> <position> [moves ...]`.
        t = rest.trim_start().split_once(char::is_whitespace).map(|x| x.1).unwrap_or("").trim();
    }
    let (pos, moves) = if t.starts_with('(') {
        (default_position.unwrap_or("startpos"), t)
    } else {
        match t.find(" moves") {
            Some(i) => (&t[..i], t[i + 6..].trim()),
            None => (t, ""),
        }
    };
    let pos = if pos.trim().is_empty() { "startpos" } else { pos.trim() };
    let mut game = Game::new(pos)?;
    let actions = split_actions(moves)?;
    let ids = replay(&mut game, &actions)?;
    let variant = variant_of(&game.nodes[ROOT].state);
    let meta = obj![
        ("format", "5duci"),
        ("variant", variant.unwrap_or("Custom")),
        ("white", "White"),
        ("black", "Black"),
        ("result", Json::Null),
        ("actions", ids.len())
    ];
    Ok(Loaded { game, meta })
}

pub fn load_pgn(text: &str, default_position: Option<&str>) -> Result<Loaded, String> {
    let default_setup = match default_position {
        Some(p) => Some(setup_of_uci(p)?),
        None => None,
    };
    let names_position = text.lines().any(|l| {
        let l = l.trim_start();
        l.starts_with("[Board") || l.starts_with("[Variant") || (l.starts_with('[') && l.contains(':') && !l.contains('"'))
    });
    let g = pgn::parse_5dpgn(text, default_setup.as_ref()).map_err(|e| {
        if names_position || default_position.is_some() {
            e
        } else {
            format!("{}\nThis game does not name its start position, so Standard was assumed; give the start position (a variant or a 5DUCI position) and load again.", e)
        }
    })?;
    let start = setup_to_uci(&g.setup);
    let mut game = Game::new(&start)?;
    game.nodes[ROOT].state.set_promotion_options(g.setup.promotions);
    game.nodes[ROOT].comments = g.root.comments.clone();
    // Depth-first over the variation tree; children[0] stays the main line. An illegal action
    // ends the main line with an error; in a variation it drops that variation (with a warning).
    let mut warnings: Vec<Json> = Vec::new();
    let mut stack = vec![(ROOT, &g.root, true)];
    while let Some((id, node, main)) = stack.pop() {
        for (k, ch) in node.children.iter().enumerate() {
            let Some(act) = &ch.action else { continue };
            match game.add_child(id, act.moves()) {
                Ok(cid) => {
                    game.nodes[cid].comments = ch.comments.clone();
                    stack.push((cid, ch, main && k == 0));
                }
                Err(e) if main && k == 0 => return Err(format!("{}: {}", ch.text, e)),
                Err(e) => warnings.push(format!("variation {} skipped: {}", ch.text, e).into()),
            }
        }
    }
    let header = |k: &str| g.headers.iter().find(|(h, _)| h.eq_ignore_ascii_case(k)).map(|(_, v)| v.clone());
    let variant = variant_of(&game.nodes[ROOT].state);
    let headers: Vec<Json> = g.headers.iter().map(|(k, v)| Json::Arr(vec![k.as_str().into(), v.as_str().into()])).collect();
    let meta = obj![
        ("format", "5dpgn"),
        ("variant", variant.map(String::from).or_else(|| header("Board")).unwrap_or_else(|| "Custom".into())),
        ("white", header("White").unwrap_or_else(|| "White".into())),
        ("black", header("Black").unwrap_or_else(|| "Black".into())),
        ("result", g.result.clone().or_else(|| header("Result"))),
        ("headers", Json::Arr(headers)),
        ("warnings", Json::Arr(warnings)),
        ("actions", game.mainline().len() - 1)
    ];
    Ok(Loaded { game, meta })
}

/// A 5DUCI position argument as a variant setup (for 5dpgn without a start position).
fn setup_of_uci(position: &str) -> Result<VariantSetup, String> {
    let p = position.trim();
    if p == "startpos" {
        return Ok(default_variant("Standard - Turn Zero").unwrap());
    }
    if let Some(v) = default_variant(p) {
        return Ok(v);
    }
    Ok(setup_of_state(&position_from_uci(p)?))
}

/// The setup a state starts from (every board of `s`, as custom FEN tags).
pub fn setup_of_state(s: &State) -> VariantSetup {
    if let Some(v) = variant_of(s).and_then(default_variant) {
        return v;
    }
    let (sx, sy) = s.get_board_size();
    let boards = s
        .multiverse()
        .get_boards(true)
        .into_iter()
        .map(|(l, t, c, fen)| crate::core::variants::BoardSpec { fen, sign: Sign::Nil, l, t, c })
        .collect();
    VariantSetup { size_x: sx, size_y: sy, boards, even: s.multiverse().layout().is_even(), promotions: s.get_promotion_options() }
}

/// Short summary of a record line for game lists (no replay).
pub fn summary(j: &Json) -> Json {
    let new_first = j.get("new_first").bool();
    let white_score = match (j.get("score").num(), new_first) {
        (Some(s), Some(nf)) => Some(if nf { s } else { 1.0 - s }),
        (s, None) => s,
        _ => None,
    };
    let moves = j.get("moves").str().unwrap_or("");
    let actions = moves.split_whitespace().filter(|t| *t == "submit").count();
    obj![
        ("game", j.get("game").int()),
        ("pair", j.get("pair").int()),
        ("newFirst", new_first),
        ("result", white_score.map(result_text)),
        ("reason", j.get("reason").str()),
        ("actions", actions),
        ("book", j.get("book").int())
    ]
}
