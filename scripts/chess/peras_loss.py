"""Plain-chess loss of Kairos's moves in single-timeline positions, judged by Peras.

Input: `ktools chesspos` lines ("<5D board fen>|<side>|<move>|<where>"). For each position,
Peras scores the position before the move and after it at a fixed depth; the loss is the
drop for the side that moved (centipawns, Peras' scale). Prints the loss distribution and
the worst moves.
  python scripts/chess/peras_loss.py <peras.exe> <chesspos.txt> [depth=10] [workers=6]
"""
import sys, re, concurrent.futures as cf
import chess, chess.engine

def to_fen(board5d, side):
    rows = board5d.split('/')
    unmoved = set()
    out_rows = []
    for r, row in enumerate(rows):
        out, file = '', 0
        i = 0
        while i < len(row):
            ch = row[i]
            if ch.isdigit():
                out += ch; file += int(ch)
            else:
                if ch not in 'pnbrqkPNBRQK':
                    return None
                if i + 1 < len(row) and row[i + 1] == '*':
                    unmoved.add((r, file, ch)); i += 1
                out += ch; file += 1
            i += 1
        out_rows.append(out)
    cast = ''
    if (7, 4, 'K') in unmoved:
        if (7, 7, 'R') in unmoved: cast += 'K'
        if (7, 0, 'R') in unmoved: cast += 'Q'
    if (0, 4, 'k') in unmoved:
        if (0, 7, 'r') in unmoved: cast += 'k'
        if (0, 0, 'r') in unmoved: cast += 'q'
    return f"{'/'.join(out_rows)} {side} {cast or '-'} - 0 1"

def score(info, pov):
    return info["score"].pov(pov).score(mate_score=3000)

def work(args):
    exe, depth, items = args
    eng = chess.engine.SimpleEngine.popen_uci(exe)
    res = []
    for fen, mv, where in items:
        try:
            b = chess.Board(fen)
            if not b.is_valid():
                continue
            m = chess.Move.from_uci(mv)
            if m not in b.legal_moves:
                continue
            pov = b.turn
            before = eng.analyse(b, chess.engine.Limit(depth=depth))
            san = b.san(m)
            best = before.get("pv", [None])[0]
            best_san = b.san(best) if best else '-'
            b.push(m)
            after = eng.analyse(b, chess.engine.Limit(depth=depth))
            res.append((score(before, pov) - score(after, pov), score(before, pov), fen, san, best_san, where))
        except Exception as e:
            print("skip", where, e, file=sys.stderr)
    eng.quit()
    return res

def main():
    exe, path = sys.argv[1], sys.argv[2]
    depth = int(sys.argv[3]) if len(sys.argv) > 3 else 10
    workers = int(sys.argv[4]) if len(sys.argv) > 4 else 6
    items = []
    for line in open(path, encoding='utf-8'):
        parts = line.rstrip('\n').split('|')
        if len(parts) != 4:
            continue
        fen = to_fen(parts[0], parts[1])
        if fen and re.fullmatch(r"[a-h][1-8][a-h][1-8][qrbn]?", parts[2]):
            items.append((fen, parts[2], parts[3]))
    chunks = [items[i::workers] for i in range(workers)]
    res = []
    with cf.ThreadPoolExecutor(workers) as ex:
        for r in ex.map(work, [(exe, depth, c) for c in chunks]):
            res.extend(r)
    n = len(res)
    print(f"positions {n} (Peras depth {depth})")
    for t in (50, 100, 200, 300, 500):
        k = sum(1 for r in res if r[0] >= t)
        print(f"  loss >= {t:>3}: {k:>5} ({100.0 * k / max(n, 1):.1f}%)")
    print(f"  mean loss {sum(max(r[0], 0) for r in res) / max(n, 1):.1f}")
    print("worst:")
    for r in sorted(res, reverse=True)[:int(__import__("os").environ.get("WORST", "25"))]:
        print(f"  {r[0]:>5} (eval {r[1]:>5})  played {r[3]:<7} Peras {r[4]:<7} {r[2]}  {r[5]}")

if __name__ == '__main__':
    main()
