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
import Search from "@/solver/Search.ts";
import { render } from "@/solver/SolverView.ts";
import {
  buildFirstRequest,
  CompletedRustSearch,
  decodeRustJsonl,
  runRustAdvisor,
  RustAdvisorCancelledError,
  type RustAdvisorFinal,
  RustAdvisorProtocolError,
  type RustAdvisorRunner,
  RustAdvisorTimeoutError,
  type RustFirstRequest,
} from "@/solver/RustAdvisor.ts";

const actions = [
  { index: 0, pillz: 0, fury: false },
  { index: 1, pillz: 1, fury: false },
];
const expectation = {
  version: 1 as const,
  requestId: "request-1",
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
});
const line = (
  kind: "progress" | "final",
  sequence: number,
  ranked_moves: unknown[],
  complete = kind === "final",
) =>
  JSON.stringify({
    protocol_version: 1,
    request_id: "request-1",
    sequence,
    kind,
    score_frame: "requester",
    evaluation_kind: "exact_continuation_policy",
    complete,
    units_done: complete ? 4 : 1,
    units_total: 4,
    elapsed_ms: sequence + 1,
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

const card = (id: number) => ({
  id,
  level: 1,
  abilityId: 0,
  ability: "",
  bonusId: 0,
  bonus: "",
});

const firstInput = () => ({
  requestId: "request-1",
  us: "p1",
  firstMover: "p1",
  battleRuleId: 10,
  night: false,
  provenance: {
    effectiveCatalogFingerprintFnv1a64: "0000000000000000",
    effectRegistryFingerprintFnv1a64: "0000000000000000",
    effectRegistrySchemaVersion: 1,
  },
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

Deno.test("Rust V1 request builder emits only the strict worker shape", () => {
  const built = request();
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
  }));
  const final: RustAdvisorFinal = {
    kind: "final",
    sequence: 0,
    evaluationKind: "opening_estimate",
    complete: true,
    unitsDone: ts.units,
    unitsTotal: ts.units,
    elapsedMs: 3,
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
  }));
  const base: RustAdvisorFinal = {
    kind: "final",
    sequence: 0,
    evaluationKind: "opening_estimate",
    complete: true,
    unitsDone: ts.units,
    unitsTotal: ts.units,
    elapsedMs: 0,
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
