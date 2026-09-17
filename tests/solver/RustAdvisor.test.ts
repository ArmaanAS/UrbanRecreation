import "colors";
import {
  assertEquals,
  assertRejects,
  assertStringIncludes,
  assertThrows,
} from "@std/assert";
import Game from "@/game/Game.ts";
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import { Turn } from "@/game/types/Types.ts";
import Search, { openingReplyWeight, SearchMode } from "@/solver/Search.ts";
import { render } from "@/solver/SolverView.ts";
import {
  buildAdvisorRequest,
  buildFirstRequest,
  CompletedRustSearch,
  decodeRustJsonl,
  runRustAdvisor,
  RustAdvisorCancelledError,
  type RustAdvisorFinal,
  RustAdvisorProtocolError,
  type RustAdvisorRunner,
  RustAdvisorTimeoutError,
  type RustFirstInput,
  type RustFirstRequest,
} from "@/solver/RustAdvisor.ts";

const actions = [
  { index: 0, pillz: 0, fury: false },
  { index: 1, pillz: 1, fury: false },
];
const provenance = {
  effectiveCatalogFingerprintFnv1a64: "0000000000000000",
  effectRegistryFingerprintFnv1a64: "0000000000000000",
  effectRegistrySchemaVersion: 1,
  compilerPolicySemanticRevision: 21,
  catalogContextPolicySemanticRevision: 3,
  advisorPolicySemanticRevision: 1,
};
const wireProvenance = {
  effective_catalog_fingerprint_fnv1a64: "0000000000000000",
  effect_registry_fingerprint_fnv1a64: "0000000000000000",
  effect_registry_schema_version: 1,
  compiler_policy_semantic_revision: 21,
  catalog_context_policy_semantic_revision: 3,
  advisor_policy_semantic_revision: 1,
};
const expectation = {
  version: 3 as const,
  requestId: "request-1",
  provenance,
  mode: "first" as const,
  opponentHandIndex: null,
  candidates: actions,
};
const row = (move = actions[0], score = 0.5) => ({
  hand_index: move.index,
  pillz: move.pillz,
  fury: move.fury,
  score,
  worst: -0.25,
  best: 0.75,
  samples: 2,
  ko_share: 0.5,
  loss_share: 0,
  hidden_outcomes: null,
});
const line = (
  kind: "progress" | "final",
  sequence: number,
  ranked_moves: unknown[],
  complete = kind === "final",
  mode: "first" | "second" | "blind_second" = "first",
  opponent_hand_index: number | null = null,
) =>
  JSON.stringify({
    protocol_version: 3,
    request_id: "request-1",
    sequence,
    kind,
    provenance: wireProvenance,
    score_frame: "requester",
    evaluation_kind: "exact_continuation_policy",
    complete,
    units_done: complete ? 4 : 1,
    units_total: 4,
    elapsed_ms: sequence + 1,
    mode,
    opponent_hand_index,
    ranked_moves,
  });

Deno.test("Rust JSONL permits a partial progress subset but requires an exact completed final", () => {
  const transcript = decodeRustJsonl(
    `${line("progress", 0, [row(actions[0])], false)}\n${
      line("final", 1, [row(actions[1]), row(actions[0])])
    }\n`,
    expectation,
  );
  assertEquals(transcript.progress.length, 1);
  assertEquals(transcript.final.ranked.length, 2);
  const wrongProvenance = JSON.parse(
    line("final", 0, [row(actions[0]), row(actions[1])]),
  );
  wrongProvenance.provenance.advisor_policy_semantic_revision = 2;
  assertThrows(
    () => decodeRustJsonl(`${JSON.stringify(wrongProvenance)}\n`, expectation),
    RustAdvisorProtocolError,
  );
  assertThrows(
    () =>
      decodeRustJsonl(`${line("final", 0, [row(actions[0])])}\n`, expectation),
    RustAdvisorProtocolError,
  );
  assertThrows(
    () =>
      decodeRustJsonl(
        `${line("progress", 0, [row(actions[0])], true)}\n`,
        expectation,
      ),
    RustAdvisorProtocolError,
  );
  assertThrows(
    () =>
      decodeRustJsonl(
        `${line("final", 0, [row(actions[0])], false)}\n`,
        { ...expectation, candidates: [actions[0]] },
      ),
    RustAdvisorProtocolError,
  );
});

