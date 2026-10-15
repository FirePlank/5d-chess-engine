//! Built-in variants, 5DFEN parsing and 5DUCI `position` handling.

use super::action::ExtMove;
use super::multiverse::{Multiverse, TimelineLayout};
use super::promotion::PromotionOptions;
use super::state::State;
use super::vec4::Vec4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sign {
    Nil,
    Positive,
    Negative,
}

#[derive(Clone, Debug)]
pub struct BoardSpec {
    pub fen: String,
    pub sign: Sign,
    pub l: i32,
    pub t: i32,
    pub c: bool,
}

#[derive(Clone, Debug)]
pub struct VariantSetup {
    pub size_x: i32,
    pub size_y: i32,
    pub boards: Vec<BoardSpec>,
    pub even: bool,
    pub promotions: PromotionOptions,
}

const STD: &str = "r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBQK*BNR*";

fn spec(fen: &str, sign: Sign, l: i32, t: i32, c: bool) -> BoardSpec {
    BoardSpec { fen: fen.into(), sign, l, t, c }
}

pub fn default_variant(name: &str) -> Option<VariantSetup> {
    let q = PromotionOptions::QUEEN;
    let one = |fen: &str| VariantSetup {
        size_x: 8,
        size_y: 8,
        boards: vec![spec(fen, Sign::Nil, 0, 1, false)],
        even: false,
        promotions: q,
    };
    Some(match name {
        "Standard" => one(STD),
        "Standard - Turn Zero" => VariantSetup {
            size_x: 8,
            size_y: 8,
            boards: vec![spec(STD, Sign::Nil, 0, 0, true), spec(STD, Sign::Nil, 0, 1, false)],
            even: false,
            promotions: q,
        },
        "Standard - Reversed Royalty" => one("r*nbycbnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBYCBNR*"),
        "Standard - Half Reflected" => one("r*nbqk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*NBK*QBNR*"),
        "Standard - Defended Pawn" => one("r*qbnk*bnr*/p*p*p*p*p*p*p*p*/8/8/8/8/P*P*P*P*P*P*P*P*/R*QBNK*BNR*"),
        "Standard - Two Timelines" => VariantSetup {
            size_x: 8,
            size_y: 8,
            boards: vec![spec(STD, Sign::Negative, 0, 1, false), spec(STD, Sign::Positive, 0, 1, false)],
            even: true,
            promotions: q,
        },
        "Very Small - Open" => VariantSetup {
            size_x: 4,
            size_y: 4,
            boards: vec![spec("nbrk/3p*/P*3/KRBN", Sign::Nil, 0, 1, false)],
            even: false,
            promotions: q,
        },
        _ => return None,
    })
}

pub fn state_from_setup(v: &VariantSetup) -> Result<State, String> {
    let layout = if v.even { TimelineLayout::even() } else { TimelineLayout::odd() };
    let boards: Vec<(i32, i32, bool, String)> = v
        .boards
        .iter()
        .map(|b| {
            let l = if b.sign == Sign::Negative { layout.mirror(b.l) } else { b.l };
            (l, b.t, b.c, b.fen.clone())
        })
        .collect();
    let m = Multiverse::new(layout, &boards, v.size_x, v.size_y).map_err(|e| e.0)?;
    Ok(State::new(m, v.promotions))
}

pub fn startpos() -> State {
    state_from_setup(&default_variant("Standard - Turn Zero").unwrap()).unwrap()
}

/// Parse one `fen:l:t:c` block (without brackets); `l` may carry a +/- sign.
pub fn parse_board_fen_metadata(s: &str) -> Result<BoardSpec, String> {
    let mut parts = s.splitn(4, ':');
    let fen = parts.next().unwrap_or("").to_string();
    let ls = parts.next().ok_or_else(|| format!("Expect line in board string:{}", s))?;
    let ts = parts.next().ok_or_else(|| format!("Expect time in board string:{}", s))?;
    let cs = parts.next().ok_or_else(|| format!("Expect color in board string:{}", s))?;
    let (sign, lnum) = if let Some(r) = ls.strip_prefix('+') {
        (Sign::Positive, r)
    } else if let Some(r) = ls.strip_prefix('-') {
        (Sign::Negative, r)
    } else {
        (Sign::Nil, ls)
    };
    let l: i32 = lnum.trim().parse().map_err(|_| format!("Expect number after ':': {}", s))?;
    let t: i32 = ts.trim().parse().map_err(|_| format!("Expect number after ':': {}", s))?;
    if l < 0 || l > Vec4::L_MAX || t < 0 || t > Vec4::T_MAX {
        return Err(format!("Board coordinate out of range: {}", s));
    }
    let c = match cs.trim() {
        "w" | "W" => false,
        "b" | "B" => true,
        _ => return Err(format!("Unknown color in {}", s)),
    };
    Ok(BoardSpec { fen, sign, l, t, c })
}

/// Build a state from a 5DUCI `position` argument string (`startpos` or
/// `[size MxN [odd|even]] fen [..][..]`).
pub fn position_from_uci(position: &str) -> Result<State, String> {
    let position = position.trim();
    if position == "startpos" {
        return Ok(startpos());
    }
    let (mut m, mut n, mut even) = (0, 0, false);
    let fen: &str;
    if let Some(rest) = position.strip_prefix("size ") {
        match rest.find(" fen ") {
            None => fen = rest,
            Some(fp) => {
                let mut it = rest[..fp].split_whitespace();
                if let Some(sz) = it.next() {
                    if let Some((a, b)) = sz.split_once('x') {
                        m = a.parse().unwrap_or(0);
                        n = b.parse().unwrap_or(0);
                    }
                }
                if it.next() == Some("even") {
                    even = true;
                }
                fen = &rest[fp + 5..];
            }
        }
    } else if let Some(rest) = position.strip_prefix("fen ") {
        fen = rest;
    } else if let Some(rest) = position.strip_prefix("even fen ") {
        even = true;
        fen = rest;
    } else if let Some(rest) = position.strip_prefix("odd fen ") {
        fen = rest;
    } else {
        fen = position;
    }
    let mut boards = Vec::new();
    let bytes = fen.as_bytes();
    let mut pos = 0;
    while pos < bytes.len() {
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() {
            break;
        }
        if bytes[pos] == b'[' {
            match fen[pos..].find(']') {
                None => {
                    boards.push(parse_board_fen_metadata(&fen[pos..])?);
                    break;
                }
                Some(e) => {
                    boards.push(parse_board_fen_metadata(&fen[pos + 1..pos + e])?);
                    pos += e + 1;
                }
            }
        } else {
            boards.push(parse_board_fen_metadata(&fen[pos..])?);
            break;
        }
    }
    state_from_setup(&VariantSetup {
        size_x: if m > 0 { m } else { 8 },
        size_y: if n > 0 { n } else { 8 },
        boards,
        even,
        promotions: PromotionOptions::QUEEN,
    })
}

/// Apply a 5DUCI move history (`<lan>`/`submit` tokens) to `s`.
pub fn apply_uci_moves(s: &mut State, moves: &str) -> Result<(), String> {
    for tok in moves.split_whitespace() {
        if tok == "submit" {
            if !s.submit::<false>() {
                return Err("position error: cannot submit move history".into());
            }
        } else {
            let mv = ExtMove::parse(tok).map_err(|e| format!("position error: cannot parse move {}: {}", tok, e))?;
            match s.normalize_promotion(mv) {
                Some(n) if s.apply_ext::<false>(n) => {}
                _ => return Err(format!("position error: cannot apply move {}", tok)),
            }
        }
    }
    Ok(())
}
