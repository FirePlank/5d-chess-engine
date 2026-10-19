//! 5dpgn parsing and printing.
//!
//! A port of the C++ engine's `pgnparser` (lexer, recursive-descent parser and move matcher)
//! and its 5dpgn printers. A written move is resolved against the position by rendering every
//! candidate move in full notation and matching the written fields against it, so SAN-like
//! moves (`Nf3`, `N>>d3`, `Rbxa2`, `(0T5)Qb3>>x(0T1)f7`), LAN (`(0T1)Ng1f3`) and castling all
//! go through the same path.

use crate::core::action::{Action, ExtMove, FullMove};
use crate::core::check_position::CheckPosition;
use crate::core::multiverse::{next_turn, previous_turn, show_turn, Turn};
use crate::core::piece::*;
use crate::core::promotion::{default_promote_to, format_promotions, parse_promotions, PromotionOptions};
use crate::core::state::{MateType, State};
use crate::core::variants::{default_variant, parse_board_fen_metadata, state_from_setup, BoardSpec, Sign, VariantSetup};
use crate::core::vec4::Vec4;

/// A parsed 5dpgn game.
#[derive(Clone, Debug)]
pub struct PgnGame {
    /// `[Key "value"]` headers in file order (board FEN tags are in `setup`).
    pub headers: Vec<(String, String)>,
    /// The initial position.
    pub setup: VariantSetup,
    /// The game tree; the root has no action.
    pub root: PgnNode,
    /// The result token ending the main line (`1-0`, `0-1`, `1/2-1/2`), if any.
    pub result: Option<String>,
}

/// A node of the game tree: the action that reaches it and its continuations.
#[derive(Clone, Debug, Default)]
pub struct PgnNode {
    /// The action played to reach this node (`None` at the root).
    pub action: Option<Action>,
    /// The action as written in the file (trimmed).
    pub text: String,
    /// `{...}` comments inside or after the action (at the root: comments among the headers).
    pub comments: Vec<String>,
    /// `children[0]` continues the main line; the others are variations, in file order.
    pub children: Vec<PgnNode>,
}

impl PgnNode {
    /// The actions of the main line below this node.
    pub fn mainline(&self) -> Vec<&Action> {
        let mut out = Vec::new();
        let mut n = self;
        while let Some(c) = n.children.first() {
            out.extend(c.action.as_ref());
            n = c;
        }
        out
    }
}

// ------------------------------------------------------------------ lexer

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tok {
    WhiteSpace,
    Comment,
    Metadata,
    Turn,
    Line,
    Time,
    Relative,
    Capture,
    Piece,
    CastleK,
    CastleQ,
    File,
    Equal,
    Zero,
    Number,
    Plus,
    Minus,
    Jump,
    Branch,
    Softmate,
    Checkmate,
    Eval,
    PresentMoved,
    LParen,
    RParen,
    WhiteWins,
    BlackWins,
    Draw,
    End,
}

/// Lexer state; copied to backtrack.
#[derive(Clone, Copy)]
struct Cursor {
    /// Start and end of the current token.
    start: usize,
    pos: usize,
    tok: Tok,
    /// Piece letter or file of the current token.
    ch: u8,
    num: u32,
    /// Turn of the last turn marker (`2b.`, `/`).
    turn: Turn,
    /// Body of the current comment or metadata token.
    body: (usize, usize),
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

struct Parser<'a> {
    src: &'a str,
    b: Cursor,
}

type PResult<T> = Result<Option<T>, String>;

