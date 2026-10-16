// The analysis board / game viewer: a game tree with variations, step-through navigation,
// book and engine actions, the recorded eval graph and live engine analysis.
import { Rules, parseInfo, threadsSupported } from './kairos.js';
import { apiGet } from './static-api.js';
import { $, $$, el, toast, fmtScore, fmtNodes, winChance, recordScore, parseLan, store, confirmDialog } from './util.js';

const THROTTLE_MS = 70;
/** "Go deeper" searches the current position to this depth. */
const MAX_DEPTH = 64;

export class Analysis {
  constructor(app) {
    this.app = app;
    this.board = app.board;
    this.input = app.input;
    this.engine = app.engine;
    this.tree = null;
    this.meta = null;
    this.cur = 0;
    this.cache = new Map(); // node id -> latest engine info (White's view)
    this.gameKey = 0;
    this.engineOn = !!store('engine');
    // Analysis searches each position to a fixed depth (engine settings), and the current
    // position deeper on request ("+").
    this.depth = Math.max(1, Math.min(MAX_DEPTH, +(store('engineSettings') || {}).depth || 15));
    this.deeperNode = null;
    this.autoplay = null;
    this.lastEmit = 0;
    this.pending = null;
    this.source = {
      mode: 'analysis',
      rules: null,
      node: () => this.cur,
      canMove: () => true,
      submit: async (moves) => this.play(moves),
      onView: (v, draft) => this.onView(v, draft),
    };
  }

  async init() {
    this.rules = await Rules.create();
    this.source.rules = this.rules;
    this.variants = this.rules.call({ cmd: 'variants' }).variants;
    this.bindUi();
    this.engine.on((m) => this.onEngine(m));
    this.bindSettings();
    $('#engine-toggle').checked = this.engineOn;
    this.newGame('Standard', { quiet: true });
  }

  // ---------------------------------------------------------------- activation
  activate(params) {
    // While you play Kairos, analysis here would take CPU from the engine: off unless asked.
    this.suppressed = this.engineOn && this.playing();
    this.input.attach(this.source);
    this.board.setOverlay({ readonly: false });
    if (params.get('file')) {
      const file = params.get('file'), index = +(params.get('index') || 0);
      const key = `${file}#${index}`;
      if (this.loadedKey !== key) {
        this.loadedKey = key;
        this.loadRemote(file, index);
        return;
      }
    } else if (params.get('uci')) {
      const key = 'uci:' + params.get('uci');
      if (this.loadedKey !== key) {
        this.loadedKey = key;
        this.load(params.get('uci'));
        return;
      }
    }
    this.show({ follow: false });
    this.restartEngine();
  }

  deactivate() {
    this.stopAutoplay();
    this.engine.stop();
  }

  async loadRemote(file, index) {
    try {
      const r = await apiGet(this.app, `api/games/get?file=${encodeURIComponent(file)}&index=${index}`);
      if (!r.ok) throw new Error(r.error);
      try {
        this.load(r.text, null, { source: { file, index, count: r.count } });
      } catch (e) {
        // Let the user fix it (e.g. give the start position) in the load dialog.
        this.openLoad(r.text);
        $('#load-error').textContent = e.message;
        $('#load-error').classList.remove('hidden');
      }
    } catch (e) {
      toast('Could not load the game: ' + e.message, { error: true });
    }
  }

  // ---------------------------------------------------------------- loading
  /** Load a record (JSONL line, 5dpgn, 5DUCI); throws with a readable message. */
  load(text, position = null, { source = null, quiet = false } = {}) {
    const r = this.rules.call({ cmd: 'load', text, position: position || undefined });
    this.setGame(r.tree, r.meta, source);
    // Open at the end of the main line, like a finished game.
    this.cur = r.mainline[r.mainline.length - 1];
    this.board.fitted = false;
    this.show({ follow: false });
    this.board.fit(false, true);
    this.restartEngine();
    if (!quiet) toast(r.meta.actions ? `Loaded ${r.meta.actions} actions` : 'Loaded the position');
    for (const w of r.meta.warnings || []) toast(w, { error: true, ms: 6000 });
  }

  newGame(position, { quiet = false } = {}) {
    const r = this.rules.call({ cmd: 'new', position });
    this.setGame(r.tree, r.meta, null);
    this.cur = 0;
    this.board.fitted = false;
    this.show({ follow: false });
    this.restartEngine();
    if (!quiet) toast('New board');
  }

