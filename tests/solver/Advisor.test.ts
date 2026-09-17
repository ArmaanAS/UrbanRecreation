import { assert, assertEquals, assertNotEquals } from "@std/assert";
import {
  buildPosition,
  parseArgs,
  positionKey,
  rustDecisionEnabled,
  RustDecisionJob,
  writeAllSync,
} from "@/solver/Advisor.ts";
import Search from "@/solver/Search.ts";
import Game from "@/game/Game.ts";
import Player from "@/game/Player.ts";
import { HandGenerator } from "@/game/Hand.ts";
import { Turn } from "@/game/types/Types.ts";
import type {
  RustAdvisorRequest,
  RustAdvisorRunner,
} from "@/solver/RustAdvisor.ts";
import type { RustAdvisorInputResult } from "@/solver/RustAdvisorInput.ts";
import { SearchMode } from "@/solver/Search.ts";

const rustJobGame = () =>
  new Game(
    new Player(12, 3, 0),
    new Player(12, 3, 1),
    HandGenerator.handOf(["Natrang", "Natrang", "Natrang", "Natrang"]),
    HandGenerator.handOf(["Natrang", "Natrang", "Natrang", "Natrang"]),
    Turn.PLAYER_1,
    false,
  );

const wireProvenance = {
  effective_catalog_fingerprint_fnv1a64: "0000000000000000",
  effect_registry_fingerprint_fnv1a64: "0000000000000000",
  effect_registry_schema_version: 1,
  compiler_policy_semantic_revision: 20,
  catalog_context_policy_semantic_revision: 3,
  advisor_policy_semantic_revision: 1,
};

const workerFinal = (requestId: string, search: Search) =>
  JSON.stringify({
    protocol_version: 3,
    request_id: requestId,
    sequence: 0,
    kind: "final",
    provenance: wireProvenance,
    mode: search.mode === SearchMode.BLIND_SECOND
      ? "blind_second"
      : search.mode,
    opponent_hand_index: search.oppIndex ?? null,
    score_frame: "requester",
    evaluation_kind: "opening_estimate",
    complete: true,
    units_done: search.units,
    units_total: search.units,
    elapsed_ms: 1,
    ranked_moves: search.candidates.map((candidate) => ({
      hand_index: candidate.index,
      pillz: candidate.pillz,
      fury: candidate.fury,
      score: 0,
      worst: 0,
      best: 0,
      samples: search.samples,
      ko_share: 0,
      loss_share: 0,
      hidden_outcomes: null,
    })),
  });

const waitFor = async (predicate: () => boolean) => {
  for (let i = 0; i < 100; i++) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  throw new Error("timed out waiting for fake Rust worker");
};

function startFakeRustJob(
  mode: "compare" | "use",
  runner: RustAdvisorRunner,
  normalise: () => Promise<RustAdvisorInputResult> = () =>
    Promise.resolve(supportedRequest()),
) {
  const game = rustJobGame();
  let search: Search = new Search(game);
  let current = true;
  const requestId = "advisor-test";
  const job = new RustDecisionJob({
    mode,
    key: requestId,
    requestId,
    game,
    getSearch: () => search,
    replaceSearch: (next) => search = next,
    isCurrent: () => current,
    normalise,
    runner,
    budgetMs: 50,
  });
  return {
    game,
    job,
    get search() {
      return search;
    },
    stop: () => current = false,
  };
}

function supportedRequest() {
  return {
    supported: true as const,
    request: {
      protocol_version: 3,
      request_id: "advisor-test",
      mode: "first",
      provenance: wireProvenance,
    } as RustAdvisorRequest,
  };
}

const successfulRunner = (search: () => Search): RustAdvisorRunner => ({
  run(request) {
    const requestId = JSON.parse(request).request_id;
    return Promise.resolve({
      code: 0,
      stdout: `${workerFinal(requestId, search())}\n`,
      stderr: "",
    });
  },
});

Deno.test("the terminal writer drains partial synchronous writes", () => {
  const input = new TextEncoder().encode(
    "a large coloured terminal frame".repeat(100),
  );
  const received: number[] = [];
  const writer = {
    writeSync(data: Uint8Array) {
      const accepted = Math.min(17, data.length);
      received.push(...data.subarray(0, accepted));
      return accepted;
    },
  };

  writeAllSync(writer, input);

  assertEquals(new Uint8Array(received), input);
});