// ------------------------------------------------------------------ syntax tree

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct AbsBoard {
    sign: Sign,
    line: Option<u32>,
    time: Option<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ToBoard {
    Any,
    Abs(AbsBoard),
    /// Line and time differences (`$(L+1T-2)`).
    Rel(Option<i64>, Option<i64>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Jump {
    None,
    Plain,
    Branching,
}

/// A written move, physical or superphysical, with the optional fields as written.
#[derive(Clone, Copy, Debug)]
struct MoveAst {
    superphysical: bool,
    board: Option<AbsBoard>,
    /// `Some(true)` for O-O, `Some(false)` for O-O-O.
    castle: Option<bool>,
    piece: Option<u8>,
    file: Option<u8>,
    rank: Option<u32>,
    jump: Jump,
    capture: bool,
    to_board: ToBoard,
    to_file: u8,
    to_rank: u32,
    promote: Option<u8>,
}

struct AstAction {
    /// Moves with their byte spans in the source.
    moves: Vec<(MoveAst, (usize, usize))>,
    comments: Vec<String>,
}

/// One position of a line: the variations played from it, then the main continuation.
struct Step {
    variations: Vec<(AstAction, Line)>,
    main: Option<AstAction>,
}

struct Line {
    steps: Vec<Step>,
    outcome: Option<&'static str>,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Result<Self, String> {
        let b = Cursor { start: 0, pos: 0, tok: Tok::End, ch: 0, num: 0, turn: previous_turn((1, false)), body: (0, 0) };
        let mut p = Parser { src, b };
        p.next()?;
        Ok(p)
    }

    fn line_of(&self, pos: usize) -> usize {
        self.src.as_bytes()[..pos.min(self.src.len())].iter().filter(|&&c| c == b'\n').count() + 1
    }

    /// An error at `from` quoting the text parsed since then.
    fn err(&self, from: usize, what: &str) -> String {
        let end = self.b.pos.max(from).min(self.src.len());
        let quoted: String = self.src[from..end].chars().take(80).collect();
        format!("line {}: {}: {}", self.line_of(from), what, quoted.trim())
    }

    fn lex_err(&self, at: usize, what: String) -> String {
        format!("line {}: next_token(): {}", self.line_of(at), what)
    }

    fn fail<T>(&mut self, fb: Cursor) -> PResult<T> {
        self.b = fb;
        Ok(None)
    }

    fn tok(&self) -> Tok {
        self.b.tok
    }

    fn next(&mut self) -> Result<(), String> {
        let src: &'a str = self.src;
        let s = src.as_bytes();
        let at = |i: usize| s.get(i).copied().unwrap_or(0);
        let mut i = self.b.pos;
        self.b.start = i;
        if i >= s.len() {
            self.b.tok = Tok::End;
            return Ok(());
        }
        let c = s[i];
        let tok = match c {
            b'L' => Tok::Line,
            b'T' => Tok::Time,
            b'$' => Tok::Relative,
            b'x' => Tok::Capture,
            b'P' | b'W' | b'K' | b'C' | b'Q' | b'Y' | b'S' | b'N' | b'R' | b'B' | b'U' | b'D' => {
                self.b.ch = c;
                Tok::Piece
            }
            b'O' => {
                if at(i + 1) != b'-' || at(i + 2) != b'O' {
                    return Err(self.lex_err(i, "expected castling after 'O'".into()));
                }
                i += 2;
                if at(i + 1) == b'-' && at(i + 2) == b'O' {
                    i += 2;
                    Tok::CastleQ
                } else {
                    Tok::CastleK
                }
            }
            b'a'..=b'h' => {
                self.b.ch = c;
                Tok::File
            }
            b'=' => Tok::Equal,
            b'0' if at(i + 1) == b'-' => {
                if at(i + 2) != b'1' {
                    return Err(self.lex_err(i, "expected '0-1' after '0-'".into()));
                }
                i += 2;
                Tok::BlackWins
            }
            b'0' => Tok::Zero,
            b'1' if at(i + 1) == b'-' => {
                if at(i + 2) != b'0' {
                    return Err(self.lex_err(i, "expected '1-0' after '1-'".into()));
                }
                i += 2;
                Tok::WhiteWins
            }
            b'1' if at(i + 1) == b'/' && at(i + 2) == b'2' && at(i + 3) == b'-' => {
                if !s[i..].starts_with(b"1/2-1/2") {
                    return Err(self.lex_err(i, "expected '1/2-1/2'".into()));
                }
                i += 6;
                Tok::Draw
            }
            b'1'..=b'9' => {
                let st = i;
                while at(i).is_ascii_digit() {
                    i += 1;
                }
                let n: u32 = src[st..i].parse().map_err(|_| self.lex_err(i, "integer literal is out of range".into()))?;
                self.b.num = n;
                let color = if matches!(at(i), b'w' | b'b') && at(i + 1) == b'.' {
                    i += 1;
                    Some(at(i - 1) == b'b')
                } else if at(i) == b'.' {
                    Some(false)
                } else {
                    i -= 1;
                    None
                };
                match color {
                    None => Tok::Number,
                    Some(c) => {
                        if n > Vec4::T_MAX as u32 {
                            return Err(self.lex_err(i, "turn number exceeds the supported range".into()));
                        }
                        let t = (n as i32, c);
                        if t != next_turn(self.b.turn) {
                            return Err(self.lex_err(i, format!(
                                "non-consecutive turn number: {} followed by {}",
                                show_turn(self.b.turn),
                                show_turn(t)
                            )));
                        }
                        self.b.turn = t;
                        Tok::Turn
                    }
                }
            }
            b'/' => {
                self.b.turn = next_turn(self.b.turn);
                Tok::Turn
            }
            b'+' => Tok::Plus,
            b'-' => Tok::Minus,
            b'*' => Tok::Softmate,
            b'#' => Tok::Checkmate,
            b'~' => Tok::PresentMoved,
            b'!' | b'?' => {
                while matches!(at(i + 1), b'!' | b'?') {
                    i += 1;
                }
                Tok::Eval
            }
            b'>' if at(i + 1) == b'>' => {
                i += 1;
                Tok::Branch
            }
            b'>' => Tok::Jump,
            b'(' => Tok::LParen,
            b')' => Tok::RParen,
            b'[' => {
                let e = src[i..].find(']').ok_or_else(|| self.lex_err(i, "expect ']' after '['".into()))?;
                self.b.body = (i + 1, i + e);
                i += e;
                Tok::Metadata
            }
            b'{' => {
                let mut depth = 1;
                let st = i;
                while depth > 0 {
                    i += 1;
                    match s.get(i) {
                        None => return Err(self.lex_err(i, "comment not closed".into())),
                        Some(b'{') => depth += 1,
                        Some(b'}') => depth -= 1,
                        _ => {}
                    }
                }
                self.b.body = (st + 1, i);
                Tok::Comment
            }
            c if is_space(c) => {
                while is_space(at(i + 1)) {
                    i += 1;
                }
                Tok::WhiteSpace
            }
            _ => {
                let ch = src[i..].chars().next().unwrap_or('?');
                return Err(self.lex_err(i, format!("unknown token {:?}", ch)));
            }
        };
        self.b.tok = tok;
        self.b.pos = i + 1;
        Ok(())
    }

    fn body(&self) -> &'a str {
        &self.src[self.b.body.0..self.b.body.1]
    }

    /// `$(` [`L`] diff `T` diff `)` with diff `=`, `+n` or `-n` (either part optional).
    fn parse_relative_board(&mut self) -> PResult<ToBoard> {
        let fb = self.b;
        if self.tok() != Tok::Relative {
            return self.fail(fb);
        }
        self.next()?;
        if self.tok() != Tok::LParen {
            return self.fail(fb);
        }
        self.next()?;
        let expect_line = self.tok() == Tok::Line;
        if expect_line {
            self.next()?;
        }
        let (mut dl, mut dt) = (None, None);
        let mut has_line = true;
        let mut sign = 0i64;
        match self.tok() {
            Tok::Plus => sign = 1,
            Tok::Minus => sign = -1,
            Tok::Equal => dl = Some(0),
            _ if expect_line => return Err(self.err(fb.start, "Expect line difference after 'L'")),
            _ => has_line = false,
        }
        if has_line {
            self.next()?;
            match (sign != 0, self.tok() == Tok::Number) {
                (true, true) => {
                    dl = Some(sign * self.b.num as i64);
                    self.next()?;
                }
                (false, true) => return Err(self.err(fb.start, "No number should appear after '='")),
                (true, false) => return Err(self.err(fb.start, "Expect number after '+'/'-'")),
                (false, false) => {}
            }
        }
        if self.tok() == Tok::Time {
            self.next()?;
            match self.tok() {
                Tok::Plus => sign = 1,
                Tok::Minus => sign = -1,
                Tok::Equal => {
                    sign = 0;
                    dt = Some(0);
                }
                _ => return Err(self.err(fb.start, "Expect time difference after 'T'")),
            }
            self.next()?;
            if self.tok() == Tok::Number {
                if sign == 0 {
                    return Err(self.err(fb.start, "No number should appear after '='"));
                }
                dt = Some(sign * self.b.num as i64);
                self.next()?;
            }
        } else if !has_line {
            return Err(self.err(fb.start, "Relative board without any LT information"));
        }
        if self.tok() != Tok::RParen {
            return Err(self.err(fb.start, "Expect ')'"));
        }
        self.next()?;
        Ok(Some(ToBoard::Rel(dl, dt)))
    }

    /// `(` [`L`] line `T` time `)`, `(L` line `)` or `(T` time `)`; line may be signed.
    fn parse_absolute_board(&mut self) -> PResult<AbsBoard> {
        let fb = self.b;
        if self.tok() != Tok::LParen {
            return self.fail(fb);
        }
        self.next()?;
        let expect_line = self.tok() == Tok::Line;
        if expect_line {
            self.next()?;
        }
        let mut ab = AbsBoard { sign: Sign::Nil, line: None, time: None };
        let mut has_line = true;
        match self.tok() {
            Tok::Plus | Tok::Minus => {
                ab.sign = if self.tok() == Tok::Plus { Sign::Positive } else { Sign::Negative };
                self.next()?;
            }
            Tok::Number | Tok::Zero => {}
            _ if expect_line => return Err(self.err(fb.start, "Expect line after 'L'")),
            _ => has_line = false,
        }
        if has_line {
            match self.tok() {
                Tok::Number | Tok::Zero => {
                    ab.line = Some(if self.tok() == Tok::Zero { 0 } else { self.b.num });
                    self.next()?;
                }
                _ if ab.sign == Sign::Nil => return self.fail(fb),
                _ => return Err(self.err(fb.start, "Expect number after '+'/'-'")),
            }
        }
        if self.tok() == Tok::Time {
            self.next()?;
            match self.tok() {
                Tok::Number | Tok::Zero => {
                    ab.time = Some(if self.tok() == Tok::Zero { 0 } else { self.b.num });
                    self.next()?;
                }
                _ => return Err(self.err(fb.start, "Expect number after 'T'")),
            }
        } else if !has_line {
            return self.fail(fb);
        }
        if self.tok() != Tok::RParen {
            return self.fail(fb);
        }
        self.next()?;
        Ok(Some(ab))
    }

    /// [`=`] piece.
    fn parse_promotion(&mut self) -> Result<Option<u8>, String> {
        let st = self.b.start;
        let eq = self.tok() == Tok::Equal;
        if eq {
            self.next()?;
        }
        if self.tok() == Tok::Piece {
            let c = self.b.ch;
            self.next()?;
            return Ok(Some(c));
        }
        if eq {
            return Err(self.err(st, "Expect promotion piece after '='"));
        }
        Ok(None)
    }

    /// Optional check, softmate or mate symbol, then an optional evaluation symbol.
    fn skip_annotations(&mut self, present_moved: bool) -> Result<(), String> {
        if matches!(self.tok(), Tok::Plus | Tok::Softmate | Tok::Checkmate) {
            self.next()?;
        }
        if present_moved && self.tok() == Tok::PresentMoved {
            self.next()?;
        }
        if self.tok() == Tok::Eval {
            self.next()?;
        }
        Ok(())
    }

    /// [board] ([piece] [file] [rank] [`x`] file rank [promotion] | `O-O` | `O-O-O`) [check] [eval].
    fn parse_physical_move(&mut self) -> PResult<MoveAst> {
        let fb = self.b;
        let mut m = MoveAst {
            superphysical: false,
            board: self.parse_absolute_board()?,
            castle: None,
            piece: None,
            file: None,
            rank: None,
            jump: Jump::None,
            capture: false,
            to_board: ToBoard::Any,
            to_file: 0,
            to_rank: 0,
            promote: None,
        };
        if matches!(self.tok(), Tok::CastleK | Tok::CastleQ) {
            m.castle = Some(self.tok() == Tok::CastleK);
            self.next()?;
            self.skip_annotations(false)?;
            return Ok(Some(m));
        }
        if self.tok() == Tok::Piece {
            m.piece = Some(self.b.ch);
            self.next()?;
        }
        if self.tok() == Tok::File {
            m.file = Some(self.b.ch);
            self.next()?;
        }
        if self.tok() == Tok::Number {
            m.rank = Some(self.b.num);
            self.next()?;
        }
        if m.file.is_some() && m.rank.is_some() && !matches!(self.tok(), Tok::Capture | Tok::File) {
            if matches!(self.tok(), Tok::Jump | Tok::Branch)
                || self.parse_relative_board()?.is_some()
                || self.parse_absolute_board()?.is_some()
            {
                return self.fail(fb);
            }
            (m.to_file, m.to_rank) = (m.file.take().unwrap(), m.rank.take().unwrap());
        } else {
            if self.tok() == Tok::Capture {
                m.capture = true;
                self.next()?;
            }
            if self.tok() != Tok::File {
                return self.fail(fb);
            }
            m.to_file = self.b.ch;
            self.next()?;
            if self.tok() != Tok::Number {
                return self.fail(fb);
            }
            m.to_rank = self.b.num;
            self.next()?;
        }
        m.promote = self.parse_promotion()?;
        self.skip_annotations(false)?;
        Ok(Some(m))
    }

    /// Whitespace then `(>L` line `)` or `(~T` time `)`.
    fn parse_timeline_comment(&mut self) -> Result<bool, String> {
        let fb = self.b;
        if self.tok() == Tok::WhiteSpace {
            self.next()?;
        }
        if self.tok() != Tok::LParen {
            self.b = fb;
            return Ok(false);
        }
        self.next()?;
        let (marker, axis) = match self.tok() {
            Tok::Jump => (Tok::Jump, Tok::Line),
            Tok::PresentMoved => (Tok::PresentMoved, Tok::Time),
            _ => {
                self.b = fb;
                return Ok(false);
            }
        };
        self.next()?;
        if self.tok() != axis {
            self.b = fb;
            return Ok(false);
        }
        self.next()?;
        if marker == Tok::Jump && matches!(self.tok(), Tok::Plus | Tok::Minus) {
            self.next()?;
        }
        if !matches!(self.tok(), Tok::Zero | Tok::Number) {
            return Err(self.err(fb.start, "Expect number in timeline comment"));
        }
        self.next()?;
        if self.tok() != Tok::RParen {
            return Err(self.err(fb.start, "Expect ')' in timeline comment"));
        }
        self.next()?;
        Ok(true)
    }

    /// [board] [piece] [file] [rank] (jump [`x`] | [jump] [`x`] to-board) file rank [promotion]
    /// [check] [`~`] [eval] timeline-comment*.
    fn parse_superphysical_move(&mut self) -> PResult<MoveAst> {
        let fb = self.b;
        let mut m = MoveAst {
            superphysical: true,
            board: self.parse_absolute_board()?,
            castle: None,
            piece: None,
            file: None,
            rank: None,
            jump: Jump::None,
            capture: false,
            to_board: ToBoard::Any,
            to_file: 0,
            to_rank: 0,
            promote: None,
        };
        if self.tok() == Tok::Piece {
            m.piece = Some(self.b.ch);
            self.next()?;
        }
        if self.tok() == Tok::File {
            m.file = Some(self.b.ch);
            self.next()?;
        }
        if self.tok() == Tok::Number {
            m.rank = Some(self.b.num);
            self.next()?;
        }
        if matches!(self.tok(), Tok::Jump | Tok::Branch) {
            m.jump = if self.tok() == Tok::Jump { Jump::Plain } else { Jump::Branching };
            self.next()?;
        }
        if self.tok() == Tok::Capture {
            m.capture = true;
            self.next()?;
        }
        if m.jump != Jump::None && self.tok() == Tok::File {
            m.to_board = ToBoard::Any;
        } else if let Some(ab) = self.parse_absolute_board()? {
            m.to_board = ToBoard::Abs(ab);
        } else if let Some(rb) = self.parse_relative_board()? {
            m.to_board = rb;
        } else if m.jump == Jump::None {
            return self.fail(fb);
        } else {
            return Err(self.err(fb.start, "Expect destination board/square after '>'/'>>'"));
        }
        if self.tok() != Tok::File {
            return Err(self.err(fb.start, "Expect destination file"));
        }
        m.to_file = self.b.ch;
        self.next()?;
        if self.tok() != Tok::Number {
            return Err(self.err(fb.start, "Expect destination rank"));
        }
        m.to_rank = self.b.num;
        self.next()?;
        m.promote = self.parse_promotion()?;
        self.skip_annotations(true)?;
        while self.parse_timeline_comment()? {}
        Ok(Some(m))
    }

    /// A move and its source span.
    fn parse_move(&mut self) -> PResult<(MoveAst, (usize, usize))> {
        let st = self.b.start;
        let m = match self.parse_physical_move()? {
            Some(m) => m,
            None => match self.parse_superphysical_move()? {
                Some(m) => m,
                None => return Ok(None),
            },
        };
        Ok(Some((m, (st, self.b.start))))
    }

    /// Skip whitespace and comments, returning the comments.
    fn parse_comments(&mut self) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        while matches!(self.tok(), Tok::WhiteSpace | Tok::Comment) {
            if self.tok() == Tok::Comment {
                out.push(self.body().to_string());
            }
            self.next()?;
        }
        Ok(out)
    }

    /// turn-marker space-comment* move (space-comment+ move)* space-comment*.
    fn parse_actions(&mut self) -> PResult<AstAction> {
        let fb = self.b;
        if self.tok() != Tok::Turn {
            return self.fail(fb);
        }
        self.next()?;
        let mut comments = self.parse_comments()?;
        let Some(first) = self.parse_move()? else {
            return self.fail(fb);
        };
        let mut moves = vec![first];
        while matches!(self.tok(), Tok::WhiteSpace | Tok::Comment) {
            comments.extend(self.parse_comments()?);
            match self.parse_move()? {
                Some(m) => moves.push(m),
                None => break,
            }
        }
        Ok(Some(AstAction { moves, comments }))
    }

    /// A game tree: per position, `(`variations`)` then the main continuation, until a result
    /// or the end. `None` if it does not start like one.
    fn parse_line(&mut self) -> PResult<Line> {
        let fb = self.b;
        let mut steps = Vec::new();
        loop {
            let outcome = match self.tok() {
                Tok::WhiteWins => Some("1-0"),
                Tok::BlackWins => Some("0-1"),
                Tok::Draw => Some("1/2-1/2"),
                _ => None,
            };
            if outcome.is_some() {
                self.next()?;
                return Ok(Some(Line { steps, outcome }));
            }
            let branch_turn = self.b.turn;
            let mut variations = Vec::new();
            while self.tok() == Tok::LParen {
                let st = self.b.start;
                self.next()?;
                self.parse_comments()?;
                let Some(act) = self.parse_actions()? else {
                    if steps.is_empty() {
                        return self.fail(fb);
                    }
                    return Err(self.err(st, "Invalid game tree continuation"));
                };
                let Some(sub) = self.parse_line()? else {
                    return Err(self.err(st, "Invalid game tree branch"));
                };
                if self.tok() != Tok::RParen {
                    return Err(self.err(st, "Expect ')'"));
                }
                variations.push((act, sub));
                self.b.turn = branch_turn;
                self.next()?;
                self.parse_comments()?;
            }
            let main = self.parse_actions()?;
            let done = main.is_none();
            steps.push(Step { variations, main });
            if done {
                return Ok(Some(Line { steps, outcome: None }));
            }
        }
    }
}

