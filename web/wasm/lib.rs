//! The browser build: `kairos::web::api` behind a C ABI. Requests and responses are JSON
//! strings in linear memory; engine progress lines go to the `kairos.info` import.
//!
//! Built for wasm32-wasip1 so the engine's clock works (`Instant` → WASI `clock_time_get`).

use kairos::web::api::Api;
use std::cell::RefCell;

#[link(wasm_import_module = "kairos")]
unsafe extern "C" {
    fn info(ptr: *const u8, len: usize);
}

thread_local! {
    static API: RefCell<Api> = RefCell::new(Api::new());
    static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Allocate `len` bytes for a request.
#[unsafe(no_mangle)]
pub extern "C" fn kw_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Free a request buffer from `kw_alloc`.
///
/// # Safety
/// `ptr` and `len` must come from one `kw_alloc` call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kw_free(ptr: *mut u8, len: usize) {
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len.max(1)) });
}

/// Handle the JSON request at `ptr`; returns a pointer to a little-endian u32 length followed
/// by the JSON response (valid until the next call).
///
/// # Safety
/// `ptr..ptr+len` must be readable memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kw_call(ptr: *const u8, len: usize) -> *const u8 {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let req = String::from_utf8_lossy(bytes);
    let resp = API.with(|a| a.borrow_mut().call(&req, &mut |line| unsafe { info(line.as_ptr(), line.len()) }));
    OUT.with(|o| {
        let mut o = o.borrow_mut();
        o.clear();
        o.extend_from_slice(&(resp.len() as u32).to_le_bytes());
        o.extend_from_slice(resp.as_bytes());
        o.as_ptr()
    })
}

/// The threads build (wasm32-wasip1-threads): set up the main thread as a WASI reactor's
/// `_initialize` would (its pthread state, then static constructors); a cdylib has no
/// `_initialize`, and without this, thread locals with destructors hang. Call it once on the
/// main instance before any other export; thread instances set themselves up.
#[cfg(target_feature = "atomics")]
#[unsafe(no_mangle)]
pub extern "C" fn kw_init() {
    unsafe extern "C" {
        fn __wasi_init_tp();
        fn __wasm_call_ctors();
    }
    unsafe {
        __wasi_init_tp();
        __wasm_call_ctors();
    }
}
