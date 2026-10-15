//! Hypercuboid legal-action search. An axis is a playable timeline or a prospective new timeline; a point picks one
//! semimove per axis; invalid regions are cut out as slices.

use super::action::{FullMove, MoveSeq};
use super::bitboard::*;
use super::board::Board;
use super::check_position::CheckPosition;
use super::multiverse::next_turn;
use super::piece::*;
use super::state::State;
use super::vec4::Vec4;
use crate::misc::geometry::{Hc, Point, SearchSpace, Slice};
use crate::misc::graph::Graph;
use crate::misc::integer_set::{Index, IntegerSet};
use crate::misc::ordering::HcOrdering;
use std::collections::BTreeMap;

const INVALID: Index = Index::MAX;

#[derive(Clone, Debug)]
pub enum Entry {
    Physical { m: FullMove, b: Board },
    /// `idx` is the paired departure coordinate on axis `fa`.
    Arriving { m: FullMove, b: Board, idx: Index, fa: u32 },
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
/// pass back in where needed), so a search can run while the state is made/unmade.
pub struct HcInfo {
    player: bool,
    line_axes: BTreeMap<i32, usize>,
    /// Entries of the existing playable timelines' axes.
    axis_coords: Vec<Vec<Entry>>,
    /// Entries shared by every prospective new-timeline axis.
    branch_entries: Vec<Entry>,
    dim: usize,
    universe: Hc,
    new_axis: usize,
    /// Coordinate groups per axis (index `new_axis` is shared by all branch axes).
    groups: Vec<AxisGroups>,
}

#[derive(Default)]
struct AxisGroups {
    nonjump: IntegerSet,
    /// (departure axis, departure coordinate, arrivals depending on it)
    by_dep: Vec<(u32, Index, IntegerSet)>,
    /// (departure axis, all arrivals from it)
    by_from: Vec<(u32, IntegerSet)>,
}

impl AxisGroups {
    fn build<'a>(entries: impl Iterator<Item = (Index, &'a Entry)>) -> Self {
        let mut g = AxisGroups::default();
        for (i, e) in entries {
            match e {
                Entry::Physical { .. } | Entry::Null => g.nonjump.insert(i),
                Entry::Arriving { idx, fa, .. } if *idx != INVALID => {
                    match g.by_dep.iter_mut().find(|d| d.0 == *fa && d.1 == *idx) {
                        Some(d) => d.2.insert(i),
                        None => g.by_dep.push((*fa, *idx, IntegerSet::singleton(i))),
                    }
                    match g.by_from.iter_mut().find(|d| d.0 == *fa) {
                        Some(d) => d.1.insert(i),
                        None => g.by_from.push((*fa, IntegerSet::singleton(i))),
                    }
                }
                _ => {}
            }
        }
        g
    }
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
    pub fn line_to_axis(&self, l: i32) -> Option<usize> {
        self.line_axes.get(&l).copied()
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
            Entry::Physical { m, b } => Some(MoveBoards { mv: *m, result: b, departure: None }),
            Entry::Arriving { m, b, idx, .. } => {
                let src = self.line_axes[&m.from.l()];
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
        let _p = crate::misc::prof::scope(0);
        let st_scope = crate::misc::prof::scope(15);
        let ts = s.get_timeline_status();
        let playable = ts.playable();
        let (_present_t, player) = s.get_present();
        let mut line_axes: BTreeMap<i32, usize> = BTreeMap::new();
        let (lines_lo, lines_hi) = s.get_lines_range();
        // Axis of each existing timeline (playable ones), indexed by l - lines_lo.
        let mut axis_of = vec![usize::MAX; (lines_hi - lines_lo + 1) as usize];
        for (axis, &l) in playable.iter().enumerate() {
            line_axes.insert(l, axis);
            axis_of[(l - lines_lo) as usize] = axis;
        }
        let axis_of_line = |l: i32| axis_of[(l - lines_lo) as usize];
        let mut stays: Vec<Vec<FullMove>> = vec![Vec::new(); playable.len()];
        let mut departures: Vec<Vec<Vec4>> = vec![Vec::new(); playable.len()];
        let mut arrivals: Vec<(i32, Vec<FullMove>)> = Vec::new();
        drop(st_scope);
        let mut tos: Vec<Vec4> = Vec::new();
        let mut scratch = super::multiverse::MoveList::new();
        let gen_scope = crate::misc::prof::scope(1);
        for from in s.get_all_pieces(&playable) {
            let source_axis = axis_of_line(from.l());
            let mut has_depart = false;
            tos.clear();
            s.multiverse().gen_piece_move_with(from, player, &mut tos, &mut scratch);
            for &to in &tos {
                let m = FullMove::new(from, to);
                if from.tl() != to.tl() {
                    if !has_depart {
                        departures[source_axis].push(from);
                        has_depart = true;
                    }
                    let line = to.l();
                    match arrivals.iter_mut().find(|b| b.0 == line) {
                        Some(b) => b.1.push(m),
                        None => arrivals.push((line, vec![m])),
                    }
                } else {
                    stays[source_axis].push(m);
                }
            }
        }
        drop(gen_scope);
        arrivals.sort_by_key(|b| b.0);
        let max_branch = departures.iter().filter(|d| !d.is_empty()).count();
        // Departure coordinate of each source square, per axis.
        let mut jump_tab: Vec<[Index; 64]> = vec![[INVALID; 64]; playable.len()];
        let find_jump = |jt: &Vec<[Index; 64]>, from: Vec4| {
            let i = jt[axis_of_line(from.l())][from.xy() as usize];
            if i == INVALID { None } else { Some(i) }
        };
        let (_, size_y) = s.get_board_size();
        let mut axis_coords: Vec<Vec<Entry>> = Vec::with_capacity(playable.len() + max_branch);

        let promo_piece = |m: FullMove| -> Piece {
            s.normalize_promotion(super::action::ExtMove::new(m, NO_PIECE)).map(|e| e.promote_to).unwrap_or(NO_PIECE)
        };

        // Royal safety per board: only royal moves, pinned pieces, en passant or a board
        // already in check can leave the mover's royal attacked, so most results skip the
        // full check test.
        let mut safety_cache: Vec<(i32, i32, super::board::RoyalSafety)> = Vec::new();
        let mut safety = |l: i32, t: i32| -> super::board::RoyalSafety {
            if let Some(e) = safety_cache.iter().find(|e| e.0 == l && e.1 == t) {
                return e.2;
            }
            let r = s.board(l, t, player).royal_safety(player);
            safety_cache.push((l, t, r));
            r
        };
        let ent_scope = crate::misc::prof::scope(12);
        for axis in 0..playable.len() {
            let l = playable[axis];
            let arriving = arrivals.iter().find(|b| b.0 == l).map(|b| &b.1);
            let mut locs: Vec<Entry> = Vec::with_capacity(1 + stays[axis].len() + departures[axis].len());
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
                    locs.push(Entry::Physical { m, b: nb });
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
            if let Some(arr) = arriving {
                for &m in arr {
                    let (last_t, last_c) = s.get_timeline_end(m.to.l());
                    if m.to.t() == last_t && player == last_c {
                        let (p, q) = (m.from, m.to);
                        let src = s.board(p.l(), p.t(), player);
                        let mut nb = *s.board(q.l(), q.t(), player);
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
                            let fa = axis_of_line(m.from.l()) as u32;
                            locs.push(Entry::Arriving { m, b: nb, idx: INVALID, fa });
                        }
                    }
                }
            }
            axis_coords.push(locs);
        }

        drop(ent_scope);
        let br_scope = crate::misc::prof::scope(13);
        let new_axis = axis_coords.len();
        let mut locs: Vec<Entry> = vec![Entry::Null];
        for (_, bucket) in &arrivals {
            for &m in bucket {
                let (p, q) = (m.from, m.to);
                if let Some(dep) = find_jump(&jump_tab, m.from) {
                    let src = s.board(p.l(), p.t(), player);
                    let mut nb = *s.board(q.l(), q.t(), player);
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
                        let fa = axis_of_line(m.from.l()) as u32;
                        locs.push(Entry::Arriving { m, b: nb, idx: dep, fa });
                    }
                }
            }
        }
        drop(br_scope);
        let _fin = crate::misc::prof::scope(14);
        let new_l = s.new_line();
        let sign = signum(new_l);
        for i in 0..max_branch {
            line_axes.insert(new_l + sign * i as i32, new_axis + i);
        }
        let dimension = new_axis + max_branch;
        let branch_range = IntegerSet::range(locs.len() as Index);
        let mut universe = Hc::new(
            (0..dimension)
                .map(|n| if n < new_axis { IntegerSet::range(axis_coords[n].len() as Index) } else { branch_range.clone() })
                .collect(),
        );
        // Branch arrivals were only admitted with a live departure; existing-axis arrivals
        // lose their coordinate when their departure was filtered out.
        for n in 0..new_axis {
            for i in 0..axis_coords[n].len() {
                if let Entry::Arriving { m, idx, .. } = &mut axis_coords[n][i] {
                    match find_jump(&jump_tab, m.from) {
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
            universe[0] = IntegerSet::new();
        }
        let mut hc_n_lines = universe.clone();
        let mut non_null = IntegerSet::new();
        if new_axis < dimension {
            for i in 1..locs.len() {
                non_null.insert(i as Index);
            }
            for n in new_axis..dimension {
                hc_n_lines[n] = IntegerSet::singleton(0);
            }
        }
        let mut ss = SearchSpace::from_vec(vec![hc_n_lines.clone()]);
        for n in new_axis..dimension {
            hc_n_lines[n] = non_null.clone();
            ss.push_front(hc_n_lines.clone());
        }
        let mut groups: Vec<AxisGroups> = axis_coords
            .iter()
            .map(|v| AxisGroups::build(v.iter().enumerate().map(|(i, e)| (i as Index, e))))
            .collect();
        groups.push(AxisGroups::build(locs.iter().enumerate().map(|(i, e)| (i as Index, e))));
        (
            HcInfo {
                groups,
                player,
                line_axes,
                axis_coords,
                branch_entries: locs,
                dim: dimension,
                universe,
                new_axis,
            },
            ss,
        )
    }

    /// Pick a point in `hc` whose departures and arrivals pair up. Prunes arrivals whose
    /// departure is no longer available.
    pub fn take_point(&self, hc: &mut Hc, order: &HcOrdering) -> Option<Point> {
        let _p = crate::misc::prof::scope(2);
        let dim = self.dimension();
        let mut g = Graph::new(dim);
        let mut must_include: Vec<Index> = Vec::new();
        let mut edge_refs = vec![INVALID; dim * dim];
        let mut result: Point = vec![INVALID; dim];
        let mut want = vec![false; dim];
        for n in 0..dim {
            let grp = &self.groups[n.min(self.new_axis)];
            // Arrivals whose departure left the hypercuboid can never be paired.
            let mut ghost: Option<IntegerSet> = None;
            for (fa, dep, set) in &grp.by_dep {
                if !hc[*fa as usize].contains(*dep) && set.intersects(&hc[n]) {
                    ghost.get_or_insert_with(IntegerSet::new).or_assign(set);
                }
            }
            if let Some(gs) = ghost {
                hc[n].minus(&gs);
            }
            if hc[n].is_empty() {
                return None;
            }
            let live = &hc[n];
            let need_nonjump = live.intersects(&grp.nonjump);
            let mut needed = need_nonjump as usize;
            for (fa, set) in &grp.by_from {
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
                HcOrdering::Precomputed(o) => {
                    for &i in &o[n] {
                        if live.contains(i) && visit(i) {
                            break;
                        }
                    }
                }
            }
            if !need_nonjump {
                must_include.push(n as Index);
            }
        }
        let matching = g.find_matching(&must_include)?;
        for (u, v) in matching {
            let (u, v) = (u as usize, v as usize);
            result[u] = edge_refs[u * dim + v];
            result[v] = edge_refs[v * dim + u];
        }
        debug_assert!(result.iter().all(|&i| i != INVALID));
        Some(result)
    }


    pub fn find_problem(&self, s: &State, p: &Point, hc: &Hc) -> Option<Slice> {
        let jp = crate::misc::prof::scope(3);
        let r = self.jump_order_consistent(s, p, hc).or_else(|| self.test_present(s, p, hc));
        drop(jp);
        if r.is_some() {
            return r;
        }
        let _p = crate::misc::prof::scope(4);
        self.find_checks(s, p, hc)
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
            if let Some(&m) = self.line_axes.get(&to.l()) {
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
            match self.line_axes.get(&l) {
                Some(&n) if !self.entry(n, p[n]).is_null() => v + 1,
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
        let w_axis = self.line_axes[&w_line];
        problem.fix_axis(w_axis, IntegerSet::singleton(0));
        for (&l, &n) in &self.line_axes {
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

    pub fn find_checks(&self, st: &State, p: &Point, hc: &Hc) -> Option<Slice> {
        let (_, c) = st.get_present();
        let (mut lo, mut hi) = st.get_lines_range();
        if let (Some((&first, _)), Some((&last, _))) = (self.line_axes.first_key_value(), self.line_axes.last_key_value()) {
            lo = lo.min(first);
            hi = hi.max(last);
        }
        let mut newstate = CheckPosition::with_range(st, lo, hi);
        for (&l, &axis) in &self.line_axes {
            let loc = self.entry(axis, p[axis]);
            if loc.is_null() {
                continue;
            }
            let (old_t, _) = loc.tl();
            newstate.add_board(l, next_turn((old_t, c)), loc.board());
        }
        let check = newstate.first_check(!c, false)?;
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
        if let Some(&n1) = self.line_axes.get(&check.from.l()) {
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
        if let Some(&n2) = self.line_axes.get(&l2) {
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
            if let Some(&n) = self.line_axes.get(&crossed.l()) {
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
        Some(problem)
    }

    pub fn to_action(&self, p: &Point) -> MoveSeq {
        let mut mvs = Vec::new();
        for (_, &i) in &self.line_axes {
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
}

impl HcCursor {
    pub fn new(ss: SearchSpace, order: HcOrdering) -> Self {
        HcCursor { ss, order }
    }

    pub fn next(&mut self, info: &HcInfo, s: &State) -> Option<MoveSeq> {
        while !self.ss.is_empty() {
            let mut hc = self.ss.pop_back().unwrap();
            let pt = match info.take_point(&mut hc, &self.order) {
                Some(p) => p,
                None => continue,
            };
            match info.find_problem(s, &pt, &hc) {
                None => {
                    let act = info.to_action(&pt);
                    self.ss.concat(hc.remove_point(&pt));
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
    heap: std::collections::BinaryHeap<BfItem>,
    order: HcOrdering,
    scores: Vec<Vec<i32>>,
    seq: u64,
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

impl BestFirstCursor {
    pub fn new(ss: SearchSpace, order: HcOrdering, scores: Vec<Vec<i32>>) -> Self {
        let mut c = BestFirstCursor { heap: std::collections::BinaryHeap::new(), order, scores, seq: 0 };
        let n = ss.hcs.len();
        for (k, hc) in ss.hcs.into_iter().enumerate() {
            c.push(hc, (n - 1 - k) as i32);
        }
        c
    }

    fn push(&mut self, hc: Hc, class: i32) {
        let _p = crate::misc::prof::scope(11);
        let mut bound = 0i64;
        for n in 0..hc.dimension() {
            let sc = &self.scores[n];
            let best = hc[n].iter().map(|i| sc.get(i as usize).copied().unwrap_or(0)).max();
            match best {
                Some(b) => bound += b as i64,
                None => return,
            }
        }
        self.seq += 1;
        self.heap.push(BfItem { key: (-class, bound, self.seq), hc });
    }

    pub fn next(&mut self, info: &HcInfo, s: &State) -> Option<MoveSeq> {
        while let Some(BfItem { key, mut hc }) = self.heap.pop() {
            let class = -key.0;
            let pt = match info.take_point(&mut hc, &self.order) {
                Some(p) => p,
                None => continue,
            };
            match info.find_problem(s, &pt, &hc) {
                None => {
                    let act = info.to_action(&pt);
                    for h in hc.remove_point(&pt).hcs {
                        self.push(h, class);
                    }
                    return Some(act);
                }
                Some(problem) => {
                    let _p = crate::misc::prof::scope(5);
                    for h in hc.remove_slice(&problem).hcs {
                        self.push(h, class);
                    }
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
