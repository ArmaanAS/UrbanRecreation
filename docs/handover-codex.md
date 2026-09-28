# Handover: engine work while Claude supervises autoplay (2026-09-28)

The owner is moving engine development to Codex, while Claude keeps supervising the automated
Training games that feed it evidence. This note says what is running, what not to disturb,
and what to work on. `AGENTS.md` is the project guide and stays authoritative. Read it first,
then the top of `docs/replay-triage.md`, then the end of `docs/rust-migration.md`.

## What is running on this machine (do not disturb)

- `deno task log`: the log server on port 8787, in watch mode. It writes new battle
  captures and hosts the autoplay broker.
- `deno task decks`: Deck Lab on port 8788.
- `deno task autoplay` (`scripts/AutoPlay.ts`): the automated Training player, restarted run
  after run by a supervisor script. Claude looks after it and after the Edge game tab it
  drives.

All of these write into the **main checkout's working tree**
(`E:/Documents/NodeJS/UrbanRecreation-monorepo-20260915-180630`):
- new untracked files under `captures/battles/` and `captures/games/`, at about 60 games an
  hour;
- a growing `captures/abilities.json`, uncommitted;
- `data/situations.json`, `data/deck_history.jsonl` and `data/my_collection.json`.

So:
- **Work in your own worktree**, never in the main checkout. For example:
  `git worktree add ../ur-codex -b codex/<topic> main`.
  On Windows, give Rust a short target dir (`CARGO_TARGET_DIR=C:/t/ur-codex`), because long
  worktree paths break the build.
- In the main checkout, never run `git checkout`, `git stash`, `git clean`, `git reset`, or
  anything else that rewrites working-tree files. That includes `git checkout -- captures`.
- Do not restart or edit the log server unless you mean to. Watch mode restarts it on any
  change to `log_server.ts` or the modules it imports, and on restart it reloads
  `captures/abilities.json` from disk. If that file was older at that moment, abilities seen
  since then are lost from its memory and the next save drops them.
- Never write to the owner's account: deck saves, evolve, sell, buy. The autoplay deck saves
  are authorised for Claude's driver only.

## State at handover

See the numbers at the top of `docs/replay-triage.md` and in `AGENTS.md`, which were current
when this was written:
- **main:** the merge of branch `autoplay-run610`, a triage of 1,083 autoplay games.
- **Replays:** every replay-ready capture replays exactly except four:
  - single points 1025413, 1517236 and 1520327;
  - 1527862, new and not yet triaged.
- **Coverage:**
  - The TypeScript engine compiles about 97% of the 1,383 distinct printed ability texts.
  - About 60% of those texts, and the ability shapes of about 92% of max-level cards, have been
    seen firing correctly in real games.
  - The strict Rust engine can score about 77% of max-level cards exactly.

## The work, in priority order

### 1. The snapshot-and-triage loop (most of the value)

New captures pile up in the main checkout. Every few hundred games:

1. **Snapshot** the completed ones into a branch of your worktree:
   ```bash
   M=/e/Documents/NodeJS/UrbanRecreation-monorepo-20260915-180630
   cd "$M" && git status --short captures | grep '^?? captures/battles/' \
     | sed 's#^?? captures/battles/##; s#\.jsonl##' > /tmp/ids.txt
   ```
   Drop any id whose `captures/games/<id>.json` still has `finalStatus: "playing"` and was
   written in the last 30 minutes, because that battle is still live. Then, in your worktree:
   ```bash
   for id in $(tr -d '\r' < /tmp/ids.txt); do
     cp "$M/captures/battles/$id.jsonl" captures/battles/
     cp "$M/captures/games/$id.json" captures/games/
   done
   cp "$M/captures/abilities.json" captures/abilities.json
   ```
   Commit that as "Add the autoplay run ... captures". Watch for CRLF in id lists written by
   Windows tools; `tr -d '\r'` them.
2. **Make it green.**
   - Run `deno task pins:update`.
   - New card data always breaks some hand-maintained Rust inventories:
     - the previous-round inventory in `rust/src/engine/combat_stat_compiler.rs`;
     - the Support inventory in `rust/tests/combat_stat_diagnostic_replay.rs`;
     - the corpus skip list in `rust/src/replay/corpus.rs`;
     - the alias sets in `rust/src/effect_registry.rs` and `rust/tests/catalog_match.rs`;
     - `CLAN_BONUS_REGISTRY_BRIDGES`;
     - new `specialAction` / `attributeAction` enum values.
   - Classify each new id by what the compiler actually does with it; never just widen a set.
   - Then run `deno task check` and `deno test -A --no-check`, and build and run
     `deno task rust:worker:test`. New data has broken that worker gate twice.
3. **Triage** every new replay mismatch.
   - Work out by arithmetic what the server did.
   - Group the mismatches by ability keyword.
   - Search the whole corpus for other rounds that bear on the rule.
   - Rule out capture artifacts first: recap snapshots, and battles cut by a tab reload.
   - Fix only what two or more rounds, or the server's printed long description, settle.
     Add a focused test in `tests/ability/`.
   - File single points in the triage doc, with what a second capture would need to show.
   - Keep the Rust engine consistent where a rule is shared.
   - The continuation-cache rule in AGENTS.md applies to any new engine state.
4. **Docs and commits.** Update the triage table and status and the `AGENTS.md` counts. Make
   one commit per rule, with an "Add X, Fix Y" subject and a body quoting the arithmetic and
   the pin diff.

The four previous rounds of this loop are the commits between 5dee2ab and this handover. Use
them as examples.

### 2. Open single points

1025413, 1517236 and 1520327 are in "Single points waiting for a second capture" in
`docs/replay-triage.md`. 1527862 is the newest mismatch and is not yet triaged. Check each new
snapshot for their second data point.

### 3. Mechanics the TypeScript engine does not implement

These compile to nothing or are handled only partially:
- Hazard and Illusion (random or disguised abilities);
- Rebirth, Corrosion, Beyond, Limitless, Perfection, Overdose and Fatal Killshot;
- Memento's `Remove Ability Conditions`.

Autoplay sees the AI opponent's random decks, so search the corpus for rounds that hold them
before coding anything.

### 4. The Rust tail

About 48% of captured draws are strictly eligible. Each refusal names its reason (the ignored
coverage report prints them).
- The remaining owner decisions are in `docs/rust-migration.md`, for example Stop Opp.
  Ability or Brawl against a lone Team Leader.
- Prefer slices with a measured unlock.

## Merging into main

Main's working tree holds uncommitted captures, so a plain fast-forward can refuse. To merge:
1. From the main checkout, save a copy of `captures/abilities.json`.
2. Remove only the untracked files your branch adds:
   `git diff --name-only --diff-filter=A main <branch>`.
3. Run `git checkout -- captures/abilities.json captures/games`. The game-file changes there
   are line-ending noise.
4. Run `git merge --ff-only <branch>`.
5. Put the saved dictionary back, but only if its key set is a superset of the committed one.
6. Verify, then push.

If in doubt, leave the branch and ask the owner, or Claude, to merge it. The owner has
authorised tested pushes to main; never force-push.
