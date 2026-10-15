//! Hypercuboid legal-action search. An axis is a playable timeline or a prospective new timeline; a point picks one
//! semimove per axis; invalid regions are cut out as slices.

use super::action::{FullMove, MoveSeq};
use super::bitboard::*;
use super::board::{Board, RoyalSafety};
use super::check_position::{CheckBuf, CheckPosition};
use super::multiverse::{next_turn, MoveList};
use super::piece::*;
use super::state::State;
use super::vec4::Vec4;
use crate::misc::geometry::{Hc, Point, SearchSpace, Slice};
use crate::misc::graph::Graph;
use crate::misc::integer_set::{Index, IntegerSet};
use crate::misc::ordering::{AxisOrders, HcOrdering};

const INVALID: Index = Index::MAX;

#[derive(Clone, Debug)]
/// Moves carry their white-normalized moving and captured piece kinds.
pub enum Entry {
    Physical { m: FullMove, b: Board, mover: Piece, victim: Piece },
    /// `idx` is the paired departure coordinate on axis `fa`.
    Arriving { m: FullMove, b: Board, idx: Index, fa: u32, mover: Piece, victim: Piece },
    Departing { from: Vec4, b: Board },
    Null,
}

impl Entry {
    #[inline(always)]
    pub fn is_null(&self) -> bool {
        matches!(self, Entry::Null)
    }
    #[inline(always)]
    pub fn board(&self) -> &Board {
        match self {
            Entry::Physical { b, .. } | Entry::Arriving { b, .. } | Entry::Departing { b, .. } => b,
            Entry::Null => panic!("cannot extract a board from a null HC entry"),
        }
    }
    /// (t, l) of the hotspot: source for physical/departing, destination for arriving.
    #[inline(always)]
    pub fn tl(&self) -> (i32, i32) {
        let p = match self {
            Entry::Physical { m, .. } => m.from,
            Entry::Arriving { m, .. } => m.to,
            Entry::Departing { from, .. } => *from,
            Entry::Null => panic!("a null semimove has no hotspot"),
        };
        (p.t(), p.l())
    }
}

/// Public semantic view of an axis coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Semimove {
    Physical(FullMove),
    Arriving(FullMove),
    Departing(Vec4),
    Null,
}

pub struct MoveBoards<'a> {
    pub mv: FullMove,
    pub result: &'a Board,
    pub departure: Option<&'a Board>,
}

/// Hypercuboid search context. Independent of the state it was built from (which callers
/// pass back in where needed), so a search can run while the state is made/unmade. Every
/// buffer is kept when it is rebuilt for another state (`rebuild`).
#[derive(Default)]
pub struct HcInfo {
    player: bool,
    /// (line, axis) of every axis, ascending by line.
    line_axes: Vec<(i32, usize)>,
    /// Axis of line `axis_lo + k` at `axis_of[k]` (`usize::MAX`: none).
    axis_lo: i32,
    axis_of: Vec<usize>,
    /// Entries of the existing playable timelines' axes (the first `new_axis` are in use).
    axis_coords: Vec<Vec<Entry>>,
    /// Entries shared by every prospective new-timeline axis.
    branch_entries: Vec<Entry>,
    dim: usize,
    universe: Hc,
    new_axis: usize,
    /// Coordinate groups per axis (index `new_axis` is shared by all branch axes).
    groups: Vec<AxisGroups>,
    scratch: BuildScratch,
    /// Every timeline ends on the mover's turn, so after a point only its moved timelines
    /// can give check.
    all_lines_mine: bool,
    /// Per existing axis (filled on demand, cleared by `rebuild`): every superphysical check
    /// from that timeline's next board, in scan order, when it is the only moved timeline
    /// and still holds all of the opponent's pieces.
    lone_checks: std::cell::RefCell<Vec<(bool, Vec<FullMove>)>>,
}

#[derive(Default)]
struct BuildScratch {
    playable: Vec<i32>,
    stays: Vec<Vec<FullMove>>,
    departures: Vec<Vec<Vec4>>,
    /// (destination line, move) of every jump, stably sorted by line.
    arrivals: Vec<(i32, FullMove)>,
    sorted: Vec<(i32, FullMove)>,
    line_counts: Vec<u32>,
    ml: MoveList,
    /// Departure coordinate of each source square, per axis.
    jump_tab: Vec<[Index; 64]>,
    /// (build stamp, per-board royal safety table).
    safety: (u32, Vec<(u32, RoyalSafety)>),
    group_slots: GroupSlots,
}

#[derive(Default)]
struct AxisGroups {
    nonjump: IntegerSet,
    /// (departure axis, departure coordinate, arrivals depending on it); the first
    /// `dep_len` are in use (the rest keep their storage).
    by_dep: Vec<(u32, Index, IntegerSet)>,
    dep_len: usize,
    /// (departure axis, all arrivals from it); the first `from_len` are in use.
    by_from: Vec<(u32, IntegerSet)>,
    from_len: usize,
}

/// Position of each departure's (and departure axis') group while one `AxisGroups` is
/// built; entries are valid when stamped with the current build.
#[derive(Default)]
struct GroupSlots {
    stamp: u32,
    /// By departure axis * 64 + departure square.
    dep: Vec<(u32, u32)>,
    /// By departure axis.
    from: Vec<(u32, u32)>,
}

impl GroupSlots {
    fn begin(&mut self, axes: usize) {
        if self.dep.len() < axes * 64 {
            self.dep.resize(axes * 64, (0, 0));
            self.from.resize(axes, (0, 0));
        }
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.dep.iter_mut().for_each(|e| e.0 = 0);
            self.from.iter_mut().for_each(|e| e.0 = 0);
            self.stamp = 1;
        }
    }
}

