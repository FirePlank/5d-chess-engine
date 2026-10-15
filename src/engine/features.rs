//! Position statistics, the 64-feature linear evaluation and the move evaluator used for
//! weighted rollouts and scored orderings.

use crate::core::action::special;
use crate::core::bitboard::*;
use crate::core::check_position::{BoardOverlay, CheckKind, CheckPosition};
use crate::core::hypercuboid::{has_physical_check, HcInfo};
use crate::core::multiverse::next_turn;
use crate::core::piece::*;
use crate::core::state::State;
use crate::misc::integer_set::Index;

pub const MATERIAL_COUNT: usize = 8;
pub const TIMELINE_COUNT: usize = 13;
pub const FEATURES: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lines {
    Mandatory,
    Optional,
    Unplayable,
}

fn lines_of(s: &State, which: Lines) -> Vec<i32> {
    let st = s.get_timeline_status();
    match which {
        Lines::Mandatory => st.mandatory,
        Lines::Optional => st.optional,
        Lines::Unplayable => st.unplayable,
    }
}

/// (sum, diff) material over the endpoint boards of the selected timelines. The diff is
/// taken from the perspective of each board's own color.
pub fn count_material(s: &State, which: Lines) -> ([i32; MATERIAL_COUNT], [i32; MATERIAL_COUNT]) {
    let mut sum = [0i32; MATERIAL_COUNT];
    let mut diff = [0i32; MATERIAL_COUNT];
    for l in lines_of(s, which) {
        let (t, c) = s.get_timeline_end(l);
        let b = s.board(l, t, c);
        let kinds = [b.lpawn(), b.lknight(), b.lrook(), b.lbishop(), b.lunicorn(), b.ldragon(), b.queen(), b.royal()];
        let friendly = if c { b.black() } else { b.white() };
        let hostile = if c { b.white() } else { b.black() };
        for k in 0..MATERIAL_COUNT {
            sum[k] += kinds[k].count_ones() as i32;
            diff[k] += (kinds[k] & friendly).count_ones() as i32 - (kinds[k] & hostile).count_ones() as i32;
        }
    }
    (sum, diff)
}

fn interval_size(a_min: i32, a_max: i32, b_min: i32, b_max: i32) -> i32 {
    0.max(a_max.min(b_max) - a_min.max(b_min) + 1)
}

pub fn count_timelines(s: &State) -> [i32; TIMELINE_COUNT] {
    let player = s.get_present().1;
    let (l_min, l_max) = s.get_lines_range();
    let (l0_min, l0_max) = s.get_initial_lines_range();
    let (amin, amax) = s.get_active_range();
    let st = s.get_timeline_status();
    let total = l_max - l_min + 1;
    let active = amax - amin + 1;
    let mandatory = st.mandatory.len() as i32;
    let optional = st.optional.len() as i32;
    let unplayable = st.unplayable.len() as i32;
    let white_created = l_max - l0_max;
    let black_created = l0_min - l_min;
    let friendly_created = if player { black_created } else { white_created };
    let hostile_created = if player { white_created } else { black_created };
    let adv = hostile_created - friendly_created;
    let white_active = interval_size(amin, amax, l0_max + 1, l_max);
    let black_active = interval_size(amin, amax, l_min, l0_min - 1);
    [
        total,
        active,
        total - active,
        mandatory,
        optional,
        unplayable,
        mandatory + optional,
        friendly_created,
        hostile_created,
        adv,
        0.max(adv + 1),
        if player { black_active } else { white_active },
        if player { white_active } else { black_active },
    ]
}

/// (log universe volume, log volume of non-branching axes).
pub fn count_move_space(s: &State) -> (f32, f32) {
    let (info, _) = HcInfo::build_hc(s);
    let (mut a, mut b) = (0f64, 0f64);
    for n in 0..info.dimension() {
        let x = (info.get_universe()[n].len() as f64).ln();
        a += x;
        if n < info.first_branch_axis() {
            b += x;
        }
    }
    (a as f32, b as f32)
}

pub mod off {
    use super::{MATERIAL_COUNT, TIMELINE_COUNT};
    pub const BIAS: usize = 0;
    pub const MAND_SUM: usize = 1;
    pub const MAND_DIFF: usize = MAND_SUM + MATERIAL_COUNT;
    pub const OPT_SUM: usize = MAND_DIFF + MATERIAL_COUNT;
    pub const OPT_DIFF: usize = OPT_SUM + MATERIAL_COUNT;
    pub const UNP_SUM: usize = OPT_DIFF + MATERIAL_COUNT;
    pub const UNP_DIFF: usize = UNP_SUM + MATERIAL_COUNT;
    pub const TIMELINE: usize = UNP_DIFF + MATERIAL_COUNT;
    pub const MOVE_SPACE: usize = TIMELINE + TIMELINE_COUNT;
    pub const TIMELINE_ADVANTAGE: usize = TIMELINE + 9;
}

