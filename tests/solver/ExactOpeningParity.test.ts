// Parity gate for round one, which both advisors now solve exactly.
//
//   deno task rust:worker
//   UR_SLOW_PARITY=1 deno test -A --no-check tests/solver/ExactOpeningParity.test.ts
//
// Round one is the one root where the opponent's reply is weighted by the captured opening
// prior and the only one whose matrix is 8464 pairings wide, so it gets a gate of its own
// beside the per-draw worker gate: the same conservative continuation policy from the
// opening root, in both implementations, compared candidate by candidate for a SECOND and a
// FIRST information set.
//
// It is skipped unless UR_SLOW_PARITY is set, because the single-threaded TypeScript half
// takes seconds for SECOND and tens of seconds for FIRST.
import { assert, assertEquals } from "@std/assert";
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

const worker = await rustWorkerCommand();
const workerAvailable = await (async () => {
  try {
    return (await Deno.stat(worker)).isFile;
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) return false;
    throw error;
  }
})();
const slowEnabled = Deno.env.get("UR_SLOW_PARITY") === "1";

/** The opening decision of a capture, with nothing of the match played yet. */
function opening(source: Capture, decision: "first" | "second") {
  const rec = structuredClone(source);
  rec.rounds = [];
  rec.testcase!.moves = [];
  rec.result = null;
  rec.finalStatus = "playing";

  if (decision === "second") {
    const round = structuredClone(source.rounds[0]!);
    const first = round.moves.find((move: { side: number }) =>
      move.side === round.first
    );
    assert(
      first !== undefined,
      "the opening round must contain its first move",
    );
    round.moves = [first];
    round.resolution = [null, null];
    round.life = [Number.NaN, Number.NaN];
    round.pillz = [Number.NaN, Number.NaN];
    round.postRoundAbilities = [];
    round.durationMs = null;
    rec.rounds.push(round);
  }

  const built = buildPosition(rec);
  assert("game" in built, "the fixture must produce an advice position");
  if (!("game" in built)) throw new Error("unreachable");
  return { rec, game: built.game };
}

async function assertOpeningParity(
  id: number,
  decision: "first" | "second",
  mode: SearchMode,
) {
  const source: Capture = JSON.parse(
    await Deno.readTextFile(`captures/games/${id}.json`),
  );
  const { rec, game } = opening(source, decision);

  const reference = new Search(game);
  assertEquals(reference.mode, mode);
  // Round one solves every leaf and weights the opponent's reply by the opening prior.
  assertEquals(reference.openingPrior, true);

  const input = await normaliseRustAdvisorInput({
    rec,
    game,
    decision: { mode: reference.mode, us: reference.us },
    requestId: `exact-opening-parity-${id}`,
    budgetMs: 30_000,
  });
  assert(input.supported, input.supported ? "" : input.reason);
  if (!input.supported) throw new Error("unreachable");

  const transcript = await runRustAdvisor(
    new DenoCommandRunner({ command: worker }),
    input.request,
    reference.candidates,
    { timeoutMs: 120_000 },
  );
  const rust = new CompletedRustSearch(game, transcript.final);
  assertEquals(transcript.final.evaluationKind, "exact_opening_policy");

  while (reference.step()) {
    /* solve the opening in TypeScript; this is the slow half */
  }
  // Same evaluator, same weighting, same tie-breaks.
  assertEquals(compareRustSearches(reference, rust), "rust match");
}

Deno.test({
  name: "the Rust and TypeScript exact openings agree for SECOND (877636)",
  ignore: !workerAvailable || !slowEnabled,
  // 877636 opens SECOND, the cheap information set: the opponent's card is already
  // visible, so this is 2116 pairings rather than 8464.
  fn: () => assertOpeningParity(877636, "second", SearchMode.SECOND),
});

Deno.test({
  name: "the Rust and TypeScript exact openings agree for FIRST (1089346)",
  ignore: !workerAvailable || !slowEnabled,
  fn: () => assertOpeningParity(1089346, "first", SearchMode.FIRST),
});
