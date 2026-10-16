// One search thread of the multi-threaded engine (wasi-threads). The engine worker starts a
// pool of these before any search: a worker busy in a synchronous search cannot wait for new
// workers to load. Each `start` runs one Rust thread (`wasi_thread_start`) on the engine's
// shared memory, then frees this worker's pool slot.
import { instantiateThread } from './wasi.js';

let ctx = null;

self.onmessage = (e) => {
  const m = e.data;
  if (m.type === 'init') {
    ctx = { module: m.module, memory: m.memory, clock: m.clock, slots: new Int32Array(m.slots), slot: m.slot };
    self.postMessage({ type: 'ready' });
  } else if (m.type === 'start') {
    try {
      instantiateThread(ctx.module, ctx).wasi_thread_start(m.tid, m.arg);
    } catch (err) {
      console.error('kairos search thread', m.tid, err);
    } finally {
      Atomics.store(ctx.slots, ctx.slot, 0);
    }
  }
};
