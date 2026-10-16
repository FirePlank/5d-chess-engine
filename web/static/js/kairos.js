// The Kairos core in the browser: a synchronous rules/game-tree instance on the main thread
// and an engine controller around a worker. When the page is cross-origin isolated (the
// kairos-web server sends COOP/COEP), the engine is the multi-threaded build (kairos-mt.wasm,
// Lazy SMP helpers in Web Workers on shared memory); otherwise the single-threaded one.
import { instantiate, importedMemory, makeClock, stopClock } from './wasi.js';

const modules = new Map();

async function compile(file, withMemory) {
  const resp = await fetch(file);
  if (!resp.ok) throw new Error(`${file}: HTTP ${resp.status} (build it with the kairos-web server)`);
  const bytes = await resp.arrayBuffer();
  return { module: await WebAssembly.compile(bytes), memory: withMemory ? importedMemory(bytes) : null };
}

/** Compile kairos.wasm once; the module is shared with the worker. */
export function loadModule() {
  if (!modules.has('st')) modules.set('st', compile('kairos.wasm', false).then((m) => m.module));
  return modules.get('st');
}

/** Threads the browser can use: shared memory needs cross-origin isolation. */
export const threadsSupported = typeof SharedArrayBuffer !== 'undefined' && !!self.crossOriginIsolated;

/** The multi-threaded build, or null when unavailable (not isolated, or not built). */
export function loadThreadedModule() {
  if (!threadsSupported) return Promise.resolve(null);
  if (!modules.has('mt')) {
    modules.set('mt', compile('kairos-mt.wasm', true).then((m) => (m.memory && m.memory.shared ? m : null)).catch((e) => {
      console.warn('multi-threaded engine unavailable:', e.message);
      return null;
    }));
  }
  return modules.get('mt');
}

/** Default engine settings: all cores but one (at most 16 threads), 64 MB of hash. */
export function defaultEngineSettings() {
  const cores = navigator.hardwareConcurrency || 2;
  return { threads: threadsSupported ? Math.max(1, Math.min(16, cores - 1)) : 1, hashMb: 64 };
}

/** Rules and game tree (one game at a time). Calls throw on `ok: false`. */
export class Rules {
  static async create() {
    const r = new Rules();
    r.module = await loadModule();
    r.inst = await instantiate(r.module);
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
    this.inst = await instantiate(this.module);
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
      const mt = this.threads > 1 ? await loadThreadedModule() : null;
      const module = mt ? mt.module : await loadModule();
      this.worker = new Worker(new URL('./engine-worker.js', import.meta.url), { type: 'module' });
      this.clock = this.shared ? makeClock() : null;
      const threads = await new Promise((resolve, reject) => {
        this.worker.onmessage = (e) => {
          if (e.data.type === 'ready') resolve(e.data.threads);
          else if (e.data.type === 'error') reject(new Error(e.data.message));
        };
        this.worker.onerror = (e) => reject(new Error(e.message || 'engine worker failed to start'));
        this.worker.postMessage({ type: 'init', module, clock: this.clock, hashMb: this.hashMb, threads: this.threads, memory: mt ? mt.memory : null });
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

  /** Start a search; a running one is stopped first. Returns the request id. */
  async search({ position, movetime = 86_400_000, depth, newGame = false }) {
    await this.start();
    this.stop();
    const id = ++this.reqId;
    this.busy = true;
    this.worker.postMessage({ type: 'search', id, position, movetime, depth, newGame });
    return id;
  }

  stop() {
    if (!this.busy) return;
    if (this.clock) {
      stopClock(this.clock);
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
