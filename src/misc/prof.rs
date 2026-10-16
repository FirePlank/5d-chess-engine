//! Feature-gated wall-clock section counters (`--features prof`); free when disabled.

pub const NAMES: [&str; 20] = [
    "build_hc", "hc_movegen", "take_point", "jump+present", "find_checks", "space_ops",
    "ordering", "state_clone", "qsearch", "eval", "in_check", "tt_validate",
    "hc_entries", "hc_branch", "hc_finish", "hc_status", "local_captures", "null_move",
    "hc_physical", "hc_jumps",
];

#[cfg(feature = "prof")]
pub static COUNTERS: [std::sync::atomic::AtomicU64; 20] = [const { std::sync::atomic::AtomicU64::new(0) }; 20];
#[cfg(feature = "prof")]
pub static CALLS: [std::sync::atomic::AtomicU64; 20] = [const { std::sync::atomic::AtomicU64::new(0) }; 20];

#[cfg(feature = "prof")]
pub struct Scope(usize, std::time::Instant);
#[cfg(feature = "prof")]
impl Drop for Scope {
    fn drop(&mut self) {
        COUNTERS[self.0].fetch_add(self.1.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
        CALLS[self.0].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}
#[cfg(feature = "prof")]
#[inline(always)]
pub fn scope(i: usize) -> Scope {
    Scope(i, std::time::Instant::now())
}
#[cfg(not(feature = "prof"))]
#[inline(always)]
pub fn scope(_: usize) {}

/// Event counters (`count`), reported by index when non-zero.
#[cfg(feature = "prof")]
pub static EVENTS: [std::sync::atomic::AtomicU64; 32] = [const { std::sync::atomic::AtomicU64::new(0) }; 32];
#[cfg(feature = "prof")]
#[inline(always)]
pub fn count(i: usize) {
    EVENTS[i].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}
#[cfg(not(feature = "prof"))]
#[inline(always)]
pub fn count(_: usize) {}

pub fn report() -> String {
    #[cfg(feature = "prof")]
    {
        let mut s = String::new();
        for (i, e) in EVENTS.iter().enumerate() {
            let v = e.load(std::sync::atomic::Ordering::Relaxed);
            if v > 0 {
                s += &format!("  event {:<2} {:>12}\n", i, v);
            }
        }
        for (i, n) in NAMES.iter().enumerate() {
            let v = COUNTERS[i].load(std::sync::atomic::Ordering::Relaxed);
            let calls = CALLS[i].load(std::sync::atomic::Ordering::Relaxed);
            if v > 0 {
                s += &format!("  {:<14} {:>9.3}s {:>10} calls {:>8.2}us/call\n", n, v as f64 * 1e-9, calls, v as f64 * 1e-3 / calls.max(1) as f64);
            }
        }
        s
    }
    #[cfg(not(feature = "prof"))]
    String::new()
}
