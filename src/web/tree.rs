//! An analysis game tree: positions reached by whole actions, with variations, the moves of an
//! action being built ("draft") and JSON views of a position for the board renderer.

use super::json::Json;
use super::pgn;
use crate::core::action::{Action, ExtMove, FullMove};
use crate::core::hypercuboid::{self, HcInfo};
use crate::core::piece::NO_PIECE;
use crate::core::state::{MateType, State};
use crate::core::vec4::Vec4;
use crate::obj;

pub const ROOT: usize = 0;

#[derive(Clone)]
pub struct Node {
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// Position after the action (the start of the next turn).
    pub state: State,
    /// The action that reached this node, in standard order (empty at the root).
    pub moves: Vec<ExtMove>,
    /// The action in 5DUCI long algebraic notation, moves separated by spaces.
    pub lan: String,
    /// The action in 5dpgn notation.
    pub san: String,
    /// Part of the game's opening book (not chosen by an engine).
    pub book: bool,
    /// Engine score recorded with the action, from the mover's point of view.
    pub eval: Option<i32>,
    pub comments: Vec<String>,
    /// Timelines the action created: (new line, line it branched from).
    pub branches: Vec<(i32, i32)>,
    /// Actions from the root.
    pub ply: usize,
    status: Option<Status>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Playing,
    Checkmate,
    Stalemate,
    /// Stalemate lost under the exiled-kings rule by White / by Black (no royal piece on the
    /// latest board of every active timeline).
    WhiteExiled,
    BlackExiled,
    /// Not decided: the position is too large for an exhaustive legal-action search.
    Unknown,
}

/// Timelines beyond which exhaustive searches (has the side to move any legal action? is a
/// check mate?) are skipped: with many boards on the present they can take minutes.
pub const SEARCH_LIMIT_LINES: i32 = 32;

/// Whether exhaustive searches are affordable in `s`.
pub fn searchable(s: &State) -> bool {
    let (lo, hi) = s.get_lines_range();
    hi - lo < SEARCH_LIMIT_LINES
}

/// 5dpgn of an action for display (short notation; mate symbols when affordable).
pub fn action_san(s: &State, act: &Action) -> String {
    pgn::action_to_pgn_opts(s, act, None, pgn::PgnOptions { short: true, mate: searchable(s) })
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Status::Playing => "playing",
            Status::Checkmate => "checkmate",
            Status::Stalemate => "stalemate",
            Status::WhiteExiled => "exiled-white",
            Status::BlackExiled => "exiled-black",
            Status::Unknown => "unknown",
        }
    }
}

#[derive(Clone)]
pub struct Game {
    /// 5DUCI `position` argument of the root.
    pub start: String,
    pub nodes: Vec<Node>,
}

pub fn v4(p: Vec4) -> Json {
    Json::Arr(vec![p.l().into(), p.t().into(), p.x().into(), p.y().into()])
}

fn parse_v4(j: &Json) -> Option<Vec4> {
    let a = j.arr();
    if a.len() < 4 {
        return None;
    }
    let n = |k: usize| a[k].int().map(|v| v as i32);
    Some(Vec4::new(n(2)?, n(3)?, n(1)?, n(0)?))
}

/// A move from JSON `{from:[l,t,x,y], to:[l,t,x,y], promote?:"Q"}`.
pub fn parse_move_json(j: &Json) -> Option<ExtMove> {
    let from = parse_v4(j.get("from"))?;
    let to = parse_v4(j.get("to"))?;
    let promote = j.get("promote").str().and_then(|s| s.bytes().next()).unwrap_or(NO_PIECE);
    Some(ExtMove::new(FullMove::new(from, to), promote))
}

fn move_json(m: &ExtMove) -> Json {
    let mut o = obj![("from", v4(m.fm.from)), ("to", v4(m.fm.to))];
    if m.promote_to != NO_PIECE
        && let Json::Obj(kv) = &mut o {
            kv.push(("promote".into(), ((m.promote_to as char).to_string()).into()));
        }
    o
}

/// Whether the side to move has a legal action.
/// How long the status check may look for a legal action. Finding one (or proving there is
/// none) can take minutes with many boards to play.
pub const STATUS_BUDGET: std::time::Duration = std::time::Duration::from_secs(2);