pub type Weights = [f32; FEATURES];

pub fn default_weights() -> Weights {
    let mut w = [0f32; FEATURES];
    let m = [0.05f32, 0.15, 0.25, 0.15, 0.08, 0.05, 0.45, 0.30];
    for k in 0..MATERIAL_COUNT {
        w[off::MAND_DIFF + k] = m[k];
        w[off::OPT_DIFF + k] = 0.5 * m[k];
        w[off::UNP_DIFF + k] = 0.15 * m[k];
    }
    w[off::TIMELINE_ADVANTAGE] = 0.25;
    w[off::MOVE_SPACE] = 0.04;
    w[off::MOVE_SPACE + 1] = 0.04;
    w
}

pub fn trained_weights() -> Weights {
    [
        -0.00123027945, -0.00600768765, 0.00338658318, 0.00632669684, -0.00422760518, -0.00573036913, -0.00573036913,
        -0.00573036913, -0.0115035931, 0.000595524965, 0.0781893134, 0.0359656215, 0.175541759, -0.0855880827,
        -0.115588054, 0.284411639, 0.288696021, -0.00817218516, -0.00686811749, -0.0119211152, -0.0104583567,
        -0.0165845193, -0.0165845193, -0.0165845193, -0.00623198925, 0.0197074413, 0.0567287579, 0.137343943,
        0.0912609622, 0.046411965, 0.0314119607, 0.231411979, 0.171710014, 0.0163448583, 0.0168772414, 0.015377258,
        0.0141730411, 0.0118304137, 0.0118304137, 0.0118304137, 0.0140779829, 0.027899893, 0.0103068734,
        0.0221800804, 0.00934366416, -0.00264071906, -0.00714072073, 0.0528592542, 0.0637308881, -0.0115995659,
        -0.0115995659, 0.0, -0.0118690757, -0.00865917187, 0.0151735367, -0.0135565149, 0.0168984998, 0.064030014,
        0.222902104, 0.00271896203, 0.0168984998, 0.064030014, 0.0140336938, 0.0197210461,
    ]
}

pub fn extract_features(s: &State) -> [f32; FEATURES] {
    let mut f = [0f32; FEATURES];
    f[off::BIAS] = 1.0;
    for (which, so, dof) in [
        (Lines::Mandatory, off::MAND_SUM, off::MAND_DIFF),
        (Lines::Optional, off::OPT_SUM, off::OPT_DIFF),
        (Lines::Unplayable, off::UNP_SUM, off::UNP_DIFF),
    ] {
        let (sum, diff) = count_material(s, which);
        for k in 0..MATERIAL_COUNT {
            f[so + k] = sum[k] as f32;
            f[dof + k] = diff[k] as f32;
        }
    }
    let tl = count_timelines(s);
    for k in 0..TIMELINE_COUNT {
        f[off::TIMELINE + k] = tl[k] as f32;
    }
    let (a, b) = count_move_space(s);
    f[off::MOVE_SPACE] = a;
    f[off::MOVE_SPACE + 1] = b;
    f
}

/// Linear evaluation squashed by tanh, from White's perspective.
pub fn linear_evaluate(s: &State, w: &Weights) -> f32 {
    let _p = crate::misc::prof::scope(9);
    let f = extract_features(s);
    let mut acc = 0f32;
    for k in 0..FEATURES {
        acc += f[k] * w[k];
    }
    let score = acc.tanh();
    if s.get_present().1 { -score } else { score }
}

// ------------------------------------------------------------------ move features

pub fn has_latent_king_threat(result: &crate::core::board::Board, king_xy: i32, player: bool) -> bool {
    let enemy = if player { result.white() } else { result.black() };
    ((knight_jump1_attack(king_xy) & enemy & result.knight())
        | (rook_copy_mask(king_xy, 1) & enemy & result.lbishop())
        | (bishop_copy_mask(king_xy, 1) & enemy & result.lunicorn()))
        != 0
}