impl AxisGroups {
    /// Group `entries` (their arrivals come from `axes` departure axes).
    fn rebuild(&mut self, entries: &[Entry], axes: usize, slots: &mut GroupSlots) {
        self.nonjump.clear();
        self.dep_len = 0;
        self.from_len = 0;
        slots.begin(axes);
        let stamp = slots.stamp;
        for (i, e) in entries.iter().enumerate() {
            let i = i as Index;
            match e {
                Entry::Physical { .. } | Entry::Null => self.nonjump.insert(i),
                Entry::Arriving { m, idx, fa, .. } if *idx != INVALID => {
                    let (fa, idx) = (*fa, *idx);
                    // (departure axis, departure square) identifies the departure coordinate.
                    let slot = &mut slots.dep[fa as usize * 64 + m.from.xy() as usize];
                    if slot.0 != stamp {
                        *slot = (stamp, self.dep_len as u32);
                        if self.dep_len == self.by_dep.len() {
                            self.by_dep.push((fa, idx, IntegerSet::new()));
                        } else {
                            let e = &mut self.by_dep[self.dep_len];
                            (e.0, e.1) = (fa, idx);
                            e.2.clear();
                        }
                        self.dep_len += 1;
                    }
                    self.by_dep[slot.1 as usize].2.insert(i);
                    let slot = &mut slots.from[fa as usize];
                    if slot.0 != stamp {
                        *slot = (stamp, self.from_len as u32);
                        if self.from_len == self.by_from.len() {
                            self.by_from.push((fa, IntegerSet::new()));
                        } else {
                            let e = &mut self.by_from[self.from_len];
                            e.0 = fa;
                            e.1.clear();
                        }
                        self.from_len += 1;
                    }
                    self.by_from[slot.1 as usize].1.insert(i);
                }
                _ => {}
            }
        }
    }
}

/// Reusable buffers for `HcInfo::take_point`; the point taken is left in `point`.
#[derive(Default)]
pub struct PointScratch {
    g: Graph,
    must_include: Vec<Index>,
    edge_refs: Vec<Index>,
    want: Vec<bool>,
    pub point: Point,
}

/// True if a royal piece of `defender`'s color is attacked on this board.
pub fn has_physical_check(position: &Board, defender: bool) -> bool {
    let defending = if defender { position.black() } else { position.white() };
    for sq in marked_pos(position.royal() & defending) {
        if position.is_under_attack(sq, defender) {
            return true;
        }
    }
    false
}

fn signum(x: i32) -> i32 {
    (x > 0) as i32 - (x < 0) as i32
}

impl HcInfo {
    #[inline(always)]
    pub fn get_universe(&self) -> &Hc {
        &self.universe
    }
    #[inline(always)]
    pub fn dimension(&self) -> usize {
        self.dim
    }
    #[inline(always)]
    pub fn player(&self) -> bool {
        self.player
    }
    #[inline(always)]
    pub fn axis_len(&self, n: usize) -> usize {
        if n < self.new_axis { self.axis_coords[n].len() } else { self.branch_entries.len() }
    }
    #[inline(always)]
    pub fn first_branch_axis(&self) -> usize {
        self.new_axis
    }
    #[inline(always)]
    pub fn line_to_axis(&self, l: i32) -> Option<usize> {
        match self.axis_of.get(l.wrapping_sub(self.axis_lo) as u32 as usize) {
            Some(&a) if a != usize::MAX => Some(a),
            _ => None,
        }
    }
    #[inline(always)]
    pub fn entry(&self, n: usize, i: Index) -> &Entry {
        if n < self.new_axis { &self.axis_coords[n][i as usize] } else { &self.branch_entries[i as usize] }
    }
    pub fn get_semimove(&self, n: usize, i: Index) -> Semimove {
        match self.entry(n, i) {
            Entry::Physical { m, .. } => Semimove::Physical(*m),
            Entry::Arriving { m, .. } => Semimove::Arriving(*m),
            Entry::Departing { from, .. } => Semimove::Departing(*from),
            Entry::Null => Semimove::Null,
        }
    }
    pub fn arrival_move(&self, n: usize, i: Index) -> Option<FullMove> {
        match self.entry(n, i) {
            Entry::Arriving { m, .. } => Some(*m),
            _ => None,
        }
    }

