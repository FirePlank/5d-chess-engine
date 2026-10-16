// The Kairos core in the browser: a synchronous rules/game-tree instance on the main thread
// and an engine controller around a worker. There is one build (kairos.wasm, from `cargo wasm`,
// wasm32-wasip1-threads): it always runs on shared memory, and the engine also runs its Lazy SMP
// helper threads in Web Workers when the page is cross-origin isolated (the kairos-web server
// sends COOP/COEP), which is what lets workers share that memory. Otherwise it uses one thread.
import { instantiate, importedMemory, makeClock, stopClock } from './wasi.js';

let modulePromise = null;

/** Compile kairos.wasm once: {module, memory} (the limits of the shared memory it imports). */
export function loadModule() {
  if (!modulePromise) {
    modulePromise = (async () => {
      const resp = await fetch('kairos.wasm');
      if (!resp.ok) throw new Error(`kairos.wasm: HTTP ${resp.status} (build it with: cargo wasm)`);
      const bytes = await resp.arrayBuffer();
      return { module: await WebAssembly.compile(bytes), memory: importedMemory(bytes) };
    })();
  }
  return modulePromise;
}

/** A shared memory with the module's limits. */
export function newMemory(desc) {
  try {
    return new WebAssembly.Memory({ initial: desc.initial, maximum: desc.maximum, shared: true });
  } catch (e) {
    throw new Error('This browser cannot give the Kairos core shared WebAssembly memory here: ' + e.message);
  }
}

/** Threads the browser can use: workers can share memory only when cross-origin isolated. */
export const threadsSupported = typeof SharedArrayBuffer !== 'undefined' && !!self.crossOriginIsolated;

/**
 * Default engine settings: all logical cores but one, at most 8 threads (past about that many,
 * most CPUs' extra threads share a core and slow the search down), 64 MB of hash, depth 15.
 */
export function defaultEngineSettings() {
  const cores = navigator.hardwareConcurrency || 2;
  return { threads: threadsSupported ? Math.max(1, Math.min(8, cores - 1)) : 1, hashMb: 64, depth: 15 };
}

/** Rules and game tree (one game at a time). Calls throw on `ok: false`. */
export class Rules {
  static async create() {
    const r = new Rules();
    r.core = await loadModule();
    r.inst = await instantiate(r.core.module, { memory: newMemory(r.core.memory) });
    return r;
  }
  raw(req) {
    try {
      return this.inst.call(req);
    } catch (e) {
      // A trap leaves the instance unusable: start a fresh one (the caller reloads its game).
      console.error('kairos trapped on', req, e);
      this.broken = true;
      throw new Error('The rules engine crashed on this request: ' + (e.message || e));
    }
  }
  call(req) {
    const r = this.raw(req);
    if (!r.ok) throw new Error(r.error);
    return r;
  }
  async reset() {
    this.inst = await instantiate(this.core.module, { memory: newMemory(this.core.memory) });
    this.broken = false;
  }
}

/** Score (cp/mate on the reported scale, and wdl when sent) from a 5DUCI info line, for White. */
export function parseInfo(line, blackToMove) {
  const t = line.trim().split(/\s+/);
  const info = {};
  for (let i = 0; i < t.length; i++) {
    const k = t[i];
    if (['depth', 'nodes', 'nps', 'time', 'roots', 'seldepth'].includes(k)) { info[k] = Number(t[++i]); }
    else if (k === 'score') {
      const kind = t[++i], v = Number(t[++i]);
      if (kind === 'mate') info.mate = blackToMove ? -v : v;
      else info.cp = blackToMove ? -v : v;
    } else if (k === 'wdl') {
      const w = [Number(t[i + 1]), Number(t[i + 2]), Number(t[i + 3])];
      i += 3;
      if (w.every(Number.isFinite)) info.wdl = blackToMove ? [w[2], w[1], w[0]] : w;
    } else if (k === 'pv') { info.pv = t.slice(i + 1).join(' '); break; }
  }
  return info;
}

/**
 * Engine controller: one persistent worker, a request id on every search, a shared stop flag
 * when the page is cross-origin isolated (otherwise the worker is replaced to stop it).
 */
export class Engine {
  constructor({ hashMb = 64, threads = 1 } = {}) {
    this.hashMb = hashMb;
    this.threads = threads;
    this.threadsActive = 1;
    this.reqId = 0;
    this.listeners = new Set();
    this.ready = null;
    this.busy = false;
    this.shared = typeof SharedArrayBuffer !== 'undefined' && self.crossOriginIsolated;
  }
  on(fn) { this.listeners.add(fn); return () => this.listeners.delete(fn); }
  emit(ev) { for (const fn of this.listeners) fn(ev); }

  /** New thread count and hash size; the worker restarts on the next search. */
  configure({ threads, hashMb }) {
    if (threads === this.threads && hashMb === this.hashMb) return;
    this.threads = threads;
    this.hashMb = hashMb;
    this.restart();
  }

  async start() {
    if (this.ready) return this.ready;
    this.ready = (async () => {
      const core = await loadModule();
      this.worker = new Worker(new URL('./engine-worker.js', import.meta.url), { type: 'module' });
      this.clock = this.shared ? makeClock() : null;
      const threads = await new Promise((resolve, reject) => {
        this.worker.onmessage = (e) => {
          if (e.data.type === 'ready') resolve(e.data.threads);
          else if (e.data.type === 'error') reject(new Error(e.data.message));
        };
        this.worker.onerror = (e) => reject(new Error(e.message || 'engine worker failed to start'));
        const threads = threadsSupported ? this.threads : 1;
        this.worker.postMessage({ type: 'init', module: core.module, clock: this.clock, hashMb: this.hashMb, threads, memory: core.memory });
      });
      this.threadsActive = threads;
      this.worker.onmessage = (e) => this.onMessage(e.data);
      this.worker.onerror = (e) => this.emit({ type: 'error', message: e.message || 'engine worker error', fatal: true });
      this.emit({ type: 'ready', threads });
    })();
    try {
      await this.ready;
    } catch (e) {
      this.restart();
      throw e;
    }
    return this.ready;
  }

  onMessage(m) {
    if (m.id !== undefined && m.id !== this.reqId) return; // a stale search
    if (m.type === 'done' || m.type === 'error') this.busy = false;
    if (m.type === 'error' && m.fatal) this.restart();
    this.emit(m);
  }

  /**
   * Start a search; a running one is stopped first. Returns the request id. On a clock, pass
   * `timeLeft`/`increment` (ms) and `movetime: null`. `book`: answer a book position from the
   * opening book at once (the engine's `OwnBook`; Play only).
   */
  async search({ position, movetime = 86_400_000, depth, newGame = false, resume = false, timeLeft, increment, book = false }) {
    await this.start();
    this.stop();
    const id = ++this.reqId;
    this.busy = true;
    this.worker.postMessage({ type: 'search', id, position, movetime, depth, newGame, resume, timeLeft, increment, book });
    return id;
  }

  stop() {
    if (!this.busy) return;
    if (this.clock) {
      stopClock(this.clock, this.reqId);
    } else {
      // No shared memory: a search cannot be interrupted, so replace the worker.
      this.restart();
    }
    this.busy = false;
    this.reqId++;
  }

  restart() {
    // Terminating the engine worker also ends its search-thread workers.
    if (this.worker) this.worker.terminate();
    this.worker = null;
    this.ready = null;
    this.busy = false;
  }
}
