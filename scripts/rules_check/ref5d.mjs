// Independent reference implementation of the 5D Chess With Multiverse Time Travel rules,
// written from the rule definitions (not from Kairos's code) to cross-check Kairos.
//
// Coordinates follow `ktools rules`: timeline l in the internal numbering (consecutive
// integers; in the even layout -1 is "-0" and 0 is "+0"), turn t, file x, rank y (y = 0 is
// rank 1). A board is identified by (l, v) with v = 2t + c, c = 0 white / 1 black.
// Pieces are 5DFEN letters (uppercase white): K Q R B N P, U unicorn, D dragon, S princess,
// W brawn, Y royal queen, C common king.

export const OPTIONS = {
  // Extra brawn captures: '5dchessjs' = forward-L + (x±1 | forward-y), back-T + forward-y.
  brawn: '5dchessjs',
  // Attacks that forbid castling: 'physical' = only pieces on the same board.
  castleAttacks: 'physical',
}

const ROYAL = new Set(['K', 'Y'])
const isWhite = (p) => p >= 'A' && p <= 'Z'
const colorOf = (p) => (p === '.' ? -1 : isWhite(p) ? 0 : 1)
const upper = (p) => p.toUpperCase()

export class Board {
  constructor(W, H, p, u) {
    this.W = W
    this.H = H
    this.p = p // array of W*H piece letters or '.'
    this.u = u // array of W*H 0/1 unmoved flags
  }
  static fromFen(fen, W, H) {
    const p = new Array(W * H).fill('.')
    const u = new Array(W * H).fill(0)
    let y = H - 1
    let x = 0
    for (let i = 0; i < fen.length; i++) {
      const ch = fen[i]
      if (ch === '/') { y--; x = 0; continue }
      if (ch >= '0' && ch <= '9') {
        let n = ch.charCodeAt(0) - 48
        while (i + 1 < fen.length && fen[i + 1] >= '0' && fen[i + 1] <= '9') n = n * 10 + fen.charCodeAt(++i) - 48
        x += n
        continue
      }
      p[y * W + x] = ch
      if (fen[i + 1] === '*') { u[y * W + x] = 1; i++ }
      x++
    }
    return new Board(W, H, p, u)
  }
  get(x, y) { return this.p[y * this.W + x] }
  unmoved(x, y) { return this.u[y * this.W + x] === 1 }
  inside(x, y) { return x >= 0 && y >= 0 && x < this.W && y < this.H }
  copy() { return new Board(this.W, this.H, this.p.slice(), this.u.slice()) }
  set(x, y, piece) { this.p[y * this.W + x] = piece; this.u[y * this.W + x] = 0 }
  key() { return this.p.join('') + this.u.join('') }
}

export class Position {
  // lines: Map l -> { start: v, boards: Board[] }
  constructor({ even, W, H, player, lines, promo = 'Q' }) {
    Object.assign(this, { even, W, H, player, lines, promo })
  }
  clone() {
    const lines = new Map()
    for (const [l, tl] of this.lines) lines.set(l, { start: tl.start, boards: tl.boards.slice() })
    return new Position({ even: this.even, W: this.W, H: this.H, player: this.player, lines, promo: this.promo })
  }
  lRange() {
    let lo = Infinity, hi = -Infinity
    for (const l of this.lines.keys()) { lo = Math.min(lo, l); hi = Math.max(hi, l) }
    return [lo, hi]
  }
  end(l) { const tl = this.lines.get(l); return tl.start + tl.boards.length - 1 }
  board(l, v) {
    const tl = this.lines.get(l)
    if (!tl || v < tl.start || v >= tl.start + tl.boards.length) return null
    return tl.boards[v - tl.start]
  }
  // Timelines created by each side and the rule that a side may only have one more active
  // timeline than its opponent has created.
  created() {
    const [lo, hi] = this.lRange()
    return { white: hi, black: this.even ? -1 - lo : -lo }
  }
  isActive(l) {
    const { white, black } = this.created()
    if (l >= 1) return l <= black + 1
    const k = this.even ? -1 - l : -l
    if (k >= 1) return k <= white + 1
    return true
  }
  presentV() {
    let v = Infinity
    for (const l of this.lines.keys()) if (this.isActive(l)) v = Math.min(v, this.end(l))
    return v
  }
  canSubmit() { return (this.presentV() & 1) !== this.player }
  playable(c = this.player) {
    const out = []
    for (const l of [...this.lines.keys()].sort((a, b) => a - b)) if ((this.end(l) & 1) === c) out.push(l)
    return out
  }
  mandatory() {
    const pv = this.presentV()
    if ((pv & 1) !== this.player) return []
    return this.playable().filter((l) => this.isActive(l) && this.end(l) === pv)
  }
  newLine(c) { const [lo, hi] = this.lRange(); return c === 0 ? hi + 1 : lo - 1 }
  key() {
    const parts = []
    for (const l of [...this.lines.keys()].sort((a, b) => a - b)) {
      const tl = this.lines.get(l)
      parts.push(`${l}:${tl.start}:${tl.boards.length}:${tl.boards[tl.boards.length - 1].key()}`)
    }
    return parts.join('|')
  }
}

