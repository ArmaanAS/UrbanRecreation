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
| `rust/` | Imported Rust implementation and candidate high-performance backend. `engine::BaseRulesGame` remains the effects-disabled 20-round reference; `engine::ClanBonusDiagnostic` preserves its separate 40-round projected gate; `engine::CombatStatDiagnosticV1` adds a distinct fail-closed combat-stat projection with a 177-round gate. Semantic revision 25 retains the bounded numeric and post-round slices, adds exact unconditional Stop Opp. Ability with allocation-free TypeScript-compatible PRE4 dependency resolution, captured Reprisal SOA aliases `1310`/`2073` with `OwnerMovesSecond`, exact Komboka Bonus `1714` `+1 Pillz And Life`, the reviewed Victory-or-Defeat Life identities (`+1`/`+2` own Life and `-1` opponent Life Min 1), exact Equalizer opponent-Life definitions `1415`/`4458`, Anita level 3's exact Ability `274` Courage conversion from final resolved damage to Life, the two unconditional Victory opponent-Life reductions (Mou's `ability:1399` and the active Berzerk `bonus:680`), the reviewed conditional ones (Doela Noel's Symmetry `4708` and Diabolus' Confidence `3016`/`4301`), and `Copy: Opp. Ability`/`Copy: Opp. Bonus` both unconditionally and under `Reprisal:`/`Revenge:`, which adopt the opposing selected card's plan in the copier's own slot and Support context while the adopted plan keeps its own predicate, the three reviewed Protection grammars (`Protection: Power And Damage`, `Protection: Ability` and `Protection: Bonus`), which refuse an opposing reduction and keep a stopped source alive, the three unconditional stat Copy grammars (`Copy: Opp. Power`, `Copy: Opp. Damage` and `Copy: Power And Damage Opp.`), which take the opposing card's printed value before every modifier, `Asymmetry:` source Copy, `+N Attack Per Opp. Damage`, whose magnitude is the opposing card's resolved Damage before Fury, and the losing-side `Defeat: -N Opp. Life, Min M` reduction, alongside strict ability-only ordinary Defeat Life and Lobo's evidence-backed Reanimate identity; unobserved same-text/source variants, Anita levels 1–2, Bonus/Copy provenance, other conditional SOA, the other Protection grammars (`Protection: Power`, `Protection: Attack`, the spaced `Protection : Damage` and its clan-conditional form), Unison and conditional stat-copying Copy, Power/Damage Exchange, Courage `4533` and Growth `1730` opponent-Life, dynamic catalog Copy, capped increases, compound Life/Pillz, and other Reanimate identities remain fail-closed. `engine::CatalogCombatStatMatchV1` is the strict catalog-only constructor for search positions: it applies reviewed runtime card overrides, derives immutable Oculus/effective-clan and day/night context, and rejects a draw unless all eight cards are executable by that projection. `advisor/` contains a current-engine make/unmake search, bounded TUI, manual four-round mode, a fixed 198-play historical opening prior, exact conservative rounds 2–4 policy, a single-thread manual blind-second pass, and strict captured-replay grading. The precompiled one-request JSONL V3 worker lets the TypeScript host use supported FIRST, SECOND, and blind-second decisions after validating data fingerprints, compiler/catalog/policy semantic revisions, replayed history, information-set identity, legal actions, bounded SECOND wager outcomes, and response bounds. Complete captures `877636`, `877812`, `925674`, `925719`, `1024673`, `1060199`, `1061897`, `1069813`, `1081463`, and `1089346` are current end-to-end gates, not full live-policy parity. The historical engine remains beside them, and the old HTTP advisor is behind `legacy-advisor`. New Rust work reads the root canonical data and captures—`rust/assets/` is historical only. See `docs/rust-migration.md`. |
| `ur-logger.user.js` + `log_server.ts` | Tampermonkey userscript mirroring site traffic to a local server that writes `ur_log.jsonl` (raw, **contains tokens, gitignored**) and secret-free per-battle files in `captures/battles/`. Binary response bodies (WebGL asset bundles, images, wasm) are dropped on both sides: `res.text()` decodes them as lossy UTF-8, so they are unrecoverable garbage, and unfiltered they were 65% of the first real log. `scripts/PruneLog.ts` retro-fixes older logs. |
| `captures/` | `battles/<id>.jsonl` raw battle capture in a compact lossless form (static block once + one dynamic line per `battles.status` poll, ~20 KB/battle instead of ~450 KB; `expandStatus()` in `scripts/BattleCapture.ts` rebuilds the original server objects); `abilities.json` shared ability/bonus dictionary (id → description + structured `abilityData`); `games/<id>.json` clean game records with decks, moves, per-round resolution, life/pillz, and an engine `testcase`. All safe to commit. |

