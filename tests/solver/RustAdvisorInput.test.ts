import { assert, assertEquals, assertStringIncludes } from "@std/assert";
import Game from "@/game/Game.ts";
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import type { HandOf } from "@/game/types/CardTypes.ts";
import { Turn } from "@/game/types/Types.ts";
import {
  normaliseRustAdvisorInput,
  readRustV1Provenance,
  rustV1Provenance,
} from "@/solver/RustAdvisorInput.ts";
import { SearchMode } from "@/solver/Search.ts";

const fixture = JSON.parse(
  await Deno.readTextFile(
    new URL("../../captures/games/1024673.json", import.meta.url),
  ),
);

function freshGame(rec: typeof fixture) {
  const tc = rec.testcase!;
  return new Game(
    new Player(tc.life, tc.pillz, 0),
    new Player(tc.life, tc.pillz, 1),
    HandGenerator.handOf(
      tc.cards.slice(0, 4) as HandOf<string>,
      tc.levels.slice(0, 4) as HandOf<number>,
    ),
    HandGenerator.handOf(
      tc.cards.slice(4, 8) as HandOf<string>,
      tc.levels.slice(4, 8) as HandOf<number>,
    ),
    Turn.PLAYER_1,
    false,
    tc.night,
  );
}

function openingCapture() {
  const rec = structuredClone(fixture);
  rec.rounds = [];
  rec.testcase.moves = [];
  rec.result = null;
  rec.finalStatus = "playing";
  return rec;
}

function afterFirstRoundCapture() {
  const rec = structuredClone(fixture);
  const firstMove = rec.testcase.moves[0];
  rec.rounds = rec.rounds.slice(0, 1);
  rec.testcase.moves = [firstMove];
  rec.result = null;
  rec.finalStatus = "playing";
  // Side 0 opens round two.  This pins the valid P2 wire orientation.
  rec.mySide = 0;
  return rec;
}

function afterFirstRoundGame() {
  const game = freshGame(openingCapture());
  const firstMove = fixture.testcase.moves[0];
  game.select(firstMove.s1[0], firstMove.s1[1], firstMove.s1[2], false);
  game.select(firstMove.s2[0], firstMove.s2[1], firstMove.s2[2], false);
  return game;
}

function emptyCurrentRound(round = 1) {
  const current = structuredClone(fixture.rounds[1]);
  current.round = round;
  current.first = null;
  current.moves = [];
  current.resolution = [null, null];
  current.life = [Number.NaN, Number.NaN];
  current.pillz = [Number.NaN, Number.NaN];
  return current;
}

function partialCurrentRoundCapture() {
  const rec = afterFirstRoundCapture();
  // Side 0 is the scheduled round-two first mover, so side 1 asks second.
  rec.mySide = 1;
  const current = emptyCurrentRound();
  current.first = 0;
  current.moves = [
    structuredClone(
      fixture.rounds[1].moves.find((move: { side: number }) => move.side === 0),
    ),
  ];
  rec.rounds.push(current);
  return rec;
}

function afterFirstRoundSecondGame() {
  const game = afterFirstRoundGame();
  const move = fixture.rounds[1].moves.find((candidate: { side: number }) =>
    candidate.side === 0
  )!;
  game.select(move.index, move.pillz, move.fury, false);
  return game;
}

async function requestAtRoundTwo(
  rec = afterFirstRoundCapture(),
  game = afterFirstRoundGame(),
) {
  return await normaliseRustAdvisorInput({
    rec,
    game,
    decision: { mode: SearchMode.FIRST, us: Turn.PLAYER_2 },
    requestId: "capture-1024673-round-2",
    budgetMs: 20,
  });
}

function assertUnsupported(
  result: Awaited<ReturnType<typeof normaliseRustAdvisorInput>>,
  text: string,
) {
  assert(!result.supported);
  assertStringIncludes(result.reason, text);
}

function gameState(game: Game) {
  return {
    round: game.round,
    turn: game.turn,
    firstHasSelected: game.firstHasSelected,
    playedCardIndex: game.firstHasSelected ? game.playedCardIndex : null,
    p1: {
      life: game.p1.life,
      pillz: game.p1.pillz,
      played: game.h1.map((card) => card.played),
    },
    p2: {
      life: game.p2.life,
      pillz: game.p2.pillz,
      played: game.h2.map((card) => card.played),
    },
  };
}

