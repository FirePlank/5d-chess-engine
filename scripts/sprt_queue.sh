#!/usr/bin/env bash
# Run queued remote SPRTs one after another: each line of the queue file is
#   <name> <patch-file> <games> <elo0> <elo1> [tc...]
# tested against the current main HEAD; every test is recorded (and reverted on sprt) when
# done. Lines starting with # are skipped. A test whose run failed (setup error, null test,
# no results) is recorded as "error", never as a verdict, and the queue moves on.
set -uo pipefail
R=$(git rev-parse --show-toplevel)
while IFS= read -r LINE; do
  LINE=${LINE%$'\r'}
  read -r NAME PATCH GAMES E0 E1 TC <<< "$LINE"
  [ -z "${NAME:-}" ] || [ "${NAME:0:1}" = "#" ] && continue
  BASE=$(git -C "$R" rev-parse HEAD)
  S="$R/games/sprt/summary_${NAME}.txt"
  rm -f "$S"
  if bash "$R/scripts/sprt_run.sh" "$NAME" "$BASE" "$PATCH" "$GAMES" "${TC:-tc 10+0.1}" "$E0" "$E1" \
       > "$R/games/sprt/run_${NAME}.log" 2>&1 && [ -s "$S" ]; then
    V=$(grep -q "H1 ACCEPTED" "$S" && echo accept || echo reject)
  else
    V=error
  fi
  bash "$R/scripts/sprt_done.sh" "$NAME" "$V" >> "$R/games/sprt/run_${NAME}.log" 2>&1
  if [ "$V" = error ]; then
    echo "$NAME: ERROR | $(grep -E '^ERROR|##\[error\]' "$R/games/sprt/run_${NAME}.log" | head -2 | tr '\n' ' ')"
  else
    echo "$NAME: $V | $(grep -E '^Elo|^LLR' "$S" | tr '\n' ' ')"
  fi
done < "$1"