/// Whether the side to move has a legal action; None if the search ran out of `budget`.
pub fn has_legal_action(s: &State, budget: std::time::Duration) -> Option<bool> {
    let (w, ss) = HcInfo::build_hc(s);
    let stop = std::sync::atomic::AtomicBool::new(false);
    let _guard = hypercuboid::interrupt_on(&stop, Some(std::time::Instant::now() + budget));
    if w.search(s, ss).next().is_some() {
        Some(true)
    } else if hypercuboid::interrupted() {
        None
    } else {
        Some(false)
    }
}

/// The status of `s`. A check that runs out of time counts as Playing: mate, stalemate and
/// exile are only declared once no legal action is proven.
pub fn status_of(s: &State) -> Status {
    if has_legal_action(s, STATUS_BUDGET) != Some(false) {
        Status::Playing
    } else if s.get_mate_type() == MateType::Checkmate {
        Status::Checkmate
    } else {
        match s.exiled_kings_loser() {
            Some(false) => Status::WhiteExiled,
            Some(true) => Status::BlackExiled,
            None => Status::Stalemate,
        }
    }
}

/// The timeline a move would create from `s` and the line it branches from, if it branches.
pub fn branch_of(s: &State, m: &ExtMove) -> Option<(i32, i32)> {
    let (p, q) = (m.fm.from, m.fm.to);
    let player = s.get_present().1;
    (p.tl() != q.tl() && s.get_timeline_end(q.l()) != (q.t(), player)).then(|| (s.new_line(), q.l()))
}

/// Apply one move of an action being built; Err explains why it is not playable.
pub fn apply_draft_move(s: &mut State, mv: ExtMove) -> Result<(), String> {
    let player = s.get_present().1;
    let p = mv.fm.from;
    if s.get_board_ptr(p.l(), p.t(), player).is_none() || s.get_timeline_end(p.l()) != (p.t(), player) {
        return Err(format!("{} is not from a playable board", mv));
    }
    let n = s.normalize_promotion(mv).ok_or_else(|| format!("bad promotion in {}", mv))?;
    if !s.apply_ext::<false>(n) {
        return Err(format!("illegal move {}", mv));
    }
    Ok(())
}

/// Play a whole action from `s` (the sprt referee's rules); returns the position after it.
pub fn play_action(s: &State, moves: &[ExtMove]) -> Result<State, String> {
    let mut c = s.clone();
    for &m in moves {
        apply_draft_move(&mut c, m)?;
    }
    if !c.submit::<false>() {
        return Err("the action does not move the present: every board on the present must be played".into());
    }
    if c.first_check(c.get_present().1).is_some() {
        return Err("the action leaves a royal piece in check".into());
    }
    Ok(c)
}

/// Parse a list of LAN tokens (`submit` tokens are ignored).
pub fn parse_lan_moves(text: &str) -> Result<Vec<ExtMove>, String> {
    text.split_whitespace().filter(|t| *t != "submit").map(ExtMove::parse).collect()
}

/// The action's LAN tokens, played in order from `s`.
pub fn action_lan(s: &State, act: &Action) -> String {
    let mut os = s.clone();
    let mut parts = Vec::new();
    for em in act.moves() {
        parts.push(em.lan(&os));
        if let Some(n) = os.normalize_promotion(*em) {
            os.apply_ext::<false>(n);
        }
    }
    parts.join(" ")
}

impl Game {
    pub fn new(start: &str) -> Result<Game, String> {
        let state = crate::core::variants::position_from_uci(start)?;
        Ok(Game {
            start: start.trim().to_string(),
            nodes: vec![Node {
                parent: None,
                children: Vec::new(),
                state,
                moves: Vec::new(),
                lan: String::new(),
                san: String::new(),
                book: false,
                eval: None,
                comments: Vec::new(),
                branches: Vec::new(),
                ply: 0,
                status: None,
            }],
        })
    }

    pub fn node(&self, id: usize) -> Result<&Node, String> {
        self.nodes.get(id).ok_or_else(|| format!("no node {}", id))
    }

