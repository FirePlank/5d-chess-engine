// Board editor: set up any 5D position, as 5DFEN. Pieces are placed, moved and removed on any
// board (lichess's palette: click or drag to paint, the same piece again removes it,
// right-click swaps colour); boards and whole timelines are added, moved and copied on the
// multiverse grid (5dchess-designer's "+" slots, infinitechess's selections). Every change is
// one undo step; the rules core checks the position as it changes (`inspect`), and it opens on
// the analysis board or as a game against Kairos.
import { $, $$, el, toast, store } from './util.js';
import { pieceImage } from './board.js';

const PIECES = ['K', 'Q', 'R', 'B', 'N', 'P', 'U', 'D', 'S', 'W', 'C', 'Y'];
const NAMES = {
  K: 'king', Q: 'queen', R: 'rook', B: 'bishop', N: 'knight', P: 'pawn',
  U: 'unicorn', D: 'dragon', S: 'princess', W: 'brawn', C: 'common king', Y: 'royal queen',
};
const FILES = { K: 'king', Q: 'queen', R: 'rook', B: 'bishop', N: 'knight', P: 'pawn', U: 'unicorn', D: 'dragon', S: 'princess', W: 'brawn', C: 'commonking', Y: 'royalqueen' };
// The pieces whose `*` (not yet moved) means something: castling, and the double step.
const UNMOVED = new Set(['K', 'R', 'P', 'W']);
const ROYAL = new Set(['K', 'Y']);
const PROMOS = ['Q', 'R', 'B', 'N', 'U', 'D', 'S', 'C'];
const HISTORY_MAX = 400;

const key = (l, v) => `${l},${v}`;
const unkey = (k) => k.split(',').map(Number);
const isWhite = (p) => p[0] === p[0].toUpperCase();
const kind = (p) => p[0].toUpperCase();
const sideName = (white) => (white ? 'White' : 'Black');
/** A timeline's label: L0, L-1… or, with an even number of starting timelines, L+0 and L-0. */
const label = (l, even) => (even ? (l >= 0 ? `+${l}` : `-${-l - 1}`) : String(l));

/** One board's squares (index y * w + x, a1 first) from its FEN rows: 'P', 'P*' or null. */
function squaresOf(fen, w, h) {
  const sq = new Array(w * h).fill(null);
  const rows = fen.split('/');
  for (let r = 0; r < rows.length && r < h; r++) {
    const y = h - 1 - r;
    let x = 0, run = '';
    for (const ch of rows[r]) {
      if (ch >= '0' && ch <= '9') { run += ch; continue; }
      if (run) { x += +run; run = ''; }
      if (ch === '*') {
        const i = y * w + x - 1;
        if (x > 0 && sq[i] && UNMOVED.has(kind(sq[i]))) sq[i] = sq[i][0] + '*';
      } else if (ch !== '+') {
        if (x < w) sq[y * w + x] = ch;
        x++;
      }
    }
  }
  return sq;
}

/** A board's FEN rows (top rank first), `*` after unmoved kings, rooks, pawns and brawns. */
function fenOf(sq, w, h) {
  const rows = [];
  for (let y = h - 1; y >= 0; y--) {
    let row = '', run = 0;
    for (let x = 0; x < w; x++) {
      const p = sq[y * w + x];
      if (!p) { run++; continue; }
      if (run) { row += run; run = 0; }
      row += p;
    }
    if (run) row += run;
    rows.push(row);
  }
  return rows.join('/');
}

/** Whether a piece placed on (x, y) starts out unmoved: kings and rooks on their side's back
 * rank, pawns and brawns on its second (as in every official variant). */
function unmovedAt(p, y, h) {
  const k = kind(p);
  if (!UNMOVED.has(k)) return false;
  const rank = isWhite(p) ? y : h - 1 - y;
  return k === 'K' || k === 'R' ? rank === 0 : rank === 1;
}
const withFlag = (p, on) => (on && UNMOVED.has(kind(p)) ? p[0] + '*' : p[0]);

/** The position being edited. */
class Model {
  constructor(w = 8, h = 8, even = false, promos = ['Q']) {
    this.w = w;
    this.h = h;
    this.even = even;
    this.promos = promos;
    this.boards = new Map(); // key(l, v) -> squares; v = 2t + c (c = 1 on Black's boards)
  }
  clone() {
    const m = new Model(this.w, this.h, this.even, [...this.promos]);
    for (const [k, sq] of this.boards) m.boards.set(k, sq.slice());
    return m;
  }
  empty() { return new Array(this.w * this.h).fill(null); }
  /** Timelines: l -> {min, max} half-turns. */
  rows() {
    const rows = new Map();
    for (const k of this.boards.keys()) {
      const [l, v] = unkey(k);
      const r = rows.get(l);
      if (!r) rows.set(l, { min: v, max: v });
      else { r.min = Math.min(r.min, v); r.max = Math.max(r.max, v); }
    }
    return rows;
  }
  /** Boards in 5DFEN order: timelines from the middle out, then by turn. */
  ordered() {
    const d = (l) => (this.even ? (l >= 0 ? l : -l - 1) : Math.abs(l));
    return [...this.boards.entries()].map(([k, sq]) => [...unkey(k), sq]).sort((a, b) => {
      if (d(a[0]) !== d(b[0])) return d(a[0]) - d(b[0]);
      if (a[0] !== b[0]) return this.even ? b[0] - a[0] : a[0] - b[0];
      return a[1] - b[1];
    });
  }
  tags() {
    return this.ordered().map(([l, v, sq]) => `[${fenOf(sq, this.w, this.h)}:${label(l, this.even)}:${v >> 1}:${v & 1 ? 'b' : 'w'}]`);
  }
  /** 5DFEN, as 5dpgn's custom-variant headers. */
  text() {
    const h = [`[Board "${this.even ? 'Custom - Even' : 'Custom - Odd'}"]`, '[Mode "5D"]', `[Size "${this.w}x${this.h}"]`];
    if (this.promos.join(',') !== 'Q') h.push(`[Promotions "${this.promos.join(',')}"]`);
    return [...h, ...this.tags()].join('\n');
  }
  /** The 5DUCI `position` argument (engines, and Play): no promotion choice there. */
  uci() {
    return `size ${this.w}x${this.h} ${this.even ? 'even' : 'odd'} fen ${this.tags().join(' ')}`;
  }
  snapshot() {
    return JSON.stringify({ w: this.w, h: this.h, even: this.even, promos: this.promos, b: [...this.boards].map(([k, sq]) => [k, fenOf(sq, this.w, this.h)]).sort() });
  }
  static fromSnapshot(s) {
    const o = JSON.parse(s);
    const m = new Model(o.w, o.h, o.even, o.promos);
    for (const [k, fen] of o.b) m.boards.set(k, squaresOf(fen, o.w, o.h));
    return m;
  }
  /** From a `view` of the rules core (its boards) and the promotion pieces. */
  static fromView(view, promos = ['Q']) {
    const [w, h] = view.size;
    const m = new Model(w, h, !!view.even, promos);
    for (const [l, t, c, fen] of view.boards) m.boards.set(key(l, 2 * t + c), squaresOf(fen, w, h));
    return m;
  }
}

