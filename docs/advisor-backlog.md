# Advisor backlog

## Make early searches useful

- Improve progressive sampling order. When answering a card in round 1, spread
  the first few samples across low, middle, all-in, plain and Fury bets instead
  of presenting an average dominated by the current extreme-first ordering.

## Worker pool (implemented 2026-09-11)

- The advisor defaults to three Web Workers; `--workers N` overrides it and
  `--workers 1` keeps the old main-thread path. Obsolete searches are terminated
  when the battle moves on.
- `ParallelSearch` uses the existing `stride` / `offset` partition and merges
  incremental samples. The integration test pins average, minimax, KO, risk and
  sample counts against a completed single-thread search.
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
- Capture and document the site's current manual-play request before
  implementing anything.
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