    pub fn get_move_boards(&self, axis: usize, coord: Index) -> Option<MoveBoards<'_>> {
        match self.entry(axis, coord) {
            Entry::Physical { m, b, .. } => Some(MoveBoards { mv: *m, result: b, departure: None }),
            Entry::Arriving { m, b, idx, .. } => {
                let src = self.line_to_axis(m.from.l()).expect("departure axis");
                let dep = match self.entry(src, *idx) {
                    Entry::Departing { b, .. } => b,
                    _ => panic!("arrival without departure"),
                };
                Some(MoveBoards { mv: *m, result: b, departure: Some(dep) })
            }
            _ => None,
        }
    }

    pub fn build_hc(s: &State) -> (HcInfo, SearchSpace) {
        let mut info = HcInfo::default();
        let ss = info.rebuild(s);
        (info, ss)
    }

    /// Build the search context of `s` in place (reusing this one's buffers); returns the
    /// initial search space.
    pub fn rebuild(&mut self, s: &State) -> SearchSpace {
        let _p = crate::misc::prof::scope(0);
        let st_scope = crate::misc::prof::scope(15);
        let HcInfo { player: info_player, line_axes, axis_lo, axis_of, axis_coords, branch_entries, dim, universe, new_axis, groups, scratch, all_lines_mine, lone_checks } =
            self;
        let BuildScratch { playable, stays, departures, arrivals, sorted, line_counts, ml, jump_tab, safety, group_slots } = scratch;
        let present = s.get_present();
        let player = present.1;
        let (lines_lo, lines_hi) = s.get_lines_range();
        let (amin, amax) = s.get_active_range();
        // Playable timelines: the mandatory ones, then the optional ones.
        playable.clear();
        for l in lines_lo..=lines_hi {
            if amin <= l && l <= amax && s.get_timeline_end(l) == present {
                playable.push(l);
            }
        }
        for l in lines_lo..=lines_hi {
            let tc = s.get_timeline_end(l);
            if !(amin <= l && l <= amax && tc == present) && tc.1 == player {
                playable.push(l);
            }
        }
        let np = playable.len();
        // Axis of each line (existing playable timelines and the new ones branches create).
        *axis_lo = lines_lo - np as i32;
        axis_of.clear();
        axis_of.resize((lines_hi - lines_lo + 1) as usize + 2 * np, usize::MAX);
        for (axis, &l) in playable.iter().enumerate() {
            axis_of[(l - *axis_lo) as usize] = axis;
        }
        let axis_lo = *axis_lo;
        let axis_of_line = |axis_of: &[usize], l: i32| axis_of[(l - axis_lo) as usize];
        if stays.len() < np {
            stays.resize_with(np, Vec::new);
            departures.resize_with(np, Vec::new);
        }
        arrivals.clear();
        drop(st_scope);
        let gen_scope = crate::misc::prof::scope(1);
        for (axis, &l) in playable.iter().enumerate() {
            stays[axis].clear();
            departures[axis].clear();
            let t = s.get_timeline_end(l).0;
            let p0 = Vec4::new(0, 0, t, l);
            let b = s.board(l, t, player);
            for pos in marked_pos(b.friendly(player) & !b.wall()) {
                let from = Vec4::from_xy(pos, p0);
                let mut has_depart = false;
                ml.clear();
                s.multiverse().gen_moves(from, player, ml);
                for &(r, bb) in ml.iter() {
                    for sq in marked_pos(bb) {
                        let to = Vec4::from_xy(sq, r);
                        let m = FullMove::new(from, to);
                        if from.tl() != to.tl() {
                            if !has_depart {
                                departures[axis].push(from);
                                has_depart = true;
                            }
                            arrivals.push((to.l(), m));
                        } else {
                            stays[axis].push(m);
                        }
                    }
                }
            }
        }
        drop(gen_scope);
        // Stable bucket sort by destination line.
        let nl = (lines_hi - lines_lo + 1) as usize;
        line_counts.clear();
        line_counts.resize(nl + 1, 0);
        for a in arrivals.iter() {
            line_counts[(a.0 - lines_lo) as usize + 1] += 1;
        }
        for k in 1..=nl {
            line_counts[k] += line_counts[k - 1];
        }
        sorted.clear();
        sorted.resize(arrivals.len(), (0, FullMove::default()));
        for &a in arrivals.iter() {
            let k = &mut line_counts[(a.0 - lines_lo) as usize];
            sorted[*k as usize] = a;
            *k += 1;
        }
        std::mem::swap(arrivals, sorted);
        let max_branch = departures[..np].iter().filter(|d| !d.is_empty()).count();
        jump_tab.clear();
        jump_tab.resize(np, [INVALID; 64]);
        let find_jump = |jt: &[[Index; 64]], axis_of: &[usize], from: Vec4| {
            let i = jt[axis_of_line(axis_of, from.l())][from.xy() as usize];
            if i == INVALID { None } else { Some(i) }
        };
        let (_, size_y) = s.get_board_size();
        let promo_piece = |m: FullMove| -> Piece {
            s.normalize_promotion(super::action::ExtMove::new(m, NO_PIECE)).map(|e| e.promote_to).unwrap_or(NO_PIECE)
        };

        // Royal safety per board: only royal moves, pinned pieces, en passant or a board
        // already in check can leave the mover's royal attacked, so most results skip the
        // full check test.
        // Memoized per board, by (line, turn) (entries stamped with this build).
        let nt = (lines_lo..=lines_hi).map(|l| s.get_timeline_end(l).0).max().unwrap_or(0) as usize + 1;
        let need = (lines_hi - lines_lo + 1) as usize * nt;
        if safety.1.len() < need {
            safety.1.resize(need, (0, RoyalSafety { in_check: false, pinned: 0 }));
        }
        safety.0 = safety.0.wrapping_add(1);
        if safety.0 == 0 {
            safety.1.iter_mut().for_each(|e| e.0 = 0);
            safety.0 = 1;
        }
        let stamp = safety.0;
        let mut safety = |l: i32, t: i32| -> RoyalSafety {
            let e = &mut safety.1[(l - lines_lo) as usize * nt + t as usize];
            if e.0 != stamp {
                *e = (stamp, s.board(l, t, player).royal_safety(player));
            }
            e.1
        };
        let ent_scope = crate::misc::prof::scope(12);
        if axis_coords.len() < np {
            axis_coords.resize_with(np, Vec::new);
        }
        for axis in 0..np {
            let l = playable[axis];
            let lo = arrivals.partition_point(|a| a.0 < l);
            let hi = arrivals.partition_point(|a| a.0 <= l);
            let locs = &mut axis_coords[axis];
            locs.clear();
            locs.push(Entry::Null);
            let board_t = s.get_timeline_end(l).0;
            let rs = safety(l, board_t);
            for &m in &stays[axis] {
                let (p, q) = (m.from, m.to);
                let d = q - p;
                let source = s.board(p.l(), p.t(), player);
                let mut nb = *source;
                let z = pmask(p.xy());
                let mut needs_test = rs.in_check || (source.royal() | rs.pinned) & z != 0;
                if source.lpawn() & z != 0 && d.x() != 0 && source.occupied() & pmask(q.xy()) == 0 {
                    nb.clear_square(ppos(q.x(), p.y()));
                    nb.transfer(p.xy(), q.xy());
                    needs_test = true;
                } else if source.king() & z != 0 && d.x().abs() > 1 {
                    let rook_x1 = source.first_occupied_x(p.x(), p.y(), d.x().signum());
                    let rook_x2 = p.x() + d.x().signum();
                    nb.transfer(ppos(rook_x1, p.y()), ppos(rook_x2, q.y()));
                    nb.transfer(p.xy(), q.xy());
                } else {
                    nb.transfer(p.xy(), q.xy());
                }
                if source.lpawn() & z != 0 && (q.y() == 0 || q.y() == size_y - 1) {
                    let promotion = promo_piece(m);
                    if promotion != NO_PIECE {
                        nb.set_piece(q.xy(), if player { to_black(promotion) } else { promotion });
                    }
                }
                if !needs_test || !has_physical_check(&nb, player) {
                    let mover = source.kind(p.xy());
                    let victim = if mover == KING_W && d.x().abs() > 1 {
                        NO_PIECE
                    } else {
                        let v = source.kind(q.xy());
                        if v == NO_PIECE && (mover == PAWN_W || mover == BRAWN_W) && d.x() != 0 { source.kind(ppos(q.x(), p.y())) } else { v }
                    };
                    locs.push(Entry::Physical { m, b: nb, mover, victim });
                }
            }
            for &p in &departures[axis] {
                let idx = locs.len() as Index;
                let source = s.board(p.l(), p.t(), player);
                let mut nb = *source;
                nb.clear_square(p.xy());
                let z = pmask(p.xy());
                let needs_test = rs.in_check || (source.royal() | rs.pinned) & z != 0;
                if !needs_test || !has_physical_check(&nb, player) {
                    locs.push(Entry::Departing { from: p, b: nb });
                    jump_tab[axis][p.xy() as usize] = idx;
                }
            }
            for &(_, m) in &arrivals[lo..hi] {
                let (last_t, last_c) = s.get_timeline_end(m.to.l());
                if m.to.t() == last_t && player == last_c {
                    let (p, q) = (m.from, m.to);
                    let src = s.board(p.l(), p.t(), player);
                    let mut nb = *s.board(q.l(), q.t(), player);
                    let victim = nb.kind(q.xy());
                    let zp = pmask(p.xy());
                    let royal = src.royal() & zp != 0;
                    if src.lrawn() & zp != 0 && (q.y() == 0 || q.y() == size_y - 1) && promo_piece(m) != NO_PIECE {
                        let promotion = promo_piece(m);
                        nb.set_piece(q.xy(), if player { to_black(promotion) } else { promotion });
                    } else {
                        nb.place_from(q.xy(), src, p.xy());
                    }
                    let needs_test = royal || safety(q.l(), q.t()).in_check;
                    if !needs_test || !has_physical_check(&nb, player) {
                        let fa = axis_of_line(axis_of, m.from.l()) as u32;
                        locs.push(Entry::Arriving { m, b: nb, idx: INVALID, fa, mover: src.kind(p.xy()), victim });
                    }
                }
            }
        }

        drop(ent_scope);
        let br_scope = crate::misc::prof::scope(13);
        branch_entries.clear();
        branch_entries.push(Entry::Null);
        for &(_, m) in arrivals.iter() {
            let (p, q) = (m.from, m.to);
            if let Some(dep) = find_jump(jump_tab, axis_of, m.from) {
                let src = s.board(p.l(), p.t(), player);
                let mut nb = *s.board(q.l(), q.t(), player);
                let victim = nb.kind(q.xy());
                let zp = pmask(p.xy());
                let royal = src.royal() & zp != 0;
                if src.lrawn() & zp != 0 && (q.y() == 0 || q.y() == size_y - 1) && promo_piece(m) != NO_PIECE {
                    let promotion = promo_piece(m);
                    nb.set_piece(q.xy(), if player { to_black(promotion) } else { promotion });
                } else {
                    nb.place_from(q.xy(), src, p.xy());
                }
                let needs_test = royal || safety(q.l(), q.t()).in_check;
                if !needs_test || !has_physical_check(&nb, player) {
                    let fa = axis_of_line(axis_of, m.from.l()) as u32;
                    branch_entries.push(Entry::Arriving { m, b: nb, idx: dep, fa, mover: src.kind(p.xy()), victim });
                }
            }
        }
        drop(br_scope);
        let _fin = crate::misc::prof::scope(14);
        let new_l = s.new_line();
        let sign = signum(new_l);
        line_axes.clear();
        for (axis, &l) in playable.iter().enumerate() {
            line_axes.push((l, axis));
        }
        for i in 0..max_branch {
            let l = new_l + sign * i as i32;
            line_axes.push((l, np + i));
            axis_of[(l - axis_lo) as usize] = np + i;
        }
        line_axes.sort_unstable_by_key(|e| e.0);
        let dimension = np + max_branch;
        let n_branch = branch_entries.len();
        *universe = Hc::ranges(dimension, |n| if n < np { axis_coords[n].len() } else { n_branch });
        // Branch arrivals were only admitted with a live departure; existing-axis arrivals
        // lose their coordinate when their departure was filtered out.
        for n in 0..np {
            for i in 0..axis_coords[n].len() {
                if let Entry::Arriving { m, idx, .. } = &mut axis_coords[n][i] {
                    match find_jump(jump_tab, axis_of, m.from) {
                        Some(d) => *idx = d,
                        None => {
                            universe[n].erase(i as Index);
                        }
                    }
                }
            }
        }
        // The search only tests same-board checks on boards the action creates. A royal already
        // attacked on the same board of an opponent-to-move board stays capturable whatever the
        // mover does, so no action is legal (unreachable by legal play; set-up positions only).
        if dimension > 0
            && (lines_lo..=lines_hi).any(|l| {
                let (t, c) = s.get_timeline_end(l);
                c != player && has_physical_check(s.board(l, t, c), player)
            })
        {
            universe[0].clear();
        }
        let mut hc_n_lines = universe.clone();
        let mut non_null = IntegerSet::new();
        if np < dimension {
            non_null = IntegerSet::range(n_branch as Index);
            non_null.erase(0);
            for n in np..dimension {
                hc_n_lines[n].assign_singleton(0);
            }
        }
        let mut ss = SearchSpace::from_vec(vec![hc_n_lines.clone()]);
        for n in np..dimension {
            hc_n_lines[n].assign(&non_null);
            ss.push_front(hc_n_lines.clone());
        }
        if groups.len() < np + 1 {
            groups.resize_with(np + 1, AxisGroups::default);
        }
        for n in 0..np {
            groups[n].rebuild(&axis_coords[n], np, group_slots);
        }
        groups[np].rebuild(branch_entries, np, group_slots);
        *info_player = player;
        *dim = dimension;
        *new_axis = np;
        *all_lines_mine = (lines_lo..=lines_hi).all(|l| s.get_timeline_end(l).1 == player);
        lone_checks.get_mut().iter_mut().for_each(|e| e.0 = false);
        ss
    }

    /// Pick a point in `hc` whose departures and arrivals pair up (into `sc.point`). Prunes
    /// arrivals whose departure is no longer available.
    pub fn take_point(&self, hc: &mut Hc, order: &HcOrdering, sc: &mut PointScratch) -> bool {
        let _p = crate::misc::prof::scope(2);
        let dim = self.dimension();
        let PointScratch { g, must_include, edge_refs, want, point: result } = sc;
        g.reset(dim);
        must_include.clear();
        edge_refs.clear();
        edge_refs.resize(dim * dim, INVALID);
        result.clear();
        result.resize(dim, INVALID);
        want.clear();
        want.resize(dim, false);
        // Arrivals on branch axes whose departure left the hypercuboid (the departure axes
        // all come first, so this is the same for every branch axis).
        let mut branch_ghosts: Option<IntegerSet> = None;
        for n in 0..dim {
            let grp = &self.groups[n.min(self.new_axis)];
            // Arrivals whose departure left the hypercuboid can never be paired.
            if n < self.new_axis {
                let mut ghost: Option<IntegerSet> = None;
                for (fa, dep, set) in &grp.by_dep[..grp.dep_len] {
                    if !hc[*fa as usize].contains(*dep) && set.intersects(&hc[n]) {
                        ghost.get_or_insert_with(IntegerSet::new).or_assign(set);
                    }
                }
                if let Some(gs) = ghost {
                    hc[n].minus(&gs);
                }
            } else {
                let gs = branch_ghosts.get_or_insert_with(|| {
                    let mut g = IntegerSet::new();
                    for (fa, dep, set) in &grp.by_dep[..grp.dep_len] {
                        if !hc[*fa as usize].contains(*dep) {
                            g.or_assign(set);
                        }
                    }
                    g
                });
                hc[n].minus(gs);
            }
            if hc[n].is_empty() {
                return false;
            }
            let live = &hc[n];
            let need_nonjump = live.intersects(&grp.nonjump);
            let mut needed = need_nonjump as usize;
            for (fa, set) in &grp.by_from[..grp.from_len] {
                // An edge already claimed from the other endpoint keeps its first pairing.
                if edge_refs[*fa as usize * dim + n] == INVALID && set.intersects(live) {
                    want[*fa as usize] = true;
                    needed += 1;
                }
            }
            // First coordinate in axis order of each category: the non-jump choice, and one
            // arrival per departure axis (its matching edge).
            let mut found = 0;
            let mut visit = |i: Index| -> bool {
                match self.entry(n, i) {
                    Entry::Physical { .. } | Entry::Null => {
                        if result[n] == INVALID {
                            result[n] = i;
                            found += 1;
                        }
                    }
                    Entry::Arriving { idx, fa, .. } => {
                        let fa = *fa as usize;
                        if want[fa] {
                            want[fa] = false;
                            g.add_edge(fa as Index, n as Index);
                            edge_refs[fa * dim + n] = *idx;
                            edge_refs[n * dim + fa] = i;
                            found += 1;
                        }
                    }
                    Entry::Departing { .. } => {}
                }
                found == needed
            };
            match order {
                HcOrdering::Natural => {
                    for i in live.iter() {
                        if visit(i) {
                            break;
                        }
                    }
                }
                HcOrdering::Precomputed(o) => match live.single() {
                    // Orders list the universe: a lone live coordinate is the only one the
                    // scan below would visit.
                    Some(i) => {
                        if self.universe[n].contains(i) {
                            visit(i);
                        }
                    }
                    None => {
                        for &i in o.axis(n) {
                            if live.contains(i) && visit(i) {
                                break;
                            }
                        }
                    }
                },
            }
            if !need_nonjump {
                must_include.push(n as Index);
            }
        }
        let Some(matching) = g.find_matching(must_include) else { return false };
        for &(u, v) in matching {
            let (u, v) = (u as usize, v as usize);
            result[u] = edge_refs[u * dim + v];
            result[v] = edge_refs[v * dim + u];
        }
        debug_assert!(result.iter().all(|&i| i != INVALID));
        true
    }


    pub fn find_problem(&self, s: &State, p: &Point, hc: &Hc, buf: &mut CheckBuf) -> Option<Slice> {
        let jp = crate::misc::prof::scope(3);
        let r = self.jump_order_consistent(s, p, hc).or_else(|| self.test_present(s, p, hc));
        drop(jp);
        if r.is_some() {
            return r;
        }
        let _p = crate::misc::prof::scope(4);
        self.find_checks(s, p, hc, buf)
    }

    pub fn jump_order_consistent(&self, st: &State, p: &Point, hc: &Hc) -> Option<Slice> {
        let mut jump_map: Vec<(Vec4, usize)> = Vec::new();
        let (_, c) = st.get_present();
        for n in self.new_axis..self.dimension() {
            let m_arr = match self.entry(n, p[n]) {
                Entry::Null => break,
                Entry::Arriving { m, .. } => *m,
                _ => unreachable!(),
            };
            let (from, to) = (m_arr.from, m_arr.to);
            if let Some(m) = self.line_to_axis(to.l()) {
                let im = p[m];
                if self.entry(m, im).is_null() && st.get_timeline_end(to.l()) == (to.t(), c) {
                    let mut s = IntegerSet::new();
                    for i in hc[n].iter() {
                        if let Entry::Arriving { m: m3, .. } = self.entry(n, i) {
                            if m3.to.tl() == to.tl() {
                                s.insert(i);
                            }
                        }
                    }
                    let mut problem = Slice::new();
                    problem.fix_axis(n, s);
                    problem.fix_axis(m, IntegerSet::singleton(im));
                    debug_assert!(problem.contains(p));
                    return Some(problem);
                }
            }
            let critical = from.tl();
            if let Some(&(_, axis_branch)) = jump_map.iter().find(|j| j.0 == critical) {
                let mut s1 = IntegerSet::new();
                let mut s2 = IntegerSet::new();
                for i in hc[n].iter() {
                    if let Entry::Arriving { m: m1, .. } = self.entry(n, i) {
                        if m1.from.tl() == critical {
                            s1.insert(i);
                        }
                    }
                }
                for i in hc[axis_branch].iter() {
                    if let Entry::Arriving { m: m2, .. } = self.entry(axis_branch, i) {
                        if m2.to.tl() == critical {
                            s2.insert(i);
                        }
                    }
                }
                let mut problem = Slice::new();
                problem.fix_axis(n, s1);
                problem.fix_axis(axis_branch, s2);
                debug_assert!(problem.contains(p));
                return Some(problem);
            }
            match jump_map.iter_mut().find(|j| j.0 == to.tl()) {
                Some(j) => j.1 = n,
                None => jump_map.push((to.tl(), n)),
            }
        }
        None
    }

    /// Whether the point moves the present; if not, a slice of points that fail for the same
    /// reason. The new present is computed exactly: moved timelines advance one half-turn, new
    /// timelines start after their arrival, and the active range is recomputed from the final
    /// timeline span (which can reactivate old timelines).
    pub fn test_present(&self, st: &State, p: &Point, _hc: &Hc) -> Option<Slice> {
        let (_, c) = st.get_present();
        let (l_min, l_max) = st.get_lines_range();
        // New timelines in creation order (the branch axes form a prefix).
        let mut new_ends: Vec<i32> = Vec::new();
        let mut k = 0;
        for n in self.new_axis..self.dim {
            match self.entry(n, p[n]) {
                Entry::Arriving { m, .. } => new_ends.push(2 * m.to.t() + c as i32 + 1),
                _ => break,
            }
            k += 1;
        }
        let (l1_min, l1_max) = if c { (l_min - k as i32, l_max) } else { (l_min, l_max + k as i32) };
        let (amin, amax) = super::multiverse::active_range_for(st.multiverse().layout(), l1_min, l1_max);
        let end_after = |l: i32| -> i32 {
            if l < l_min || l > l_max {
                let j = if c { l_min - 1 - l } else { l - l_max - 1 };
                return new_ends[j as usize];
            }
            let v = st.multiverse().timeline_end_v(l);
            match self.line_to_axis(l) {
                Some(n) if !self.entry(n, p[n]).is_null() => v + 1,
                _ => v,
            }
        };
        let mut wit: Option<(i32, i32)> = None;
        for l in amin..=amax {
            let v = end_after(l);
            if wit.map(|w| v < w.1).unwrap_or(true) {
                wit = Some((l, v));
            }
        }
        let (w_line, w_v) = wit?;
        if (w_v & 1 != 0) != c {
            return None;
        }
        // Every point below keeps the witness timeline unmoved and active and adds no active
        // timeline that ends (on the opponent's turn) before it.
        let mut problem = Slice::new();
        let w_axis = self.line_to_axis(w_line).expect("witness axis");
        problem.fix_axis(w_axis, IntegerSet::singleton(0));
        for &(l, n) in &self.line_axes {
            if n >= self.new_axis || n == w_axis || l < amin || l > amax {
                continue;
            }
            if st.multiverse().timeline_end_v(l) + 1 < w_v {
                problem.fix_axis(n, IntegerSet::singleton(0));
            }
        }
        let t_w = w_v >> 1;
        for (j, n) in (self.new_axis..self.dim).enumerate() {
            let mut s = IntegerSet::new();
            if j < k {
                let line = if c { l_min - 1 - j as i32 } else { l_max + 1 + j as i32 };
                let active = line >= amin && line <= amax;
                for i in self.universe[n].iter() {
                    if let Entry::Arriving { m, .. } = self.entry(n, i) {
                        if !active || m.to.t() >= t_w {
                            s.insert(i);
                        }
                    }
                }
            } else {
                s.insert(0);
            }
            problem.fix_axis(n, s);
        }
        debug_assert!(problem.contains(p));
        Some(problem)
    }

    /// False when `p` moves a single existing timeline (physically) and none of the checks
    /// its next board could give survives the move; true when the full test is needed.
    fn lone_move_may_check(&self, st: &State, p: &Point, lo: i32, hi: i32, buf: &mut CheckBuf) -> bool {
        let mut moved = None;
        for &(l, axis) in &self.line_axes {
            if !self.entry(axis, p[axis]).is_null() {
                if axis >= self.new_axis || moved.is_some() {
                    return true;
                }
                moved = Some((l, axis));
            }
        }
        let Some((l, axis)) = moved else { return true };
        let Entry::Physical { b: result, .. } = self.entry(axis, p[axis]) else { return true };
        let c = self.player;
        let mut cache = self.lone_checks.borrow_mut();
        if cache.len() < self.new_axis {
            cache.resize_with(self.new_axis, Default::default);
        }
        let (known, checks) = &mut cache[axis];
        if !*known {
            // The timeline's next board with every opponent piece in place: the moved board
            // differs only by the mover's pieces and possibly one captured piece.
            let (t, _) = st.get_timeline_end(l);
            let mut cp = CheckPosition::with_range_in(st, lo, hi, std::mem::take(buf));
            cp.add_board(l, next_turn((t, c)), st.board(l, t, c));
            checks.clear();
            cp.checks_into(!c, false, checks);
            *buf = cp.into_buf();
            *known = true;
        }
        let attackers = result.hostile(c);
        checks.iter().any(|m| attackers & pmask(m.from.xy()) != 0)
    }

    pub fn find_checks(&self, st: &State, p: &Point, hc: &Hc, buf: &mut CheckBuf) -> Option<Slice> {
        let (_, c) = st.get_present();
        let (mut lo, mut hi) = st.get_lines_range();
        if let (Some(&(first, _)), Some(&(last, _))) = (self.line_axes.first(), self.line_axes.last()) {
            lo = lo.min(first);
            hi = hi.max(last);
        }
        if self.all_lines_mine && !self.lone_move_may_check(st, p, lo, hi, buf) {
            return None;
        }
        let mut newstate = CheckPosition::with_range_in(st, lo, hi, std::mem::take(buf));
        for &(l, axis) in &self.line_axes {
            let loc = self.entry(axis, p[axis]);
            if loc.is_null() {
                continue;
            }
            let (old_t, _) = loc.tl();
            newstate.add_board(l, next_turn((old_t, c)), loc.board());
        }
        let Some(check) = newstate.first_check(!c, false) else {
            *buf = newstate.into_buf();
            return None;
        };
        let (path, sliding_type) = get_move_path(&newstate, check, !c);
        let is_next = |t1: i32, t2: i32| if c { t1 + 1 == t2 } else { t1 == t2 };
        let sliding_bb = |nb: &Board| -> Bitboard {
            let bb = if c { nb.white() } else { nb.black() };
            bb & match sliding_type {
                1 => nb.lrook(),
                2 => nb.lbishop(),
                3 => nb.lunicorn(),
                4 => nb.ldragon(),
                _ => unreachable!("wrong sliding type inferred"),
            }
        };
        let mut problem = Slice::new();
        if let Some(n1) = self.line_to_axis(check.from.l()) {
            let mut not_taking = IntegerSet::new();
            for i in hc[n1].iter() {
                let loc = self.entry(n1, i);
                if loc.is_null() || !is_next(loc.tl().0, check.from.t()) {
                    continue;
                }
                let nb = loc.board();
                if sliding_type != 0 {
                    if pmask(check.from.xy()) & sliding_bb(nb) != 0 {
                        not_taking.insert(i);
                    }
                } else if nb.get_piece(check.from.xy()) == newstate.get_piece(check.from, !c) {
                    not_taking.insert(i);
                }
            }
            problem.fix_axis(n1, not_taking);
        }
        let l2 = check.to.l();
        if let Some(n2) = self.line_to_axis(l2) {
            let loc0 = self.entry(n2, p[n2]);
            if !(loc0.is_null() || !is_next(loc0.tl().0, check.to.t())) {
                let mut expose = IntegerSet::new();
                for i in hc[n2].iter() {
                    let loc = self.entry(n2, i);
                    if loc.is_null() || !is_next(loc.tl().0, check.to.t()) {
                        continue;
                    }
                    let nb = loc.board();
                    let friendly = if c { nb.black() } else { nb.white() };
                    if pmask(check.to.xy()) & nb.royal() & friendly != 0 {
                        expose.insert(i);
                    }
                }
                problem.fix_axis(n2, expose);
            }
        }
        for &crossed in &path {
            if let Some(n) = self.line_to_axis(crossed.l()) {
                let loc0 = self.entry(n, p[n]);
                if loc0.is_null() || !is_next(loc0.tl().0, crossed.t()) {
                    continue;
                }
                let z = pmask(crossed.xy());
                let mut not_blocking = IntegerSet::new();
                for i in hc[n].iter() {
                    let loc = self.entry(n, i);
                    if loc.is_null() || !is_next(loc.tl().0, crossed.t()) {
                        continue;
                    }
                    let nb = loc.board();
                    if z & nb.occupied() == 0 {
                        not_blocking.insert(i);
                        continue;
                    }
                    if sliding_type != 0 && z & sliding_bb(nb) != 0 {
                        not_blocking.insert(i);
                        continue;
                    }
                    let friendly = if c { nb.black() } else { nb.white() };
                    if z & nb.royal() & friendly != 0 {
                        not_blocking.insert(i);
                        continue;
                    }
                }
                problem.fix_axis(n, not_blocking);
            }
        }
        debug_assert!(problem.contains(p));
        *buf = newstate.into_buf();
        Some(problem)
    }

    pub fn to_action(&self, p: &Point) -> MoveSeq {
        let mut mvs = MoveSeq::new();
        for &(_, i) in &self.line_axes {
            match self.entry(i, p[i]) {
                Entry::Physical { m, .. } | Entry::Arriving { m, .. } => mvs.push(*m),
                _ => {}
            }
        }
        if self.player {
            mvs.reverse();
        }
        mvs
    }

    pub fn search<'a>(&'a self, s: &'a State, ss: SearchSpace) -> HcSearch<'a> {
        HcSearch { info: self, s, cursor: HcCursor::new(ss, HcOrdering::Natural) }
    }
}

