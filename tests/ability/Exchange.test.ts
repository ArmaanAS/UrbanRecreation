// An Exchange or an Impose writes printed values onto the cards before any increase or
// reduction, so an increase registered before it survives. The engine used to run the swap
// among the PRE2 increases and overwrite whatever had already landed: the owner's own bonus
// always, and the opponent's own increase whenever the opponent was internal P1. Every round
// below is a captured one, replayed from the start of its game, with the server's numbers.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

const hand = (names: string[], levels: number[]) =>
  HandGenerator.handOf(names as HandOf<string>, levels as HandOf<number | undefined>);
const stats = (c: { power: { final: number }; damage: { final: number }; attack: { final: number } }) =>
  [c.power.final, c.damage.final, c.attack.final];

Deno.test("Power Exchange keeps the owner's own increase (1507008 r0)", () => {
  // Calamity (internal P2) swaps her printed 5 for Tatane's 6, then her own GhosTown "Day:
  // Power And Damage + 1" makes 7/5, and Tatane's "-12 Opp Attack, Min 5" leaves 7 x 3 - 12 =
  // 9. Tatane fights at Calamity's printed 5 with his Damage +2: 5 x 6 = 30.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Alexei", "Kati", "Tatane", "Keanew"], [4, 4, 3, 3]),
    hand(["Calamity", "Comanche", "Dakota Cr", "Heaven"], [3, 3, 2, 2]),
    Turn.PLAYER_1,
    false,
  );

  g.select(2, 5, false, false); // P1 Tatane lv3, 6/5
  g.select(0, 2, false, false); // P2 Calamity lv3, 5/4

  const [tatane, calamity] = [g.h1[2], g.h2[0]];
  assertEquals(stats(tatane), [5, 7, 30]);
  assertEquals(stats(calamity), [7, 5, 9]);
  assertEquals([tatane.won, calamity.won], [true, false]);
  assertEquals([g.p1.life, g.p2.life], [15, 8]);
});

Deno.test("Power Exchange keeps the opponent's own increase (1059149 r1)", () => {
  // Round two at night. Tina (internal P1) takes Calamity's printed 5, her own Revenge "Power
  // And Damage +2" makes 7/6, and Calamity's GhosTown "Night: -1 Opp Pow. And Damage, Min 1"
  // leaves 6/5: 6 x 4 + Rescue Support 12 = 36.
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    hand(["Aurora", "Callie", "Sue", "Tina"], [5, 3, 2, 3]),
    hand(["Calamity", "Cravy", "Fork Joe", "Padre Frollo"], [3, 4, 2, 4]),
    Turn.PLAYER_1,
    false,
    true,
  );

  g.select(1, 0, false, false); // P1 Callie
  g.select(3, 6, false, false); // P2 Padre Frollo
  g.select(0, 0, false, false); // P2 Calamity lv3, 5/4
  g.select(3, 3, false, false); // P1 Tina lv3, 5/4

  const [tina, calamity] = [g.h1[3], g.h2[0]];
  assertEquals(stats(tina), [6, 5, 36]);
  assertEquals(stats(calamity), [5, 4, 5]);
  assertEquals([tina.won, calamity.won], [true, false]);
  assertEquals([g.p1.life, g.p2.life], [9, 7]);
});

Deno.test("Damage Impose keeps the opponent's own increase (874712 r1)", () => {
  // Round two at night. Kochar imposes his printed 2 Damage on Tina (internal P1), and her own
  // Revenge "Power And Damage +2" lands on it: 7/4, 7 x 2 + Rescue Support 12 = 26. Kochar's
  // Oblivion "Copy: Opp. Ability" adopts that Revenge, which does not hold for him.
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    hand(["Aurora", "Callie", "Sue", "Tina"], [5, 3, 2, 3]),
    hand(["Anoda", "Kassar", "Kochar", "Viperine"], [3, 4, 3, 3]),
    Turn.PLAYER_1,
    false,
    true,
  );

  g.select(0, 5, false, false); // P1 Aurora
  g.select(3, 4, false, false); // P2 Viperine
  g.select(2, 0, false, false); // P2 Kochar lv3, 8/2
  g.select(3, 1, false, false); // P1 Tina lv3, 5/4

  const [tina, kochar] = [g.h1[3], g.h2[2]];
  assertEquals(stats(tina), [7, 4, 26]);
  assertEquals(stats(kochar), [8, 2, 8]);
  assertEquals([tina.won, kochar.won], [true, false]);
  assertEquals([g.p1.life, g.p2.life], [9, 11]);
});
