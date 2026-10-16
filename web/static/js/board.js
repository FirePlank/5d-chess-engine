// Multiverse renderer: every board of every timeline on one pannable, zoomable canvas.
//
// World layout (1 unit = 1 square): the board of timeline l at half-turn v = 2t + c (c = 1 for
// black's boards) has its top-left corner at (v * pitchX, row(l) * pitchY). Timelines are rows
// (positive lines below from White's side), half-turns are columns, so the present is a
// vertical band. Flipping mirrors the rows and turns every board around.

const PIECE_FILES = {
  k: 'king', q: 'queen', b: 'bishop', n: 'knight', r: 'rook', p: 'pawn',
  u: 'unicorn', d: 'dragon', w: 'brawn', s: 'princess', y: 'royalqueen', c: 'commonking',
};
const GAP_X = 1.7;
const GAP_Y = 2.4;

/** Parse a board FEN (rows top to bottom, `*` after unmoved pieces) into a W*H array. */
export function parseFen(fen, w, h) {
  const sq = new Array(w * h).fill(null);
  const rows = fen.split('/');
  for (let r = 0; r < rows.length && r < h; r++) {
    const y = h - 1 - r;
    let x = 0;
    for (const ch of rows[r]) {
      if (ch >= '0' && ch <= '9') x += ch.charCodeAt(0) - 48;
      else if (ch === '*') { if (x > 0 && sq[y * w + x - 1]) sq[y * w + x - 1].unmoved = true; }
      else { if (x < w) sq[y * w + x] = { p: ch, unmoved: false }; x++; }
    }
  }
  return sq;
}

class PieceSet {
  constructor() {
    this.images = {};
    this.cache = new Map();
    this.ready = Promise.all(Object.entries(PIECE_FILES).flatMap(([k, name]) => ['white', 'black'].map((col) => new Promise((resolve) => {
      const img = new Image();
      img.onload = () => resolve();
      img.onerror = () => resolve();
      img.src = `pieces/${name}-${col}.svg`;
      this.images[(col === 'white' ? k.toUpperCase() : k)] = img;
    }))));
  }
  /** A raster of piece `p` at about `px` pixels (power-of-two buckets). */
  get(p, px) {
    const size = Math.max(8, Math.min(256, 2 ** Math.ceil(Math.log2(px))));
    const key = p + size;
    let c = this.cache.get(key);
    if (!c) {
      const img = this.images[p];
      if (!img || !img.complete || !img.naturalWidth) return null;
      c = document.createElement('canvas');
      c.width = c.height = size;
      c.getContext('2d').drawImage(img, 0, 0, size, size);
      this.cache.set(key, c);
    }
    return c;
  }
}

const pieces = new PieceSet();

export class MultiverseView {
  constructor(canvas) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');
    this.view = null;
    this.boards = new Map(); // "l,v" -> {l, t, c, v, squares}
    this.flipped = false;
    this.cam = { x: 0, y: 0, s: 40 };
    this.target = null; // camera animation target
    this.overlay = { selected: null, targets: [], pv: [], hover: null, readonly: false };
    this.colors = {};
    this.dirty = true;
    this.onSquare = null;
    this.onHover = null;
    this.pointers = new Map();
    this.readColors();
    pieces.ready.then(() => this.redraw());
    new ResizeObserver(() => this.resize()).observe(canvas);
    this.bindInput();
    this.resize();
    const loop = () => {
      this.step();
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
  }

  readColors() {
    const cs = getComputedStyle(document.documentElement);
    const v = (n) => cs.getPropertyValue(n).trim();
    this.colors = {
      bg: v('--mv-bg'), lane: v('--mv-lane'), text: v('--mv-text'), textStrong: v('--mv-text-strong'),
      frameW: v('--mv-frame-w'), frameB: v('--mv-frame-b'), outline: v('--mv-outline'), present: v('--mv-present'),
      branch: v('--mv-branch'), presentEdge: v('--mv-present-edge'), inactive: parseFloat(v('--mv-inactive')) || 0.5,
      light: v('--sq-light'), dark: v('--sq-dark'), last: v('--hl-last'), select: v('--hl-select'), target: v('--hl-target'),
      draft: v('--hl-draft'), check: v('--hl-check'), arMove: v('--ar-move'), arCheck: v('--ar-check'), arPv: v('--ar-pv'),
      arDraft: v('--ar-draft'), peek: v('--hl-peek'), peekSelect: v('--hl-peek-select'), accent: v('--accent'), font: getComputedStyle(document.body).fontFamily,
    };
    this.redraw();
  }