Deno.test("Rust JSONL fails closed for duplicate candidates, score violations, and non-monotonic work", () => {
  assertThrows(
    () =>
      decodeRustJsonl(
        `${line("final", 0, [row(actions[0]), row(actions[0])])}\n`,
        expectation,
      ),
    RustAdvisorProtocolError,
  );
  assertThrows(
    () =>
      decodeRustJsonl(
        `${line("final", 0, [row(actions[0], 2), row(actions[1])])}\n`,
        expectation,
      ),
    RustAdvisorProtocolError,
  );
  const backward = JSON.parse(
    line("final", 2, [row(actions[0]), row(actions[1])]),
  ) as Record<string, unknown>;
  backward.units_done = 0;
  assertThrows(
    () =>
      decodeRustJsonl(
        `${line("progress", 1, [row(actions[0])], false)}\n${
          JSON.stringify(backward)
        }\n`,
        expectation,
      ),
    RustAdvisorProtocolError,
  );
});

Deno.test("Rust V3 SECOND JSONL requires one exact outcome per hidden wager", () => {
  const secondExpectation = {
    ...expectation,
    mode: "second" as const,
    opponentHandIndex: 0,
    opponentMoves: [{ pillz: 0, fury: false }],
  };
  const withOutcome = (move: typeof actions[number]) => ({
    ...row(move, 0),
    worst: 0,
    best: 0,
    samples: 1,
    ko_share: 0,
    hidden_outcomes: [[0, false, 0, 0]],
  });
  const valid = line(
    "final",
    0,
    actions.map(withOutcome),
    true,
    "second",
    0,
  );
  assertEquals(
    decodeRustJsonl(`${valid}\n`, secondExpectation).final.ranked[0]
      .hiddenOutcomes?.length,
    1,
  );
  const duplicate = JSON.parse(valid) as Record<string, unknown>;
  const moves = duplicate.ranked_moves as { hidden_outcomes: unknown[] }[];
  moves[0].hidden_outcomes.push([0, false, 0, 0]);
  assertThrows(
    () => decodeRustJsonl(`${JSON.stringify(duplicate)}\n`, secondExpectation),
    RustAdvisorProtocolError,
  );
  assertThrows(
    () =>
      decodeRustJsonl(
        `${
          line("progress", 0, actions.map(withOutcome), false, "second", 0)
        }\n`,
        secondExpectation,
      ),
    RustAdvisorProtocolError,
  );
  assertThrows(
    () =>
      decodeRustJsonl(
        `${line("final", 0, actions.map(withOutcome))}\n`,
        secondExpectation,
      ),
    RustAdvisorProtocolError,
  );
});

const card = (id: number) => ({
  id,
  level: 1,
  abilityId: 0,
  ability: "",
  bonusId: 0,
  bonus: "",
});

const firstInput = (): RustFirstInput => ({
  requestId: "request-1",
  us: "p1",
  firstMover: "p1",
  battleRuleId: 10,
  night: false,
  provenance,
  players: {
    p1: {
      initial: { life: 12, pillz: 0 },
      current: { life: 12, pillz: 0 },
      played: [false, false, false, false],
      hand: [card(1), card(2), card(3), card(4)],
    },
    p2: {
      initial: { life: 12, pillz: 0 },
      current: { life: 12, pillz: 0 },
      played: [false, false, false, false],
      hand: [card(5), card(6), card(7), card(8)],
    },
  },
  history: [],
  budgetMs: 10,
});
const request = (): RustFirstRequest => buildFirstRequest(firstInput());

Deno.test("Rust V3 request builder emits only the strict worker shape", () => {
  const built = request();
  assertEquals(built.protocol_version, 3);
  assertEquals(built.provenance, wireProvenance);
  assertEquals(built.battle_rule_id, 10);
  assertEquals(built.players.p1.hand.length, 4);
  assertEquals(Object.keys(built.players.p1.hand[0]).sort(), [
    "ability",
    "ability_id",
    "bonus",
    "bonus_id",
    "id",
    "level",
  ]);
  assertThrows(
    () => buildFirstRequest({ ...firstInput(), battleRuleId: 11 }),
    RustAdvisorProtocolError,
  );
  assertThrows(
    () =>
      buildFirstRequest({
        ...firstInput(),
        players: {
          ...firstInput().players,
          p1: { ...firstInput().players.p1, played: [false] },
        },
      }),
    RustAdvisorProtocolError,
  );
});

Deno.test("Rust V3 builder makes mode and visible card explicit", () => {
  const second = buildAdvisorRequest({
    ...firstInput(),
    mode: "second",
    us: "p2",
    firstMover: "p1",
    opponentHandIndex: 2,
  });
  assertEquals(second.mode, "second");
  assertEquals(second.mode === "second" && second.opponent_hand_index, 2);
  const blind = buildAdvisorRequest({
    ...firstInput(),
    mode: "blind_second",
    us: "p1",
    firstMover: "p2",
    history: [{
      firstMover: "p1",
      p1: { handIndex: 0, pillz: 0, fury: false },
      p2: { handIndex: 0, pillz: 0, fury: false },
    }],
  });
  assertEquals(blind.mode, "blind_second");
  assertEquals("opponent_hand_index" in blind, false);
});

