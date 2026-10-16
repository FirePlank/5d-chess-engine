//! Time management: how long to think about one action, from a fixed movetime or a clock
//! (`go wtime/btime/winc/binc`).

use super::Limits;

/// Charged per action for process and protocol latency (ms).
const MOVE_OVERHEAD: u64 = 20;
/// Own actions expected to remain. 5D games are short: self-play games end after a median of
/// 38 actions (19 per side) and only one in ten lasts past 34 per side.
const MOVES_TO_GO: u64 = 24;
/// Below this many increments on the clock, an action gets most of one increment.
const LOW_CLOCK_INCS: u64 = 6;

pub struct TimePlan {
    /// Time the action should take (ms); a clock plan scales it by the search's stability.
    pub optimum: u64,
    /// The search is aborted here (ms).
    pub maximum: u64,
    /// From a clock rather than a fixed movetime.
    pub clock: bool,
}

impl TimePlan {
    /// `boards`: boards the side to move must play this action.
    pub fn new(limits: &Limits, boards: usize) -> Option<TimePlan> {
        if let Some(mt) = limits.movetime {
            return Some(TimePlan { optimum: mt, maximum: mt, clock: false });
        }
        let remaining = limits.time_left?;
        let inc = limits.increment.unwrap_or(0);
        let left = (remaining + inc * (MOVES_TO_GO - 1)).saturating_sub(MOVE_OVERHEAD * (MOVES_TO_GO + 2)).max(1);
        // An action on several boards is several moves' worth of decisions, with a branching
        // factor that multiplies per board.
        let scale = (1.0 + 0.25 * (boards.max(1) - 1) as f64).min(2.0);
        let mut optimum = (left as f64 * scale / MOVES_TO_GO as f64) as u64;
        let mut maximum = (optimum * 4).min((remaining / 3).saturating_sub(MOVE_OVERHEAD));
        if inc > 0 && remaining < inc * LOW_CLOCK_INCS {
            // Live on the increment instead of draining the last of the clock.
            let survive = (inc * 9 / 10).saturating_sub(MOVE_OVERHEAD).max(inc / 4);
            optimum = optimum.min(survive);
            maximum = maximum.min(survive);
        }
        let maximum = maximum.max(5);
        Some(TimePlan { optimum: optimum.clamp(5, maximum), maximum, clock: true })
    }

    /// Time (ms) after which a clock search should stop: the optimum, longer while the score
    /// is falling (`falling` > 1) or the best action keeps changing (`instability` > 1).
    pub fn target(&self, falling: f64, instability: f64) -> f64 {
        (self.optimum as f64 * falling * instability).min(self.maximum as f64)
    }
}