Deno.test("the safe-table preview is available without live-mode options", () => {
  const options = parseArgs(["--preview-safe"]);
  assertEquals(options.preview, "safe");
  assertEquals(options.replay, undefined);
  assertEquals(options.resultStyle, "classic");
  assertEquals(options.feed, "http://127.0.0.1:8787/events");
  assertEquals(options.top, undefined);
  assertEquals(options.rust, "off");
});

Deno.test("Rust advisor mode is explicit and fails closed on unknown values", () => {
  assertEquals(parseArgs(["--rust=compare"]).rust, "compare");
  assertEquals(parseArgs(["--rust", "use"]).rust, "use");
  assertEquals(parseArgs(["--rust=off"]).rust, "off");
  for (const args of [["--rust"], ["--rust=maybe"]]) {
    let rejected = false;
    try {
      parseArgs(args);
    } catch {
      rejected = true;
    }
    assertEquals(rejected, true);
  }
});

Deno.test("Rust worker eligibility is off by default and covers every search mode", () => {
  assertEquals(rustDecisionEnabled("off", SearchMode.FIRST), false);
  assertEquals(rustDecisionEnabled("off", SearchMode.SECOND), false);
  assertEquals(rustDecisionEnabled("compare", SearchMode.SECOND), true);
  assertEquals(rustDecisionEnabled("use", SearchMode.BLIND_SECOND), true);
  assertEquals(rustDecisionEnabled("compare", SearchMode.FIRST), true);
});

Deno.test("compare keeps the TypeScript search authoritative", async () => {
  const runner = successfulRunner(() => state.search);
  const state = startFakeRustJob("compare", runner);
  const ts = state.search;
  await waitFor(() => state.job.status.startsWith("rust ready"));
  assertEquals(state.search, ts);
  // A complete TS ranking is the point at which compare publishes match/differs.
  while (state.search.step()) { /* small opening search */ }
  state.job.settleCompare();
  assert(
    state.job.status.startsWith("rust match") ||
      state.job.status.startsWith("rust differs"),
  );
  assertEquals(state.search, ts);
});

Deno.test("use atomically replaces TypeScript only after a valid complete Rust response", async () => {
  const runner = successfulRunner(() => state.search);
  const state = startFakeRustJob("use", runner);
  const ts = state.search;
  await waitFor(() => !state.job.waiting);
  assertNotEquals(state.search, ts);
  assertEquals(state.search.done, true);
  assertEquals(
    state.job.status,
    "rust active · v3:20/3/1",
  );
});

Deno.test("a rejected Rust normalisation leaves the TypeScript fallback live", async () => {
  const neverRuns: RustAdvisorRunner = {
    run: () => Promise.reject(new Error("runner should not start")),
  };
  const state = startFakeRustJob(
    "use",
    neverRuns,
    () =>
      Promise.resolve({
        supported: false as const,
        reason: "test input is unsupported",
      }),
  );
  const ts = state.search;
  await waitFor(() => !state.job.waiting);
  assertEquals(state.search, ts);
  assertEquals(state.job.status.startsWith("rust rejected"), true);
});

Deno.test("a Rust semantic-provenance mismatch leaves the TypeScript fallback live", async () => {
  const runner: RustAdvisorRunner = {
    run(request) {
      const requestId = JSON.parse(request).request_id;
      const response = JSON.parse(workerFinal(requestId, state.search));
      response.provenance.advisor_policy_semantic_revision = 2;
      return Promise.resolve({
        code: 0,
        stdout: `${JSON.stringify(response)}\n`,
        stderr: "",
      });
    },
  };
  const state = startFakeRustJob("use", runner);
  const ts = state.search;
  await waitFor(() => !state.job.waiting);
  assertEquals(state.search, ts);
  assert(state.job.status.includes("TS fallback"));
  assert(state.job.status.includes("v3:20/3/1"));
});

Deno.test("a cancelled position ignores a late Rust result", async () => {
  let complete!: () => void;
  const runner: RustAdvisorRunner = {
    run: () =>
      new Promise((resolve) =>
        complete = () =>
          resolve({
            code: 0,
            stdout: `${workerFinal("advisor-test", state.search)}\n`,
            stderr: "",
          })
      ),
  };
  const state = startFakeRustJob("use", runner);
  const ts = state.search;
  await waitFor(() => state.job.status.startsWith("rust running"));
  state.job.cancel();
  complete();
  await new Promise((resolve) => setTimeout(resolve, 0));
  assertEquals(state.search, ts);
  assertEquals(state.job.waiting, false);
});

