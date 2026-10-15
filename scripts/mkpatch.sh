#!/usr/bin/env bash
# Write games/sprt/patches/<name>.patch from a sed edit of one file (working tree restored).
#   scripts/mkpatch.sh <name> <file> <sed-expr>
set -euo pipefail
NAME=$1 FILE=$2 EXPR=$3
mkdir -p games/sprt/patches
cp "$FILE" "$FILE.mkpatch.bak"
sed -i "$EXPR" "$FILE"
git diff -- "$FILE" > "games/sprt/patches/$NAME.patch"
mv "$FILE.mkpatch.bak" "$FILE"
[ -s "games/sprt/patches/$NAME.patch" ] || { echo "empty patch for $NAME" >&2; exit 1; }
echo "games/sprt/patches/$NAME.patch: $(grep -c '^[-+][^-+]' "games/sprt/patches/$NAME.patch") changed lines"
