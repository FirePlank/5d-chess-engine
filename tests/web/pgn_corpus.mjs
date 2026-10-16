// Load every 5dpgn file of a directory tree through the WASM viewer core, view every
// main-line position and report failures and the slowest files:
//   node tests/web/pgn_corpus.mjs <dir>
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { loadCore } from './core.mjs';

const k = await loadCore();
const files = [];
const walk = (d) => { for (const f of readdirSync(d)) { const p = join(d, f); if (statSync(p).isDirectory()) walk(p); else if (p.endsWith('.5dpgn')) files.push(p); } };
walk(process.argv[2]);
let ok = 0, actions = 0;
const slow = [], bad = [];
for (const f of files) {
  const t = performance.now();
  const r = k.call({ cmd: 'load', text: readFileSync(f, 'utf8') });
  if (!r.ok) { bad.push(`${f}: ${r.error.split('\n')[0]}`); continue; }
  for (const id of r.mainline) {
    const v = k.call({ cmd: 'view', node: id, draft: [] });
    if (!v.ok) { bad.push(`${f} node ${id}: ${v.error}`); break; }
  }
  ok++;
  actions += r.meta.actions;
  slow.push([performance.now() - t, f]);
}
slow.sort((a, b) => b[0] - a[0]);
console.log(`${ok}/${files.length} files, ${actions} actions; slowest:`, slow.slice(0, 3).map(([ms, f]) => `${f.split(/[\/]/).pop()} ${ms.toFixed(0)} ms`).join(', '));
for (const b of bad) console.log('FAIL', b);
