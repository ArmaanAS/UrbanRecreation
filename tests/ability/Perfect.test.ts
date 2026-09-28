// `Perfect:` holds when its card wins "with the exact number of pillz needed" (captures/
// abilities.json 5362 for Cyloxxt): it won, and one pill fewer would not have. The Attack on one
// pill fewer goes through the same Attack modifiers as the real one (`BattleData.fewerAttack`),
// so a Min clamp that holds the real Attack holds it too.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

const hand = (names: string[], levels: number[]) =>
  HandGenerator.handOf(names as HandOf<string>, levels as HandOf<number | undefined>);

Deno.test("A pill the win did not need is not Perfect, under a Min clamp (1525735 r2)", () => {
  // Cyloxxt lv1 (8/1) on 1 pill and Fury is 16, cut to 6 by the Uppers `-10 Opp Attack, Min
  // 3`; Sabrina lv5 with Courage is 9, cut to 3 by the Sakrohm `-8 Opp Attack, Min 3`. On no
  // pill Cyloxxt is 8, held at 3: a tie his single star wins, so the pill was not needed and
  // `Perfect: -5 Opp. Life Min 0` pays nothing. `attack - power` gave 6 - 8 = -2 and paid.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Dorian Cr", "MFK", "Nega D Ld", "Sabrina"], [5, 5, 5, 5]),
    hand(["Cyloxxt", "Jautya", "Virginia", "Zinzinxxt"], [1, 4, 5, 1]),
    Turn.PLAYER_1,
    false,
    true,
  );
  g.select(3, 0, false, false); // P1 Sabrina lv5, first: Courage
  g.select(0, 1, true, false); // P2 Cyloxxt lv1, Fury
  assertEquals([g.h1[3].attack.final, g.h2[0].attack.final], [3, 6]);
  assertEquals(g.h2[0].won, true);
  assertEquals(g.p1.life, 12); // 1 Damage + 2 Fury, no Perfect
});

Deno.test("A pill the win needed is Perfect (1522747 r1)", () => {
  // Cyloxxt lv2 (9/1) on 2 pills is 27, cut to 17 by the Uppers bonus; Colin lv2 with Courage
  // is 6 x 3 = 18, cut to 10. On one pill Cyloxxt would be 18 - 10 = 8 and lose, so the win
  // was exact and the -5 pays: 15 - 1 - 5 = 9.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Elias Renko", "Spidee", "Cosmos", "Cyloxxt"], [2, 4, 4, 2]),
    hand(["Colin", "El D10S", "Gail Ld", "McArthur"], [2, 3, 2, 1]),
    Turn.PLAYER_1,
    false,
    true,
  );
  g.select(0, 0, false, false); // P1 Elias Renko lv2
  g.select(3, 0, false, false); // P2 McArthur lv1
  assertEquals(g.p2.life, 15);
  g.select(0, 2, false, false); // P2 Colin lv2, first: Courage
  g.select(3, 2, false, false); // P1 Cyloxxt lv2
  assertEquals([g.h1[3].attack.final, g.h2[0].attack.final], [17, 10]);
  assertEquals(g.p2.life, 9);
});