  setGame(tree, meta, source) {
    this.tree = tree;
    this.meta = meta;
    this.meta.source = source;
    this.cache.clear();
    this.gameKey++;
    this.input.attach(this.source);
    this.rootPlayer = this.rules.call({ cmd: 'view', node: 0, draft: [] }).view.player;
    this.renderGameCard();
  }

  // ---------------------------------------------------------------- tree helpers
  node(id) { return this.tree.nodes[id]; }
  /** Root → current, then the first-child continuation. */
  line() {
    const path = [];
    for (let n = this.cur; n != null; n = this.node(n).parent) path.unshift(n);
    let c = this.cur;
    while (this.node(c).children.length) {
      c = this.node(c).children[0];
      path.push(c);
    }
    return path;
  }
  mainline() {
    const ml = [0];
    let c = 0;
    while (this.node(c).children.length) { c = this.node(c).children[0]; ml.push(c); }
    return ml;
  }
  moveNumber(n) {
    const k = n.ply - 1 + this.rootPlayer;
    return { num: Math.floor(k / 2) + 1, black: k % 2 === 1 };
  }
  /** Best known score at node (engine analysis, else the record), White's view. */
  scoreAt(id) {
    const c = this.cache.get(id);
    if (c && (c.cp != null || c.mate != null)) return c;
    return this.recordedScoreAt(id);
  }
  /** The recorded score of the position after node's action. */
  recordedScoreAt(id) {
    const n = this.node(id);
    if (!n || n.eval == null) return null;
    return recordScore(n.eval, n.player);
  }

  // ---------------------------------------------------------------- navigation
  goto(id, { follow = true } = {}) {
    if (id == null || !this.tree.nodes[id] || (id === this.cur && !this.input.active)) return;
    this.cur = id;
    this.input.reset();
    this.show({ follow });
    this.restartEngine();
  }
  next() { const c = this.node(this.cur).children; if (c.length) this.goto(c[0]); else this.stopAutoplay(); }
  prev() { const p = this.node(this.cur).parent; if (p != null) this.goto(p); }
  first() { this.goto(0); }
  last() { const l = this.line(); this.goto(l[l.length - 1]); }
  sibling(d) {
    const p = this.node(this.cur).parent;
    if (p == null) return;
    const sib = this.node(p).children;
    const i = sib.indexOf(this.cur);
    const j = i + d;
    if (j >= 0 && j < sib.length) this.goto(sib[j]);
  }
  toggleAutoplay() {
    if (this.autoplay) return this.stopAutoplay();
    if (!this.node(this.cur).children.length) this.first();
    this.autoplay = setInterval(() => this.next(), 1000);
    $('#nav-play').classList.add('playing');
  }
  stopAutoplay() {
    clearInterval(this.autoplay);
    this.autoplay = null;
    $('#nav-play').classList.remove('playing');
  }

  async play(moves) {
    const r = this.rules.call({ cmd: 'play', node: this.cur, moves });
    this.tree = r.tree;
    this.cur = r.node;
    this.input.reset();
    this.show();
    this.restartEngine();
  }

  // ---------------------------------------------------------------- rendering
  show({ follow = true } = {}) {
    let v;
    try {
      v = this.input.refresh(follow);
    } catch (e) {
      toast(e.message, { error: true });
      return;
    }
    this.renderMoves();
    this.renderNav();
    this.renderGraph();
    this.renderEngine();
  }

  onView(v, draft) {
    this.view = v;
    const chip = $('#status-chip');
    chip.className = 'status-line';
    if (draft.length) {
      chip.textContent = '';
    } else if (v.status === 'checkmate') {
      chip.textContent = `Checkmate · ${v.player ? 'White' : 'Black'} wins`;
      chip.classList.add('over');
    } else if (v.status === 'stalemate') {
      chip.textContent = 'Stalemate · draw';
      chip.classList.add('over');
    } else if (v.status === 'exiled-white' || v.status === 'exiled-black') {
      chip.textContent = `Stalemate · exiled kings · ${v.status === 'exiled-white' ? 'Black' : 'White'} wins`;
      chip.classList.add('over');
    } else {
      const side = v.player ? 'Black' : 'White';
      const inCheck = v.checks.length > 0;
      chip.textContent = `${side} to move${inCheck ? ' · in check' : ''}`;
      if (inCheck) chip.classList.add('check');
    }
  }