// ------------------------------------------------------------------ matching

fn opt_match<T: PartialEq>(written: Option<T>, full: Option<T>) -> bool {
    written.is_none() || written == full
}

fn board_match(a: &AbsBoard, b: &AbsBoard) -> bool {
    (a.line.is_none() || (a.sign, a.line) == (b.sign, b.line)) && opt_match(a.time, b.time)
}

/// Whether the written move `a` is consistent with the fully written move `b`.
fn move_match(a: &MoveAst, b: &MoveAst) -> bool {
    if a.superphysical != b.superphysical {
        return false;
    }
    if let (Some(x), Some(y)) = (&a.board, &b.board) {
        if !board_match(x, y) {
            return false;
        }
    }
    if let Some(kingside) = a.castle {
        let to = if kingside { b'g' } else { b'c' };
        return b.piece == Some(KING_W)
            && b.file == Some(b'e')
            && b.to_file == to
            && ((b.rank == Some(1) && b.to_rank == 1) || (b.rank == Some(8) && b.to_rank == 8));
    }
    let to_board = match (&a.to_board, &b.to_board) {
        (ToBoard::Any, _) => true,
        (ToBoard::Abs(x), ToBoard::Abs(y)) => board_match(x, y),
        (ToBoard::Rel(dl, dt), ToBoard::Rel(fl, ft)) => opt_match(*dl, *fl) && opt_match(*dt, *ft),
        _ => false,
    };
    opt_match(a.piece, b.piece)
        && opt_match(a.file, b.file)
        && opt_match(a.rank, b.rank)
        && (a.jump == Jump::None || a.jump == b.jump)
        && to_board
        && (!a.capture || b.capture)
        && a.to_file == b.to_file
        && a.to_rank == b.to_rank
        && opt_match(a.promote, b.promote)
}