    /// Add (or find) the child of `parent` reached by `moves`.
    pub fn add_child(&mut self, parent: usize, moves: &[ExtMove]) -> Result<usize, String> {
        let ps = self.node(parent)?.state.clone();
        let act = {
            let mut norm = Vec::with_capacity(moves.len());
            for &m in moves {
                norm.push(ps.normalize_promotion(m).ok_or_else(|| format!("bad promotion in {}", m))?);
            }
            Action::from_vector(&norm, &ps)
        };
        let state = play_action(&ps, act.moves())?;
        let moves: Vec<ExtMove> = act.moves().to_vec();
        let mut branches = Vec::new();
        let mut bs = ps.clone();
        for &m in &moves {
            branches.extend(branch_of(&bs, &m));
            let _ = apply_draft_move(&mut bs, m);
        }
        if let Some(&c) = self.nodes[parent].children.iter().find(|&&c| self.nodes[c].moves == moves) {
            return Ok(c);
        }
        let id = self.nodes.len();
        let san = action_san(&ps, &act);
        self.nodes.push(Node {
            parent: Some(parent),
            children: Vec::new(),
            state,
            lan: action_lan(&ps, &act),
            san,
            moves,
            book: false,
            eval: None,
            comments: Vec::new(),
            branches,
            ply: self.nodes[parent].ply + 1,
            status: None,
        });
        self.nodes[parent].children.push(id);
        self.nodes[parent].status = Some(Status::Playing);
        Ok(id)
    }

    pub fn status(&mut self, id: usize) -> Status {
        if let Some(s) = self.nodes[id].status {
            return s;
        }
        // A node with a continuation has a legal action; large positions are not searched.
        let s = if !self.nodes[id].children.is_empty() {
            Status::Playing
        } else if searchable(&self.nodes[id].state) {
            status_of(&self.nodes[id].state)
        } else {
            Status::Unknown
        };
        self.nodes[id].status = Some(s);
        s
    }

    /// Node ids from the root to `id` (inclusive).
    pub fn path(&self, id: usize) -> Vec<usize> {
        let mut v = vec![id];
        let mut c = id;
        while let Some(p) = self.nodes[c].parent {
            v.push(p);
            c = p;
        }
        v.reverse();
        v
    }

    /// The main line: the root followed by first children.
    pub fn mainline(&self) -> Vec<usize> {
        let mut v = vec![ROOT];
        let mut c = ROOT;
        while let Some(&n) = self.nodes[c].children.first() {
            v.push(n);
            c = n;
        }
        v
    }

    /// 5DUCI `position` argument (with `moves`) for node `id`.
    pub fn position_command(&self, id: usize) -> String {
        let mut s = self.start.clone();
        let path = self.path(id);
        if path.len() > 1 {
            s += " moves";
            for &n in &path[1..] {
                s.push(' ');
                s += &self.nodes[n].lan;
                s += " submit";
            }
        }
        s
    }

    /// Remove `id` and its subtree from its parent (the nodes stay allocated).
    pub fn delete(&mut self, id: usize) -> Result<usize, String> {
        let p = self.node(id)?.parent.ok_or("cannot delete the root")?;
        self.nodes[p].children.retain(|&c| c != id);
        self.nodes[p].status = None;
        Ok(p)
    }

    /// Make `id`'s branch the first child at every level up to the root (the main line).
    pub fn promote(&mut self, id: usize) {
        let mut c = id;
        while self.promote_at_fork(c) {
            c = self.nodes[c].parent.unwrap();
        }
    }

    /// Make `id` the first child of its parent only (one level up among its siblings); false
    /// for the root.
    pub fn promote_at_fork(&mut self, id: usize) -> bool {
        let Some(p) = self.nodes[id].parent else { return false };
        let ch = &mut self.nodes[p].children;
        if let Some(i) = ch.iter().position(|&x| x == id) {
            let x = ch.remove(i);
            ch.insert(0, x);
        }
        true
    }