/** The pieces pawns promote to, from a 5dpgn `[Promotions "..."]` header (else queen only). */
function promosOf(text) {
  const m = /\[\s*Promotions\s+"([^"]*)"\s*\]/i.exec(text);
  if (!m) return ['Q'];
  const list = PROMOS.filter((p) => m[1].toUpperCase().split(/[\s,]+/).includes(p));
  return list.length ? list : ['Q'];
}

/** A rules-core error, in the editor's words. */
function explain(err, even) {
  let m = /gap between boards on timeline L(-?\d+)/.exec(err);
  if (m) return { text: `Timeline L${label(+m[1], even)} has a gap: its boards must follow one another, every half-turn.` };
  if (/gap between timelines/.test(err)) return { text: 'A timeline is missing between others: timelines are numbered without gaps.', fix: 'close' };
  if (/Duplicate definition/.test(err)) return { text: 'Two boards are on the same timeline and half-turn.' };
  if (/Negative time/.test(err)) return { text: 'A board is before turn 0.' };
  m = /^(?:Error: )?(.*)$/.exec(err);
  return { text: m ? m[1] : err };
}

export class Editor {
  constructor(app) {
    this.app = app;
    this.board = app.board;
    this.m = new Model();
    this.tool = 'move';
    this.piece = 'Q';
    this.selection = new Set(); // board keys (Boards tool)
    this.picked = null; // {l, v, x, y}: a piece clicked to move (Move tool)
    this.gesture = null;
    this.hoverSlot = null;
    this.clipboard = null; // {w, h, boards: [[dl, dv, squares]]}
    this.history = [];
    this.at = -1;
    this.result = null; // the last `inspect`
    this.active = false;
  }

  init() {
    this.rules = this.app.pages.analysis.rules;
    this.buildPalette();
    $$('.ed-tools button').forEach((b) => { b.onclick = () => this.setTool(b.dataset.tool); });
    $('#ed-undo').onclick = () => this.undo();
    $('#ed-redo').onclick = () => this.redo();
    for (const sel of ['#ed-w', '#ed-h']) {
      $(sel).replaceChildren(...[1, 2, 3, 4, 5, 6, 7, 8].map((n) => el('option', { value: n, text: n })));
      $(sel).onchange = () => this.resize(+$('#ed-w').value, +$('#ed-h').value);
    }
    $$('#ed-layout button').forEach((b) => { b.onclick = () => this.change((m) => { m.even = b.dataset.even === '1'; }); });
    $('#ed-promos').replaceChildren(...PROMOS.map((p) => el('button', { type: 'button', dataset: { p }, title: `Promote to a ${NAMES[p]}`, onclick: () => this.togglePromo(p) },
      el('img', { src: `pieces/${FILES[p]}-white.svg`, alt: p }))));
    let variants = [];
    try { variants = this.rules.call({ cmd: 'variants' }).variants; } catch (e) { /* none */ }
    this.variants = variants;
    $('#ed-preset').replaceChildren(...variants.map((v) => el('option', { value: v.name, text: v.name })));
    $('#ed-preset').value = 'Standard';
    $('#ed-preset-load').onclick = () => this.loadPreset($('#ed-preset').value);
    $('#ed-clear').onclick = () => this.change((m) => { for (const k of m.boards.keys()) m.boards.set(k, m.empty()); });
    $('#ed-empty').onclick = () => this.change((m) => { m.boards.clear(); m.boards.set(key(0, 2), m.empty()); this.selection.clear(); }, { fit: true });
    $('#ed-extend').onclick = () => this.extend();
    $('#ed-branch').onclick = () => this.branch();
    $('#ed-sel-clear').onclick = () => this.change((m) => { for (const k of this.selection) m.boards.set(k, m.empty()); });
    $('#ed-sel-swap').onclick = () => this.swapColours();
    $('#ed-sel-copy').onclick = () => this.copy();
    $('#ed-sel-delete').onclick = () => this.deleteSelection();
    $('#ed-sel-none').onclick = () => { this.selection.clear(); this.render({ inspect: false }); };
    $('#ed-analyse').onclick = () => this.analyse();
    $('#ed-play').onclick = () => this.play();
    $('#ed-copy-fen').onclick = () => this.copyText(this.m.text(), '5DFEN copied');
    $('#ed-copy-uci').onclick = () => this.copyText(this.m.uci(), '5DUCI position copied');
    const fen = $('#ed-fen');
    let typing = null;
    fen.addEventListener('input', () => {
      clearTimeout(typing);
      typing = setTimeout(() => this.applyText(fen.value, { typed: true }), 250);
    });
    fen.addEventListener('blur', () => {
      clearTimeout(typing);
      // What was typed and does not load is dropped, as on lichess: the box shows the board.
      if (!this.applyText(fen.value, { typed: true })) this.renderText(true);
    });
    // Ctrl shortcuts (the app's own key handler leaves modified keys alone).
    document.addEventListener('keydown', (e) => this.ctrlKey(e));
    document.addEventListener('paste', (e) => this.onPaste(e));
    $('#btn-edit').onclick = () => {
      const v = this.app.pages.analysis.view;
      location.hash = '#/editor?position=' + encodeURIComponent(v ? v.position : '');
    };

    const saved = store('editor');
    try {
      if (saved) this.m = Model.fromSnapshot(saved);
      else this.m = this.modelOf('Standard') || new Model();
    } catch (e) {
      this.m = new Model();
      this.m.boards.set(key(0, 2), this.m.empty());
    }
    this.history = [this.m.snapshot()];
    this.at = 0;
  }

  // ---------------------------------------------------------------- page
  activate(params) {
    this.active = true;
    this.board.editor = this;
    this.board.clearMarks();
    this.board.setOverlay({ selected: null, targets: [], pv: [], targetColor: null, peek: false });
    this.board.fitted = false;
    const pos = params.get('position') || params.get('fen');
    if (pos) {
      const m = this.modelOf(pos);
      if (m) {
        this.m = m;
        this.selection.clear();
        this.commit();
      } else toast('Could not read that position', { error: true });
      history.replaceState(null, '', '#/editor');
    }
    this.render();
    this.board.fit(false, true);
  }

  deactivate() {
    this.active = false;
    this.board.editor = null;
    this.gesture = null;
    this.picked = null;
    this.board.fitted = false;
    this.board.canvas.style.cursor = '';
  }

  // ---------------------------------------------------------------- the rules core
  inspect(text) {
    try {
      return this.rules.call({ cmd: 'inspect', text });
    } catch (e) {
      return { valid: false, error: e.message };
    }
  }

  /** A model from any position text (5DFEN, a 5DUCI position, a variant name, a game). */
  modelOf(text) {
    const r = this.inspect(text);
    if (!r.valid) return null;
    return Model.fromView(r.view, promosOf(text));
  }