/// A board as written by `State::pretty_lt`.
fn abs_board(s: &State, v: Vec4) -> AbsBoard {
    let layout = s.multiverse().layout();
    let l = v.l();
    let (sign, line) = match (layout.even, l >= 0) {
        (true, true) => (Sign::Positive, l),
        (true, false) => (Sign::Negative, layout.mirror(l)),
        (false, true) => (Sign::Nil, l),
        (false, false) => (Sign::Negative, -l),
    };
    AbsBoard { sign, line: Some(line as u32), time: Some(v.t() as u32) }
}

/// Whether the piece on `p` is a pawn or brawn reaching its last rank on `q`.
fn promotes(s: &State, pic: Piece, q: Vec4) -> bool {
    (pic == PAWN_W || pic == BRAWN_W) && q.y() == if s.player() { 0 } else { s.get_board_size().1 - 1 }
}

/// The fully written form of `fm` (every field present), as the matcher sees it.
fn full_ast(s: &State, fm: FullMove, promote: Option<u8>, relative: bool) -> MoveAst {
    let (p, q) = (fm.from, fm.to);
    let player = s.player();
    let pic = to_white(piece_name(s.get_piece(p, player)));
    let sp = p.tl() != q.tl();
    let pt = promote.unwrap_or_else(|| default_promote_to(s.get_promotion_options()));
    let d = q - p;
    MoveAst {
        superphysical: sp,
        board: Some(abs_board(s, p)),
        castle: None,
        piece: Some(pic),
        file: Some(b'a' + p.x() as u8),
        rank: Some(p.y() as u32 + 1),
        jump: match (sp, (q.t(), player) < s.get_timeline_end(q.l())) {
            (false, _) => Jump::None,
            (true, true) => Jump::Branching,
            (true, false) => Jump::Plain,
        },
        capture: fm.captured_piece(s) != NO_PIECE,
        to_board: match (sp, relative) {
            (false, _) => ToBoard::Any,
            (true, true) => ToBoard::Rel(Some(d.l() as i64), Some(d.t() as i64)),
            (true, false) => ToBoard::Abs(abs_board(s, q)),
        },
        to_file: b'a' + q.x() as u8,
        to_rank: q.y() as u32 + 1,
        promote: (pt != NO_PIECE && promotes(s, pic, q)).then_some(pt),
    }
}