  redraw() { this.dirty = true; }

  resize() {
    const dpr = window.devicePixelRatio || 1;
    const r = this.canvas.getBoundingClientRect();
    this.w = r.width;
    this.h = r.height;
    this.canvas.width = Math.max(1, Math.round(r.width * dpr));
    this.canvas.height = Math.max(1, Math.round(r.height * dpr));
    this.dpr = dpr;
    if (!this.fitted && this.view && this.w > 0) this.fit(false, true);
    this.redraw();
  }

  // ---------------------------------------------------------------- geometry
  get bw() { return this.view ? this.view.size[0] : 8; }
  get bh() { return this.view ? this.view.size[1] : 8; }
  get px() { return this.bw + GAP_X; }
  get py() { return this.bh + GAP_Y; }
  row(l) { return this.flipped ? -l : l; }
  boardX(v) { return v * this.px; }
  boardY(l) { return this.row(l) * this.py; }
  /** World coordinates of the centre of square (x, y) on board (l, v). */
  squareCenter(l, v, x, y) {
    const col = this.flipped ? this.bw - 1 - x : x;
    const rw = this.flipped ? y : this.bh - 1 - y;
    return [this.boardX(v) + col + 0.5, this.boardY(l) + rw + 0.5];
  }
  toScreen(wx, wy) { return [(wx - this.cam.x) * this.cam.s + this.w / 2, (wy - this.cam.y) * this.cam.s + this.h / 2]; }
  toWorld(sx, sy) { return [(sx - this.w / 2) / this.cam.s + this.cam.x, (sy - this.h / 2) / this.cam.s + this.cam.y]; }

  /** The square under a screen point: {l, t, c, v, x, y} or null. */
  pick(sx, sy) {
    if (!this.view) return null;
    const [wx, wy] = this.toWorld(sx, sy);
    const v = Math.floor(wx / this.px);
    const rowIdx = Math.floor(wy / this.py);
    const l = this.flipped ? -rowIdx : rowIdx;
    const bx = wx - v * this.px, by = wy - rowIdx * this.py;
    if (bx < 0 || by < 0 || bx >= this.bw || by >= this.bh) return null;
    if (!this.boards.has(`${l},${v}`)) return null;
    const col = Math.floor(bx), rw = Math.floor(by);
    const x = this.flipped ? this.bw - 1 - col : col;
    const y = this.flipped ? rw : this.bh - 1 - rw;
    return { l, v, t: v >> 1, c: v & 1, x, y };
  }

  bounds() {
    let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
    for (const b of this.boards.values()) {
      const bx = this.boardX(b.v), by = this.boardY(b.l);
      x0 = Math.min(x0, bx); x1 = Math.max(x1, bx + this.bw);
      y0 = Math.min(y0, by); y1 = Math.max(y1, by + this.bh);
    }
    if (x0 === Infinity) return [0, 0, this.bw, this.bh];
    return [x0, y0, x1, y1];
  }