  // ---------------------------------------------------------------- edits and history
  /** Apply `fn` to the position as one undo step. */
  change(fn, { fit = false } = {}) {
    const before = this.m.snapshot();
    fn(this.m);
    if (this.m.snapshot() !== before) this.commit();
    this.render();
    if (fit) this.board.fit();
  }

  /** Record the position as an undo step (if it changed), remember it, and show it. */
  commit() {
    const snap = this.m.snapshot();
    if (this.history[this.at] === snap) return;
    this.history = this.history.slice(0, this.at + 1);
    this.history.push(snap);
    if (this.history.length > HISTORY_MAX) this.history.shift();
    this.at = this.history.length - 1;
    store('editor', snap);
  }

  undo() { this.travel(-1); }
  redo() { this.travel(1); }
  travel(d) {
    if (this.gesture) return;
    const at = this.at + d;
    if (at < 0 || at >= this.history.length) return;
    this.at = at;
    const prev = this.m;
    this.m = Model.fromSnapshot(this.history[at]);
    store('editor', this.history[at]);
    for (const k of [...this.selection]) if (!this.m.boards.has(k)) this.selection.delete(k);
    this.picked = null;
    this.render();
    if (prev.w !== this.m.w || prev.h !== this.m.h) this.board.fit();
  }

  // ---------------------------------------------------------------- tools
  setTool(tool, piece) {
    this.tool = tool;
    if (piece) this.piece = piece;
    this.picked = null;
    this.render({ inspect: false });
  }

  buildPalette() {
    const btn = (p) => el('button', { type: 'button', dataset: { piece: p }, title: `${sideName(isWhite(p))} ${NAMES[kind(p)]} (${p})`, onclick: () => this.setTool('piece', p) },
      el('img', { src: `pieces/${FILES[kind(p)]}-${isWhite(p) ? 'white' : 'black'}.svg`, alt: p }));
    $('#ed-palette').replaceChildren(
      ...PIECES.map((p) => btn(p)),
      ...PIECES.map((p) => btn(p.toLowerCase())),
    );
  }

  hint() {
    const name = `${sideName(isWhite(this.piece))} ${NAMES[kind(this.piece)]}`;
    return {
      move: 'Drag a piece to any square of any board, or click it and then the square. Drop it off the boards to remove it; hold Alt to copy it. Drag empty space to pan.',
      piece: `Click or drag to place a ${name}; on the same piece, it removes it. Right-click swaps its colour. Keys: a piece's letter (Shift for White).`,
      erase: 'Click or drag over pieces to remove them.',
      unmoved: 'A filled dot marks a king or rook that can still castle, a pawn or brawn that can still move two squares; a ring, one that has moved. Click or drag over pieces to toggle them.',
      boards: 'Click a + to add a board (a copy of its neighbour). Drag boards to move them, Ctrl+drag to copy; click a timeline\'s label to select it all, Shift+drag to select several. Del deletes.',
    }[this.tool];
  }

  // ---------------------------------------------------------------- pointer
  boardAt(slot) { return slot ? this.m.boards.get(key(slot.l, slot.v)) : null; }

  /** The timeline whose label is at screen point (sx, sy), if any. */
  labelAt(sx, sy) {
    if (sx > 46) return null;
    const b = this.board;
    for (const l of this.m.rows().keys()) {
      const [, y] = b.toScreen(0, b.boardY(l) + b.bh / 2);
      if (Math.abs(y - sy) < 11) return l;
    }
    return null;
  }

  pointerDown(info) {
    const { slot, e } = info;
    const right = e.button === 2;
    this.downAt = { sx: info.sx, sy: info.sy };
    if (!right && e.button !== 0) return false;
    const tl = right ? null : this.labelAt(info.sx, info.sy);
    if (tl != null) {
      this.gesture = { kind: 'label', l: tl, additive: e.shiftKey || e.ctrlKey || e.metaKey };
      return true;
    }
    const sq = this.boardAt(slot);
    const on = sq && slot.inside;
    if (this.tool === 'piece' && right) {
      // As on lichess: right-click swaps the colour of the piece being placed.
      this.gesture = { kind: 'none' };
      const p = this.piece;
      this.piece = isWhite(p) ? p.toLowerCase() : p.toUpperCase();
      this.render({ inspect: false });
      return true;
    }
    if (right) return false;
    if (this.tool === 'piece' || this.tool === 'erase' || this.tool === 'unmoved') {
      if (!on) return false;
      this.gesture = { kind: 'paint', before: this.m.snapshot(), done: new Set(), mode: null, last: [info.sx, info.sy] };
      this.paint(slot);
      return true;
    }
    if (this.tool === 'move') {
      if (!on) return false;
      const p = sq[slot.y * this.m.w + slot.x];
      if (!p && !this.picked) return false;
      this.gesture = { kind: p ? 'lift' : 'drop', from: slot, p, moved: false };
      return true;
    }
    if (this.tool === 'boards') {
      if (sq) {
        const k = key(slot.l, slot.v);
        const additive = e.shiftKey || e.ctrlKey || e.metaKey;
        this.gesture = { kind: 'boards', from: slot, to: slot, moved: false, additive, wasSelected: this.selection.has(k) };
        if (!this.selection.has(k)) {
          if (!additive) this.selection.clear();
          this.selection.add(k);
          this.render({ inspect: false });
        }
        return true;
      }
      if (this.isGhost(slot.l, slot.v)) {
        this.gesture = { kind: 'ghost', slot };
        return true;
      }
      if (e.shiftKey) {
        this.gesture = { kind: 'marquee', x0: slot.wx, y0: slot.wy, x1: slot.wx, y1: slot.wy, base: new Set(this.selection) };
        return true;
      }
    }
    return false;
  }

  pointerMove(info) {
    const g = this.gesture;
    if (!g) return;
    const { slot } = info;
    this.pointer = info;
    const far = Math.hypot(info.sx - this.downAt.sx, info.sy - this.downAt.sy) > 4;
    if (g.kind === 'paint') {
      this.strokeTo(info);
    } else if (g.kind === 'lift') {
      if (!g.moved && far) {
        g.moved = true;
        this.picked = null;
        this.render({ inspect: false });
      }
      this.board.redraw();
    } else if (g.kind === 'boards') {
      if (!g.moved && far) g.moved = true;
      g.to = slot;
      this.board.redraw();
    } else if (g.kind === 'marquee') {
      g.x1 = slot.wx;
      g.y1 = slot.wy;
      this.selection = new Set([...g.base, ...this.inMarquee(g)]);
      this.board.redraw();
    }
  }

