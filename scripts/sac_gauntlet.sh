#!/bin/sh
# The sac gauntlet: a candidate engine against the sac-bot (kairos with SacBot=true, which plays
# an f7/f2 travel sac whenever one is legal) from the Turn Zero and Standard starts and the f7
# guide's lines (data/sac_gauntlet.g), then the losses-by-sac report (scripts/sac_gauntlet.py).
#
#   scripts/sac_gauntlet.sh <candidate kairos.exe> <sacbot kairos.exe> <games.jsonl> [games] [tc]
#
# The sac-bot is any kairos build with the SacBot option; keep the same one for every candidate.
# Both engines must be built inside the repo. On Windows run it at low priority, e.g.
#   cmd //c "start /low /b /wait sh scripts/sac_gauntlet.sh ..."
# CONC (default 4) sets the concurrency, SPRT the harness binary (default target/release/sprt).
set -e
SPRT=${SPRT:-target/release/sprt}
GAMES=${4:-240}
"$SPRT" --new "$1" --base "$2" --base-option SacBot=true --book data/sac_gauntlet.g \
  --tc "${5:-5+0.05}" --max-games "$GAMES" --min-games "$GAMES" --concurrency "${CONC:-4}" \
  --games-out "$3" --compact
python scripts/sac_gauntlet.py "$3"
