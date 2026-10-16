// Minimal WASI (preview 1) host for the Kairos WASM build, shared by the page, the engine
// worker and its thread workers. Only what Rust's std needs here: clocks, randomness,
// stdout/stderr, exit.
//
// The clock doubles as the engine's stop signal. `clock` is a 16-byte SharedArrayBuffer that
// every thread of one engine shares: an Int32 stop flag at byte 0 and a BigInt64 clock offset
// at byte 8. When the flag is set, the next clock read (by any thread) moves the shared offset
// a day ahead, so the search sees its deadline pass and returns its best action at once (see
// `check_abort` in src/engine/ab.rs). A search always runs with a movetime limit for this.
//
// The multi-threaded build (wasm32-wasip1-threads) also imports a shared `env.memory` and the
// wasi-threads `wasi.thread-spawn`; engine-worker.js runs the thread pool behind it.

const ENOSYS = 52;
const DAY_NS = 86_400_000_000_000n;

/** A new shared clock/stop state for one engine (null without SharedArrayBuffer). */
export function makeClock() {
  return typeof SharedArrayBuffer !== 'undefined' ? new SharedArrayBuffer(16) : null;
}

/** Ask the engine using `clock` to stop its search. */
export function stopClock(clock) {
  if (clock) Atomics.store(new Int32Array(clock, 0, 1), 0, 1);
}

export function makeWasi({ clock = null, onStdout = null } = {}) {
  let memory = null;
  const stop = clock ? new Int32Array(clock, 0, 1) : null;
  const offset = clock ? new BigInt64Array(clock, 8, 1) : null;
  const view = () => new DataView(memory.buffer);
  const nowNs = () => {
    if (stop && Atomics.exchange(stop, 0, 0) !== 0) Atomics.add(offset, 0, DAY_NS);
    const off = offset ? Atomics.load(offset, 0) : 0n;
    return BigInt(Math.round((performance.timeOrigin + performance.now()) * 1e6)) + off;
  };
  const decoder = new TextDecoder();
  const lineBuf = { 1: '', 2: '' };
  const imports = {
    clock_time_get(_id, _precision, out) {
      view().setBigUint64(out, nowNs(), true);
      return 0;
    },
    clock_res_get(_id, out) {
      view().setBigUint64(out, 1000n, true);
      return 0;
    },
    random_get(ptr, len) {
      const bytes = new Uint8Array(memory.buffer, ptr, len);
      // getRandomValues refuses views over shared memory and more than 64 KiB at once.
      for (let i = 0; i < len; i += 65536) {
        const chunk = new Uint8Array(Math.min(65536, len - i));
        crypto.getRandomValues(chunk);
        bytes.set(chunk, i);
      }
      return 0;
    },
    fd_write(fd, iovs, iovsLen, nwritten) {
      const dv = view();
      let total = 0;
      for (let i = 0; i < iovsLen; i++) {
        const ptr = dv.getUint32(iovs + i * 8, true);
        const len = dv.getUint32(iovs + i * 8 + 4, true);
        const text = decoder.decode(new Uint8Array(memory.buffer, ptr, len).slice());
        if (fd === 1 || fd === 2) {
          lineBuf[fd] += text;
          let nl;
          while ((nl = lineBuf[fd].indexOf('\n')) >= 0) {
            const line = lineBuf[fd].slice(0, nl);
            lineBuf[fd] = lineBuf[fd].slice(nl + 1);
            if (onStdout) onStdout(fd, line);
            else (fd === 2 ? console.warn : console.log)('[kairos]', line);
          }
        }
        total += len;
      }
      dv.setUint32(nwritten, total, true);
      return 0;
    },
    environ_sizes_get(count, size) {
      view().setUint32(count, 0, true);
      view().setUint32(size, 0, true);
      return 0;
    },
    environ_get() { return 0; },
    args_sizes_get(count, size) {
      view().setUint32(count, 0, true);
      view().setUint32(size, 0, true);
      return 0;
    },
    args_get() { return 0; },
    proc_exit(code) { throw new Error(`kairos exited with code ${code}`); },
    sched_yield() { return 0; },
    fd_close() { return 0; },
    fd_fdstat_get(fd, out) {
      const dv = view();
      dv.setUint8(out, fd <= 2 ? 2 : 0); // character device
      dv.setUint16(out + 2, 0, true);
      dv.setBigUint64(out + 8, 0n, true);
      dv.setBigUint64(out + 16, 0n, true);
      return 0;
    },
  };
  const wasi = new Proxy(imports, {
    get: (t, k) => (k in t ? t[k] : () => ENOSYS),
  });
  return {
    imports: wasi,
    setMemory(m) { memory = m; },
  };
}

