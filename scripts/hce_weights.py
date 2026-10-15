#!/usr/bin/env python3
"""Write tuner output (name value lines) into hce::tuned_weights(), optionally rescaled.
   scripts/hce_weights.py weights.txt [--queen 1250 | --scale x] [--frozen-log tune.log]
Features the tuner froze (listed as 'frozen <name>' in its log) keep their value unscaled."""
import re
import sys

args = sys.argv[1:]
w = [(n, float(v)) for n, v in (line.split() for line in open(args[0]) if line.strip())]
scale = 1.0
if '--scale' in args:
    scale = float(args[args.index('--scale') + 1])
elif '--queen' in args:
    scale = float(args[args.index('--queen') + 1]) / dict(w)['mat_a_queen']
frozen = set()
if '--frozen-log' in args:
    log = args[args.index('--frozen-log') + 1]
    frozen = {line.split()[1] for line in open(log) if line.startswith('frozen ')}
rows = [f'        ("{n}", {(v if n in frozen else v * scale):.1f}),' for n, v in w]
code = 'pub fn tuned_weights() -> Weights {\n    weights_by_name(&[\n' + '\n'.join(rows) + '\n    ])\n}'
p = 'src/engine/hce.rs'
s = open(p).read()
s = re.sub(r'pub fn tuned_weights\(\) -> Weights \{.*?\n\}', lambda _: code, s, count=1, flags=re.S)
open(p, 'w').write(s)
print(f'scale {scale:.3f}, frozen {len(frozen)}')
