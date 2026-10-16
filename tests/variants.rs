//! Every built-in variant sets up, can move, and round-trips through its 5DUCI position and its
//! 5dpgn Board header.

use kairos::core::state::MateType;
use kairos::core::variants::{default_variant, position_from_uci, state_from_setup, VARIANT_NAMES};
use kairos::web::record;

#[test]
fn every_variant_sets_up_and_can_move() {
    for name in VARIANT_NAMES {
        let setup = default_variant(name).unwrap_or_else(|| panic!("{name}: no setup"));
        let s = state_from_setup(&setup).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(s.get_mate_type(), MateType::None, "{name}: the side to move cannot move");
        let uci = record::variant_position(name).unwrap();
        let back = position_from_uci(&uci).unwrap_or_else(|e| panic!("{name}: {uci}: {e}"));
        assert_eq!(record::variant_of(&back), Some(name), "{name}: not recognised from {uci}");
    }
}
