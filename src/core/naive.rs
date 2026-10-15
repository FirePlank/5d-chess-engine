//! Exhaustive depth-first legal-action enumeration and an independent legality check.
//! Exponential in the number of playable timelines: for testing only.

use super::action::{FullMove, MoveSeq};
use super::state::State;
use super::vec4::Vec4;

/// Whether `mvs` is a legal action in `s`: every move pseudo-legal from a playable board,
/// the present moves, and afterwards no royal piece of the mover can be captured.
pub fn is_legal_action(s: &State, mvs: &[FullMove]) -> bool {
    let player = s.get_present().1;
    let mut c = s.clone();
    let mut used: Vec<i32> = Vec::new();
    for &m in mvs {
        let p = m.from;
        if c.get_board_ptr(p.l(), p.t(), player).is_none() || c.get_timeline_end(p.l()) != (p.t(), player) {
            return false;
        }
        if used.contains(&p.l()) {
            return false;
        }
        used.push(p.l());
        if !c.apply_move::<false>(m) {
            return false;
        }
    }
    if !c.submit::<false>() {
        return false;
    }
    c.first_check(c.get_present().1).is_none()
}

/// All legal actions, moves in canonical order (non-branching by timeline, then branching).
pub fn naive_actions(s: &State, limit: usize) -> Vec<MoveSeq> {
    naive_actions_budget(s, limit, u64::MAX).unwrap_or_default()
}

/// Like `naive_actions`, but gives up (None) after visiting `budget` partial actions.
pub fn naive_actions_budget(s: &State, limit: usize, budget: u64) -> Option<Vec<MoveSeq>> {
    let player = s.get_present().1;
    let (lmin, lmax) = s.get_lines_range();
    let mut out = Vec::new();
    let k = if player { lmax + 1 } else { lmin - 1 };
    let mut nodes = budget;
    rec(s, &mut MoveSeq::new(), k, false, player, &mut out, limit, &mut nodes);
    if nodes == 0 { None } else { Some(out) }
}

#[allow(clippy::too_many_arguments)]
fn rec(s: &State, mvs: &mut MoveSeq, k: i32, b: bool, c: bool, out: &mut Vec<MoveSeq>, limit: usize, nodes: &mut u64) {
    if out.len() >= limit || *nodes == 0 {
        return;
    }
    *nodes -= 1;
    if s.first_check(!c).is_some() {
        return;
    }
    if s.can_submit().is_some() {
        out.push(mvs.clone());
    }
    let mut tos: Vec<Vec4> = Vec::new();
    for p in s.get_movable_pieces() {
        tos.clear();
        s.gen_piece_move(p, &mut tos);
        for &q in &tos {
            let branching = (q.t(), c) < s.get_timeline_end(q.l());
            if !branching && (b || if c { q.l() > k } else { q.l() < k }) {
                continue;
            }
            let Some(t) = s.can_apply(FullMove::new(p, q), 0) else { continue };
            mvs.push(FullMove::new(p, q));
            rec(&t, mvs, if branching { k } else { q.l() }, branching, c, out, limit, nodes);
            mvs.pop();
        }
    }
}