// --------------------------------------------------------------- move generation

const DIRS = []
for (let dx = -1; dx <= 1; dx++)
  for (let dy = -1; dy <= 1; dy++)
    for (let dt = -1; dt <= 1; dt++)
      for (let dl = -1; dl <= 1; dl++) {
        const n = (dx !== 0) + (dy !== 0) + (dt !== 0) + (dl !== 0)
        if (n > 0) DIRS.push({ d: [dx, dy, dt, dl], n })
      }
const KNIGHT = []
for (let a = 0; a < 4; a++)
  for (let b = 0; b < 4; b++) {
    if (a === b) continue
    for (const sa of [-2, 2])
      for (const sb of [-1, 1]) {
        const d = [0, 0, 0, 0]
        d[a] = sa
        d[b] = sb
        KNIGHT.push(d)
      }
  }

const RIDER_AXES = { R: [1], B: [2], U: [3], D: [4], Q: [1, 2, 3, 4], Y: [1, 2, 3, 4], S: [1, 2] }

// A destination square: [l, t, x, y]. Moves are { from: [l,t,x,y], to: [l,t,x,y], kind }.
function sq(pos, c, l, t, x, y) {
  const b = pos.board(l, 2 * t + c)
  if (!b || !b.inside(x, y)) return null
  return b
}

export function pieceMoves(pos, c, l, t, x, y, capturesOnly = false) {
  const b = pos.board(l, 2 * t + c)
  const piece = b.get(x, y)
  const kind = upper(piece)
  const out = []
  const push = (to, extra) => out.push({ from: [l, t, x, y], to, ...extra })
  const target = (d, k = 1) => {
    const [tx, ty, tt, tl] = [x + d[0] * k, y + d[1] * k, t + d[2] * k, l + d[3] * k]
    const tb = sq(pos, c, tl, tt, tx, ty)
    if (!tb) return null
    return { to: [tl, tt, tx, ty], occ: colorOf(tb.get(tx, ty)) }
  }
  if (RIDER_AXES[kind]) {
    const axes = RIDER_AXES[kind]
    for (const { d, n } of DIRS) {
      if (!axes.includes(n)) continue
      for (let k = 1; ; k++) {
        const tg = target(d, k)
        if (!tg || tg.occ === c) break
        if (!capturesOnly || tg.occ !== -1) push(tg.to)
        if (tg.occ !== -1) break
      }
    }
  } else if (kind === 'K' || kind === 'C') {
    for (const { d } of DIRS) {
      const tg = target(d)
      if (tg && tg.occ !== c && (!capturesOnly || tg.occ !== -1)) push(tg.to)
    }
    if (kind === 'K' && b.unmoved(x, y) && !capturesOnly) castling(pos, c, b, l, t, x, y, push)
  } else if (kind === 'N') {
    for (const d of KNIGHT) {
      const tg = target(d)
      if (tg && tg.occ !== c && (!capturesOnly || tg.occ !== -1)) push(tg.to)
    }
  } else if (kind === 'P' || kind === 'W') {
    const fy = c === 0 ? 1 : -1
    const fl = c === 0 ? -1 : 1
    const unmoved = b.unmoved(x, y)
    if (!capturesOnly) {
      // Non-capturing steps forward along y and along L (two steps when unmoved).
      for (const d of [[0, fy, 0, 0], [0, 0, 0, fl]]) {
        const t1 = target(d)
        if (!t1 || t1.occ !== -1) continue
        push(t1.to)
        if (unmoved) {
          const t2 = target(d, 2)
          if (t2 && t2.occ === -1) push(t2.to)
        }
      }
    }
    const caps = [[1, fy, 0, 0], [-1, fy, 0, 0], [0, 0, 1, fl], [0, 0, -1, fl]]
    if (kind === 'W' && OPTIONS.brawn === '5dchessjs') {
      caps.push([1, 0, 0, fl], [-1, 0, 0, fl], [0, fy, 0, fl], [0, fy, -1, 0])
    }
    const seen = new Set()
    for (const d of caps) {
      const tg = target(d)
      if (!tg || tg.occ !== 1 - c) continue
      const k = tg.to.join(',')
      if (seen.has(k)) continue
      seen.add(k)
      push(tg.to)
    }
    if (!capturesOnly) enPassant(pos, c, b, l, t, x, y, fy, push)
  }
  return out
}

