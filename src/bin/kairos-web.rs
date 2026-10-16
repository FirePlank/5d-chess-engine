//! kairos-web: the Kairos website. Serves the viewer / analysis board (web/static, with the
//! rules and engine compiled to WASM), a browser over game record files and play against
//! the engine, one human player at a time.
//!
//! kairos-web [--port 8080] [--host 127.0.0.1] [--root <repo>] [--games <dir>]...
//!            [--hash 2048] [--threads <cores - 1>] [--no-wasm-build]
//!
//! The play engine searches with `--threads` threads into a `--hash` MB table, kept across
//! actions, and ponders (searches the position) while the human thinks.
//!
//! On start it builds web/static/kairos.wasm (`cargo build --target wasm32-wasip1` of web/wasm)
//! when it is missing or older than the crate sources.

use kairos::core::state::State;
use kairos::engine::ab::AlphaBeta;
use kairos::engine::{Limits, OptionValue};
use kairos::obj;
use kairos::web::api::resolve_position;
use kairos::web::json::Json;
use kairos::web::record::{self, VARIANTS};
use kairos::web::tree::{self, parse_lan_moves, play_action, Status};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime};

// ------------------------------------------------------------------ HTTP

struct Request {
    method: String,
    path: String,
    query: Vec<(String, String)>,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn q(&self, k: &str) -> Option<&str> {
        self.query.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str()).filter(|v| !v.is_empty())
    }
    fn header(&self, k: &str) -> Option<&str> {
        self.headers.iter().find(|(a, _)| a.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str())
    }
    fn json(&self) -> Json {
        Json::parse(std::str::from_utf8(&self.body).unwrap_or("")).unwrap_or(Json::Null)
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn read_request(r: &mut BufReader<TcpStream>) -> Option<Request> {
    let mut line = String::new();
    if r.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let mut headers = Vec::new();
    let mut total = 0;
    loop {
        let mut h = String::new();
        let n = r.read_line(&mut h).ok()?;
        total += n;
        if n == 0 || total > 64 * 1024 {
            return None;
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let len: usize = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    if len > 4 << 20 {
        return None;
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body).ok()?;
    let (path, qs) = target.split_once('?').unwrap_or((&target, ""));
    let query = qs
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (percent_decode(k), percent_decode(v))
        })
        .collect();
    Some(Request { method, path: percent_decode(path), query, headers, body })
}

struct Response {
    status: u16,
    ctype: &'static str,
    body: Vec<u8>,
    extra: Vec<(&'static str, String)>,
}

impl Response {
    fn json(j: Json) -> Response {
        Response { status: 200, ctype: "application/json", body: j.to_string().into_bytes(), extra: vec![("Cache-Control", "no-store".into())] }
    }
    fn error(status: u16, msg: &str) -> Response {
        let mut r = Response::json(obj![("ok", false), ("error", msg)]);
        r.status = status;
        r
    }
}

fn status_text(s: u16) -> &'static str {
    match s {
        200 => "OK",
        304 => "Not Modified",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        _ => "Error",
    }
}

/// Headers every response carries: cross-origin isolation lets the analysis worker share a
/// stop flag with the page (SharedArrayBuffer).
const ISOLATION: &str = "Cross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\nCross-Origin-Resource-Policy: same-origin\r\nX-Content-Type-Options: nosniff\r\n";

fn write_response(w: &mut TcpStream, r: &Response, head_only: bool, keep_alive: bool) -> std::io::Result<()> {
    let mut h = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n{}Connection: {}\r\n",
        r.status,
        status_text(r.status),
        r.ctype,
        r.body.len(),
        ISOLATION,
        if keep_alive { "keep-alive" } else { "close" }
    );
    for (k, v) in &r.extra {
        h += &format!("{}: {}\r\n", k, v);
    }
    h += "\r\n";
    w.write_all(h.as_bytes())?;
    if !head_only {
        w.write_all(&r.body)?;
    }
    w.flush()
}

// ------------------------------------------------------------------ static files

fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        "woff2" => "font/woff2",
        "txt" | "md" | "5dpgn" => "text/plain; charset=utf-8",
        "webmanifest" => "application/manifest+json",
        _ => "application/octet-stream",
    }
}

fn serve_static(dir: &Path, req: &Request) -> Response {
    let rel = req.path.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };
    if rel.split(['/', '\\']).any(|c| c == ".." || c.contains(':')) {
        return Response::error(403, "forbidden");
    }
    let mut p = dir.join(rel);
    if p.is_dir() {
        p = p.join("index.html");
    }
    let Ok(meta) = std::fs::metadata(&p) else { return Response::error(404, "not found") };
    let mtime = meta.modified().ok().and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    let etag = format!("\"{:x}-{:x}\"", meta.len(), mtime);
    if req.header("If-None-Match") == Some(etag.as_str()) {
        return Response { status: 304, ctype: mime(&p), body: Vec::new(), extra: vec![("ETag", etag)] };
    }
    match std::fs::read(&p) {
        Ok(body) => Response { status: 200, ctype: mime(&p), body, extra: vec![("Cache-Control", "no-cache".into()), ("ETag", etag)] },
        Err(_) => Response::error(404, "not found"),
    }
}

// ------------------------------------------------------------------ game files

#[derive(Clone)]
struct GameSummary {
    offset: u64,
    len: u32,
    variant: Arc<str>,
    result: Option<&'static str>,
    reason: Arc<str>,
    actions: u32,
    book: u32,
    game: Option<i64>,
    pair: Option<i64>,
    new_first: Option<bool>,
}

struct FileIndex {
    mtime: SystemTime,
    size: u64,
    games: Vec<GameSummary>,
}

struct Games {
    dirs: Vec<PathBuf>,
    index: Mutex<HashMap<PathBuf, Arc<FileIndex>>>,
    variants: Mutex<HashMap<String, Arc<str>>>,
}

