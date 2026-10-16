//! Support for the web front end: JSON, game records (SPRT JSONL, 5dpgn, 5DUCI), the 5dpgn
//! notation, the analysis game tree and the JSON command interface the browser (WASM) uses.
//! Shared by the `kairos-web` server and the WASM build.

pub mod api;
pub mod json;
pub mod pgn;
pub mod record;
pub mod tree;
