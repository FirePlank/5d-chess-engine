pub mod ab;
pub mod book;
pub mod chess;
pub mod chess_tab;
pub mod evalnet;
pub mod hce;
pub mod local;
pub mod sac;
mod skill;
pub mod time;
pub mod uci;

/// Reported scores. Internally a pawn is about 142 and a queen 1250, and an evaluation `s`
/// stands for an expected score of about 1 / (1 + exp(-WDL_K * s)): the logistic fitted to game
/// results for search scores (refitted 2026-10-17 on the SPRT games of the current nets:
/// 1/4348..1/4649, after the 1/2700 of the first net). `info ... score cp X` reports
/// X = s * 100 / REPORT_SCALE, so that +100 is about a 75% expected score, +200 about 90% and
/// +400 about 99%.
pub const REPORT_SCALE: i32 = 4800;
/// Slope of the win-probability logistic of search scores, per internal unit.
pub const WDL_K: f64 = 1.0 / 4350.0;

/// Internal score to the reported (normalized) centipawn scale.
pub fn report_cp(internal: i32) -> i32 {
    (internal as f64 * 100.0 / REPORT_SCALE as f64).round() as i32
}

/// Reported (normalized) centipawns back to internal units.
pub fn internal_cp(reported: i32) -> i32 {
    (reported as f64 * REPORT_SCALE as f64 / 100.0).round() as i32
}

/// Win/draw/loss per mille for an internal (non-mate) score, from the evaluation's logistic
/// (no draw model: draws are 0).
pub fn wdl(internal: i32) -> (u32, u32, u32) {
    let p = 1.0 / (1.0 + (-WDL_K * internal as f64).exp());
    let w = (p * 1000.0).round() as u32;
    (w, 0, 1000 - w)
}

#[derive(Clone, Debug, PartialEq)]
pub enum OptionValue {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl OptionValue {
    pub fn parse(v: &str) -> OptionValue {
        if v == "true" {
            return OptionValue::Bool(true);
        }
        if v == "false" {
            return OptionValue::Bool(false);
        }
        if let Ok(i) = v.parse::<i64>() {
            return OptionValue::Int(i);
        }
        if let Ok(f) = v.parse::<f64>() {
            return OptionValue::Float(f);
        }
        OptionValue::Str(v.to_string())
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            OptionValue::Int(i) => Some(*i as f64),
            OptionValue::Float(f) => Some(*f),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Limits {
    /// Search depth in turns.
    pub depth: Option<u32>,
    pub movetime: Option<u64>,
    pub nodes: Option<u64>,
    /// Remaining clock (ms) and increment for the side to move.
    pub time_left: Option<u64>,
    pub increment: Option<u64>,
    /// Simple delay (ms) before the clock runs each turn: a fixed part, and a part per active
    /// timeline of the position (the 5D tournament format: 3 s per active timeline). Unused
    /// delay is lost.
    pub delay: Option<u64>,
    pub delay_per_timeline: Option<u64>,
    /// Analysis: if the last search was of this same position, continue its iterative
    /// deepening (at the depth it completed + 1, with its root order and best action) instead
    /// of starting again from depth 1.
    pub resume: bool,
}
