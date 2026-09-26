// Black-box JSONL worker gate. It is skipped during ordinary `deno test` when the release
// worker has not been built; `deno task rust:worker:test` builds it first.
import { assert, assertEquals, assertStringIncludes } from "@std/assert";
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
import Game from "@/game/Game.ts";

const worker = await rustWorkerCommand();

const workerAvailable = await (async () => {
  try {
    return (await Deno.stat(worker)).isFile;
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) return false;
    throw error;
  }
})();

type Capture = Parameters<typeof buildPosition>[0];
type Decision = "first" | "second" | "blind";

async function capture(id: number): Promise<Capture> {
  return JSON.parse(await Deno.readTextFile(`captures/games/${id}.json`));
}

/** A live-shaped decision point containing only completed history and, optionally, a card. */
function atDecision(
  source: Capture,
  completed: number,
  decision: Decision,
): { rec: Capture; game: Game; search: Search } {
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
    assert(
      first !== undefined,
      "fixture current round must contain its first move",
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
  assert("game" in built, "fixture must produce an advice position");
  if (!("game" in built)) throw new Error("unreachable");
  const search = new Search(built.game, 1, 0, decision === "blind");
  return { rec, game: built.game, search };
}

async function runDecision(
  id: number,
  completed: number,
  decision: Decision,
  expectedMode: SearchMode,
) {
  const state = atDecision(await capture(id), completed, decision);
  assertEquals(state.search.mode, expectedMode, `capture ${id} TS mode`);
  const input = await normaliseRustAdvisorInput({
    rec: state.rec,
    game: state.game,
    decision: { mode: state.search.mode, us: state.search.us },
    requestId: `rust-worker-${id}-${decision}`,
    budgetMs: 30_000,
  });
  assert(input.supported, input.supported ? "" : input.reason);
  if (!input.supported) throw new Error("unreachable");

  // `runRustAdvisor` accepts only a clean worker exit plus a complete, schema/action-
  // validated final. This intentionally uses the production runner, not a test wrapper.
  const transcript = await runRustAdvisor(
    new DenoCommandRunner({ command: worker }),
    input.request,
    state.search.candidates,
    { timeoutMs: 40_000 },
  );
  assertEquals(transcript.final.complete, true);
  assertEquals(transcript.final.mode, input.request.mode);
  assertEquals(transcript.final.unitsDone, transcript.final.unitsTotal);
  assertEquals(
    transcript.final.evaluationKind,
    completed === 0 ? "exact_opening_policy" : "exact_continuation_policy",
  );
  const rust = new CompletedRustSearch(state.game, transcript.final);
  assertEquals(rust.done, true);
  assertEquals(rust.mode, state.search.mode);
  assertEquals(rust.candidates.length, state.search.candidates.length);
  return { ...state, rust };
}

Deno.test({
  name:
    "release Rust worker handles admissible capture decisions through the production bridge",
  ignore: !workerAvailable,
  async fn() {
    // At least one decision from every rule-10 strict draw, spanning all three live
    // information modes and both the opening and later rounds. Round one is an exact solve
    // in both implementations; a FIRST opening costs the single-threaded TypeScript side
    // four to thirteen seconds, so this gate keeps one (877812, the cheapest) and meets the
    // other FIRST-opening draws in round two. tests/solver/ExactOpeningParity.test.ts adds
    // 1089346's FIRST opening under UR_SLOW_PARITY.
    // Complete each TS semantic comparison before constructing the next Game. Each Game
    // owns its battle cache now, so this is only to keep one comparison in flight at a time.
    const vodLifeSecond = await runDecision(
      925719,
      1,
      "second",
      SearchMode.SECOND,
    );
    while (vodLifeSecond.search.step()) {
      /* complete the TypeScript VOD-Life round-two matrix */
    }
    assertEquals(
      compareRustSearches(vodLifeSecond.search, vodLifeSecond.rust),
      "rust match",
    );
    const equalizerLifeOpeningFirst = await runDecision(
      877812,
      0,
      "first",
      SearchMode.FIRST,
    );
    while (equalizerLifeOpeningFirst.search.step()) {
      /* complete the TypeScript Equalizer-Life opening matrix */
    }
    assertEquals(
      compareRustSearches(
        equalizerLifeOpeningFirst.search,
        equalizerLifeOpeningFirst.rust,
      ),
      "rust match",
    );
    // Anita's exact Courage conversion admits these three further strict rule-10 draws.
    // Keep one completed production decision for each, rather than treating catalog
    // eligibility as proof that the worker's post-round implementation agrees with TS.
    // 1061897's owner moved second, so retain the committed first card for an actual
    // opening SECOND decision rather than forcing a nonexistent FIRST view. The other two
    // owners opened FIRST, so they answer the revealed card in round two.
    const anitaDecisions: Array<[number, number]> = [
      [1061897, 0],
      [1069813, 1],
      [1089346, 1],
    ];
    for (const [id, completed] of anitaDecisions) {
      const anita = await runDecision(
        id,
        completed,
        "second",
        SearchMode.SECOND,
      );
      while (anita.search.step()) {
        /* complete the TypeScript Anita matrix */
      }
      assertEquals(
        compareRustSearches(anita.search, anita.rust),
        "rust match",
      );
    }
    // Mou's unconditional Victory opponent-Life adds strict rule-10 draw 925674. The
    // recording side moved second in round 0, so this is an opening SECOND decision.
    const victoryOpponentLifeOpening = await runDecision(
      925674,
      0,
      "second",
      SearchMode.SECOND,
    );
    while (victoryOpponentLifeOpening.search.step()) {
      /* complete the TypeScript opening matrix for the new strict draw */
    }
    assertEquals(
      compareRustSearches(
        victoryOpponentLifeOpening.search,
        victoryOpponentLifeOpening.rust,
      ),
      "rust match",
    );
    const reprisalSecond = await runDecision(
      1060199,
      1,
      "second",
      SearchMode.SECOND,
    );
    while (reprisalSecond.search.step()) {
      /* complete the TypeScript round-two matrix */
    }
    assertEquals(
      compareRustSearches(reprisalSecond.search, reprisalSecond.rust),
      "rust match",
    );
    const openingSecond = await runDecision(
      877950,
      0,
      "second",
      SearchMode.SECOND,
    );
    while (openingSecond.search.step()) {
      /* complete the TypeScript opening matrix */
    }
    assertEquals(
      compareRustSearches(openingSecond.search, openingSecond.rust),
      "rust match",
    );
    const exactFirst = await runDecision(877636, 1, "first", SearchMode.FIRST);
    while (exactFirst.search.step()) {
      /* complete the TypeScript exact matrix */
    }
    assertEquals(
      compareRustSearches(exactFirst.search, exactFirst.rust),
      "rust match",
    );
    const exactSecond = await runDecision(
      1024673,
      1,
      "second",
      SearchMode.SECOND,
    );
    while (exactSecond.search.step()) {
      /* complete the TypeScript exact matrix, including hidden opponent wagers */
    }
    assertEquals(
      compareRustSearches(exactSecond.search, exactSecond.rust),
      "rust match",
    );
    const blindSecond = await runDecision(
      877636,
      2,
      "blind",
      SearchMode.BLIND_SECOND,
    );
    while (blindSecond.search.step()) {
      /* complete the TypeScript blind-second matrix */
    }
    assertEquals(
      compareRustSearches(blindSecond.search, blindSecond.rust),
      "rust match",
    );
  },
});

Deno.test({
  name:
    "the hosted worker solves the opening exactly, weighted by the opening prior",
  ignore: !workerAvailable,
  async fn() {
    // 877636 opens SECOND, the cheap information set: the opponent's card is visible.
    const opening = await runDecision(877636, 0, "second", SearchMode.SECOND);
    assert(opening.search.openingPrior);
    while (opening.search.step()) {
      /* solve the TypeScript opening */
    }
    // Every leaf is a solved line, so the Worst column is categorical on both sides.
    for (const candidate of opening.rust.candidates) {
      assert([-1, 0, 1].includes(candidate.minimax), candidate.key);
    }
    assertEquals(
      compareRustSearches(opening.search, opening.rust),
      "rust match",
    );
  },
});

Deno.test("Rust production normaliser keeps rule-3 capture 1081463 fail-closed", async () => {
  const state = atDecision(await capture(1081463), 1, "first");
  assertEquals(state.search.mode, SearchMode.FIRST);
  const result = await normaliseRustAdvisorInput({
    rec: state.rec,
    game: state.game,
    decision: { mode: state.search.mode, us: state.search.us },
    requestId: "rust-worker-1081463-rule-3",
    budgetMs: 100,
  });
  assert(!result.supported);
  if (result.supported) {
    throw new Error("rule-3 capture unexpectedly reached worker");
  }
  assertStringIncludes(result.reason, "battle rule 3");
});