fn turn_name(s: &State) -> String {
    let (t, c) = s.get_present();
    format!("T{} {}", t, if c { "black" } else { "white" })
}

/// Resolve a written move to the unique legal move it denotes in `s` (a unique pawn move wins
/// among several matches, so `e4` is never ambiguous).
fn resolve_move(s: &State, m: &MoveAst, text: &str) -> Result<ExtMove, String> {
    let player = s.player();
    let relative = matches!(m.to_board, ToBoard::Rel(..));
    let (mut matched, mut pawns) = (Vec::new(), Vec::new());
    let mut dests = Vec::new();
    for p in s.get_movable_pieces() {
        dests.clear();
        s.gen_piece_move(p, &mut dests);
        let pic = to_white(piece_name(s.get_piece(p, player)));
        for &q in &dests {
            if (p.tl() != q.tl()) != m.superphysical {
                continue;
            }
            let fm = FullMove::new(p, q);
            if move_match(m, &full_ast(s, fm, m.promote, relative)) {
                matched.push(fm);
                if pic == PAWN_W {
                    pawns.push(fm);
                }
            }
        }
    }
    let fm = match (matched.len(), pawns.len()) {
        (1, _) => matched[0],
        (_, 1) => pawns[0],
        (0, _) => return Err(format!("Invalid move {} at {}: no legal move matches", text, turn_name(s))),
        _ => {
            let c: Vec<String> = matched.iter().map(|m| m.to_string()).collect();
            return Err(format!("Ambiguous move {} at {}; candidates: {}", text, turn_name(s), c.join(" ")));
        }
    };
    s.normalize_promotion(ExtMove::new(fm, m.promote.unwrap_or(NO_PIECE)))
        .ok_or_else(|| format!("Illegal promotion in {} at {}", text, turn_name(s)))
}

/// Resolve and play the moves of one action from `s`; returns the action and the position
/// after it is submitted.
fn resolve_action(s: &State, moves: &[(MoveAst, (usize, usize))], src: &str) -> Result<(Action, State), String> {
    let mut t = s.clone();
    let mut ext = Vec::with_capacity(moves.len());
    for (m, (a, b)) in moves {
        let text = src[*a..*b].trim();
        let em = resolve_move(&t, m, text)?;
        if !t.apply_ext::<false>(em) {
            return Err(format!("Illegal move {} (parsed as {}) at {}", text, em, turn_name(&t)));
        }
        ext.push(em);
    }
    let act = Action::from_vector(&ext, s);
    let after = s.can_apply_action(&act).ok_or_else(|| {
        let text = moves.first().map_or("", |(_, (a, _))| &src[*a..moves.last().unwrap().1.1]);
        format!("Cannot submit after {} at {}", text.trim(), turn_name(s))
    })?;
    Ok((act, after))
}

/// Parse one action (moves separated by whitespace or comments, no turn marker) played from `s`.
pub fn parse_action(s: &State, text: &str) -> Result<Action, String> {
    let mut p = Parser::new(text)?;
    p.parse_comments()?;
    let mut moves = Vec::new();
    while let Some(m) = p.parse_move()? {
        moves.push(m);
        p.parse_comments()?;
    }
    if p.tok() != Tok::End {
        return Err(p.err(p.b.start, "Unexpected input in action"));
    }
    if moves.is_empty() {
        return Err("Empty action".into());
    }
    Ok(resolve_action(s, &moves, text)?.0)
}

/// Parse one move played from `s` (which may already have earlier moves of the action applied).
pub fn parse_move(s: &State, text: &str) -> Result<ExtMove, String> {
    let mut p = Parser::new(text.trim())?;
    let m = p.parse_move()?.filter(|_| p.tok() == Tok::End);
    let (m, _) = m.ok_or_else(|| format!("Cannot parse move {}", text.trim()))?;
    resolve_move(s, &m, text.trim())
}

// ------------------------------------------------------------------ games

fn find<'h>(headers: &'h [(String, String)], key: &str) -> Option<&'h str> {
    headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str())
}

/// Leading integer of `s`, like `std::stoi`.
fn stoi(s: &str) -> Option<i32> {
    let s = s.trim_start();
    let end = s.char_indices().find(|&(i, c)| !(c.is_ascii_digit() || (i == 0 && (c == '+' || c == '-')))).map_or(s.len(), |(i, _)| i);
    s[..end].parse().ok()
}

