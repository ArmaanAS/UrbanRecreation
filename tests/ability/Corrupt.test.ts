// Corrupt — captures/abilities.json 5286 (Nega D Ld lv5, "Corrupt 2 Min. 5"): whether the
// card wins or loses, its owner's own Life drops by 2 at the end of the round, never below
// 5 (sideAffected "player", Life decrease, currentRoundRequirement "any"). Both captured
// rounds have Nega winning a knockout round with its owner on 6, and the server posting a
// Life decrease of exactly 1 for that owner: the Min binds. The engine used to ignore the
// text entirely and left the owner on 6.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

type Move = [number, number, boolean];

/** Play captured rounds; `moves` are [first mover, second mover] in engine pillz. */
function play(
  cards: string[],
  levels: number[],
  moves: [Move, Move][],
) {
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(cards.slice(0, 4) as HandOf<string>, levels.slice(0, 4) as HandOf<number | undefined>),
    HandGenerator.handOf(cards.slice(4, 8) as HandOf<string>, levels.slice(4, 8) as HandOf<number | undefined>),
    Turn.PLAYER_1,
    false,
    true, // both battles were played at night
  );
  for (const [first, second] of moves) {
    g.select(...first, false);
    g.select(...second, false);
  }
  return g;
}

Deno.test("Corrupt costs its owner Life down to its Min (captured 1065308 r2)", () => {
  const g = play(
    ["Dorian Cr", "Gradymag", "Maurice", "Nega D Ld", "Anita", "Lothar", "Spidee", "Tina"],
    [5, 5, 4, 5, 3, 2, 4, 3],
    [
      [[0, 3, false], [2, 7, false]],
      [[1, 0, false], [2, 1, false]],
      [[3, 5, true], [3, 5, false]],
    ],
  );

  const [nega, tina] = [g.h1[3], g.h2[3]];
  assertEquals([nega.attack.final, tina.attack.final], [48, 44]);
  assertEquals(nega.won, true);
  // Tina's owner is knocked out (7 - 10); Nega's owner pays 1 of the 2, stopping at 5.
  assertEquals([g.p1.life, g.p2.life], [5, 0]);
});

Deno.test("Corrupt costs its owner Life down to its Min (captured 1066210 r2)", () => {
  const g = play(
    ["Dorian Cr", "Gradymag", "Joan Cena", "Nega D Ld", "Anita", "Aurora", "Spidee", "Sue"],
    [5, 5, 3, 5, 3, 5, 4, 2],
    [
      [[2, 0, false], [2, 5, false]],
      [[0, 0, false], [1, 1, false]],
      [[3, 7, true], [1, 4, true]],
    ],
  );

  const [nega, aurora] = [g.h1[3], g.h2[1]];
  assertEquals([nega.attack.final, aurora.attack.final], [64, 37]);
  assertEquals(nega.won, true);
  // Aurora's owner is knocked out (5 - 10) and her +3 Life is a Victory effect; Nega's
  // owner goes 6 -> 5.
  assertEquals([g.p1.life, g.p2.life], [5, 0]);
});