  pointerUp(info) {
    const g = this.gesture;
    this.gesture = null;
    if (!g) return;
    const { slot, e } = info;
    const far = Math.hypot(info.sx - this.downAt.sx, info.sy - this.downAt.sy) > 4;
    if (g.kind === 'paint') {
      this.gesture = g;
      this.strokeTo(info);
      this.gesture = null;
      if (this.m.snapshot() !== g.before) this.commit();
      this.render();
    } else if (g.kind === 'lift' || g.kind === 'drop') {
      this.dropPiece(g, slot, e, far);
    } else if (g.kind === 'boards') {
      const dl = g.to.l - g.from.l, dv = g.to.v - g.from.v;
      if (g.moved && (dl || dv)) this.moveSelection(dl, dv, e.ctrlKey || e.metaKey || e.altKey);
      else if (!g.moved) {
        // A click: select just this board (Shift/Ctrl: toggle it in the selection).
        const k = key(g.from.l, g.from.v);
        if (g.additive && g.wasSelected) this.selection.delete(k);
        else if (!g.additive) this.selection = new Set([k]);
        this.render({ inspect: false });
      } else this.render({ inspect: false });
    } else if (g.kind === 'ghost') {
      if (slot.l === g.slot.l && slot.v === g.slot.v) this.addBoard(slot.l, slot.v);
    } else if (g.kind === 'label') {
      if (!far) this.selectTimeline(g.l, g.additive);
    } else if (g.kind === 'marquee') {
      this.render({ inspect: false });
    }
  }

  cancel() {
    const g = this.gesture;
    this.gesture = null;
    if (g && g.kind === 'paint') this.m = Model.fromSnapshot(g.before);
    this.render({ inspect: g && g.kind === 'paint' });
  }

  /** A click on empty space (not a drag, which pans): drop the selection and picked piece. */
  emptyClick() {
    if (this.picked || this.selection.size) {
      this.picked = null;
      this.selection.clear();
      this.render({ inspect: false });
    }
  }

  hover(info) {
    const slot = info ? info.slot : null;
    const prev = this.hoverSlot;
    this.hoverSlot = slot;
    this.pointer = info;
    const same = prev && slot && prev.l === slot.l && prev.v === slot.v && prev.x === slot.x && prev.y === slot.y && prev.inside === slot.inside;
    if (!same) this.board.redraw();
    let cursor = '';
    if (info) {
      const on = this.boardAt(slot) && slot.inside;
      if (this.labelAt(info.sx, info.sy) != null) cursor = 'pointer';
      else if (this.tool === 'piece' || this.tool === 'erase' || this.tool === 'unmoved') cursor = on ? 'crosshair' : '';
      else if (this.tool === 'move') cursor = on && (this.boardAt(slot)[slot.y * this.m.w + slot.x] || this.picked) ? 'pointer' : '';
      else if (this.tool === 'boards') cursor = this.boardAt(slot) ? 'move' : this.isGhost(slot.l, slot.v) ? 'copy' : '';
    }
    this.board.canvas.style.cursor = cursor;
  }

  /** A stroke from the last pointer sample to this one: every square on the way (a fast
   * pointer skips squares between its events). */
  strokeTo(info) {
    const g = this.gesture;
    const [x0, y0] = g.last;
    const n = Math.max(1, Math.ceil(Math.hypot(info.sx - x0, info.sy - y0) / Math.max(4, this.board.cam.s / 3)));
    for (let i = 1; i <= n; i++) {
      const slot = this.board.pickSlot(x0 + ((info.sx - x0) * i) / n, y0 + ((info.sy - y0) * i) / n);
      if (this.boardAt(slot) && slot.inside) this.paint(slot);
    }
    g.last = [info.sx, info.sy];
  }

  /** One square of a stroke: place, erase or toggle unmoved, once per square. */
  paint(slot) {
    const g = this.gesture;
    const k = key(slot.l, slot.v), i = slot.y * this.m.w + slot.x;
    if (g.done.has(k + ':' + i)) return;
    g.done.add(k + ':' + i);
    const sq = this.m.boards.get(k);
    const cur = sq[i];
    if (this.tool === 'piece') {
      // The stroke adds, or (begun on the same piece) removes, never both (lichess).
      if (g.mode == null) g.mode = cur && cur[0] === this.piece ? 'remove' : 'add';
      if (g.mode === 'remove') { if (cur && cur[0] === this.piece) sq[i] = null; } else sq[i] = withFlag(this.piece, unmovedAt(this.piece, slot.y, this.m.h));
    } else if (this.tool === 'erase') {
      sq[i] = null;
    } else if (this.tool === 'unmoved') {
      if (!cur || !UNMOVED.has(kind(cur))) return;
      // The first piece decides: the stroke marks pieces unmoved, or moved (infinitechess).
      if (g.mode == null) g.mode = cur.length === 1 ? 'on' : 'off';
      sq[i] = withFlag(cur, g.mode === 'on');
    }
    this.render({ inspect: false });
  }

  /** The end of a Move-tool press: a drag drops the piece, a click picks it or drops one. */
  dropPiece(g, slot, e, far) {
    const target = this.boardAt(slot) && slot.inside ? slot : null;
    if (g.kind === 'lift' && g.moved) {
      const copy = e.altKey || e.ctrlKey || e.metaKey;
      if (target && target.l === g.from.l && target.v === g.from.v && target.x === g.from.x && target.y === g.from.y) return this.render({ inspect: false });
      this.change((m) => {
        if (!copy) m.boards.get(key(g.from.l, g.from.v))[g.from.y * m.w + g.from.x] = null;
        // Off the boards: removed (lichess).
        if (target) m.boards.get(key(target.l, target.v))[target.y * m.w + target.x] = withFlag(g.p, unmovedAt(g.p, target.y, m.h));
      });
      return;
    }
    if (far) return this.render({ inspect: false });
    const p = this.picked;
    if (p && target && !(p.l === target.l && p.v === target.v && p.x === target.x && p.y === target.y)) {
      // Click-to-move: the picked piece goes to the clicked square.
      this.picked = null;
      this.change((m) => {
        const from = m.boards.get(key(p.l, p.v));
        const pc = from && from[p.y * m.w + p.x];
        if (!pc) return;
        from[p.y * m.w + p.x] = null;
        m.boards.get(key(target.l, target.v))[target.y * m.w + target.x] = withFlag(pc, unmovedAt(pc, target.y, m.h));
      });
      return;
    }
    this.picked = p && target && p.l === target.l && p.v === target.v && p.x === target.x && p.y === target.y ? null : (g.kind === 'lift' ? { l: g.from.l, v: g.from.v, x: g.from.x, y: g.from.y } : null);
    this.render({ inspect: false });
  }

  // ---------------------------------------------------------------- boards and timelines
  /** An empty slot where a board can be added: next to a timeline's ends, in a gap of one,
   * or on a new timeline next to the outermost ones. */
  isGhost(l, v) {
    if (v < 0 || this.m.boards.has(key(l, v))) return false;
    const rows = this.m.rows();
    const r = rows.get(l);
    if (r) return v >= r.min - 1 && v <= r.max + 1;
    const adj = rows.get(l - 1) || rows.get(l + 1);
    if (!adj) return false;
    return v >= adj.min && v <= adj.max + 1;
  }