/// The initial position described by the headers and board tags (`derive_variant_setup`).
fn derive_setup(headers: &[(String, String)], mut boards: Vec<BoardSpec>, default: Option<&VariantSetup>) -> Result<VariantSetup, String> {
    let size = find(headers, "size").unwrap_or("8x8");
    let (sx, sy) = size.split_once('x').ok_or_else(|| format!("Invalid board size format: {}", size))?;
    let (mut size_x, mut size_y) = match (stoi(sx), stoi(sy)) {
        (Some(x), Some(y)) if (1..=BOARD_LENGTH).contains(&x) && (1..=BOARD_LENGTH).contains(&y) => (x, y),
        _ => return Err(format!("Invalid board size: {} (max board size allowed: {})", size, BOARD_LENGTH)),
    };
    let promotions = find(headers, "promotions").map(parse_promotions).transpose()?;
    let mut even = None;
    let board = find(headers, "board").or_else(|| find(headers, "variant"));
    match board {
        Some("Custom - Even" | "Even") => even = Some(true),
        Some("Custom - Odd" | "Odd") => even = Some(false),
        Some(b) if b.starts_with("Custom") => {}
        Some(b) if boards.is_empty() => {
            let v = default_variant(b).ok_or_else(|| format!("Unknown variant: {}", b))?;
            (size_x, size_y, boards, even) = (v.size_x, v.size_y, v.boards, Some(v.even));
        }
        _ => {}
    }
    if let Some(t) = find(headers, "timeline") {
        even = Some(match t.to_ascii_lowercase().as_str() {
            "even" => true,
            "odd" => false,
            _ => return Err(format!("Invalid Timeline value: {}", t)),
        });
    }
    if boards.is_empty() {
        if board.is_some() {
            return Err("Variant is unspecific: no Board header or 5DFEN given".into());
        }
        let mut v = default.cloned().unwrap_or_else(|| default_variant("Standard").unwrap());
        if let Some(p) = promotions {
            v.promotions = p;
        }
        return Ok(v);
    }
    for b in &boards {
        if b.fen.matches('/').count() as i32 + 1 != size_y {
            return Err(format!("FEN does not contain exactly {} rows: {}", size_y, b.fen));
        }
    }
    let even = even.unwrap_or_else(|| boards.iter().any(|b| b.sign != Sign::Nil && b.l == 0));
    Ok(VariantSetup { size_x, size_y, boards, even, promotions: promotions.unwrap_or(PromotionOptions::QUEEN) })
}

/// Parse a `[Key "value"]` header body.
fn parse_header(body: &str) -> Result<(String, String), String> {
    let k = body.find(|c: char| c.is_ascii() && is_space(c as u8)).ok_or_else(|| format!("Expect space in header: {}", body))?;
    let rest = &body[k + 1..];
    let rest = rest.strip_prefix('"').ok_or_else(|| format!("Expect '\"' in header: {}", body))?;
    let e = rest.find('"').ok_or_else(|| format!("'\"' not closed: {}", body))?;
    if e + 1 != rest.len() {
        return Err(format!("Too many arguments in header: {}", body));
    }
    Ok((body[..k].to_string(), rest[..e].to_string()))
}

/// Add `child`, replacing an existing child with the same action (the later one wins).
fn add_child(node: &mut PgnNode, child: PgnNode) {
    node.children.retain(|c| c.action != child.action);
    node.children.push(child);
}

fn make_child(s: &State, act: &AstAction, src: &str) -> Result<(PgnNode, State), String> {
    let (action, after) = resolve_action(s, &act.moves, src)?;
    let (a, b) = (act.moves[0].1.0, act.moves.last().unwrap().1.1);
    let node = PgnNode { action: Some(action), text: src[a..b].trim().to_string(), comments: act.comments.clone(), children: Vec::new() };
    Ok((node, after))
}

/// Play `line` from `node` (at position `s`); recursion only into variations.
fn build_line(node: &mut PgnNode, s: &State, line: &Line, src: &str) -> Result<(), String> {
    let mut cur = node;
    let mut s = s.clone();
    for step in &line.steps {
        for (act, sub) in &step.variations {
            let (mut child, after) = make_child(&s, act, src)?;
            build_line(&mut child, &after, sub, src)?;
            add_child(cur, child);
        }
        let Some(act) = &step.main else {
            // A line ending in variations: the last one counts as the main line.
            if !cur.children.is_empty() {
                cur.children.rotate_right(1);
            }
            break;
        };
        let (child, after) = make_child(&s, act, src)?;
        add_child(cur, child);
        cur.children.rotate_right(1);
        cur = &mut cur.children[0];
        s = after;
    }
    Ok(())
}

/// Parse a 5dpgn game. `default_setup` is used when the text has no `[Board]`/`[Variant]`
/// header and no board tags (e.g. a bare move list); if `None`, such input is "Standard".
pub fn parse_5dpgn(text: &str, default_setup: Option<&VariantSetup>) -> Result<PgnGame, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut p = Parser::new(text)?;
    let mut comments = p.parse_comments()?;
    let (mut headers, mut boards) = (Vec::new(), Vec::new());
    while p.tok() == Tok::Metadata {
        let body = p.body();
        if !body.contains('"') {
            boards.push(parse_board_fen_metadata(body).map_err(|e| format!("line {}: {}", p.line_of(p.b.start), e))?);
        } else {
            let (k, v) = parse_header(body).map_err(|e| format!("line {}: {}", p.line_of(p.b.start), e))?;
            if find(&headers, &k).is_some() {
                return Err(format!("Duplicate header key: {}", k));
            }
            headers.push((k, v));
        }
        p.next()?;
        comments.extend(p.parse_comments()?);
    }
    let line = p.parse_line()?.ok_or_else(|| p.err(p.b.start, "Bad input, parse failed"))?;
    p.parse_comments()?;
    if p.tok() != Tok::End {
        return Err(p.err(p.b.start, "Unexpected trailing input"));
    }
    let setup = derive_setup(&headers, boards, default_setup)?;
    let s = state_from_setup(&setup)?;
    let mut root = PgnNode { comments, ..PgnNode::default() };
    build_line(&mut root, &s, &line, text)?;
    Ok(PgnGame { headers, setup, root, result: line.outcome.map(String::from) })
}

// ------------------------------------------------------------------ printing

fn square(v: Vec4) -> String {
    format!("{}{}", (b'a' + v.x() as u8) as char, (b'1' + v.y() as u8) as char)
}