  /** Move the camera so that a world rectangle fits (with margins). */
  frame(x0, y0, x1, y1, animate = true, maxScale = 80) {
    const padX = 48, padY = 56;
    const s = Math.max(2, Math.min(maxScale, (this.w - 2 * padX) / Math.max(1, x1 - x0), (this.h - 2 * padY) / Math.max(1, y1 - y0)));
    this.moveTo({ x: (x0 + x1) / 2, y: (y0 + y1) / 2, s }, animate);
  }
  /**
   * The whole multiverse in view. With `readable` (on load), a multiverse too long to show at a
   * readable size keeps every timeline in view around the present.
   */
  fit(animate = true, readable = false) {
    if (!this.w) return;
    this.fitted = true;
    const [x0, y0, x1, y1] = this.bounds();
    const sx = (this.w - 80) / (x1 - x0 + 2), sy = (this.h - 72) / (y1 - y0 + 2);
    const MIN = 4;
    if (!readable || Math.min(sx, sy) >= MIN) return this.frame(x0 - 1.5, y0 - 1.5, x1 + 0.5, y1 + 0.5, animate, 60);
    const s = Math.max(MIN, Math.min(sy, 18));
    // The present a little left of centre, so the boards after it show too; never past the end.
    const vw = this.w / s;
    const present = this.view ? this.boardX(2 * this.view.present + this.view.player) + this.bw / 2 : x1;
    const x = Math.min(present + vw * 0.15, x1 + 1 - vw / 2);
    this.moveTo({ x, y: (y0 + y1) / 2, s }, animate);
  }
  /** Show the boards on the present, as large as they fit. */
  goPresent(animate = true) {
    if (!this.view) return;
    const v = 2 * this.view.present + this.view.player;
    const [a0, a1] = this.view.active;
    const ys = [this.boardY(a0), this.boardY(a1)];
    const x0 = this.boardX(Math.max(0, v - 3)), x1 = this.boardX(v) + this.bw;
    let [y0, y1] = [Math.min(...ys) - 1.5, Math.max(...ys) + this.bh + 0.5];
    // Many timelines: keep squares readable, centred on the main line.
    const maxRows = Math.max(1, Math.floor((this.h - 112) / 14 / this.py));
    if ((y1 - y0) / this.py > maxRows) {
      const mid = this.boardY(0) + this.bh / 2;
      y0 = mid - (maxRows * this.py) / 2;
      y1 = mid + (maxRows * this.py) / 2;
    }
    this.frame(x0 - 1, y0, x1 + 1, y1, animate, 60);
  }
  moveTo(t, animate = true) {
    if (animate) this.target = t;
    else { this.cam = { ...t }; this.target = null; }
    this.redraw();
  }
  zoomAt(factor, sx = this.w / 2, sy = this.h / 2) {
    const base = this.target || this.cam;
    const s = Math.max(1.2, Math.min(220, base.s * factor));
    const [wx, wy] = [(sx - this.w / 2) / base.s + base.x, (sy - this.h / 2) / base.s + base.y];
    this.target = { x: wx - (sx - this.w / 2) / s, y: wy - (sy - this.h / 2) / s, s };
    this.redraw();
  }
  /**
   * Keep a world rectangle comfortably in view: when it leaves the middle of the view, centre
   * on it (or, if it is wider than the view, show its right end: the latest turns).
   */
  ensureVisible(x0, y0, x1, y1) {
    const c = this.target || this.cam;
    const vw = this.w / c.s, vh = (this.h - 24) / c.s;
    const mx = vw * 0.1, my = vh * 0.1;
    let { x, y } = c;
    if (x0 < x - vw / 2 + mx || x1 > x + vw / 2 - mx) x = x1 - x0 > vw - 2 * mx ? x1 + mx - vw / 2 : (x0 + x1) / 2;
    if (y0 < y - vh / 2 + my || y1 > y + vh / 2 - my) y = (y0 + y1) / 2;
    if (x !== c.x || y !== c.y) this.moveTo({ x, y, s: c.s });
  }

  setFlipped(f) {
    if (f === this.flipped) return;
    const c = this.cam;
    this.flipped = f;
    this.cam = { ...c, y: -c.y + (this.bh) };
    if (this.target) this.target = { ...this.target, y: -this.target.y + this.bh };
    this.redraw();
  }

  // ---------------------------------------------------------------- data
  /** Show a position (a `view` from the rules core). */
  setView(view, { follow = true } = {}) {
    const prev = this.view;
    this.view = view;
    this.boards.clear();
    const [w, h] = view.size;
    for (const [l, t, c, fen] of view.boards) {
      const v = 2 * t + c;
      this.boards.set(`${l},${v}`, { l, t, c, v, squares: parseFen(fen, w, h) });
    }
    this.phantom = (view.phantom || []).map(([l, t, c, fen]) => ({ l, t, c, v: 2 * t + c, squares: parseFen(fen, w, h) }));
    this.timelines = new Map(view.timelines.map((tl) => [tl.l, tl]));
    if (!prev || prev.size[0] !== w || prev.size[1] !== h || !this.fitted) {
      if (this.w) this.fit(false, true);
    } else if (follow) {
      this.followAction();
    }
    this.redraw();
  }

  /** Keep the boards the last action touched in view. */
  followAction() {
    const v = this.view;
    const pts = [];
    for (const m of [...(v.last || []), ...(v.draft || [])]) {
      for (const p of [m.from, m.to]) {
        const c = m.c ?? v.player;
        pts.push([p[0], 2 * p[1] + c]);
        pts.push([p[0], 2 * p[1] + c + 1]);
      }
    }
    const present = 2 * v.present + v.player;
    for (const tl of v.timelines) if (tl.status === 'mandatory') pts.push([tl.l, present]);
    if (!pts.length) return;
    let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
    for (const [l, vv] of pts) {
      x0 = Math.min(x0, this.boardX(vv)); x1 = Math.max(x1, this.boardX(vv) + this.bw);
      const y = this.boardY(l);
      y0 = Math.min(y0, y); y1 = Math.max(y1, y + this.bh);
    }
    this.ensureVisible(x0, y0, x1, y1);
  }