class CleanupRunner implements RustAdvisorRunner {
  cleaned = false;
  run(
    _request: string,
    options: { signal: AbortSignal },
  ): Promise<{ code: number; stdout: string; stderr: string }> {
    return new Promise((resolve) =>
      options.signal.addEventListener("abort", () => {
        this.cleaned = true;
        resolve({ code: 143, stdout: "", stderr: "" });
      }, { once: true })
    );
  }
}

Deno.test("Rust runner wrapper reports timeout and cancellation without a process", async () => {
  const timeout = new CleanupRunner();
  await assertRejects(
    () => runRustAdvisor(timeout, request(), actions, { timeoutMs: 1 }),
    RustAdvisorTimeoutError,
  );
  assertEquals(timeout.cleaned, true);
  const cancel = new AbortController();
  const cancelled = new CleanupRunner();
  const pending = runRustAdvisor(cancelled, request(), actions, {
    timeoutMs: 1_000,
    signal: cancel.signal,
  });
  cancel.abort();
  await assertRejects(() => pending, RustAdvisorCancelledError);
  assertEquals(cancelled.cleaned, true);

  const alreadyCancelled = new AbortController();
  alreadyCancelled.abort();
  await assertRejects(
    () =>
      runRustAdvisor(new CleanupRunner(), request(), actions, {
        signal: alreadyCancelled.signal,
      }),
    RustAdvisorCancelledError,
  );

  await assertRejects(
    () =>
      runRustAdvisor(
        {
          run: () =>
            Promise.resolve({
              code: 1,
              stdout: "",
              stderr: "worker\nrejected\x1b[31m",
            }),
        },
        request(),
        actions,
      ),
    RustAdvisorProtocolError,
    "worker rejected [31m",
  );
});

function p2FirstGame() {
  return new Game(
    new Player(12, 0, 0),
    new Player(12, 0, 1),
    HandGenerator.handOf(["Genmaicha", "Orka", "Sando", "Deborah"]),
    HandGenerator.handOf(["Nathan", "El Kuzco", "Noon Steevens", "Strygia"]),
    Turn.PLAYER_2,
    false,
  );
}

Deno.test("completed Rust FIRST adapter flips requester P2 scores into the TS P1 frame and renders", () => {
  const game = p2FirstGame();
  const ts = new Search(game);
  const ranked = ts.candidates.map((candidate) => ({
    handIndex: candidate.index,
    pillz: candidate.pillz,
    fury: candidate.fury,
    score: 0.5,
    worst: 0.25,
    best: 0.75,
    samples: ts.samples,
    kos: 0,
    koed: 0,
    hiddenOutcomes: null,
  }));
  const final: RustAdvisorFinal = {
    kind: "final",
    sequence: 0,
    provenance,
    evaluationKind: "opening_estimate",
    complete: true,
    unitsDone: ts.units,
    unitsTotal: ts.units,
    elapsedMs: 3,
    mode: "first",
    opponentHandIndex: null,
    ranked,
  };
  const adapter = new CompletedRustSearch(game, final);
  assertEquals(adapter.us, Turn.PLAYER_2);
  assertEquals(adapter.candidates[0].average, -0.5);
  assertEquals(adapter.percent(adapter.candidates[0].average), 75);
  assertStringIncludes(
    render(game, adapter, { size: { columns: 80, rows: 24 } }),
    "Opening estimates",
  );
});

Deno.test("completed adapter rejects malformed direct finals and the wrong evaluation phase", () => {
  const game = p2FirstGame();
  const ts = new Search(game);
  const ranked = ts.candidates.map((candidate) => ({
    handIndex: candidate.index,
    pillz: candidate.pillz,
    fury: candidate.fury,
    score: 0,
    worst: 0,
    best: 0,
    samples: ts.samples,
    kos: 0,
    koed: 0,
    hiddenOutcomes: null,
  }));
  const base: RustAdvisorFinal = {
    kind: "final",
    sequence: 0,
    provenance,
    evaluationKind: "opening_estimate",
    complete: true,
    unitsDone: ts.units,
    unitsTotal: ts.units,
    elapsedMs: 0,
    mode: "first",
    opponentHandIndex: null,
    ranked,
  };
  assertThrows(
    () =>
      new CompletedRustSearch(game, {
        ...base,
        evaluationKind: "exact_continuation_policy",
      }),
    RustAdvisorProtocolError,
  );
  assertThrows(
    () =>
      new CompletedRustSearch(game, {
        ...base,
        ranked: [{ ...ranked[0], score: Number.NaN }, ...ranked.slice(1)],
      }),
    RustAdvisorProtocolError,
  );
});