/// Sliding path (endpoints excluded) and slider type (0 none, 1 rook .. 4 dragon).
fn get_move_path(s: &CheckPosition<'_>, fm: FullMove, c: bool) -> (Vec<Vec4>, i32) {
    let (p, q) = (fm.from, fm.to);
    let d = q - p;
    let b = s.get_board_ptr(p.l(), p.t(), c).unwrap();
    if b.sliding() & pmask(p.xy()) != 0 {
        let dir = Vec4::new(signum(d.x()), signum(d.y()), signum(d.t()), signum(d.l()));
        let st = dir.dot(dir);
        let mut path = Vec::new();
        let mut r = p + dir;
        while r != q {
            path.push(r);
            r = r + dir;
        }
        (path, st)
    } else {
        (Vec::new(), 0)
    }
}


/// Resumable legal-action enumeration. The caller supplies the context and the state it
/// was built from on every call (the state may be modified between calls if restored).
pub struct HcCursor {
    pub ss: SearchSpace,
    pub order: HcOrdering,
    /// The branch axes' shared order is still to be filled (see `next_with`).
    pub branch_pending: bool,
    sc: PointScratch,
    checks: CheckBuf,
}

impl Default for HcCursor {
    fn default() -> Self {
        HcCursor::new(SearchSpace::default(), HcOrdering::Natural)
    }
}