/// Printing options for `action_to_pgn_opts`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PgnOptions {
    /// Minimal disambiguation (see `action_to_pgn_short`).
    pub short: bool,
    /// Search for checkmate/softmate to mark the last check `#`/`*`; without it checks are `+`.
    pub mate: bool,
}

impl Default for PgnOptions {
    fn default() -> Self {
        PgnOptions { short: false, mate: true }
    }
}

/// `mv` (normalized, played from `s`) without check symbols: `board` as the source board
/// prefix, then the piece, the source file/rank when asked (a pawn always shows its file when
/// capturing or jumping), the jump, `x`, the target board when `to_tl`, the target square and
/// the promotion.
fn move_core(s: &State, mv: ExtMove, board: &str, file: bool, rank: bool, to_tl: bool) -> String {
    let (p, q) = (mv.fm.from, mv.fm.to);
    let player = s.player();
    let pic = to_white(piece_name(s.get_piece(p, player)));
    let capture = mv.fm.captured_piece(s) != NO_PIECE;
    let sp = p.tl() != q.tl();
    let mut o = board.to_string();
    if pic != PAWN_W {
        o.push(pic as char);
    }
    if file || (pic == PAWN_W && (capture || sp)) {
        o.push((b'a' + p.x() as u8) as char);
    }
    if rank {
        o.push((b'1' + p.y() as u8) as char);
    }
    if sp {
        o += if (q.t(), player) < s.get_timeline_end(q.l()) { ">>" } else { ">" };
        if capture {
            o.push('x');
        }
        if to_tl {
            o += &s.pretty_lt(q);
        }
    } else if capture {
        o.push('x');
    }
    o += &square(q);
    if mv.promote_to != NO_PIECE && promotes(s, pic, q) {
        o.push('=');
        o.push(mv.promote_to as char);
    }
    o
}

/// `mv` with its source board and the least of source file, rank and target board that
/// resolves back to it in `s` (in the spirit of the C++ SHOW_SHORT), e.g. `(0T2)Nf3`,
/// `(0T3)exd5`, `(0T1)O-O`, `(1T3)N>>(1T2)f5`.
fn short_core(s: &State, mv: ExtMove) -> String {
    // (file, rank, target board)
    const ATTEMPTS: [(bool, bool, bool); 7] = [
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (false, false, true),
        (true, false, true),
        (false, true, true),
        (true, true, true),
    ];
    let (p, q) = (mv.fm.from, mv.fm.to);
    let board = s.pretty_lt(p);
    for (file, rank, to_tl) in ATTEMPTS {
        let text = move_core(s, mv, &board, file, rank, to_tl);
        if parse_move(s, &text).is_ok_and(|m| m == mv) {
            let king = to_white(piece_name(s.get_piece(p, s.player()))) == KING_W;
            if king && p.tl() == q.tl() && p.y() == q.y() && (q.x() - p.x()).abs() == 2 {
                let castle = format!("{}{}", board, if q.x() > p.x() { "O-O" } else { "O-O-O" });
                if parse_move(s, &castle).is_ok_and(|m| m == mv) {
                    return castle;
                }
            }
            return text;
        }
    }
    move_core(s, mv, &board, true, true, true)
}

/// Play `mv` on `s`; returns its notation without check symbol, whether it gives check, and
/// the branching suffix (`~` when the present moves back, then ` (>L<new line>)`).
fn play_move(s: &mut State, mv: ExtMove, short: bool) -> Option<(String, bool, String)> {
    let mv = s.normalize_promotion(mv)?;
    let core = if short { short_core(s, mv) } else { move_core(s, mv, &s.pretty_lt(mv.fm.from), true, true, true) };
    let (player, present, new_line) = (s.player(), s.get_present().0, s.new_line());
    let q = mv.fm.to;
    let branching = mv.fm.from.tl() != q.tl() && (q.t(), player) < s.get_timeline_end(q.l());
    if !s.apply_ext::<false>(mv) {
        return None;
    }
    let check = CheckPosition::for_check_symbols(s).first_check(player, true).is_some();
    let mut suffix = String::new();
    if branching {
        if s.get_present().0 < present {
            suffix.push('~');
        }
        suffix += &format!(" (>L{})", s.pretty_l(new_line));
    }
    Some((core, check, suffix))
}

fn single_move(s: &State, mv: &ExtMove, short: bool) -> String {
    let mut t = s.clone();
    match play_move(&mut t, *mv, short) {
        Some((core, check, suffix)) => format!("{}{}{}", core, if check { "+" } else { "" }, suffix),
        None => "---INVALID MOVE---".into(),
    }
}

/// One move of an action in 5dpgn (full notation, capture, promotion and check symbols), e.g.
/// `(0T1)Ng1f3`, `(0T5)Qb3>>x(0T1)f7~ (>L1)`. `s` has the earlier moves of the action applied.
pub fn move_to_pgn(s: &State, mv: &ExtMove) -> String {
    single_move(s, mv, false)
}

/// `move_to_pgn` in short notation (see `action_to_pgn_short`).
pub fn move_to_pgn_short(s: &State, mv: &ExtMove) -> String {
    single_move(s, mv, true)
}

/// An action played from `s` in 5dpgn, like the C++ printer with SHOW_CAPTURE, SHOW_PROMOTION
/// and SHOW_MATE: the last checking move carries `#` (checkmate), `*` (softmate) or `+`.
/// A checking action needs a mate search, which can be slow in huge positions; prefer
/// `action_to_pgn_with_witness` when the next action is known.
pub fn action_to_pgn(s: &State, act: &Action) -> String {
    action_to_pgn_opts(s, act, None, PgnOptions::default())
}

/// `action_to_pgn` given the action played next (`witness`): when it can be played, the
/// position is not checkmate, and only a softmate test is needed (if it branches).
pub fn action_to_pgn_with_witness(s: &State, act: &Action, witness: Option<&Action>) -> String {
    action_to_pgn_opts(s, act, witness, PgnOptions::default())
}

/// `action_to_pgn` in short notation for move lists: the source board always, the source
/// file/rank and the target board only where needed to resolve the move, e.g. `(0T2)Nf3`,
/// `(0T3)exd5`, `(0T5)Q>>x(0T1)f7+~ (>L1)`. Re-parses to the same action.
pub fn action_to_pgn_short(s: &State, act: &Action) -> String {
    action_to_pgn_opts(s, act, None, PgnOptions { short: true, mate: true })
}