Deno.test("result banners can be previewed and selected", () => {
  const options = parseArgs([
    "--preview-results",
    "--result-style",
    "framed",
  ]);
  assertEquals(options.preview, "results");
  assertEquals(options.resultStyle, "framed");
});

Deno.test("a completed capture carries its result into the holding screen", async () => {
  const captured = JSON.parse(
    await Deno.readTextFile("captures/games/866431.json"),
  ) as Parameters<typeof buildPosition>[0];
  const built = buildPosition(captured);

  assertEquals("game" in built, false);
  if ("game" in built) return;
  assertEquals(built.finished, true);
  assertEquals(built.why, "");
  assertEquals(built.holding?.outcome, "win");
  assertNotEquals(built.holding?.board.selectedYou, undefined);
  assertNotEquals(built.holding?.board.selectedThem, undefined);
  assertEquals(built.holding?.board.turn, undefined);
  const resolved = captured.testcase!.moves.length;
  const beforeLast = captured.rounds[resolved - 2];
  const expectedAvailable = beforeLast === undefined
    ? captured.players[captured.mySide!].basePillz
    : beforeLast.pillz[captured.mySide!];
  assertEquals(
    built.holding?.board.battle?.you?.availablePillz,
    expectedAvailable,
  );

  // The live feed first sends `done`, then the authoritative result response. They must be
  // different decision identities or the second event is skipped as an ordinary poll.
  const beforeResult = { ...captured, result: null };
  const provisional = buildPosition(beforeResult);
  assertEquals("game" in provisional, false);
  if ("game" in provisional) return;
  assertEquals(provisional.finished, true);
  assertEquals(provisional.holding?.outcome, undefined);
  assertEquals(provisional.holding?.progress, "confirming result...");
  assertNotEquals(
    positionKey(beforeResult, captured.id),
    positionKey(captured, captured.id),
  );
});

Deno.test("live abilities missing from static card data suppress unsafe advice", async () => {
  const captured = JSON.parse(
    await Deno.readTextFile("captures/games/866431.json"),
  ) as Parameters<typeof buildPosition>[0];
  const changed = structuredClone(captured);
  const card = changed.players.flatMap((player) => player.hand).find((
    candidate,
  ) => candidate.name === "Leonaparte")!;
  // Leonaparte has no ability in the loaded data. This models an EFC semi-evo changing
  // before the periodic character dump, as happened to Quetzal Cr in battle 1131463.
  card.ability = { id: -1, description: "+1 Damage" };

  const built = buildPosition(changed);

  assertEquals("game" in built, false);
  if ("game" in built) return;
  assertEquals(built.settled, false);
  assertEquals(built.why.includes("advice withheld"), true);
});

Deno.test("a result ends the advisor even when a forfeited battle still says playing", async () => {
  const captured = JSON.parse(
    await Deno.readTextFile("captures/games/866431.json"),
  ) as Parameters<typeof buildPosition>[0];
  const firstRound = captured.rounds[0];
  const forfeit = {
    ...captured,
    finalStatus: "playing",
    rounds: [
      firstRound,
      {
        round: 1,
        first: 1,
        moves: [],
        resolution: [null, null],
        life: [captured.result!.player.life, captured.result!.opponent.life],
        pillz: [captured.result!.player.pillz, captured.result!.opponent.pillz],
        postRoundAbilities: [],
        durationMs: 0,
      },
    ],
    result: {
      ...captured.result!,
      result: "lose",
      byKo: false,
      player: { ...captured.result!.player, life: 8, pillz: 2 },
      opponent: { ...captured.result!.opponent, life: 12, pillz: 6 },
    },
    testcase: {
      ...captured.testcase!,
      moves: captured.testcase!.moves.slice(0, 1),
    },
  } as Parameters<typeof buildPosition>[0];

  const built = buildPosition(forfeit);

  assertEquals("game" in built, false);
  if ("game" in built) return;
  assertEquals(built.finished, true);
  assertEquals(built.holding?.outcome, "lose");
  assertEquals(built.holding?.round, 2);
  assertEquals(built.holding?.you, { life: 8, pillz: 2 });
  assertEquals(built.holding?.them, { life: 12, pillz: 6 });
});

