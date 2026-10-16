// The game browser's requests (api/games/files, list, get). kairos-web answers them; the
// static site answers them here, from the record files bundled with it (games/files.json,
// written by scripts/build_pages.sh) and the files the visitor opens, with the same replies.

let files = null;
const opened = new Map(); // id -> text of a file the visitor opened
const indexes = new Map(); // id -> {games, lines}
let rules = null;

/** The rules instance used to name variants. */
export function setRules(r) { rules = r; }

/** GET `path` (`api/...`): from kairos-web when there is one, else from the bundled files. */
export async function apiGet(app, path) {
  if (app.server) return fetch(path).then((r) => r.json());
  try {
    return await staticGet(path);
  } catch (e) {
    return { ok: false, error: e.message };
  }
}

/** Add a record file the visitor opened; returns its id. */
export async function openFile(name, text) {
  await fileList();
  const id = 'opened:' + name;
  opened.set(id, text);
  indexes.delete(id);
  files = files.filter((f) => f.id !== id);
  files.push({ id, name, dir: 'opened', size: text.length, mtime: Date.now() / 1000 });
  return id;
}

async function fileList() {
  if (!files) {
    try {
      const r = await fetch('games/files.json');
      files = r.ok ? (await r.json()).files || [] : [];
    } catch (e) {
      files = [];
    }
  }
  return files;
}

async function fileText(id) {
  if (opened.has(id)) return opened.get(id);
  if (!(await fileList()).some((f) => f.id === id)) throw new Error('no such file');
  const r = await fetch('games/' + id.split('/').map(encodeURIComponent).join('/'));
  if (!r.ok) throw new Error(`games/${id}: HTTP ${r.status}`);
  return r.text();
}

const variantNames = new Map();
function variantOf(position) {
  if (!variantNames.has(position)) {
    let name = 'Custom';
    try { name = rules.call({ cmd: 'variant', position }).variant; } catch (e) { name = 'Invalid'; }
    variantNames.set(position, name);
  }
  return variantNames.get(position);
}

/** White's result from a record's score (as kairos-web's result_of). */
function resultOf(j) {
  if (typeof j.score !== 'number') return null;
  const ws = j.new_first == null ? j.score : j.new_first ? j.score : 1 - j.score;
  return ws > 0.75 ? '1-0' : ws < 0.25 ? '0-1' : '1/2-1/2';
}

async function index(id) {
  if (indexes.has(id)) return indexes.get(id);
  const text = await fileText(id);
  let ix;
  if (/\.5dpgn$/i.test(id)) {
    ix = { lines: [text], games: [{ variant: '5dpgn', result: null, reason: '', actions: 0, book: 0, game: null, pair: null, newFirst: null }] };
  } else {
    ix = { lines: [], games: [] };
    for (const line of text.split('\n')) {
      if (!line.trimStart().startsWith('{')) continue;
      let j;
      try { j = JSON.parse(line); } catch (e) { continue; }
      ix.lines.push(line.trim());
      ix.games.push({
        variant: variantOf(j.position || 'startpos'),
        result: resultOf(j),
        reason: j.reason || '',
        actions: (j.moves || '').split(/\s+/).filter((t) => t === 'submit').length,
        book: j.book || 0,
        game: j.game ?? null,
        pair: j.pair ?? null,
        newFirst: j.new_first ?? null,
      });
    }
  }
  indexes.set(id, ix);
  return ix;
}

function facet(counts) {
  return [...counts].sort((a, b) => b[1] - a[1]);
}

async function staticGet(path) {
  const [route, qs] = path.split('?');
  const q = new URLSearchParams(qs || '');
  if (route === 'api/games/files') return { ok: true, files: await fileList() };
  if (route === 'api/games/get') {
    const ix = await index(q.get('file'));
    const i = +q.get('index') || 0;
    if (!ix.lines[i]) throw new Error('no such game');
    return { ok: true, text: ix.lines[i], count: ix.games.length };
  }
  if (route === 'api/games/list') {
    const ix = await index(q.get('file'));
    const variant = q.get('variant'), result = q.get('result'), reason = q.get('reason');
    const min = +(q.get('min') || 0), max = q.get('max') ? +q.get('max') : Infinity;
    const offset = +(q.get('offset') || 0), limit = Math.min(1000, +(q.get('limit') || 100));
    let rows = ix.games.map((g, i) => ({ ...g, index: i })).filter((g) =>
      (!variant || g.variant === variant)
      && (!result || g.result === result || (result === 'decisive' && (g.result === '1-0' || g.result === '0-1')))
      && (!reason || g.reason.startsWith(reason))
      && g.actions >= min && g.actions <= max);
    if (q.get('sort') === 'long') rows.sort((a, b) => b.actions - a.actions);
    else if (q.get('sort') === 'short') rows.sort((a, b) => a.actions - b.actions);
    const variants = new Map(), reasons = new Map(), results = new Map();
    const inc = (m, k) => m.set(k, (m.get(k) || 0) + 1);
    for (const g of ix.games) {
      inc(variants, g.variant);
      inc(reasons, g.reason.split(/[:(]/)[0].trim());
      inc(results, g.result || '?');
    }
    return {
      ok: true, total: rows.length, count: ix.games.length, games: rows.slice(offset, offset + limit),
      variants: facet(variants), reasons: facet(reasons), results: facet(results),
    };
  }
  throw new Error('unknown endpoint');
}
