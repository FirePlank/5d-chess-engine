// Replay SPRT game records through the WASM viewer core and cross-check them:
//   node tests/web/records.mjs <games.jsonl>... [--max N]
// Every line must load; its action count must match the record; book/eval tags must line up
// with the record's fields; a "checkmate"/"stalemate" ending must be the final position's status.
import { readFileSync } from 'node:fs';
import { instantiate } from '../../web/static/js/wasi.js';

const args = process.argv.slice(2);
const maxIdx = args.indexOf('--max');
const max = maxIdx >= 0 ? +args.splice(maxIdx, 2)[1] : Infinity;
const module = await WebAssembly.compile(readFileSync(new URL('../../web/static/kairos.wasm', import.meta.url)));
const k = await instantiate(module);
let total = 0, failed = 0;
const t0 = performance.now();
for (const file of args) {
  const lines = readFileSync(file, 'utf8').split('\n').filter((l) => l.trim().startsWith('{'));
  let n = 0;
  for (const line of lines) {
    if (n++ >= max) break;
    total++;
    const j = JSON.parse(line);
    const fail = (why) => { failed++; if (failed <= 20) console.log(`FAIL ${file}#${n - 1} game ${j.game}: ${why}`); };
    const r = k.call({ cmd: 'load', text: line });
    if (!r.ok) { fail(r.error); continue; }
    const acts = j.moves.split(/\s+/).filter((t) => t === 'submit').length;
    if (r.meta.actions !== acts) { fail(`${r.meta.actions} actions, record has ${acts}`); continue; }
    const nodes = r.mainline.slice(1).map((id) => r.tree.nodes[id]);
    const book = j.book ?? (j.evals ? Math.max(0, acts - j.evals.length) : 0);
    if (nodes.some((nd, i) => nd.book !== i < book)) { fail('book flags'); continue; }
    // Records without eval_scale hold internal units; the viewer shows them on the reported
    // scale (x 100 / 275, kairos::engine::REPORT_SCALE). Mates (+-20000) are kept.
    const rep = (e) => (e == null ? null : j.eval_scale || Math.abs(e) >= 20000 ? e : Math.sign(e) * Math.round(Math.abs(e * 100) / 275));
    if (j.evals && nodes.slice(book).some((nd, i) => (nd.eval ?? null) !== rep(j.evals[i]))) { fail('evals'); continue; }
    // LAN round trip: the tree's LAN must be the record's tokens.
    const recActs = j.moves.split('submit').map((s) => s.trim()).filter(Boolean);
    const sameLan = (a, b) => a.split(/\s+/).sort().join(' ') === b.split(/\s+/).sort().join(' ');
    const bad = nodes.findIndex((nd, i) => !sameLan(nd.lan, recActs[i]));
    if (bad >= 0) { fail(`action ${bad + 1}: lan ${nodes[bad].lan} vs ${recActs[bad]}`); continue; }
    const last = r.mainline[r.mainline.length - 1];
    const v = k.call({ cmd: 'view', node: last, draft: [] }).view;
    if (j.reason === 'checkmate' && v.status !== 'checkmate') { fail(`ends in checkmate, status ${v.status}`); continue; }
    if (j.reason === 'stalemate' && v.status !== 'stalemate') { fail(`ends in stalemate, status ${v.status}`); continue; }
    if (j.reason === 'checkmate') {
      // The loser is the side to move; the record's score must agree.
      const whiteScore = j.new_first ? j.score : 1 - j.score;
      if ((v.player === 1) !== (whiteScore === 1)) { fail('checkmate winner disagrees with score'); continue; }
    }
  }
}
console.log(`${total - failed}/${total} records OK in ${((performance.now() - t0) / 1000).toFixed(1)} s`);
process.exit(failed ? 1 : 0);
