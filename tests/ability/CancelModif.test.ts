// `Cancel Opp. <stat> Modif.` deactivates the modifiers whose source is the cancelled card -
// its own increases and its reductions of the canceller - and never the canceller's own
// reductions aimed at it. The engine used to refuse any opposing reduction of a cancelled
// stat, so a canceller's own `-N Opp Power` did nothing. Every round below is a captured one,
// replayed from the start of its game, with the server's numbers; attack = power x (bet + 1)
// plus any Attack modifiers.
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

Deno.test("Cancel Opp. Power And Damage Modif. keeps its own Growth reduction (1089974 r2)", () => {
  // Round three at night. Dookor's own Dominion "Growth: -1 Opp Power, Min 4" is -3 and takes
  // Sue from 6 to 4 under Dookor's cancel: 4 x 1 + Rescue Support 12 = 16. Sue's own "-1 Opp
  // Power And Damage, Min 3" is hers, so the cancel refuses it and Dookor stays 6/4, 6 x 3.
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    hand(["Barcius", "Lucien", "Carnibox", "Dookor"], [3, 4, 2, 3]),
    hand(["Anita", "Lothar", "Sue", "Tina"], [3, 2, 2, 3]),
    Turn.PLAYER_1,
    false,
    true,
  );

  g.select(1, 1, false, false); // P1 Lucien
  g.select(1, 5, false, false); // P2 Lothar
  g.select(0, 0, false, false); // P2 Anita
  g.select(2, 7, false, false); // P1 Carnibox
  g.select(3, 2, false, false); // P1 Dookor lv3, 6/4
  g.select(2, 0, false, false); // P2 Sue lv2, 6/3

  const [dookor, sue] = [g.h1[3], g.h2[2]];
  assertEquals(stats(dookor), [6, 4, 18]);
  assertEquals(stats(sue), [4, 3, 16]);
  assertEquals([dookor.won, sue.won], [true, false]);
  assertEquals([g.p1.life, g.p2.life], [8, 6]);
});

Deno.test("Cancel Opp. Power And Damage Modif. keeps its own bonus reduction (1506931 r3)", () => {
  // Round four. Eyrton Cr's own All Stars "-2 Opp Power, Min 1" takes Hammer Cr from 6 to 4
  // under Eyrton's cancel: 4 x 7 = 28. Hammer Cr's identical bonus is his own, so the cancel
  // refuses it and Eyrton Cr keeps 8 Power, 8 x 6 = 48.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Alexei", "Hammer Cr", "Fuzz", "Emma"], [4, 5, 5, 5]),
    hand(["Bixente", "Eyrton Cr", "Diana", "Malicia"], [2, 5, 4, 4]),
    Turn.PLAYER_1,
    false,
  );

  g.select(0, 0, false, false); // P1 Alexei
  g.select(3, 3, false, false); // P2 Malicia
  g.select(0, 0, false, false); // P2 Bixente
  g.select(2, 4, false, false); // P1 Fuzz
  g.select(3, 2, false, false); // P1 Emma
  g.select(2, 4, false, false); // P2 Diana
  g.select(1, 5, false, false); // P2 Eyrton Cr lv5, 8/5
  g.select(1, 6, false, false); // P1 Hammer Cr lv5, 6/7

  const [hammer, eyrton] = [g.h1[1], g.h2[1]];
  assertEquals(stats(hammer), [4, 7, 28]);
  assertEquals(stats(eyrton), [8, 5, 48]);
  assertEquals([hammer.won, eyrton.won], [false, true]);
  assertEquals([g.p1.life, g.p2.life], [1, 10]);
});

Deno.test("Cancel Opp. Power And Damage Modif. keeps its own reduction down to the Min (1507792 r3)", () => {
  // Round four. Angie copies Dookor's 6 Power; Dookor's own "Growth: -1 Opp Power, Min 4" is
  // -4 and stops at the Min under Dookor's cancel: 4 x 4 = 16. Dookor is 6 x 5 = 30.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["BroKen", "Dookor", "Golrock", "Kaboom"], [4, 3, 5, 3]),
    hand(["Angie", "Massiv", "Usman", "Rowdy Cr"], [5, 2, 4, 4]),
    Turn.PLAYER_1,
    false,
  );

  g.select(3, 0, false, false); // P1 Kaboom
  g.select(1, 0, false, false); // P2 Massiv
  g.select(3, 0, false, false); // P2 Rowdy Cr
  g.select(0, 2, false, false); // P1 BroKen
  g.select(2, 6, false, false); // P1 Golrock
  g.select(2, 8, false, false); // P2 Usman, whose cancel refuses Golrock's own reductions
  assertEquals(stats(g.h2[2]), [8, 6, 72]);
  g.select(0, 3, false, false); // P2 Angie lv5, 6/8
  g.select(1, 4, false, false); // P1 Dookor lv3, 6/4

  const [dookor, angie] = [g.h1[1], g.h2[0]];
  assertEquals(stats(dookor), [6, 4, 30]);
  assertEquals(stats(angie), [4, 8, 16]);
  assertEquals([dookor.won, angie.won], [true, false]);
  assertEquals([g.p1.life, g.p2.life], [4, 11]);
});