    /// The tree as nested move lists for the move panel.
    pub fn tree_json(&self) -> Json {
        let mut nodes = Vec::with_capacity(self.nodes.len());
        for (i, n) in self.nodes.iter().enumerate() {
            nodes.push(obj![
                ("id", i),
                ("parent", n.parent),
                ("children", n.children.clone()),
                ("ply", n.ply),
                ("san", n.san.as_str()),
                ("lan", n.lan.as_str()),
                ("book", n.book),
                ("eval", n.eval),
                ("comments", n.comments.clone()),
                ("player", n.ply > 0 && self.nodes[n.parent.unwrap()].state.get_present().1),
                ("turn", if n.ply > 0 { self.nodes[n.parent.unwrap()].state.get_present().0 } else { 0 })
            ]);
        }
        obj![("start", self.start.as_str()), ("nodes", Json::Arr(nodes))]
    }

    /// The position at `id` with `draft` moves applied.
    pub fn draft_state(&self, id: usize, draft: &[ExtMove]) -> Result<State, String> {
        let mut s = self.node(id)?.state.clone();
        for &m in draft {
            apply_draft_move(&mut s, m)?;
        }
        Ok(s)
    }

    /// Destination squares of the piece at `from` in the draft position.
    pub fn targets(&self, id: usize, draft: &[ExtMove], from: Vec4) -> Result<Vec<Vec4>, String> {
        self.piece_targets(id, draft, from, None)
    }