#[derive(Clone, Copy)]
pub struct MoveWeights(pub [f32; 12]);
pub mod mw {
    pub const LATENT_KING_THREAT: usize = 0;
    pub const NORMAL_KING_MOVE: usize = 1;
    pub const KING_BRANCH: usize = 2;
    pub const QUEEN_CAPTURE: usize = 3;
    pub const PRINCESS_CAPTURE: usize = 4;
    pub const DRAGON_UNICORN_CAPTURE: usize = 5;
    pub const BISHOP_KNIGHT_CAPTURE: usize = 6;
    pub const ROOK_COMMON_KING_CAPTURE: usize = 7;
    pub const BRAWN_CAPTURE: usize = 8;
    pub const PAWN_CAPTURE: usize = 9;
    pub const CHECK: usize = 10;
    pub const SUPERPHYSICAL: usize = 11;
}

impl MoveWeights {
    pub fn default_rollout() -> Self {
        let mut w = [0f32; 12];
        w[mw::LATENT_KING_THREAT] = -50000.0;
        w[mw::NORMAL_KING_MOVE] = -1200.0;
        w[mw::KING_BRANCH] = -8000.0;
        let k = 1.1f32;
        w[mw::QUEEN_CAPTURE] = 1440.0 * k;
        w[mw::PRINCESS_CAPTURE] = 800.0 * k;
        w[mw::DRAGON_UNICORN_CAPTURE] = 450.0 * k;
        w[mw::BISHOP_KNIGHT_CAPTURE] = 350.0 * k;
        w[mw::ROOK_COMMON_KING_CAPTURE] = 300.0 * k;
        w[mw::BRAWN_CAPTURE] = 120.0 * k;
        w[mw::PAWN_CAPTURE] = 100.0 * k;
        w[mw::CHECK] = 1200.0;
        w[mw::SUPERPHYSICAL] = -700.0;
        MoveWeights(w)
    }
    pub fn capture() -> Self {
        let mut w = [0f32; 12];
        for i in mw::QUEEN_CAPTURE..=mw::PAWN_CAPTURE {
            w[i] = 480.0;
        }
        MoveWeights(w)
    }
    pub fn capture_check() -> Self {
        let mut w = Self::capture();
        w.0[mw::CHECK] = 300.0;
        w
    }
}

/// Scores HC coordinates (physical and arriving moves); other coordinates score zero.
pub struct MoveEvaluator<'a> {
    info: &'a HcInfo,
    s: &'a State,
    w: MoveWeights,
    checks: Option<CheckPosition<'a>>,
    evaluate_captures: bool,
}

impl<'a> MoveEvaluator<'a> {
    pub fn new(info: &'a HcInfo, s: &'a State, w: MoveWeights) -> Self {
        let evaluate_captures = w.0[mw::QUEEN_CAPTURE..=mw::PAWN_CAPTURE].iter().any(|&x| x != 0.0);
        MoveEvaluator { info, s, w, checks: None, evaluate_captures }
    }

    pub fn is_physical_check(&self, axis: usize, coord: Index) -> bool {
        let Some(mb) = self.info.get_move_boards(axis, coord) else { return false };
        let defender = !self.s.get_present().1;
        has_physical_check(mb.result, defender) || mb.departure.map(|d| has_physical_check(d, defender)).unwrap_or(false)
    }

    fn check_kind(&mut self, kind: CheckKind, axis: usize, coord: Index) -> bool {
        let info: &'a HcInfo = self.info;
        let Some(mb) = info.get_move_boards(axis, coord) else { return false };
        let s: &'a State = self.s;
        if self.checks.is_none() {
            self.checks = Some(CheckPosition::for_move_candidates(s));
        }
        let checks = self.checks.as_mut().unwrap();
        let player = s.get_present().1;
        let mv = mb.mv;
        let arrival_line = mv.new_position(s).l();
        let at = next_turn((mv.to.t(), player));
        let arrival = BoardOverlay { l: arrival_line, t: at.0, c: at.1, value: mb.result };
        if mv.from.tl() == mv.to.tl() {
            return checks.gives_check(kind, arrival, player, None);
        }
        let dt = next_turn((mv.from.t(), player));
        let dep = BoardOverlay { l: mv.from.l(), t: dt.0, c: dt.1, value: mb.departure.unwrap() };
        checks.gives_check(kind, arrival, player, Some(dep))
    }

