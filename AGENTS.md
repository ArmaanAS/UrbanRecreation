# UrbanRecreation — agent guide

A Deno + TypeScript recreation of the **Urban Rivals** card-game engine, plus a game-tree
solver and a pipeline for capturing real games from the live site to use as ground truth.
Written by hand by the repo owner (armaanas); AI assistance started September 2026.

## Layout

| Path | Purpose |
| --- | --- |
| `src/game/` | Engine: `Game` (incl. `make`/`unmake` + `Undo`, the allocation-free way the solver explores), `Hand`, `Card`, `Player`, `PlayerRound`; abilities are parsed from text by `AbilityParser.ts` → `Ability.ts` → `modifiers/*`; a round is resolved by `battle/CardBattle.ts` firing `Events` at ordered `EventTime`s (START, PRE4..PRE1, POST1..POST4, END). `battle/Cached*` are memoised variants used by the solver. |
| `src/solver/` | `Minimax.ts` + `Analysis.ts` = the original breadth-first game-tree search (`iterTree`, used by `worker.ts` and `Main.ts`; `iterTree3`/`processRound` is an unused DFS rewrite). `Deep.ts` is its allocation-free perfect-information reference. `Search.ts` divides live work into depth-2 units and hands each future subtree to `Policy.ts`, whose conservative pure policy never conditions our reply on hidden pillz/Fury. `Advisor.ts` + `SolverView.ts` are the live terminal view. |
| `src/utils/` | Console rendering, misc helpers. |
| `data/` | `data.json` = the card list the engine loads, **one row per card per level** (power, damage and ability differ by level), built by `deno task cards` from `site_characters.jsonl` (gitignored 40 MB dump of the site's own card DB, refreshed via `__ur.dumpCharacters()` in the browser). `site_clans.json` (from `__ur.dumpClans()`) supplies clan names/bonuses; otherwise they come from the legacy `cards.json`. `cards.json` / `data.maxlevel.json` = older OAuth-API dumps (Dec 2024, max level only, stale); `compiled.json` = ability inventory from `deno task compile`. |
| `scripts/` | Card data (`BuildCardData.ts` is the live path; `RequestCards.ts` / `RequestAllCardLevels.ts` / `UR_API.ts` are the OAuth-API path, needs API_KEY/API_SECRET in `.env` plus a browser auth step), ability compiler (`CompileAbilities.js`), battle capture (`BattleCapture.ts`, `ExtractBattle.ts`). |
| `tests/` | `deno test -A`. Per-ability tests in `tests/ability/`, replay of captured games in `tests/replay/`, Rust cross-check testcases in `tests/rust/`. |
| `rust/` | Imported Rust implementation and candidate high-performance backend. `engine::BaseRulesGame` remains the effects-disabled 20-round reference; `engine::ClanBonusDiagnostic` preserves its separate 40-round projected gate; `engine::CombatStatDiagnosticV1` adds a distinct fail-closed combat-stat projection with a 905-round server-backed gate; at semantic revision 76 it makes 302 of the 383 complete captured draws strictly eligible (revisions 38-76 are written up at the end of `docs/rust-migration.md`, each with its measured unlock, its evidence and the contexts it refuses rather than guesses). Semantic revision 37 retains the bounded numeric and post-round slices, adds exact unconditional Stop Opp. Ability with allocation-free TypeScript-compatible PRE4 dependency resolution, captured Reprisal SOA aliases `1310`/`2073` with `OwnerMovesSecond`, exact Komboka Bonus `1714` `+1 Pillz And Life`, the reviewed Victory-or-Defeat Life identities (`+1`/`+2` own Life and `-1` opponent Life Min 1), exact Equalizer opponent-Life definitions `1415`/`4458`, Anita level 3's exact Ability `274` Courage conversion from final resolved damage to Life, the unconditional opponent-Life reduction as one grammar on all three outcome channels - the plain `-N Opp. Life, Min M` Victory form over every printed ability (`512`/`524`/`594`/`602`/`769`/`842`/`935`/`1002`/`1399`/`3491`/`3571`/`3716`/`4948`), its `Victory Or Defeat: - N Opp. Life Min M` form (`1386`/`1628`/`1726`/`3367`/`4331`) and the losing-side `Defeat:` form - with the active Berzerk `bonus:680` and `1628` identity-locked because the Bonus slot is not theirs alone, alongside the reviewed conditional Victory ones (Doela Noel's Symmetry `4708` and Diabolus' Confidence `3016`/`4301`), and `Copy: Opp. Ability`/`Copy: Opp. Bonus` both unconditionally and under `Reprisal:`/`Revenge:`, which adopt the opposing selected card's plan in the copier's own slot and Support context while the adopted plan keeps its own predicate, the three reviewed Protection grammars (`Protection: Power And Damage`, `Protection: Ability` and `Protection: Bonus`), which refuse an opposing reduction and keep a stopped source alive, the three unconditional stat Copy grammars (`Copy: Opp. Power`, `Copy: Opp. Damage` and `Copy: Power And Damage Opp.`), which take the opposing card's printed value before every modifier, `Asymmetry:` source Copy, `+N Attack Per Opp. Damage`, whose magnitude is the opposing card's resolved Damage before Fury, and the four plain permanent grammars on one latch - `Heal N Max. M` and `Regen N, Max. M` on the owner's Life below a cap, `Poison N, Min M` (including the active Freaks bonus) and `Toxin N, Min M` on the opposing player's Life above a floor - which a live winning round writes into the owner's position so that every later round pays them (Toxin and Regen also pay in the latching round; Poison and Toxin keep paying when their owner is knocked out and a Min 0 Toxin can end the match), the plain `+N Pillz` Victory grammar (`337`/`455`/`503`/`1054`/`1150`/`1229`/`2262`/`2525`/`4855`/`5258`), admitted by exact text and shape from the Ability slot and paid to a living winner after the bet, with its tight-printed `Confidence: +N Pillz` form (`1702`/`4449`) carrying the previous-round predicate, the plain `-N Opp Pillz. Min M` Victory grammar (`334`/`339`/`343`/`360`/`570`/`854`/`3541`/`5532`/`5682`), which takes from the opposing player's post-bet Pillz and leaves a target at or below Min alone, its losing-side `Defeat: -N Opp. Pillz, Min M` sibling (`912`), which reads the same post-bet Pillz when its owner loses and still pays from an owner the round has knocked out, and `+1 Pillz Per Damage` (`809`/`1051`/`1090`) with its `Symmetry:` form (`1852`), which pays the winner its final resolved Damage under the plan's hand-slot predicate, `+N Life Per Damage` (`141`/`189`/`226`/`492`/`1125`/`1224`/`4500`) with its `Revenge:` (`1661`), `Confidence:` (`1810`) and capped `Max. M` (`1146`/`1161`) forms, the capped one never carrying its owner past the cap and never paying an owner already there, and the four plain permanents under one `Symmetry:`/`Asymmetry:`/`Revenge:`/`Confidence:` prefix as the plan's predicate (`5092`, `5692`, `5693`, `3301`), the both-sides `Xantiax: -N Life, Min. M` reduction, which charges each player at the end of the round whatever the outcome and floors each of them independently, fixed Victory Life under the two conditions it prints - `Asymmetry: +3 Life` (`2638`) from the hand-slot field and `Confidence : +N Life` (`814`/`2113`/`3546`) from the previous-round field, both card abilities only - alongside strict ability-only ordinary Defeat Life and Lobo's evidence-backed Reanimate identity; unobserved same-text/source variants, every prefixed permanent (`Defeat`, `Killshot`, `Symmetry`, `Asymmetry`, `Growth`, `Unison`, `Revenge`, `Backlash`, `Perfect`, Victory-or-Defeat and clan-gated forms) and the Pillz permanents (Dope, Consume, Repair, Combust, Mindwipe), Anita levels 1–2, Bonus/Copy provenance, other conditional SOA, the other Protection grammars (`Protection: Power`, `Protection: Attack`, the spaced `Protection : Damage` and its clan-conditional form), Unison and conditional stat-copying Copy, Power/Damage Exchange, Courage `4533` and Growth `1730` opponent-Life, dynamic catalog Copy, capped combat-stat increases, a capped Life conversion under a previous-round prefix, compound Life/Pillz, and other Reanimate identities remain fail-closed. `engine::CatalogCombatStatMatchV1` is the strict catalog-only constructor for search positions: it applies reviewed runtime card overrides, derives immutable Oculus/effective-clan and day/night context, and rejects a draw unless all eight cards are executable by that projection. `advisor/` contains a current-engine make/unmake search, bounded TUI, manual four-round mode, a fixed 198-play historical opening prior, exact conservative rounds 2–4 policy, a manual blind-second pass, and strict captured-replay grading. The root search runs its transactional row/column blocks on every hardware thread by default (`--threads N` on both binaries; one thread is the old serial loop, and a complete result is bit-identical at any count). The precompiled one-request JSONL V3 worker lets the TypeScript host use supported FIRST, SECOND, and blind-second decisions after validating data fingerprints, compiler/catalog/policy semantic revisions, replayed history, information-set identity, legal actions, bounded SECOND wager outcomes, and response bounds. Complete captures `877636`, `877812`, `925674`, `925719`, `1024673`, `1060199`, `1061897`, `1069813`, `1081463`, and `1089346` are current end-to-end gates, not full live-policy parity. The historical engine remains beside them, and the old HTTP advisor is behind `legacy-advisor`. New Rust work reads the root canonical data and captures—`rust/assets/` is historical only. See `docs/rust-migration.md`. |
| `ur-logger.user.js` + `log_server.ts` | Tampermonkey userscript mirroring site traffic to a local server that writes `ur_log.jsonl` (raw, **contains tokens, gitignored**) and secret-free per-battle files in `captures/battles/`. Binary response bodies (WebGL asset bundles, images, wasm) are dropped on both sides: `res.text()` decodes them as lossy UTF-8, so they are unrecoverable garbage, and unfiltered they were 65% of the first real log. `scripts/PruneLog.ts` retro-fixes older logs. |
| `captures/` | `battles/<id>.jsonl` raw battle capture in a compact lossless form (static block once + one dynamic line per `battles.status` poll, ~20 KB/battle instead of ~450 KB; `expandStatus()` in `scripts/BattleCapture.ts` rebuilds the original server objects); `abilities.json` shared ability/bonus dictionary (id → description + structured `abilityData`); `games/<id>.json` clean game records with decks, moves, per-round resolution, life/pillz, and an engine `testcase`. All safe to commit. |

## Common commands

```bash
deno test -A                     # run tests (type-checks first; --no-check skips that)
deno task check                  # type-check src/ tests/ scripts/ log_server.ts
deno task log                    # optional standalone capture/debug server
deno task extract                # captures/battles/*.jsonl → captures/games/*.json
deno task extract --raw ur_log.jsonl   # re-split a raw log into battle files
deno task prune                  # shrink ur_log.jsonl (dropped 5.4 GB -> 82 MB once); --replace to swap it in
deno test -A --no-check tests/replay/  # replay captured games through the engine
deno task cards                  # rebuild data/data.json from data/site_characters.jsonl (after __ur.dumpCharacters())
deno task bench / deno task time # iterTree benchmark (the breadth-first reference)
deno task time-search            # Search benchmark - the depth-first path the advisor uses
deno task advise                 # live view; starts its capture server automatically
deno task advise --replay 866431 --budget 10   # grade your moves in a captured battle
deno task rust:check             # Rust library + optional historical advisor compile check
deno task rust:test              # Rust foundation, catalog, replay and current base-engine tests
deno task pins:update            # regenerate the derived pins, then review `git diff rust/tests/expect tests/expect`
deno task rust:worker            # build the precompiled Rust JSONL advisor worker
deno task rust:worker:test       # build it and run the real-process V3 integration gate
deno task rust:advise --plain    # current Rust engine: strict current-round TUI
deno task rust:advise --interactive --plain  # manually advance the supported match
deno task rust:advise --replay 877636 --plain  # grade and verify a real captured match
deno task rust:advise --replay 1024673 --plain # SOA + round-two KO replay smoke
deno task time-rust              # same decisions in both solvers, time + semantic verdict
deno task rust:advise --exact-opening --plain  # solve round one instead of estimating it
deno task advise --rust=use --exact-opening    # the same, through the hosted worker
UR_SLOW_PARITY=1 deno test -A --no-check tests/solver/ExactOpeningParity.test.ts  # its gate
deno task advise --rust=compare  # TS stays authoritative; compare every supported Rust mode
deno task advise --rust=use      # use complete protocol-validated Rust results, else TS fallback
UR_DEBUG=1 deno test -A --no-check tests/ability/   # verbose engine tracing (off by default)
```

## Current priorities (Sept 2026)

1. Capture many real PvP games and make the engine reproduce them (`tests/replay/`).
   As of 2026-09-25: **383 battles captured, 376 replay-ready, 368 replay exactly** (life,
   pillz, power, damage, attack, winner per round), **8 mismatch**, and 7 captures ignored:
   6 stopped mid-match, and 1414087 deals card 2714, which is newer than the card data. Dojo (battle rule 6) battles are now extracted and
   replayed like any other room: see the triage doc for why the old "rules differ" exclusion
   did not survive contact with the captures. All eight remaining mismatches are triaged in
   the doc: each waits on a second capture or an open ordering question, so new games that
   exercise those abilities are what moves this number now. The triage table is the authority here - this line has gone stale
   before, so re-run the suite rather than quoting it.
   The corpus grew by 31 on 2026-09-17 because commit `b7a56d1` had archived 29 battle
   captures without ever extracting them; `deno task extract` is byte-identical for every
   game already committed, so run it before trusting a count here.
   `docs/replay-triage.md` tracks what
   was fixed and what is open (Revenge/Impose and single-point rules waiting for a second
   capture). It also says which open questions need more
   captured games and what to play to answer them.
   Work through that list, but check each entry against `captures/games/<id>.json` before
   coding: several turned out to be misattributed, two of them to abilities that were
   already implemented. Re-run the replay suite after each fix.
2. **Backlog, not started: deck building for a game mode.** The owner's next bigger idea
   (2026-09-26): help build decks under a mode's rules (star cap, bans, Leaders, Oculus,
   single or dual clan) and rank clans against each other in theory (solver matchups) and in
   practice (captures). Mostly thinking and data gathering first - the mode rules and the
   owner's collection are not captured yet. `docs/deck-building.md` holds the problem
   statement, what exists, what is missing and a layered approach. Do not start it without
   the owner.

