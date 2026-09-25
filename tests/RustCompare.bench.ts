// Head-to-head timing for the same decision in both solvers.
//
//   deno task rust:worker        # the release worker must exist first
//   deno task time-rust
//
// `Search.bench.ts` times the TypeScript live path on a synthetic hand the strict Rust
// catalog cannot construct, so it cannot be compared with anything. This drives real
// decision points from strict-eligible captures through both implementations and prints
// the time each took beside the semantic verdict, because a faster answer that differs is
// not a faster answer. Rust's column is whole-process wall time - spawn, canonical data
// load, search, response - which is what the host actually waits for; `spawn` isolates
// everything but the search so the two parts can be read separately.
import { assert } from "@std/assert";
import {
  buildPosition,
  compareRustSearches,
  rustWorkerCommand,
} from "@/solver/Advisor.ts";
import {
  CompletedRustSearch,
  DenoCommandRunner,
  runRustAdvisor,
} from "@/solver/RustAdvisor.ts";
import { normaliseRustAdvisorInput } from "@/solver/RustAdvisorInput.ts";
import Search, { SearchMode } from "@/solver/Search.ts";

type Capture = Parameters<typeof buildPosition>[0];
type Decision = "first" | "second" | "blind";

/** Decision points across the rule-10 strict draws, covering every live mode. */
const CASES: ReadonlyArray<
  { id: number; completed: number; decision: Decision; label: string }
> = [
  { id: 1024673, completed: 0, decision: "first", label: "round 1 opening FIRST" },
  { id: 1061897, completed: 0, decision: "second", label: "round 1 opening SECOND" },
  { id: 877636, completed: 1, decision: "first", label: "round 2 exact FIRST" },
  { id: 877636, completed: 1, decision: "second", label: "round 2 exact SECOND" },
  { id: 877636, completed: 2, decision: "blind", label: "round 3 blind-second" },
  { id: 877636, completed: 2, decision: "second", label: "round 3 exact SECOND" },
  { id: 1069813, completed: 2, decision: "first", label: "round 3 exact FIRST" },
  { id: 1089346, completed: 3, decision: "first", label: "round 4 exact FIRST" },
  { id: 877636, completed: 3, decision: "first", label: "round 4 exact FIRST" },
];

const worker = await rustWorkerCommand();
try {
  assert((await Deno.stat(worker)).isFile);
} catch {
  console.error(`build the worker first: deno task rust:worker (${worker})`);
  Deno.exit(1);
}

async function capture(id: number): Promise<Capture> {
  return JSON.parse(await Deno.readTextFile(`captures/games/${id}.json`));
}

/** A live-shaped decision point holding only completed history and, for SECOND, a card. */
function atDecision(source: Capture, completed: number, decision: Decision) {
  const rec = structuredClone(source);
  rec.rounds = rec.rounds.slice(0, completed);
  rec.testcase!.moves = rec.testcase!.moves.slice(0, completed);
  rec.result = null;
  rec.finalStatus = "playing";

  if (decision === "second") {
    const round = structuredClone(source.rounds[completed]!);
    const first = round.moves.find((move: { side: number }) =>
      move.side === round.first
    );
    assert(first !== undefined, "the current round must contain its first move");
    round.moves = [first];
    round.resolution = [null, null];
    round.life = [Number.NaN, Number.NaN];
    round.pillz = [Number.NaN, Number.NaN];
    round.postRoundAbilities = [];
    round.durationMs = null;
    rec.rounds.push(round);
  }

  const built = buildPosition(rec);
  if (!("game" in built)) return null;
  return {
    rec,
    game: built.game,
    search: new Search(built.game, 1, 0, decision === "blind"),
  };
}

// SearchMode is a string enum, so its value is already the name to print.
const modeName = (mode: SearchMode): string => mode;

const REPEATS = 3;
const median = (values: number[]) =>
  [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

const log = console.log, info = console.info;
const rows: string[] = [];
let floorMs = Number.POSITIVE_INFINITY;

for (const [at, one] of CASES.entries()) {
  console.log = () => 0;
  console.info = () => 0;
  const state = atDecision(await capture(one.id), one.completed, one.decision);
  if (state === null) {
    console.log = log;
    console.info = info;
    console.log(`${one.label.padEnd(24)} skipped: no advice position here`);
    continue;
  }
  const input = await normaliseRustAdvisorInput({
    rec: state.rec,
    game: state.game,
    decision: { mode: state.search.mode, us: state.search.us },
    requestId: `bench-${one.id}-${one.decision}`,
    budgetMs: 30_000,
  });
  if (!input.supported) {
    // Not every mode exists at every round: whose turn it is decides that, so a case the
    // bridge refuses is a fixture choice rather than a failure. Say so and move on.
    console.log = log;
    console.info = info;
    console.log(`${one.label.padEnd(24)} skipped: ${input.reason}`);
    continue;
  }

  const rustSamples: number[] = [];
  let rust!: CompletedRustSearch;
  for (let run = 0; run < REPEATS; run++) {
    const started = performance.now();
    const transcript = await runRustAdvisor(
      new DenoCommandRunner({ command: worker }),
      { ...input.request, request_id: `${input.request.request_id}-${run}` },
      state.search.candidates,
      { timeoutMs: 600_000 },
    );
    rustSamples.push(performance.now() - started);
    assert(transcript.final.complete, "the Rust result must be complete");
    rust = new CompletedRustSearch(state.game, transcript.final);
  }
  const rustMs = median(rustSamples);
  floorMs = Math.min(floorMs, ...rustSamples);

  // The TypeScript reference owns a process-global battle cache, so every repeat has to
  // reuse this one Game. Each run gets a fresh single-threaded Search over it.
  const tsSamples: number[] = [];
  for (let run = 0; run < REPEATS; run++) {
    const search = new Search(state.game, 1, 0, one.decision === "blind");
    const started = performance.now();
    while (search.step()) { /* complete the TypeScript search */ }
    tsSamples.push(performance.now() - started);
    if (run === REPEATS - 1) state.search = search;
  }
  const tsMs = median(tsSamples);

  const verdict = compareRustSearches(state.search, rust);
  console.log = log;
  console.info = info;
  rows.push(
    `${one.label.padEnd(24)} ${modeName(state.search.mode).padEnd(12)} ` +
      `${state.search.units.toString().padStart(6)} units  ` +
      `TS ${tsMs.toFixed(0).padStart(7)}ms  Rust ${rustMs.toFixed(0).padStart(6)}ms  ` +
      `${(tsMs / rustMs).toFixed(1).padStart(6)}x  ${verdict}`,
  );
  console.log(rows.at(-1));
}

console.log(
  `\nMedian of ${REPEATS} runs each. The Rust column is whole-process wall time, so the ` +
    `smallest case is almost entirely fixed cost: the floor measured here is ` +
    `${floorMs.toFixed(0)}ms of spawn, canonical data load and protocol.`,
);
