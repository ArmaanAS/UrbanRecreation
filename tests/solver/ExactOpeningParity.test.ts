// Parity gate for the Rust worker's exact opening.
//
//   deno task rust:worker
//   UR_SLOW_PARITY=1 deno test -A --no-check tests/solver/ExactOpeningParity.test.ts
//
// The live TypeScript advisor estimates round one, so the ordinary `--rust=compare` path
// has nothing to compare an exact opening against. `Search`'s `exactOpening` reference mode
// closes that hole: it runs the same conservative continuation policy from the opening root
// that both implementations already agree on for rounds two through four, which makes the
// Rust answer checkable against the reference rather than merely plausible.
//
// It is skipped unless UR_SLOW_PARITY is set, because TypeScript takes roughly twenty times
// as long as Rust for identical work: about half a minute for the SECOND information set
// used here, and several minutes for FIRST.
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
    assert(first !== undefined, "the opening round must contain its first move");
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

Deno.test({
  name: "the Rust exact opening agrees with the TypeScript reference exact opening",
  ignore: !workerAvailable || !slowEnabled,
  async fn() {
    // 877636 opens SECOND, which is the cheap information set: the opponent's card is
    // already visible, so this is 2116 pairings rather than 8464.
    const source: Capture = JSON.parse(
      await Deno.readTextFile("captures/games/877636.json"),
    );
    const { rec, game } = opening(source, "second");

    const reference = new Search(game, 1, 0, false, true);
    assertEquals(reference.mode, SearchMode.SECOND);
    // The reference solves the opening, so it must present a win chance and a guaranteed
    // Worst, while still weighting the opponent's reply by the captured opening prior.
    assertEquals(reference.openingEstimate, false);
    assertEquals(reference.openingPrior, true);

    const input = await normaliseRustAdvisorInput({
      rec,
      game,
      decision: { mode: reference.mode, us: reference.us },
      requestId: "exact-opening-parity-877636",
      budgetMs: 30_000,
      openingPolicy: "exact_continuation",
    });
    assert(input.supported, input.supported ? "" : input.reason);
    if (!input.supported) throw new Error("unreachable");

    const transcript = await runRustAdvisor(
      new DenoCommandRunner({ command: worker }),
      input.request,
      reference.candidates,
      { timeoutMs: 120_000 },
    );
    const rust = new CompletedRustSearch(
      game,
      transcript.final,
      "exact_continuation",
    );
    assertEquals(rust.exactOpening, true);
    assertEquals(rust.openingEstimate, false);

    while (reference.step()) {
      /* solve the opening in TypeScript; this is the slow half */
    }
    // Same evaluator, same weighting, same tie-breaks: this is a real comparison, unlike
    // an exact opening measured against the live heuristic.
    assertEquals(compareRustSearches(reference, rust), "rust match");
  },
});