/**
 * The memory a module imports (`env.memory`, as the threads build does): {initial, maximum,
 * shared} in 64 KiB pages, read from the binary's import section; null if it imports none.
 */
export function importedMemory(bytes) {
  const b = new Uint8Array(bytes);
  let i = 8;
  const leb = () => {
    let r = 0, shift = 0, x;
    do { x = b[i++]; r += (x & 0x7f) * 2 ** shift; shift += 7; } while (x & 0x80);
    return r;
  };
  const skipName = () => { const n = leb(); i += n; };
  while (i < b.length) {
    const id = b[i++];
    const size = leb();
    const end = i + size;
    if (id === 2) {
      for (let n = leb(); n > 0; n--) {
        skipName();
        skipName();
        const kind = b[i++];
        if (kind === 0) leb();
        else if (kind === 1) { i++; const f = leb(); leb(); if (f & 1) leb(); }
        else if (kind === 3) i += 2;
        else if (kind === 2) {
          const f = leb();
          const initial = leb();
          const maximum = f & 1 ? leb() : undefined;
          return { initial, maximum, shared: !!(f & 2) };
        }
      }
      return null;
    }
    i = end;
  }
  return null;
}

/**
 * Instantiate the Kairos module; returns a small JSON-call interface. The threads build needs
 * `memory` (its shared memory) and `threadSpawn` (the wasi-threads `thread-spawn` import).
 */
export async function instantiate(module, { clock = null, onInfo = null, memory = null, threadSpawn = null } = {}) {
  const wasi = makeWasi({ clock });
  const decoder = new TextDecoder();
  const encoder = new TextEncoder();
  let exports = null;
  const mem = () => memory || exports.memory;
  const imports = {
    wasi_snapshot_preview1: wasi.imports,
    kairos: {
      info(ptr, len) {
        if (onInfo) onInfo(decoder.decode(new Uint8Array(mem().buffer, ptr, len).slice()));
      },
    },
  };
  if (memory) {
    imports.env = { memory };
    imports.wasi = { 'thread-spawn': threadSpawn || (() => -1) };
  }
  const instance = await WebAssembly.instantiate(module, imports);
  exports = instance.exports;
  wasi.setMemory(mem());
  if (exports._initialize) exports._initialize();
  if (exports.kw_init) exports.kw_init();
  return {
    call(req) {
      const bytes = encoder.encode(JSON.stringify(req));
      const ptr = exports.kw_alloc(bytes.length);
      new Uint8Array(mem().buffer, ptr, bytes.length).set(bytes);
      const out = exports.kw_call(ptr, bytes.length);
      exports.kw_free(ptr, bytes.length);
      const len = new DataView(mem().buffer).getUint32(out, true);
      const text = decoder.decode(new Uint8Array(mem().buffer, out + 4, len).slice());
      return JSON.parse(text);
    },
  };
}

/** Instantiate a thread of the threads build (no `_initialize`: the main instance did that). */
export function instantiateThread(module, { clock, memory }) {
  const wasi = makeWasi({ clock });
  const instance = new WebAssembly.Instance(module, {
    wasi_snapshot_preview1: wasi.imports,
    kairos: { info() {} },
    env: { memory },
    wasi: { 'thread-spawn': () => -1 },
  });
  wasi.setMemory(memory);
  return instance.exports;
}