Deno.test("Rust V3 normalises 1024673's opening into engine P1 and preserves null identities", async () => {
  const rec = openingCapture();
  const result = await normaliseRustAdvisorInput({
    rec,
    game: freshGame(rec),
    decision: { mode: SearchMode.FIRST, us: Turn.PLAYER_1 },
    requestId: "capture-1024673-opening",
    budgetMs: 20,
  });
  if (!result.supported) throw new Error(result.reason);
  const { request } = result;
  assertEquals(request.us, "p1");
  assertEquals(request.first_mover, "p1");
  assertEquals(request.protocol_version, 3);
  assertEquals(request.mode, "first");
  assertEquals("opponent_hand_index" in request, false);
  assertEquals(request.players.p1.hand.map((card) => card.id), [
    1983,
    1985,
    1986,
    2179,
  ]);
  assertEquals(request.players.p2.hand.map((card) => card.id), [
    189,
    413,
    2349,
    1962,
  ]);
  assertEquals(
    request.players.p1.hand[2] as unknown,
    {
      id: 1986,
      level: 3,
      ability_id: 0,
      ability: "No Ability",
      bonus_id: 1844,
      bonus: "Asymmetry: Damage +3",
    },
  );
  assertEquals(request.players.p1.current, { life: 12, pillz: 12 });
  assertEquals(request.players.p2.current, { life: 12, pillz: 12 });
  assertEquals(request.history, []);
});

Deno.test("Rust V3 maps 1024673's round two into P2-first history and server resources", async () => {
  const result = await requestAtRoundTwo();
  if (!result.supported) throw new Error(result.reason);
  const { request } = result;
  assertEquals(request.us, "p2");
  assertEquals(request.first_mover, "p2");
  assertEquals(request.mode, "first");
  assertEquals(request.players.p1.current, { life: 10, pillz: 12 });
  assertEquals(request.players.p2.current, { life: 12, pillz: 9 });
  assertEquals(request.players.p1.played, [false, false, false, true]);
  assertEquals(request.players.p2.played, [true, false, false, false]);
  assertEquals(request.history, [{
    first_mover: "p1",
    p1: { hand_index: 3, pillz: 0, fury: false },
    p2: { hand_index: 0, pillz: 3, fury: false },
  }]);
});

Deno.test("Rust V3 provenance includes exact inputs and semantic revisions", async () => {
  const expected = {
    effectiveCatalogFingerprintFnv1a64: "95774366ab5ee807",
    effectRegistryFingerprintFnv1a64: "e63d83a094b9b6d2",
    effectRegistrySchemaVersion: 1,
    compilerPolicySemanticRevision: 33,
    catalogContextPolicySemanticRevision: 3,
    advisorPolicySemanticRevision: 2,
  };
  assertEquals(await readRustV1Provenance(), expected);
  assertEquals(await rustV1Provenance(), expected);
});

Deno.test("Rust V3 normalises the revealed first card for SECOND without adding it to history", async () => {
  const rec = partialCurrentRoundCapture();
  const game = afterFirstRoundSecondGame();
  const before = gameState(game);
  const result = await normaliseRustAdvisorInput({
    rec,
    game,
    decision: { mode: SearchMode.SECOND, us: Turn.PLAYER_1 },
    requestId: "capture-1024673-round-2-second",
    budgetMs: 20,
  });
  if (!result.supported) throw new Error(result.reason);
  const firstMove = fixture.rounds[1].moves.find((move: { side: number }) =>
    move.side === 0
  )!;
  assertEquals(result.request.protocol_version, 3);
  assertEquals(result.request.mode, "second");
  assertEquals(result.request.us, "p1");
  assertEquals(result.request.first_mover, "p2");
  assertEquals(result.request.opponent_hand_index, firstMove.index);
  assertEquals(result.request.history.length, 1);
  // Wire history deliberately excludes the current partial selection. The worker binds
  // that revealed slot through opponent_hand_index after reconstructing this clean root.
  assertEquals(result.request.players.p2.played[firstMove.index], false);
  assertEquals(gameState(game), before);
});

Deno.test("Rust V3 normalises BLIND_SECOND after a completed round without a current move", async () => {
  const rec = afterFirstRoundCapture();
  rec.mySide = 1;
  const current = emptyCurrentRound();
  current.first = 0;
  rec.rounds.push(current);
  const game = afterFirstRoundGame();
  const before = gameState(game);
  const result = await normaliseRustAdvisorInput({
    rec,
    game,
    decision: { mode: SearchMode.BLIND_SECOND, us: Turn.PLAYER_1 },
    requestId: "capture-1024673-round-2-blind",
    budgetMs: 20,
  });
  if (!result.supported) throw new Error(result.reason);
  assertEquals(result.request.protocol_version, 3);
  assertEquals(result.request.mode, "blind_second");
  assertEquals(result.request.us, "p1");
  assertEquals(result.request.first_mover, "p2");
  assertEquals("opponent_hand_index" in result.request, false);
  assertEquals(result.request.history.length, 1);
  assertEquals(gameState(game), before);
});