## How to resume (read this first in a new session)

1. `deno test -A --no-check tests/replay/` — the score to beat is in the table at the top of
   `docs/replay-triage.md`. The whole project type-checks (`deno task check`), so
   `--no-check` is only a speed-up; keep `deno task check` clean before committing.
2. New games: the owner runs `deno task advise` (or standalone `deno task log` for capture
   without the TUI) and plays; then `deno task extract` and re-run the replay suite. The userscript
   is at 0.7 - if `ur_log.jsonl` starts growing by gigabytes again, Tampermonkey is still
   running an older copy and needs it re-pasted. Failures print the round, both cards and the engine-vs-server diff;
   `captures/games/<id>.json` has the full round (moves, abilities, server results,
   post-round effects). Group new failures by ability keyword before fixing anything.
3. Fix loop: change the engine → replay suite → full `deno test -A --no-check` (only the
   open replay mismatches in the triage table should fail) → `deno task pins:update`
   and read `git diff rust/tests/expect tests/expect`, which is where the derived id sets,
   counts, revisions and provenance fingerprints now live → update the triage doc
   → commit with a subject in the repo's "Add X, Fix Y" style. Never hand-edit a file under
   an `expect/` directory: regenerate it and review the diff, which is also the unlock
   evidence a slice's commit message should quote.