impl HcCursor {
    pub fn new(ss: SearchSpace, order: HcOrdering) -> Self {
        HcCursor { ss, order, branch_pending: false, sc: PointScratch::default(), checks: CheckBuf::default() }
    }

    pub fn next(&mut self, info: &HcInfo, s: &State) -> Option<MoveSeq> {
        self.next_with(info, s, &mut |_| {})
    }

    /// `next`, filling the branch axes' order with `fill_branch` when `branch_pending` and a
    /// hypercuboid first leaves several coordinates on a branch axis (only then can taking a
    /// point look at that order).
    pub fn next_with(&mut self, info: &HcInfo, s: &State, fill_branch: &mut dyn FnMut(&mut AxisOrders)) -> Option<MoveSeq> {
        while !self.ss.is_empty() {
            let mut hc = self.ss.pop_back().unwrap();
            if self.branch_pending
                && (info.first_branch_axis()..hc.dimension()).any(|n| !hc[n].is_empty() && hc[n].single().is_none())
            {
                fill_branch(self.order.axes_mut());
                self.branch_pending = false;
            }
            if !info.take_point(&mut hc, &self.order, &mut self.sc) {
                continue;
            }
            let pt = &self.sc.point;
            match info.find_problem(s, pt, &hc, &mut self.checks) {
                None => {
                    let act = info.to_action(pt);
                    hc.remove_point_with(pt, |h| self.ss.push_back(h));
                    return Some(act);
                }
                Some(problem) => {
                    let _p = crate::misc::prof::scope(5);
                    self.ss.push_back(hc);
                    self.ss.remove_slice_backwards(&problem, true, 10, 1);
                }
            }
        }
        None
    }
}

