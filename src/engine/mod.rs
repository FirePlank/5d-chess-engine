pub mod ab;
pub mod eval;
pub mod features;
pub mod finetree;
pub mod hce;
pub mod mcts;
pub mod uci;

use crate::core::action::Action;
use crate::core::state::State;
use std::sync::atomic::AtomicBool;

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
    /// MCTS: iterations = 10 * depth. Alpha-beta: search depth in turns.
    pub depth: Option<u32>,
    pub movetime: Option<u64>,
    pub nodes: Option<u64>,
    /// Remaining clock (ms) and increment for the side to move.
    pub time_left: Option<u64>,
    pub increment: Option<u64>,
}

pub trait Engine: Send {
    fn name(&self) -> String;
    fn new_game(&mut self) {}
    /// Returns true when the option was recognized.
    fn set_option(&mut self, key: &str, value: &OptionValue) -> bool;
    fn find_best_move(&mut self, s: &State, limits: &Limits, stop: &AtomicBool, info: &mut dyn FnMut(String)) -> Option<Action>;
}