  /** The ghost slots drawn: every timeline's two ends and its gaps (new timelines' slots only
   * appear under the pointer). */
  ghosts() {
    const out = [];
    const rows = this.m.rows();
    for (const [l, r] of rows) {
      for (let v = Math.max(0, r.min - 1); v <= r.max + 1; v++) if (!this.m.boards.has(key(l, v))) out.push([l, v]);
    }
    const h = this.hoverSlot;
    if (h && !rows.has(h.l) && this.isGhost(h.l, h.v)) out.push([h.l, h.v]);
    return out;
  }

  /** Add a board on an empty slot: a copy of the board before it on its timeline (or after
   * it), or for a new timeline, of the neighbouring timeline's board it would branch from. */
  addBoard(l, v) {
    const m = this.m;
    const src = m.boards.get(key(l, v - 1)) || m.boards.get(key(l, v + 1))
      || m.boards.get(key(l - 1, v - 1)) || m.boards.get(key(l + 1, v - 1))
      || m.boards.get(key(l - 1, v)) || m.boards.get(key(l + 1, v));
    this.change((mm) => {
      mm.boards.set(key(l, v), src ? src.slice() : mm.empty());
      this.selection = new Set([key(l, v)]);
    });
    // Keep the new board and the slot after it in view, for the next one.
    this.board.showBoards([[l, v - 1], [l, v], [l, v + 1]], true);
  }

  /** Move (or copy) the selected boards by (dl, dv) slots; boards already there are replaced. */
  moveSelection(dl, dv, copy) {
    const moving = [...this.selection].map((k) => [unkey(k), this.m.boards.get(k)]).filter(([, sq]) => sq);
    if (moving.some(([[, v]]) => v + dv < 0)) {
      toast('A board cannot go before turn 0', { error: true });
      return this.render({ inspect: false });
    }
    this.change((m) => {
      if (!copy) for (const [[l, v]] of moving) m.boards.delete(key(l, v));
      this.selection = new Set();
      for (const [[l, v], sq] of moving) {
        m.boards.set(key(l + dl, v + dv), sq.slice());
        this.selection.add(key(l + dl, v + dv));
      }
    });
  }

  selectTimeline(l, additive) {
    const ks = [...this.m.boards.keys()].filter((k) => unkey(k)[0] === l);
    if (!additive) this.selection.clear();
    const all = ks.every((k) => this.selection.has(k));
    for (const k of ks) { if (additive && all) this.selection.delete(k); else this.selection.add(k); }
    if (this.tool !== 'boards') this.tool = 'boards';
    this.render({ inspect: false });
  }

  inMarquee(g) {
    const b = this.board;
    const [x0, x1] = [Math.min(g.x0, g.x1), Math.max(g.x0, g.x1)];
    const [y0, y1] = [Math.min(g.y0, g.y1), Math.max(g.y0, g.y1)];
    return [...this.m.boards.keys()].filter((k) => {
      const [l, v] = unkey(k);
      const bx = b.boardX(v), by = b.boardY(l);
      return bx < x1 && bx + b.bw > x0 && by < y1 && by + b.bh > y0;
    });
  }

  /** Each selected board copied to the next half-turn of its timeline (where that is free). */
  extend() {
    this.change((m) => {
      const next = new Set();
      for (const k of this.selection) {
        const [l, v] = unkey(k);
        if (!m.boards.has(key(l, v + 1))) {
          m.boards.set(key(l, v + 1), m.boards.get(k).slice());
          next.add(key(l, v + 1));
        }
      }
      if (next.size) this.selection = next;
      else toast('The next half-turn is taken on every selected timeline');
    });
  }

  /** A new timeline from each selected board, as a jump there would make: a copy one
   * half-turn later, above the others when White branches, below when Black does. */
  branch() {
    this.change((m) => {
      const out = new Set();
      for (const k of [...this.selection].sort()) {
        const [, v] = unkey(k);
        const rows = [...m.rows().keys()];
        const white = (v & 1) === 0;
        const nl = white ? Math.max(...rows) + 1 : Math.min(...rows) - 1;
        m.boards.set(key(nl, v + 1), m.boards.get(k).slice());
        out.add(key(nl, v + 1));
      }
      this.selection = out;
    });
  }

  swapColours() {
    this.change((m) => {
      for (const k of this.selection) {
        const sq = m.boards.get(k), out = m.empty();
        for (let y = 0; y < m.h; y++) {
          for (let x = 0; x < m.w; x++) {
            const p = sq[y * m.w + x];
            if (!p) continue;
            const q = isWhite(p) ? p.toLowerCase() : p.toUpperCase();
            out[(m.h - 1 - y) * m.w + x] = q;
          }
        }
        m.boards.set(k, out);
      }
    });
  }

  deleteSelection() {
    if (!this.selection.size) return;
    if (this.selection.size >= this.m.boards.size) return toast('At least one board stays', { error: true });
    this.change((m) => {
      for (const k of this.selection) m.boards.delete(k);
      this.selection.clear();
    });
  }

  /** Renumber timelines without gaps, keeping each on its side of the middle. */
  closeGaps() {
    this.change((m) => {
      const ls = [...m.rows().keys()];
      const map = new Map();
      ls.filter((l) => l >= 0).sort((a, b) => a - b).forEach((l, i) => map.set(l, i));
      ls.filter((l) => l < 0).sort((a, b) => b - a).forEach((l, i) => map.set(l, -1 - i));
      const boards = new Map();
      for (const [k, sq] of m.boards) {
        const [l, v] = unkey(k);
        boards.set(key(map.get(l), v), sq);
      }
      m.boards = boards;
      this.selection.clear();
    });
  }

  copy() {
    if (!this.selection.size) return;
    const ks = [...this.selection].map(unkey);
    const l0 = Math.min(...ks.map((k) => k[0])), v0 = Math.min(...ks.map((k) => k[1]));
    this.clipboard = { w: this.m.w, h: this.m.h, boards: ks.map(([l, v]) => [l - l0, v - v0, this.m.boards.get(key(l, v)).slice()]) };
    toast(`${ks.length} board${ks.length === 1 ? '' : 's'} copied: Ctrl+V pastes where the pointer is`);
  }

  /** Paste copied boards with their top-left at the slot under the pointer (or as new
   * timelines below the others). */
  paste() {
    const c = this.clipboard;
    if (!c) return;
    if (c.w !== this.m.w || c.h !== this.m.h) return toast('The copied boards are another size', { error: true });
    let at = this.hoverSlot;
    if (!at) {
      const rows = [...this.m.rows().keys()];
      at = { l: Math.max(...rows) + 1, v: Math.min(...[...this.m.boards.keys()].map((k) => unkey(k)[1])) };
    }
    if (at.v < 0) return;
    this.tool = 'boards';
    this.change((m) => {
      this.selection = new Set();
      for (const [dl, dv, sq] of c.boards) {
        const k = key(at.l + dl, at.v + dv);
        m.boards.set(k, sq.slice());
        this.selection.add(k);
      }
    });
  }

