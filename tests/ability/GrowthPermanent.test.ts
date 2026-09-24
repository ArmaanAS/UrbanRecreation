// Growth: Heal N Max. M - a Growth permanent's amount is fixed by the round it latches in and
// is not rescaled by every later round. Captured battle 1414168: Abby Salia wins round 2 and
// the server heals 2 in rounds 3 and 4 alike (post-round quantity 2 both times), where the
// engine used to heal 3 and then 4. The server's text for every Growth permanent says the
// same: "multiplied by the number of the round in which <card> has won". Ordinary Growth
// keeps scaling by the current round - Wooly's "Growth: Power +1" in the same battle.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

// Abby Salia lv5 (Ulu Watu, 8/5, "Growth: Heal 1 Max. 12", bonus Power +2) against a
// Skeelz hand that cannot touch Life: Corvus Cr's +3 Pillz, two Damage reductions and a
// Support Damage reduction.
const game = () =>
  new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(["H4rp3r", "Abby Salia", "Stanly", "Wooly"] as HandOf<string>, [1, 5, 5, 5] as HandOf<number | undefined>),
    HandGenerator.handOf(["Corvus Cr", "Deebler", "Dwan", "Henry"] as HandOf<string>, [4, 3, 5, 3] as HandOf<number | undefined>),
    Turn.PLAYER_1,
    false,
    true,
  );

Deno.test("Growth Heal latched in round 1 heals 1 in every later round", () => {
  const g = game();

  g.select(1, 5, false, false); // p1 Abby, 10 x 6 = 60
  g.select(1, 0, false, false); // p2 Deebler, 6 x 1 = 6
  assertEquals(g.h1[1].won, true);
  assertEquals(g.p1.life, 12); // Heal skips the round that started it

  g.select(2, 7, false, false); // p2 Dwan, 7 x 8 = 56, deals 6
  g.select(0, 0, false, false); // p1 H4rp3r, 7 x 1 = 7 - loses, so its Poison never starts
  assertEquals(g.h2[2].won, true);
  assertEquals(g.p1.life, 7); // 12 - 6 + 1, not + 2

  g.select(2, 0, false, false); // p1 Stanly, 8 x 1 = 8 (no Confidence: p1 lost round 2)
  g.select(0, 1, false, false); // p2 Corvus Cr, 8 x 2 = 16, deals 3
  assertEquals(g.h2[0].won, true);
  assertEquals(g.p1.life, 5); // 7 - 3 + 1, not + 3
});

Deno.test("Growth Heal latched in round 2 heals 2 in every later round", () => {
  const g = game();

  // The opening of captured battle 1414168.
  g.select(0, 3, false, false); // p1 H4rp3r
  g.select(0, 5, false, false); // p2 Corvus Cr wins, deals 3
  assertEquals(g.p1.life, 9);

  g.select(1, 0, false, false); // p2 Deebler
  g.select(1, 1, false, false); // p1 Abby wins round 2
  assertEquals(g.h1[1].won, true);
  assertEquals(g.p1.life, 9);

  g.select(2, 2, false, false); // p1 Stanly
  g.select(2, 7, false, false); // p2 Dwan wins, deals 6
  assertEquals(g.p1.life, 5); // 9 - 6 + 2, not + 3

  g.select(3, 3, false, false); // p2 Henry
  g.select(3, 3, true, false); // p1 Wooly wins, with fury
  assertEquals(g.h1[3].won, true);
  assertEquals(g.p1.life, 7); // 5 + 2, not + 4
});