function enPassant(pos, c, b, l, t, x, y, fy, push) {
  const v = 2 * t + c
  const prev = pos.board(l, v - 1) // the board before the opponent's last move on this timeline
  if (!prev) return
  for (const dx of [-1, 1]) {
    const ex = x + dx
    if (!b.inside(ex, y + fy) || !b.inside(ex, y - fy)) continue
    const victim = b.get(ex, y)
    if (colorOf(victim) !== 1 - c || !'PW'.includes(upper(victim))) continue
    const origin = y + 2 * fy // where the opponent's pawn started its double step
    if (!b.inside(ex, origin)) continue
    if (b.get(ex, y + fy) !== '.' || b.get(ex, origin) !== '.') continue
    if (prev.get(ex, origin) !== victim || !prev.unmoved(ex, origin)) continue
    if (prev.get(ex, y + fy) !== '.' || prev.get(ex, y) !== '.') continue
    push([l, t, ex, y + fy], { ep: [ex, y] })
  }
}

function attackedOnBoard(b, x, y, c) {
  // Is square (x, y) attacked by the opponent of c using moves within this board only?
  const opp = 1 - c
  for (let yy = 0; yy < b.H; yy++)
    for (let xx = 0; xx < b.W; xx++) {
      const p = b.get(xx, yy)
      if (colorOf(p) !== opp) continue
      const k = upper(p)
      const dx = x - xx, dy = y - yy
      if (k === 'P' || k === 'W') {
        if (dy === (opp === 0 ? 1 : -1) && Math.abs(dx) === 1) return true
      } else if (k === 'N') {
        if ((Math.abs(dx) === 1 && Math.abs(dy) === 2) || (Math.abs(dx) === 2 && Math.abs(dy) === 1)) return true
      } else if (k === 'K' || k === 'C') {
        if (Math.max(Math.abs(dx), Math.abs(dy)) === 1) return true
      } else if (RIDER_AXES[k]) {
        const n = (dx !== 0) + (dy !== 0)
        if (n === 0) continue
        if (n === 2 && Math.abs(dx) !== Math.abs(dy)) continue
        if (!RIDER_AXES[k].includes(n)) continue
        const sx = Math.sign(dx), sy = Math.sign(dy)
        let cx = xx + sx, cy = yy + sy, blocked = false
        while (cx !== x || cy !== y) {
          if (b.get(cx, cy) !== '.') { blocked = true; break }
          cx += sx; cy += sy
        }
        if (!blocked) return true
      }
    }
  return false
}

function castling(pos, c, b, l, t, x, y, push) {
  const rook = c === 0 ? 'R' : 'r'
  if (OPTIONS.castleAttacks === 'physical' && attackedOnBoard(b, x, y, c)) return
  for (const d of [1, -1]) {
    // The first piece along the rank must be an unmoved friendly rook at least two squares away
    // (Wikipedia, note a: in variants the rook may stand anywhere on the rank and the king may
    // land on its square). The king moves two squares; it may not start on, pass through or
    // land on a square attacked on this board.
    let rx = x + d
    while (b.inside(rx, y) && b.get(rx, y) === '.') rx += d
    if (!b.inside(rx, y) || b.get(rx, y) !== rook || !b.unmoved(rx, y)) continue
    if (Math.abs(rx - x) < 2) continue
    if (attackedOnBoard(b, x + d, y, c) || attackedOnBoard(b, x + 2 * d, y, c)) continue
    push([l, t, x + 2 * d, y], { castle: [rx, x + d] })
  }
}

export function allMoves(pos, c = pos.player, lines = pos.playable(c), capturesOnly = false) {
  const out = []
  for (const l of lines) {
    const v = pos.end(l)
    if ((v & 1) !== c) continue
    const t = v >> 1
    const b = pos.board(l, v)
    for (let y = 0; y < b.H; y++)
      for (let x = 0; x < b.W; x++)
        if (colorOf(b.get(x, y)) === c) out.push(...pieceMoves(pos, c, l, t, x, y, capturesOnly))
  }
  return out
}

// ------------------------------------------------------------------ applying moves

function promote(pos, c, piece, y) {
  const k = upper(piece)
  if ((k === 'P' || k === 'W') && y === (c === 0 ? pos.H - 1 : 0)) return c === 0 ? pos.promo : pos.promo.toLowerCase()
  return piece
}

