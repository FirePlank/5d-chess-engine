#!/usr/bin/env bash
# Run queued remote SPRTs one after another: each line of the queue file is
#   <name> <patch-file> <games> <elo0> <elo1> [tc...]
# tested against the current main HEAD; every test is recorded (and reverted on sprt) when
# done. Lines starting with # are skipped. A test whose run failed (setup error, null test,
# no results) is recorded as "error", never as a verdict, and the queue moves on.
#
# A formally accepted test (H1 ACCEPTED) is merged into main before the next test starts, so
# every test runs against the newest main: the patch is applied to the (clean) main checkout,
# built and tested, committed with its Final Summary and pushed. The commit subject comes
# from games/sprt/patches/<name>.msg when present (subject + description lines), else
# "<name> (SPRT accepted)". Set NO_AUTOMERGE=1 to only record verdicts.
set -uo pipefail
R=$(git rev-parse --show-toplevel)

automerge() {
  local name=$1 patch=$2 summary=$3 msg
  if [ -n "$(git -C "$R" status --porcelain --untracked-files=no)" ]; then
    echo "automerge skipped: main checkout has local changes"; return 1
  fi
  git -C "$R" fetch -q origin && git -C "$R" merge -q --ff-only origin/main || { echo "automerge skipped: main diverged from origin"; return 1; }
  git -C "$R" apply "$patch" || { echo "automerge failed: patch does not apply"; return 1; }
  if ! (cd "$R" && cargo build --release --bins -q && cargo test -q > /dev/null 2>&1); then
    git -C "$R" checkout -q -- . && git -C "$R" clean -fdq -- src tests data scripts
    echo "automerge failed: build or tests"; return 1
  fi
  msg=$(mktemp)
  if [ -f "$R/games/sprt/patches/$name.msg" ]; then cat "$R/games/sprt/patches/$name.msg"; else echo "$name (SPRT accepted)"; fi > "$msg"
  { echo; sed -n '/==== Final Summary/,$p' "$summary" | awk '!seen[$0]++'; } >> "$msg"
  git -C "$R" add -A src tests data scripts Cargo.toml Cargo.lock .github 2> /dev/null
  git -C "$R" commit -q -F "$msg" && git -C "$R" push -q origin main && echo "merged into main: $(git -C "$R" log --oneline -1)"
  rm -f "$msg"
}

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
    if [ "$V" = accept ] && [ -z "${NO_AUTOMERGE:-}" ]; then
      automerge "$NAME" "$(realpath "$PATCH")" "$S" 2>&1 | sed 's/^/  /'
    fi
  fi
done < "$1"
