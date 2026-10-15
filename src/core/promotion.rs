//! Promotion rule sets (bitmask of allowed promotion pieces).

use super::piece::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct PromotionOptions(pub u8);

impl PromotionOptions {
    pub const NONE: Self = Self(0);
    pub const QUEEN: Self = Self(1 << 0);
    pub const ROOK: Self = Self(1 << 1);
    pub const BISHOP: Self = Self(1 << 2);
    pub const KNIGHT: Self = Self(1 << 3);
    pub const UNICORN: Self = Self(1 << 4);
    pub const DRAGON: Self = Self(1 << 5);
    pub const PRINCESS: Self = Self(1 << 6);
    pub const COMMON_KING: Self = Self(1 << 7);
    pub const ALL: Self = Self(0xff);
}

const PRIORITY: [Piece; 8] =
    [QUEEN_W, PRINCESS_W, ROOK_W, BISHOP_W, KNIGHT_W, COMMON_KING_W, UNICORN_W, DRAGON_W];

pub fn piece_to_promotion_option(p: Piece) -> PromotionOptions {
    match p {
        QUEEN_W => PromotionOptions::QUEEN,
        PRINCESS_W => PromotionOptions::PRINCESS,
        ROOK_W => PromotionOptions::ROOK,
        BISHOP_W => PromotionOptions::BISHOP,
        KNIGHT_W => PromotionOptions::KNIGHT,
        COMMON_KING_W => PromotionOptions::COMMON_KING,
        UNICORN_W => PromotionOptions::UNICORN,
        DRAGON_W => PromotionOptions::DRAGON,
        _ => PromotionOptions::NONE,
    }
}

pub fn can_promote_to(options: PromotionOptions, p: Piece) -> bool {
    let o = piece_to_promotion_option(p);
    o != PromotionOptions::NONE && options.0 & o.0 != 0
}

pub fn default_promote_to(options: PromotionOptions) -> Piece {
    for p in PRIORITY {
        if can_promote_to(options, p) {
            return p;
        }
    }
    NO_PIECE
}

pub fn parse_promotions(value: &str) -> Result<PromotionOptions, String> {
    let mut result = 0u8;
    let mut wildcard = false;
    for c in value.bytes() {
        if c == b',' || c.is_ascii_whitespace() {
            continue;
        }
        if c == b'*' {
            wildcard = true;
            continue;
        }
        let o = piece_to_promotion_option(c);
        if o == PromotionOptions::NONE {
            return Err(format!("parse_promotions(): Invalid promotion piece: {}", c as char));
        }
        result |= o.0;
    }
    if wildcard {
        if result != 0 {
            return Err("parse_promotions(): '*' cannot be combined with promotion pieces".into());
        }
        return Ok(PromotionOptions::ALL);
    }
    Ok(PromotionOptions(result))
}

pub fn format_promotions(o: PromotionOptions) -> String {
    if o == PromotionOptions::NONE {
        return String::new();
    }
    if o == PromotionOptions::ALL {
        return "*".into();
    }
    PRIORITY.iter().filter(|&&p| can_promote_to(o, p)).map(|&p| p as char).collect()
}
