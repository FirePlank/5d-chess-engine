// Node smoke test of the WASM build: node tests/web/smoke.mjs [games.jsonl]
import { readFileSync } from 'node:fs';
import { loadCore } from './core.mjs';

const infos = [];
const k = await loadCore({ onInfo: (l) => infos.push(l) });
const line = process.argv[2] ? readFileSync(process.argv[2], 'utf8').split('\n')[0]
  : '{"game":0,"new_first":true,"score":1,"reason":"checkmate","position":"size 8x8 odd fen [k7/p*n6/K7/8/8/8/6P*B/8:0:1:w]","moves":"(0T1)h2g1 submit (0T1)a8b8 submit (0T2)g1d4 submit (0T2)b8c8 submit (0T3)d4c3 submit (0T3)b7(0T1)c7 submit (0T4)c3(0T1)c6 submit (0T4)c8(0T3)b8 (1T1)a8(0T2)a8 submit (1T2)c6(0T1)c6 submit (2T1)a8(1T2)a8 submit (2T2)h2b8 (-1T2)a6(0T3)b5 submit (-1T2)c7(1T1)c7 submit (-4T2)h2c7 submit"}';
let t = performance.now();
const r = k.call({ cmd: 'load', text: line });
if (!r.ok) { console.error(r); process.exit(1); }
console.log('load ms', (performance.now() - t).toFixed(1), r.meta, 'mainline', r.mainline.length);
console.log(r.tree.nodes.slice(1).map((n) => n.san).join(' | '));
t = performance.now();
const last = r.mainline[r.mainline.length - 1];
const v = k.call({ cmd: 'view', node: last, draft: [] }).view;
console.log('view ms', (performance.now() - t).toFixed(1), 'boards', v.boards.length, 'status', v.status, 'checks', v.checks.length, 'branches', JSON.stringify(v.branches));
const v0 = k.call({ cmd: 'view', node: r.mainline[r.mainline.length - 2], draft: [] }).view;
t = performance.now();
const s = k.call({ cmd: 'search', position: v0.position, movetime: 1000 });
console.log('search ms', (performance.now() - t).toFixed(0), s, infos.slice(-2));
