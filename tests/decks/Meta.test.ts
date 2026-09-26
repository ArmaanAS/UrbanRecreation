import { assertEquals } from "@std/assert";
import { formatMeta, type GameRecord } from "@/decks/Meta.ts";

const hand = (...ids: number[]) => ids.map((id) => ({ id, level: 3, clan: id < 100 ? "Rescue" : "Skeelz" }));
const game = (id: number, format: number, mine: number[], theirs: number[], over: Partial<GameRecord> = {}): GameRecord => ({
  id,
  capturedAt: `2026-09-${10 + id}T00:00:00Z`,
  mySide: 0,
  room: { idDeckFormat: format },
  players: [{ side: 0, id: 7, hand: hand(...mine) }, { side: 1, id: 8, hand: hand(...theirs) }],
  ...over,
});

Deno.test("only opposing hands of the chosen format count", () => {
  const games = [
    game(1, 54363, [1, 2, 3, 4], [101, 102, 103, 1]),
    game(2, 54363, [1, 2, 3, 4], [101, 104, 105, 106]),
    game(3, 1, [1, 2, 3, 4], [101, 107, 108, 109]), // EFC: another format
  ];
  const meta = formatMeta(games, 54363);
  assertEquals(meta.hands, 2);
  assertEquals(meta.cards[0], { id: 101, count: 2, levels: { "3": 2 } });
  // The owner's own cards never count, even when an opponent plays the same card.
  assertEquals(meta.cards.find((c) => c.id === 1)?.count, 1);
  assertEquals(meta.clans, [{ clan: "Skeelz", count: 7 }, { clan: "Rescue", count: 1 }]);
  assertEquals([meta.from, meta.to], ["2026-09-11T00:00:00Z", "2026-09-12T00:00:00Z"]);
});

Deno.test("an old capture without mySide is read through the owner's id", () => {
  const old = game(4, 54363, [1, 2, 3, 4], [101, 102, 103, 104], { mySide: null, myId: undefined });
  assertEquals(formatMeta([old], 54363).hands, 0);
  assertEquals(formatMeta([old], 54363, 7).hands, 1);
});
