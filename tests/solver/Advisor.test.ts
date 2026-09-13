import { assertEquals, assertNotEquals } from "@std/assert";
import {
  buildPosition,
  parseArgs,
  positionKey,
  writeAllSync,
} from "@/solver/Advisor.ts";
import Search from "@/solver/Search.ts";

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
