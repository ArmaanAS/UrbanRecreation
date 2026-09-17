# UrbanRecreation

UrbanRecreation is a recreation of the Urban Rivals card-game engine, a game-tree solver,
and a capture pipeline for checking the implementation against real games.

The repository contains two implementations:

- The Deno + TypeScript engine at the repository root is the current reference. It owns
  live capture, canonical card data, replay tests, and the working advisor.
- The Rust implementation in [`rust/`](rust/) is being brought back to parity as a
  candidate high-performance engine and solver backend. It now has an experimental
  current-engine advisor for strict supported draws; the TypeScript advisor remains the
  live, full-policy reference.

Keeping both implementations here lets them share the same card data and captured games.
It also makes every parity change reviewable beside the server-backed reference that
motivated it.

## Repository map

| Path | Purpose |
| --- | --- |
| `src/game/` | TypeScript game engine and battle resolution. |
| `src/solver/` | TypeScript search, policy, advisor, and terminal view. |
| `scripts/` | Card-data, capture, extraction, and maintenance tools. |
| `data/` | Canonical card data, with one row per card level. |
| `captures/games/` | Redacted real-game records used as ground truth. |
| `tests/` | TypeScript engine, solver, replay, and cross-check tests. |
| `rust/` | Rust library, experimental current-engine advisor, and opt-in historical advisor. |

See [`AGENTS.md`](AGENTS.md) for the detailed code map and
[`docs/rust-migration.md`](docs/rust-migration.md) for the parity plan and source-of-truth
rules.

## TypeScript

Install Deno, then run:

```bash
deno test -A --no-check
deno test -A --no-check tests/replay/
deno task advise
```

The replay suite intentionally exposes known server mismatches while that backlog is being
worked through; its current score is recorded in `docs/replay-triage.md`.

The engine's `select(index, pillz, fury)` call uses paid pillz, excluding the free pill:

```ts
const game = Game.create(false);
game.select(0, 3, false);
game.select(1, 0, false);
```

## Rust

The normal Rust build is a library and does not compile the historical HTTP advisor:

```bash
deno task rust:check
deno task rust:test
deno task rust:advise --plain
deno task rust:advise --interactive --plain
deno task rust:advise --replay 877636 --plain
deno task rust:advise --replay 1024673 --plain
```

`rust:advise` defaults to a deterministic supported demo and accepts exact `id:level`
hands; run `deno task rust:advise --help` for the complete interface. It resolves the real
current Rust engine's complete current-round matrix with make/unmake and renders a bounded
terminal ranking. `--interactive` carries the same strict match through four manually
observed rounds, asking for the opponent's revealed card before second-mover advice and
showing updated resources after each resolution. Rounds 1–2 remain clearly labelled
position heuristics; rounds 3–4 use an exact conservative continuation policy that never
conditions a reply on hidden pillz or Fury. `--replay` loads a captured battle through the
strict catalog boundary, renders and grades every recorded decision, then verifies each
committed round's power, damage, attack, winner, life, and pillz against the server record.
Battles `877636` and `1024673` are complete supported real draws; the latter exercises
unconditional Stop Opp. Ability through the real solver/TUI and ends in a round-two KO.
Rust does not yet ingest the live capture stream, use captured opening weights, or claim
complete TypeScript policy parity.

To inspect the old advisor explicitly:

```bash
deno task rust:legacy
```

Rust parity work should consume `data/data.json` and `captures/games/` from this repository.
The files under `rust/assets/` preserve the old implementation's historical baseline; they
are not the current source of truth.

## Capture safety

Raw logger output can contain authentication tokens and is intentionally ignored. Never
commit `ur_log*.jsonl`, `tokens.json`, `.env`, or `data/site_characters.jsonl`. Files under
`captures/` are redacted by the capture pipeline and are safe to version.