4. Prefer fixes backed by ≥2 captured data points; note single-point hypotheses in the
   triage doc instead of coding them. Every ability rule fixed so far was confirmed by
   arithmetic against the server's power/damage/attack numbers, not by reading rules text.
   The Rust `ClanBonusDiagnostic` gate is fixed at 40 sequential prefix rounds: the
   unchanged 20-round base set plus 20 audited additions. It trusts captured active bonus
   identity for replay preparation only; do not use its source-bonus Support grouping as a
   catalog-only/effective-clan solver rule. Night bonus id 1442 remains deferred.
   The separate Rust `CombatStatDiagnosticV1` gate is 905 curated sequential prefix rounds at
   semantic revision 76 (the count is pinned in `rust/tests/expect/combat_stat_gate_rounds.txt`;
   what follows describes the slices up to revision 37 - revisions 38-76 are written up at the
   end of `docs/rust-migration.md`).
   It admits fixed ordinary combat stats with Always/Courage/Reprisal and numeric
   Symmetry/Asymmetry (immutable original hand-slot equality/inequality), round-scaled
   Growth/Degrowth, selected-opponent-level Equalizer, ordinary unconditional Support
   Attack/Power/Damage abilities, the earlier bonus/control slice, and the exact
   post-round Defeat recovery identities (bonus `577`; abilities `729`/`1418`) and the exact
   Victory-or-Defeat Pillz family (bonus `1034`; abilities `1034/1375/4111/5085/5520`),
   plus Argos' exact `Defeat: +2 Pillz Max. 11` ability (`1158`), the plain ability `+N Pillz`
   grammar paid to a living winner (`337/455/503/1054/1150/1229/2262/2525/4855/5258`) and its
   tight-printed `Confidence: +N Pillz` form (`1702`/`4449`), the plain
   ability `-N Opp Pillz. Min M` grammar taken from the opposing player's post-bet Pillz
   (`334/339/343/360/570/854/3541/5532/5682`) with its losing-side `Defeat: -N Opp. Pillz,
   Min M` sibling (`912`), the clan-gated `4673` staying out of that family,
   Hattori's `Courage: -4 Opp. Dmg, Min 2` (`304`/`961`), which is the opponent-Damage
   reduction the compiler already admitted, printed with the stat abbreviated, `+1 Pillz Per Damage` (`809/1051/1090`) and
   its Symmetry form (`1852`) paid to the winner as its final resolved Damage, `+N Life
   Per Damage` (`141/189/226/492/1125/1224/4500`) with its Revenge (`1661`), Confidence
   (`1810`) and capped `Max. M` (`1146`/`1161`) forms, the cap being Heal's - never past it
   and nothing to an owner already there - the both-sides `Xantiax: -N Life, Min. M`
   reduction (`1379`/`5198`), which names no outcome and no beneficiary and charges both
   players at once, exact structured
   fixed `+N Life` on victory, plain or under the two conditions it prints (`2638`
   `Asymmetry: +3 Life` from the hand-slot field, `814`/`2113`/`3546` `Confidence : +N Life`
   from the previous-round field, abilities only), strict ability-only uncapped `Defeat: +N Life`, and Lobo's
   evidence-backed `Reanimate: +2 Life` (`4951`), exact Komboka Bonus `1714` `+1 Pillz And Life` on Victory,
   the reviewed Victory-or-Defeat Life identities (`1396/2944/2992/5799/5802/5835/1628`),
   exact Equalizer opponent-Life definitions `1415`/`4458`, Anita level 3's exact
   `ability:274` Courage conversion from final resolved damage to Life, the unconditional
   opponent-Life reduction as one printed-text-and-shape grammar on the Victory
   (`512`/`524`/`594`/`602`/`769`/`842`/`935`/`1002`/`1399`/`3491`/`3571`/`3716`/`4948`),
   Victory-or-Defeat (`1386`/`1628`/`1726`/`3367`/`4331`) and `Defeat:` channels, with
   `bonus:680` and `1628` identity-locked to the Bonus slot they also appear in, and
   unconditional Copy resolved against the opposing selected card,
   plus exact unconditional Stop Opp. Ability from Ability,
   Roots, and GHEIST sources and Reprisal aliases `1310`/`2073` using source-dependency ordering shared with Stop Bonus,
   and the four plain permanent Life grammars - `Heal N Max. M`, `Regen N, Max. M`,
   `Poison N, Min M` (abilities and the active Freaks bonus) and `Toxin N, Min M` - on one
   latch: a live source whose card wins writes the effect into its owner's
   `BaseRulesPosition::latched`, and every later round pays it after that owner's own
   current-round effects, in latch order. Heal and Poison pay nothing in the latching round;
   Toxin and Regen pay at once. Heal and Regen pay a living owner below the cap and never past
   it; Poison and Toxin take from the opposing player above the floor and never below it,
   still pay in the round their owner is knocked out, and a Min 0 Toxin can end the match
   (undo restores the latch);
   replay-only dynamic `ability:1034` is never synthesized by catalog construction; global/off-card
   hazards are preparation-fatal, while unadmitted combat-stat, control, and `recover_pillz`
   hazards reject if selected. Ability and bonus Support use independently validated immutable
   effective-clan character counts;
   provenance revision 34 records the current compiler policy. Every other prefixed permanent
   (`Defeat`, `Killshot`, `Perfect`, `Backlash`, `Growth`, `Unison`, Victory-or-Defeat
   and clan-gated forms) and every Pillz permanent stays visible-but-disabled; a plain
   permanent's text in the wrong slot or over a malformed shape, and the complete plain shape
   under other text - which is what `Growth:`, `Unison :` and `Revenge:` Poison are - are
   selected hazards. Recovery ratios, source-id
   variants (including `2475`), every unlisted same-text identity (including Ellie/Lorea's
   Anita-like text), Anita levels 1–2, Bonus/Copy provenance, malformed Anita records, the deferred
   conditional Victory opponent-Life siblings `4533`/`1730` (refused in a compact plan too, so
   neither can be relabelled as the plain grammar), `Night: -2 Opp. Life Min 0` (`4750`), which
   prints the complete Victory shape under text the grammar does not name and so rejects when
   selected, every conditional or stat-copying Copy variant, generic capped increases,
   a capped Life conversion under a previous-round prefix (never printed, so its text would be a guess), and controls beyond unconditional Stop Bonus/Stop Opp. Ability and the two exact Reprisal aliases remain fail-closed.
   Revision 22 adds the reviewed conditional Victory opponent-Life identities (Symmetry
   `4708`, Confidence `3016`/`4301`) and Reprisal/Revenge `Copy`, both by putting an
   already-resolved predicate on plans the engine was always able to gate. Courage `4533`
   (no selected observation in the corpus) and Growth `1730` (a round-scaled magnitude, not
   a predicate) stay deferred, as do Asymmetry/Unison and stat-copying Copy.
