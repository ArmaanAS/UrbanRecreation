// "Bet > N Pillz: ..." is a condition on the bet, not decoration: the server's own wording
// is "strictly greater than N, including free Pillz and excluding Fury"
// (captures/abilities.json 4893, 4949). Battle 1207064 r0 has Tyd win on five Pillz and
// gain nothing, and 1131144 r1 has Ilarius leave Rescue's Support bonus standing on a
// zero-Pillz bet.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

/** Battle 1207064 r0: Tyd's "Bet > 6 Pillz: +2 Life" against Bubbles. */
const piranasGame = () =>
  new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    HandGenerator.handOf(
      ["Goldie", "Morgane", "Segar", "Tyd"] as HandOf<string>,
      [3, 3, 3, 3] as HandOf<number | undefined>,
    ),
    HandGenerator.handOf(
      ["Bubbles", "Joy", "Rocket", "Wilde"] as HandOf<string>,
      [4, 4, 3, 4] as HandOf<number | undefined>,
    ),
    Turn.PLAYER_2,
  );

/** Battle 1131144 r1: Ilarius' "Bet > 4 Pillz: Stop Opp. Bonus" against Callie. */
const paradoxGame = () =>
  new Game(
    new Player(14, 12, 0),
    new Player(14, 12, 1),
    HandGenerator.handOf(
      ["Demusa", "Ilarius", "Lucanine", "Tinomor"] as HandOf<string>,
      [2, 3, 3, 3] as HandOf<number | undefined>,
    ),
    HandGenerator.handOf(
      ["Anita", "Aurora", "Callie", "Tina"] as HandOf<string>,
      [3, 5, 3, 3] as HandOf<number | undefined>,
    ),
    Turn.PLAYER_1,
  );

Deno.test("a bet equal to the threshold does not trigger", () => {
  const g = piranasGame();

  g.select(0, 5, false, false); // Bubbles: 36 Attack
  g.select(3, 5, false, false); // Tyd: 48 Attack, six Pillz counting the free one

  assertEquals(g.h1[3].won, true);
  assertEquals(g.p1.life, 15); // no +2: 5 + 1 is not > 6
  assertEquals(g.p2.life, 11);
});

Deno.test("one more Pillz triggers it", () => {
  const g = piranasGame();

  g.select(0, 5, false, false); // Bubbles: 36 Attack
  g.select(3, 6, false, false); // Tyd: 56 Attack, seven Pillz counting the free one

  assertEquals(g.h1[3].won, true);
  assertEquals(g.p1.life, 17); // 15 + 2
});

Deno.test("Fury does not count towards the bet", () => {
  const g = piranasGame();

  g.select(0, 5, false, false); // Bubbles: 36 Attack
  g.select(3, 5, true, false); // Tyd: 48 Attack, nine Pillz paid, six of them bet

  assertEquals(g.h1[3].won, true);
  assertEquals(g.p1.life, 15); // still no +2
  assertEquals(g.p2.life, 9); // 15 - (4 + 2 Fury)
  assertEquals(g.p1.pillz, 4); // 12 - 5 - 3 Fury
});

Deno.test("a bet below the threshold leaves the opposing bonus alone", () => {
  const g = paradoxGame();

  g.select(1, 0, false, false); // Ilarius: one Pillz, so no Stop Opp. Bonus
  g.select(2, 5, false, false); // Callie

  assertEquals(g.h2[2].attack.final, 48); // 6 x 6 + Support 3 x 4
  assertEquals(g.h2[2].won, true);
});

Deno.test("a bet above the threshold stops the opposing bonus", () => {
  const g = paradoxGame();

  g.select(1, 5, false, false); // Ilarius: six Pillz, so Stop Opp. Bonus
  g.select(2, 5, false, false); // Callie

  assertEquals(g.h2[2].attack.final, 36); // 6 x 6, Support cancelled
});
