// Trace a search depth by depth: for one position of a site game, print each completed depth's
// score and the best action, to see when (if ever) the search finds the refutation.
//
//   node scripts/depth_trace.mjs <games.json> <game id> <ply> [ms] [threads]
import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';

const [gamesPath, id, plyArg, msArg = '30000', threadsArg = '8'] = process.argv.slice(2);
const g = JSON.parse(readFileSync(gamesPath, 'utf8')).find((x) => x.id === id);
const lans = g.moves.map((m) => m.lan).slice(0, +plyArg);
const exe = new URL('../target/release/kairos.exe', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');
const p = spawn(exe, [], { stdio: ['pipe', 'pipe', 'inherit'] });
let buf = '';
p.stdout.on('data', (d) => {
  buf += d;
  let i;
  while ((i = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, i).trim();
    buf = buf.slice(i + 1);
    const m = line.match(/\bdepth (\d+)\b.*?\bscore (cp|mate) (-?\d+).*?\bnodes (\d+).*?\bpv (.*)$/);
    if (m) console.log(`depth ${m[1].padStart(2)}  ${m[2]} ${m[3].padStart(6)}  nodes ${m[4].padStart(10)}  ${m[5].split(' ').slice(0, 6).join(' ')}`);
    if (line.startsWith('bestmove') || line.startsWith('nobestmove')) { console.log(line); p.stdin.write('quit\n'); }
  }
});
const w = (s) => p.stdin.write(s + '\n');
w('5duci');
w(`setoption name Threads value ${threadsArg}`);
w('setoption name Hash value 256');
w(`position ${g.position}${lans.length ? ' moves ' + lans.map((l) => `${l} submit`).join(' ') : ''}`);
w(`go movetime ${msArg}`);