fn result_of(j: &Json) -> Option<&'static str> {
    let nf = j.get("new_first").bool();
    let ws = match (j.get("score").num(), nf) {
        (Some(s), Some(nf)) => Some(if nf { s } else { 1.0 - s }),
        (s, None) => s,
        _ => None,
    }?;
    Some(if ws > 0.75 {
        "1-0"
    } else if ws < 0.25 {
        "0-1"
    } else {
        "1/2-1/2"
    })
}

impl Games {
    /// Every record file under the game directories (two levels deep), with ids.
    fn files(&self) -> Vec<(String, PathBuf, u64, u64)> {
        let mut out = Vec::new();
        for (di, d) in self.dirs.iter().enumerate() {
            let mut stack = vec![(d.clone(), 0)];
            while let Some((p, depth)) = stack.pop() {
                let Ok(rd) = std::fs::read_dir(&p) else { continue };
                for e in rd.flatten() {
                    let path = e.path();
                    if path.is_dir() {
                        if depth < 2 {
                            stack.push((path, depth + 1));
                        }
                        continue;
                    }
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if !matches!(ext, "jsonl" | "5dpgn") {
                        continue;
                    }
                    let Ok(meta) = e.metadata() else { continue };
                    let rel = path.strip_prefix(d).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                    let mtime = meta.modified().ok().and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
                    out.push((format!("{}:{}", di, rel), path, meta.len(), mtime));
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    fn resolve(&self, id: &str) -> Option<PathBuf> {
        let (di, rel) = id.split_once(':')?;
        let d = self.dirs.get(di.parse::<usize>().ok()?)?;
        if rel.split(['/', '\\']).any(|c| c == ".." || c.contains(':')) {
            return None;
        }
        let p = d.join(rel);
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        (p.is_file() && matches!(ext, "jsonl" | "5dpgn")).then_some(p)
    }

    fn variant_name(&self, position: &str) -> Arc<str> {
        if let Some(v) = self.variants.lock().unwrap().get(position) {
            return v.clone();
        }
        let name: Arc<str> = match kairos::core::variants::position_from_uci(position) {
            Ok(s) => record::variant_of(&s).unwrap_or("Custom").into(),
            Err(_) => "Invalid".into(),
        };
        self.variants.lock().unwrap().insert(position.to_string(), name.clone());
        name
    }

    fn index(&self, path: &Path) -> Result<Arc<FileIndex>, String> {
        let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
        let mtime = meta.modified().map_err(|e| e.to_string())?;
        if let Some(ix) = self.index.lock().unwrap().get(path) {
            if ix.mtime == mtime && ix.size == meta.len() {
                return Ok(ix.clone());
            }
        }
        let data = std::fs::read(path).map_err(|e| e.to_string())?;
        let mut games = Vec::new();
        let is_pgn = path.extension().and_then(|e| e.to_str()) == Some("5dpgn");
        if is_pgn {
            games.push(GameSummary {
                offset: 0,
                len: data.len() as u32,
                variant: "5dpgn".into(),
                result: None,
                reason: "".into(),
                actions: 0,
                book: 0,
                game: None,
                pair: None,
                new_first: None,
            });
        } else {
            let mut off = 0usize;
            let mut empty: Arc<str> = "".into();
            for line in data.split(|&b| b == b'\n') {
                let start = off;
                off += line.len() + 1;
                let Ok(text) = std::str::from_utf8(line) else { continue };
                if !text.trim_start().starts_with('{') {
                    continue;
                }
                let Ok(j) = Json::parse(text.trim()) else { continue };
                let moves = j.get("moves").str().unwrap_or("");
                let actions = moves.split_whitespace().filter(|t| *t == "submit").count() as u32;
                let reason = j.get("reason").str().unwrap_or("");
                if &*empty != reason {
                    empty = reason.into();
                }
                games.push(GameSummary {
                    offset: start as u64,
                    len: line.len() as u32,
                    variant: self.variant_name(j.get("position").str().unwrap_or("startpos")),
                    result: result_of(&j),
                    reason: empty.clone(),
                    actions,
                    book: j.get("book").int().unwrap_or(0) as u32,
                    game: j.get("game").int(),
                    pair: j.get("pair").int(),
                    new_first: j.get("new_first").bool(),
                });
            }
        }
        let ix = Arc::new(FileIndex { mtime, size: meta.len(), games });
        self.index.lock().unwrap().insert(path.to_path_buf(), ix.clone());
        Ok(ix)
    }

    fn handle(&self, req: &Request) -> Response {
        match req.path.as_str() {
            "/api/games/files" => {
                let files: Vec<Json> = self
                    .files()
                    .into_iter()
                    .map(|(id, p, size, mtime)| {
                        obj![
                            ("id", id),
                            ("name", p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
                            ("dir", p.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
                            ("size", size),
                            ("mtime", mtime)
                        ]
                    })
                    .collect();
                Response::json(obj![("ok", true), ("files", Json::Arr(files))])
            }
            "/api/games/list" => {
                let Some(path) = req.q("file").and_then(|f| self.resolve(f)) else { return Response::error(404, "no such file") };
                let ix = match self.index(&path) {
                    Ok(ix) => ix,
                    Err(e) => return Response::error(400, &e),
                };
                let variant = req.q("variant");
                let result = req.q("result");
                let reason = req.q("reason");
                let min: u32 = req.q("min").and_then(|v| v.parse().ok()).unwrap_or(0);
                let max: u32 = req.q("max").and_then(|v| v.parse().ok()).unwrap_or(u32::MAX);
                let offset: usize = req.q("offset").and_then(|v| v.parse().ok()).unwrap_or(0);
                let limit: usize = req.q("limit").and_then(|v| v.parse().ok()).unwrap_or(100).min(1000);
                let mut rows: Vec<(usize, &GameSummary)> = ix
                    .games
                    .iter()
                    .enumerate()
                    .filter(|(_, g)| variant.is_none_or(|v| &*g.variant == v))
                    .filter(|(_, g)| result.is_none_or(|r| g.result == Some(r) || (r == "decisive" && matches!(g.result, Some("1-0" | "0-1")))))
                    .filter(|(_, g)| reason.is_none_or(|r| g.reason.starts_with(r)))
                    .filter(|(_, g)| g.actions >= min && g.actions <= max)
                    .collect();
                match req.q("sort") {
                    Some("long") => rows.sort_by(|a, b| b.1.actions.cmp(&a.1.actions)),
                    Some("short") => rows.sort_by(|a, b| a.1.actions.cmp(&b.1.actions)),
                    _ => {}
                }
                // Facets over the whole file for the filter menus.
                let mut variants: HashMap<&str, usize> = HashMap::new();
                let mut reasons: HashMap<String, usize> = HashMap::new();
                let mut results: HashMap<&str, usize> = HashMap::new();
                for g in &ix.games {
                    *variants.entry(&g.variant).or_default() += 1;
                    *reasons.entry(g.reason.split([':', '(']).next().unwrap_or("").trim().to_string()).or_default() += 1;
                    *results.entry(g.result.unwrap_or("?")).or_default() += 1;
                }
                let facet = |m: Vec<(String, usize)>| {
                    let mut m = m;
                    m.sort_by(|a, b| b.1.cmp(&a.1));
                    Json::Arr(m.into_iter().map(|(k, n)| Json::Arr(vec![k.into(), n.into()])).collect())
                };
                let total = rows.len();
                let games: Vec<Json> = rows
                    .into_iter()
                    .skip(offset)
                    .take(limit)
                    .map(|(i, g)| {
                        obj![
                            ("index", i),
                            ("game", g.game),
                            ("pair", g.pair),
                            ("variant", &*g.variant),
                            ("result", g.result),
                            ("reason", &*g.reason),
                            ("actions", g.actions as i64),
                            ("book", g.book as i64),
                            ("newFirst", g.new_first)
                        ]
                    })
                    .collect();
                Response::json(obj![
                    ("ok", true),
                    ("total", total),
                    ("count", ix.games.len()),
                    ("games", Json::Arr(games)),
                    ("variants", facet(variants.into_iter().map(|(k, v)| (k.to_string(), v)).collect())),
                    ("reasons", facet(reasons.into_iter().collect())),
                    ("results", facet(results.into_iter().map(|(k, v)| (k.to_string(), v)).collect()))
                ])
            }
            "/api/games/get" => {
                let Some(path) = req.q("file").and_then(|f| self.resolve(f)) else { return Response::error(404, "no such file") };
                let ix = match self.index(&path) {
                    Ok(ix) => ix,
                    Err(e) => return Response::error(400, &e),
                };
                let i: usize = req.q("index").and_then(|v| v.parse().ok()).unwrap_or(0);
                let Some(g) = ix.games.get(i) else { return Response::error(404, "no such game") };
                let mut f = match std::fs::File::open(&path) {
                    Ok(f) => f,
                    Err(e) => return Response::error(400, &e.to_string()),
                };
                use std::io::Seek;
                let mut buf = vec![0u8; g.len as usize];
                if f.seek(std::io::SeekFrom::Start(g.offset)).and_then(|_| f.read_exact(&mut buf)).is_err() {
                    return Response::error(400, "read failed");
                }
                let text = String::from_utf8_lossy(&buf).into_owned();
                Response::json(obj![("ok", true), ("text", text), ("count", ix.games.len())])
            }
            _ => Response::error(404, "unknown endpoint"),
        }
    }
}

// ------------------------------------------------------------------ play vs engine

/// Seconds a waiting queue entry or an idle player keeps its place without contact.
const QUEUE_TIMEOUT: u64 = 25;
const PLAYER_TIMEOUT: u64 = 90;
/// Seconds the head of the queue has to start a game once the engine is free.
const CLAIM_TIMEOUT: u64 = 30;
/// Seconds a finished player keeps the engine while others wait.
const REMATCH_TIMEOUT: u64 = 20;
/// Seconds an untimed player may think while someone is queued.
const UNTIMED_IDLE: u64 = 300;

#[derive(Clone)]
struct Clock {
    base_ms: u64,
    inc_ms: u64,
}

struct Session {
    token: String,
    human_black: bool,
    variant: String,
    start: String,
    state: State,
    /// LAN of each action played.
    history: Vec<String>,
    /// Remaining time (ms) [white, black]; None for an untimed game.
    clock: Option<Clock>,
    left: [u64; 2],
    turn_start: Instant,
    /// Engine think time per action in an untimed game (ms).
    movetime: u64,
    over: Option<(String, String)>,
    ended_at: Option<Instant>,
    thinking: bool,
    last_seen: Instant,
    /// Latest engine info line while it thinks (parsed fields).
    info: Json,
    /// Engine scores per engine action (engine POV, the engine's reported scale; mates as
    /// +-20000).
    evals: Vec<Option<i32>>,
    started: SystemTime,
}

struct QueueEntry {
    ticket: String,
    last_seen: Instant,
}

struct PlayState {
    session: Option<Session>,
    queue: VecDeque<QueueEntry>,
    /// When the engine became free with someone waiting.
    free_since: Option<Instant>,
    subscribers: Vec<(u64, Sender<String>, Option<String>)>,
    version: u64,
}

struct Play {
    st: Mutex<PlayState>,
    stop: Arc<AtomicBool>,
    /// Bumped per game so a stale engine reply is dropped.
    game_id: AtomicU64,
    engine: Arc<(Mutex<Option<EngineJob>>, Condvar)>,
    hash_mb: usize,
    threads: usize,
    archive: Option<PathBuf>,
}

struct EngineJob {
    game_id: u64,
    state: State,
    limits: Limits,
    /// Search the human's position until stopped, only to fill the table for the reply.
    ponder: bool,
}

fn new_token() -> String {
    let t = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos();
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let mut x = (t as u64) ^ (n.wrapping_mul(0x9e3779b97f4a7c15)) ^ ((std::process::id() as u64) << 32);
    let mut s = String::new();
    for _ in 0..2 {
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51afd7ed558ccd);
        x ^= x >> 33;
        x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
        x ^= x >> 33;
        s += &format!("{:016x}", x);
        x = x.wrapping_add(t as u64 ^ 0x632be59bd9b4e019);
    }
    s
}

fn parse_info(line: &str) -> Json {
    let toks: Vec<&str> = line.split_whitespace().collect();
    let mut kv: Vec<(String, Json)> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        match toks[i] {
            "depth" | "nodes" | "nps" | "time" | "roots" => {
                if let Some(v) = toks.get(i + 1).and_then(|v| v.parse::<i64>().ok()) {
                    kv.push((toks[i].into(), v.into()));
                }
                i += 2;
            }
            "score" => {
                let kind = toks.get(i + 1).copied().unwrap_or("");
                if let Some(v) = toks.get(i + 2).and_then(|v| v.parse::<i64>().ok()) {
                    kv.push((if kind == "mate" { "mate" } else { "cp" }.into(), v.into()));
                }
                i += 3;
            }
            // Win/draw/loss per mille (the "cp" score is on the engine's reported scale:
            // +100 is about a 75% expected score).
            "wdl" => {
                let v: Vec<Json> = toks[i + 1..].iter().take(3).filter_map(|v| v.parse::<i64>().ok()).map(Json::from).collect();
                if v.len() == 3 {
                    kv.push(("wdl".into(), Json::Arr(v)));
                }
                i += 4;
            }
            "pv" => {
                kv.push(("pv".into(), toks[i + 1..].join(" ").into()));
                break;
            }
            _ => i += 1,
        }
    }
    Json::Obj(kv)
}

impl Play {
    fn new(hash_mb: usize, threads: usize, archive: Option<PathBuf>) -> Arc<Play> {
        let play = Arc::new(Play {
            st: Mutex::new(PlayState { session: None, queue: VecDeque::new(), free_since: None, subscribers: Vec::new(), version: 0 }),
            stop: Arc::new(AtomicBool::new(false)),
            game_id: AtomicU64::new(0),
            engine: Arc::new((Mutex::new(None), Condvar::new())),
            hash_mb,
            threads,
            archive,
        });
        let p = play.clone();
        std::thread::Builder::new().name("engine".into()).stack_size(64 << 20).spawn(move || p.engine_loop()).expect("engine thread");
        let p = play.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(500));
            p.tick();
        });
        play
    }

    fn engine_loop(self: Arc<Self>) {
        let mut eng = AlphaBeta::new();
        eng.set_option("Hash", &OptionValue::Int(self.hash_mb as i64));
        eng.set_option("Threads", &OptionValue::Int(self.threads as i64));
        let (lock, cv) = &*self.engine;
        loop {
            let job = {
                let mut g = lock.lock().unwrap();
                while g.is_none() {
                    g = cv.wait(g).unwrap();
                }
                // Under the lock: a job posted after this one stops it (`post_job`).
                self.stop.store(false, Ordering::SeqCst);
                g.take().unwrap()
            };
            if job.ponder {
                // Think on the human's time: the table (kept across actions) fills for the
                // reply. Stopped when the human moves or the game ends; the result is unused.
                eng.find_best_move(&job.state, &job.limits, &self.stop, &mut |_| {});
                continue;
            }
            let mut last_score: Option<i32> = None;
            let mut last_emit = Instant::now() - Duration::from_secs(1);
            let gid = job.game_id;
            let best = eng.find_best_move(&job.state, &job.limits, &self.stop, &mut |line| {
                let info = parse_info(&line);
                last_score = info.get("cp").int().map(|v| v as i32).or_else(|| info.get("mate").int().map(|m| if m > 0 { 20000 } else { -20000 }));
                if last_emit.elapsed() > Duration::from_millis(100) {
                    last_emit = Instant::now();
                    let mut st = self.st.lock().unwrap();
                    if self.game_id.load(Ordering::SeqCst) == gid {
                        if let Some(s) = st.session.as_mut() {
                            s.info = info.clone();
                        }
                        let msg = format!("event: thinking\ndata: {}\n\n", info);
                        Self::broadcast_locked(&mut st, &msg);
                    }
                }
            });
            let lan = best.map(|a| tree::action_lan(&job.state, &a));
            self.engine_done(gid, lan, last_score);
        }
    }

    fn broadcast_locked(st: &mut PlayState, msg: &str) {
        st.subscribers.retain(|(_, tx, _)| tx.send(msg.to_string()).is_ok());
    }

    /// Push the public state to every subscriber.
    fn publish(&self, st: &mut PlayState) {
        st.version += 1;
        let msg = format!("event: state\ndata: {}\n\n", Self::public_state(st, None));
        Self::broadcast_locked(st, &msg);
    }

    fn public_state(st: &PlayState, viewer: Option<&str>) -> Json {
        let game = st.session.as_ref().map(|s| {
            let now_left = Self::clock_now(s);
            obj![
                ("variant", s.variant.as_str()),
                ("start", s.start.as_str()),
                ("humanColor", if s.human_black { "b" } else { "w" }),
                ("moves", s.history.clone()),
                ("clock", s.clock.as_ref().map(|c| obj![("base", c.base_ms), ("inc", c.inc_ms)])),
                ("left", now_left.map(|l| Json::Arr(vec![l[0].into(), l[1].into()]))),
                ("toMove", if s.state.get_present().1 { "b" } else { "w" }),
                ("movetime", s.movetime),
                ("thinking", s.thinking),
                ("over", s.over.as_ref().map(|(r, why)| obj![("result", r.as_str()), ("reason", why.as_str())])),
                ("info", s.info.clone()),
                ("evals", s.evals.clone())
            ]
        });
        let you = viewer.is_some_and(|t| st.session.as_ref().is_some_and(|s| s.token == t));
        let queue_pos = viewer.and_then(|t| st.queue.iter().position(|q| q.ticket == t));
        let busy = st.session.as_ref().is_some_and(|s| s.over.is_none() || st.queue.is_empty() || s.ended_at.is_some_and(|e| e.elapsed().as_secs() < REMATCH_TIMEOUT));
        obj![
            ("ok", true),
            ("busy", st.session.is_some()),
            ("locked", busy),
            ("you", you),
            ("queue", st.queue.len()),
            ("queuePos", queue_pos),
            ("canStart", Self::can_start(st, viewer)),
            ("spectators", st.subscribers.len()),
            ("game", game),
            ("version", st.version)
        ]
    }

    /// Remaining clock times with the running side's elapsed time taken off.
    fn clock_now(s: &Session) -> Option<[u64; 2]> {
        s.clock.as_ref()?;
        let mut l = s.left;
        if s.over.is_none() {
            let side = s.state.get_present().1 as usize;
            l[side] = l[side].saturating_sub(s.turn_start.elapsed().as_millis() as u64);
        }
        Some(l)
    }

    /// Whether `who` (token or ticket) may start a game now.
    fn can_start(st: &PlayState, who: Option<&str>) -> bool {
        let mine = who.is_some_and(|t| st.session.as_ref().is_some_and(|s| s.token == t));
        let free = match &st.session {
            None => true,
            Some(s) => mine || (s.over.is_some() && s.ended_at.is_some_and(|e| e.elapsed().as_secs() >= REMATCH_TIMEOUT)),
        };
        let first = st.queue.front().map(|q| Some(q.ticket.as_str()) == who).unwrap_or(true);
        free && (mine || first)
    }

    fn tick(&self) {
        let mut st = self.st.lock().unwrap();
        let mut changed = false;
        let before = st.queue.len();
        st.queue.retain(|q| q.last_seen.elapsed().as_secs() < QUEUE_TIMEOUT);
        changed |= before != st.queue.len();
        // An untimed game cannot hold the engine forever while others wait.
        let waiting = !st.queue.is_empty();
        if let Some(s) = st.session.as_mut() {
            let human_to_move = s.state.get_present().1 == s.human_black;
            if s.over.is_none() && s.clock.is_none() && waiting && human_to_move && s.turn_start.elapsed().as_secs() > UNTIMED_IDLE {
                self.finish(s, if s.human_black { "1-0" } else { "0-1" }, "abandoned (no move while others wait)");
                changed = true;
            }
        }
        // Flag fall.
        if let Some(s) = st.session.as_mut() {
            if s.over.is_none() {
                if let Some(l) = Self::clock_now(s) {
                    let side = s.state.get_present().1 as usize;
                    if l[side] == 0 {
                        let human_flagged = (side == 1) == s.human_black;
                        s.left = l;
                        self.finish(s, if side == 0 { "0-1" } else { "1-0" }, if human_flagged { "time forfeit" } else { "engine time forfeit" });
                        changed = true;
                    }
                }
            }
        }
        // Release an abandoned or finished session.
        let release = st.session.as_ref().is_some_and(|s| {
            s.last_seen.elapsed().as_secs() > PLAYER_TIMEOUT
                || (s.over.is_some() && !st.queue.is_empty() && s.ended_at.is_some_and(|e| e.elapsed().as_secs() > REMATCH_TIMEOUT + CLAIM_TIMEOUT))
        });
        if release {
            self.end_session(&mut st, "abandoned");
            changed = true;
        }
        // The head of the queue loses its turn if it does not claim the engine in time.
        if st.session.is_none() && !st.queue.is_empty() {
            let since = *st.free_since.get_or_insert_with(Instant::now);
            if since.elapsed().as_secs() > CLAIM_TIMEOUT {
                st.queue.pop_front();
                st.free_since = Some(Instant::now());
                changed = true;
            }
        } else {
            st.free_since = None;
        }
        if changed {
            self.publish(&mut st);
        }
    }

    /// Hand the engine a job, stopping its current search (a ponder) at once.
    fn post_job(&self, job: EngineJob) {
        let (lock, cv) = &*self.engine;
        let mut g = lock.lock().unwrap();
        self.stop.store(true, Ordering::SeqCst);
        *g = Some(job);
        cv.notify_one();
    }

    /// Stop pondering (and drop a ponder job not started yet).
    fn stop_ponder(&self) {
        let (lock, _) = &*self.engine;
        let mut g = lock.lock().unwrap();
        if g.as_ref().is_some_and(|j| j.ponder) {
            *g = None;
        }
        self.stop.store(true, Ordering::SeqCst);
    }

    fn end_session(&self, st: &mut PlayState, why: &str) {
        if let Some(s) = st.session.take() {
            self.stop_ponder();
            self.game_id.fetch_add(1, Ordering::SeqCst);
            if s.over.is_none() && !s.history.is_empty() {
                let r = if s.human_black { "1-0" } else { "0-1" };
                self.archive_game(&s, r, why);
            }
        }
    }

    /// End the game: result, reason, engine stopped, record archived.
    fn finish(&self, s: &mut Session, result: &str, reason: &str) {
        s.over = Some((result.into(), reason.into()));
        s.ended_at = Some(Instant::now());
        if s.thinking {
            self.stop.store(true, Ordering::SeqCst);
            self.game_id.fetch_add(1, Ordering::SeqCst);
        }
        self.stop_ponder();
        s.thinking = false;
        self.archive_game(s, result, reason);
    }

    fn archive_game(&self, s: &Session, result: &str, reason: &str) {
        let Some(path) = &self.archive else { return };
        let mut moves = String::new();
        for a in &s.history {
            moves += a;
            moves += " submit ";
        }
        let human = if s.human_black { "black" } else { "white" };
        let line = obj![
            ("white", if s.human_black { "Kairos" } else { "Human" }),
            ("black", if s.human_black { "Human" } else { "Kairos" }),
            ("human", human),
            ("variant", s.variant.as_str()),
            ("score", match result { "1-0" => 1.0, "0-1" => 0.0, _ => 0.5 }),
            ("reason", reason),
            ("actions", s.history.len()),
            ("position", s.start.as_str()),
            ("book", 0),
            ("moves", moves.trim()),
            ("started", s.started.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))
        ];
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{}", line);
        }
    }

    /// After an action: game over, or hand the move to the engine.
    fn after_action(&self, st: &mut PlayState) {
        let gid = self.game_id.load(Ordering::SeqCst);
        let Some(s) = st.session.as_mut() else { return };
        let status = tree::status_of(&s.state);
        let mover_black = !s.state.get_present().1;
        if status != Status::Playing {
            let result = match status {
                Status::Checkmate => if mover_black { "0-1" } else { "1-0" },
                Status::WhiteExiled => "0-1",
                Status::BlackExiled => "1-0",
                _ => "1/2-1/2",
            };
            self.finish(s, result, status.name());
            return;
        }
        if s.state.get_present().1 != s.human_black {
            s.thinking = true;
            s.info = Json::Null;
            let limits = match &s.clock {
                Some(c) => {
                    let side = s.state.get_present().1 as usize;
                    Limits { time_left: Some(s.left[side]), increment: Some(c.inc_ms), ..Limits::default() }
                }
                None => Limits { movetime: Some(s.movetime), ..Limits::default() },
            };
            self.post_job(EngineJob { game_id: gid, state: s.state.clone(), limits, ponder: false });
        } else {
            self.post_job(EngineJob { game_id: gid, state: s.state.clone(), limits: Limits::default(), ponder: true });
        }
    }

    fn engine_done(&self, gid: u64, lan: Option<String>, score: Option<i32>) {
        let mut st = self.st.lock().unwrap();
        if self.game_id.load(Ordering::SeqCst) != gid {
            return;
        }
        let Some(s) = st.session.as_mut() else { return };
        if s.over.is_some() {
            return;
        }
        s.thinking = false;
        let Some(lan) = lan else {
            self.after_action(&mut st);
            self.publish(&mut st);
            return;
        };
        let moves = parse_lan_moves(&lan).unwrap_or_default();
        match play_action(&s.state, &moves) {
            Ok(next) => {
                Self::charge_clock(s);
                s.state = next;
                s.history.push(lan);
                s.evals.push(score);
                s.turn_start = Instant::now();
                self.after_action(&mut st);
            }
            Err(e) => {
                let r = if s.human_black { "0-1" } else { "1-0" };
                self.finish(s, r, &format!("engine played an illegal action: {}", e));
            }
        }
        self.publish(&mut st);
    }

    /// Take the mover's elapsed time off their clock and add the increment.
    fn charge_clock(s: &mut Session) {
        if let Some(c) = &s.clock {
            let side = s.state.get_present().1 as usize;
            let used = s.turn_start.elapsed().as_millis() as u64;
            s.left[side] = s.left[side].saturating_sub(used) + c.inc_ms;
        }
    }

    fn handle(&self, req: &Request) -> Response {
        let body = req.json();
        let who = body.get("token").str().or(req.q("token")).map(String::from);
        let mut st = self.st.lock().unwrap();
        // Any contact keeps the player's session and queue place alive.
        if let Some(t) = &who {
            if let Some(s) = st.session.as_mut().filter(|s| &s.token == t) {
                s.last_seen = Instant::now();
            }
            if let Some(q) = st.queue.iter_mut().find(|q| &q.ticket == t) {
                q.last_seen = Instant::now();
            }
        }
        match req.path.as_str() {
            "/api/play/state" => Response::json(Self::public_state(&st, who.as_deref())),
            "/api/play/queue" => {
                if req.method != "POST" {
                    return Response::error(405, "POST only");
                }
                let mine = who.as_deref().is_some_and(|t| st.session.as_ref().is_some_and(|s| s.token == t));
                let queued = who.as_deref().is_some_and(|t| st.queue.iter().any(|q| q.ticket == t));
                let ticket = if mine || queued {
                    who.clone().unwrap()
                } else {
                    let t = new_token();
                    st.queue.push_back(QueueEntry { ticket: t.clone(), last_seen: Instant::now() });
                    t
                };
                self.publish(&mut st);
                let mut r = Self::public_state(&st, Some(&ticket));
                if let Json::Obj(kv) = &mut r {
                    kv.push(("token".into(), ticket.into()));
                }
                Response::json(r)
            }
            "/api/play/start" => {
                if req.method != "POST" {
                    return Response::error(405, "POST only");
                }
                if !Self::can_start(&st, who.as_deref()) {
                    return Response::json(obj![("ok", false), ("error", "busy"), ("state", Self::public_state(&st, who.as_deref()))]);
                }
                let variant = body.get("variant").str().unwrap_or("Standard").to_string();
                if !VARIANTS.contains(&variant.as_str()) {
                    return Response::error(400, "unknown variant");
                }
                let start = match resolve_position(&variant) {
                    Ok(p) => p,
                    Err(e) => return Response::error(400, &e),
                };
                let state = match kairos::core::variants::position_from_uci(&start) {
                    Ok(s) => s,
                    Err(e) => return Response::error(400, &e),
                };
                let human_black = match body.get("color").str() {
                    Some("b") => true,
                    Some("w") => false,
                    _ => new_token().as_bytes()[0] & 1 == 1,
                };
                let clock = body.get("clock");
                let clock = if clock.is_null() {
                    None
                } else {
                    let base = clock.get("base").int().unwrap_or(300_000).clamp(10_000, 3 * 3_600_000) as u64;
                    let inc = clock.get("inc").int().unwrap_or(0).clamp(0, 600_000) as u64;
                    Some(Clock { base_ms: base, inc_ms: inc })
                };
                let movetime = body.get("movetime").int().unwrap_or(2000).clamp(100, 60_000) as u64;
                // Replace any previous game (the caller's own, or a finished one).
                self.end_session(&mut st, "abandoned");
                let token = who.clone().filter(|t| st.queue.iter().any(|q| &q.ticket == t)).unwrap_or_else(new_token);
                st.queue.retain(|q| q.ticket != token);
                let base = clock.as_ref().map(|c| c.base_ms).unwrap_or(0);
                self.game_id.fetch_add(1, Ordering::SeqCst);
                st.session = Some(Session {
                    token: token.clone(),
                    human_black,
                    variant,
                    start,
                    state,
                    history: Vec::new(),
                    clock,
                    left: [base, base],
                    turn_start: Instant::now(),
                    movetime,
                    over: None,
                    ended_at: None,
                    thinking: false,
                    last_seen: Instant::now(),
                    info: Json::Null,
                    evals: Vec::new(),
                    started: SystemTime::now(),
                });
                self.after_action(&mut st);
                self.publish(&mut st);
                let mut r = Self::public_state(&st, Some(&token));
                if let Json::Obj(kv) = &mut r {
                    kv.push(("token".into(), token.into()));
                }
                Response::json(r)
            }
            "/api/play/move" => {
                let Some(s) = st.session.as_mut().filter(|s| Some(&s.token) == who.as_ref()) else {
                    return Response::error(403, "not your game");
                };
                if s.over.is_some() {
                    return Response::error(409, "the game is over");
                }
                if s.state.get_present().1 != s.human_black {
                    return Response::error(409, "not your turn");
                }
                let lan = body.get("lan").str().unwrap_or("");
                let moves = match parse_lan_moves(lan) {
                    Ok(m) if !m.is_empty() => m,
                    Ok(_) => return Response::error(400, "empty action"),
                    Err(e) => return Response::error(400, &e),
                };
                let act = kairos::core::action::Action::from_vector(
                    &moves.iter().filter_map(|m| s.state.normalize_promotion(*m)).collect::<Vec<_>>(),
                    &s.state,
                );
                match play_action(&s.state, act.moves()) {
                    Ok(next) => {
                        Self::charge_clock(s);
                        let lan = tree::action_lan(&s.state, &act);
                        s.state = next;
                        s.history.push(lan);
                        s.turn_start = Instant::now();
                        self.after_action(&mut st);
                        self.publish(&mut st);
                        Response::json(Self::public_state(&st, who.as_deref()))
                    }
                    Err(e) => Response::error(400, &e),
                }
            }
            "/api/play/resign" | "/api/play/leave" => {
                let mine = st.session.as_ref().is_some_and(|s| Some(&s.token) == who.as_ref());
                if mine {
                    let s = st.session.as_mut().unwrap();
                    if s.over.is_none() {
                        let r = if s.human_black { "1-0" } else { "0-1" };
                        self.finish(s, r, if req.path == "/api/play/leave" { "abandoned" } else { "resignation" });
                    }
                    if req.path == "/api/play/leave" {
                        st.session = None;
                    }
                } else if let Some(t) = &who {
                    st.queue.retain(|q| &q.ticket != t);
                }
                self.publish(&mut st);
                Response::json(Self::public_state(&st, who.as_deref()))
            }
            _ => Response::error(404, "unknown endpoint"),
        }
    }

    /// Server-sent events: `state` on every change, `thinking` while the engine searches.
    fn events(&self, req: &Request, stream: &mut TcpStream) {
        let token = req.q("token").map(String::from);
        let (tx, rx) = channel::<String>();
        let id = {
            let mut st = self.st.lock().unwrap();
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            st.subscribers.push((id, tx, token.clone()));
            let first = format!("event: state\ndata: {}\n\n", Self::public_state(&st, token.as_deref()));
            let _ = st.subscribers.last().unwrap().1.send(first);
            id
        };
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\n{}Connection: keep-alive\r\n\r\n",
            ISOLATION
        );
        if stream.write_all(head.as_bytes()).is_err() {
            return;
        }
        loop {
            let msg = match rx.recv_timeout(Duration::from_secs(10)) {
                Ok(m) => {
                    // State events are personalised (you / queue position) per subscriber.
                    if m.starts_with("event: state") {
                        let st = self.st.lock().unwrap();
                        format!("event: state\ndata: {}\n\n", Self::public_state(&st, token.as_deref()))
                    } else {
                        m
                    }
                }
                Err(RecvTimeoutError::Timeout) => ": ping\n\n".to_string(),
                Err(RecvTimeoutError::Disconnected) => break,
            };
            if stream.write_all(msg.as_bytes()).and_then(|_| stream.flush()).is_err() {
                break;
            }
            // An open event stream counts as contact.
            if let Some(t) = &token {
                let mut st = self.st.lock().unwrap();
                if let Some(s) = st.session.as_mut().filter(|s| &s.token == t) {
                    s.last_seen = Instant::now();
                }
                if let Some(q) = st.queue.iter_mut().find(|q| &q.ticket == t) {
                    q.last_seen = Instant::now();
                }
            }
        }
        self.st.lock().unwrap().subscribers.retain(|s| s.0 != id);
    }
}

