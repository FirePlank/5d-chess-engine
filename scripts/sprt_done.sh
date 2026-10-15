#!/usr/bin/env bash
# Record a finished test on the sprt branch. The test commit is always reverted so the branch
# matches main again (accepted changes land on main separately); the record carries the
# verdict and the result summary.
#   scripts/sprt_done.sh <name> accept|reject [summary-file]
set -euo pipefail
NAME=$1 VERDICT=$2
R=$(git rev-parse --show-toplevel); W="$R/../5d-sprt-branch"
SUMMARY=${3:-"$R/games/sprt/summary_${NAME}.txt"}
cd "$W"
git fetch -q origin && git reset -q --hard origin/sprt
C=$(git log --format=%H --grep="^test: $NAME\$" -1)
[ -n "$C" ] || { echo "no 'test: $NAME' commit on sprt" >&2; exit 1; }
MSG=$(mktemp); { echo "$VERDICT: $NAME"; echo; [ -f "$SUMMARY" ] && cat "$SUMMARY"; } > "$MSG"
git revert --no-edit "$C" > /dev/null
git commit -q --amend -F "$MSG"
rm -f "$MSG"
for try in 1 2 3 4 5; do git push -q origin sprt && break; [ $try = 5 ] && exit 1; sleep 10; done
echo "recorded $VERDICT: $NAME"
