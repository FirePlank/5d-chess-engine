//! Built-in variants, 5DFEN parsing and 5DUCI `position` handling.

use super::action::ExtMove;
use super::multiverse::{Multiverse, TimelineLayout};
use super::promotion::{parse_promotions, PromotionOptions};
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
        _ => return official_variant(name),
    })
}

/// Every variant of the official game, by its name there (the game's own list, in its menu
/// order), except Random and Global Warming, whose start positions are not known here.
pub const VARIANT_NAMES: [&str; 44] = [
    "Standard",
    "Standard - Turn Zero",
    "Standard - Reversed Royalty",
    "Standard - Half Reflected",
    "Standard - Defended Pawn",
    "Standard - Princess",
    "Standard - Two Timelines",
    "Simple - No Bishops",
    "Simple - No Knights",
    "Simple - No Rooks",
    "Simple - No Queens",
    "Simple - Knights vs. Bishops",
    "Simple - Simple Set",
    "Small",
    "Small - Flipped",
    "Small - Centered",
    "Small - Open",
    "Very Small",
    "Very Small - Open",
    "Misc - Timeline Invasion",
    "Misc - Timeline Formations",
    "Misc - Timeline Tactician",
    "Misc - Timeline Strategos",
    "Misc - Timeline Battleground",
    "Misc - Timeline Skirmish",
    "Misc - Timeline Fragments",
    "Misc - Timeline Marauders",
    "Misc - Excessive",
    "Misc - King of Kings",
    "Misc - Royal Queen Showdown",
    "Focused - Just Knights",
    "Focused - Just Bishops",
    "Focused - Just Rooks",
    "Focused - Just Queens",
    "Focused - Just Pawns",
    "Focused - Just Kings",
    "Focused - Just Unicorns",
    "Focused - Just Dragons",
    "Focused - Just Brawns",
    "Checkmate Practice - Knight",
    "Checkmate Practice - Bishop",
    "Checkmate Practice - Rook",
    "Checkmate Practice - Queen",
    "Checkmate Practice - Pawns",
];

/// Official variants from adri326/5dchess-notation: (name, width, height, timelines, boards).
/// Boards are space-separated per timeline, `|` between boards of one timeline; a timeline may
/// carry `:n`, its start half-turn. Brawns `w`/`W`, royal queens `y`/`Y`, common kings `c`/`C`.
const OFFICIAL: [(&str, i32, i32, &str, &str); 37] = [
    ("Standard - Princess", 8, 8, "0", "rnbskbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBSKBNR"),
    ("Simple - No Bishops", 6, 6, "0", "rnqknr/pppppp/6/6/PPPPPP/RNQKNR"),
    ("Simple - No Knights", 6, 6, "0", "rbqkbr/pppppp/6/6/PPPPPP/RBQKBR"),
    ("Simple - No Rooks", 6, 6, "0", "nbqkbn/pppppp/6/6/PPPPPP/NBQKBN"),
    ("Simple - No Queens", 7, 7, "0", "rnbknbr/ppppppp/7/7/7/PPPPPPP/RNBKNBR"),
    ("Simple - Knights vs. Bishops", 6, 6, "0", "rbqkbr/pppppp/6/6/PPPPPP/RNQKNR"),
    ("Simple - Simple Set", 6, 6, "0", "rnbqkr/pppppp/6/6/PPPPPP/RKQBNR"),
    ("Small", 5, 5, "0", "kqbnr/ppppp/5/PPPPP/KQBNR"),
    ("Small - Flipped", 5, 5, "0", "nbrqk/ppppp/5/PPPPP/KQRBN"),
    ("Small - Centered", 5, 5, "0", "rnkqr/ppppp/5/PPPPP/RQKNR"),
    ("Small - Open", 5, 5, "0", "prnbk/3pp/5/PP3/KBNRP"),
    ("Very Small", 4, 4, "0", "nbrk/pppp/PPPP/KRBN"),
    ("Misc - Timeline Invasion", 5, 5, "-0 +0", "nbkrb/ppppp/5/5/PPPPP ppppp/5/5/PPPPP/NBKRB"),
    ("Misc - Timeline Formations", 5, 5, "-0 +0", "ppppp/5/5/5/2K2 2k2/5/5/5/PPPPP"),
    ("Misc - Timeline Tactician", 4, 4, "-0 +0", "kbnr/pppp/4/4 4/4/PPPP/KBNR"),
    ("Misc - Timeline Strategos", 5, 5, "-0 +0", "nbkur/ppppp/5/5/5 5/5/5/PPPPP/RUKBN"),
    ("Misc - Timeline Battleground", 5, 5, "-1 0 1", "rrkrr/bbqbb/ppppp/5/PPPPP nnnnn/ppppp/5/PPPPP/NNNNN ppppp/5/PPPPP/BBQBB/RRKRR"),
    ("Misc - Timeline Skirmish", 5, 5, "-0 +0", "3rk/3pp/5/BB3/NN3 3nn/3bb/5/PP3/KR3"),
    ("Misc - Timeline Fragments", 4, 4, "-0:1 +0", "kppp/4/4/NBRU nbru/4/4/KPPP"),
    ("Misc - Timeline Marauders", 5, 5, "-1 0 1", "wrkrw/1www1/5/5/5 w1w1w/5/5/5/W1W1W 5/5/5/1WWW1/WRKRW"),
    ("Misc - Excessive", 7, 7, "0", "kruqdrk/rnbknbr/ppppppp/7/PPPPPPP/RNBKNBR/KRUQDRK"),
    ("Misc - King of Kings", 5, 5, "0", "cckcc/5/5/5/CCKCC"),
    ("Misc - Royal Queen Showdown", 6, 6, "0", "4y1/6/6/6/6/1Y4"),
    ("Focused - Just Knights", 5, 5, "0", "n1kn1/5/5/5/1NK1N"),
    ("Focused - Just Bishops", 5, 5, "0", "1bbk1/5/5/5/1KBB1"),
    ("Focused - Just Rooks", 5, 5, "0", "1rk1r/5/5/5/R1KR1"),
    ("Focused - Just Queens", 6, 6, "0", "1q1k2/6/6/6/6/2K1Q1"),
    ("Focused - Just Pawns", 5, 5, "0", "ppppk/5/5/5/KPPPP"),
    ("Focused - Just Kings", 3, 3, "0", "2k/3/K2"),
    ("Focused - Just Unicorns", 5, 5, "0", "1u1uk/5/5/5/KU1U1"),
    ("Focused - Just Dragons", 5, 5, "0", "2ddk/5/5/5/KDD2"),
    ("Focused - Just Brawns", 5, 5, "0", "wwwwk/5/5/5/KWWWW"),
    ("Checkmate Practice - Knight", 6, 6, "0", "5n/6/6/6/6/K5"),
    ("Checkmate Practice - Bishop", 6, 6, "0", "4b1/6/6/6/6/K5"),
    ("Checkmate Practice - Rook", 6, 6, "0", "5r/6/6/6/6/K5"),
    ("Checkmate Practice - Queen", 6, 6, "0", "4q1/6/6/6/6/K5"),
    ("Checkmate Practice - Pawns", 6, 6, "0", "2ppp1/6/6/6/6/3K2"),
];