  setOverlay(o) {
    Object.assign(this.overlay, o);
    this.redraw();
  }

  // ---------------------------------------------------------------- input
  bindInput() {
    const cv = this.canvas;
    let down = null, pinch = null;
    cv.addEventListener('pointerdown', (e) => {
      cv.setPointerCapture(e.pointerId);
      this.pointers.set(e.pointerId, { x: e.offsetX, y: e.offsetY });
      if (this.pointers.size === 1) {
        down = { x: e.offsetX, y: e.offsetY, cam: { ...this.cam }, moved: false, button: e.button };
        this.target = null;
      } else if (this.pointers.size === 2) {
        const [a, b] = [...this.pointers.values()];
        pinch = { d: Math.hypot(a.x - b.x, a.y - b.y), cam: { ...this.cam }, mx: (a.x + b.x) / 2, my: (a.y + b.y) / 2 };
        down = null;
      }
    });
    cv.addEventListener('pointermove', (e) => {
      if (this.pointers.has(e.pointerId)) this.pointers.set(e.pointerId, { x: e.offsetX, y: e.offsetY });
      if (pinch && this.pointers.size === 2) {
        const [a, b] = [...this.pointers.values()];
        const d = Math.hypot(a.x - b.x, a.y - b.y);
        const mx = (a.x + b.x) / 2, my = (a.y + b.y) / 2;
        const s = Math.max(1.2, Math.min(220, pinch.cam.s * d / Math.max(1, pinch.d)));
        const wx = (pinch.mx - this.w / 2) / pinch.cam.s + pinch.cam.x;
        const wy = (pinch.my - this.h / 2) / pinch.cam.s + pinch.cam.y;
        this.cam = { x: wx - (mx - this.w / 2) / s, y: wy - (my - this.h / 2) / s, s };
        this.redraw();
        return;
      }
      if (down) {
        const dx = e.offsetX - down.x, dy = e.offsetY - down.y;
        if (!down.moved && Math.hypot(dx, dy) > 5) {
          down.moved = true;
          cv.classList.add('dragging');
        }
        if (down.moved) {
          this.cam = { ...this.cam, x: down.cam.x - dx / this.cam.s, y: down.cam.y - dy / this.cam.s };
          this.redraw();
        }
        return;
      }
      const sq = this.pick(e.offsetX, e.offsetY);
      const key = sq ? `${sq.l},${sq.v},${sq.x},${sq.y}` : null;
      if (key !== this.hoverKey) {
        this.hoverKey = key;
        this.overlay.hover = sq;
        cv.classList.toggle('pointing', !!sq && ((!this.overlay.readonly && this.isInteresting(sq)) || this.isPeekable(sq)));
        if (this.onHover) this.onHover(sq);
        this.redraw();
      }
    });
    const up = (e) => {
      this.pointers.delete(e.pointerId);
      cv.classList.remove('dragging');
      if (this.pointers.size < 2) pinch = null;
      if (down && !down.moved && e.type === 'pointerup' && down.button === 0) {
        const sq = this.pick(e.offsetX, e.offsetY);
        if (this.onSquare) this.onSquare(sq, e);
      }
      down = null;
    };
    cv.addEventListener('pointerup', up);
    cv.addEventListener('pointercancel', up);
    cv.addEventListener('pointerleave', () => {
      if (this.overlay.hover) { this.overlay.hover = null; this.hoverKey = null; this.redraw(); if (this.onHover) this.onHover(null); }
    });
    cv.addEventListener('wheel', (e) => {
      e.preventDefault();
      if (e.shiftKey && !e.ctrlKey) {
        const c = this.target || this.cam;
        this.moveTo({ ...c, x: c.x + (e.deltaY || e.deltaX) / c.s }, false);
        return;
      }
      const k = e.deltaMode === 1 ? 0.06 : 0.0022;
      this.zoomAt(Math.exp(-e.deltaY * k * (e.ctrlKey ? 2.5 : 1)), e.offsetX, e.offsetY);
    }, { passive: false });
    cv.addEventListener('dblclick', (e) => {
      const sq = this.pick(e.offsetX, e.offsetY);
      if (!sq) return;
      const x = this.boardX(sq.v), y = this.boardY(sq.l);
      this.frame(x - 0.6, y - 0.6, x + this.bw + 0.6, y + this.bh + 0.6, true, 110);
    });
    cv.addEventListener('contextmenu', (e) => e.preventDefault());
  }

