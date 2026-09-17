// Handing the opponent Pillz does not require them to have any: the engine's non-empty
// pool check only exists so that taking Pillz cannot drive the counter negative. Battle
// 1130425 r2: Pr SenQ loses on 0 Life, and its "Defeat: +1 Opp. Pillz" still pays a
// Rescue player who has just spent their last three Pillz.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

Deno.test("Defeat: +1 Opp. Pillz pays a player on zero Pillz", () => {
  const g = new Game(
    new Player(15, 3, 0),
    new Player(3, 9, 1),
    HandGenerator.handOf(
      ["Anita", "Spidee", "Sue", "Tina"] as HandOf<string>,
      [3, 4, 2, 3] as HandOf<number | undefined>,
    ),
    HandGenerator.handOf(
      ["Argos", "Dr Van Wesel Ld", "Korakine", "Pr SenQ"] as HandOf<string>,
      [2, 1, 3, 2] as HandOf<number | undefined>,
    ),
    Turn.PLAYER_1,
  );

  g.select(2, 3, false, false); // Sue: 24 + Support 12 = 36 Attack
  g.select(3, 3, false, false); // Pr SenQ: 7 x 4 = 28 Attack after Sue's -1 Power

  assertEquals(g.h2[3].won, false);
  assertEquals(g.p2.life, 0);
  assertEquals(g.p1.pillz, 1); // spent the last 3, then Pr SenQ's Defeat pays 1
  assertEquals(g.p2.pillz, 7); // 9 - 3 + the Riots Pillz, which survives the KO
});
