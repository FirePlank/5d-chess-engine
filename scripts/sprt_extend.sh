#!/usr/bin/env bash
# Extend an inconclusive SPRT: play GAMES more games of <name>'s patch against <base> (no early
# stop), saved as <name>_ext, then add the two runs' pentanomial counts and print the combined
# result. The original run's counts come from games/sprt/summary_<name>.txt.
#   scripts/sprt_extend.sh <name> <base-sha> <elo0> <elo1> [games=3000] [tc="tc 10+0.1"]
set -uo pipefail
R=$(git rev-parse --show-toplevel)
NAME=$1 BASE=$2 E0=$3 E1=$4 GAMES=${5:-3000} TC=${6:-"tc 10+0.1"}
cp "$R/games/sprt/patches/$NAME.patch" "$R/games/sprt/patches/${NAME}_ext.patch"
[ -f "$R/games/sprt/patches/$NAME.msg" ] && cp "$R/games/sprt/patches/$NAME.msg" "$R/games/sprt/patches/${NAME}_ext.msg"
rm -f "$R/games/sprt/summary_${NAME}_ext.txt"
MIN_GAMES=$GAMES bash "$R/scripts/sprt_run.sh" "${NAME}_ext" "$BASE" "$R/games/sprt/patches/${NAME}_ext.patch" "$GAMES" "$TC" "$E0" "$E1" \
  > "$R/games/sprt/run_${NAME}_ext.log" 2>&1
penta() { sed -n 's/^Pentanomial \[LL LD WL+DD WD WW\]: \[\(.*\)\]/\1/p' "$1" | tr -d ' '; }
A=$(penta "$R/games/sprt/summary_${NAME}.txt"); B=$(penta "$R/games/sprt/summary_${NAME}_ext.txt")
[ -n "$A" ] && [ -n "$B" ] || { echo "missing counts: '$A' '$B'"; exit 1; }
SUM=$(python3 -c "import sys; a=[int(x) for x in '$A'.split(',')]; b=[int(x) for x in '$B'.split(',')]; print(','.join(str(x+y) for x,y in zip(a,b)))" 2>/dev/null \
  || python -c "a=[int(x) for x in '$A'.split(',')]; b=[int(x) for x in '$B'.split(',')]; print(','.join(str(x+y) for x,y in zip(a,b)))")
echo "original: $A"; echo "extension: $B"; echo "combined: $SUM"
python "$R/scripts/sprt_merge.py" --from-counts "$SUM" --elo0 "$E0" --elo1 "$E1"
