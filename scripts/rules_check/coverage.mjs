// Coverage of a `ktools rules` dump: how often each kind of rule was exercised by the
// generated (and cross-checked) moves and checks.  usage: node coverage.mjs <rules>...

import fs from 'node:fs'
import * as ref from './ref5d.mjs'

const c = {}
const inc = (k, n = 1) => (c[k] = (c[k] || 0) + n)
for (const file of process.argv.slice(2)) {
  let rec = null
  const flush = () => {
    if (!rec || !rec.I) return
    const pos = ref.positionFromRecord(rec)
    inc('positions')
    const [lo, hi] = pos.lRange()
    inc(`timelines:${hi - lo + 1 > 8 ? '9+' : hi - lo + 1}`)
    if (pos.even) inc('even layout positions')
    const inactive = [...pos.lines.keys()].filter((l) => !pos.isActive(l)).length
    if (inactive) inc('positions with inactive timelines')
    if (rec.k.length) inc('positions in check')
    for (const mk of rec.m) {
      const [a, b] = mk.split('>')
      const [l1, t1, x1, y1] = a.split(',').map(Number)
      const [l2, t2, x2, y2] = b.split(',').map(Number)
      const c0 = pos.player
      const piece = pos.board(l1, 2 * t1 + c0).get(x1, y1).toUpperCase()
      const dst = pos.board(l2, 2 * t2 + c0)
      const capture = dst.get(x2, y2) !== '.'
      inc('moves')
      inc(`moves:${piece}`)
      if (l1 === l2 && t1 === t2) {
        if (piece === 'K' && Math.abs(x2 - x1) === 2) inc('castling moves')
        if ('PW'.includes(piece) && x1 !== x2 && !capture) inc('en passant moves')
        if ('PW'.includes(piece) && (y2 === 0 || y2 === pos.H - 1)) inc('promotions (physical)')
        if ('PW'.includes(piece) && Math.abs(y2 - y1) === 2) inc('pawn double steps (y)')
      } else {
        inc('superphysical moves')
        inc(pos.end(l2) === 2 * t2 + c0 ? 'non-branching jumps' : 'branching moves')
        if (t2 > t1) inc('moves to a later turn')
        if (capture) inc('superphysical captures')
        if ('PW'.includes(piece)) {
          inc(`superphysical ${piece === 'W' ? 'brawn' : 'pawn'} moves`)
          if (Math.abs(l2 - l1) === 2) inc('pawn double steps (L)')
          if (y2 !== y1) inc('brawn y+T/L captures')
          if (x2 !== x1) inc('brawn x+L captures')
          if (y2 === 0 || y2 === pos.H - 1) inc('promotions (superphysical)')
        }
      }
    }
    for (const mk of rec.k) {
      const [a, b] = mk.split('>')
      const t1 = Number(a.split(',')[1]), t2 = Number(b.split(',')[1])
      const l1 = Number(a.split(',')[0]), l2 = Number(b.split(',')[0])
      inc('check moves')
      if (t2 < t1 || l1 !== l2) inc('check moves across time/timelines')
      if (pos.end(l2) > 2 * t2 + (1 - pos.player)) inc('checks against royals on past boards')
    }
  }
  for (const line of fs.readFileSync(file, 'utf8').split(/\r?\n/)) {
    if (line.startsWith('G ')) { flush(); rec = { B: [], m: [], k: [] } }
    else if (!rec) continue
    else if (line.startsWith('I ')) {
      const w = line.split(' ')
      rec.I = { player: +w[2], even: w[w.indexOf('even') + 1] === '1', W: +w[w.indexOf('size') + 1], H: +w[w.indexOf('size') + 2] }
    } else if (line.startsWith('B ')) { const [, l, t, cc, fen] = line.split(' '); rec.B.push([+l, +t, +cc, fen]) }
    else if (line.startsWith('m ')) rec.m.push(line.slice(2))
    else if (line.startsWith('k ')) rec.k.push(line.slice(2))
  }
  flush()
}
for (const k of Object.keys(c).sort()) console.log(`${k}\t${c[k]}`)