5. The owner has authorized tested `main` pushes. Never force-push, and never commit
   `ur_log*.jsonl`, `tokens.json`, `.env`, `data/site_characters.jsonl`.
2. Card data was complete as of 2026-09-10 (2496 cards, every level, 36 clans incl. the new
   Tolvack). To refresh: `__ur.dumpCharacters()` and `__ur.dumpClans()` in the browser (log
   server running), then `deno task cards`. A new clan must also be added to `Clans` in
   `src/game/types/CardTypes.ts`. Cards that became collectors got a " Cr" suffix; the loader
   keeps the old name as an alias. Card 2714, dealt in capture 1414087 on 2026-09-23, is
   newer than that dump, so a refresh is due.
3. Longer term: replace regex ability parsing with the structured `abilityData` the battle
   API returns per card (see any `captures/games/*.json`; the site card DB dump does NOT
   include it), and build a "what do players play" dataset from captured moves and timings.
4. Engine bugs are addressed only when a replay exposes them. The two legacy tests that
   used to fail were repaired on 2026-09-25 without an engine change (see "Legacy tests" in
   `docs/replay-triage.md`).

## Rust revival

- The TypeScript engine, captures and current advisor remain the operational reference.
  Rust is a candidate backend, not a second source of game rules.
- Follow `docs/rust-migration.md`: canonical `(card id, level)` data and the versioned replay
  adapter come before engine parity; engine parity comes before porting current solver policy.
