#!/usr/bin/env bash
# Run queued remote SPRTs one after another: each line of the queue file is
#   <name> <patch-file|-> <games> <elo0> <elo1> [tc...] [| <new engine args>]
# against the current main HEAD; every test is recorded (and reverted on sprt) when done.
# With a patch of "-" and new engine args such as "ab --opt q_restrict=true", a test needs
# no code change, so main can move on while the queue runs.
set -uo pipefail
R=$(git rev-parse --show-toplevel)
while IFS= read -r LINE; do
  SPEC=${LINE%%|*}
  ARGS=""
  [ "$SPEC" != "$LINE" ] && ARGS=$(sed 's/^ *//; s/ *$//' <<< "${LINE#*|}")
  read -r NAME PATCH GAMES E0 E1 TC <<< "$SPEC"
  [ -z "${NAME:-}" ] || [ "${NAME:0:1}" = "#" ] && continue
  BASE=$(git -C "$R" rev-parse HEAD)
  NEW_ARGS=${ARGS:-${NEW_ARGS:-ab}} bash "$R/scripts/sprt_run.sh" "$NAME" "$BASE" "$PATCH" "$GAMES" "${TC:-movetime 100}" "$E0" "$E1" \
    > "$R/games/sprt/run_${NAME}.log" 2>&1
  V=$(grep -q "H1 ACCEPTED" "$R/games/sprt/summary_${NAME}.txt" 2> /dev/null && echo accept || echo reject)
  bash "$R/scripts/sprt_done.sh" "$NAME" "$V" >> "$R/games/sprt/run_${NAME}.log" 2>&1
  echo "$NAME: $V | $(grep -E '^Elo|^LLR' "$R/games/sprt/summary_${NAME}.txt" 2> /dev/null | tr '\n' ' ')"
done < "$1"
