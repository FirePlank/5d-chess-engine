//! Time management: how long to think about one action, from a fixed movetime or a clock
//! (`go wtime/btime/winc/binc`, with an optional delay: `wdelay/bdelay`, `wtldelay/btldelay`).
//!
//! A delay clock (simple / US delay; the 5D tournament clock is 5 min + 3 s per active
//! timeline) waits `delay + delay_per_timeline * active timelines` before the base time starts
//! running; unused delay is lost. The delay is therefore free time: every action uses nearly
//! all of it (`TimePlan::free`), and the base time is spent on top of it.

use super::Limits;

/// Charged per action for process and protocol latency (ms).
const MOVE_OVERHEAD: u64 = 20;
/// Own actions expected to remain. 5D games are short: self-play games end after a median of
/// 38 actions (19 per side) and only one in ten lasts past 34 per side.
const MOVES_TO_GO: u64 = 24;
/// Own actions the base time is spread over on a delay clock: every later action also gets
/// its delay, so running the base low costs much less than on a plain clock.
const DELAY_MOVES_TO_GO: u64 = 16;
/// Part of the delay kept back for latency (1/25 = 4%, at least `MOVE_OVERHEAD`).
const DELAY_SAFETY_DIV: u64 = 25;
/// Below this many increments on the clock, an action gets most of one increment.
const LOW_CLOCK_INCS: u64 = 6;

#[derive(Debug)]
pub struct TimePlan {
    /// Time the action should take (ms); a clock plan scales it by the search's stability.
    pub optimum: u64,
    /// The search is aborted here (ms).
    pub maximum: u64,
    /// From a clock rather than a fixed movetime.
    pub clock: bool,
    /// Delay clocks: time (ms) that costs nothing off the base, the delay less a safety
    /// margin. A clock search never stops before it (bar a forced action or a mate).
    pub free: u64,
}

impl TimePlan {
    /// `boards`: boards the side to move must play this action. `timelines`: active
    /// timelines at the root (for `Limits::delay_per_timeline`).
    pub fn new(limits: &Limits, boards: usize, timelines: usize) -> Option<TimePlan> {
        if let Some(mt) = limits.movetime {
            return Some(TimePlan { optimum: mt, maximum: mt, clock: false, free: 0 });
        }
        let remaining = limits.time_left?;
        let inc = limits.increment.unwrap_or(0);
        // An action on several boards is several moves' worth of decisions, with a branching
        // factor that multiplies per board.
        let scale = (1.0 + 0.15 * (boards.max(1) - 1) as f64).min(1.6);
        let delay = limits.delay.unwrap_or(0) + limits.delay_per_timeline.unwrap_or(0) * timelines as u64;
        if delay > 0 {
            return Some(Self::with_delay(remaining, inc, delay, scale));
        }
        let left = (remaining + inc * (MOVES_TO_GO - 1)).saturating_sub(MOVE_OVERHEAD * (MOVES_TO_GO + 2)).max(1);
        let mut optimum = (left as f64 * scale / MOVES_TO_GO as f64) as u64;
        let mut maximum = (optimum * 4).min((remaining / 3).saturating_sub(MOVE_OVERHEAD));
        if inc > 0 && remaining < inc * LOW_CLOCK_INCS {
            // Live on the increment instead of draining the last of the clock.
            let survive = (inc * 9 / 10).saturating_sub(MOVE_OVERHEAD).max(inc / 4);
            optimum = optimum.min(survive);
            maximum = maximum.min(survive);
        }
        let maximum = maximum.max(5);
        Some(TimePlan { optimum: optimum.clamp(5, maximum), maximum, clock: true, free: 0 })
    }