  renderGameCard() {
    const m = this.meta;
    $('#pl-white').textContent = m.white || 'White';
    $('#pl-black').textContent = m.black || 'Black';
    $('#gm-result').textContent = m.result ? m.result.replace('1/2-1/2', '½-½') : '*';
    const parts = [];
    if (m.variant) parts.push(m.variant);
    if (m.format && m.format !== 'new') parts.push({ sprt: 'SPRT record', '5dpgn': '5dpgn', '5duci': '5DUCI' }[m.format] || m.format);
    if (m.reason) parts.push(m.reason);
    if (m.actions) parts.push(`${m.actions} actions`);
    if (m.book) parts.push(`${m.book} book (grey)`);
    if (m.game != null) parts.push(`game ${m.game}${m.pair != null ? `, pair ${m.pair}` : ''}`);
    const pills = parts.map((t, i) => el('span', { class: i ? 'sep' : '', text: t }));
    if (m.source) {
      const { file, index, count } = m.source;
      const nav = (d) => () => { location.hash = `#/analysis?file=${encodeURIComponent(file)}&index=${index + d}`; };
      if (index > 0) pills.push(el('span', { class: 'sep' }, el('button', { text: '‹ previous', onclick: nav(-1) })));
      if (index + 1 < count) pills.push(el('span', { class: 'sep' }, el('button', { text: 'next game ›', onclick: nav(1) })));
    }
    $('#gm-meta').replaceChildren(...pills);
  }

  moveCell(id, cls = 'mv') {
    const n = this.node(id);
    const cell = el('div', { class: cls + (n.book ? ' book' : '') + (id === this.cur ? ' cur' : ''), dataset: { id }, title: n.lan + (n.book ? '  (book)' : '') });
    cell.append(n.san || n.lan);
    const s = this.cache.get(id) || this.recordedScoreAt(id);
    if (s) cell.append(el('span', { class: 'mv-eval', text: fmtScore(s) }));
    return cell;
  }

  renderMoves() {
    const box = $('#moves');
    const frag = document.createDocumentFragment();
    const ml = this.mainline();
    if (ml.length === 1) {
      box.replaceChildren(el('div', { class: 'moves-empty', text: 'No actions yet. Make a move on the board, load a game, or open one from the Games tab.' }));
      if (this.node(0).comments.length) box.prepend(el('div', { class: 'mv-comment', text: this.node(0).comments.join(' ') }));
      return;
    }
    let row = null;
    const closeRow = () => {
      if (row && row.children.length === 2) row.append(el('div', { class: 'mv empty' }));
      row = null;
    };
    for (let k = 1; k < ml.length; k++) {
      const n = this.node(ml[k]);
      const { num, black } = this.moveNumber(n);
      if (!row || !black) {
        closeRow();
        row = el('div', { class: 'mv-row' }, el('div', { class: 'mv-num', text: num + '.' }));
        frag.append(row);
        if (black) row.append(el('div', { class: 'mv empty', text: '…' }));
      }
      row.append(this.moveCell(n.id));
      if (black) row = null;
      if (n.comments.length) { closeRow(); frag.append(el('div', { class: 'mv-row' }, el('div', { class: 'mv-comment', text: n.comments.join(' ') }))); }
      const parent = this.node(n.parent);
      if (parent.children.length > 1) {
        closeRow();
        for (const sib of parent.children.slice(1)) frag.append(el('div', { class: 'mv-row' }, this.variationEl(sib)));
      }
    }
    closeRow();
    box.replaceChildren(frag);
    const cur = box.querySelector('.cur');
    if (cur) {
      const r = cur.getBoundingClientRect(), b = box.getBoundingClientRect();
      if (r.top < b.top + 8 || r.bottom > b.bottom - 8) box.scrollTop += r.top - b.top - b.height / 2 + r.height / 2;
    } else if (this.cur === 0) box.scrollTop = 0;
  }

  variationEl(start) {
    const wrap = el('div', { class: 'variation' });
    let first = true;
    const walk = (id) => {
      for (let c = id; c != null;) {
        const n = this.node(c);
        const { num, black } = this.moveNumber(n);
        const label = !black ? `${num}. ` : first ? `${num}… ` : '';
        first = false;
        wrap.append(el('span', { class: 'vmv' + (c === this.cur ? ' cur' : ''), dataset: { id: c }, text: label + (n.san || n.lan) }));
        const kids = n.children;
        for (const alt of kids.slice(1)) {
          wrap.append('(');
          first = true;
          walk(alt);
          wrap.append(')');
        }
        c = kids[0];
      }
    };
    walk(start);
    return wrap;
  }