    /// Checks given by the moved piece itself from its arrival square (superphysically), or by
    /// anything on its result board (physically). Misses discovered superphysical checks.
    pub fn is_check_fast(&mut self, axis: usize, coord: Index) -> bool {
        if self.is_physical_check(axis, coord) {
            return true;
        }
        let info: &'a HcInfo = self.info;
        let Some(mb) = info.get_move_boards(axis, coord) else { return false };
        let s: &'a State = self.s;
        if self.checks.is_none() {
            self.checks = Some(CheckPosition::for_move_candidates(s));
        }
        let checks = self.checks.as_mut().unwrap();
        let player = s.get_present().1;
        let mv = mb.mv;
        let arrival_line = mv.new_position(s).l();
        let at = next_turn((mv.to.t(), player));
        let arrival = BoardOverlay { l: arrival_line, t: at.0, c: at.1, value: mb.result };
        let mask = pmask(mv.to.xy());
        if mv.from.tl() == mv.to.tl() {
            return checks.gives_check_masked(CheckKind::Superphysical, arrival, player, None, mask);
        }
        let dt = next_turn((mv.from.t(), player));
        let dep = BoardOverlay { l: mv.from.l(), t: dt.0, c: dt.1, value: mb.departure.unwrap() };
        checks.gives_check_masked(CheckKind::Superphysical, arrival, player, Some(dep), mask)
    }

    pub fn is_check(&mut self, axis: usize, coord: Index) -> bool {
        self.is_physical_check(axis, coord) || self.check_kind(CheckKind::Superphysical, axis, coord)
    }

    pub fn score(&mut self, axis: usize, coord: Index) -> f32 {
        let info = self.info;
        let Some(mb) = info.get_move_boards(axis, coord) else { return 0.0 };
        let s = self.s;
        let w = self.w.0;
        let mv = mb.mv;
        let mut result = 0f32;
        let mut king_move = false;
        if w[mw::LATENT_KING_THREAT] != 0.0 || w[mw::NORMAL_KING_MOVE] != 0.0 || w[mw::KING_BRANCH] != 0.0 {
            let moved = to_white(piece_name(mv.moved_piece(s)));
            king_move = moved == KING_W || moved == COMMON_KING_W;
        }
        if king_move && (w[mw::LATENT_KING_THREAT] != 0.0 || w[mw::NORMAL_KING_MOVE] != 0.0) {
            let threat = to_white(mv.moved_piece(s)) == KING_W
                && has_latent_king_threat(mb.result, mv.to.xy(), s.get_present().1);
            result += if threat { w[mw::LATENT_KING_THREAT] } else { w[mw::NORMAL_KING_MOVE] };
        }
        let mut mt = special::NONE;
        if (king_move && w[mw::KING_BRANCH] != 0.0) || w[mw::SUPERPHYSICAL] != 0.0 {
            mt = mv.move_type(s);
        }
        if king_move && w[mw::KING_BRANCH] != 0.0 && mt & special::BRANCHING != 0 {
            result += w[mw::KING_BRANCH];
        }
        if self.evaluate_captures {
            result += match to_white(piece_name(mv.captured_piece(s))) {
                QUEEN_W | ROYAL_QUEEN_W => w[mw::QUEEN_CAPTURE],
                PRINCESS_W => w[mw::PRINCESS_CAPTURE],
                DRAGON_W | UNICORN_W => w[mw::DRAGON_UNICORN_CAPTURE],
                BISHOP_W | KNIGHT_W => w[mw::BISHOP_KNIGHT_CAPTURE],
                ROOK_W | COMMON_KING_W => w[mw::ROOK_COMMON_KING_CAPTURE],
                BRAWN_W => w[mw::BRAWN_CAPTURE],
                PAWN_W => w[mw::PAWN_CAPTURE],
                _ => 0.0,
            };
        }
        if w[mw::CHECK] != 0.0 && self.is_check(axis, coord) {
            result += w[mw::CHECK];
        }
        if w[mw::SUPERPHYSICAL] != 0.0 && mt & special::SUPERPHYSICAL != 0 {
            result += w[mw::SUPERPHYSICAL];
        }
        result
    }

    /// table[axis][coordinate] over the universe (gaps are zero).
    pub fn score_table(&mut self) -> Vec<Vec<f32>> {
        let info = self.info;
        let u = info.get_universe();
        let mut out = Vec::with_capacity(u.dimension());
        for axis in 0..u.dimension() {
            let mut v = vec![0f32; u[axis].max().map(|m| m as usize + 1).unwrap_or(0)];
            for c in u[axis].iter() {
                v[c as usize] = self.score(axis, c);
            }
            out.push(v);
        }
        out
    }
}
