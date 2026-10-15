pub mod ab;
pub mod evalnet;
pub mod hce;
pub mod uci;

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
}