/// An action in 5dpgn with the given options and optional next action (see
/// `action_to_pgn_with_witness`).
pub fn action_to_pgn_opts(s: &State, act: &Action, witness: Option<&Action>, opts: PgnOptions) -> String {
    let mut t = s.clone();
    let mut parts = Vec::with_capacity(act.len());
    for &mv in act.moves() {
        match play_move(&mut t, mv, opts.short) {
            Some(p) => parts.push(p),
            None => return "---INVALID ACTION---".into(),
        }
    }
    if !t.submit::<false>() {
        return "---INVALID ACTION---".into();
    }
    let mut symbols: Vec<&str> = parts.iter().map(|p| if p.1 { "+" } else { "" }).collect();
    if let Some(last) = symbols.iter().rposition(|&c| c == "+").filter(|_| opts.mate) {
        let mt = match witness.filter(|w| t.can_apply_action(w).is_some()) {
            Some(w) => {
                let mut v = w.moves().to_vec();
                let branches = (Action::sort(&mut v, &t) as usize) < v.len();
                if branches && t.is_softmate() { MateType::Softmate } else { MateType::None }
            }
            None => t.get_mate_type(),
        };
        match mt {
            MateType::Checkmate => symbols[last] = "#",
            MateType::Softmate => symbols[last] = "*",
            MateType::None | MateType::Stalemate => {}
        }
    }
    // A win by exiled kings (the opponent's royals are all on inactive timelines, and it has no
    // legal action) ends the game like a mate: the last move carries `#`.
    if opts.mate && !symbols.iter().any(|&c| c == "#") && t.exiled_kings_loser().is_some() && t.get_mate_type() == MateType::Stalemate {
        if let Some(last) = symbols.last_mut() {
            *last = "#";
        }
    }
    let texts: Vec<String> = parts.iter().zip(symbols).map(|((core, _, suffix), sym)| format!("{}{}{}", core, sym, suffix)).collect();
    texts.join(" ")
}

const BUILTIN: [&str; 44] = crate::core::variants::VARIANT_NAMES;

/// Whether two setups have the same boards (promotions aside).
fn same_boards(a: &VariantSetup, b: &VariantSetup) -> bool {
    (a.size_x, a.size_y, a.even) == (b.size_x, b.size_y, b.even)
        && a.boards.len() == b.boards.len()
        && a.boards.iter().zip(&b.boards).all(|(x, y)| (&x.fen, x.sign, x.l, x.t, x.c) == (&y.fen, y.sign, y.l, y.t, y.c))
}

/// Board tag body `fen:l:t:c` of a board spec.
fn board_tag(b: &BoardSpec) -> String {
    let sign = match b.sign {
        Sign::Nil => "",
        Sign::Positive => "+",
        Sign::Negative => "-",
    };
    format!("{}:{}{}:{}:{}", b.fen, sign, b.l, b.t, if b.c { "b" } else { "w" })
}

/// Header lines for a setup: Board and Mode (plus Size and the board tags for a custom
/// position) and Promotions when not queen-only. A board tag is a pair with an empty key and
/// the tag body (`fen:l:t:c`) as value, written `[fen:l:t:c]`.
pub fn setup_headers(setup: &VariantSetup) -> Vec<(String, String)> {
    let pair = |k: &str, v: String| (k.to_string(), v);
    let builtin = BUILTIN.iter().find(|n| default_variant(n).is_some_and(|v| same_boards(&v, setup)));
    let mut h = Vec::new();
    match builtin {
        Some(name) => h.extend([pair("Board", name.to_string()), pair("Mode", "5D".into())]),
        None => h.extend([
            pair("Board", if setup.even { "Custom - Even" } else { "Custom - Odd" }.into()),
            pair("Mode", "5D".into()),
            pair("Size", format!("{}x{}", setup.size_x, setup.size_y)),
        ]),
    }
    if setup.promotions != PromotionOptions::QUEEN {
        h.push(pair("Promotions", format_promotions(setup.promotions)));
    }
    if builtin.is_none() {
        h.extend(setup.boards.iter().map(|b| pair("", board_tag(b))));
    }
    h
}

/// Write a game: the seven-tag roster from `headers`, the setup headers, the other `headers`,
/// the board tags, then the main line as numbered `1. a / b` lines (ending with the Result
/// header's value when it is a result token).
pub fn write_5dpgn(headers: &[(String, String)], setup: &VariantSetup, actions: &[Action]) -> String {
    write_5dpgn_opts(headers, setup, actions, PgnOptions::default())
}

/// `write_5dpgn` with printing options (each action is printed with the next as witness).
pub fn write_5dpgn_opts(headers: &[(String, String)], setup: &VariantSetup, actions: &[Action], opts: PgnOptions) -> String {
    const ROSTER: [&str; 7] = ["Event", "Site", "Date", "Round", "White", "Black", "Result"];
    const SETUP_KEYS: [&str; 6] = ["board", "variant", "mode", "size", "timeline", "promotions"];
    let clean = |v: &str| v.replace('"', "'").replace(']', ")");
    let mut out = String::new();
    for k in ROSTER {
        if let Some(v) = find(headers, k) {
            out += &format!("[{} \"{}\"]\n", k, clean(v));
        }
    }
    let sh = setup_headers(setup);
    for (k, v) in sh.iter().filter(|(k, _)| !k.is_empty()) {
        out += &format!("[{} \"{}\"]\n", k, v);
    }
    let mut seen: Vec<String> = Vec::new();
    for (k, v) in headers {
        let lk = k.to_ascii_lowercase();
        if k.is_empty() || ROSTER.iter().any(|r| r.eq_ignore_ascii_case(k)) || SETUP_KEYS.contains(&lk.as_str()) || seen.contains(&lk) {
            continue;
        }
        seen.push(lk);
        out += &format!("[{} \"{}\"]\n", k, clean(v));
    }
    for (_, v) in sh.iter().filter(|(k, _)| k.is_empty()) {
        out += &format!("[{}]\n", v);
    }
    out.push('\n');
    let Ok(mut s) = state_from_setup(setup) else { return out };
    let mut turn: Turn = (1, false);
    for (i, act) in actions.iter().enumerate() {
        let text = action_to_pgn_opts(&s, act, actions.get(i + 1), opts);
        if turn.1 {
            out += &format!(" / {}", text);
        } else {
            if i > 0 {
                out.push('\n');
            }
            out += &format!("{}. {}", turn.0, text);
        }
        match s.can_apply_action(act) {
            Some(n) => s = n,
            None => break,
        }
        turn = next_turn(turn);
    }
    if let Some(r) = find(headers, "result").filter(|r| matches!(*r, "1-0" | "0-1" | "1/2-1/2")) {
        out += &format!(" {}", r);
    }
    out.push('\n');
    out
}
