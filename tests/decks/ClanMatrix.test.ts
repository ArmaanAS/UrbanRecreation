import { assert, assertAlmostEquals, assertEquals, assertNotEquals } from "@std/assert";
import { cellPairs, clanGroups, clanMatrix, fieldHands, handClan, type MatrixGameRecord } from "@/decks/ClanMatrix.ts";
import type { MatchupRequest, MatchupResponse, MatchupRunner, SolveRequest } from "@/decks/Matchup.ts";

const hand = (clan: string, other: string, base: number, mates = 3) =>
  Array.from({ length: 4 }, (_, i) => ({ id: base + i, level: 2, clan: i < mates ? clan : other }));
let nextId = 1;
const game = (mine: ReturnType<typeof hand>, theirs: ReturnType<typeof hand>, result: string | null, format = 54363): MatrixGameRecord => ({
  id: nextId++,
  mySide: 0,
  room: { idDeckFormat: format },
  result: result ? { result } : null,
  players: [{ side: 0, id: 7, hand: mine }, { side: 1, id: 100 + nextId, hand: theirs }],
});

Deno.test("a hand's clan is the one three of its cards share", () => {
  assertEquals(handClan(hand("Hive", "Rescue", 1)), "Hive");
  assertEquals(handClan(hand("Hive", "Hive", 1, 4)), "Hive");
  assertEquals(handClan(hand("Hive", "Rescue", 1, 2)), null);
});

Deno.test("field hands keep both sides of a format's games, with each side's result", () => {
  const games = [
    game(hand("Hive", "X", 10), hand("Rescue", "Y", 20), "win"),
    game(hand("Hive", "X", 10), hand("Riots", "Y", 30, 2), "draw"), // their hand has no clan
    game(hand("Hive", "X", 10), hand("Rescue", "Y", 20), null),
    game(hand("Hive", "X", 10), hand("Rescue", "Y", 20), "lose", 1), // another format
  ];
  const hands = fieldHands(games, 54363);
  assertEquals(hands.map((h) => [h.clan, h.owner, h.score]), [
    ["Hive", true, 1],
    ["Rescue", false, 0],
    ["Hive", true, 0.5],
    ["Hive", true, null],
    ["Rescue", false, null],
  ]);
  assertEquals(hands[0].hand[0], { id: 10, level: 2 }, "only id and level are kept");
  const groups = clanGroups(hands, 2);
  assertEquals(groups.map((g) => [g.clan, g.hands.length, g.ownerHands, g.players]), [["Hive", 3, 3, 1], ["Rescue", 2, 0, 2]]);
  assertEquals(clanGroups(hands, 3).map((g) => g.clan), ["Hive"]);
});

Deno.test("a cell's hands depend on its two clans and the seed, not on the other clans", () => {
  const games = [
    game(hand("Hive", "X", 10), hand("Rescue", "Y", 20), "win"),
    game(hand("Hive", "X", 40), hand("Rescue", "Y", 50), "win"),
    game(hand("Hive", "X", 70), hand("Riots", "Y", 80), "win"),
  ];
  const groups = clanGroups(fieldHands(games, 54363), 1);
  const [hive, rescue] = [groups.find((g) => g.clan === "Hive")!, groups.find((g) => g.clan === "Rescue")!];
  const pairs = cellPairs(hive, rescue, 12, 1);
  assertEquals(cellPairs(hive, rescue, 12, 1), pairs);
  assertNotEquals(cellPairs(hive, rescue, 12, 2), pairs);
  assert(pairs.every((p) => hive.hands.some((h) => h.hand === p.a) && rescue.hands.some((h) => h.hand === p.b)));
});

/** Scores a solve by the first card id of each hand: the lower id wins by the difference. */
const runner: MatchupRunner = {
  run(requests: readonly MatchupRequest[], onResponse?: (r: MatchupResponse, i: number) => void) {
    return Promise.resolve((requests as SolveRequest[]).map((r, id) => {
      const response: MatchupResponse = {
        id,
        kind: "solve",
        value: Math.tanh((r.p2[0][0] - r.p1[0][0]) / 50),
        worst: -1,
        best: 1,
        best_move: { hand_index: 0, pillz: 0, fury: false },
        ko_share: 0,
        koed_share: 0,
        root_moves: 1,
        replies: 1,
        ms: 1,
      };
      onResponse?.(response, id);
      return response;
    }));
  },
};

Deno.test("the matrix is antisymmetric by construction and ranks the clans by their cells", async () => {
  const games = [
    game(hand("Hive", "X", 10), hand("Rescue", "Y", 40), "win"),
    game(hand("Hive", "X", 12), hand("Riots", "Y", 70), "win"),
    game(hand("Rescue", "X", 41), hand("Riots", "Y", 71), "lose"),
  ];
  const groups = clanGroups(fieldHands(games, 54363), 1);
  const result = await clanMatrix(groups, { runner, perCell: 6, seed: 3 });
  assertEquals(result.cells.length, 3);
  assertEquals(result.clans.map((c) => c.clan), ["Hive", "Rescue", "Riots"], "lower ids win in this fake");
  const hive = result.clans[0];
  const cells = result.cells.filter((c) => c.a === "Hive" || c.b === "Hive").map((c) => (c.a === "Hive" ? c.mean : -c.mean));
  assertAlmostEquals(hive.vsClans, (cells[0] + cells[1]) / 2, 1e-12);
  // In practice: Hive won both its games; Rescue (the owner's side in game 3) lost both.
  assertEquals(hive.practice, { games: 2, score: 2 });
  assertEquals(result.clans[1].practice, { games: 2, score: 0 });
  assertEquals(result.clans[2].practice, { games: 2, score: 1 });
  const hiveRescue = result.cells.find((c) => c.a === "Hive" && c.b === "Rescue")!;
  assertEquals(hiveRescue.practice, { games: 1, score: 1 });
});
