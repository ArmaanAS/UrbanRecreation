# UrbanRecreation — agent guide

A Deno + TypeScript recreation of the **Urban Rivals** card-game engine, plus a game-tree
solver and a pipeline for capturing real games from the live site to use as ground truth.
Written by hand by the repo owner (armaanas); AI assistance started September 2026.

## Layout

| Path | Purpose |
| --- | --- |
| `src/game/` | Engine: `Game`, `Hand`, `Card`, `Player`, `PlayerRound`; abilities are parsed from text by `AbilityParser.ts` → `Ability.ts` → `modifiers/*`; a round is resolved by `battle/CardBattle.ts` firing `Events` at ordered `EventTime`s (START, PRE4..PRE1, POST1..POST4, END). `battle/Cached*` are memoised variants used by the solver. |
| `src/solver/` | Minimax / iterative game-tree analysis with worker threads. |
| `src/utils/` | Console rendering, misc helpers. |
| `data/` | `data.json` = the card list the engine loads, **one row per card per level** (power, damage and ability differ by level), built by `deno task cards` from `site_characters.jsonl` (gitignored 40 MB dump of the site's own card DB, refreshed via `__ur.dumpCharacters()` in the browser). `site_clans.json` (from `__ur.dumpClans()`) supplies clan names/bonuses; otherwise they come from the legacy `cards.json`. `cards.json` / `data.maxlevel.json` = older OAuth-API dumps (Dec 2024, max level only, stale); `compiled.json` = ability inventory from `deno task compile`. |
| `scripts/` | Card data (`BuildCardData.ts` is the live path; `RequestCards.ts` / `RequestAllCardLevels.ts` / `UR_API.ts` are the OAuth-API path, needs API_KEY/API_SECRET in `.env` plus a browser auth step), ability compiler (`CompileAbilities.js`), battle capture (`BattleCapture.ts`, `ExtractBattle.ts`). |
| `tests/` | `deno test -A`. Per-ability tests in `tests/ability/`, replay of captured games in `tests/replay/`, Rust cross-check testcases in `tests/rust/`. |
| `ur-logger.user.js` + `log_server.ts` | Tampermonkey userscript mirroring site traffic to a local server that writes `ur_log.jsonl` (raw, **contains tokens, gitignored**) and secret-free per-battle files in `captures/battles/`. Binary response bodies (WebGL asset bundles, images, wasm) are dropped on both sides: `res.text()` decodes them as lossy UTF-8, so they are unrecoverable garbage, and unfiltered they were 65% of the first real log. `scripts/PruneLog.ts` retro-fixes older logs. |
| `captures/` | `battles/<id>.jsonl` raw battle capture in a compact lossless form (static block once + one dynamic line per `battles.status` poll, ~20 KB/battle instead of ~450 KB; `expandStatus()` in `scripts/BattleCapture.ts` rebuilds the original server objects); `abilities.json` shared ability/bonus dictionary (id → description + structured `abilityData`); `games/<id>.json` clean game records with decks, moves, per-round resolution, life/pillz, and an engine `testcase`. All safe to commit. |

## Common commands

```bash
deno test -A --no-check          # run tests (type-check currently fails on src/utils/Utils.ts:184)
deno task log                    # start capture server, then play on urban-rivals.com with the userscript on
deno task extract                # captures/battles/*.jsonl → captures/games/*.json
deno task extract --raw ur_log.jsonl   # re-split a raw log into battle files
deno task prune                  # shrink ur_log.jsonl (dropped 5.4 GB -> 82 MB once); --replace to swap it in
deno test -A --no-check tests/replay/  # replay captured games through the engine
deno task cards                  # rebuild data/data.json from data/site_characters.jsonl (after __ur.dumpCharacters())
deno task bench / deno task time # solver benchmark
```

## Current priorities (Sept 2026)

1. Capture many real PvP games and make the engine reproduce them (`tests/replay/`).
   As of 2026-09-11: 62 battles captured, 60 replayable, 57 replay exactly (life, pillz,
   power, damage, attack, winner per round), 3 mismatch. `docs/replay-triage.md` tracks what
   was fixed and what is open (Damage Exchange, Revenge/Impose, plus one Hazard game that a
   name-and-level testcase cannot express). It also says which open questions need more
   captured games and what to play to answer them.
   Work through that list, but check each entry against `captures/games/<id>.json` before
   coding: several turned out to be misattributed, two of them to abilities that were
   already implemented. Re-run the replay suite after each fix.

## How to resume (read this first in a new session)

1. `deno test -A --no-check tests/replay/` — the score to beat is in the table at the top of
   `docs/replay-triage.md`. Type-checking fails on a pre-existing issue in
   `src/utils/Utils.ts:184`, hence `--no-check`.
2. New games: the owner runs `deno task log` (it restarts itself when the server file
   changes) and plays; then `deno task extract` and re-run the replay suite. The userscript
   is at 0.5 - if `ur_log.jsonl` starts growing by gigabytes again, Tampermonkey is still
   running an older copy and needs it re-pasted. Failures print the round, both cards and the engine-vs-server diff;
   `captures/games/<id>.json` has the full round (moves, abilities, server results,
   post-round effects). Group new failures by ability keyword before fixing anything.
3. Fix loop: change the engine → replay suite → full `deno test -A --no-check` (two legacy
   failures are expected: `Game_2 Protection`, `Oculus Infiltrated`) → update the triage doc
   → commit with a subject in the repo's "Add X, Fix Y" style.
4. Prefer fixes backed by ≥2 captured data points; note single-point hypotheses in the
   triage doc instead of coding them. Every ability rule fixed so far was confirmed by
   arithmetic against the server's power/damage/attack numbers, not by reading rules text.
5. Do not push without asking the owner. Never commit `ur_log*.jsonl`, `tokens.json`, `.env`,
   `data/site_characters.jsonl`.
2. Card data is complete as of 2026-09-10 (2496 cards, every level, 36 clans incl. the new
   Tolvack). To refresh: `__ur.dumpCharacters()` and `__ur.dumpClans()` in the browser (log
   server running), then `deno task cards`. A new clan must also be added to `Clans` in
   `src/game/types/CardTypes.ts`. Cards that became collectors got a " Cr" suffix; the loader
   keeps the old name as an alias.
3. Longer term: replace regex ability parsing with the structured `abilityData` the battle
   API returns per card (see any `captures/games/*.json`; the site card DB dump does NOT
   include it), and build a "what do players play" dataset from captured moves and timings.
4. Engine bugs are addressed only when a replay exposes them; two legacy tests
   (`Game_2 Protection` — empty card names, `Oculus Infiltrated`) fail and predate this work.

## Conventions

- Deno, no Node build step. Imports via `@/` (src) and `@data/` aliases in `deno.json`.
- Engine code is performance-sensitive (solver explores millions of states): avoid logging
  or allocation in `CardBattle`, `Events`, modifiers. Debug `console.log`s are removed
  before commit.
- Never commit `ur_log*.jsonl`, `tokens.json`, or `.env`. Capture files under `captures/`
  are redacted by `BattleCapture.ts` and are fine to commit.
- Player names/ids in captures are public profile data from the game; keep it that way.

## Domain notes

- A game is 4 rounds; each round both players pick a card, pillz (0..remaining) and
  optional fury (+2 damage, costs 3 pillz). Attack = power × (pillz + 1). Higher attack
  wins the round; the winner's damage is subtracted from the loser's life. KO ends the game.
- First mover alternates each round; the engine's `Turn.PLAYER_1` is whoever moved first
  in round 0. Captured testcases are normalised to that convention.
- Pillz conventions differ: the engine's `select(index, pillz, fury)` pillz excludes the free
  pill (attack = power × (pillz + 1)); the server's `pillzUsed` includes it (attack =
  power × pillzUsed) and excludes the 3 fury pillz. `ExtractBattle.ts` converts.
- Server snapshots (`battles.status`) hide a player's pillz until the round resolves, show
  `roundAttack` only in the snapshot right after resolution, and the final "done" snapshot
  does not include the last round's damage; `ExtractBattle.ts` handles all three.
- Each card in a battle snapshot carries a `state`, which is a property of that player's
  *copy* of the card and not of the card itself. Seen so far: `""` (plain), `"p"`, `"s"`,
  and `"m1"` / `"m2"` / `"m3"`. `p` and `s` are Prismatic and Savage, display editions that
  give the card an animated background and make it worth more to sell; they turn up on every
  rarity. The `m` values look like three tiers of a third edition - they sit almost only on
  Cr cards, and the same character appears at different tiers on different copies (Merweiss
  Cr as both m1 and m3, Kolos Cr as m1 and m2), so they are a per-copy grade rather than a
  card property. It is not `isMeteora` from the card DB, which does not correlate.
  **None of them change stats or abilities**, so the engine ignores `state` entirely: cards
  carrying `p`, `s`, `m1` and `m3` were played in 34 rounds of games that replay exactly,
  with power, damage, attack and winner reproduced from the plain `data/data.json` numbers.
