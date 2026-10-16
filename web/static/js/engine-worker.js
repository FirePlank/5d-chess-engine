// Engine worker: runs Kairos searches off the main thread.
//
// in:  {type:'init', module, stopBuffer?, hashMb}
//      {type:'search', id, position, movetime, depth?, newGame?}
// out: {type:'ready'} | {type:'info', id, line} | {type:'done', id, bestmove}
//      {type:'error', id?, message}
import { instantiate } from './wasi.js';

let kairos = null;
let current = 0;

self.onmessage = async (e) => {
  const msg = e.data;
  try {
    if (msg.type === 'init') {
      const stopFlag = msg.stopBuffer ? new Int32Array(msg.stopBuffer) : null;
      kairos = await instantiate(msg.module, {
        stopFlag,
        onInfo: (line) => self.postMessage({ type: 'info', id: current, line }),
      });
      kairos.call({ cmd: 'set_hash', mb: msg.hashMb || 64 });
      self.postMessage({ type: 'ready' });
    } else if (msg.type === 'search') {
      current = msg.id;
      const r = kairos.call({
        cmd: 'search',
        position: msg.position,
        movetime: msg.movetime,
        depth: msg.depth,
        newGame: msg.newGame,
      });
      if (r.ok) self.postMessage({ type: 'done', id: msg.id, bestmove: r.bestmove });
      else self.postMessage({ type: 'error', id: msg.id, message: r.error });
    }
  } catch (err) {
    self.postMessage({ type: 'error', id: msg.id, message: String(err && err.message || err), fatal: true });
  }
};
