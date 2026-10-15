#!/usr/bin/env bash
# SPRT on GitHub runners: commits `test: <name>` (= <base> + patch + .github/sprt.json) on the
# `sprt` branch, sums the shards' live pair counts, cancels the run once the LLR crosses a
# bound, and saves games + Final Summary under games/sprt. Record the verdict afterwards with
# scripts/sprt_done.sh.
#   scripts/sprt_run.sh <name> <base-sha> <patch-file|-> <games> [tc] [elo0] [elo1]
#   tc: "movetime 100" | "nodes 20000" | "depth 4"
#   Env: SHARDS (default 40), NEW_ARGS / OLD_ARGS (extra engine args, default none),
#        BASE (self | cpp:<engine>, e.g. cpp:zero-capture-check-pw), MIN_GAMES, RUN_ID,
#        OLD_SHA (build the old engine from this commit instead of <base-sha>, e.g. to
#        measure main against an earlier version; use with patch "-").
set -euo pipefail
NAME=$1 BASE_SHA=$2 PATCH=$3 GAMES=$4 TC=${5:-movetime 100} E0=${6:-0} E1=${7:-5}
R=$(git rev-parse --show-toplevel); W="$R/../5d-sprt-branch"
[ "$PATCH" = "-" ] || PATCH=$(realpath "$PATCH")
mkdir -p "$R/games/sprt"
ghr() { local o; for _ in $(seq 40); do o=$("$@") && { echo "$o"; return 0; }; sleep 30; done; return 1; }
REPO=$(ghr gh repo view --json nameWithOwner -q .nameWithOwner)
if [ -n "${RUN_ID:-}" ]; then
  ID=$RUN_ID; SHA=$(ghr gh run view "$ID" --json headSha -q .headSha)
else
  git -C "$R" fetch -q origin
  if [ ! -d "$W" ]; then
    if git -C "$R" ls-remote --exit-code --heads origin sprt > /dev/null; then
      git -C "$R" worktree add -q -B sprt "$W" origin/sprt
    else
      git -C "$R" worktree add -q -b sprt "$W" "$BASE_SHA"
    fi
  fi
  cd "$W"
  git ls-remote --exit-code --heads origin sprt > /dev/null && git reset -q --hard origin/sprt
  git merge -q --no-edit -m "merge main into sprt" "$BASE_SHA"
  if ! git diff --quiet "$BASE_SHA" HEAD -- . ':!.github/sprt.json'; then
    echo "sprt branch differs from $BASE_SHA outside .github/sprt.json; record or revert the last test first" >&2
    exit 1
  fi
  [ "$PATCH" = "-" ] || git apply "$PATCH"
  jq -n --arg n "$NAME" --arg o "${OLD_SHA:-$BASE_SHA}" --arg tc "$TC" --argjson g "$GAMES" \
        --argjson e0 "$E0" --argjson e1 "$E1" --argjson sh "${SHARDS:-40}" \
        --arg na "${NEW_ARGS:-}" --arg oa "${OLD_ARGS:-}" --arg b "${BASE:-self}" \
        '{name:$n, old:$o, games:$g, shards:$sh, tc:$tc, elo0:$e0, elo1:$e1, new_args:$na, old_args:$oa, base:$b}' \
        > .github/sprt.json
  git add -A && git commit -q -m "test: $NAME"
  SHA=$(git rev-parse HEAD)
  for try in 1 2 3 4 5; do git push -q origin sprt && break; [ $try = 5 ] && exit 1; sleep 10; done
  ID=""
  for _ in $(seq 60); do
    ID=$(gh run list --branch sprt --limit 10 --json databaseId,headSha,workflowName \
         -q ".[] | select(.headSha==\"$SHA\" and .workflowName==\"SPRT remote\") | .databaseId" | head -1)
    [ -n "$ID" ] && break; sleep 5
  done
fi
echo "run $ID  https://github.com/$REPO/actions/runs/$ID"
STALL_S=1200
declare -A LAST_SEEN LAST_CHANGE
run_status() { ghr gh api "repos/$REPO/actions/runs/$ID" -q .status; }
cancel_and_wait() {
  ghr gh api -X POST "repos/$REPO/actions/runs/$ID/cancel" > /dev/null
  until [ "$(run_status)" = completed ]; do sleep 5; done
}
tick=0
while :; do
  [ "$(run_status)" = completed ] && break
  if (( tick % 6 == 0 )); then
    LINES=$(gh api "repos/$REPO/commits/$SHA/status?per_page=100" \
      -q '[.statuses[] | select(.context | startswith("sprt/shard-"))] | group_by(.context)
          | map(max_by(.updated_at)) | .[] | "\(.context)|\(.state)|\(.description)"' 2> /dev/null || true)
    SUM=$(awk -F'|' 'NF == 3 { split($3, c, ","); for (i = 1; i <= 5; i++) s[i] += c[i] }
          END { printf "%d,%d,%d,%d,%d", s[1], s[2], s[3], s[4], s[5] }' <<< "$LINES")
    OUT=$(python "$R/scripts/sprt_merge.py" --from-counts "$SUM" --elo0 "$E0" --elo1 "$E1")
    echo "$(date +%H:%M:%S) $(tr '\n' ' ' <<< "$OUT")"
    NOW=$(date +%s); STUCK=""
    while IFS='|' read -r CTX STATE DESC; do
      [ -n "$CTX" ] || continue
      if [ "${LAST_SEEN[$CTX]:-}" != "$DESC" ]; then LAST_SEEN[$CTX]=$DESC; LAST_CHANGE[$CTX]=$NOW; fi
      if [ "$STATE" = pending ] && [ $(( NOW - ${LAST_CHANGE[$CTX]} )) -ge $STALL_S ]; then STUCK="$STUCK $CTX"; fi
    done <<< "$LINES"
    if [ -n "$STUCK" ]; then
      echo "stalled for $((STALL_S / 60)) min:$STUCK; cancelling"
      cancel_and_wait; break
    fi
    if grep -q '^stop=true' <<< "$OUT" && [ "$(sed -n 's/^pairs=//p' <<< "$OUT")" -ge $(( ${MIN_GAMES:-0} / 2 )) ]; then
      echo "LLR bound crossed ($(grep '^llr=' <<< "$OUT")): run cancelled"
      cancel_and_wait; break
    fi
  fi
  tick=$((tick + 1)); sleep 10
done
T="$R/games/sprt/.remote_${NAME}_$$"; rm -rf "$T"
until timeout 600 gh run download "$ID" -p "shard-*" -D "$T"; do
  echo "shard download failed; retrying in 30s" >&2; rm -rf "$T"; sleep 30
done
python "$R/scripts/sprt_merge.py" --label "$NAME" --old "${OLD_SHA:-$BASE_SHA}" --elo0 "$E0" --elo1 "$E1" \
  --out "$R/games/sprt/games_${NAME}.jsonl" "$T"/shard-*/shard_*.jsonl \
  | tee "$R/games/sprt/summary_${NAME}.txt"
rm -rf "$T"
