// Adapter for 5d-chess-js (https://gitlab.com/5d-chess/5d-chess-js), loaded straight from a
// source checkout without npm install: the '@local/...' alias and the 'present' timer package
// are resolved here. Only board.js, piece.js, mate.js and their local requires are loaded.
//
// Set JS5D_SRC to the checkout's src directory (default: ext/5d-chess-js/src).

const path = require('path')
const Module = require('module')

const SRC = process.env.JS5D_SRC || path.resolve(__dirname, '../../ext/5d-chess-js/src')
const STUB_PRESENT = path.join(__dirname, 'stub_present.cjs')
const origResolve = Module._resolveFilename
Module._resolveFilename = function (request, ...rest) {
  if (request.startsWith('@local/')) return origResolve.call(this, path.join(SRC, request.slice(7)), ...rest)
  if (request === 'present') return STUB_PRESENT
  return origResolve.call(this, request, ...rest)
}

const boardFuncs = require(path.join(SRC, 'board.js'))
const mateFuncs = require(path.join(SRC, 'mate.js'))

// 5d-chess-js piece numbers: white even, black odd; negative = unmoved.
const CODES = { P: 2, B: 4, N: 6, R: 8, Q: 10, K: 12, S: 14, W: 16, C: 18, Y: 20, U: 22, D: 24 }

// Timeline index layout used by 5d-chess-js (see piece.timelineMove).
function lToIndex(l, even) {
  if (even) return l >= 0 ? 2 * l + 2 : -2 * l - 1
  return l >= 0 ? 2 * l : -2 * l - 1
}
function indexToL(i, even) {
  if (even) return i % 2 === 0 ? i / 2 - 1 : -(i + 1) / 2
  return i % 2 === 0 ? i / 2 : -(i + 1) / 2
}

function toFullBoard(pos) {
  const fb = []
  const maxIdx = Math.max(...[...pos.lines.keys()].map((l) => lToIndex(l, pos.even)))
  for (let i = 0; i <= maxIdx; i++) fb.push(null)
  for (const [l, tl] of pos.lines) {
    const arr = []
    for (let v = 0; v < tl.start; v++) arr.push(null)
    for (const b of tl.boards) {
      const rows = []
      for (let y = 0; y < pos.H; y++) {
        const row = []
        for (let x = 0; x < pos.W; x++) {
          const p = b.get(x, y)
          if (p === '.') { row.push(0); continue }
          const white = p >= 'A' && p <= 'Z'
          let code = CODES[p.toUpperCase()] - (white ? 0 : 1)
          if (b.unmoved(x, y)) code = -code
          row.push(code)
        }
        rows.push(row)
      }
      arr.push(rows)
    }
    fb[lToIndex(l, pos.even)] = arr
  }
  return fb
}

const fromCoord = (c, even) => [indexToL(c[0], even), c[1] >> 1, c[3], c[2]]
const key = (m, even) => `${fromCoord(m[0], even).join(',')}>${fromCoord(m[1], even).join(',')}`

// Pseudo-legal moves from every board where the side to move may move.
function moves(pos) {
  const fb = toFullBoard(pos)
  const promo = pos.player === 0 ? [10] : [9]
  const out = boardFuncs.moves(fb, pos.player, false, false, false, promo)
  return [...new Set(out.map((m) => key(m, pos.even)))].sort()
}

// Royal captures available to the opponent after the side to move passes its present boards.
function checks(pos) {
  const fb = toFullBoard(pos)
  return [...new Set(mateFuncs.checks(fb, pos.player).map((m) => key(m, pos.even)))].sort()
}

// 5d-chess-js mate detection (time-limited heuristics in the library): [checkmate, stalemate].
function mate(pos, ms = 20000) {
  const fb = toFullBoard(pos)
  const cm = mateFuncs.checkmate(fb, pos.player, ms)
  const sm = mateFuncs.stalemate(fb, pos.player, ms)
  return { checkmate: cm[0], checkmateTimeout: cm[1], stalemate: sm[0], stalemateTimeout: sm[1] }
}

module.exports = { toFullBoard, moves, checks, mate, lToIndex, indexToL }
