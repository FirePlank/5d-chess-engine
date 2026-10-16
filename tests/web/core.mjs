// The browser core (built by `cargo wasm`) for Node scripts: a single-threaded instance on
// shared memory, as the page's rules instance runs it.
import { readFileSync } from 'node:fs';
import { instantiate, importedMemory } from '../../web/static/js/wasi.js';

export async function loadCore(opts = {}) {
  const bytes = readFileSync(new URL('../../web/wasm/target/wasm32-wasip1-threads/release/kairos_wasm.wasm', import.meta.url));
  const module = await WebAssembly.compile(bytes);
  const desc = importedMemory(bytes);
  const memory = new WebAssembly.Memory({ initial: desc.initial, maximum: desc.maximum, shared: true });
  return instantiate(module, { ...opts, memory });
}