/// An official variant from `OFFICIAL`, with kings, rooks, pawns and brawns unmoved (castling
/// and the double step as in Standard) and promotion to a queen.
fn official_variant(name: &str) -> Option<VariantSetup> {
    let &(_, size_x, size_y, lines, fens) = OFFICIAL.iter().find(|v| v.0 == name)?;
    let unmoved = |fen: &str| fen.chars().flat_map(|c| if "KkRrPpWw".contains(c) { vec![c, '*'] } else { vec![c] }).collect::<String>();
    let even = lines.split(' ').any(|l| l.starts_with("-0") || l.starts_with("+0"));
    let mut boards = Vec::new();
    for (line, timeline) in lines.split(' ').zip(fens.split(' ')) {
        let (l, start) = match line.split_once(':') {
            Some((l, s)) => (l, s.parse::<i32>().ok()?),
            None => (line, 0),
        };
        let (sign, l) = match l {
            "-0" => (Sign::Negative, 0),
            "+0" => (Sign::Positive, 0),
            n => (Sign::Nil, n.parse().ok()?),
        };
        for (k, fen) in timeline.split('|').enumerate() {
            // Half-turn 2 is White's turn 1.
            let v = 2 + start + k as i32;
            boards.push(spec(&unmoved(fen), sign, l, v.div_euclid(2), v.rem_euclid(2) == 1));
        }
    }
    Some(VariantSetup { size_x, size_y, boards, even, promotions: PromotionOptions::QUEEN })
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
    if !(0..=Vec4::L_MAX).contains(&l) || !(0..=Vec4::T_MAX).contains(&t) {
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
    let mut promotions = PromotionOptions::QUEEN;
    let fen: &str;
    if let Some(rest) = position.strip_prefix("size ") {
        match rest.find(" fen ") {
            None => fen = rest,
            Some(fp) => {
                let mut it = rest[..fp].split_whitespace();
                if let Some(sz) = it.next()
                    && let Some((a, b)) = sz.split_once('x') {
                        m = a.parse().unwrap_or(0);
                        n = b.parse().unwrap_or(0);
                    }
                for tok in it.by_ref() {
                    match tok {
                        "even" => even = true,
                        "promotions" => break,
                        _ => {}
                    }
                }
                // `promotions <pieces>`: the pieces pawns promote to (as 5dpgn's header: letters,
                // or `*`); without it, the queen only.
                if let Some(p) = it.next() {
                    promotions = parse_promotions(p)?;
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
        promotions,
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
