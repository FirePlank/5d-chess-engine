//! Cheap properties of a physical move that tend to make it good or bad (escaping an attack,
//! landing safely, checking, threatening, developing): inputs for a static move-ordering
//! score fitted to the moves played in SPRT games (`scripts/policy/train_attrs.py`), and the
//! fitted weights (`data/order_attrs.bin`).

use crate::core::bitboard::{marked_pos, pmask};
use crate::core::board::Board;
use crate::core::piece::*;

/// Number of attributes (`attrs`).
pub const NA: usize = 13;

/// Rough material value in pawns x 10 (royals: 0).
fn value(p: Piece) -> i32 {
    match p & 0x7f {
        PAWN_W | BRAWN_W => 10,
        KNIGHT_W => 30,
        BISHOP_W => 32,
        ROOK_W => 50,
        QUEEN_W | PRINCESS_W => 90,
        UNICORN_W | DRAGON_W => 40,
        _ => 0,
    }
}

/// Distance from the centre in king steps, x2 (0 at the centre squares).
fn edge(sq: i32) -> i32 {
    let (x, y) = (sq & 7, sq >> 3);
    (2 * x - 7).abs().max((2 * y - 7).abs())
}

/// The attributes of a physical move by `black` (or white) from `from` to `to` on board `src`,
/// `after` being the board the move leaves: [the mover was attacked, was defended; its
/// destination is attacked, defended; hanging there (attacked, undefended); it gives check; it
/// attacks an enemy piece worth more than itself; victim value; mover value; develops a minor
/// piece off the back rank; centrality gain; pawn advance (relative rank); moves a royal].
pub fn attrs(src: &Board, after: &Board, from: i32, to: i32, mover: Piece, victim: Piece, black: bool) -> [i32; NA] {
    let (enemy_s, own_s) = (src.hostile(black), src.hostile(!black));
    let (enemy_a, own_a) = (after.hostile(black), after.hostile(!black));
    let att_from = src.attacks_to(from);
    let att_to = after.attacks_to(to) & !pmask(to);
    let to_attacked = att_to & enemy_a != 0;
    let to_defended = att_to & own_a != 0;
    let royals = after.royal() & enemy_a;
    let check = marked_pos(royals).any(|r| after.attacks_to(r) & own_a != 0);
    let mv = value(mover);
    let threat = marked_pos(enemy_a & !after.royal()).any(|e| value(after.kind(e)) > mv && after.attacks_to(e) & pmask(to) != 0);
    let back = if black { 7 } else { 0 };
    let rank = |sq: i32| if black { 7 - (sq >> 3) } else { sq >> 3 };
    let minor = matches!(mover & 0x7f, KNIGHT_W | BISHOP_W);
    let pawn = matches!(mover & 0x7f, PAWN_W | BRAWN_W);
    [
        (att_from & enemy_s != 0) as i32,
        (att_from & own_s & !pmask(from) != 0) as i32,
        to_attacked as i32,
        to_defended as i32,
        (to_attacked && !to_defended) as i32,
        check as i32,
        threat as i32,
        value(victim),
        mv,
        (minor && (from >> 3) == back && (to >> 3) != back) as i32,
        edge(from) - edge(to),
        if pawn { rank(to) } else { 0 },
        (after.royal() & pmask(to) != 0) as i32,
    ]
}

/// Fitted ordering weights: one per attribute, and per piece kind a value for each origin and
/// destination square (seen from the side to move).
pub struct OrderWeights {
    w: [f32; NA],
    from: Vec<f32>,
    to: Vec<f32>,
}

static EMBEDDED: std::sync::OnceLock<Option<OrderWeights>> = std::sync::OnceLock::new();

/// The weights built into the engine, if they parse.
pub fn embedded() -> Option<&'static OrderWeights> {
    EMBEDDED.get_or_init(|| OrderWeights::from_bytes(include_bytes!("../../data/order_attrs.bin"))).as_ref()
}

impl OrderWeights {
    fn from_bytes(b: &[u8]) -> Option<OrderWeights> {
        if b.len() < 16 || &b[..8] != b"KORD0001" {
            return None;
        }
        let u = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize;
        let (na, nt) = (u(8), u(12));
        if na != NA || b.len() != 16 + 4 * (na + 2 * nt) {
            return None;
        }
        let f: Vec<f32> = b[16..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
        Some(OrderWeights { w: f[..NA].try_into().ok()?, from: f[NA..NA + nt].to_vec(), to: f[NA + nt..].to_vec() })
    }

    /// The fitted logit of a physical move (`attrs`' arguments): higher is likelier played.
    pub fn logit(&self, src: &Board, after: &Board, from: i32, to: i32, mover: Piece, victim: Piece, black: bool) -> f32 {
        let a = attrs(src, after, from, to, mover, victim, black);
        let sq = |xy: i32| (if black { (xy & 7) + 8 * (7 - (xy >> 3)) } else { xy }) as usize;
        let p = (mover & 0x7f) as usize * 64;
        let t = |v: &[f32], i: usize| v.get(i).copied().unwrap_or(0.0);
        self.w.iter().zip(a).map(|(&w, x)| w * x as f32).sum::<f32>() + t(&self.from, p + sq(from)) + t(&self.to, p + sq(to))
    }
}