  /** The piece on a square, if any. */
  pieceAt(sq) {
    const b = this.boards.get(`${sq.l},${sq.v}`);
    return b ? b.squares[sq.y * this.bw + sq.x] : null;
  }

  /** A piece on a board of its own colour (its side's turn there): its moves can be shown. */
  isPeekable(sq) {
    const pc = this.pieceAt(sq);
    return !!pc && (pc.p === pc.p.toLowerCase() ? 1 : 0) === sq.c;
  }

  /** Squares worth a pointer cursor: own pieces on playable boards and move targets. */
  isInteresting(sq) {
    const o = this.overlay;
    if (o.targets.some((t) => t[0] === sq.l && 2 * t[1] + this.view.player === sq.v && t[2] === sq.x && t[3] === sq.y)) return true;
    const b = this.boards.get(`${sq.l},${sq.v}`);
    const tl = this.timelines.get(sq.l);
    if (!b || !tl || sq.c !== this.view.player) return false;
    if (tl.end[0] !== sq.t || tl.end[1] !== sq.c) return false;
    const pc = b.squares[sq.y * this.bw + sq.x];
    return !!pc && (pc.p === pc.p.toLowerCase()) === (this.view.player === 1);
  }

  // ---------------------------------------------------------------- drawing
  step() {
    if (this.target) {
      const t = this.target, c = this.cam;
      const k = 0.2;
      const ls = Math.log(c.s) + (Math.log(t.s) - Math.log(c.s)) * k;
      this.cam = { x: c.x + (t.x - c.x) * k, y: c.y + (t.y - c.y) * k, s: Math.exp(ls) };
      if (Math.abs(t.x - this.cam.x) * this.cam.s < 0.3 && Math.abs(t.y - this.cam.y) * this.cam.s < 0.3 && Math.abs(t.s - this.cam.s) < 0.01) {
        this.cam = { ...t };
        this.target = null;
      }
      this.dirty = true;
    }
    if (this.dirty) {
      this.dirty = false;
      this.draw();
    }
  }

  draw() {
    const { ctx, colors: C } = this;
    const dpr = this.dpr || 1;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.fillStyle = C.bg;
    ctx.fillRect(0, 0, this.w, this.h);
    if (!this.view) return;
    const v = this.view;
    const s = this.cam.s;
    ctx.setTransform(dpr * s, 0, 0, dpr * s, dpr * (this.w / 2 - this.cam.x * s), dpr * (this.h / 2 - this.cam.y * s));
    const [vx0, vy0] = this.toWorld(0, 0);
    const [vx1, vy1] = this.toWorld(this.w, this.h);
    const vMin = Math.floor(vx0 / this.px) - 1, vMax = Math.ceil(vx1 / this.px) + 1;
    const [lMin, lMax] = v.lines;
    const [aMin, aMax] = v.active;
    const presentV = 2 * v.present + v.player;
    const firstV = Math.min(...v.timelines.map((t) => 2 * t.start[0] + t.start[1]));
    const lastV = Math.max(...v.timelines.map((t) => 2 * t.end[0] + t.end[1]), presentV);
    const px1 = 1 / s; // one screen pixel in world units

    // Timeline lanes: every other line faintly shaded, like table rows.
    const laneX0 = this.boardX(firstV) - GAP_X / 2, laneX1 = this.boardX(lastV + 1) - GAP_X / 2;
    ctx.fillStyle = C.lane;
    for (let l = lMin; l <= lMax; l++) {
      if ((l & 1) === 0) continue;
      const y = this.boardY(l) - GAP_Y / 2;
      if (y > vy1 || y + this.py < vy0) continue;
      ctx.fillRect(laneX0, y, laneX1 - laneX0, this.py);
    }
    // The present: a faint column over the active timelines.
    {
      const ya = this.boardY(aMin), yb = this.boardY(aMax);
      const lo = Math.min(ya, yb) - GAP_Y / 2, hi = Math.max(ya, yb) + this.bh + GAP_Y / 2;
      const px0 = this.boardX(presentV) - GAP_X / 2;
      ctx.fillStyle = C.present;
      ctx.fillRect(px0, lo, this.px, hi - lo);
      // Its edges, so the present reads as a line through the active timelines.
      ctx.fillStyle = C.presentEdge;
      const ew = Math.max(0.06, 2 * px1);
      ctx.fillRect(px0, lo, ew, hi - lo);
      ctx.fillRect(px0 + this.px - ew, lo, ew, hi - lo);
    }
    // Branch connectors and timeline continuity, behind the boards.
    ctx.strokeStyle = C.branch;
    ctx.lineWidth = Math.max(0.06, 1.25 * px1);
    ctx.beginPath();
    for (const [l, from, t, c] of v.branches || []) {
      const v1 = 2 * t + c;
      const x0 = this.boardX(v1 - 1) + this.bw, y0 = this.boardY(from) + this.bh / 2;
      const x1 = this.boardX(v1), y1 = this.boardY(l) + this.bh / 2;
      ctx.moveTo(x0, y0);
      ctx.bezierCurveTo(x0 + GAP_X * 0.8, y0, x1 - GAP_X * 0.8, y1, x1, y1);
    }
    for (const tl of v.timelines) {
      const y = this.boardY(tl.l) + this.bh / 2;
      if (y < vy0 - this.py || y > vy1 + this.py) continue;
      const a = Math.max(2 * tl.start[0] + tl.start[1], vMin), b = Math.min(2 * tl.end[0] + tl.end[1], vMax);
      if (b <= a) continue;
      ctx.moveTo(this.boardX(a) + this.bw, y);
      ctx.lineTo(this.boardX(b), y);
    }
    ctx.stroke();

    // Boards, then square highlights, then pieces (so highlights sit under them, as on
    // lichess), then move dots and arrows on top.
    const visible = [];
    for (const b of this.boards.values()) {
      if (b.v < vMin || b.v > vMax) continue;
      const by = this.boardY(b.l);
      if (by > vy1 || by + this.bh < vy0) continue;
      const tl = this.timelines.get(b.l);
      visible.push([b, tl && !tl.active ? C.inactive : 1, tl, false]);
    }
    for (const b of this.phantom) {
      if (b.v >= vMin && b.v <= vMax) visible.push([b, 0.35, null, true]);
    }
    for (const [b, alpha, tl, phantom] of visible) this.drawBoard(b, alpha, s, tl, phantom);
    this.drawHighlights();
    for (const [b, alpha] of visible) this.drawPieces(b, alpha, s);
    this.drawTargets();
    this.drawArrows(s);

    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.drawLabels(vMin, vMax, lMin, lMax);
  }

