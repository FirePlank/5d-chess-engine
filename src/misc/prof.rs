//! Feature-gated wall-clock section counters (`--features prof`); free when disabled.

pub const NAMES: [&str; 16] = [
    "build_hc", "hc_movegen", "take_point", "jump+present", "find_checks", "space_ops",
    "ordering", "state_clone", "apply_actions", "eval", "in_check", "tt_validate",
    "hc_entries", "hc_branch", "hc_finish", "hc_status",
];

#[cfg(feature = "prof")]
pub static COUNTERS: [std::sync::atomic::AtomicU64; 16] = [const { std::sync::atomic::AtomicU64::new(0) }; 16];

#[cfg(feature = "prof")]
pub struct Scope(usize, std::time::Instant);
#[cfg(feature = "prof")]
impl Drop for Scope {
    fn drop(&mut self) {
        COUNTERS[self.0].fetch_add(self.1.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
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

pub fn report() -> String {
    #[cfg(feature = "prof")]
    {
        let mut s = String::new();
        for (i, n) in NAMES.iter().enumerate() {
            let v = COUNTERS[i].load(std::sync::atomic::Ordering::Relaxed);
            if v > 0 {
                s += &format!("  {:<14} {:>9.3}s\n", n, v as f64 * 1e-9);
            }
        }
        s
    }
    #[cfg(not(feature = "prof"))]
    String::new()
}