Deno.test("Rust V3 accepts a live-shaped empty current round", async () => {
  const rec = afterFirstRoundCapture();
  const current = emptyCurrentRound();
  current.first = 0; // reconstruct() exposes the scheduled round-two mover before a play.
  rec.rounds.push(current);
  const result = await requestAtRoundTwo(rec);
  assert(result.supported, result.supported ? "" : result.reason);
  assertEquals(result.request.history.length, 1);
});

Deno.test("Rust V3 rejects a partial current round and chronology tails", async () => {
  const partial = afterFirstRoundCapture();
  const current = emptyCurrentRound();
  current.first = 0;
  current.moves = [structuredClone(fixture.rounds[1].moves[0])];
  partial.rounds.push(current);
  assertUnsupported(
    await requestAtRoundTwo(partial),
    "first-mode capture has a selected",
  );

  const tail = afterFirstRoundCapture();
  tail.rounds.push(emptyCurrentRound(), structuredClone(fixture.rounds[1]));
  assertUnsupported(await requestAtRoundTwo(tail), "later round is populated");

  const contradictory = afterFirstRoundCapture();
  const empty = emptyCurrentRound();
  empty.first = 1;
  contradictory.rounds.push(empty);
  assertUnsupported(
    await requestAtRoundTwo(contradictory),
    "contradictory first mover",
  );
});

Deno.test("Rust V3 rejects contradictory SECOND and BLIND current states without mutating the game", async () => {
  const secondRec = partialCurrentRoundCapture();
  const unselected = afterFirstRoundGame();
  assertUnsupported(
    await normaliseRustAdvisorInput({
      rec: secondRec,
      game: unselected,
      decision: { mode: SearchMode.SECOND, us: Turn.PLAYER_1 },
      requestId: "second-without-engine-selection",
      budgetMs: 20,
    }),
    "second mode requires",
  );

  const wrongCard = afterFirstRoundGame();
  const firstMove = fixture.rounds[1].moves.find((move: { side: number }) =>
    move.side === 0
  )!;
  wrongCard.select((firstMove.index + 1) % 4, 0, false, false);
  assertUnsupported(
    await normaliseRustAdvisorInput({
      rec: secondRec,
      game: wrongCard,
      decision: { mode: SearchMode.SECOND, us: Turn.PLAYER_1 },
      requestId: "second-wrong-card",
      budgetMs: 20,
    }),
    "selected card does not match",
  );

  const blindRec = afterFirstRoundCapture();
  blindRec.mySide = 1;
  const current = emptyCurrentRound();
  current.first = 0;
  blindRec.rounds.push(current);
  const selected = afterFirstRoundSecondGame();
  assertUnsupported(
    await normaliseRustAdvisorInput({
      rec: blindRec,
      game: selected,
      decision: { mode: SearchMode.BLIND_SECOND, us: Turn.PLAYER_1 },
      requestId: "blind-with-engine-selection",
      budgetMs: 20,
    }),
    "blind-second mode cannot",
  );
  assertUnsupported(
    await normaliseRustAdvisorInput({
      rec: openingCapture(),
      game: freshGame(openingCapture()),
      decision: { mode: SearchMode.BLIND_SECOND, us: Turn.PLAYER_2 },
      requestId: "blind-opening",
      budgetMs: 20,
    }),
    "requires at least one completed round",
  );
});

Deno.test("Rust V3 rejects a first-mode decision when the capture owner is not first", async () => {
  const rec = openingCapture();
  rec.mySide = 0;
  const result = await normaliseRustAdvisorInput({
    rec,
    game: freshGame(rec),
    decision: { mode: SearchMode.FIRST, us: Turn.PLAYER_1 },
    requestId: "not-first",
    budgetMs: 20,
  });
  assertUnsupported(result, "not the current first mover");
});

Deno.test("Rust V3 forwards authoritative server resources despite TS replay drift", async () => {
  const game = afterFirstRoundGame();
  // This deliberately differs from the server's end-of-round totals.  Eligibility uses
  // the capture totals; the worker's strict replay remains responsible for rejecting a
  // model that cannot produce them.
  game.p1.life = 1;
  game.p2.pillz = 1;
  const result = await requestAtRoundTwo(afterFirstRoundCapture(), game);
  if (!result.supported) throw new Error(result.reason);
  assertEquals(result.request.players.p1.current, { life: 10, pillz: 12 });
  assertEquals(result.request.players.p2.current, { life: 12, pillz: 9 });
});