  drawBoard(b, alpha, s, tl, phantom = false) {
    const { ctx, colors: C, bw, bh } = this;
    const x0 = this.boardX(b.v), y0 = this.boardY(b.l);
    const px1 = 1 / s;
    ctx.globalAlpha = alpha;
    // Frame colour: whose move it is on that board (White light, Black dark).
    const fr = Math.max(0.12, Math.min(0.3, 4 * px1));
    ctx.fillStyle = b.c ? C.frameB : C.frameW;
    ctx.fillRect(x0 - fr, y0 - fr, bw + 2 * fr, bh + 2 * fr);
    ctx.strokeStyle = C.outline;
    ctx.lineWidth = px1;
    ctx.strokeRect(x0 - fr, y0 - fr, bw + 2 * fr, bh + 2 * fr);
    ctx.fillStyle = C.dark;
    ctx.fillRect(x0, y0, bw, bh);
    if (s >= 2) {
      ctx.fillStyle = C.light;
      for (let r = 0; r < bh; r++) {
        for (let col = 0; col < bw; col++) {
          const x = this.flipped ? bw - 1 - col : col;
          const y = this.flipped ? r : bh - 1 - r;
          if ((x + y) % 2 === 1) ctx.fillRect(x0 + col, y0 + r, 1, 1);
        }
      }
    }
    // Boards the side to move can play: a thin accent outline (solid when it must be played).
    if (tl && !phantom && tl.end[0] === b.t && tl.end[1] === b.c && b.c === this.view.player) {
      const o = fr + 3 * px1;
      ctx.strokeStyle = C.accent;
      ctx.lineWidth = 2 * px1;
      if (tl.status !== 'mandatory') {
        ctx.globalAlpha = alpha * 0.55;
        ctx.setLineDash([4 * px1, 3 * px1]);
      }
      ctx.strokeRect(x0 - o, y0 - o, bw + 2 * o, bh + 2 * o);
      ctx.setLineDash([]);
    }
    ctx.globalAlpha = 1;
  }

