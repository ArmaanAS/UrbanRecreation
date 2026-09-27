// The end of the round settles every increase before every decrease, on either seat and
// whoever won, and the decreases in descending order of their Min clamp (Events.executeEnd).
// The rule it replaced ran the round-one second mover's (internal P2's) END effects first;
// that fitted 1093173 r1 and 1496283 r2 only, and 1496283 r2 stopped constraining the order
// once its two Oculus were left uninfiltrated. Every round below is a captured one, with the
// server's numbers.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

const hand = (names: string[], levels: number[]) =>
  HandGenerator.handOf(names as HandOf<string>, levels as HandOf<number | undefined>);
const state = (g: Game) => [g.p1.life, g.p1.pillz, g.p2.life, g.p2.pillz];

Deno.test("A capped Pillz gain lands before the opposing reduction (1514836 r0)", () => {
  // DJ LBerto (internal P1) loses on no pillz and Fury: 12 - 3 = 9. His `Defeat: +2 Pillz
  // Max. 10` takes it to 10, capped, and Yomi Ld's `-2 Opp. Pillz And Life, Min 1` to 8. The
  // reduction first would give 9 - 2 + 2 = 9. Life: 15 - 1 Damage - 2 = 12.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Carlito", "DJ LBerto", "Kaskar", "Lil Jey"], [3, 2, 4, 2]),
    hand(["Sasha", "Schredder", "Strixine", "Yomi Ld"], [2, 4, 3, 2]),
    Turn.PLAYER_1,
    false,
    true,
  );
  g.select(1, 0, true, false); // P1 DJ LBerto lv2
  g.select(3, 7, false, false); // P2 Yomi Ld lv2, 8 x 8 = 64
  assertEquals([g.h1[1].won, g.h2[3].won], [false, true]);
  assertEquals(state(g), [12, 8, 15, 5]);
});

Deno.test("A Recover lands before the opposing reduction (1515298 r1)", () => {
  // Round one: the Piranas hand (internal P1) is on 7 pillz and bets 3 with Tortuga, whose
  // `Defeat: Recover 2 Pillz Out Of 3` returns floor(4 x 2 / 3) = 2; Brampah Noel's `-2 Opp
  // Pillz. Min 3` wins. 4 + 2 - 2 = 4, where the reduction first gives 4 -> 3, + 2 = 5.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Blackfin", "Raeth", "Sheryl", "Tortuga"], [2, 4, 4, 4]),
    hand(["Ahnutt Cr", "Brampah Noel", "Eleanore", "Forjoten Ld"], [3, 2, 5, 2]),
    Turn.PLAYER_1,
    false,
    true,
  );
  g.select(2, 5, false, false); // P1 Sheryl wins round zero
  g.select(2, 0, false, false); // P2 Eleanore
  assertEquals(state(g), [15, 7, 9, 9]);
  g.select(1, 7, false, false); // P2 Brampah Noel lv2, 6 x 8 = 48
  g.select(3, 3, false, false); // P1 Tortuga lv4, 8 x 4 = 32
  assertEquals([g.h1[3].won, g.h2[1].won], [false, true]);
  assertEquals(state(g), [13, 4, 9, 2]);
});

Deno.test("Gains and a latched Regen land before a latched Poison (1515451 r2)", () => {
  // Esther's `Poison 1, Min 2` (internal P2) latched on P1 in round zero; P1's Butcher
  // Braxton latched `Growth: Regen 1, Max. 17` (2 a round) in round one. In round two P1's
  // Ennio wins with `Victory Or Defeat : +4 Life`: 13 + 4 = 17, the Regen is capped there,
  // and the Poison takes 1: 16. The old order (P2's Poison, then P1's) gave 12 + 4 + 1 = 17.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Butcher Braxton", "Ennio", "Guillotinette", "Judge Lynch"], [5, 5, 4, 3]),
    hand(["Esther", "Miloz", "Felynn", "Zippy"], [2, 2, 3, 3]),
    Turn.PLAYER_1,
    false,
    true,
  );
  g.select(3, 0, true, false); // P1 Judge Lynch
  g.select(0, 7, false, false); // P2 Esther wins, latches Poison
  g.select(3, 4, false, false); // P2 Zippy
  g.select(0, 4, false, false); // P1 Butcher Braxton wins, latches Regen
  assertEquals(state(g), [13, 5, 13, 1]);
  g.select(1, 5, false, false); // P1 Ennio lv5 wins
  g.select(1, 0, false, false); // P2 Miloz
  assertEquals(g.h1[1].won, true);
  assertEquals(state(g), [16, 0, 10, 1]);
});

Deno.test("Two reductions of one Life resolve by descending Min, bonus first (876752 r1)", () => {
  // Macey Rook wins on 5 pillz and Fury and deals 3: the Komboka owner goes 8 -> 5. The
  // Berzerk bonus `-2 Opp. Life Min 2` takes it to 3, then the Brawl `- 1 Opp. Life Min 0`
  // (x4 Komboka) to 0; the ability first would give 5 -> 1, and the bonus leave 1 alone.
  const g = new Game(
    new Player(12, 12, 0),
    new Player(8, 12, 1),
    hand(["Macey Rook", "Sixx Meycry", "Andy Ld", "Lady Ametia Cr"], [2, 3, 1, 3]),
    hand(["Adytia Ld", "Bekum", "Ceida Cr", "Mavi"], [2, 2, 2, 2]),
    Turn.PLAYER_1,
    false,
    true,
  );
  g.select(0, 5, true, false); // P1 Macey Rook lv2, 8 x 6 = 48
  g.select(2, 3, false, false); // P2 Ceida Cr lv2, 3 x 4 + 20 = 32
  assertEquals(g.h1[0].won, true);
  assertEquals(g.p2.life, 0);
});

Deno.test("Two reductions of one Life resolve by descending Min, ability first (1515853 r0)", () => {
  // Hilly Billy wins on 12 pillz and deals 5: 15 -> 10. Its ability `-5 Opp. Life Min 4`
  // takes that to 5, then the Berzerk bonus `-2 Opp. Life Min 2` to 3; the bonus first would
  // give 10 -> 8 -> 4 (Min 4). 1515873 r0 is the same round against another hand. Both
  // captures belong to the 2026-09-27 autoplay run's second batch.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Hilly Billy", "La Bestia", "Melanie", "Ranesh"], [5, 5, 2, 4]),
    hand(["C.T", "Firmin", "Dr Van Wesel Ld", "Pr SenQ"], [3, 3, 3, 2]),
    Turn.PLAYER_1,
    false,
    false,
  );
  g.select(0, 12, false, false); // P1 Hilly Billy lv5, 7 x 13 = 91
  g.select(2, 2, false, false); // P2 Dr Van Wesel Ld lv3, 9 x 3 = 27
  assertEquals(g.h1[0].won, true);
  // P2's Riots `Victory Or Defeat : +1 Pillz` returns one: 12 - 2 + 1 = 11.
  assertEquals(state(g), [15, 0, 3, 11]);
});