  renderNav() {
    const n = this.node(this.cur);
    $('#nav-first').disabled = $('#nav-prev').disabled = n.parent == null;
    $('#nav-next').disabled = $('#nav-last').disabled = !n.children.length;
  }

  // ---------------------------------------------------------------- eval graph
  renderGraph() {
    const cv = $('#graph');
    const wrap = $('#graph-wrap');
    const line = this.line();
    const hasData = line.some((id) => this.scoreAt(id));
    wrap.classList.toggle('hidden', line.length < 3 || !hasData);
    if (wrap.classList.contains('hidden')) return;
    const dpr = window.devicePixelRatio || 1;
    const W = cv.clientWidth, H = cv.clientHeight;
    cv.width = W * dpr; cv.height = H * dpr;
    const ctx = cv.getContext('2d');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const css = getComputedStyle(document.documentElement);
    const text = css.getPropertyValue('--muted').trim();
    const accent = css.getPropertyValue('--accent').trim();
    const dark = document.documentElement.dataset.theme === 'dark';
    const n = line.length;
    const X = (i) => 6 + (W - 12) * (i / Math.max(1, n - 1));
    const Y = (w) => H / 2 - (H / 2 - 6) * w;
    this.graphGeom = { X, n, line };
    // Book region.
    let book = 0;
    while (book + 1 < n && this.node(line[book + 1]).book) book++;
    if (book > 0) {
      ctx.fillStyle = css.getPropertyValue('--warn').trim();
      ctx.globalAlpha = 0.09;
      ctx.fillRect(0, 0, X(book), H);
      ctx.globalAlpha = 1;
    }
    // Area under the curve.
    const pts = line.map((id, i) => [X(i), this.scoreAt(id)]).filter((p) => p[1]);
    if (pts.length) {
      ctx.beginPath();
      ctx.moveTo(pts[0][0], Y(0));
      for (const [x, s] of pts) ctx.lineTo(x, Y(winChance(s)));
      ctx.lineTo(pts[pts.length - 1][0], Y(0));
      ctx.closePath();
      ctx.save();
      ctx.clip();
      ctx.fillStyle = dark ? 'rgba(235,235,245,.85)' : 'rgba(255,255,255,1)';
      ctx.fillRect(0, 0, W, H / 2);
      ctx.fillStyle = dark ? 'rgba(0,0,0,.55)' : 'rgba(40,40,52,.85)';
      ctx.fillRect(0, H / 2, W, H / 2);
      ctx.restore();
      ctx.strokeStyle = accent;
      ctx.lineWidth = 1.4;
      ctx.beginPath();
      pts.forEach(([x, s], i) => (i ? ctx.lineTo(x, Y(winChance(s))) : ctx.moveTo(x, Y(winChance(s)))));
      ctx.stroke();
    }
    ctx.strokeStyle = text;
    ctx.globalAlpha = 0.35;
    ctx.beginPath(); ctx.moveTo(0, H / 2); ctx.lineTo(W, H / 2); ctx.stroke();
    ctx.globalAlpha = 1;
    const ci = line.indexOf(this.cur);
    if (ci >= 0) {
      ctx.strokeStyle = '#e8590c';
      ctx.lineWidth = 1.5;
      ctx.beginPath(); ctx.moveTo(X(ci), 0); ctx.lineTo(X(ci), H); ctx.stroke();
    }
  }

  // ---------------------------------------------------------------- engine
  /** A Play game of this browser is in progress (its engine needs the CPU). */
  playing() {
    const p = this.app.pages.play;
    const st = p && p.state;
    return !!(st && st.you && st.game && !st.game.over);
  }

