// Engine worker: runs Kairos searches off the main thread.
//
// in:  {type:'init', module, clock?, hashMb, threads, memory}  (memory: the limits of the
//        shared memory the module imports; with threads > 1 a pool of threads - 1 thread
//        workers is started on it)
//      {type:'search', id, position, movetime, depth?, newGame?, resume?, timeLeft?, increment?}
// out: {type:'ready', threads} | {type:'info', id, line} | {type:'done', id, bestmove}
//      {type:'error', id?, message}
import { instantiate, startClock, clockStopped } from './wasi.js';

let kairos = null;
let current = 0;
let clock = null;

/** Start `n` thread workers on `memory`; returns the wasi-threads `thread-spawn` import. */
async function threadPool(n, module, memory, clock) {
  const slots = new Int32Array(new SharedArrayBuffer(4 * Math.max(1, n)));
  const pool = await Promise.all(Array.from({ length: n }, (_, slot) => new Promise((resolve, reject) => {
    const w = new Worker(new URL('./thread-worker.js', import.meta.url), { type: 'module' });
    w.onmessage = (e) => (e.data.type === 'ready' ? resolve(w) : reject(new Error(e.data.message)));
    w.onerror = (e) => reject(new Error(e.message || 'search thread worker failed to start'));
    w.postMessage({ type: 'init', module, memory, clock, slots: slots.buffer, slot });
  })));
  let nextTid = 1;
  return (arg) => {
    for (let i = 0; i < pool.length; i++) {
      if (Atomics.compareExchange(slots, i, 0, 1) === 0) {
        const tid = nextTid++;
        pool[i].postMessage({ type: 'start', tid, arg });
        return tid;
      }
    }
    return -6; // EAGAIN: the search goes on without that helper.
  };
}

self.onmessage = async (e) => {
  const msg = e.data;
  try {
    if (msg.type === 'init') {
      const memory = new WebAssembly.Memory({ initial: msg.memory.initial, maximum: msg.memory.maximum, shared: true });
      const threads = Math.max(1, msg.threads | 0);
      const threadSpawn = threads > 1 ? await threadPool(threads - 1, msg.module, memory, msg.clock) : () => -6;
      clock = msg.clock;
      kairos = await instantiate(msg.module, {
        clock: msg.clock,
        memory,
        threadSpawn,
        onInfo: (line) => self.postMessage({ type: 'info', id: current, line }),
      });
      kairos.call({ cmd: 'set_hash', mb: msg.hashMb || 64 });
      kairos.call({ cmd: 'set_option', name: 'Threads', value: threads });
      self.postMessage({ type: 'ready', threads });
    } else if (msg.type === 'search') {
      current = msg.id;
      // Stopped before it began (the page moved on while an earlier search ran): skip it.
      if (clockStopped(clock, msg.id)) {
        self.postMessage({ type: 'done', id: msg.id, bestmove: null });
        return;
      }
      startClock(clock, msg.id);
      const r = kairos.call({
        cmd: 'search',
        position: msg.position,
        movetime: msg.movetime,
        depth: msg.depth,
        newGame: msg.newGame,
        resume: msg.resume,
        timeLeft: msg.timeLeft,
        increment: msg.increment,
      });
      if (r.ok) self.postMessage({ type: 'done', id: msg.id, bestmove: r.bestmove });
      else self.postMessage({ type: 'error', id: msg.id, message: r.error });
    }
  } catch (err) {
    self.postMessage({ type: 'error', id: msg.id, message: String((err && err.message) || err), fatal: true });
  }
};
