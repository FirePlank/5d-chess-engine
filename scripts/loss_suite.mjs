// Solve times on positions where Kairos lost a game on the site: one thread (deterministic), each
// completed depth's best action; a position is solved once the search leaves the action that
// lost the game for good. Prints the nodes (and depth) at which each position is solved.
//
//   node scripts/loss_suite.mjs <games.json> <exe> [ms per position] [ENV=VALUE...]
import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';

// [game, ply, the action played there (it lost the game)]
const SUITE = [
  ['v4jkO2', 21, '(0T11)e6e5'],
  ['XfEGIW', 33, '(-1T12)b8(1T12)c8'],
  ['Ps578j', 18, '(0T10)f5(0T8)f7'],
  ['Ps578j', 36, '(-1T9)c2c3 (1T9)d3f5'],
  ['Ps578j', 38, '(-1T10)h2h3 (1T10)e1f1'],
];

const [gamesPath, exe, msArg = '60000', ...envArgs] = process.argv.slice(2);
const games = JSON.parse(readFileSync(gamesPath, 'utf8'));
const env = { ...process.env, ...Object.fromEntries(envArgs.map((a) => a.split('='))) };
const same = (a, b) => a.trim().split(/\s+/).sort().join(' ') === b.trim().split(/\s+/).sort().join(' ');

async function solve([id, ply, bad]) {
  const g = games.find((x) => x.id === id);
  const lans = g.moves.map((m) => m.lan).slice(0, ply);
  const p = spawn(exe, [], { stdio: ['pipe', 'pipe', 'inherit'], env });
  const depths = [];
  await new Promise((done) => {
    let buf = '';
    p.stdout.on('data', (d) => {
      buf += d;
      let i;
      while ((i = buf.indexOf('\n')) >= 0) {
        const line = buf.slice(0, i).trim();
        buf = buf.slice(i + 1);
        const m = line.match(/\bdepth (\d+)\b.*?\bnodes (\d+).*?\bpv (.*)$/);
        if (m && !/upperbound|lowerbound/.test(line)) {
          const first = m[3].split(' submit')[0];
          depths.push({ depth: +m[1], nodes: +m[2], best: first });
        }
        if (line.startsWith('bestmove') || line.startsWith('nobestmove')) { p.stdin.write('quit\n'); done(); }
      }
    });
    const w = (s) => p.stdin.write(s + '\n');
    w('5duci');
    w('setoption name Threads value 1');
    w('setoption name Hash value 256');
    w(`position ${g.position} moves ${lans.map((l) => `${l} submit`).join(' ')}`);
    w(`go movetime ${msArg}`);
  });
  // Solved at the first depth from which no later depth plays the losing action again.
  let at = null;
  for (let k = depths.length - 1; k >= 0 && !same(depths[k].best, bad); k--) at = depths[k];
  return { id, ply, solved: at ? `depth ${at.depth} at ${at.nodes} nodes` : 'never', last: depths.at(-1) };
}

for (const pos of SUITE.filter((p) => !process.env.ONLY || p[0] === process.env.ONLY)) {
  const r = await solve(pos);
  console.log(`${r.id} ply ${r.ply}: ${r.solved} (reached depth ${r.last?.depth}, ${r.last?.nodes} nodes, best ${r.last?.best})`);
}