// ------------------------------------------------------------------ server

struct Server {
    static_dir: PathBuf,
    games: Games,
    play: Arc<Play>,
}

impl Server {
    fn route(&self, req: &Request) -> Response {
        let p = req.path.as_str();
        if p == "/api/info" {
            return Response::json(obj![
                ("ok", true),
                ("name", "kairos"),
                ("version", env!("CARGO_PKG_VERSION")),
                ("games", !self.games.dirs.is_empty()),
                ("play", true)
            ]);
        }
        if p.starts_with("/api/games/") {
            return self.games.handle(req);
        }
        if p.starts_with("/api/play/") {
            return self.play.handle(req);
        }
        if p.starts_with("/api/") {
            return Response::error(404, "unknown endpoint");
        }
        if req.method != "GET" && req.method != "HEAD" {
            return Response::error(405, "method not allowed");
        }
        serve_static(&self.static_dir, req)
    }

    fn connection(&self, stream: TcpStream) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
        let Ok(mut w) = stream.try_clone() else { return };
        let mut r = BufReader::new(stream);
        while let Some(req) = read_request(&mut r) {
            if req.path == "/api/play/events" {
                let _ = w.set_read_timeout(None);
                self.play.events(&req, &mut w);
                return;
            }
            let keep = !req.header("Connection").is_some_and(|c| c.eq_ignore_ascii_case("close"));
            let resp = self.route(&req);
            if write_response(&mut w, &resp, req.method == "HEAD", keep).is_err() || !keep {
                return;
            }
        }
    }
}

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1).cloned())
}