/// Legal actions roughly best-first by the sum of per-axis coordinate scores. The remaining
/// space is kept as hypercuboids in a heap keyed by the score of their best point (each
/// axis' first coordinate in `order`, which must be sorted by `scores` descending), so
/// after the top action the next ones deviate on whichever axis loses least, instead of
/// varying the last axis first. Problems are removed from the popped hypercuboid only.
/// Subspaces of the initial search space keep their precedence (later ones first).
pub struct BestFirstCursor {
    heap: BfHeap,
    sc: PointScratch,
    checks: CheckBuf,
}

struct BfHeap {
    heap: std::collections::BinaryHeap<BfItem>,
    order: HcOrdering,
    scores: Vec<Vec<i32>>,
    /// The coordinates the orders list (the context's universe).
    universe: Hc,
    seq: u64,
    /// The branch axes (from `first_branch`) are not ordered yet: only the null
    /// coordinate's score is known.
    branch_pending: bool,
    first_branch: usize,
}

/// Fills the branch axes' order and scores of a cursor whose branch ordering is pending.
pub type BranchFill<'a> = dyn FnMut(&mut AxisOrders, &mut Vec<Vec<i32>>) + 'a;

impl BfHeap {
    /// Order the branch axes now when `hc` has a branch coordinate other than the null one.
    fn need_branch(&mut self, hc: &Hc, several: bool, fill: &mut BranchFill) {
        if self.branch_pending
            && (self.first_branch..hc.dimension()).any(|n| if several { !hc[n].is_empty() && hc[n].single().is_none() } else { hc[n].has_other_than(0) })
        {
            fill(self.order.axes_mut(), &mut self.scores);
            self.branch_pending = false;
        }
    }

