#!/usr/bin/env python3
"""Write tuner output (name value lines) into hce::tuned_weights(), optionally rescaled.
   scripts/hce_weights.py weights.txt [--queen 1250] [--scale x]"""
import re, sys
args = sys.argv[1:]
src = args[0]
w = [(n, float(v)) for n, v in (l.split() for l in open(src) if l.strip())]
scale = 1.0
if '--scale' in args:
    scale = float(args[args.index('--scale') + 1])
elif '--queen' in args:
    q = dict(w)['mat_a_queen']
    scale = float(args[args.index('--queen') + 1]) / q
body = '\n'.join(f'        {v * scale:.1f}, // {n}' for n, v in w)
code = f"""pub fn tuned_weights() -> Weights {{
    [
{body}
    ]
}}"""
p = 'src/engine/hce.rs'
s = open(p).read()
s2 = re.sub(r'pub fn tuned_weights\(\) -> Weights \{.*?\n\}', code, s, count=1, flags=re.S)
assert s2 != s or code in s
open(p, 'w').write(s2)
print(f'scale {scale:.3f}')