Deno.test("rounds two and three offer provisional replies while the opponent is choosing", async () => {
  const captured = JSON.parse(
    await Deno.readTextFile("captures/games/866431.json"),
  ) as Parameters<typeof buildPosition>[0];

  const afterRoundOne = captured.rounds[0];
  const roundTwo = {
    round: 1,
    first: 1,
    moves: [],
    resolution: [null, null],
    life: [...afterRoundOne.life],
    pillz: [...afterRoundOne.pillz],
    postRoundAbilities: [],
    durationMs: null,
  };
  const roundTwoWaiting = {
    ...captured,
    mySide: 0,
    result: null,
    finalStatus: "playing",
    rounds: [afterRoundOne, roundTwo],
    testcase: {
      ...captured.testcase!,
      moves: captured.testcase!.moves.slice(0, 1),
    },
  } as Parameters<typeof buildPosition>[0];
  const secondRoundProvisional = buildPosition(roundTwoWaiting);
  assertEquals("game" in secondRoundProvisional, true);
  if (!("game" in secondRoundProvisional)) return;
  assertEquals(secondRoundProvisional.round, 2);
  assertEquals(secondRoundProvisional.provisional, true);
  assertEquals(secondRoundProvisional.board.turn, "them");

  const previous = captured.rounds[1];
  const current = {
    round: 2,
    first: 0,
    moves: [],
    resolution: [null, null],
    life: [...previous.life],
    pillz: [...previous.pillz],
    postRoundAbilities: [],
    durationMs: null,
  };
  const waiting = {
    ...captured,
    mySide: 1,
    result: null,
    finalStatus: "playing",
    rounds: [...captured.rounds.slice(0, 2), current],
    testcase: {
      ...captured.testcase!,
      moves: captured.testcase!.moves.slice(0, 2),
    },
  } as Parameters<typeof buildPosition>[0];

  const provisional = buildPosition(waiting);
  assertEquals("game" in provisional, true);
  if (!("game" in provisional)) return;
  assertEquals(provisional.round, 3);
  assertEquals(provisional.provisional, true);
  assertEquals(provisional.board.turn, "them");

  const committed = {
    ...waiting,
    rounds: [
      ...waiting.rounds.slice(0, 2),
      {
        ...current,
        moves: [{
          side: 0,
          index: 0,
          cardId: waiting.players[0].hand[0].id,
          pillz: 0,
          pillzUsed: 1,
          fury: false,
          t: Date.now(),
        }],
      },
    ],
  } as Parameters<typeof buildPosition>[0];
  const precise = buildPosition(committed);
  assertEquals("game" in precise, true);
  if (!("game" in precise)) return;
  assertEquals(precise.provisional, undefined);
  assertEquals(precise.board.turn, "you");
  assertEquals(
    precise.board.battle?.them?.availablePillz,
    previous.pillz[0],
  );
});

Deno.test("live advice resynchronises engine pillz to the server between rounds", async () => {
  const captured = JSON.parse(
    await Deno.readTextFile("captures/games/1093173.json"),
  ) as Parameters<typeof buildPosition>[0];
  const afterRoundThree = captured.rounds[2];
  const beforeFinal = {
    ...captured,
    result: null,
    finalStatus: "playing",
    rounds: [
      ...captured.rounds.slice(0, 3),
      {
        ...captured.rounds[3],
        moves: [],
        resolution: [null, null],
        life: [...afterRoundThree.life],
        pillz: [...afterRoundThree.pillz],
        postRoundAbilities: [],
        durationMs: null,
      },
    ],
    testcase: {
      ...captured.testcase!,
      moves: captured.testcase!.moves.slice(0, 3),
    },
  } as Parameters<typeof buildPosition>[0];

  const built = buildPosition(beforeFinal);

  assertEquals("game" in built, true);
  if (!("game" in built)) return;
  assertEquals(built.round, 4);
  assertEquals(built.game.playingPlayer.pillz, 3);
  assertEquals(
    built.warning?.includes("your pillz 4 vs server 3"),
    true,
  );
  assertEquals(
    new Search(built.game).candidates.some((candidate) => candidate.pillz > 3),
    false,
  );
});