    /// One of the initial hypercuboids: each has a class of its own, so its bound never
    /// decides an order and is not computed.
    fn push_initial(&mut self, hc: Hc, class: i32) {
        if (0..hc.dimension()).any(|n| hc[n].is_empty()) {
            return;
        }
        self.seq += 1;
        self.heap.push(BfItem { key: (-class, 0, self.seq), hc });
    }

    fn push(&mut self, hc: Hc, class: i32, fill: &mut BranchFill) {
        let _p = crate::misc::prof::scope(11);
        self.need_branch(&hc, false, fill);
        let mut bound = 0i64;
        let shared = match &self.order {
            HcOrdering::Precomputed(o) => o.shared,
            HcOrdering::Natural => usize::MAX,
        };
        for n in 0..hc.dimension() {
            let sc = &self.scores[n.min(shared)];
            let set = &hc[n];
            let best = match &self.order {
                HcOrdering::Precomputed(_) if set.single().is_some_and(|i| self.universe[n].contains(i)) => {
                    set.single().map(|i| sc[i as usize])
                }
                // An order lists its axis' universe best first, so the first member of the
                // set has the set's best score.
                HcOrdering::Precomputed(o) if set.is_subset_of(&self.universe[n]) => {
                    o.axis(n).iter().find(|&&i| set.contains(i)).map(|&i| sc[i as usize])
                }
                _ => set.iter().map(|i| sc.get(i as usize).copied().unwrap_or(0)).max(),
            };
            match best {
                Some(b) => bound += b as i64,
                None => return,
            }
        }
        self.seq += 1;
        self.heap.push(BfItem { key: (-class, bound, self.seq), hc });
    }
}

