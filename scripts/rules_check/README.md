# Rules cross-check

Scripts that compare Kairos's rules with an independent reference implementation and with
other open-source 5D chess implementations. Results and verdicts: `RULES_VERIFICATION.md`.

| File | What it is |
|---|---|
| `ref5d.mjs` | Independent reference implementation of the rules (move generation, present, submit, check, legal actions), written from the rule definitions rather than from Kairos's code. |
| `js5d.cjs` | Adapter for [5d-chess-js](https://gitlab.com/5d-chess/5d-chess-js), loaded from a source checkout without `npm install`. |
| `col5d.mjs` | Adapter for the [5DC OL](https://github.com/ForkKILLET/5dcol) core (`packages/core/src/index.ts`). Its one dependency (`mutative`) is replaced by a structuredClone stand-in. |
| `compare.mjs` | Reads `ktools rules` output and diffs moves, checks, legal actions and mate status. |
| `coverage.mjs` | Counts which rules a dump exercised (castling, en passant, time travel, ...). |
| `select_small.py` | Picks positions small enough for exhaustive legal-action comparison. |
| `movekinds.py` | Summarises a diff dump by piece and 4D move vector. |

## Running

The other implementations go in untracked folders (`ext/` is git-ignored through `.git/info/exclude`):

```bash
git clone https://gitlab.com/5d-chess/5d-chess-js.git ext/5d-chess-js
git clone https://github.com/ForkKILLET/5dcol.git ext/5dcol
```

Nothing is installed or built for them. Node 22 or newer is needed for `--experimental-transform-types` (5dcol is TypeScript).

```bash
cargo build --release
# Positions: "G ..." lines (data/*.g, ktools randplay output, oracle `lines` output of 5DPGN games)
./target/release/ktools rules 0 nomate < positions.g > positions.rules        # moves and checks only
node --experimental-transform-types scripts/rules_check/compare.mjs positions.rules

./target/release/ktools rules 5000 < small.g > small.rules                   # + mate type and legal actions
node --experimental-transform-types scripts/rules_check/compare.mjs small.rules --actions --cap 5000

./target/release/ktools randplay 16 6 31 branchy < starts.g > random.g        # random games that time travel
./target/release/ktools playcheck < games.g                                   # replay games, check every action
```

`compare.mjs` options: `--actions`, `--mate-only`, `--budget N` (reference search nodes), `--col-mate`, `--js-mate`, `--no-js`, `--no-col`, `--show N`; with `DIFF_JSON=file` it also writes every difference as JSON.
