// The games you finished against Kairos, kept in this browser (IndexedDB; localStorage where
// IndexedDB is unavailable, as in some private windows). One record per game, shaped as the
// SPRT/kairos-web JSONL records (position, moves, white/black, score, reason) so the analysis
// board loads it as it is, plus when and how it was played.

const DB = 'kairos';
const STORE = 'games';
const FALLBACK = 'kairos.games';

let dbPromise = null;

/** The IndexedDB database, or null where it cannot be opened. */
function db() {
  if (!dbPromise) {
    dbPromise = new Promise((resolve) => {
      try {
        const req = indexedDB.open(DB, 1);
        req.onupgradeneeded = () => req.result.createObjectStore(STORE, { keyPath: 'id', autoIncrement: true });
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => resolve(null);
        req.onblocked = () => resolve(null);
      } catch (e) {
        resolve(null);
      }
    });
  }
  return dbPromise;
}

function tx(d, mode, fn) {
  return new Promise((resolve, reject) => {
    const t = d.transaction(STORE, mode);
    const req = fn(t.objectStore(STORE));
    t.oncomplete = () => resolve(req && req.result);
    t.onerror = () => reject(t.error);
    t.onabort = () => reject(t.error);
  });
}

function readFallback() {
  try { return JSON.parse(localStorage.getItem(FALLBACK) || '[]'); } catch (e) { return []; }
}
function writeFallback(list) {
  try { localStorage.setItem(FALLBACK, JSON.stringify(list)); } catch (e) { /* storage full or blocked */ }
}

/** Store a finished game; returns its id. */
export async function saveGame(rec) {
  const d = await db();
  if (d) return tx(d, 'readwrite', (s) => s.add(rec));
  const list = readFallback();
  const id = list.reduce((m, g) => Math.max(m, g.id), 0) + 1;
  list.push({ ...rec, id });
  writeFallback(list);
  return id;
}

/** Every stored game, newest first. */
export async function listGames() {
  const d = await db();
  const list = d ? await tx(d, 'readonly', (s) => s.getAll()) : readFallback();
  return (list || []).sort((a, b) => (b.ended || 0) - (a.ended || 0));
}

export async function getGame(id) {
  const d = await db();
  if (d) return tx(d, 'readonly', (s) => s.get(id));
  return readFallback().find((g) => g.id === id) || null;
}

/** Store `rec` over the game with its id (a saved analysis updated in place). */
export async function putGame(rec) {
  const d = await db();
  if (d) return tx(d, 'readwrite', (s) => s.put(rec));
  writeFallback(readFallback().map((g) => (g.id === rec.id ? rec : g)));
  return rec.id;
}

/** Change some fields of a stored game (name, players, notes). */
export async function updateGame(id, patch) {
  const g = await getGame(id);
  if (!g) throw new Error('no such game');
  return putGame({ ...g, ...patch, id });
}

export async function deleteGame(id) {
  const d = await db();
  if (d) return tx(d, 'readwrite', (s) => s.delete(id));
  writeFallback(readFallback().filter((g) => g.id !== id));
}

export async function clearGames() {
  const d = await db();
  if (d) return tx(d, 'readwrite', (s) => s.clear());
  writeFallback([]);
}

/** A stored game as a JSONL record line, for the analysis board (and export). */
export function recordLine(g) {
  return JSON.stringify({
    kind: g.kind || 'played',
    name: g.name || undefined,
    notes: g.notes || undefined,
    white: g.white,
    black: g.black,
    human: g.human,
    variant: g.variant,
    position: g.position,
    moves: g.moves.map((m) => `${m} submit`).join(' '),
    score: g.score,
    reason: g.reason,
    actions: g.moves.length,
    timeControl: g.timeControl,
    openingBook: g.openingBook,
    engine: g.engine,
    started: Math.round((g.started || 0) / 1000),
    ended: Math.round((g.ended || 0) / 1000),
    // A saved analysis: its whole tree (variations, comments) and where it came from.
    tree: g.tree || undefined,
    base: g.base || undefined,
  });
}