    /// A delay clock: the delay (less a safety margin) is free, and the base time is shared
    /// out on top of it as on a plain clock, a little more freely since every later action
    /// has its delay too. The latency is covered by the safety margin, so it is not charged
    /// to the base per action. The abort time never passes `delay + remaining - safety`.
    fn with_delay(remaining: u64, inc: u64, delay: u64, scale: f64) -> TimePlan {
        let safety = MOVE_OVERHEAD.max(delay / DELAY_SAFETY_DIV);
        let free = delay.saturating_sub(safety);
        let left = (remaining + inc * (DELAY_MOVES_TO_GO - 1)).saturating_sub(MOVE_OVERHEAD * 2);
        let mut base_opt = (left as f64 * scale / DELAY_MOVES_TO_GO as f64) as u64;
        let mut base_max = (base_opt * 4).min(remaining / 3);
        if inc > 0 && remaining < inc * LOW_CLOCK_INCS {
            let survive = inc * 9 / 10;
            base_opt = base_opt.min(survive);
            base_max = base_max.min(survive);
        }
        let cap = (delay + remaining).saturating_sub(safety);
        let maximum = (free + base_max).min(cap).max(5);
        let optimum = (free + base_opt).clamp(5, maximum);
        TimePlan { optimum, maximum, clock: true, free: free.min(maximum) }
    }

    /// Time (ms) after which a clock search should stop: the optimum, longer while the score
    /// is falling (`falling` > 1) or the best action keeps changing (`instability` > 1), and
    /// never before the free time of a delay clock.
    pub fn target(&self, falling: f64, instability: f64) -> f64 {
        (self.optimum as f64 * falling * instability).max(self.free as f64).min(self.maximum as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delay_clock(left: u64, delay: u64, per_tl: u64) -> Limits {
        Limits { time_left: Some(left), delay: Some(delay), delay_per_timeline: Some(per_tl), ..Default::default() }
    }

    #[test]
    fn tiny_base_uses_the_delay() {
        // 1 s on the clock and a 3 s delay (one timeline): ~95% of the delay is used.
        let p = TimePlan::new(&delay_clock(1000, 0, 3000), 1, 1).unwrap();
        assert!(p.free >= 2850 && p.free < 3000, "{:?}", p);
        assert!(p.target(0.6, 0.6) >= 2850.0);
        assert!(p.maximum <= 3000 + 1000 - 120, "{:?}", p);
        // Nothing left on the clock at all.
        let p = TimePlan::new(&delay_clock(0, 3000, 0), 1, 1).unwrap();
        assert!(p.free >= 2850 && p.maximum < 3000, "{:?}", p);
    }

    #[test]
    fn timelines_scale_the_delay() {
        let one = TimePlan::new(&delay_clock(300_000, 0, 3000), 1, 1).unwrap();
        let ten = TimePlan::new(&delay_clock(300_000, 0, 3000), 1, 10).unwrap();
        assert!(ten.free >= 28_500 && ten.free < 30_000, "{:?}", ten);
        assert!(ten.optimum >= ten.free + (one.optimum - one.free), "{:?} {:?}", one, ten);
        assert!(ten.target(0.6, 0.6) >= 28_500.0);
        // The base is spent on top of the delay.
        assert!(one.optimum > one.free + 10_000, "{:?}", one);
    }

    #[test]
    fn hard_cap_within_delay_and_base() {
        for left in [0, 50, 500, 2000, 10_000, 60_000, 300_000] {
            for (d, t, tl) in [(0, 3000, 1), (0, 3000, 10), (1000, 0, 1), (500, 500, 3), (10, 0, 1)] {
                for boards in [1, 4] {
                    let p = TimePlan::new(&delay_clock(left, d, t), boards, tl).unwrap();
                    let delay = d + t * tl as u64;
                    assert!(p.maximum <= (delay + left).max(5), "{} {} {:?}", left, delay, p);
                    assert!(delay < 100 || p.maximum + delay / 25 <= delay + left, "{} {} {:?}", left, delay, p);
                    assert!(p.free <= p.optimum && p.optimum <= p.maximum, "{:?}", p);
                    assert!(p.target(2.0, 2.0) <= p.maximum as f64);
                }
            }
        }
    }

    #[test]
    fn no_delay_is_unchanged() {
        let l = Limits { time_left: Some(10_000), increment: Some(100), ..Default::default() };
        let p = TimePlan::new(&l, 1, 5).unwrap();
        assert_eq!(p.free, 0);
        let left = 10_000 + 100 * 23 - 20 * 26;
        assert_eq!(p.optimum, left / 24);
        let l = Limits { time_left: Some(10_000), delay: Some(0), delay_per_timeline: Some(0), ..Default::default() };
        assert_eq!(TimePlan::new(&l, 1, 5).unwrap().free, 0);
    }
}
