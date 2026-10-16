// Small shared helpers.

export const $ = (sel, root = document) => root.querySelector(sel);
export const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

export function el(tag, attrs = {}, ...children) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v == null || v === false) continue;
    if (k === 'class') e.className = v;
    else if (k === 'text') e.textContent = v;
    else if (k.startsWith('on')) e.addEventListener(k.slice(2), v);
    else if (k === 'dataset') Object.assign(e.dataset, v);
    else e.setAttribute(k, v === true ? '' : v);
  }
  for (const c of children.flat()) if (c != null) e.append(c instanceof Node ? c : document.createTextNode(String(c)));
  return e;
}

export function toast(msg, { error = false, ms = 3200 } = {}) {
  const host = $('#toasts');
  if (!host) return;
  const t = el('div', { class: 'toast' + (error ? ' err' : ''), text: msg });
  host.append(t);
  setTimeout(() => t.remove(), ms);
}

/**
 * In-app confirmation: resolves true when the user confirms, false on Cancel, Esc or a click
 * outside. `danger` styles the confirm button as destructive.
 */
export function confirmDialog(title, { text = '', ok = 'OK', danger = false } = {}) {
  const dlg = $('#dlg-confirm');
  $('#confirm-title').textContent = title;
  $('#confirm-text').textContent = text;
  $('#confirm-text').classList.toggle('hidden', !text);
  const btn = $('#confirm-ok');
  btn.textContent = ok;
  btn.classList.toggle('danger', danger);
  return new Promise((resolve) => {
    // Settle from the controls themselves (not the dialog's close event).
    const handlers = [
      [btn, 'click', (e) => { e.preventDefault(); done(true); }],
      [$('#confirm-cancel'), 'click', (e) => { e.preventDefault(); done(false); }],
      [dlg, 'cancel', (e) => { e.preventDefault(); done(false); }],
      [dlg, 'click', (e) => { if (e.target === dlg) done(false); }],
    ];
    const done = (v) => {
      for (const [node, ev, fn] of handlers) node.removeEventListener(ev, fn);
      if (dlg.open) dlg.close();
      resolve(v);
    };
    for (const [node, ev, fn] of handlers) node.addEventListener(ev, fn);
    dlg.showModal();
    btn.focus();
  });
}

/** Engine score (White's view): "+1.23", "−0.40", "#3", "#−2". Units are hundredths. */
export function fmtScore(s) {
  if (!s) return '—';
  // Recorded mates (SPRT writes ±20000) carry no distance.
  if (s.mate != null && s.recorded) return s.mate > 0 ? '+#' : '−#';
  if (s.mate != null) return s.mate > 0 ? `#${s.mate}` : `#−${-s.mate}`;
  if (s.cp == null) return '—';
  const v = s.cp / 100;
  const t = Math.abs(v).toFixed(Math.abs(v) >= 10 ? 1 : 2);
  return v > 0 ? `+${t}` : v < 0 ? `−${t}` : '0.00';
}

/** Winning chances in [-1, 1] for White (for the bar and the graph). */
export function winChance(s) {
  if (!s) return 0;
  if (s.mate != null) return s.mate > 0 ? 1 : -1;
  const cp = Math.max(-2500, Math.min(2500, s.cp || 0));
  return 2 / (1 + Math.exp(-0.0022 * cp)) - 1;
}

/** A recorded engine score (mover's view, ±20000 = mate) as {cp|mate} for White. */
export function recordScore(e, moverBlack) {
  if (e == null) return null;
  const v = moverBlack ? -e : e;
  if (Math.abs(v) >= 20000) return { mate: v > 0 ? 1 : -1, recorded: true };
  return { cp: v };
}

export function fmtNodes(n) {
  if (n >= 1e9) return (n / 1e9).toFixed(1) + 'G';
  if (n >= 1e6) return (n / 1e6).toFixed(1) + 'M';
  if (n >= 1e3) return (n / 1e3).toFixed(1) + 'k';
  return String(n);
}

export function fmtClock(ms) {
  if (ms == null) return '--:--';
  ms = Math.max(0, ms);
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  if (ms < 10000) return `0:${String(Math.floor(ms / 1000)).padStart(2, '0')}.${Math.floor((ms % 1000) / 100)}`;
  if (m >= 60) return `${Math.floor(m / 60)}:${String(m % 60).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;
  return `${m}:${String(s % 60).padStart(2, '0')}`;
}

export function fmtBytes(n) {
  if (n >= 1 << 20) return (n / (1 << 20)).toFixed(1) + ' MB';
  if (n >= 1 << 10) return Math.round(n / (1 << 10)) + ' KB';
  return n + ' B';
}

const LAN = /^\((-?\d+)T(-?\d+)\)([a-h])([1-8])(?:\((-?\d+)T(-?\d+)\))?([a-h])([1-8])([A-Z])?$/;

/** A 5DUCI move like "(0T5)b5(0T3)b6" as {from:[l,t,x,y], to:[l,t,x,y]}. */
export function parseLan(tok) {
  const m = LAN.exec(tok);
  if (!m) return null;
  const l1 = +m[1], t1 = +m[2];
  const l2 = m[5] != null ? +m[5] : l1, t2 = m[6] != null ? +m[6] : t1;
  const f = (c) => c.charCodeAt(0) - 97;
  return { from: [l1, t1, f(m[3]), +m[4] - 1], to: [l2, t2, f(m[7]), +m[8] - 1], promote: m[9] };
}

export function debounce(fn, ms) {
  let t = 0;
  return (...a) => { clearTimeout(t); t = setTimeout(() => fn(...a), ms); };
}

export function store(key, value) {
  try {
    if (value === undefined) return JSON.parse(localStorage.getItem('kairos.' + key));
    localStorage.setItem('kairos.' + key, JSON.stringify(value));
  } catch (e) { return undefined; }
}
