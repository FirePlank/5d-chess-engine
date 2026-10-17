// The analysis board / game viewer: a game tree with variations, step-through navigation,
// book and engine actions, the recorded eval graph and live engine analysis.
import { Rules, parseInfo, threadsSupported } from './kairos.js';
import { apiGet } from './static-api.js';
import { getGame, saveGame, putGame, recordLine } from './history.js';
import { $, $$, el, toast, fmtScore, fmtNodes, winChance, recordScore, parseLan, store, confirmDialog } from './util.js';

const THROTTLE_MS = 70;
/** "Go deeper" searches the current position to this depth. */
const MAX_DEPTH = 64;
// Most engine lines (MultiPV) the Lines setting offers.
const MAX_LINES = 5;

/** Close the move list's menu, if one is open. */
function closeMenu() {
  for (const m of document.querySelectorAll('.ctx-menu')) m.remove();
}
document.addEventListener('pointerdown', (e) => { if (!e.target.closest('.ctx-menu')) closeMenu(); }, true);
document.addEventListener('keydown', (e) => { if (e.key === 'Escape') closeMenu(); });
addEventListener('resize', closeMenu);

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
    // MultiPV: how many engine lines to show (the engine searches that many best actions).
    this.lines = Math.max(1, Math.min(MAX_LINES, +(store('engineSettings') || {}).lines || 1));
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
    this.newGame('Standard - Turn Zero', { quiet: true });
  }

  // ---------------------------------------------------------------- activation
  activate(params) {
    // While you play Kairos, analysis here would take CPU from the engine: off unless asked.
    this.suppressed = this.engineOn && this.playing();
    this.input.attach(this.source);
    this.board.setOverlay({ readonly: false });
    this.applyFlip();
    if (params.get('file')) {
      const file = params.get('file'), index = +(params.get('index') || 0);
      const key = `${file}#${index}`;
      if (this.loadedKey !== key) {
        this.loadedKey = key;
        this.loadRemote(file, index);
        return;
      }
    } else if (params.get('game')) {
      // A game from Your games (played against Kairos, or a saved analysis).
      const key = 'game:' + params.get('game');
      if (this.loadedKey !== key) {
        this.loadedKey = key;
        this.loadSaved(+params.get('game'));
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
    // Keep a turn the user gave the board here for when Analysis is shown again.
    this.flipped = this.board.flipped;
  }

  /** Show the board from Analysis's own side (Play turns the shared board to its game's). */
  applyFlip() {
    this.board.setFlipped(!!this.flipped);
    $('#tool-flip').classList.toggle('on', this.board.flipped);
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
  load(text, position = null, { source = null, quiet = false, prepare = null } = {}) {
    const r = this.rules.call({ cmd: 'load', text, position: position || undefined });
    this.setGame(r.tree, r.meta, source);
    // What was loaded, so a saved analysis of it keeps the record's scores and book marks.
    this.loadedText = text;
    this.loadedPosition = position;
    // Open at the end of the main line, like a finished game (or where `prepare` says).
    this.cur = prepare ? prepare(r) : r.mainline[r.mainline.length - 1];
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
    this.loadedText = null;
    this.loadedPosition = null;
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
    // First continuations shown as variations ("Force variation"), by node id.
    this.forced = new Set();
    this.board.clearMarks();
    // Node ids start over with every game: a "go deeper" on the last one does not carry over.
    this.deeperNode = null;
    this.gameKey++;
    this.input.attach(this.source);
    this.rootPlayer = this.rules.call({ cmd: 'view', node: 0, draft: [] }).view.player;
    // A new game is seen from the side to move at its start (White in Turn Zero, whose first
    // board is a Black one).
    this.flipped = this.rootPlayer === 1;
    if (this.app.route === 'analysis') this.applyFlip();
    this.renderGameCard();
    this.renderVariant();
  }

  /** The variant picker shows the game's variant (Custom position for any other start). */
  renderVariant() {
    const name = this.meta && this.meta.variant;
    const known = this.variants.some((v) => v.name === name);
    $('#variant-name').textContent = known ? name : 'Custom position';
  }

  /**
   * The variant picker at the top of the sidebar (as on infinitechess.org): the button opens
   * the variant groups, a group opens its variants, and picking one starts a new board of it.
   * A custom position is loaded with Load game.
   */
  bindVariantPicker() {
    const btn = $('#variant-display'), menu = $('#variant-dropdown');
    const groups = [];
    for (const v of this.variants) {
      const [group, rest] = v.name.includes(' - ') ? v.name.split(' - ') : [v.name, null];
      let g = groups.find((x) => x.name === group);
      if (!g) groups.push((g = { name: group, variants: [] }));
      g.variants.push({ name: v.name, label: rest || v.name });
    }
    const desc = {
      Standard: '8\u00d78, the main game',
      Simple: 'Smaller sets, 6\u00d76 and 7\u00d77',
      Small: '5\u00d75',
      'Very Small': '4\u00d74',
      Misc: 'Several timelines, odd sets',
      Focused: 'One kind of piece',
      'Checkmate Practice': 'Mate with a lone piece',
    };
    const chevron = (pts, cls) => {
      const s = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      s.setAttribute('class', cls);
      s.setAttribute('viewBox', '0 0 24 24');
      s.innerHTML = `<polyline points="${pts}" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>`;
      return s;
    };
    const close = () => {
      menu.classList.remove('open');
      btn.classList.remove('open');
      btn.setAttribute('aria-expanded', 'false');
    };
    // The group of the variant on the board is marked, so the list shows where you are.
    const curGroup = () => groups.find((x) => x.variants.some((v) => v.name === (this.meta && this.meta.variant)));
    const showGroups = () => menu.replaceChildren(...groups.map((g) => el('button', {
      type: 'button', class: 'variant-group-item' + (g === curGroup() ? ' on' : ''), role: 'menuitem',
      onclick: (e) => { e.stopPropagation(); showGroup(g); },
    }, el('span', { class: 'group-name', text: g.name }), el('span', { class: 'group-desc', text: desc[g.name] || '' }), chevron('9 6 15 12 9 18', 'group-arrow'))));
    const showGroup = (g) => {
      const cur = this.meta && this.meta.variant;
      menu.replaceChildren(
        el('button', { type: 'button', class: 'variant-list-back', onclick: (e) => { e.stopPropagation(); showGroups(); } },
          chevron('15 18 9 12 15 6', 'back-arrow'), el('span', { class: 'group-name', text: g.name }), el('span', { class: 'group-desc', text: desc[g.name] || '' })),
        ...g.variants.map((v) => el('button', {
          type: 'button', class: 'variant-item' + (v.name === cur ? ' on' : ''), role: 'menuitem', title: v.name,
          onclick: () => { close(); this.pickVariant(v.name); },
        }, v.label)));
      menu.scrollTop = 0;
    };
    btn.onclick = (e) => {
      e.stopPropagation();
      if (menu.classList.contains('open')) return close();
      showGroups();
      menu.classList.add('open');
      btn.classList.add('open');
      btn.setAttribute('aria-expanded', 'true');
    };
    menu.addEventListener('click', (e) => e.stopPropagation());
    document.addEventListener('click', close);
    document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && menu.classList.contains('open')) close(); });
  }

  /** Start a new board of variant `name`. */
  pickVariant(name) {
    try {
      this.stopAutoplay();
      this.loadedKey = null;
      this.newGame(name);
      if (location.hash !== '#/analysis') history.replaceState(null, '', '#/analysis');
    } catch (err) {
      toast(err.message, { error: true });
    }
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
  /** The main line: first children from the root, stopping before a forced variation. */
  mainline() {
    const ml = [0];
    let c = 0;
    while (this.node(c).children.length && !this.forced.has(this.node(c).children[0])) { c = this.node(c).children[0]; ml.push(c); }
    return ml;
  }
  /** Whether `id` is on the main line. */
  isMainLine(id) {
    for (let c = id; this.node(c).parent != null; c = this.node(c).parent) {
      if (this.node(this.node(c).parent).children[0] !== c || this.forced.has(c)) return false;
    }
    return true;
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
    this.renderBook();
  }

  /**
   * The opening book's actions for this position, as a hint (the engine still searches):
   * theory moves to play, and struck through, known moves that are not recommended.
   */
  renderBook() {
    const box = $('#book-hint');
    let moves = [];
    try { moves = this.rules.call({ cmd: 'book', node: this.cur }).moves; } catch (e) { moves = []; }
    box.classList.toggle('hidden', !moves.length);
    box.replaceChildren(...(moves.length ? [el('span', { class: 'book-label', text: 'Book' })] : []), ...moves.map((m) => {
      const title = m.play ? `Opening theory (weight ${m.weight})` : 'Known theory, not recommended';
      const span = el('span', { class: 'book-move' + (m.play ? '' : ' know'), title, text: m.pgn });
      span.addEventListener('pointerdown', (e) => {
        e.preventDefault();
        try {
          const r = this.rules.call({ cmd: 'play', node: this.cur, lan: m.lan });
          this.tree = r.tree;
          this.goto(r.node);
        } catch (err) {
          toast(err.message, { error: true });
        }
      });
      return span;
    }));
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
    if (m.format && m.format !== 'new' && !(m.source && m.source.game != null)) parts.push({ sprt: 'SPRT record', '5dpgn': '5dpgn', '5duci': '5DUCI' }[m.format] || m.format);
    if (m.reason) parts.push(m.reason);
    if (m.actions) parts.push(`${m.actions} action${m.actions === 1 ? '' : 's'}`);
    if (m.book) parts.push(`${m.book} book (grey)`);
    if (m.game != null) parts.push(`game ${m.game}${m.pair != null ? `, pair ${m.pair}` : ''}`);
    // A saved game's name on a line of its own above the rest.
    const pills = parts.map((t, i) => el('span', { class: i ? 'sep' : '', text: t }));
    if (m.name) pills.unshift(el('span', { class: 'game-name', text: m.name }));
    if (m.source && m.source.file) {
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
    // Empty only with no actions at all (a forced first action leaves the main line empty but
    // is shown as a variation below).
    if (!this.node(0).children.length) {
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
    // The main line ends before a forced variation: all of the last action's continuations
    // are variations.
    for (const v of this.node(ml[ml.length - 1]).children) frag.append(el('div', { class: 'mv-row' }, this.variationEl(v)));
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

  /**
   * Point the engine at the current position (as infinitechess.org's ceval). `restart`: search
   * it afresh from depth 1 (a Lines change) instead of resuming the engine's last search of it.
   */
  restartEngine({ restart = false } = {}) {
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
    // A position newly shown whose cached result has fewer lines than now wanted: drop it and
    // search afresh rather than show a short list until the engine catches up.
    if (node !== this.lastSearchedNode && (this.cache.get(node)?.multiPv || 1) < this.lines) this.cache.delete(node);
    this.lastSearchedNode = node;
    // Already searched to the target depth with enough lines: show it without searching again.
    const cached = this.cache.get(node);
    if (cached && cached.depth >= depth && (cached.multiPv || 1) >= this.lines) {
      this.engine.stop();
      this.searching = { node, key, blackToMove, finished: true };
      this.renderEngine();
      return;
    }
    // After Lines was raised, the fresh search's shallower lines may replace the deeper cached
    // ones (so the new lines show at once) until it gets as deep again.
    this.searching = { node, key, blackToMove, started: performance.now(), multiPv: this.lines, regress: restart && !!this.allowRegression };
    this.allowRegression = false;
    // The movetime only bounds the search (and lets a stop interrupt it); the depth ends it.
    // `resume`: when the engine's last search was this position (go deeper, a raised depth, a
    // stopped search revisited), it continues from the depth reached instead of from 1; never
    // after a Lines change, whose search has to produce every line.
    const multi = this.lines > 1 ? { multiPV: this.lines } : {};
    this.engine.search({ position: v.position, depth, movetime: 86_400_000, resume: !restart, ...multi }).then((id) => {
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
      // MultiPV: `multipv k` numbers the lines (1 the best); without it there is one line.
      const k = info.multipv || 1;
      const prev = this.cache.get(s.node);
      // Line 1 starts a depth's set of lines; the others join the set of their own depth (the
      // lines of one depth arrive together, line 1 first). A shallower set than the cached one
      // is kept only right after Lines was raised (`regress`), until the search is as deep.
      if (k === 1) {
        if (!prev || info.depth >= prev.depth || s.regress) {
          this.cache.set(s.node, { ...info, lines: [{ ...info }], multiPv: s.multiPv || 1, at: performance.now() });
          if (prev && info.depth >= prev.depth) s.regress = false;
        }
      } else if (prev && prev.depth === info.depth && k <= MAX_LINES) {
        prev.lines = prev.lines || [prev];
        prev.lines[k - 1] = { ...info };
      }
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
    // Lines (MultiPV): the best first, each with its score and clickable actions.
    const lines = $('#engine-lines');
    const firsts = new Set();
    const shown = this.engineOn && info && info.pv
      ? (info.lines || [info]).filter((l) => {
        if (!l || !l.pv) return false;
        // A line repeating an earlier line's first action is that action's weaker follow-up.
        const first = l.pv.split('submit')[0].trim();
        if (firsts.has(first)) return false;
        firsts.add(first);
        return true;
      }).slice(0, this.lines)
      : [];
    if (shown.length) {
      const key = this.cur + '|' + shown.map((l) => l.pv).join('|');
      if (this.pvKey !== key) {
        this.pvKey = key;
        const base = this.node(this.cur);
        lines.replaceChildren(...shown.map((line) => {
          let acts = [];
          try { acts = this.rules.call({ cmd: 'line_san', node: this.cur, line: line.pv }).actions; } catch (e) { acts = []; }
          const moves = el('div', { class: 'line-moves' });
          acts.forEach((a, i) => {
            const { num, black } = this.moveNumber({ ply: base.ply + 1 + i });
            const span = el('span', { class: 'line-move', title: a.lan }, (i === 0 || !black) ? el('span', { class: 'num', text: black ? `${num}…` : `${num}.` }) : null, a.san);
            span.addEventListener('pointerdown', (e) => { e.preventDefault(); this.playPv(line.pv, i + 1); });
            moves.append(span);
          });
          return el('div', { class: 'engine-line' }, el('span', { class: 'line-eval', text: fmtScore(line) }), moves);
        }));
        // Arrows for each line's first action, weaker lines fainter.
        const player = this.view ? this.view.player : 0;
        this.board.setOverlay({
          pv: shown.flatMap((line, r) => (line.pv.split('submit')[0] || '').trim().split(/\s+/).map(parseLan).filter(Boolean)
            .map((m) => ({ ...m, c: player, alpha: r === 0 ? 1 : Math.max(0.25, 0.7 - 0.15 * (r - 1)) }))),
        });
      } else {
        lines.querySelectorAll('.line-eval').forEach((ev, i) => { if (shown[i]) ev.textContent = fmtScore(shown[i]); });
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
    const threads = $('#set-threads'), hash = $('#set-hash'), depth = $('#set-depth'), lines = $('#set-lines');
    lines.max = String(MAX_LINES);
    lines.value = String(this.lines);
    depth.max = String(MAX_DEPTH);
    depth.value = String(this.depth);
    const cores = navigator.hardwareConcurrency || 2;
    threads.max = String(threadsSupported ? Math.max(1, Math.min(16, cores)) : 1);
    const cur = { threads: this.engine.threads, hashMb: this.engine.hashMb };
    threads.value = String(cur.threads);
    hash.value = String(cur.hashMb);
    const show = () => {
      $('#set-threads-v').textContent = threads.value;
      $('#set-depth-v').textContent = depth.value;
      $('#set-lines-v').textContent = lines.value;
      $('#set-note').textContent = threadsSupported
        ? `${cores} logical cores here; leave one free for the page. Beyond about 8 threads the search gets no faster.`
        : 'This page is not cross-origin isolated, so the engine runs on one thread.';
    };
    show();
    btn.onclick = () => {
      const open = panel.classList.toggle('hidden') === false;
      btn.setAttribute('aria-expanded', String(open));
    };
    const apply = () => {
      const s = { threads: +threads.value, hashMb: +hash.value, depth: +depth.value, lines: +lines.value };
      store('engineSettings', s);
      const linesChanged = s.lines !== this.lines;
      // Raising Lines lets the restarted search's first, shallower lines replace the cache.
      this.allowRegression = s.lines > this.lines;
      this.depth = s.depth;
      this.lines = s.lines;
      this.pvKey = null;
      this.engine.configure(s);
      // Show the new line count at once (fewer lines when lowered), before the engine re-runs.
      this.renderEngine();
      if (this.engineOn) this.restartEngine({ restart: linesChanged });
    };
    threads.oninput = show;
    depth.oninput = show;
    lines.oninput = show;
    lines.onchange = apply;
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
    $('#moves').addEventListener('contextmenu', (e) => {
      const c = e.target.closest('[data-id]');
      if (!c) return;
      e.preventDefault();
      c.blur();
      this.openMoveMenu(+c.dataset.id, e.clientX, e.clientY);
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
    $('#variant-list').replaceChildren(...this.variants.map((v) => el('option', { value: v.name })));
    this.bindVariantPicker();
    // Export dialog.
    $('#btn-export').onclick = () => this.openExport();
    $('#btn-save').onclick = () => this.openSave();
    $('#save-go').onclick = (e) => { e.preventDefault(); this.saveAnalysis(false); };
    $('#save-copy').onclick = (e) => { e.preventDefault(); this.saveAnalysis(true); };
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

  // ---------------------------------------------------------------- Your games
  /**
   * The whole tree as saved: per action its LAN, comment, whether it is a forced variation,
   * and its continuations in order (`k`; the first is the main one).
   */
  snapshot(id = 0) {
    const n = this.node(id);
    const out = { lan: n.lan || '', k: n.children.map((c) => this.snapshot(c)) };
    if (n.comments.length) out.c = n.comments.join(' ');
    if (this.forced.has(id)) out.f = 1;
    return out;
  }

  /** The LANs from the start to node `id`. */
  pathTo(id) {
    const out = [];
    for (let c = id; this.node(c).parent != null; c = this.node(c).parent) out.unshift(this.node(c).lan);
    return out;
  }

  /** The Save dialog: a name and the players (a game opened from Your games can be updated). */
  async openSave() {
    const m = this.meta || {};
    const existing = m.source && m.source.game != null ? await getGame(m.source.game).catch(() => null) : null;
    this.saveTarget = existing;
    const date = new Date().toLocaleDateString(undefined, { dateStyle: 'medium' });
    const players = m.white && m.black && !(m.white === 'White' && m.black === 'Black') ? `${m.white} vs ${m.black}` : m.variant || 'Analysis';
    $('#save-name').value = (existing && existing.name) || m.name || `${players} · ${date}`;
    $('#save-white').value = m.white && m.white !== 'White' ? m.white : (existing && existing.white) || '';
    $('#save-black').value = m.black && m.black !== 'Black' ? m.black : (existing && existing.black) || '';
    $('#save-copy').classList.toggle('hidden', !existing);
    $('#save-go').textContent = existing ? 'Update' : 'Save';
    $('#dlg-save').showModal();
    $('#save-name').select();
  }

  /** Save this game and its analysis to Your games (`copy`: as a new one even if opened from there). */
  async saveAnalysis(copy) {
    const name = $('#save-name').value.trim();
    if (!name) { $('#save-name').focus(); return; }
    const m = this.meta || {};
    const existing = copy ? null : this.saveTarget;
    const rec = {
      kind: existing ? existing.kind || 'analysis' : 'analysis',
      name,
      white: $('#save-white').value.trim() || 'White',
      black: $('#save-black').value.trim() || 'Black',
      variant: m.variant || null,
      position: this.rules.call({ cmd: 'view', node: 0, draft: [] }).view.position,
      moves: this.mainline().slice(1).map((id) => this.node(id).lan),
      result: m.result || (existing && existing.result) || null,
      reason: m.reason || (existing && existing.reason) || null,
      tree: this.snapshot(),
      cur: this.pathTo(this.cur),
      base: this.loadedText || null,
      basePosition: this.loadedPosition || null,
      saved: Date.now(),
    };
    // A game you played keeps its record (what was played, how it ended); the analysis goes on top.
    if (existing && existing.kind === 'played') {
      for (const k of ['variant', 'position', 'moves', 'result', 'reason', 'base', 'basePosition']) delete rec[k];
    }
    try {
      let id;
      if (existing) {
        id = existing.id;
        await putGame({ ...existing, ...rec, id });
      } else {
        id = await saveGame({ ...rec, started: Date.now(), ended: Date.now() });
      }
      this.meta.source = { game: id };
      this.meta.name = name;
      this.meta.white = rec.white;
      this.meta.black = rec.black;
      this.loadedKey = 'game:' + id;
      history.replaceState(null, '', `#/analysis?game=${id}`);
      $('#dlg-save').close();
      this.renderGameCard();
      toast(existing ? 'Updated in Your games' : 'Saved to Your games');
    } catch (e) {
      toast('Could not save: ' + e.message, { error: true });
    }
  }

  /**
   * Open a game from Your games: its record (as loaded or played), then a saved analysis's
   * tree on top (variations added, their order, comments and forced variations restored,
   * removed actions removed), at the position it was saved at.
   */
  async loadSaved(id) {
    let g;
    try { g = await getGame(id); } catch (e) { g = null; }
    if (!g) {
      toast('That game is no longer in Your games', { error: true });
      return this.newGame('Standard - Turn Zero', { quiet: true });
    }
    const prepare = (r) => {
      if (g.tree) this.applyTree(0, g.tree);
      Object.assign(this.meta, { name: g.name || null, white: g.white || this.meta.white, black: g.black || this.meta.black });
      if (g.result) this.meta.result = g.result;
      if (g.reason) this.meta.reason = g.reason;
      if (g.moves) this.meta.actions = g.moves.length;
      this.renderGameCard();
      if (!g.cur) return r.mainline[r.mainline.length - 1];
      let cur = 0;
      for (const lan of g.cur) {
        const next = this.node(cur).children.find((c) => this.node(c).lan === lan);
        if (next == null) break;
        cur = next;
      }
      return cur;
    };
    try {
      this.load(g.base || recordLine(g), g.basePosition || null, { quiet: true, source: { game: id }, prepare });
    } catch (e) {
      toast('Could not open the game: ' + e.message, { error: true });
    }
  }

  /** Make node `id`'s subtree match the saved `snap` (see `snapshot`). */
  applyTree(id, snap) {
    const comment = snap.c || '';
    if ((this.node(id).comments.join(' ')) !== comment) this.tree = this.rules.call({ cmd: 'comment', node: id, text: comment }).tree;
    const want = snap.k || [];
    const ids = want.map((c) => {
      let child = this.node(id).children.find((k) => this.node(k).lan === c.lan);
      if (child == null) {
        const r = this.rules.call({ cmd: 'play', node: id, lan: c.lan });
        this.tree = r.tree;
        child = r.node;
      }
      if (c.f) this.forced.add(child);
      this.applyTree(child, c);
      return child;
    });
    // Actions removed before saving are removed again; then the saved order.
    for (const k of [...this.node(id).children]) {
      if (!ids.includes(k)) this.tree = this.rules.call({ cmd: 'delete', node: k }).tree;
    }
    for (const k of [...ids].reverse()) this.tree = this.rules.call({ cmd: 'promote', node: k, fork: true }).tree;
  }

  /**
   * The move list's menu for action `id` (as infinitechess.org's analysis board): Promote
   * variation (first among its siblings), Make main line (first at every level), Force
   * variation (show the first continuation as a variation) and Delete from here. Each of the
   * first three then shows the action on the board.
   */
  openMoveMenu(id, x, y) {
    closeMenu();
    const n = this.node(id);
    if (n.parent == null) return;
    const parent = this.node(n.parent);
    const { num, black } = this.moveNumber(n);
    const title = `${num}${black ? '…' : '.'} ${n.san || n.lan}`;
    const item = (label, fn, cls = '') => el('button', { type: 'button', class: 'ctx-item' + cls, onclick: () => { closeMenu(); fn(); } }, label);
    const items = [];
    if (parent.children[0] !== id) items.push(item('Promote variation', () => this.promoteNode(id, true)));
    if (!this.isMainLine(id)) items.push(item('Make main line', () => this.promoteNode(id, false)));
    if (parent.children[0] === id && !this.forced.has(id)) items.push(item('Force variation', () => { this.forced.add(id); this.afterTreeEdit(id); }));
    items.push(item('Delete from here', () => this.deleteNode(id), ' danger'));
    const menu = el('div', { class: 'ctx-menu', role: 'menu' }, el('div', { class: 'ctx-title', title, text: title }), ...items);
    document.body.append(menu);
    const r = menu.getBoundingClientRect();
    menu.style.left = `${Math.max(4, Math.min(x, innerWidth - r.width - 4))}px`;
    menu.style.top = `${Math.max(4, Math.min(y, innerHeight - r.height - 4))}px`;
    menu.querySelector('.ctx-item').focus({ preventScroll: true });
  }

  /** Promote `id` among its siblings (`fork`) or to the main line, and show it. */
  promoteNode(id, fork) {
    this.tree = this.rules.call({ cmd: 'promote', node: id, fork }).tree;
    // Promoting undoes a forced variation on the way.
    for (let c = id; c != null; c = this.node(c).parent) {
      this.forced.delete(c);
      if (fork) break;
    }
    this.afterTreeEdit(id);
  }

  /** After the tree's order changed: show action `id` on the board and redraw the list. */
  afterTreeEdit(id) {
    if (id !== this.cur) this.goto(id);
    else this.show({ follow: false });
  }

  /**
   * Delete action `id` and everything after it. Viewing a position inside it moves the view to
   * the position before it, and the action being built there (it belonged to the deleted
   * position) is cleared.
   */
  deleteNode(id) {
    const r = this.rules.call({ cmd: 'delete', node: id });
    this.tree = r.tree;
    const moved = this.isInSubtree(this.cur, id);
    if (moved) {
      this.stopAutoplay();
      this.cur = r.node;
      this.input.reset();
    }
    if (this.deeperNode != null && this.isInSubtree(this.deeperNode, id)) this.deeperNode = null;
    this.show({ follow: false });
    if (moved) this.restartEngine();
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
