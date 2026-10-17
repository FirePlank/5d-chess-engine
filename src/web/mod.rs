//! Support for the web front end: JSON, game records (SPRT JSONL, 5dpgn, 5DUCI), the 5dpgn
//! notation, the analysis game tree and the JSON command interface the browser (WASM) uses.
//! 5D Chess Lab (https://github.com/FirePlank/5d-chess-lab), its server and its WASM build,
//! is built on it.

pub mod api;
pub mod json;
pub mod pgn;
pub mod record;
pub mod tree;