  restartEngine() {
    if (this.suppressed && !this.playing()) this.suppressed = false;
    if (!this.engineOn || this.suppressed || !this.tree) {
      this.engine.stop();
      this.searching = null;
      this.renderEngine();
      return;
    }
    const v = this.view;
    if (!v || v.status === 'checkmate' || v.status === 'stalemate' || v.status === 'exiled-white' || v.status === 'exiled-black') {
      this.engine.stop();
      this.searching = null;
      this.renderEngine();
      return;
    }
    const node = this.cur;
    const blackToMove = v.player === 1;
    const key = this.gameKey;
    const depth = this.targetDepth();
    // Already searched to the target depth: show that result without searching again.
    const cached = this.cache.get(node);
    if (cached && cached.depth >= depth) {
      this.engine.stop();
      this.searching = { node, key, blackToMove, finished: true };
      this.renderEngine();
      return;
    }
    this.searching = { node, key, blackToMove, started: performance.now() };
    // The movetime only bounds the search (and lets a stop interrupt it); the depth ends it.
    // `resume`: when the engine's last search was this position (go deeper, a raised depth, a
    // stopped search revisited), it continues from the depth reached instead of from 1.
    this.engine.search({ position: v.position, depth, movetime: 86_400_000, resume: true }).then((id) => {
      if (this.searching && this.searching.node === node) this.searching.id = id;
    }).catch((e) => {
      toast('Engine failed to start: ' + e.message, { error: true });
      this.setEngine(false);
    });
    this.renderEngine();
  }

  onEngine(m) {
    if (m.type === 'ready') return this.renderEngine();
    const s = this.searching;
    if (!s || (s.id != null && m.id !== s.id)) return;
    if (m.type === 'info') {
      const info = parseInfo(m.line, s.blackToMove);
      // Info lines come once per completed depth; keep the deepest one per position, so a
      // revisited position shows its best result until the new search gets deeper.
      if (info.depth == null || (info.cp == null && info.mate == null) || !info.pv) return;
      const prev = this.cache.get(s.node);
      if (!prev || info.depth >= prev.depth) this.cache.set(s.node, { ...info, at: performance.now() });
      this.scheduleRender();
    } else if (m.type === 'done') {
      const c = this.cache.get(s.node);
      if (c) c.done = true;
      this.searching = { ...s, finished: true };
      this.renderEngine();
      this.renderMovesSoon();
    } else if (m.type === 'error') {
      toast('Engine error: ' + m.message, { error: true });
      this.searching = null;
      this.renderEngine();
    }
  }

  scheduleRender() {
    const now = performance.now();
    if (now - this.lastEmit > THROTTLE_MS) {
      this.lastEmit = now;
      this.renderEngine();
      this.renderMovesSoon();
    } else if (!this.pending) {
      this.pending = setTimeout(() => {
        this.pending = null;
        this.lastEmit = performance.now();
        this.renderEngine();
        this.renderMovesSoon();
      }, THROTTLE_MS);
    }
  }
  renderMovesSoon() {
    clearTimeout(this.movesTimer);
    this.movesTimer = setTimeout(() => { this.renderMoves(); this.renderGraph(); }, 250);
  }

  setEngine(on) {
    this.suppressed = false;
    this.engineOn = on;
    store('engine', on);
    $('#engine-toggle').checked = on;
    if (on) this.restartEngine();
    else {
      this.engine.stop();
      this.searching = null;
      this.renderEngine();
    }
  }

