// Minimal WASI (preview 1) host for the Kairos WASM build, shared by the page and the engine
// worker. Only what Rust's std needs here: clocks, randomness, stdout/stderr, exit.
//
// The clock doubles as the engine's stop signal: when `stopFlag[0]` is set, the next clock
// read jumps forward by a day, so the search sees its deadline pass and returns its best
// action at once (see `check_abort` in src/engine/ab.rs). A search always runs with a
// movetime limit for this reason.

const ENOSYS = 52;
const DAY_NS = 86_400_000_000_000n;

export function makeWasi({ stopFlag = null, onStdout = null } = {}) {
  let memory = null;
  let offsetNs = 0n;
  const view = () => new DataView(memory.buffer);
  const nowNs = () => {
    if (stopFlag && Atomics.load(stopFlag, 0) !== 0) {
      Atomics.store(stopFlag, 0, 0);
      offsetNs += DAY_NS;
    }
    return BigInt(Math.round((performance.timeOrigin + performance.now()) * 1e6)) + offsetNs;
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

/** Instantiate the Kairos module; returns a small JSON-call interface. */
export async function instantiate(module, { stopFlag = null, onInfo = null } = {}) {
  const wasi = makeWasi({ stopFlag });
  const decoder = new TextDecoder();
  const encoder = new TextEncoder();
  let exports = null;
  const instance = await WebAssembly.instantiate(module, {
    wasi_snapshot_preview1: wasi.imports,
    kairos: {
      info(ptr, len) {
        if (onInfo) onInfo(decoder.decode(new Uint8Array(exports.memory.buffer, ptr, len).slice()));
      },
    },
  });
  exports = instance.exports;
  wasi.setMemory(exports.memory);
  if (exports._initialize) exports._initialize();
  return {
    call(req) {
      const bytes = encoder.encode(JSON.stringify(req));
      const ptr = exports.kw_alloc(bytes.length);
      new Uint8Array(exports.memory.buffer, ptr, bytes.length).set(bytes);
      const out = exports.kw_call(ptr, bytes.length);
      exports.kw_free(ptr, bytes.length);
      const len = new DataView(exports.memory.buffer).getUint32(out, true);
      const text = decoder.decode(new Uint8Array(exports.memory.buffer, out + 4, len).slice());
      return JSON.parse(text);
    },
  };
}