  // ---------------------------------------------------------------- position settings
  resize(w, h) {
    if (w === this.m.w && h === this.m.h) return;
    let lost = 0;
    this.change((m) => {
      for (const [k, sq] of m.boards) {
        const out = new Array(w * h).fill(null);
        for (let y = 0; y < m.h; y++) {
          for (let x = 0; x < m.w; x++) {
            const p = sq[y * m.w + x];
            if (!p) continue;
            if (x < w && y < h) out[y * w + x] = p;
            else lost++;
          }
        }
        m.boards.set(k, out);
      }
      m.w = w;
      m.h = h;
    }, { fit: true });
    if (lost) toast(`${lost} piece${lost === 1 ? '' : 's'} beyond the new size removed (Ctrl+Z brings them back)`);
  }

  togglePromo(p) {
    const has = this.m.promos.includes(p);
    if (has && this.m.promos.length === 1) return toast('Pawns need something to promote to');
    this.change((m) => { m.promos = has ? m.promos.filter((x) => x !== p) : PROMOS.filter((x) => x === p || m.promos.includes(x)); });
  }

  loadPreset(name) {
    const v = this.variants.find((x) => x.name === name);
    const m = v && this.modelOf(v.position);
    if (!m) return toast('Could not load ' + name, { error: true });
    this.m = m;
    this.selection.clear();
    this.picked = null;
    this.commit();
    this.render();
    this.board.fit();
  }

  /** Load typed or pasted position text; false when it does not load. */
  applyText(text, { typed = false } = {}) {
    if (text.trim() === this.m.text().trim()) { this.showFenError(null); return true; }
    const r = this.inspect(text);
    if (!r.valid) {
      if (typed) this.showFenError(explain(r.error, this.m.even).text);
      return false;
    }
    this.showFenError(null);
    const prev = this.m;
    this.m = Model.fromView(r.view, promosOf(text));
    this.selection.clear();
    this.commit();
    this.render({ keepText: typed });
    if (prev.w !== this.m.w || prev.h !== this.m.h || prev.boards.size !== this.m.boards.size) this.board.fit();
    return true;
  }

  showFenError(msg) {
    $('#ed-fen-error').textContent = msg || '';
    $('#ed-fen-error').classList.toggle('hidden', !msg);
    $('#ed-fen').classList.toggle('bad', !!msg);
  }

  async copyText(text, msg) {
    try {
      await navigator.clipboard.writeText(text);
      toast(msg);
    } catch (e) {
      toast('Could not copy: ' + e.message, { error: true });
    }
  }

