import { assertEquals } from "@std/assert";
import {
  type CaptureEntry,
  expandEntries,
  extractFromRecord,
  loadAbilities,
  newCaptureState,
} from "../scripts/BattleCapture.ts";
import { mergeCaptureMeta, reconstruct } from "../scripts/ExtractBattle.ts";

Deno.test("restart metadata repairs identity without erasing the known room", () => {
  const room = { id: 6, name: "Training", idBattleRule: 2, idDeckFormat: 0 };
  const entries = [
    { kind: "meta", t: 1, myId: 0, room },
    { kind: "meta", t: 2, myId: 19309601, room: undefined },
  ] as CaptureEntry[];

  assertEquals(mergeCaptureMeta(entries), { myId: 19309601, room });
});

Deno.test("remote card hover frames become deduplicated absolute hand events", () => {
  const state = newCaptureState();
  state.lastBattleId = 42;
  state.battleActive = true;
  const frame = (t: number, code: number, value: string) =>
    extractFromRecord({
      t,
      kind: "ws_in",
      payload: JSON.stringify({ code, values: [value] }),
    }, state);

  assertEquals(frame(1, 5, "6"), [{
    battleId: 42,
    entry: { kind: "hover", t: 1, side: 1, index: 1, active: true },
  }]);
  assertEquals(frame(2, 5, "6"), [], "the socket duplicates hover-enter");
  assertEquals(frame(3, 6, "6"), [{
    battleId: 42,
    entry: { kind: "hover", t: 3, side: 1, index: 1, active: false },
  }]);
  assertEquals(frame(4, 6, "6"), [], "the socket duplicates hover-leave");
  assertEquals(frame(5, 5, "6"), [{
    battleId: 42,
    entry: { kind: "hover", t: 5, side: 1, index: 1, active: true },
  }]);
  assertEquals(
    frame(6, 5, "3"),
    [
      {
        battleId: 42,
        entry: { kind: "hover", t: 6, side: 1, index: 1, active: false },
      },
      {
        battleId: 42,
        entry: { kind: "hover", t: 6, side: 0, index: 2, active: true },
      },
    ],
    "one remote mouse entering a new card clears a missing leave",
  );
  assertEquals(
    frame(7, 5, "9"),
    [],
    "only the eight battle-card slots are valid",
  );
});

Deno.test("a result finishes a forfeited battle without a done snapshot", async () => {
  const raw = await Deno.readTextFile("captures/battles/866431.jsonl");
  const entries = expandEntries(
    raw.split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line)),
    await loadAbilities(),
  );
  const meta = entries.find((entry) => entry.kind === "meta")!;
  const statuses = entries.filter((entry) => entry.kind === "status");
  const roundTwo = statuses.findIndex((entry) => entry.battle.round === 1);
  const result = entries.find((entry) => entry.kind === "result")!;
  const simulated = [
    meta,
    ...statuses.slice(0, roundTwo + 1),
    {
      ...result,
      result: {
        ...result.result,
        result: "lose",
        byKo: false,
        player: { ...result.result.player, life: 8, pillz: 2 },
        opponent: { ...result.result.opponent, life: 12, pillz: 6 },
      },
    },
  ] as CaptureEntry[];

  const captured = reconstruct(866431, simulated);

  assertEquals(captured.finalStatus, "playing");
  assertEquals(captured.result?.result, "lose");
  assertEquals(captured.testcase?.moves.length, 1);
  assertEquals(captured.rounds.at(-1)?.life, [8, 12]);
  assertEquals(captured.rounds.at(-1)?.pillz, [2, 6]);
});