  drawPieces(b, alpha, s) {
    const { ctx, bw, bh } = this;
    const x0 = this.boardX(b.v), y0 = this.boardY(b.l);
    ctx.globalAlpha = alpha;
    if (s >= 4) {
      for (let y = 0; y < bh; y++) {
        for (let x = 0; x < bw; x++) {
          const pc = b.squares[y * bw + x];
          if (!pc) continue;
          const col = this.flipped ? bw - 1 - x : x;
          const r = this.flipped ? y : bh - 1 - y;
          const img = pieces.get(pc.p, s * this.dpr);
          if (img) ctx.drawImage(img, x0 + col + 0.03, y0 + r + 0.03, 0.94, 0.94);
        }
      }
    } else if (s >= 1.5) {
      for (let y = 0; y < bh; y++) {
        for (let x = 0; x < bw; x++) {
          const pc = b.squares[y * bw + x];
          if (!pc) continue;
          const col = this.flipped ? bw - 1 - x : x;
          const r = this.flipped ? y : bh - 1 - y;
          ctx.fillStyle = pc.p === pc.p.toLowerCase() ? '#111' : '#fff';
          ctx.fillRect(x0 + col + 0.2, y0 + r + 0.2, 0.6, 0.6);
        }
      }
    }
    ctx.globalAlpha = 1;
  }

  squareRect(l, v, x, y) {
    const [cx, cy] = this.squareCenter(l, v, x, y);
    return [cx - 0.5, cy - 0.5];
  }

  drawHighlights() {
    const { ctx, colors: C, view: v, overlay: o } = this;
    const fill = (l, vv, x, y, color) => {
      if (!this.boards.has(`${l},${vv}`)) return;
      const [x0, y0] = this.squareRect(l, vv, x, y);
      ctx.fillStyle = color;
      ctx.fillRect(x0, y0, 1, 1);
    };
    // The last action: origin and destination, on the boards before and after it.
    for (const m of v.last || []) {
      const c = m.c;
      const [fl, ft, fx, fy] = m.from, [tl, tt, tx, ty] = m.to;
      fill(fl, 2 * ft + c, fx, fy, C.last);
      fill(fl, 2 * ft + c + 1, fx, fy, C.last);
      fill(tl, 2 * tt + c, tx, ty, C.last);
      let arrive = tl;
      const br = (v.branches || []).find((b) => b[1] === tl && 2 * b[2] + b[3] === 2 * tt + c + 1);
      if (br && (fl !== tl || ft !== tt)) arrive = br[0];
      fill(arrive, 2 * tt + c + 1, tx, ty, C.last);
    }
    for (const m of v.draft || []) {
      fill(m.from[0], 2 * m.from[1] + v.player, m.from[2], m.from[3], C.draft);
      fill(m.to[0], 2 * m.to[1] + v.player, m.to[2], m.to[3], C.draft);
    }
    // A royal in check: a red glow under the piece, as on lichess.
    for (const ch of v.checks || []) {
      const [tl, tt, tx, ty] = ch.to;
      const vv = 2 * tt + ch.c;
      if (!this.boards.has(`${tl},${vv}`) && !this.phantom.some((b) => b.l === tl && b.v === vv)) continue;
      const [cx, cy] = this.squareCenter(tl, vv, tx, ty);
      const g = ctx.createRadialGradient(cx, cy, 0, cx, cy, 0.7);
      g.addColorStop(0, C.check);
      g.addColorStop(0.3, C.check);
      g.addColorStop(1, 'rgba(169,0,0,0)');
      ctx.fillStyle = g;
      ctx.fillRect(cx - 0.5, cy - 0.5, 1, 1);
    }
    if (o.hover && !o.readonly && this.isInteresting(o.hover)) fill(o.hover.l, o.hover.v, o.hover.x, o.hover.y, 'rgba(255,255,255,.18)');
    if (o.selected) fill(o.selected.l, 2 * o.selected.t + o.selected.c, o.selected.x, o.selected.y, o.peek ? C.peekSelect : C.select);
  }