function secondGame() {
  const game = new Game(
    new Player(12, 3, 0),
    new Player(12, 3, 1),
    HandGenerator.handOf(["Genmaicha", "Orka", "Sando", "Deborah"]),
    HandGenerator.handOf(["Nathan", "El Kuzco", "Noon Steevens", "Strygia"]),
    Turn.PLAYER_1,
    false,
  );
  game.select(0, 0, false, false);
  return game;
}

Deno.test("completed Rust SECOND adapter restores hidden-wager outcomes for the read panel", () => {
  const game = secondGame();
  const ts = new Search(game);
  assertEquals(ts.mode, SearchMode.SECOND);
  assertEquals(ts.us, Turn.PLAYER_2);
  const hiddenOutcomes = ts.opponentMoves.map((move, index) => ({
    pillz: move.pillz,
    fury: move.fury,
    score: [-1, 0, 1][index % 3]!,
    flags: index === 0 ? 1 : index === 1 ? 2 : 0,
  }));
  const totalWeight = hiddenOutcomes.reduce(
    (sum, outcome) =>
      sum + openingReplyWeight({
        index: 0,
        pillz: outcome.pillz,
        fury: outcome.fury,
      }),
    0,
  );
  const average = hiddenOutcomes.reduce(
    (sum, outcome) =>
      sum + outcome.score * openingReplyWeight({
          index: 0,
          pillz: outcome.pillz,
          fury: outcome.fury,
        }),
    0,
  ) / totalWeight;
  const ranked = ts.candidates.map((candidate) => ({
    handIndex: candidate.index,
    pillz: candidate.pillz,
    fury: candidate.fury,
    score: average,
    worst: Math.min(...hiddenOutcomes.map((outcome) => outcome.score)),
    best: Math.max(...hiddenOutcomes.map((outcome) => outcome.score)),
    samples: ts.samples,
    kos: hiddenOutcomes.filter((outcome) => (outcome.flags & 1) !== 0).length,
    koed: hiddenOutcomes.filter((outcome) => (outcome.flags & 2) !== 0).length,
    hiddenOutcomes,
  }));
  const final: RustAdvisorFinal = {
    kind: "final",
    sequence: 0,
    provenance,
    evaluationKind: "opening_estimate",
    complete: true,
    unitsDone: ts.units,
    unitsTotal: ts.units,
    elapsedMs: 1,
    mode: "second",
    opponentHandIndex: 0,
    ranked,
  };
  const adapter = new CompletedRustSearch(game, final);
  const outcome = adapter.outcome(
    adapter.candidates[0],
    adapter.opponentMoves[0],
  );
  assertEquals(outcome, { value: 1, ko: true, koed: false });
  assertStringIncludes(
    render(game, adapter, { size: { columns: 100, rows: 30 } }),
    "answering",
  );
  assertThrows(
    () =>
      new CompletedRustSearch(game, {
        ...final,
        ranked: [{
          ...final.ranked[0],
          hiddenOutcomes: [{
            ...final.ranked[0].hiddenOutcomes![0],
            score: 1,
          }, ...final.ranked[0].hiddenOutcomes!.slice(1)],
        }, ...final.ranked.slice(1)],
      }),
    RustAdvisorProtocolError,
  );
});

Deno.test("completed Rust BLIND_SECOND adapter keeps the provisional view shape", () => {
  const game = p2FirstGame();
  const ts = new Search(game, 1, 0, true);
  assertEquals(ts.mode, SearchMode.BLIND_SECOND);
  const adapter = new CompletedRustSearch(game, {
    kind: "final",
    sequence: 0,
    provenance,
    evaluationKind: "opening_estimate",
    complete: true,
    unitsDone: ts.units,
    unitsTotal: ts.units,
    elapsedMs: 1,
    mode: "blind_second",
    opponentHandIndex: null,
    ranked: ts.candidates.map((candidate) => ({
      handIndex: candidate.index,
      pillz: candidate.pillz,
      fury: candidate.fury,
      score: 0,
      worst: 0,
      best: 0,
      samples: ts.samples,
      kos: 0,
      koed: 0,
      hiddenOutcomes: null,
    })),
  });
  assertEquals(adapter.mode, SearchMode.BLIND_SECOND);
  assertStringIncludes(
    render(game, adapter, { size: { columns: 100, rows: 30 } }),
    "opponent choosing",
  );
});
