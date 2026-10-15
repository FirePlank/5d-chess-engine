//! Perft over legal actions (one action = one ply).

use super::hypercuboid::HcInfo;
use super::promotion::PromotionOptions;
use super::state::State;
use super::variants::{apply_uci_moves, position_from_uci};

/// Leaf count at `depth`; `cap` (if non-zero) limits the actions expanded per node.
pub fn perft(s: &State, depth: u32, cap: usize) -> u64 {
    let (w, ss) = HcInfo::build_hc(s);
    let mut total = 0u64;
    for (n, mv) in w.search(s, ss).enumerate() {
        if cap != 0 && n >= cap {
            break;
        }
        if depth <= 1 {
            total += 1;
            continue;
        }
        let mut t = s.clone();
        for &m in &mv {
            t.apply_move::<true>(m);
        }
        t.submit::<true>();
        total += perft(&t, depth - 1, cap);
    }
    total
}

/// Parse a "G <promotions> <5duci position> moves <lan|submit>*" line.
pub fn parse_g_line(line: &str) -> Result<State, String> {
    let rest = line.strip_prefix("G ").ok_or("not a G line")?;
    let (promo, rest) = rest.split_once(' ').ok_or("bad G line")?;
    let (pos, moves) = match rest.find(" moves") {
        Some(i) => (&rest[..i], &rest[i + 6..]),
        None => (rest, ""),
    };
    let mut s = position_from_uci(pos)?;
    s.set_promotion_options(PromotionOptions(promo.parse::<u8>().map_err(|e| e.to_string())?));
    apply_uci_moves(&mut s, moves)?;
    Ok(s)
}
