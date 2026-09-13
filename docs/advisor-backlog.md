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