fn newest_mtime(dir: &Path, exts: &[&str], acc: &mut SystemTime) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "target") {
                newest_mtime(&p, exts, acc);
            }
        } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| exts.contains(&x)) {
            if let Ok(m) = e.metadata().and_then(|m| m.modified()) {
                *acc = (*acc).max(m);
            }
        }
    }
}

/// Build web/static/kairos.wasm when it is missing or older than the sources.
fn ensure_wasm(root: &Path) -> Result<(), String> {
    let out = root.join("web/static/kairos.wasm");
    let mut newest = SystemTime::UNIX_EPOCH;
    newest_mtime(&root.join("src"), &["rs"], &mut newest);
    newest_mtime(&root.join("web/wasm"), &["rs", "toml"], &mut newest);
    newest_mtime(&root.join("data"), &["bin"], &mut newest);
    if std::fs::metadata(&out).and_then(|m| m.modified()).is_ok_and(|m| m >= newest) {
        return Ok(());
    }
    eprintln!("building the browser engine (web/wasm → web/static/kairos.wasm)...");
    let manifest = root.join("web/wasm/Cargo.toml");
    let target_dir = root.join("web/wasm/target");
    let status = std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["build", "--release", "--target", "wasm32-wasip1", "--manifest-path"])
        .arg(&manifest)
        .arg("--target-dir")
        .arg(&target_dir)
        // A 32 MB stack for the recursive search; RUSTFLAGS also overrides target-cpu=native.
        .env("RUSTFLAGS", "-C link-arg=-zstack-size=33554432")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .status()
        .map_err(|e| format!("cannot run cargo: {}", e))?;
    if !status.success() {
        return Err("wasm build failed (is the wasm32-wasip1 target installed? `rustup target add wasm32-wasip1`)".into());
    }
    std::fs::copy(target_dir.join("wasm32-wasip1/release/kairos_wasm.wasm"), &out).map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("kairos-web [--port 8080] [--host 127.0.0.1] [--root <repo>] [--games <dir>]... [--hash 2048] [--threads N] [--archive <file>] [--no-wasm-build]");
        return;
    }
    let root = PathBuf::from(arg(&args, "--root").unwrap_or_else(|| env!("CARGO_MANIFEST_DIR").to_string()));
    let port: u16 = arg(&args, "--port").and_then(|p| p.parse().ok()).unwrap_or(8080);
    let host = arg(&args, "--host").unwrap_or_else(|| "127.0.0.1".into());
    let hash: usize = arg(&args, "--hash").and_then(|p| p.parse().ok()).unwrap_or(2048);
    // One core is left to the server and the browser.
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    let threads: usize = arg(&args, "--threads").and_then(|p| p.parse().ok()).unwrap_or(cores.saturating_sub(1)).max(1);
    let mut dirs: Vec<PathBuf> = args.iter().enumerate().filter(|(_, a)| *a == "--games").filter_map(|(i, _)| args.get(i + 1)).map(PathBuf::from).collect();
    if dirs.is_empty() && root.join("games").is_dir() {
        dirs.push(root.join("games"));
    }
    let archive = arg(&args, "--archive").map(PathBuf::from).or_else(|| Some(root.join("games/web/played.jsonl")));
    if !args.iter().any(|a| a == "--no-wasm-build") {
        if let Err(e) = ensure_wasm(&root) {
            eprintln!("warning: {}", e);
        }
    }
    let server = Arc::new(Server {
        static_dir: root.join("web/static"),
        games: Games { dirs: dirs.clone(), index: Mutex::new(HashMap::new()), variants: Mutex::new(HashMap::new()) },
        play: Play::new(hash, threads, archive),
    });
    let listener = TcpListener::bind((host.as_str(), port)).unwrap_or_else(|e| {
        eprintln!("cannot listen on {}:{}: {}", host, port, e);
        std::process::exit(1);
    });
    eprintln!("Kairos web: http://{}:{}/", if host == "0.0.0.0" { "localhost" } else { &host }, port);
    eprintln!("  engine: {} threads, {} MB hash, pondering", threads, hash);
    for d in &dirs {
        eprintln!("  games: {}", d.display());
    }
    for stream in listener.incoming().flatten() {
        let s = server.clone();
        std::thread::spawn(move || s.connection(stream));
    }
}