  /** Where the selected piece can go: dots on empty squares, corners on captures. */
  drawTargets() {
    const { ctx, colors: C, view: v, overlay: o } = this;
    const tc = o.targetColor ?? v.player;
    for (const t of o.targets) {
      const vv = 2 * t[1] + tc;
      const b = this.boards.get(`${t[0]},${vv}`);
      if (!b) continue;
      const [cx, cy] = this.squareCenter(t[0], vv, t[2], t[3]);
      ctx.fillStyle = o.peek ? C.peek : C.target;
      if (b.squares[t[3] * this.bw + t[2]]) {
        // A capture: the square's corners, like lichess.
        const [x0, y0] = [cx - 0.5, cy - 0.5];
        ctx.beginPath();
        ctx.rect(x0, y0, 1, 1);
        ctx.arc(cx, cy, 0.6, 0, Math.PI * 2, true);
        ctx.fill('evenodd');
      } else {
        ctx.beginPath();
        ctx.arc(cx, cy, 0.17, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }

  drawArrows(s) {
    const { colors: C, view: v, overlay: o } = this;
    const width = Math.max(0.09, Math.min(0.16, 2.6 / s));
    for (const m of v.last || []) this.arrow(m.from, m.to, m.c, C.arMove, width);
    for (const m of v.draft || []) this.arrow(m.from, m.to, v.player, C.arDraft, width);
    for (const m of o.pv || []) this.arrow(m.from, m.to, m.c, C.arPv, width);
    for (const ch of v.checks || []) this.arrow(ch.from, ch.to, ch.c, C.arCheck, width * 0.85);
  }

  /** An arrow between squares; jumps between boards bend so they clear the boards between. */
  arrow(from, to, c, color, width) {
    const { ctx } = this;
    const [x0, y0] = this.squareCenter(from[0], 2 * from[1] + c, from[2], from[3]);
    const [x1, y1] = this.squareCenter(to[0], 2 * to[1] + c, to[2], to[3]);
    const dx = x1 - x0, dy = y1 - y0;
    const len = Math.hypot(dx, dy);
    if (len < 0.01) return;
    const sameBoard = from[0] === to[0] && from[1] === to[1];
    const bend = sameBoard ? 0 : Math.min(1.6, 0.12 * len);
    const nx = -dy / len, ny = dx / len;
    const mx = (x0 + x1) / 2 + nx * bend, my = (y0 + y1) / 2 + ny * bend;
    const head = Math.max(0.32, width * 3);
    let ex = x1 - mx, ey = y1 - my;
    const el = Math.hypot(ex, ey) || 1;
    ex /= el; ey /= el;
    const bx = x1 - ex * head, by = y1 - ey * head;
    ctx.save();
    ctx.strokeStyle = color;
    ctx.fillStyle = color;
    ctx.lineWidth = width;
    ctx.lineCap = 'butt';
    ctx.beginPath();
    ctx.moveTo(x0, y0);
    if (bend) ctx.quadraticCurveTo(mx, my, bx, by);
    else ctx.lineTo(bx, by);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(bx - ey * head * 0.62, by + ex * head * 0.62);
    ctx.lineTo(bx + ey * head * 0.62, by - ex * head * 0.62);
    ctx.closePath();
    ctx.fill();
    ctx.restore();
  }

  /** Timeline names down the left edge and turn numbers along the top, in screen space. */
  drawLabels(vMin, vMax, lMin, lMax) {
    const { ctx, colors: C, view: v } = this;
    const s = this.cam.s;
    const size = Math.round(Math.max(11, Math.min(13, s * 0.42)));
    ctx.font = `${size}px ${C.font}`;
    ctx.textBaseline = 'middle';
    // Gutters so labels never sit on a board.
    ctx.fillStyle = C.bg;
    ctx.globalAlpha = 0.86;
    ctx.fillRect(0, 0, this.w, 22);
    ctx.globalAlpha = 1;
    ctx.textAlign = 'left';
    for (let l = lMin; l <= lMax; l++) {
      const [, sy] = this.toScreen(0, this.boardY(l) + this.bh / 2);
      if (sy < 26 || sy > this.h - 4) continue;
      const tl = this.timelines.get(l);
      const text = 'L' + (tl ? tl.label : l);
      const tw = ctx.measureText(text).width;
      ctx.fillStyle = C.bg;
      ctx.globalAlpha = 0.86;
      ctx.fillRect(0, sy - 9, tw + 14, 18);
      ctx.globalAlpha = 1;
      ctx.fillStyle = tl && tl.status === 'mandatory' ? C.textStrong : C.text;
      ctx.fillText(text, 7, sy);
    }
    ctx.textAlign = 'center';
    const presentT = v.present;
    const step = Math.max(1, Math.ceil(44 / (s * this.px * 2)));
    for (let t = Math.max(0, (vMin >> 1) - 1); t <= (vMax >> 1) + 1; t++) {
      if (t % step && t !== presentT) continue;
      const [sx] = this.toScreen(this.boardX(2 * t) + this.bw / 2, 0);
      if (sx < 34 || sx > this.w - 16) continue;
      ctx.fillStyle = t === presentT ? C.presentEdge : C.text;
      ctx.fillText(t === presentT && s * this.px > 70 ? `T${t} · present` : 'T' + t, sx, 11);
    }
  }
}