    /// Destinations of the piece at `from` on the board of colour `color` (default: the side to
    /// move, on a playable board). With the other colour this previews a move the opponent could
    /// make from that board (e.g. on a board this action just created); nothing is played.
    pub fn piece_targets(&self, id: usize, draft: &[ExtMove], from: Vec4, color: Option<bool>) -> Result<Vec<Vec4>, String> {
        let s = self.draft_state(id, draft)?;
        let player = s.get_present().1;
        if let Some(c) = color.filter(|&c| c != player) {
            let Some(b) = s.get_board_ptr(from.l(), from.t(), c) else { return Ok(Vec::new()) };
            let p = b.get_piece(from.xy());
            if p == NO_PIECE || p == crate::core::piece::WALL_PIECE || crate::core::piece::piece_color(p) != c {
                return Ok(Vec::new());
            }
            let mut out = Vec::new();
            s.multiverse().gen_piece_move(from, c, &mut out);
            return Ok(out);
        }
        if s.get_board_ptr(from.l(), from.t(), player).is_none() || s.get_timeline_end(from.l()) != (from.t(), player) {
            return Ok(Vec::new());
        }
        let p = s.get_piece(from, player);
        if p == NO_PIECE || crate::core::piece::piece_color(p) != player {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        s.gen_piece_move(from, &mut out);
        Ok(out)
    }

    /// Everything the renderer needs to draw node `id` (with `draft` moves applied).
    pub fn view(&mut self, id: usize, draft: &[ExtMove]) -> Result<Json, String> {
        let status = if draft.is_empty() { Some(self.status(id)) } else { None };
        let node = self.node(id)?;
        let s = self.draft_state(id, draft)?;
        let (t, player) = s.get_present();
        let (sx, sy) = s.get_board_size();
        let (lmin, lmax) = s.get_lines_range();
        let (amin, amax) = s.get_active_range();
        let (ilmin, ilmax) = s.get_initial_lines_range();
        let boards: Vec<Json> = s
            .multiverse()
            .get_boards(true)
            .into_iter()
            .map(|(l, t, c, fen)| Json::Arr(vec![l.into(), t.into(), (c as i32).into(), fen.into()]))
            .collect();
        let ts = s.get_timeline_status();
        let mut timelines = Vec::new();
        for l in lmin..=lmax {
            let (st, sc) = s.get_timeline_start(l);
            let (et, ec) = s.get_timeline_end(l);
            let kind = if ts.mandatory.contains(&l) {
                "mandatory"
            } else if ts.optional.contains(&l) {
                "optional"
            } else {
                "unplayable"
            };
            timelines.push(obj![
                ("l", l),
                ("label", s.pretty_l(l)),
                ("start", Json::Arr(vec![st.into(), (sc as i32).into()])),
                ("end", Json::Arr(vec![et.into(), (ec as i32).into()])),
                ("status", kind),
                ("active", l >= amin && l <= amax)
            ]);
        }
        let chk = |fm: &FullMove| obj![("from", v4(fm.from)), ("to", v4(fm.to)), ("c", (!player) as i32)];
        // At the start of a turn, checks are found on the phantom position (every present
        // board passed); while building an action, on the boards already played.
        let (checks, phantom): (Vec<Json>, Vec<Json>) = if draft.is_empty() {
            let ps = s.phantom();
            let checks = ps.find_checks(!player);
            let mut tls: Vec<(i32, i32)> = Vec::new();
            for c in &checks {
                tls.push((c.from.l(), c.from.t()));
                tls.push((c.to.l(), c.to.t()));
            }
            let real: std::collections::HashSet<(i32, i32, bool)> =
                s.multiverse().get_boards(false).into_iter().map(|(l, t, c, _)| (l, t, c)).collect();
            let ph = ps
                .multiverse()
                .get_boards(true)
                .into_iter()
                .filter(|(l, t, c, _)| tls.contains(&(*l, *t)) && !real.contains(&(*l, *t, *c)))
                .map(|(l, t, c, fen)| Json::Arr(vec![l.into(), t.into(), (c as i32).into(), fen.into()]))
                .collect();
            (checks.iter().map(chk).collect(), ph)
        } else {
            (s.find_checks(!player).iter().map(chk).collect(), Vec::new())
        };
        // The action that reached this node: arrows on the boards it was played from.
        let mut last = Vec::new();
        if let Some(p) = node.parent {
            let pc = self.nodes[p].state.get_present().1;
            for m in &node.moves {
                last.push(obj![("from", v4(m.fm.from)), ("to", v4(m.fm.to)), ("c", pc as i32)]);
            }
        }
        let mut branches: Vec<(i32, i32)> = Vec::new();
        for &n in &self.path(id) {
            branches.extend_from_slice(&self.nodes[n].branches);
        }
        // The draft's moves in short 5dpgn, as the move list writes them.
        let mut draft_san = Vec::with_capacity(draft.len());
        let mut ds = self.nodes[id].state.clone();
        for &m in draft {
            branches.extend(branch_of(&ds, &m));
            draft_san.push(Json::from(pgn::move_to_pgn_short(&ds, &m)));
            let _ = apply_draft_move(&mut ds, m);
        }
        let branches: Vec<Json> = branches
            .into_iter()
            .filter(|&(l, _)| l >= lmin && l <= lmax)
            .map(|(l, from)| {
                let (t, c) = s.get_timeline_start(l);
                Json::Arr(vec![l.into(), from.into(), t.into(), (c as i32).into()])
            })
            .collect();
        let submit = if draft.is_empty() { None } else { Some(play_action(&self.nodes[id].state, draft)) };
        let can_submit = submit.as_ref().is_some_and(|r| r.is_ok());
        let submit_error = submit.and_then(|r| r.err());
        let present_ok = s.can_submit().is_some();
        Ok(obj![
            ("id", id),
            ("size", Json::Arr(vec![sx.into(), sy.into()])),
            ("even", s.multiverse().layout().is_even()),
            // The pieces a pawn or brawn may promote to, one letter each, the engine's default
            // first ("" when promotion is off).
            ("promotions", crate::core::promotion::promotion_letters(s.get_promotion_options())),
            ("boards", Json::Arr(boards)),
            ("present", t),
            ("player", player as i32),
            ("lines", Json::Arr(vec![lmin.into(), lmax.into()])),
            ("initialLines", Json::Arr(vec![ilmin.into(), ilmax.into()])),
            ("active", Json::Arr(vec![amin.into(), amax.into()])),
            ("timelines", Json::Arr(timelines)),
            ("checks", Json::Arr(checks)),
            ("phantom", Json::Arr(phantom)),
            ("last", Json::Arr(last)),
            ("branches", Json::Arr(branches)),
            ("draft", Json::Arr(draft.iter().map(move_json).collect())),
            ("draftSan", Json::Arr(draft_san)),
            ("presentMoved", present_ok),
            ("canSubmit", can_submit),
            ("submitError", submit_error),
            ("status", status.map(|s| s.name())),
            ("position", self.position_command(id))
        ])
    }
}
