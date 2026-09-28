// Bobby Cornteeth's `Players Combust 1, Min 0`: "If Bobby Cornteeth wins the round, at the end
// of each of the following turns, both players will lose 1 Life points and Pillz, minimum 0"
// (captures/abilities.json 5580, sideAffected both). It normalises to "Players 1 Combust Min 0",
// which no branch of `compileAbility` read, so it did nothing. Four autoplay captures show it:
// 1521294, 1527285, 1528501 and 1529213. Every round after the win both players pay, each with a
// permanent entry posted, and a player on 0 Pillz posts no Pillz entry.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

const hand = (names: string[], levels: number[]) =>
  HandGenerator.handOf(names as HandOf<string>, levels as HandOf<number | undefined>);

Deno.test("Players Combust takes Life and Pillz from both players from the next round (1527285)", () => {
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Bobby Cornteeth", "Colton", "Lucien", "Tatiana"], [2, 2, 4, 4]),
    hand(["Davina", "Matilda", "Mikki", "Stenmark"], [3, 2, 3, 3]),
    Turn.PLAYER_1,
    false,
    true,
  );
  // Round 0: Bobby Cornteeth wins on 5 pillz under the Cosmohnuts `Tune Out` and latches it.
  g.select(0, 5, false, false); // P1 Bobby Cornteeth lv2
  g.select(2, 0, false, false); // P2 Mikki lv3
  assertEquals(g.h1[0].won, true);
  // Delayed, like Poison: nothing in the latching round.
  assertEquals([g.p1.life, g.p1.pillz, g.p2.life, g.p2.pillz], [15, 7, 10, 12]);

  // Round 1: Lucien wins, 1 Damage and `-6 Opp. Life Min 0` take P2 from 10 to 3; then the
  // Combust takes 1 Life and 1 Pillz from each player: 3 -> 2 and 12 - 2 -> 9 for P2, 15 - 1 =
  // 14 and 7 - 6 -> 0 for P1.
  g.select(3, 2, false, false); // P2 Stenmark lv3
  g.select(2, 6, false, false); // P1 Lucien lv4
  assertEquals(g.h1[2].won, true);
  assertEquals([g.p1.life, g.p1.pillz, g.p2.life, g.p2.pillz], [14, 0, 2, 9]);
});

Deno.test("Players Combust also takes from its own owner (1521294)", () => {
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Sakazuki", "Sando", "Steven Dan", "Sung Tsu"], [2, 3, 3, 5]),
    hand(["Bobby Cornteeth", "Cosmo Curcan", "Cley", "Damian"], [2, 2, 3, 4]),
    Turn.PLAYER_1,
  );
  g.select(2, 12, false, false); // P1 Steven Dan lv3
  g.select(1, 0, false, false); // P2 Cosmo Curcan lv2
  g.select(0, 1, false, false); // P2 Bobby Cornteeth lv2 wins round 1
  g.select(0, 0, false, false); // P1 Sakazuki lv2
  assertEquals(g.h2[0].won, true);
  assertEquals([g.p1.life, g.p1.pillz, g.p2.life, g.p2.pillz], [16, 0, 12, 11]);
  // Round 2: Cley wins and deals 6 (16 -> 10), and the Combust takes 1 more from P1 (a player on
  // 0 Pillz keeps 0) and 1 Life and 1 Pillz from its own owner: 12 -> 11, 11 - 2 -> 8.
  g.select(1, 0, false, false); // P1 Sando lv3
  g.select(2, 2, false, false); // P2 Cley lv3
  assertEquals([g.p1.life, g.p1.pillz, g.p2.life, g.p2.pillz], [9, 0, 11, 8]);
});
