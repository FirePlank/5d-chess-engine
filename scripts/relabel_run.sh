#!/usr/bin/env bash
# Relabel positions on GitHub runners: commits `relabel: <name>` (main + .github/relabel.json +
# the picks split into shards) on the `relabel` branch, waits for the run and merges the shards'
# labels into games/nnue/labels_<name>.tsv (line numbers of <picks>, as ktools nnuedata's
# NNUE_LABELS reads them).
#   scripts/relabel_run.sh <name> <picks.jsonl (ktools labelpick)> <shards> <nodes> [max_seconds]
set -euo pipefail
NAME=$1 PICKS=$(realpath "$2") SHARDS=$3 NODES=$4 MAXS=${5:-19800}
R=$(git rev-parse --show-toplevel); W="$R/../5d-relabel-branch"
ghr() {
  local o end=$(( $(date +%s) + 1800 )) n=0
  while :; do
    o=$(timeout 60 "$@") && { echo "$o"; return 0; }
    n=$((n + 1)); [ "$(date +%s)" -ge "$end" ] && return 1; sleep 15
  done
}
git -C "$R" fetch -q origin
if [ ! -d "$W" ]; then
  if git -C "$R" ls-remote --exit-code --heads origin relabel > /dev/null; then
    git -C "$R" worktree add -q -B relabel "$W" origin/relabel
  else
    git -C "$R" worktree add -q -b relabel "$W" "$(git -C "$R" rev-parse HEAD)"
  fi
fi
cd "$W"
git ls-remote --exit-code --heads origin relabel > /dev/null && git reset -q --hard origin/relabel
git merge -q --no-edit -m "merge main into relabel" "$(git -C "$R" rev-parse HEAD)" || git merge -q --no-edit -X theirs "$(git -C "$R" rev-parse HEAD)"
mkdir -p .github "relabel/$NAME"
cp "$R/.github/workflows/relabel.yml" .github/workflows/relabel.yml
# Line i of the picks goes to shard (i % SHARDS) + 1.
python3 - "$PICKS" "relabel/$NAME" "$SHARDS" <<'EOF'
import gzip, sys
picks, out, n = sys.argv[1], sys.argv[2], int(sys.argv[3])
fs = [gzip.open(f'{out}/shard_{k + 1}.jsonl.gz', 'wt', encoding='utf8') for k in range(n)]
for i, line in enumerate(open(picks, encoding='utf8')):
    fs[i % n].write(line)
for f in fs:
    f.close()
EOF
printf '{"name": "%s", "shards": %s, "nodes": %s, "max_seconds": %s}\n' "$NAME" "$SHARDS" "$NODES" "$MAXS" > .github/relabel.json
git add .github/relabel.json .github/workflows/relabel.yml "relabel/$NAME"
git commit -q -m "relabel: $NAME"
SHA=$(git rev-parse HEAD)
ghr git push -q origin relabel
REPO=$(ghr gh repo view --json nameWithOwner -q .nameWithOwner)
ID=""
while [ -z "$ID" ]; do
  sleep 20
  ID=$(ghr gh run list -R "$REPO" -w Relabel --json databaseId,headSha -q ".[] | select(.headSha == \"$SHA\") | .databaseId" | head -1)
done
echo "run $ID  https://github.com/$REPO/actions/runs/$ID"
while :; do
  st=$(ghr gh run view "$ID" -R "$REPO" --json status -q .status || echo unknown)
  [ "$st" = completed ] && break
  sleep 60
done
D="$R/games/nnue/relabel_$NAME"
rm -rf "$D"; mkdir -p "$D"
until ghr gh run download "$ID" -R "$REPO" -D "$D" -p 'label-*'; do sleep 30; done
# Shard k's line j is line j * SHARDS + k - 1 of the picks.
python3 - "$D" "$SHARDS" "$R/games/nnue/labels_$NAME.tsv" <<'EOF'
import glob, os, sys
d, n, out = sys.argv[1], int(sys.argv[2]), sys.argv[3]
rows = 0
with open(out, 'w') as o:
    for k in range(1, n + 1):
        for p in glob.glob(f'{d}/label-{k}/label_{k}.tsv'):
            for line in open(p):
                f = line.rstrip('\n').split('\t')
                if len(f) == 5:
                    f[0] = str(int(f[0]) * n + k - 1)
                    o.write('\t'.join(f) + '\n')
                    rows += 1
print(rows, 'labels ->', out)
EOF
