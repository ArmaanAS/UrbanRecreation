# Advisor backlog

## Make early searches useful (implemented 2026-09-17)

- SECOND-mode hidden wagers are sampled read-panel defaults first (plain all-in,
  zero, Fury all-in), then middle-out: each of the plain and Fury families is
  walked by repeated bisection and the two are interleaved. Enumeration order
  used to continue outwards from those three extremes, so for most of a
  round-one search the running average stood on them, while the wagers in
  between carry most of the captured opening prior's weight.
- The reordering is confined to `Search`'s SECOND mode. `legalMoves` keeps the
  probe order `Analysis.iterTree` and the Rust `legal_moves` mirror, and the
  Rust worker orders its own hidden-wager column independently, so no parity
  gate compares the two orders - only completed results.
- Pinned in `tests/solver/Search.test.ts`: the panel's three hypotheses stay
  first, the next four alternate plain and Fury from each family's midpoint, and
  nothing is dropped or duplicated.

## Worker pool (implemented 2026-09-11)

- The advisor defaults to three Web Workers; `--workers N` overrides it and
  `--workers 1` keeps the old main-thread path. Obsolete searches are terminated
  when the battle moves on.
- `ParallelSearch` uses the existing `stride` / `offset` partition and merges
  incremental samples. Since 2026-09-26 the partition deals whole card pairs
  rather than single units, so each worker's continuation cache sees the
  positions its pairs repeat (see `Search.ts`). The integration test pins
  average, minimax, KO, risk and sample counts against a completed single-thread
  search.
- On this six-logical-CPU machine, repeated round-two timings were approximately
  8.4-9.6s (one), 4.8-6.1s (two), 3.2-4.0s (three), 2.6-3.4s (four), and
  2.2-4.2s (five). Three is the default balance: roughly 2.5x throughput while
  leaving three logical CPUs to the game, browser and capture server. Four/five
  remain opt-in when latency matters more than load.

## Optional final-card move sender

- Investigate only as an explicit opt-in experiment; do not combine it with Auto
  Queue.
- Narrow initial scope: round 4, exactly one card remains, at most three pillz,
  advisor has a settled legal recommendation, and battle / side / turn identity
  all agree.
- The request is already in every capture, as a `play` entry:
  `{"requests": [{"call": "battles.play", "params": {"id": <battle>, "characterInBattleID":
  <1-8, the absolute seat slot>, "pillz": <bet, free Pillz excluded>, "fury": <bool>}}]}`.
  `captures/battles/1207064.jsonl` has one per move. The capture records no
  authentication: the site's session carries it, which is why any sender has to
  live in the userscript.
- Nothing beyond that documentation should be built without the owner asking for
  it explicitly, and only after checking the game's automation rules.
- Send from the browser userscript, which owns the authenticated session. Never
  copy tokens from `ur_log.jsonl` into the advisor or another persisted file.
- Make submission one-shot and idempotent per battle, visibly armed in the TUI,
  and default it to OFF on every restart. Check the game's automation rules
  before live use.

## Future experiment: scriptc

- Revisit [Vercel Labs scriptc](https://github.com/vercel-labs/scriptc) only after
  Rust replay parity has settled. Start with `scriptc coverage` on an isolated,
  Node-compatible, single-worker `Search` / `Policy` / `Deep` harness; keep Deno
  APIs, workers, capture code and terminal rendering outside the experiment.
- Require the hot search graph to be fully static and build with
  `--backend llvm`. A normal static build contains no JavaScript engine, whereas
  `--dynamic` deliberately embeds quickjs-ng for dynamic islands. An unpinned
  executable build may also report that LLVM fell back to the native C backend;
  neither case should be described as the same experiment. See the official
  [coverage](https://scriptc.dev/coverage), [CLI](https://scriptc.dev/cli) and
  [limitations](https://scriptc.dev/limitations) documentation.
- Gate timings on exact candidate-vector and best-move parity, an unchanged
  post-search game-state digest, and representative replay/equivalence tests.
  Then compare identical one-worker inputs with interleaved samples, separating
  cold startup/compilation from warmed solver throughput and recording elapsed
  time, CPU time, peak memory, binary size, machine, commits and exact scriptc
  version.
- Treat Deno imports/APIs and import maps, JSON imports, prototype manipulation,
  `Hand`'s `Array` subclass, and V8-specific optimization assumptions as coverage
  probes, not confirmed blockers. The tool is experimental and rapidly evolving;
  only the pinned version's coverage and build results can answer them.