  onPaste(e) {
    if (!this.active || e.target.closest('input, textarea, select, dialog')) return;
    const text = e.clipboardData && e.clipboardData.getData('text');
    if (!text || !/[[:]/.test(text) && !this.variants.some((v) => v.name === text.trim())) return;
    e.preventDefault();
    if (this.applyText(text)) toast('Position pasted');
    else toast('The clipboard holds no position', { error: true });
  }

  // ---------------------------------------------------------------- leaving the editor
  analyse() {
    if (!this.result || !this.result.valid) return;
    location.hash = '#/analysis?uci=' + encodeURIComponent(this.m.text());
  }

  play() {
    if (!this.result || !this.result.valid) return;
    location.hash = '#/play?position=' + encodeURIComponent(this.m.uci());
  }

  // ---------------------------------------------------------------- keys
  key(e) {
    const k = e.key;
    if (k === 'Escape') {
      this.selection.clear();
      this.setTool('move');
      return true;
    }
    if ((k === 'Delete' || k === 'Backspace') && this.tool === 'boards') { this.deleteSelection(); return true; }
    if ((k === 'Delete' || k === 'Backspace') && this.picked) {
      const p = this.picked;
      this.picked = null;
      this.change((m) => { m.boards.get(key(p.l, p.v))[p.y * m.w + p.x] = null; });
      return true;
    }
    if (this.tool === 'boards' && this.selection.size && k.startsWith('Arrow')) {
      const up = this.board.flipped ? 1 : -1;
      const d = { ArrowLeft: [0, -1], ArrowRight: [0, 1], ArrowUp: [up, 0], ArrowDown: [-up, 0] }[k];
      this.moveSelection(d[0], d[1], false);
      return true;
    }
    if (k === 'x' || k === 'X') { this.setTool('erase'); return true; }
    if (k === 't' || k === 'T') { this.setTool('boards'); return true; }
    if (k === 'o' || k === 'O') { this.setTool('unmoved'); return true; }
    if (k.length === 1 && PIECES.includes(k.toUpperCase())) {
      // A piece's FEN letter: White in capitals (Shift), Black in lower case.
      this.setTool('piece', e.shiftKey ? k.toUpperCase() : k.toLowerCase());
      return true;
    }
    return false;
  }

  ctrlKey(e) {
    if (!this.active || !(e.ctrlKey || e.metaKey) || e.altKey) return;
    if (e.target.closest('input, textarea, select, dialog')) return;
    const k = e.key.toLowerCase();
    if (k === 'z' && !e.shiftKey) this.undo();
    else if (k === 'y' || (k === 'z' && e.shiftKey)) this.redo();
    else if (k === 'a') { this.selection = new Set(this.m.boards.keys()); this.tool = 'boards'; this.render({ inspect: false }); }
    else if (k === 'c' && this.selection.size && this.tool === 'boards') this.copy();
    else if (k === 'x' && this.selection.size && this.tool === 'boards') { this.copy(); this.deleteSelection(); }
    else if (k === 'v' && this.clipboard) this.paste();
    else return;
    e.preventDefault();
  }

  // ---------------------------------------------------------------- showing it
  render({ inspect = true, keepText = false } = {}) {
    if (!this.active) return;
    const m = this.m;
    if (inspect || !this.result) this.result = this.inspect(m.text());
    const lift = this.gesture && this.gesture.kind === 'lift' && this.gesture.moved ? this.gesture.from : null;
    const boards = [];
    for (const [l, v, sq] of m.ordered()) {
      let s = sq;
      if (lift && lift.l === l && lift.v === v) { s = sq.slice(); s[lift.y * m.w + lift.x] = null; }
      boards.push([l, v >> 1, v & 1, fenOf(s, m.w, m.h)]);
    }
    // Branch connectors: a timeline beyond the starting ones from the board before its first,
    // on the next timeline toward the middle.
    const rows = m.rows();
    const branches = [];
    for (const [l, rr] of rows) {
      const inner = l > 0 ? l - 1 : l < (m.even ? -1 : 0) ? l + 1 : null;
      if (inner != null && m.boards.has(key(inner, rr.min - 1))) branches.push([l, inner, rr.min >> 1, rr.min & 1]);
    }
    const r = this.result;
    let view;
    if (r.valid && r.view.size[0] === m.w && r.view.size[1] === m.h && r.view.boards.length === boards.length) {
      // Checks on the present are found on its "phantom" boards (as if it were passed); the
      // editor shows them on the boards themselves.
      const has = (sq, c) => m.boards.has(key(sq[0], 2 * sq[1] + c));
      const checks = r.view.checks.map((ch) => (has(ch.from, ch.c) && has(ch.to, ch.c) ? ch : { ...ch, c: r.view.player }));
      view = { ...r.view, boards, phantom: [], last: [], branches, draft: [], allChecks: true, checks: [...checks, ...(r.capture ? [r.capture] : [])] };
    } else {
      // Not a position (yet): the boards as they are, with no present.
      const ls = [...rows.keys()];
      const lines = ls.length ? [Math.min(...ls), Math.max(...ls)] : [0, 0];
      view = {
        size: [m.w, m.h], even: m.even, boards, present: 0, player: 0, lines, initialLines: lines, active: lines,
        timelines: [...rows].map(([l, rr]) => ({ l, label: label(l, m.even), start: [rr.min >> 1, rr.min & 1], end: [rr.max >> 1, rr.max & 1], status: 'unplayable', active: true })),
        checks: [], phantom: [], last: [], branches, draft: [], status: null, position: m.text(), noPresent: true,
      };
    }
    this.board.setView(view, { follow: false });
    this.renderPanel(keepText);
  }

  renderPanel(keepText) {
    const m = this.m;
    $$('.ed-tools button').forEach((b) => b.classList.toggle('on', b.dataset.tool === this.tool));
    $$('#ed-palette button').forEach((b) => b.classList.toggle('on', this.tool === 'piece' && b.dataset.piece === this.piece));
    $('#ed-hint').textContent = this.hint();
    $('#ed-undo').disabled = this.at <= 0;
    $('#ed-redo').disabled = this.at >= this.history.length - 1;
    $('#ed-w').value = m.w;
    $('#ed-h').value = m.h;
    $$('#ed-layout button').forEach((b) => b.classList.toggle('on', (b.dataset.even === '1') === m.even));
    $$('#ed-promos button').forEach((b) => b.classList.toggle('on', m.promos.includes(b.dataset.p)));
    // The selection.
    const sel = [...this.selection].filter((k) => m.boards.has(k));
    $('#ed-selection').classList.toggle('hidden', !sel.length || this.tool !== 'boards');
    if (sel.length) {
      const [l, v] = unkey(sel[0]);
      $('#ed-sel-title').textContent = sel.length === 1
        ? `L${label(l, m.even)} T${v >> 1} · ${v & 1 ? 'Black' : 'White'}'s board`
        : `${sel.length} boards`;
    }
    this.renderStatus();
    this.renderText(!keepText);
  }

  renderText(force) {
    const fen = $('#ed-fen');
    if (force || document.activeElement !== fen) {
      fen.value = this.m.text();
      this.showFenError(null);
    }
  }

  /** Whether the position loads, whose move it is, checks, mate, and what looks wrong. */
  renderStatus() {
    const m = this.m, r = this.result;
    const rows = [];
    const line = (cls, text, extra) => rows.push(el('div', { class: 'ed-st ' + cls }, text, extra || null));
    let playable = false;
    if (!r.valid) {
      const x = explain(r.error, m.even);
      line('bad', x.text, x.fix === 'close' ? el('button', { class: 'link-btn', text: 'Close the gap', onclick: () => this.closeGaps() }) : null);
    } else {
      const v = r.view;
      const side = sideName(v.player === 0);
      const nTl = v.timelines.length, nActive = v.timelines.filter((t) => t.active).length;
      rows.push(el('div', { class: 'ed-st head' }, el('strong', { text: `${side} to move` }), ` · present T${v.present}`));
      line('muted', `${nTl} timeline${nTl === 1 ? '' : 's'}${nActive < nTl ? `, ${nActive} active` : ''} · ${m.w}×${m.h}`);
      if (v.status === 'checkmate') line('warn', `Checkmate: ${sideName(v.player !== 0)} wins.`);
      else if (v.status === 'stalemate') line('warn', 'Stalemate: no legal action, and no check.');
      else if (v.status === 'whiteExiled' || v.status === 'blackExiled') line('warn', 'Game over: a side\'s royals are all exiled.');
      else if (v.checks.length) line('warn', `${side} is in check.`);
      playable = !r.capture && (!v.status || v.status === 'playing' || v.status === 'unknown');
      if (r.capture) line('bad', `${side} could capture a royal piece at once: ${sideName(v.player !== 0)} left it in check, which cannot happen in a game.`);
    }
    // What the rules allow but is surely not meant.
    const rowsMap = m.rows();
    const royals = { w: 0, b: 0 };
    let backPawn = false;
    for (const [l, rr] of rowsMap) {
      const sq = m.boards.get(key(l, rr.max));
      if (!sq) continue;
      sq.forEach((p, i) => {
        if (!p) return;
        if (ROYAL.has(kind(p))) royals[isWhite(p) ? 'w' : 'b']++;
        const y = Math.floor(i / m.w);
        if ((kind(p) === 'P' || kind(p) === 'W') && (isWhite(p) ? y === m.h - 1 : y === 0)) backPawn = true;
      });
    }
    for (const [c, n] of Object.entries(royals)) {
      if (!n) line('note', `${c === 'w' ? 'White' : 'Black'} has no king (or royal queen) on its latest boards: it cannot be checkmated.`);
    }
    if (backPawn) line('note', 'A pawn on its last rank can never move.');
    $('#ed-status').replaceChildren(...rows);
    $('#ed-analyse').disabled = !r.valid;
    $('#ed-play').disabled = !r.valid || !playable;
    $('#ed-play').title = !r.valid ? 'Not a valid position yet' : playable ? 'Play Kairos from this position' : 'The game is already over here';
  }

  // ---------------------------------------------------------------- drawing (board.js calls these)
  drawWorld(ctx, s) {
    const b = this.board, m = this.m;
    const px1 = 1 / s;
    const accent = b.colors.accent;
    const rect = (l, v, pad = 0) => [b.boardX(v) - pad, b.boardY(l) - pad, b.bw + 2 * pad, b.bh + 2 * pad];
    const g = this.gesture;
    // Ghost slots: where boards can be added.
    if (this.tool === 'boards' && !(g && g.kind === 'boards' && g.moved)) {
      const h = this.hoverSlot;
      for (const [l, v] of this.ghosts()) {
        const [x, y, w, hh] = rect(l, v);
        const hot = h && h.l === l && h.v === v;
        ctx.globalAlpha = hot ? 0.9 : 0.45;
        ctx.strokeStyle = hot ? accent : b.colors.text;
        ctx.lineWidth = (hot ? 2 : 1.25) * px1;
        ctx.setLineDash([5 * px1, 4 * px1]);
        ctx.strokeRect(x + 0.1, y + 0.1, w - 0.2, hh - 0.2);
        ctx.setLineDash([]);
        if (hot) {
          ctx.globalAlpha = 0.12;
          ctx.fillStyle = accent;
          ctx.fillRect(x + 0.1, y + 0.1, w - 0.2, hh - 0.2);
          ctx.globalAlpha = 0.9;
        }
        // The "+".
        const cx = x + w / 2, cy = y + hh / 2, arm = Math.min(w, hh) * 0.14;
        ctx.strokeStyle = hot ? accent : b.colors.text;
        ctx.lineWidth = Math.max(0.12, 2.5 * px1);
        ctx.beginPath();
        ctx.moveTo(cx - arm, cy); ctx.lineTo(cx + arm, cy);
        ctx.moveTo(cx, cy - arm); ctx.lineTo(cx, cy + arm);
        ctx.stroke();
        ctx.globalAlpha = 1;
      }
    }
    // Selected boards.
    if (this.tool === 'boards') {
      ctx.strokeStyle = accent;
      ctx.lineWidth = Math.max(0.1, 3 * px1);
      for (const k of this.selection) {
        if (!m.boards.has(k)) continue;
        const [l, v] = unkey(k);
        const [x, y, w, h] = rect(l, v, 0.42);
        ctx.strokeRect(x, y, w, h);
      }
    }
    // Boards being dragged: where they would land (red over boards they would replace).
    if (g && g.kind === 'boards' && g.moved) {
      const dl = g.to.l - g.from.l, dv = g.to.v - g.from.v;
      for (const k of this.selection) {
        const sq = m.boards.get(k);
        if (!sq) continue;
        const [l, v] = unkey(k);
        const tl = l + dl, tv = v + dv;
        const fake = { l: tl, v: tv, t: tv >> 1, c: tv & 1, squares: sq.map((p) => (p ? { p: p[0], unmoved: p.length > 1 } : null)) };
        if (tv >= 0) {
          b.drawBoard(fake, 0.55, s, null);
          b.drawPieces(fake, 0.6, s);
        }
        const over = tv < 0 || (m.boards.has(key(tl, tv)) && !this.selection.has(key(tl, tv)));
        ctx.strokeStyle = over ? '#d64545' : accent;
        ctx.lineWidth = Math.max(0.1, 2.5 * px1);
        const [x, y, w, h] = rect(tl, tv, 0.3);
        ctx.strokeRect(x, y, w, h);
      }
    }
    // Unmoved markers (Unmoved tool): a filled star dot for unmoved, a ring for moved.
    if (this.tool === 'unmoved') {
      for (const [k, sq] of m.boards) {
        const [l, v] = unkey(k);
        sq.forEach((p, i) => {
          if (!p || !UNMOVED.has(kind(p))) return;
          const [cx, cy] = b.squareCenter(l, v, i % m.w, Math.floor(i / m.w));
          const r = 0.15;
          ctx.beginPath();
          ctx.arc(cx + 0.31, cy - 0.31, r, 0, Math.PI * 2);
          if (p.length > 1) { ctx.fillStyle = accent; ctx.fill(); }
          ctx.lineWidth = Math.max(0.04, 1.5 * px1);
          ctx.strokeStyle = p.length > 1 ? '#fff' : 'rgba(128,128,128,.9)';
          ctx.stroke();
        });
      }
    }
    // The square under the pointer: a preview of the piece to place, or a highlight.
    const h = this.hoverSlot;
    if (h && h.inside && m.boards.has(key(h.l, h.v)) && !(g && g.kind === 'lift' && g.moved)) {
      const [x0, y0] = b.squareRect(h.l, h.v, h.x, h.y);
      const cur = m.boards.get(key(h.l, h.v))[h.y * m.w + h.x];
      if (this.tool === 'piece' && cur !== this.piece && !(cur && cur[0] === this.piece)) {
        const img = pieceImage(this.piece, s * b.dpr);
        if (img) { ctx.globalAlpha = 0.5; ctx.drawImage(img, x0 + 0.03, y0 + 0.03, 0.94, 0.94); ctx.globalAlpha = 1; }
      } else if (this.tool === 'erase' || this.tool === 'piece' || this.tool === 'unmoved' || (this.tool === 'move' && (cur || this.picked))) {
        ctx.fillStyle = this.tool === 'erase' || (this.tool === 'piece' && cur) ? 'rgba(214,69,69,.35)' : 'rgba(255,255,255,.2)';
        ctx.fillRect(x0, y0, 1, 1);
      }
    }
    if (this.picked) {
      const p = this.picked;
      const [x0, y0] = b.squareRect(p.l, p.v, p.x, p.y);
      ctx.fillStyle = b.colors.select;
      ctx.globalAlpha = 0.8;
      ctx.fillRect(x0, y0, 1, 1);
      ctx.globalAlpha = 1;
    }
    if (g && g.kind === 'lift' && g.moved) {
      // The square the piece left.
      const [x0, y0] = b.squareRect(g.from.l, g.from.v, g.from.x, g.from.y);
      ctx.fillStyle = b.colors.select;
      ctx.globalAlpha = 0.45;
      ctx.fillRect(x0, y0, 1, 1);
      ctx.globalAlpha = 1;
    }
  }

  drawScreen(ctx) {
    const b = this.board, g = this.gesture, p = this.pointer;
    if (g && g.kind === 'lift' && g.moved && p) {
      // The piece in hand, under the pointer, a little larger than the squares.
      const size = Math.max(28, b.cam.s * 1.15);
      const img = pieceImage(g.p[0], size * b.dpr);
      if (img) ctx.drawImage(img, p.sx - size / 2, p.sy - size / 2, size, size);
    }
    if (g && g.kind === 'marquee') {
      const [x0, y0] = b.toScreen(g.x0, g.y0), [x1, y1] = b.toScreen(g.x1, g.y1);
      ctx.fillStyle = b.colors.accent;
      ctx.globalAlpha = 0.1;
      ctx.fillRect(Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1 - x0), Math.abs(y1 - y0));
      ctx.globalAlpha = 0.9;
      ctx.strokeStyle = b.colors.accent;
      ctx.lineWidth = 1;
      ctx.strokeRect(Math.min(x0, x1) + 0.5, Math.min(y0, y1) + 0.5, Math.abs(x1 - x0), Math.abs(y1 - y0));
      ctx.globalAlpha = 1;
    }
    if (g && g.kind === 'boards' && g.moved && p) {
      const copy = p.e && (p.e.ctrlKey || p.e.metaKey || p.e.altKey);
      const n = this.selection.size;
      const text = `${copy ? 'Copy' : 'Move'} ${n} board${n === 1 ? '' : 's'}`;
      ctx.font = `12px ${b.colors.font}`;
      const w = ctx.measureText(text).width + 14;
      ctx.fillStyle = b.colors.accent;
      ctx.globalAlpha = 0.92;
      ctx.fillRect(p.sx + 14, p.sy + 12, w, 20);
      ctx.globalAlpha = 1;
      ctx.fillStyle = '#fff';
      ctx.textAlign = 'left';
      ctx.textBaseline = 'middle';
      ctx.fillText(text, p.sx + 21, p.sy + 22);
    }
  }
}