  renderEngine() {
    const card = $('#engine-card');
    $('#engine-toggle').checked = this.engineOn && !this.suppressed;
    const n = this.engine.ready ? this.engine.threadsActive : threadsSupported ? this.engine.threads : 1;
    $('#engine-where').textContent = `· ${n} thread${n > 1 ? 's' : ''}`;
    const info = this.cache.get(this.cur);
    const s = this.searching;
    const running = this.engineOn && s && s.node === this.cur && !s.finished;
    card.classList.toggle('computing', !!running);
    const score = this.scoreAt(this.cur);
    $('#engine-eval').textContent = this.engineOn && info ? fmtScore(info) : score ? fmtScore(score) : '-';
    let stats;
    if (this.engineOn && this.suppressed) stats = 'Paused while your game against Kairos runs (E to start)';
    else if (!this.engineOn) stats = score && !info ? 'Recorded eval · analysis off (E)' : 'Off · press E to analyse';
    else if (this.view && ['checkmate', 'stalemate', 'exiled-white', 'exiled-black'].includes(this.view.status)) stats = 'Game over';
    else if (!info) stats = running ? 'Starting…' : 'Waiting';
    else {
      stats = `Depth ${info.depth}/${this.targetDepth()}` + (info.nps ? ` · ${fmtNodes(info.nps)} nodes/s` : '');
    }
    $('#engine-stats').textContent = stats;
    $('#engine-progress').style.width = this.engineOn && info ? `${Math.min(100, (info.depth / this.targetDepth()) * 100)}%` : '0%';
    $('#engine-deeper').classList.toggle('hidden', !this.engineOn || this.targetDepth() >= MAX_DEPTH);
    // Eval bar.
    const bar = $('#eval-bar');
    if (score) {
      bar.classList.remove('hidden');
      const w = winChance(score);
      $('#eval-bar-black').style.height = `${50 - w * 50}%`;
      bar.classList.toggle('flipped', this.board.flipped);
    } else bar.classList.add('hidden');
    // PV.
    const lines = $('#engine-lines');
    if (this.engineOn && info && info.pv) {
      if (this.pvKey !== this.cur + '|' + info.pv) {
        this.pvKey = this.cur + '|' + info.pv;
        let acts = [];
        try { acts = this.rules.call({ cmd: 'line_san', node: this.cur, line: info.pv }).actions; } catch (e) { acts = []; }
        const base = this.node(this.cur);
        const moves = el('div', { class: 'line-moves' });
        acts.forEach((a, i) => {
          const { num, black } = this.moveNumber({ ply: base.ply + 1 + i });
          const span = el('span', { class: 'line-move', title: a.lan }, (i === 0 || !black) ? el('span', { class: 'num', text: black ? `${num}…` : `${num}.` }) : null, a.san);
          span.addEventListener('pointerdown', (e) => { e.preventDefault(); this.playPv(info.pv, i + 1); });
          moves.append(span);
        });
        lines.replaceChildren(el('div', { class: 'engine-line' }, el('span', { class: 'line-eval', text: fmtScore(info) }), moves));
        // Arrows for the first action of the PV.
        const first = (info.pv.split('submit')[0] || '').trim().split(/\s+/).map(parseLan).filter(Boolean);
        this.board.setOverlay({ pv: first.map((m) => ({ ...m, c: this.view ? this.view.player : 0 })) });
      } else {
        const ev = lines.querySelector('.line-eval');
        if (ev) ev.textContent = fmtScore(info);
      }
    } else {
      lines.replaceChildren();
      this.pvKey = null;
      this.board.setOverlay({ pv: [] });
    }
  }

  /** The depth to search the current position to. */
  targetDepth() {
    return this.deeperNode === this.cur ? MAX_DEPTH : this.depth;
  }

  playPv(pv, count) {
    try {
      const r = this.rules.call({ cmd: 'play_line', node: this.cur, line: pv, count });
      this.tree = r.tree;
      this.goto(r.node);
    } catch (e) {
      toast(e.message, { error: true });
    }
  }

  // ---------------------------------------------------------------- engine settings
  /** Depth, threads and hash for the in-browser engine (saved; threads and hash restart it). */
  bindSettings() {
    const btn = $('#engine-settings-btn'), panel = $('#engine-settings');
    const threads = $('#set-threads'), hash = $('#set-hash'), depth = $('#set-depth');
    depth.max = String(MAX_DEPTH);
    depth.value = String(this.depth);
    const cores = navigator.hardwareConcurrency || 2;
    threads.max = String(threadsSupported ? Math.max(1, Math.min(32, cores)) : 1);
    const cur = { threads: this.engine.threads, hashMb: this.engine.hashMb };
    threads.value = String(cur.threads);
    hash.value = String(cur.hashMb);
    const show = () => {
      $('#set-threads-v').textContent = threads.value;
      $('#set-depth-v').textContent = depth.value;
      $('#set-note').textContent = threadsSupported
        ? `${cores} logical cores here; leave one free for the page.`
        : 'This page is not cross-origin isolated, so the engine runs on one thread.';
    };
    show();
    btn.onclick = () => {
      const open = panel.classList.toggle('hidden') === false;
      btn.setAttribute('aria-expanded', String(open));
    };
    const apply = () => {
      const s = { threads: +threads.value, hashMb: +hash.value, depth: +depth.value };
      store('engineSettings', s);
      this.depth = s.depth;
      this.engine.configure(s);
      this.renderEngine();
      if (this.engineOn) this.restartEngine();
    };
    threads.oninput = show;
    depth.oninput = show;
    threads.onchange = apply;
    depth.onchange = apply;
    hash.onchange = apply;
  }

