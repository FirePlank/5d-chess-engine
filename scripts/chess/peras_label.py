"""Peras labels for the single-timeline positions in netdata FEN sidecars.

python -I scripts/chess/peras_label.py <peras.exe> <depth> <workers> <out.tsv> <sidecar.fen>...

Each sidecar line is "<5D board fen>|<w|b>" or "-". Unique chess positions (standard pieces,
8x8, legal by python-chess) are scored by Peras at a fixed depth with UCI_ShowWDL; the label
is Peras' own expected score for the side to move, (W + D/2) / 1000. Writes
"<chess fen>\t<expected>\t<cp or M<n>>" lines (resumable: positions already in out.tsv are
skipped)."""
import multiprocessing as mp
import os
import subprocess
import sys
import time

import chess


def to_fen(board5d, side):
    rows = board5d.strip('/').split('/')
    if len(rows) != 8:
        return None
    unmoved = set()
    out_rows = []
    for r, row in enumerate(rows):
        out, file, i = '', 0, 0
        while i < len(row):
            ch = row[i]
            if ch.isdigit():
                out += ch
                file += int(ch)
            else:
                if ch not in 'pnbrqkPNBRQK':
                    return None
                if i + 1 < len(row) and row[i + 1] == '*':
                    unmoved.add((r, file, ch))
                    i += 1
                out += ch
                file += 1
            i += 1
        if file != 8:
            return None
        out_rows.append(out)
    cast = ''
    if (7, 4, 'K') in unmoved:
        if (7, 7, 'R') in unmoved:
            cast += 'K'
        if (7, 0, 'R') in unmoved:
            cast += 'Q'
    if (0, 4, 'k') in unmoved:
        if (0, 7, 'r') in unmoved:
            cast += 'k'
        if (0, 0, 'r') in unmoved:
            cast += 'q'
    fen = f"{'/'.join(out_rows)} {side} {cast or '-'} - 0 1"
    try:
        b = chess.Board(fen)
    except ValueError:
        return None
    return fen if b.is_valid() else None


def worker(args):
    exe, depth, fens, q = args
    p = subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)

    def send(x):
        p.stdin.write(x + '\n')
        p.stdin.flush()

    send('uci')
    send('setoption name UCI_ShowWDL value true')
    send('setoption name Hash value 16')
    send('isready')
    while not p.stdout.readline().startswith('readyok'):
        pass
    out = []
    for fen in fens:
        send('position fen ' + fen)
        send(f'go depth {depth}')
        last = None
        while True:
            l = p.stdout.readline()
            if not l:
                break
            if l.startswith('info') and ' wdl ' in l:
                last = l
            if l.startswith('bestmove'):
                break
        if last is None:
            continue
        t = last.split()
        w, d, _ = (int(x) for x in t[t.index('wdl') + 1:t.index('wdl') + 4])
        k = t.index('score')
        sc = t[k + 2] if t[k + 1] == 'cp' else 'M' + t[k + 2]
        out.append(f'{fen}\t{(w + d / 2) / 1000:.4f}\t{sc}')
        if len(out) >= 200:
            q.put(out)
            out = []
    q.put(out)
    send('quit')
    q.put(None)


def main():
    exe, depth, workers, out_path = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
    done = set()
    if os.path.exists(out_path):
        done = {l.split('\t', 1)[0] for l in open(out_path)}
    raw = set()
    for path in sys.argv[5:]:
        with open(path) as fh:
            raw.update(fh.read().split('\n'))
    raw.discard('-')
    raw.discard('')
    print(f'{len(raw)} distinct single-timeline boards', flush=True)
    todo = set()
    for line in raw:
        b, side = line.rsplit('|', 1)
        fen = to_fen(b, side)
        if fen and fen not in done:
            todo.add(fen)
    todo = sorted(todo)
    print(f'{len(done)} already labelled, {len(todo)} to label at depth {depth} with {workers} workers', flush=True)
    m = mp.Manager()
    q = m.Queue()
    chunks = [todo[i::workers] for i in range(workers)]
    pool = mp.Pool(workers)
    pool.map_async(worker, [(exe, depth, c, q) for c in chunks])
    t0, n, alive = time.time(), 0, workers
    with open(out_path, 'a') as f:
        while alive:
            item = q.get()
            if item is None:
                alive -= 1
                continue
            for l in item:
                f.write(l + '\n')
            n += len(item)
            if n % 50000 < len(item):
                el = time.time() - t0
                print(f'{n} labelled, {n / el:.0f}/s, eta {(len(todo) - n) / max(n / el, 1) / 60:.0f} min', flush=True)
    pool.close()
    pool.join()
    print(f'done: {n} in {time.time() - t0:.0f}s', flush=True)


if __name__ == '__main__':
    main()
