// Review games Kairos lost: for each of Kairos's actions, search the position before it (Kairos to
// move) and after it (the opponent to move) with a long, multi-threaded search, and report what
// the action threw away: the score Kairos could have had minus the score its action left.
//
//   node scripts/loss_review.mjs <games.json> <engine.json> [ms per search] [threads] [ids...]
//
// games.json: the site's `games` rows (id, position, moves[].lan); engine.json: its `engine_games`
// rows (game_id, color: Kairos's side 1 White / 2 Black). Writes one JSON line per action to stdout.
import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';

const [gamesPath, enginePath, msArg = '8000', threadsArg = '8', ...only] = process.argv.slice(2);
const games = JSON.parse(readFileSync(gamesPath, 'utf8'));
const engines = JSON.parse(readFileSync(enginePath, 'utf8'));
const exe = new URL('../target/release/kairos.exe', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');

const proc = spawn(exe, [], { stdio: ['pipe', 'pipe', 'inherit'] });
let buf = '';
let waiters = [];
proc.stdout.on('data', (d) => {
  buf += d;
  let i;
  while ((i = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, i).trim();
    buf = buf.slice(i + 1);
    for (const w of waiters) w(line);
  }
});
const send = (s) => proc.stdin.write(s + '\n');
const until = (pred) => new Promise((ok) => {
  const w = (line) => { if (pred(line)) { waiters = waiters.filter((x) => x !== w); ok(line); } };
  waiters.push(w);
});

/** Search `position` for `ms`: {score (side to move's, cp; mates as ±30000 - plies), depth, best}. */
async function search(position, ms) {
  let last = null;
  const info = (line) => { if (/^info .*\bdepth\b/.test(line) && /\bscore\b/.test(line)) last = line; };
  waiters.push(info);
  send(`position ${position}`);
  send(`go movetime ${ms}`);
  const done = await until((l) => l.startsWith('bestmove') || l.startsWith('nobestmove'));
  waiters = waiters.filter((x) => x !== info);
  let score = null, depth = null;
  if (last) {
    const m = last.match(/score (cp|mate) (-?\d+)/);
    if (m) score = m[1] === 'cp' ? +m[2] : (+m[2] > 0 ? 30000 - +m[2] : -30000 - +m[2]);
    const d = last.match(/\bdepth (\d+)/);
    if (d) depth = +d[1];
  }
  return { score, depth, best: done.startsWith('bestmove') ? done.slice(9) : null };
}

send('5duci');
await until((l) => l === '5duciok');
send(`setoption name Threads value ${threadsArg}`);
send('setoption name Hash value 256');
send('isready');
await until((l) => l === 'readyok');

const ms = +msArg;
for (const g of games) {
  if (only.length && !only.includes(g.id)) continue;
  const e = engines.find((x) => x.game_id === g.id);
  if (!e) continue;
  const lans = g.moves.map((m) => m.lan);
  // The side to move at ply 0 (the start position's), from the position text's last board.
  const startBlack = /:b\]\s*$/.test(g.position.trim()) && !/:w\]/.test(g.position.split(']').slice(-2).join(']'));
  const at = (n) => `${g.position} moves ${lans.slice(0, n).map((l) => `${l} submit`).join(' ')}`.replace(/ moves $/, '');
  send('5ducinewgame');
  for (let k = 0; k < lans.length; k++) {
    const moverWhite = (k % 2 === 0) !== startBlack;
    if ((moverWhite ? 1 : 2) !== e.color) continue;
    const before = await search(at(k), ms);
    const after = await search(at(k + 1), ms);
    const left = after.score == null ? null : -after.score;
    console.log(JSON.stringify({
      game: g.id, ply: k, played: lans[k], best: before.best, before: before.score, depthBefore: before.depth,
      left, depthAfter: after.depth, lost: before.score == null || left == null ? null : before.score - left,
    }));
  }
}
send('quit');
