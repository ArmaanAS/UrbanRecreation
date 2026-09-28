// `-N Players Pillz. Min M` reduces both players' Pillz when its card wins: "If D4ggers wins the
// round, the number of Pillz of the two competing players will be reduced by 2 Pillz, or up to a
// minimum of 4" (captures/abilities.json 3202; Karter's 3608 prints Min 5). "Players" after a
// negative number was an unknown stat, so the text compiled to nothing. Three autoplay rounds
// show it, all won: 1533508 r0, 1527810 r1 and 1533638 r1.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

const hand = (names: string[], levels: number[]) =>
  HandGenerator.handOf(names as HandOf<string>, levels as HandOf<number | undefined>);
const karter = () => hand(["Karter", "Dark Desmond", "Bernardite", "Cleo"], [3, 5, 3, 3]);
const piranas = () => hand(["Dalhia Cr", "Lizbeth Cr", "Mayhem", "Rhed Cr"], [5, 2, 1, 1]);

Deno.test("Players Pillz takes from both players when its card wins (1533508 r0)", () => {
  // Karter wins on 3 pillz under the Cosmohnuts `Tune Out`: 12 -> 10 for the opponent, and his
  // own owner's 12 - 3 = 9 -> 7.
  const g = new Game(new Player(15, 12, 0), new Player(15, 12, 1), karter(), piranas(), Turn.PLAYER_1, false, true);
  g.select(0, 3, false, false); // P1 Karter lv3
  g.select(2, 0, false, false); // P2 Mayhem lv1
  assertEquals(g.h1[0].won, true);
  assertEquals([g.p1.pillz, g.p2.pillz], [7, 10]);
});

Deno.test("Players Pillz needs its card to win (printed text)", () => {
  // No capture has Karter or D4ggers lose; the text says "If Karter wins the round".
  const g = new Game(new Player(15, 12, 0), new Player(15, 12, 1), karter(), piranas(), Turn.PLAYER_1, false, true);
  g.select(0, 0, false, false); // P1 Karter lv3
  g.select(2, 12, false, false); // P2 Mayhem lv1 outbids him
  assertEquals(g.h1[0].won, false);
  assertEquals([g.p1.pillz, g.p2.pillz], [12, 0]);
});

Deno.test("Players Pillz stops at its Min on its own owner (1533638 r1)", () => {
  // D4ggers wins on 4 pillz: his owner goes 10 - 4 = 6 -> 4, the Min; the opponent is on 0.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["D4ggers", "Chelonite", "Markus", "Sicula"], [3, 3, 5, 4]),
    hand(["Hawkins Cr", "Mayhem", "Rhed Cr", "Smokey Cr"], [2, 1, 1, 3]),
    Turn.PLAYER_1,
    false,
    true,
  );
  g.select(1, 0, false, false); // P1 Chelonite lv3
  g.select(0, 12, false, false); // P2 Hawkins Cr lv2
  assertEquals([g.p1.pillz, g.p2.pillz], [10, 0]);
  g.select(1, 0, false, false); // P2 Mayhem lv1
  g.select(0, 4, false, false); // P1 D4ggers lv3
  assertEquals(g.h1[0].won, true);
  assertEquals([g.p1.pillz, g.p2.pillz], [4, 0]);
});