- `rust/src/engine/` keeps base rules and projected effect models separate. The 20-round
  base gate proves replay/combat plumbing, the 40-round clan diagnostic executes its bounded
  bonus slice, and the combat-stat diagnostic (905 rounds at revision 76) adds fixed ordinary abilities,
  numeric hand-slot predicates, round-scaled magnitudes, combat-stat Equalizer, exact
  Equalizer opponent-Life, Anita's identity-locked final-damage Courage Life conversion,
  unconditional and reviewed conditional Victory opponent-Life, unconditional, Reprisal/Revenge and Asymmetry source Copy, unconditional stat Copy, the three reviewed Protection grammars, Attack per opposing Damage, Defeat opponent-Life, the four latched plain permanent Life grammars (plain or under one Symmetry/Asymmetry/Revenge/Confidence prefix), plain own and opposing Victory Pillz, Pillz per Damage, Life per Damage capped and uncapped, and ordinary numeric Support abilities without claiming general condition or full-effect parity.
- `EffectiveCardCatalog` is required for solver-facing construction and validates
  `data/battle_card_overrides.json` before exposing rows. `CatalogCombatStatMatchV1` takes
  exact `(card id, level)` keys, derives canonical versus effective clans without mutation,
  resolves active day/night descriptions through the effect registry, and rejects duplicate
  characters, Leaders, a Copy with an opposing source it cannot adopt, and every other
  source outside the bounded projection (the Oblivion clan-bonus Copy is bridged since
  revision 68, catalog-context revision 4, and since revision 69, catalog-context revision 5,
  a selected night variant, which has no catalog id, reaches the post-round grammars that
  print a `Night:` form by its exact text, in a night match only).
  Its provenance includes exact effective-catalog and registry source fingerprints plus
  compiler and catalog-context policy revisions. It prepares trustworthy match inputs; only
  the exact conservative late policy has been ported, not the complete TypeScript solver.
- `rust/src/effect_registry.rs` strictly parses and classifies `captures/abilities.json`.
  Supported compiler output is a string-free future execution plan; no effect is implemented
  until an engine slice executes it and replay evidence establishes its behavior. Never treat
  `Unsupported` as a no-op or infer Team/Day/Night semantics from otherwise identical data.
- Keep structural cleanup separate from behavior changes. The ignored 10,000-case Rust
  diagnostic preserves historical behavior, while `captures/games/` is the server-backed
  correctness oracle.
- Do not revive the historical perfect-information advisor as the live recommendation
  model. The current hidden-information `Search`/`Policy` behavior must be ported explicitly.
- Round one has two evaluators. The default is still the one-round position heuristic with
  the fixed 198-play reply prior, in both implementations. `--exact-opening` asks Rust to
  solve the opening with the ordinary continuation policy instead: complete, 0.2-0.5 s for
  SECOND and 0.2-2.1 s for FIRST on one thread and about half that on six, because the
  policy remembers solved positions (it was 8.5-24 s for FIRST on one thread before
  2026-09-26; see "The exact opening" in `docs/rust-migration.md`). Weighting replies by the captured prior is a
  property of round one and survives that switch; only leaf scoring changes. Never read
  `openingEstimate` as "this is round one" - use `openingPrior` (TS) or
  `weights_by_opening_prior` (Rust), because conflating them is a live bug three times over.
  An exact opening cannot be compared against the TypeScript heuristic; the reference
  `Search` `exactOpening` mode and `tests/solver/ExactOpeningParity.test.ts` exist for that.
