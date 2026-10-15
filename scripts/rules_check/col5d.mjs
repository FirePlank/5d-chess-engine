// Adapter for the 5DC OL core (https://github.com/ForkKILLET/5dcol, packages/core/src/index.ts).
// The TypeScript source is copied next to a stand-in for its only dependency (mutative's
// `create`, replaced by structuredClone + mutate) and run with node --experimental-transform-types.
//
// Set COL5D_SRC to packages/core/src/index.ts (default: ext/5dcol/packages/core/src/index.ts).

import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const SRC = process.env.COL5D_SRC || path.resolve(here, '../../ext/5dcol/packages/core/src/index.ts')
const RUN = path.resolve(here, '../../ext/5dcol_run')
fs.mkdirSync(RUN, { recursive: true })
const text = fs.readFileSync(SRC, 'utf8')
if (!text.includes("import { create } from 'mutative'")) throw new Error('unexpected 5dcol import header')
fs.writeFileSync(path.join(RUN, 'index.ts'), text.replace("import { create } from 'mutative'", "import { create } from './mutative_stub.mjs'"))
fs.writeFileSync(
  path.join(RUN, 'mutative_stub.mjs'),
  'export const create = (base, recipe) => { const draft = structuredClone(base); const r = recipe(draft); return r === undefined ? draft : r }\n',
)
const core = await import(pathToFileURL(path.join(RUN, 'index.ts')).href)
const { Multiverse, GameState, Piece } = core

const CODES = {
  P: Piece.PW, R: Piece.RW, N: Piece.NW, B: Piece.BW, Q: Piece.QW, K: Piece.KW,
  U: Piece.UW, D: Piece.DW, S: Piece.SW, W: Piece.WW, C: Piece.CW, Y: Piece.YW,
  p: Piece.PB, r: Piece.RB, n: Piece.NB, b: Piece.BB, q: Piece.QB, k: Piece.KB,
  u: Piece.UB, d: Piece.DB, s: Piece.SB, w: Piece.WB, c: Piece.CB, y: Piece.YB,
}

// 5dcol stores black's timelines one lower in the odd layout (-1 is reserved for "-0").
const toL5 = (l, even) => (!even && l < 0 ? l - 1 : l)
const fromL5 = (l, even) => (!even && l < 0 ? l + 1 : l)

export function toMultiverse(pos) {
  const lOffset = 128
  const lines = []
  let lo = Infinity, hi = -Infinity
  for (const [l, tl] of pos.lines) {
    const l5 = toL5(l, pos.even)
    lo = Math.min(lo, l5)
    hi = Math.max(hi, l5)
    const boards = []
    tl.boards.forEach((b, k) => {
      const pieces = [], unmoved = []
      for (let x = 0; x < pos.W; x++) {
        const col = [], ucol = []
        for (let y5 = 0; y5 < pos.H; y5++) {
          const y = pos.H - 1 - y5
          const p = b.get(x, y)
          col.push(p === '.' ? Piece.E : CODES[p])
          ucol.push(b.unmoved(x, y))
        }
        pieces.push(col)
        unmoved.push(ucol)
      }
      const right = (kx, ky, king, rx, rook) =>
        b.get(kx, ky) === king && b.unmoved(kx, ky) && b.get(rx, ky) === rook && b.unmoved(rx, ky)
      boards[tl.start + k] = {
        width: pos.W, height: pos.H, pieces, unmoved,
        canCastleKW: right(4, 0, 'K', pos.W - 1, 'R'),
        canCastleQW: right(4, 0, 'K', 0, 'R'),
        canCastleKB: right(4, pos.H - 1, 'k', pos.W - 1, 'r'),
        canCastleQB: right(4, pos.H - 1, 'k', 0, 'r'),
        createdBy: null, createdByPlayer: null, createdByRole: null, createdByOrder: null,
      }
    })
    lines[lOffset + l5] = { boards, mStart: tl.start }
  }
  return { lines, lOffset, lFurthestB: lOffset + lo, lFurthestW: lOffset + hi, lastMove: null }
}

const key = (m, pos) => {
  const c = (q) => `${fromL5(q.l, pos.even)},${q.t},${q.x},${pos.H - 1 - q.y}`
  return `${c(m.from)}>${c(m.to)}`
}

export function moves(pos) {
  const mv = toMultiverse(pos)
  const { mandatory, optional } = Multiverse.getTimelineStatus(mv, pos.player)
  const out = new Set()
  for (const l of [...mandatory, ...optional]) {
    const m = mv.lines[mv.lOffset + l].boards.length - 1
    const t = (m - pos.player) / 2
    const board = mv.lines[mv.lOffset + l].boards[m]
    for (let x = 0; x < pos.W; x++)
      for (let y = 0; y < pos.H; y++) {
        const p = board.pieces[x][y]
        if (p === Piece.E || (p < 0x10 ? 0 : 1) !== pos.player) continue
        const from = { x, y, l, t }
        for (const to of Multiverse.getMoveTargets(mv, from, pos.player)) out.add(key({ from, to }, pos))
      }
  }
  return [...out].sort()
}

// Checks as 5dcol defines them for mate detection (phantom = pass on every playable board).
export function checks(pos) {
  const mv = toMultiverse(pos)
  return [...new Set(Multiverse.findChecks(Multiverse.createPhantom(mv, pos.player), 1 - pos.player).map((m) => key(m, pos)))].sort()
}

// Checks after passing on the mandatory boards only (Kairos's definition), using 5dcol's findChecks.
export function checksMandatory(pos) {
  const mv = toMultiverse(pos)
  const { mandatory } = Multiverse.getTimelineStatus(mv, pos.player)
  return [...new Set(Multiverse.findChecks(Multiverse.createPass(mv, pos.player, mandatory), 1 - pos.player).map((m) => key(m, pos)))].sort()
}

export function mate(pos) {
  const mv = toMultiverse(pos)
  const r = GameState.getCheckmateResult({ multiverse: mv, player: pos.player })
  return { status: r.status, action: r.legalAction ? r.legalAction.moves.map((m) => key(m, pos)) : null }
}