## Common commands

```bash
deno test -A --no-check          # run tests (type-check currently fails on src/utils/Utils.ts:184)
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
   As of 2026-09-17: **359 battles captured, 352 replay-ready, 304 replay exactly** (life,
   pillz, power, damage, attack, winner per round), **48 mismatch**, and 7 incomplete/Dojo
   captures ignored. Four open cases have already been investigated (874590, 874712,
   901004, 1093173); the other 44 are fresh regression targets from the expanded corpus and
   remain untriaged. This is fresh ground truth rather than evidence that earlier working
   replays regressed.
   The corpus grew by 31 on 2026-09-17 because commit `b7a56d1` had archived 29 battle
   captures without ever extracting them; `deno task extract` is byte-identical for every
   game already committed, so run it before trusting a count here.
   `docs/replay-triage.md` tracks what
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
2. New games: the owner runs `deno task advise` (or standalone `deno task log` for capture
   without the TUI) and plays; then `deno task extract` and re-run the replay suite. The userscript
   is at 0.7 - if `ur_log.jsonl` starts growing by gigabytes again, Tampermonkey is still
   running an older copy and needs it re-pasted. Failures print the round, both cards and the engine-vs-server diff;
   `captures/games/<id>.json` has the full round (moves, abilities, server results,
   post-round effects). Group new failures by ability keyword before fixing anything.
3. Fix loop: change the engine → replay suite → full `deno test -A --no-check` (two legacy
   failures are expected: `Game_2 Protection`, `Oculus Infiltrated`) → update the triage doc
   → commit with a subject in the repo's "Add X, Fix Y" style.
4. Prefer fixes backed by ≥2 captured data points; note single-point hypotheses in the
   triage doc instead of coding them. Every ability rule fixed so far was confirmed by
   arithmetic against the server's power/damage/attack numbers, not by reading rules text.
   The Rust `ClanBonusDiagnostic` gate is fixed at 40 sequential prefix rounds: the
   unchanged 20-round base set plus 20 audited additions. It trusts captured active bonus
   identity for replay preparation only; do not use its source-bonus Support grouping as a
   catalog-only/effective-clan solver rule. Night bonus id 1442 remains deferred.
   The separate Rust `CombatStatDiagnosticV1` gate is fixed at 177 unique sequential prefix rounds.
   It admits fixed ordinary combat stats with Always/Courage/Reprisal and numeric
   Symmetry/Asymmetry (immutable original hand-slot equality/inequality), round-scaled
   Growth/Degrowth, selected-opponent-level Equalizer, ordinary unconditional Support
   Attack/Power/Damage abilities, the earlier bonus/control slice, and the exact
   post-round Defeat recovery identities (bonus `577`; abilities `729`/`1418`) and the exact
   Victory-or-Defeat Pillz family (bonus `1034`; abilities `1034/1375/4111/5085/5520`),
   plus Argos' exact `Defeat: +2 Pillz Max. 11` ability (`1158`), exact structured
   fixed `+N Life` on victory, strict ability-only uncapped `Defeat: +N Life`, and Lobo's
   evidence-backed `Reanimate: +2 Life` (`4951`), exact Komboka Bonus `1714` `+1 Pillz And Life` on Victory,
   the reviewed Victory-or-Defeat Life identities (`1396/2944/2992/5799/5802/5835/1628`),
   exact Equalizer opponent-Life definitions `1415`/`4458`, Anita level 3's exact
   `ability:274` Courage conversion from final resolved damage to Life, and the two
   unconditional Victory opponent-Life reductions `ability:1399`/`bonus:680`, and
   unconditional Copy resolved against the opposing selected card,
   plus exact unconditional Stop Opp. Ability from Ability,
   Roots, and GHEIST sources and Reprisal aliases `1310`/`2073` using source-dependency ordering shared with Stop Bonus;
   replay-only dynamic `ability:1034` is never synthesized by catalog construction; global/off-card
   hazards are preparation-fatal, while unadmitted combat-stat, control, and `recover_pillz`
   hazards reject if selected. Ability and bonus Support use independently validated immutable
   effective-clan character counts;
   provenance revision 25 records the current compiler policy. Recovery ratios, source-id
   variants (including `2475`), every unlisted same-text identity (including Ellie/Lorea's
   Anita-like text), Anita levels 1–2, Bonus/Copy provenance, malformed Anita records, the conditional
   Victory opponent-Life siblings `4708`/`4533`/`3016`/`1730`, every conditional or
   stat-copying Copy variant, generic capped increases,
   and controls beyond unconditional Stop Bonus/Stop Opp. Ability and the two exact Reprisal aliases remain fail-closed.
   Revision 22 adds the reviewed conditional Victory opponent-Life identities (Symmetry
   `4708`, Confidence `3016`/`4301`) and Reprisal/Revenge `Copy`, both by putting an
   already-resolved predicate on plans the engine was always able to gate. Courage `4533`
   (no selected observation in the corpus) and Growth `1730` (a round-scaled magnitude, not
   a predicate) stay deferred, as do Asymmetry/Unison and stat-copying Copy.
5. The owner has authorized tested `main` pushes. Never force-push, and never commit
   `ur_log*.jsonl`, `tokens.json`, `.env`, `data/site_characters.jsonl`.
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

## Rust revival

- The TypeScript engine, captures and current advisor remain the operational reference.
  Rust is a candidate backend, not a second source of game rules.
- Follow `docs/rust-migration.md`: canonical `(card id, level)` data and the versioned replay
  adapter come before engine parity; engine parity comes before porting current solver policy.
- `rust/src/engine/` keeps base rules and projected effect models separate. The 20-round
  base gate proves replay/combat plumbing, the 40-round clan diagnostic executes its bounded
  bonus slice, and the 177-round combat-stat diagnostic adds fixed ordinary abilities,
  numeric hand-slot predicates, round-scaled magnitudes, combat-stat Equalizer, exact
  Equalizer opponent-Life, Anita's identity-locked final-damage Courage Life conversion,
  unconditional and reviewed conditional Victory opponent-Life, unconditional, Reprisal/Revenge and Asymmetry source Copy, unconditional stat Copy, the three reviewed Protection grammars, Attack per opposing Damage, Defeat opponent-Life, and ordinary numeric Support abilities without claiming general condition or full-effect parity.
- `EffectiveCardCatalog` is required for solver-facing construction and validates
  `data/battle_card_overrides.json` before exposing rows. `CatalogCombatStatMatchV1` takes
  exact `(card id, level)` keys, derives canonical versus effective clans without mutation,
  resolves active day/night descriptions through the effect registry, and rejects duplicate
  characters, Leaders, dynamic Copy, and every other source outside the bounded projection.
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
  solve the opening with the ordinary continuation policy instead: 2-6 s for SECOND, 6-30 s
  for FIRST, single-threaded and complete. Weighting replies by the captured prior is a
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
  hidden pillz/Fury. In manual rounds 2-4 where the opponent moves first, a single-thread
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
  game as live, walked `id` off the end of its half of `baseGames`, and produced leaves with
  neither result nor children — `Node.rating()`'s `Infinity` sentinel, inherited by every
  MAX ancestor (one position went from 825,715 poisoned nodes to 199 clean ones).
  `Ability.clone()` shared any permanent whose `delayed` was unset (Toxin, Consume, Regen,
  Dope, Repair, Mindwipe), so the first branch that won with one latched it for the whole
  tree. `Node.toString()` scaled `[-1, 1]` as if it were `[0, 100]`, printing a draw as
  `[Loss]` and a P2 win as `[Win -100%]`.
- **Still open.** `Card`'s `clan` / `bonusString` setters write to the process-global base
  row, so an Oculus infiltration outlives its game. `Game.createBattleDataCache` keys a
  process-global cache by card-index pair, so only one `Game` may be alive at a time in a
  process.

## Performance (measured Sept 2026)

`deno task time` went from **40.0s to 17.0s** (2.36x) on the round-2 bench, same tree
explored (2,303,378 states at the last ply, unchanged). Both changes came from profiling,
not from reading the code - the first guess was wrong by 20x, so measure before believing
anything below. `deno bench -A --no-check tests/CardAccess.bench.ts` holds the micro
numbers; a full profile is `--v8-flags=--prof,--logfile=<path>` then `node --prof-process`.

- **The packing is not the problem, and never was.** One object per card holding two SMIs
  is what makes `clone()` cheap enough to hold millions of nodes; cloning measured
  identically (1.01x) whichever way the stat views are built. What cost was
  `Object.setPrototypeOf` as the *dispatch*: it cannot be inlined, it moves a finished
  object onto a new map through the runtime, and it drove the `.final` sites megamorphic.
  Replacing it with a throwaway view object per access was worth **10.8%** - the view
  itself is free, since one view class per site keeps the site monomorphic and escape
  analysis then drops the allocation.
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
  1%**. What is left is battle resolution itself - `Events.execute` is 122 of 152
  shared-library ticks - so the next lever would be algorithmic (transpositions, better
  pruning), not allocation. `shiftRange` at 1.7% and `unplayedCardIndexes` at 0.3% are not
  worth chasing.

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