  // ---------------------------------------------------------------- UI wiring
  bindUi() {
    $('#moves').addEventListener('click', (e) => {
      const c = e.target.closest('[data-id]');
      if (c) this.goto(+c.dataset.id);
    });
    $('#moves').addEventListener('contextmenu', async (e) => {
      const c = e.target.closest('[data-id]');
      if (!c) return;
      e.preventDefault();
      const id = +c.dataset.id;
      const n = this.node(id);
      if (n.parent == null) return;
      if (await confirmDialog('Delete this action?', { text: 'Everything after it is deleted too.', ok: 'Delete', danger: true })) {
        const r = this.rules.call({ cmd: 'delete', node: id });
        this.tree = r.tree;
        if (!this.line().includes(this.cur) || this.isInSubtree(this.cur, id)) this.cur = r.node;
        this.show();
      }
    });
    $('#nav-first').onclick = () => this.first();
    $('#nav-prev').onclick = () => this.prev();
    $('#nav-next').onclick = () => this.next();
    $('#nav-last').onclick = () => this.last();
    $('#nav-play').onclick = () => this.toggleAutoplay();
    $('#engine-toggle').onchange = (e) => this.setEngine(e.target.checked);
    $('#engine-deeper').onclick = () => { this.deeperNode = this.cur; this.restartEngine(); };
    const graph = $('#graph');
    graph.addEventListener('click', (e) => {
      const g = this.graphGeom;
      if (!g) return;
      const r = graph.getBoundingClientRect();
      const i = Math.round(((e.clientX - r.left - 6) / (r.width - 12)) * (g.n - 1));
      this.goto(g.line[Math.max(0, Math.min(g.n - 1, i))]);
    });

    // Load dialog.
    const dlg = $('#dlg-load');
    $('#btn-load').onclick = () => this.openLoad();
    $('#load-file').onchange = async (e) => {
      const f = e.target.files[0];
      if (f) { $('#load-text').value = await f.text(); this.updatePick(); }
    };
    $('#load-text').addEventListener('input', () => this.updatePick());
    $('#load-go').onclick = (e) => {
      e.preventDefault();
      const text = this.pickedText();
      try {
        this.loadedKey = null;
        this.load(text, $('#load-position').value.trim());
        dlg.close();
        if (location.hash !== '#/analysis') history.replaceState(null, '', '#/analysis');
      } catch (err) {
        const box = $('#load-error');
        box.textContent = err.message;
        box.classList.remove('hidden');
      }
    };
    // New dialog.
    const sel = $('#new-variant');
    sel.replaceChildren(...this.variants.map((v) => el('option', { value: v.name, text: v.name })));
    $('#variant-list').replaceChildren(...this.variants.map((v) => el('option', { value: v.name })));
    $('#btn-new').onclick = () => { $('#new-error').classList.add('hidden'); $('#dlg-new').showModal(); };
    $('#new-go').onclick = (e) => {
      e.preventDefault();
      try {
        this.loadedKey = null;
        this.newGame($('#new-position').value.trim() || sel.value);
        $('#dlg-new').close();
        if (location.hash !== '#/analysis') history.replaceState(null, '', '#/analysis');
      } catch (err) {
        $('#new-error').textContent = err.message;
        $('#new-error').classList.remove('hidden');
      }
    };
    // Export dialog.
    $('#btn-export').onclick = () => this.openExport();
    $$('#export-kind button').forEach((b) => (b.onclick = () => this.fillExport(b.dataset.v)));
    $('#export-copy').onclick = async () => {
      try { await navigator.clipboard.writeText($('#export-text').value); toast('Copied'); }
      catch (e) { $('#export-text').select(); document.execCommand('copy'); toast('Copied'); }
    };
    // Ctrl+V anywhere on the analysis page loads a game or position from the clipboard;
    // Ctrl+C (with no text selected) copies the position and moves as a 5DUCI line.
    const typing = (e) => e.target.closest && e.target.closest('input, textarea, select, dialog');
    document.addEventListener('paste', (e) => {
      if (this.app.route !== 'analysis' || typing(e) || document.querySelector('dialog[open]')) return;
      const text = (e.clipboardData && e.clipboardData.getData('text')) || '';
      if (!text.trim()) return;
      e.preventDefault();
      try {
        this.loadedKey = null;
        this.load(text);
        if (location.hash !== '#/analysis') history.replaceState(null, '', '#/analysis');
      } catch (err) {
        console.warn('paste:', err.message);
        toast('The clipboard holds no game or position Kairos can read', { error: true, ms: 4000 });
      }
    });
    document.addEventListener('copy', (e) => {
      if (this.app.route !== 'analysis' || typing(e) || String(getSelection()).length) return;
      try {
        // Up to the position shown (not the rest of the line).
        const pos = this.rules.call({ cmd: 'view', node: this.cur, draft: [] }).view.position;
        e.clipboardData.setData('text/plain', 'position ' + pos);
        e.preventDefault();
        toast('Copied the position (5DUCI)');
      } catch (err) {
        toast(err.message, { error: true });
      }
    });
    // Drop a file on the board.
    const area = $('#board-area');
    area.addEventListener('dragover', (e) => { e.preventDefault(); });
    area.addEventListener('drop', async (e) => {
      e.preventDefault();
      const f = e.dataTransfer.files[0];
      const text = f ? await f.text() : e.dataTransfer.getData('text');
      if (!text) return;
      if (this.app.route !== 'analysis') location.hash = '#/analysis';
      this.openLoad(text);
    });
  }