- `rust/src/advisor/` is the current-engine vertical slice: strict manual/demo/replay input, a
  complete current-round make/unmake matrix, deadline-safe partial results, a bounded
  terminal view, a manual four-round session, and server-backed grading of every decision in
  supported captures `877636`, `877812`, `925674`, `925719`, `1024673`, `1060199`,
  `1061897`, `1069813`, `1081463`, and `1089346`.
  Replay mode verifies complete card and resource results before
  advancing. Round 1 uses a labelled, fixed 198-play historical opening prior; rounds 2–4
  use an exact conservative policy whose reply may depend on a visible opposing card but not
  hidden pillz/Fury. In manual rounds 2-4 where the opponent moves first, a
  blind-second pass groups every unplayed opponent card and hidden wager beneath each fixed
  reply, then the revealed card replaces it with precise SECOND advice. The TypeScript host
  can launch the precompiled `rust:worker` through a strict one-request JSONL V3 boundary for
  supported FIRST, SECOND, and blind-second decisions: `--rust=compare` retains TS authority,
  while `--rust=use` accepts only structurally validated complete Rust results and otherwise
  falls back to TS. Default is off; data fingerprints, engine/catalog/policy revisions,
  replayed history, information-set identity,
  the legal action matrix, bounded SECOND wager outcomes, and response bounds are strictly
  checked without claiming per-request semantic equivalence. Do not present this as refreshed
  opening data or complete engine parity.
- Keep the JSONL process boundary owned by the TypeScript advisor; reconsider in-process FFI
  only after correctness and protocol stability.

## Solver notes (reviewed Sept 2026)

- **Ranking prefers a knockout, then safety, on ties.** `Search.ranked()` orders by win
  chance *as displayed* (rounded to a whole percent), then by the share of sampled lines
  that end the game outright this round, then by the share that end *you* on the spot
  (lower first), then by the cheaper bet. Rounding the sort key matters: sorting on the raw
  mean produced orders the screen could not explain - two bets both reading 90% with the
  riskier one above, over a difference of half a point that is well inside the error of a
  model assuming a uniformly random opponent. Equal on screen now means equal in the sort,
  so every tie is decided by something visible. Win chance still dominates, and ending it now beats
  winning on life at the end of round 4, because a knockout stops depending on the
  evaluation being right - everything below depth 2 assumes an opponent who can see your
  pillz. `Minimax.best()` breaks these ties on cost alone, so `Search.best()` and
  `Minimax.best()` can now pick different moves of equal value; `tests/solver/` pins the
  value rather than the move for that reason.

- **The terminal view has two hard rules** (`SolverView.ts`), both learned from a real
  game on an 80-column terminal where the matrix was 116 wide and the frame overprinted
  itself into duplicated letters and misaligned tables. Nothing may wrap or scroll:
  `frame()` clips every line to the real width and drops any past the last row, and the
  matrix sizes its columns to fit before drawing. And colours come from the terminal's own
  sixteen, never the 256-colour cube, so it sits in the user's theme.
  `tests/solver/View.test.ts` pins both and fails if the clamp is removed.

  Related: `Debug.ts` asks `Deno.permissions.querySync` before reading `UR_DEBUG`. A bare
  `Deno.env.get` on an ungranted variable makes Deno *prompt*, and a permission prompt
  drawn over the advisor's alt-screen is unanswerable and corrupts the frame.

- **Shape.** `iterTree` is breadth-first over the whole tree, so no answer exists until it
  finishes: ~5.2M states and ~17s from round 2 with one card down (`deno task time`; it was
  ~40s before the performance work below).
  `Search.ts` evaluates one depth-2 state at a time, which is what makes a live ranking,
  cancellation and a bounded memory footprint possible. It takes a `stride`/`offset` so
  the units can be split across workers (also tested). Its original perfect-information
  evaluator reproduced `iterTree` + `best()` exactly; the live information-aware policy
  now deliberately differs when the reference relies on seeing a hidden bet. A
  worker pool splits those units across processes from round 2 onward. Round 1 no longer
  recursively solves the remaining game: the advisor evaluates all 92 × 92 current-round
  pairings with a life/pillz/remaining-card position heuristic on one worker (~80ms in the
  reference opening). Opponent replies are weighted by the 198 captured round-one plays
  (plus Laplace smoothing), and remaining pillz use a nonlinear reserve term so implausible
  all-ins do not dominate. The view labels its weighted average and floor-to-ceiling range
  as opening scores rather than win percentages, then returns to exact search in round 2.
  When the opponent moves first in rounds 3–4,
  a blind-second search also ranks our replies across every card and hidden bet they might
  choose; their actual card replaces it with the ordinary precise SECOND-mode search.
- **What the percentage means.** From round 2 onward, each root move is averaged over the
  opponent's possible current replies (uniformly; this is still a model rather than an
  empirical probability). Every later continuation is evaluated by `Policy.ts` as a
  conservative pure policy that can actually be followed with the information visible in
  the live game. When we move second, one reply may depend on the revealed opposing card,
  but never on its hidden pillz/Fury; that hidden bet is adversarial. When we move first,
  our move must survive the worst opposing reply. This deliberately differs from the old
  perfect-information recursion, which produced a false 100% in capture 1065812 by choosing
  a different future Lothar bet for each hidden Scott bet. The visible Worst column is the
  extremum over the opponent's current choice. Exact endpoints are reserved for exact
  results: rounded near-wins/near-losses display as 99%/1%, not 100%/0%. A mixed-strategy
  equilibrium would be less conservative, but remains a separate future solver model.
