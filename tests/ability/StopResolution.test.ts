// Stop effects have to resolve from their dependencies, not from which internal Player
// object owns a card. Captures 867116 and 1090338 exercise opposite player orders.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

Deno.test("an ability stops an opposing Stop Bonus before that bonus can fire", () => {
  // Captured battle 867116 r1: P2's Bonnie moves first, but its Piranas Stop Bonus is
  // itself stopped by Miyo's ability. Miyo's Hive bonus therefore reduces Bonnie to 5.
  const g = new Game(
    new Player(6, 9, 0),
    new Player(6, 9, 1),
    HandGenerator.handOf(
      ["Aegis Cr", "Lumia Cr", "Miyo", "Uuber"] as HandOf<string>,
      [5, 4, 3, 2] as HandOf<number | undefined>,
    ),
    HandGenerator.handOf(
      ["Bonnie Ld", "Lagertha Cr", "Blackie", "Miss Demonink"] as HandOf<
        string
      >,
      [1, 4, 3, 3] as HandOf<number | undefined>,
    ),
    Turn.PLAYER_2,
  );

  g.select(0, 0, false, false); // P2 Bonnie Ld, bonus: Stop Opp. Bonus
  g.select(2, 0, false, false); // P1 Miyo, ability: Stop Opp. Bonus

  assertEquals(g.h1[2].attack.final, 6);
  assertEquals(g.h1[2].won, true);
  assertEquals(g.h2[0].attack.final, 5);
  assertEquals(g.h2[0].won, false);
});