function append(pos, l, board) {
  const tl = pos.lines.get(l)
  tl.boards.push(board)
}

// Returns a new Position with the move applied (does not check legality).
export function applyMove(pos0, m) {
  const pos = pos0.clone()
  const c = pos.player
  const [l1, t1, x1, y1] = m.from
  const [l2, t2, x2, y2] = m.to
  const v1 = 2 * t1 + c
  const v2 = 2 * t2 + c
  const src = pos.board(l1, v1)
  const piece = src.get(x1, y1)
  if (l1 === l2 && t1 === t2) {
    const nb = src.copy()
    nb.set(x1, y1, '.')
    if (m.ep) nb.set(m.ep[0], m.ep[1], '.')
    else if (upper(piece) === 'P' || upper(piece) === 'W') {
      if (x1 !== x2 && src.get(x2, y2) === '.') nb.set(x2, y1, '.') // en passant
    }
    if (upper(piece) === 'K' && Math.abs(x2 - x1) === 2) {
      const d = Math.sign(x2 - x1)
      let rx = x1 + d
      while (src.get(rx, y1) === '.') rx += d
      nb.set(x1 + d, y1, src.get(rx, y1))
      nb.set(rx, y1, '.')
    }
    nb.set(x2, y2, promote(pos, c, piece, y2))
    append(pos, l1, nb)
    return pos
  }
  const left = src.copy()
  left.set(x1, y1, '.')
  const dst = pos.board(l2, v2)
  const arrived = dst.copy()
  arrived.set(x2, y2, promote(pos, c, piece, y2))
  const nonBranching = pos.end(l2) === v2
  append(pos, l1, left)
  if (nonBranching) append(pos, l2, arrived)
  else pos.lines.set(pos.newLine(c), { start: v2 + 1, boards: [arrived] })
  return pos
}

export function submit(pos0) {
  const pos = pos0.clone()
  pos.player = 1 - pos.player
  return pos
}

// Captures of a royal piece of `defender` by the side to move after it, from every board
// where it is the attacker's turn.
export function royalCaptures(pos, attacker) {
  const out = []
  for (const m of allMoves(pos, attacker, pos.playable(attacker), true)) {
    const [l, t, x, y] = m.to
    const p = pos.board(l, 2 * t + attacker).get(x, y)
    if (colorOf(p) === 1 - attacker && ROYAL.has(upper(p))) out.push(m)
  }
  return out
}

// Checks against the side to move if it passed on every mandatory board.
export function phantomChecks(pos) {
  const ph = pos.clone()
  for (const l of pos.mandatory()) append(ph, l, pos.board(l, pos.end(l)))
  return royalCaptures(ph, 1 - pos.player)
}

export const moveKey = (m) => `${m.from.join(',')}>${m.to.join(',')}`

// Depth-first enumeration of every legal action (deduplicated by resulting position).
// Returns { results: Map(key -> moves), complete }.
export function legalActions(pos, { budget = 200000, limit = Infinity, stopAtFirst = false } = {}) {
  const results = new Map()
  const seen = new Set()
  let nodes = 0
  let complete = true
  const rec = (p, moves) => {
    if (results.size >= limit) return true
    if (++nodes > budget) { complete = false; return true }
    const k = p.key()
    if (seen.has(k)) return false
    seen.add(k)
    if (p.canSubmit()) {
      const after = submit(p)
      if (royalCaptures(after, after.player).length === 0) {
        const rk = after.key()
        if (!results.has(rk)) results.set(rk, moves.slice())
        if (stopAtFirst) return true
      }
    }
    for (const m of allMoves(p)) {
      moves.push(m)
      const stop = rec(applyMove(p, m), moves)
      moves.pop()
      if (stop) return true
    }
    return false
  }
  rec(pos, [])
  return { results, complete, nodes }
}

// Parse the record of one position from `ktools rules` output (I and B lines).
export function positionFromRecord(rec) {
  const info = rec.I
  const lines = new Map()
  const boards = rec.B.map(([l, t, c, fen]) => ({ l, v: 2 * t + c, b: Board.fromFen(fen, info.W, info.H) }))
  boards.sort((a, b) => a.l - b.l || a.v - b.v)
  for (const { l, v, b } of boards) {
    if (!lines.has(l)) lines.set(l, { start: v, boards: [] })
    const tl = lines.get(l)
    if (tl.start + tl.boards.length !== v) throw new Error(`gap on timeline ${l}`)
    tl.boards.push(b)
  }
  return new Position({ even: info.even, W: info.W, H: info.H, player: info.player, lines })
}
