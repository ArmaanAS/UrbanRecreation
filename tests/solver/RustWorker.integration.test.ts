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
    budgetMs: 5_000,
  });
  assert(input.supported, input.supported ? "" : input.reason);
  if (!input.supported) throw new Error("unreachable");

  // `runRustAdvisor` accepts only a clean worker exit plus a complete, schema/action-
  // validated final. This intentionally uses the production runner, not a test wrapper.
  const transcript = await runRustAdvisor(
    new DenoCommandRunner({ command: worker }),
    input.request,
    state.search.candidates,
    { timeoutMs: 10_000 },
  );
  assertEquals(transcript.final.complete, true);
  assertEquals(transcript.final.mode, input.request.mode);
  assertEquals(transcript.final.unitsDone, transcript.final.unitsTotal);
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
    // information modes and both opening and exact continuation evaluation.
    const openingFirst = await runDecision(
      1024673,
      0,
      "first",
      SearchMode.FIRST,
    );
    // Complete each TS semantic comparison before constructing another Game. The TS
    // reference still owns a process-global CardBattle cache and supports one live Game.
    while (openingFirst.search.step()) {
      /* complete the TypeScript opening matrix */
    }
    assertEquals(
      compareRustSearches(openingFirst.search, openingFirst.rust),
      "rust match",
    );
    const vodLifeOpeningFirst = await runDecision(
      925719,
      0,
      "first",
      SearchMode.FIRST,
    );
    while (vodLifeOpeningFirst.search.step()) {
      /* complete the TypeScript VOD-Life opening matrix */
    }
    assertEquals(
      compareRustSearches(vodLifeOpeningFirst.search, vodLifeOpeningFirst.rust),
      "rust match",
    );
    const reprisalOpeningFirst = await runDecision(
      1060199,
      0,
      "first",
      SearchMode.FIRST,
    );
    while (reprisalOpeningFirst.search.step()) {
      /* complete the TypeScript opening matrix */
    }
    assertEquals(
      compareRustSearches(
        reprisalOpeningFirst.search,
        reprisalOpeningFirst.rust,
      ),
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
    await runDecision(1024673, 1, "second", SearchMode.SECOND);
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