struct BfItem {
    key: (i32, i64, u64),
    hc: Hc,
}
impl PartialEq for BfItem {
    fn eq(&self, o: &Self) -> bool {
        self.key == o.key
    }
}
impl Eq for BfItem {}
impl PartialOrd for BfItem {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for BfItem {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.key.cmp(&o.key)
    }
}

impl Default for BestFirstCursor {
    fn default() -> Self {
        BestFirstCursor {
            heap: BfHeap {
                heap: std::collections::BinaryHeap::new(),
                order: HcOrdering::Precomputed(AxisOrders::default()),
                scores: Vec::new(),
                universe: Hc::default(),
                seq: 0,
                branch_pending: false,
                first_branch: 0,
            },
            sc: PointScratch::default(),
            checks: CheckBuf::default(),
        }
    }
}

impl BestFirstCursor {
    /// The per-axis orders (each listing its axis' universe coordinates, best first) and
    /// coordinate scores, to fill before `restart` (their storage is kept between uses).
    pub fn ordering_mut(&mut self) -> (&mut AxisOrders, &mut Vec<Vec<i32>>) {
        (self.heap.order.axes_mut(), &mut self.heap.scores)
    }

    /// Whether the branch axes' order is still to come (`next_with` asks for it when a
    /// hypercuboid first needs it); until then only their null coordinate is scored.
    pub fn set_branch_pending(&mut self, pending: bool) {
        self.heap.branch_pending = pending;
    }

    /// Enumerate `ss` (of the context `info`) from the start under the current ordering.
    pub fn restart(&mut self, info: &HcInfo, ss: SearchSpace) {
        self.heap.heap.clear();
        self.heap.seq = 0;
        self.heap.universe.clone_from(info.get_universe());
        self.heap.first_branch = info.first_branch_axis();
        let n = ss.hcs.len();
        for (k, hc) in ss.hcs.into_iter().enumerate() {
            self.heap.push_initial(hc, (n - 1 - k) as i32);
        }
    }

    pub fn next(&mut self, info: &HcInfo, s: &State) -> Option<MoveSeq> {
        self.next_with(info, s, &mut |_, _| {})
    }

    /// `next`, ordering pending branch axes with `fill` once needed.
    pub fn next_with(&mut self, info: &HcInfo, s: &State, fill: &mut BranchFill) -> Option<MoveSeq> {
        while let Some(BfItem { key, mut hc }) = self.heap.heap.pop() {
            let class = -key.0;
            self.heap.need_branch(&hc, true, fill);
            if !info.take_point(&mut hc, &self.heap.order, &mut self.sc) {
                continue;
            }
            let pt = &self.sc.point;
            let heap = &mut self.heap;
            match info.find_problem(s, pt, &hc, &mut self.checks) {
                None => {
                    let act = info.to_action(pt);
                    hc.remove_point_with(pt, |h| heap.push(h, class, fill));
                    return Some(act);
                }
                Some(problem) => {
                    let _p = crate::misc::prof::scope(5);
                    hc.remove_slice_with(&problem, |h| heap.push(h, class, fill));
                }
            }
        }
        None
    }
}

/// Lazy enumeration of legal actions over a borrowed state.
pub struct HcSearch<'a> {
    info: &'a HcInfo,
    s: &'a State,
    cursor: HcCursor,
}

impl Iterator for HcSearch<'_> {
    type Item = MoveSeq;
    fn next(&mut self) -> Option<MoveSeq> {
        self.cursor.next(self.info, self.s)
    }
}
