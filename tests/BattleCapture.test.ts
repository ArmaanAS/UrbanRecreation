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

Deno.test("remote hover and pillz-chooser frames become absolute hand events", () => {
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
  assertEquals(frame(8, 7, "4"), [{
    battleId: 42,
    entry: { kind: "selecting", t: 8, side: 0, index: 3, active: true },
  }]);
  assertEquals(frame(9, 7, "4"), [], "duplicate chooser-open is suppressed");
  assertEquals(frame(10, 7, "6"), [
    {
      battleId: 42,
      entry: { kind: "selecting", t: 10, side: 0, index: 3, active: false },
    },
    {
      battleId: 42,
      entry: { kind: "selecting", t: 10, side: 1, index: 1, active: true },
    },
  ]);
  assertEquals(frame(11, 8, "4"), [], "a stale chooser-close is ignored");
  assertEquals(frame(12, 8, "6"), [{
    battleId: 42,
    entry: { kind: "selecting", t: 12, side: 1, index: 1, active: false },
  }]);
  assertEquals(frame(13, 8, "6"), [], "duplicate chooser-close is suppressed");
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

const STALE = "final round life/pillz may be stale: result could not be attributed to a side";
const loadCapture = async (id: number) => {
  const raw = await Deno.readTextFile(`captures/battles/${id}.jsonl`);
  return expandEntries(
    raw.split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line)),
    await loadAbilities(),
  );
};

Deno.test("an unattributed result is placed by the final round's own arithmetic", async () => {
  // 924740 was captured without my id, and its "done" snapshot predates round 2's damage.
  // Side 0 bets 3 of 7 and takes Lumia Cr's 6 on 4 Life; side 1 bets 4 of 4. Only
  // player = side 1 fits battles.result (player 12/0, opponent 0/4).
  const captured = reconstruct(924740, await loadCapture(924740));

  assertEquals(captured.mySide, null);
  assertEquals(captured.rounds.at(-1)?.life, [0, 12]);
  assertEquals(captured.rounds.at(-1)?.pillz, [4, 0]);
  assertEquals(captured.issues.includes(STALE), false);
});

Deno.test("an unattributed result credits post-round abilities to their player", async () => {
  // 924669 r3: Ashara wins for side 0, which then gains 2 Life (Versus) and 1 (a latched
  // Heal): 13 + 3 = 16, while side 1 falls from 2 to 0.
  const captured = reconstruct(924669, await loadCapture(924669));

  assertEquals(captured.rounds.at(-1)?.life, [16, 0]);
  assertEquals(captured.rounds.at(-1)?.pillz, [0, 0]);
  assertEquals(captured.issues.includes(STALE), false);
});

Deno.test("a result that fits neither side stays flagged", async () => {
  const entries = (await loadCapture(924740)).map((entry) =>
    entry.kind === "result"
      ? {
        ...entry,
        result: { ...entry.result, player: { ...entry.result.player, life: 11 } },
      } as CaptureEntry
      : entry
  );
  const captured = reconstruct(924740, entries);

  assertEquals(captured.rounds.at(-1)?.life, [4, 12]);
  assertEquals(captured.issues.includes(STALE), true);
});