- **Reference pruning.** `Deep.ts` retains two rules from the original solver, both only on
  terminal children: a cutoff when the mover finds
  a win (sound), and a domination rule — if the all-in bet and the biggest fury bet both
  lose, skip the rest of that card's bets, since every other bet is dominated in attack *and*
  damage by one of those two. That argument assumes more attack/damage is never worse for
  the mover, which **Backlash** and **Defeat** abilities violate. It can also truncate the
  set of replies that then gets averaged, though only when solving round 4. `Policy.ts`
  does not use that domination rule; it only stops once an exact best/worst terminal result
  means the remaining actions cannot change the value.
- **Fixed here.** A double KO left `winner` on `PLAYING`, so the solver treated a finished
  game as live, walked `id` off the end of its half of the turn-order table, and produced
  leaves with neither result nor children — `Node.rating()`'s `Infinity` sentinel,
  inherited by every MAX ancestor (one position went from 825,715 poisoned nodes to 199 clean ones).
  `Ability.clone()` shared any permanent whose `delayed` was unset (Toxin, Consume, Regen,
  Dope, Repair, Mindwipe), so the first branch that won with one latched it for the whole
  tree. `Node.toString()` scaled `[-1, 1]` as if it were `[0, 100]`, printing a draw as
  `[Loss]` and a P2 win as `[Win -100%]`.
- **Fixed 2026-09-25: nothing engine-wide is process-global any more.** An Oculus
  infiltration was written onto the shared (id, level) base row, so it outlived its game
  and an opposing copy of the same card overwrote it; it is now a per-instance
  `Card.infiltration` that `Hand.from` recomputes (`tests/ability/Oculus.test.ts`). The
  compiled `CachedCardBattle` table and the turn-order (`base`) table were module globals
  filled by the last `Game` built, so a second live `Game` played the first one's battles
  with its own cards; both are now symbol-keyed per-match fields that `clone()` shares by
  reference, a structured clone skips and `Game.from` rebuilds, and the battle lookup is
  an index rather than a string key (`tests/GameIsolation.test.ts`). Any number of Games
  may be alive at once. The advisor still drops a search before building the next `Game`,
  which is now tidiness rather than a requirement.

## Performance (measured Sept 2026)

`deno task time` went from **40.0s to 17.0s** (2.36x) on the round-2 bench, same tree
explored (2,303,378 states at the last ply, unchanged), and to about 10 s after the
three 2026-09-25 fixes below (stat views, `length = 0`, empty event times), which took
`deno task time-search` from 1.69 s to 0.47 s. Both of the first changes came from profiling,
not from reading the code - the first guess was wrong by 20x, so measure before believing
anything below. `deno bench -A --no-check tests/CardAccess.bench.ts` holds the micro
numbers; a full profile is `--v8-flags=--prof,--logfile=<path>` then `node --prof-process`.

- **The packing is not the problem, and never was.** One object per card holding two SMIs
  is what makes `clone()` cheap enough to hold millions of nodes; cloning measured
  identically (1.01x) whichever way the stat views are built. What cost was
  `Object.setPrototypeOf` as the *dispatch*: it cannot be inlined, it moves a finished
  object onto a new map through the runtime, and it drove the `.final` sites megamorphic.
  Replacing it with a throwaway view object per access was worth **10.8%**, but the view
  was *not* free until 2026-09-25: escape analysis dropped it only in the bench's own
  standalone classes (the bench's baseline had silently been measuring the engine's views).
  The engine's seven views inherited from an abstract `BaseAttr -> BaseStat` chain whose
  constructor stored `d`, and on V8 15 that kept a real allocation per access twice over:
  every `super()` went through the `FindNonDefaultConstructorOrConstruct` builtin, which
  TurboFan did not inline, and the one shared `d` store saw seven maps, past what a
  polymorphic IC holds. Making each view a standalone class, with the shared shape as
  type-only interfaces, took **`deno task time-search` about 24% and `deno task time`
  about 17% faster** (alternating runs, identical checksums and state counts), and
  removed those builtins from the profile. Do not give the stat views a runtime base
  class again.
- **The same call in the clone path was worth far more.** A profile put
  `ObjectSetPrototypeOf` at **29% of the whole search** (665 of 2309 ticks), 57% of that
  from `Game.clone` and 32% from `Hand.clone`, both of which built a literal and then
  re-prototyped it. `Object.create` + assignment in declaration order is ~1600x cheaper for
  an object and ~345x for an array (`Hand.of` is worse than either). That change alone took
  35.7s to 17.0s. The builtin no longer appears in the profile at all.
- **Make/unmake killed the allocation bound.** After the two fixes above the search was
  allocation-bound - GC 27% of ticks, `Game.clone` 15% of JS - because `iterTree` clones a
  game per node (~23 objects) to keep its breadth-first frontier. `Game.make`/`unmake` plus
  the depth-first evaluators in `Deep.ts` and `Policy.ts` mutate one game and walk it back, which
  costs no allocation at all: the mutable state is already packed (a Player and a
  PlayerRound are one int each) and the two cards a battle touches are *replaced* in the
  hand rather than edited, so undoing them is restoring two references. The original
  perfect-information `Search` used `Deep.ts` instead of `iterTree(game, false)` per unit:
  **`deno task time-search` went 12.0s ->
  7.9s** (1.53x) with an identical result checksum, and **GC fell from 27% of ticks to
  1%**. The note here then read the 122 of 152 shared-library ticks under `Events.execute`
  as battle resolution itself and called the next lever algorithmic. They were not: see
  the next bullet.
