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
      bg: v('--board-bg'), lane: v('--board-lane'), laneInactive: v('--board-lane-inactive'), text: v('--board-grid-text'),
      light: v('--board-light'), dark: v('--board-dark'), frameW: v('--board-frame-white'), frameB: v('--board-frame-black'),
      outline: v('--board-frame-outline'), present: v('--board-present'), presentEdge: v('--board-present-edge'),
      branch: v('--board-branch'), last: v('--board-last'), select: v('--board-select'), targetDot: v('--board-target'),
      check: v('--board-check'), arrowW: v('--board-arrow-w'), arrowB: v('--board-arrow-b'), pv: v('--board-arrow-pv'),
      draft: v('--board-draft'), accent: v('--accent'), font: getComputedStyle(document.body).fontFamily,
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
  /** Everything in view; if that would make squares too small to read, the present instead. */
  fit(animate = true, readable = false) {
    if (!this.w) return;
    this.fitted = true;
    const [x0, y0, x1, y1] = this.bounds();
    const s = Math.min((this.w - 96) / (x1 - x0 + 2), (this.h - 112) / (y1 - y0 + 2));
    if (readable && s < 14 && this.view) return this.goPresent(animate);
    this.frame(x0 - 1.5, y0 - 1.5, x1 + 0.5, y1 + 0.5, animate, 60);
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
  /** Pan just enough that a world rectangle is visible. */
  ensureVisible(x0, y0, x1, y1) {
    const c = this.target || this.cam;
    const vw = this.w / c.s, vh = this.h / c.s;
    const m = 1.2;
    let { x, y } = c;
    if (x1 - x0 + 2 * m > vw) x = (x0 + x1) / 2;
    else if (x0 - m < x - vw / 2) x = x0 - m + vw / 2;
    else if (x1 + m > x + vw / 2) x = x1 + m - vw / 2;
    if (y1 - y0 + 2 * m > vh) y = (y0 + y1) / 2;
    else if (y0 - m < y - vh / 2) y = y0 - m + vh / 2;
    else if (y1 + m > y + vh / 2) y = y1 + m - vh / 2;
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
        cv.classList.toggle('pointing', !!sq && !this.overlay.readonly && this.isInteresting(sq));
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
    // World transform.
    ctx.setTransform(dpr * s, 0, 0, dpr * s, dpr * (this.w / 2 - this.cam.x * s), dpr * (this.h / 2 - this.cam.y * s));
    const [vx0, vy0] = this.toWorld(0, 0);
    const [vx1, vy1] = this.toWorld(this.w, this.h);
    const vMin = Math.floor(vx0 / this.px) - 1, vMax = Math.ceil(vx1 / this.px) + 1;
    const [lMin, lMax] = v.lines;
    const [aMin, aMax] = v.active;
    const presentV = 2 * v.present + v.player;
    const maxV = Math.max(...v.timelines.map((t) => 2 * t.end[0] + t.end[1]), presentV) + 1;

    // Timeline lanes.
    for (let l = lMin; l <= lMax; l++) {
      const y = this.boardY(l) - GAP_Y / 2;
      if (y > vy1 || y + this.py < vy0) continue;
      const tl = this.timelines.get(l);
      ctx.fillStyle = tl && tl.active ? C.lane : C.laneInactive;
      const x0 = this.boardX(Math.min(...v.timelines.map((t) => 2 * t.start[0] + t.start[1]))) - GAP_X;
      ctx.fillRect(x0, y + 0.15, this.boardX(maxV) - x0, this.py - 0.3);
    }
    // The present.
    {
      const lo = Math.min(this.boardY(aMin), this.boardY(aMax)) - GAP_Y * 0.42;
      const hi = Math.max(this.boardY(aMin), this.boardY(aMax)) + this.bh + GAP_Y * 0.42;
      const x = this.boardX(presentV) - GAP_X * 0.38;
      const wdt = this.bw + GAP_X * 0.76;
      ctx.fillStyle = C.present;
      roundRect(ctx, x, lo, wdt, hi - lo, 0.6);
      ctx.fill();
    }
    // Branch connectors: from the board a timeline branched off to its first board.
    ctx.strokeStyle = C.branch;
    ctx.lineWidth = Math.max(0.12, 2 / s);
    for (const [l, from, t, c] of v.branches || []) {
      const v1 = 2 * t + c;
      const x0 = this.boardX(v1 - 1) + this.bw, y0 = this.boardY(from) + this.bh / 2;
      const x1 = this.boardX(v1), y1 = this.boardY(l) + this.bh / 2;
      ctx.beginPath();
      ctx.moveTo(x0, y0);
      ctx.bezierCurveTo(x0 + GAP_X * 0.9, y0, x1 - GAP_X * 0.9, y1, x1, y1);
      ctx.stroke();
    }
    // Timeline continuity: thin line between consecutive boards.
    for (const tl of v.timelines) {
      const y = this.boardY(tl.l) + this.bh / 2;
      if (y < vy0 - this.py || y > vy1 + this.py) continue;
      const a = 2 * tl.start[0] + tl.start[1], b = 2 * tl.end[0] + tl.end[1];
      if (b <= a) continue;
      ctx.strokeStyle = C.outline;
      ctx.lineWidth = Math.max(0.08, 1.2 / s);
      ctx.beginPath();
      ctx.moveTo(this.boardX(Math.max(a, vMin)) + this.bw, y);
      ctx.lineTo(this.boardX(Math.min(b, vMax)), y);
      ctx.stroke();
    }

    // Boards.
    const squarePx = s;
    for (const b of this.boards.values()) {
      if (b.v < vMin || b.v > vMax) continue;
      const by = this.boardY(b.l);
      if (by > vy1 || by + this.bh < vy0) continue;
      const tl = this.timelines.get(b.l);
      const alpha = tl && !tl.active ? 0.5 : 1;
      this.drawBoard(b, alpha, squarePx, tl);
    }
    for (const b of this.phantom) {
      if (b.v < vMin || b.v > vMax) continue;
      this.drawBoard(b, 0.35, squarePx, null, true);
    }

    this.drawHighlights(squarePx);
    this.drawArrows(squarePx);

    // Screen-space labels: timelines (left edge) and turns (top edge), sticky.
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.drawLabels(vMin, vMax, lMin, lMax);
  }

  drawBoard(b, alpha, squarePx, tl, phantom = false) {
    const { ctx, colors: C, bw, bh } = this;
    const x0 = this.boardX(b.v), y0 = this.boardY(b.l);
    ctx.globalAlpha = alpha;
    // Frame: White's boards light, Black's dark (whose move it is on that board).
    const fr = Math.min(0.42, Math.max(0.16, 5 / squarePx));
    ctx.fillStyle = b.c ? C.frameB : C.frameW;
    roundRect(ctx, x0 - fr, y0 - fr, bw + 2 * fr, bh + 2 * fr, fr * 0.9);
    ctx.fill();
    ctx.strokeStyle = C.outline;
    ctx.lineWidth = 1 / squarePx;
    ctx.stroke();
    // Squares.
    ctx.fillStyle = C.dark;
    ctx.fillRect(x0, y0, bw, bh);
    ctx.fillStyle = C.light;
    for (let r = 0; r < bh; r++) {
      for (let col = 0; col < bw; col++) {
        const x = this.flipped ? bw - 1 - col : col;
        const y = this.flipped ? r : bh - 1 - r;
        if ((x + y) % 2 === 1) ctx.fillRect(x0 + col, y0 + r, 1, 1);
      }
    }
    // Playable boards: mandatory ones get an accent outline.
    if (tl && !phantom && tl.end[0] === b.t && tl.end[1] === b.c && b.c === this.view.player) {
      ctx.strokeStyle = tl.status === 'mandatory' ? C.accent : C.presentEdge;
      ctx.lineWidth = Math.max(0.1, (tl.status === 'mandatory' ? 2.5 : 1.5) / squarePx);
      if (tl.status !== 'mandatory') ctx.setLineDash([0.4, 0.3]);
      roundRect(ctx, x0 - fr - 0.18, y0 - fr - 0.18, bw + 2 * fr + 0.36, bh + 2 * fr + 0.36, fr + 0.1);
      ctx.stroke();
      ctx.setLineDash([]);
    }
    // Pieces.
    if (squarePx >= 5) {
      for (let y = 0; y < bh; y++) {
        for (let x = 0; x < bw; x++) {
          const pc = b.squares[y * bw + x];
          if (!pc) continue;
          const col = this.flipped ? bw - 1 - x : x;
          const r = this.flipped ? y : bh - 1 - y;
          const img = pieces.get(pc.p, squarePx * this.dpr);
          if (img) ctx.drawImage(img, x0 + col + 0.04, y0 + r + 0.04, 0.92, 0.92);
          else {
            ctx.fillStyle = pc.p === pc.p.toLowerCase() ? '#222' : '#fff';
            ctx.beginPath();
            ctx.arc(x0 + col + 0.5, y0 + r + 0.5, 0.3, 0, 7);
            ctx.fill();
          }
        }
      }
    } else if (squarePx >= 1.5) {
      for (let y = 0; y < bh; y++) {
        for (let x = 0; x < bw; x++) {
          const pc = b.squares[y * bw + x];
          if (!pc) continue;
          const col = this.flipped ? bw - 1 - x : x;
          const r = this.flipped ? y : bh - 1 - y;
          ctx.fillStyle = pc.p === pc.p.toLowerCase() ? '#1d1d24' : '#ffffff';
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

  drawHighlights(squarePx) {
    const { ctx, colors: C, view: v, overlay: o } = this;
    const fill = (l, vv, x, y, color) => {
      if (!this.boards.has(`${l},${vv}`)) return;
      const [x0, y0] = this.squareRect(l, vv, x, y);
      ctx.fillStyle = color;
      ctx.fillRect(x0, y0, 1, 1);
    };
    // The last action: origin and destination, before and after.
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
      const [fl, ft, fx, fy] = m.from, [tl, tt, tx, ty] = m.to;
      fill(fl, 2 * ft + v.player, fx, fy, C.draft);
      fill(tl, 2 * tt + v.player, tx, ty, C.draft);
    }
    // Royal pieces in check.
    for (const ch of v.checks || []) {
      const [tl, tt, tx, ty] = ch.to;
      const vv = 2 * tt + ch.c;
      if (!this.boards.has(`${tl},${vv}`) && !this.phantom.some((b) => b.l === tl && b.v === vv)) continue;
      const [cx, cy] = this.squareCenter(tl, vv, tx, ty);
      const g = ctx.createRadialGradient(cx, cy, 0.05, cx, cy, 0.72);
      g.addColorStop(0, C.check);
      g.addColorStop(1, 'transparent');
      ctx.fillStyle = g;
      ctx.fillRect(cx - 0.5, cy - 0.5, 1, 1);
    }
    if (o.hover && !o.readonly) fill(o.hover.l, o.hover.v, o.hover.x, o.hover.y, 'rgba(255,255,255,0.18)');
    if (o.selected) {
      const s = o.selected;
      fill(s.l, 2 * s.t + s.c, s.x, s.y, C.select);
    }
    for (const t of o.targets) {
      const vv = 2 * t[1] + v.player;
      if (!this.boards.has(`${t[0]},${vv}`)) continue;
      const [cx, cy] = this.squareCenter(t[0], vv, t[2], t[3]);
      const b = this.boards.get(`${t[0]},${vv}`);
      const occupied = b && b.squares[t[3] * this.bw + t[2]];
      ctx.fillStyle = C.targetDot;
      ctx.strokeStyle = C.targetDot;
      if (occupied) {
        ctx.lineWidth = 0.09;
        ctx.beginPath();
        ctx.arc(cx, cy, 0.44, 0, 7);
        ctx.stroke();
      } else {
        ctx.beginPath();
        ctx.arc(cx, cy, 0.16, 0, 7);
        ctx.fill();
      }
    }
  }

  drawArrows(squarePx) {
    const { colors: C, view: v, overlay: o } = this;
    const width = Math.max(0.1, Math.min(0.2, 3.2 / squarePx));
    for (const ch of v.checks || []) this.arrow(ch.from, ch.to, ch.c, C.check, width * 0.9, false);
    for (const m of v.last || []) this.arrow(m.from, m.to, m.c, m.c ? C.arrowB : C.arrowW, width, false);
    for (const m of v.draft || []) this.arrow(m.from, m.to, v.player, C.draft, width, false);
    for (const m of o.pv || []) this.arrow(m.from, m.to, m.c, C.pv, width, true);
  }

  arrow(from, to, c, color, width, dashed) {
    const { ctx } = this;
    const [x0, y0] = this.squareCenter(from[0], 2 * from[1] + c, from[2], from[3]);
    const [x1, y1] = this.squareCenter(to[0], 2 * to[1] + c, to[2], to[3]);
    const dx = x1 - x0, dy = y1 - y0;
    const len = Math.hypot(dx, dy);
    if (len < 0.01) return;
    const sameBoard = from[0] === to[0] && from[1] === to[1];
    // Jumps between boards curve away from the boards in between.
    const bend = sameBoard ? 0 : Math.min(2.2, 0.18 * len);
    const nx = -dy / len, ny = dx / len;
    const mx = (x0 + x1) / 2 + nx * bend, my = (y0 + y1) / 2 + ny * bend;
    const head = Math.min(0.62, width * 3.6);
    // End direction (tangent of the quadratic at t = 1).
    let ex = x1 - mx, ey = y1 - my;
    const el = Math.hypot(ex, ey) || 1;
    ex /= el; ey /= el;
    const bx = x1 - ex * head * 0.9, by = y1 - ey * head * 0.9;
    ctx.save();
    ctx.globalAlpha = 0.88;
    ctx.strokeStyle = color;
    ctx.fillStyle = color;
    ctx.lineWidth = width;
    ctx.lineCap = 'round';
    if (dashed) ctx.setLineDash([width * 2.2, width * 1.6]);
    ctx.beginPath();
    ctx.moveTo(x0, y0);
    if (bend) ctx.quadraticCurveTo(mx, my, bx, by);
    else ctx.lineTo(bx, by);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(bx - ey * head * 0.55, by + ex * head * 0.55);
    ctx.lineTo(bx + ey * head * 0.55, by - ex * head * 0.55);
    ctx.closePath();
    ctx.fill();
    ctx.restore();
  }

  drawLabels(vMin, vMax, lMin, lMax) {
    const { ctx, colors: C, view: v } = this;
    const s = this.cam.s;
    ctx.font = `600 ${Math.round(Math.max(10, Math.min(14, s * 0.45)))}px ${C.font}`;
    ctx.textBaseline = 'middle';
    // Timeline labels at the left edge.
    ctx.textAlign = 'left';
    for (let l = lMin; l <= lMax; l++) {
      const [, sy] = this.toScreen(0, this.boardY(l) + this.bh / 2);
      if (sy < -10 || sy > this.h + 10) continue;
      const tl = this.timelines.get(l);
      const text = 'L' + (tl ? tl.label : l);
      const tw = ctx.measureText(text).width;
      ctx.fillStyle = C.bg;
      ctx.globalAlpha = 0.85;
      roundRect(ctx, 4, sy - 10, tw + 12, 20, 6);
      ctx.fill();
      ctx.globalAlpha = 1;
      ctx.fillStyle = tl && tl.status === 'mandatory' ? C.accent : C.text;
      ctx.fillText(text, 10, sy);
    }
    // Turn labels at the top edge, one per turn over its white and black boards.
    ctx.textAlign = "center";
    const presentT = v.present;
    const step = Math.max(1, Math.ceil(46 / (s * this.px * 2)));
    for (let t = Math.max(0, (vMin >> 1) - 1); t <= (vMax >> 1) + 1; t++) {
      if (t % step && t !== presentT) continue;
      const [sx] = this.toScreen(this.boardX(2 * t) + this.bw / 2, 0);
      if (sx < 30 || sx > this.w - 20) continue;
      ctx.fillStyle = t === presentT ? C.accent : C.text;
      ctx.fillText("T" + t, sx, 14);
    }
  }
}

function roundRect(ctx, x, y, w, h, r) {
  ctx.beginPath();
  if (ctx.roundRect) ctx.roundRect(x, y, w, h, r);
  else ctx.rect(x, y, w, h);
}
