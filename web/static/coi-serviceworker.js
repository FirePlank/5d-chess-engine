// Cross-origin isolation on static hosting (as the coi-serviceworker approach does). The
// engine's helper threads need SharedArrayBuffer, which a page only gets when it is served
// with COOP/COEP headers; kairos-web sends them, GitHub Pages cannot. Loaded as a page script,
// this registers itself as a service worker and reloads the page once; as the worker, it adds
// the headers to every response of the site. If that is impossible (no service workers, not
// a secure context), the page simply runs the engine on one thread.
if (typeof window === 'undefined') {
  self.addEventListener('install', () => self.skipWaiting());
  self.addEventListener('activate', (e) => e.waitUntil(self.clients.claim()));
  self.addEventListener('fetch', (e) => {
    const req = e.request;
    if (req.cache === 'only-if-cached' && req.mode !== 'same-origin') return;
    // Revalidate every file (a cheap 304 when unchanged): GitHub Pages lets browsers keep a file
    // for ten minutes, which would mix an old script with a new page after a deploy.
    e.respondWith(fetch(req, { cache: 'no-cache' }).then((res) => {
      if (res.status === 0) return res;
      const headers = new Headers(res.headers);
      headers.set('Cross-Origin-Embedder-Policy', 'require-corp');
      headers.set('Cross-Origin-Opener-Policy', 'same-origin');
      headers.set('Cross-Origin-Resource-Policy', 'same-origin');
      return new Response(res.body, { status: res.status, statusText: res.statusText, headers });
    }));
  });
} else if (!window.crossOriginIsolated && window.isSecureContext && navigator.serviceWorker) {
  const sw = navigator.serviceWorker;
  const reloadOnce = () => {
    // Once per tab session, so a browser that will not isolate the page never loops.
    try {
      if (sessionStorage.getItem('coiReloaded')) return;
      sessionStorage.setItem('coiReloaded', '1');
    } catch (e) { return; }
    location.reload();
  };
  sw.register(document.currentScript.src).then((reg) => {
    if (sw.controller) return; // controlled yet not isolated: stay on one thread
    if (reg.active) reloadOnce();
    else sw.addEventListener('controllerchange', reloadOnce);
  }, (e) => console.warn('coi-serviceworker:', e));
} else if (window.crossOriginIsolated) {
  try { sessionStorage.removeItem('coiReloaded'); } catch (e) { /* ignore */ }
}