  isInSubtree(id, root) {
    for (let n = id; n != null; n = this.node(n).parent) if (n === root) return true;
    return false;
  }

  openLoad(text) {
    $('#load-error').classList.add('hidden');
    if (text != null) $('#load-text').value = text;
    this.updatePick();
    $('#dlg-load').showModal();
    if (text == null) $('#load-text').focus();
  }

  /** Several JSON lines: let the user pick one. */
  updatePick() {
    const text = $('#load-text').value;
    const lines = text.split(/\r?\n/).filter((l) => l.trim().startsWith('{'));
    const box = $('#load-pick');
    this.pickLines = lines;
    if (lines.length < 2) { box.classList.add('hidden'); this.picked = 0; return; }
    box.classList.remove('hidden');
    this.picked = 0;
    box.replaceChildren(...lines.map((l, i) => {
      let j = {};
      try { j = JSON.parse(l); } catch (e) { /* shown as unparsable */ }
      const acts = (j.moves || '').split(/\s+/).filter((t) => t === 'submit').length;
      const res = j.score == null ? '?' : (j.new_first ? j.score : 1 - j.score) === 1 ? '1-0' : (j.new_first ? j.score : 1 - j.score) === 0 ? '0-1' : '½-½';
      const row = el('div', { class: 'pick' + (i === 0 ? ' on' : '') }, el('b', { text: `#${j.game ?? i}` }), el('span', { text: res }), el('span', { class: 'muted', text: j.reason || '' }), el('span', { class: 'muted', text: `${acts} actions` }));
      row.onclick = () => { this.picked = i; $$('.pick', box).forEach((p, k) => p.classList.toggle('on', k === i)); };
      row.ondblclick = () => { this.picked = i; $('#load-go').click(); };
      return row;
    }));
  }
  pickedText() {
    if (this.pickLines && this.pickLines.length > 1) return this.pickLines[this.picked || 0];
    return $('#load-text').value;
  }

  openExport() {
    this.fillExport(this.exportKind || 'pgn');
    $('#dlg-export').showModal();
  }
  fillExport(kind) {
    this.exportKind = kind;
    $$('#export-kind button').forEach((b) => b.classList.toggle('on', b.dataset.v === kind));
    let text = '';
    try {
      const headers = [];
      if (this.meta.white && this.meta.white !== 'White') headers.push(['White', this.meta.white]);
      if (this.meta.black && this.meta.black !== 'Black') headers.push(['Black', this.meta.black]);
      if (this.meta.result) headers.push(['Result', this.meta.result]);
      const r = this.rules.call({ cmd: 'export', node: this.cur, headers });
      if (kind === 'pgn') text = r.pgn;
      else if (kind === 'uci') text = 'position ' + r.uci;
      else text = `${location.origin}${location.pathname}#/analysis?uci=${encodeURIComponent(r.uci)}`;
    } catch (e) { text = e.message; }
    $('#export-text').value = text;
  }

  /** Keyboard shortcuts (the app routes keys here while the analysis tab is shown). */
  key(e) {
    const k = e.key;
    if (k === 'ArrowRight') this.next();
    else if (k === 'ArrowLeft') this.prev();
    else if (k === 'ArrowUp') this.sibling(-1);
    else if (k === 'ArrowDown') this.sibling(1);
    else if (k === 'Home') this.first();
    else if (k === 'End') this.last();
    else if (k === ' ') this.toggleAutoplay();
    else if (k === 'e' || k === 'E') this.setEngine(!(this.engineOn && !this.suppressed));
    else if (k === 'l' || k === 'L') this.openLoad();
    else if (k === 'Enter') this.input.submit();
    else if (k === 'Backspace') this.input.undo();
    else if (k === 'Escape') this.input.clear();
    else return false;
    return true;
  }
}