- **`arr.length = 0` was half the live search (fixed 2026-09-25).** An array's `length` is
  an accessor with a C++ setter, so assigning it cannot be inlined: optimised code goes
  StoreIC -> CEntry -> `JSArray::SetLength` on every call, even on an empty array, and
  right-trims the backing store for the next `push` to regrow. `Events` cleared twenty
  buckets per battle that way, most of them empty; a profile put 30% of ticks in
  unsymbolised `deno.exe` C++ under `Events.execute`/`executeCancels` plus 10% in the
  StoreIC builtins. A `pop()` loop, which TurboFan inlines and which keeps the capacity,
  and skipping the Min-clamp sort below two abilities took `deno task time-search` from
  1.27 s to 0.64 s and `deno task time` from about 13.8 s to 11.5 s, identical output. A
  large `deno.exe` share in a `--prof` Summary means C++ runtime calls (GC is counted
  separately): look for accessor stores such as `length`, `delete`, or megamorphic keyed
  access in the caller. Do not reintroduce `length = 0` in engine code.
- **Empty event times are skipped (2026-09-25).** A battle fills two or three of its
  twenty event times, yet `CardBattle` called `Events.execute` for all twenty, too big to
  inline at twenty sites, so each empty time was a real call doing nothing. `Events.mask`
  has bit t set whenever `events[t]` or `repeat[t]` might hold an ability; `CardBattle`
  tests it before each call and `CachedEvents.merge` walks only the times it filled. The
  invariant to keep: **anything that pushes into a bucket must set its bit** (today only
  `add`, `addGlobal` and `merge` push). A stale bit costs one no-op `execute`; `Undo`
  saves and restores the mask so make/unmake stays exact. In the same change a `Card`
  keeps its base row under a symbol key: `baseCards` is a dictionary-mode object (keys up
  to 2^19), and TurboFan never inlines a dictionary keyed load, so every clan/name/star
  read was a generic KeyedLoadIC. Do not add hot-path lookups into integer-keyed plain
  objects. Together: `deno task time-search` 0.645 s -> 0.47 s, `deno task time` about
  10-20% faster, identical output. The profile left is spread thin: `Policy`'s loops
  about 16%, make/unmake bookkeeping about 10%, `shiftRange` and its generator 3.5%, the
  PRE1/POST2 sort 2%.
- **Rust: the exact policy remembers positions (2026-09-26).** A single-threaded profile
  of the Rust exact opening (an in-process sampler; samply needs an elevated ETW session
  here) found zero allocations per `make`, 9% in `Instant::now()` per `make`, and `make`
  itself flat. The lever was the node count: 85-97% of the positions entering rounds 3 and
  4 were repeats (a Fury bet of p and a plain bet of p + 3 leave the same pillz).
  `PolicyControl` caches each completed continuation value by (`BaseRulesPosition`, us,
  first mover), which is exact because `CombatStatDiagnosticV1` is its spec plus that
  position and the recursion prunes only on exact endpoints; the cache is dropped when the
  match spec changes and never stores a deadline-cut value. The clock is read every 64th
  `make`. Demo exact opening FIRST 8.5 s -> 0.19 s, worst capture 24.4 s -> 2.1 s, results
  bit-identical. Each worker owns its cache, so six threads now buy about 2x, not 4.3x,
  and peak memory is up to about 40 MB per worker. Keep everything a round can change in
  `BaseRulesPosition`: state kept anywhere else would make this cache silently wrong.

  `iterTree` and `Deep.ts` are deliberately left as-is as the perfect-information reference
  (`tests/solver/DeepEquivalence.test.ts` checks `deepValue` returns the same number *and*
  leaves the game byte-identical, on hands chosen for latching permanents and Backlash).
  `deno task time` still measures it; `deno task time-search` measures the current live path.
- **Engine tracing is behind `DEBUG`** (`src/utils/Debug.ts`): 39 sites across `Ability`,
  `Condition`, `CardBattle` and the modifiers read `if (DEBUG) console.log(...)`, so the
  strings and their ANSI colours are never built when it is off. `UR_DEBUG=1` turns it back
  on, which `deno task run` does, so playing by hand narrates itself exactly as before
  (verified byte-identical, 265 lines either way). The user-facing output in `Game.ts` -
  prompts, game-over banners - is not guarded.

  Worth knowing why it is *not* justified on speed any more: building those strings
  measured at 5.9% **before** the clone-path fix (35.7s -> 33.6s), but re-measured after it
  over 6 runs a side it is 17.2s vs 17.4s - about 1%, inside the noise. The clone fix
  absorbed it. The likely reason is that the cost was mostly marginal GC pressure, which
  stopped mattering once total allocation roughly halved, though that is a hypothesis and
  was not measured directly. Kept anyway: it is free, it matches the rule below about not
  allocating in `CardBattle` / `Events` / modifiers, and it stops the tracing flooding any
  caller that has not stubbed `console.log`.

## Conventions

- Deno, no Node build step. Imports via `@/` (src) and `@data/` aliases in `deno.json`.
- Engine code is performance-sensitive (solver explores millions of states): avoid logging
  or allocation in `CardBattle`, `Events`, modifiers. Debug `console.log`s are removed
  before commit.
- `.gitattributes` checks out `captures/abilities.json`, `data/data.json` and
  `data/battle_card_overrides.json` as LF everywhere, because the Rust provenance
  fingerprints hash them byte for byte and `tests/expect/rust-provenance.json` pins the
  result. Keep them LF, or the pin and every fresh checkout disagree.
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
