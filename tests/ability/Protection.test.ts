// `Protection: Power And Damage` keeps its card's Power and Damage at its own values when the
// opposing card prints a reduction of them. The engine used to treat Protection as resisting
// a Cancel only, so the reduction landed. Every round below is a captured one, with the
// server's numbers; attack = power x (bet + 1) plus any Attack modifiers.
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

Deno.test("Protection: Power And Damage refuses an opposing Power And Damage reduction", () => {
  // Captured battle 1069506 r0. Sue's "-1 Opp Power And Damage, Min 3" would take Miss
  // Pandora from 7/4 to 6/3 and 30 Attack; the server keeps 7/4 and 7 x 5 = 35. Pandora's
  // Nightmare "Stop Opp. Bonus" leaves Sue at 6 x 6 = 36 without her Rescue Support.
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    hand(["Spidee", "Sue", "Tina", "Wesley"], [4, 2, 3, 3]),
    hand(["Hubert", "Miss Pandora", "Regan", "Zis"], [3, 3, 2, 3]),
    Turn.PLAYER_1,
    false,
  );

  g.select(1, 5, false, false); // P1 Sue lv2, 6/3
  g.select(1, 4, false, false); // P2 Miss Pandora lv3, 7/4

  const [sue, pandora] = [g.h1[1], g.h2[1]];
  assertEquals(stats(pandora), [7, 4, 35]);
  assertEquals(stats(sue), [6, 3, 36]);
  assertEquals([sue.won, pandora.won], [true, false]);
  assertEquals([g.p1.life, g.p2.life], [12, 9]);
});

Deno.test("Protection: Power And Damage refuses an opposing Power reduction", () => {
  // Captured battle 949439 r0, at night. Olga Cr's "-2 Opp Power, Min 5" would leave
  // Nebula 5 x 5 = 25; the server has 7 x 5 = 35. Nebula's Hive Equalizer still takes
  // 3 x 3 off Olga Cr: 8 x 7 - 9 = 47.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Olga Cr", "Oren", "Dark Kalaa", "Lyra"], [3, 5, 2, 3]),
    hand(["AI-Lycs", "Miyo", "Mou", "Nebula"], [3, 3, 3, 3]),
    Turn.PLAYER_1,
    false,
    true,
  );

  g.select(0, 6, false, false); // P1 Olga Cr lv3, 8/3
  g.select(3, 4, false, false); // P2 Nebula lv3, 7/4

  const [olga, nebula] = [g.h1[0], g.h2[3]];
  assertEquals(stats(nebula), [7, 4, 35]);
  assertEquals(stats(olga), [8, 3, 47]);
  assertEquals(olga.won, true);
});

Deno.test("Protection: Power And Damage refuses an opposing Damage reduction", () => {
  // Captured battle 924320 r1, at night. Donald's "-3 Opp Damage, Min 2" would take Nebula's
  // Damage from 4 to 2; the server keeps 4 on the losing Nebula (7 x 1 = 7). Donald is
  // 6 x 2 plus his Support 12, minus Nebula's Hive Equalizer 3 x 3: 15.
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    hand(["AI-Lycs", "Aegis Cr", "Mou", "Nebula"], [3, 5, 3, 3]),
    hand(["Bobby", "Bose", "Dashiell", "Donald"], [3, 2, 5, 3]),
    Turn.PLAYER_1,
    false,
    true,
  );

  g.select(0, 4, false, false); // P1 AI-Lycs
  g.select(1, 2, false, false); // P2 Bose
  g.select(3, 1, false, false); // P2 Donald lv3, 6/4
  g.select(3, 0, false, false); // P1 Nebula lv3, 7/4

  const [nebula, donald] = [g.h1[3], g.h2[3]];
  assertEquals(stats(nebula), [7, 4, 7]);
  assertEquals(stats(donald), [6, 4, 15]);
  assertEquals(donald.won, true);
});

Deno.test("Protection: Power And Damage does not refuse an opposing Attack reduction", () => {
  // Captured battle 956805 r2. Latifa's own Oblivion "Equalizer: Power +1" lifts her to
  // 8 + 5 = 13, and Aegis Cr's Hive "Equalizer: -3 Opp Attack, Min 5" still lands on the
  // Attack: 13 x 3 - 15 = 24. Aegis Cr's own Equalizer makes him 5 + 5 = 10, 10 x 8 = 80.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Gibson Cr", "Quest", "Kochar", "Latifa"], [4, 2, 3, 5]),
    hand(["AI-Lycs", "Aegis Cr", "Lumia Cr", "Uuber"], [3, 5, 4, 2]),
    Turn.PLAYER_1,
    false,
  );

  g.select(3, 2, false, false); // P1 Latifa lv5, 8/6
  g.select(1, 7, false, false); // P2 Aegis Cr lv5, 5/7

  const [latifa, aegis] = [g.h1[3], g.h2[1]];
  assertEquals(stats(latifa), [13, 6, 24]);
  assertEquals(stats(aegis), [10, 7, 80]);
  assertEquals(aegis.won, true);
});

Deno.test("Protection: Attack does not refuse a Power And Damage reduction", () => {
  // Captured battle 1131208 r1. Matriochka's "Protection: Attack" leaves Sue's "-1 Opp
  // Power And Damage, Min 3" alone: 8/4 becomes 7/3 and 7 x 3 = 21. Sue is 6 x 1 plus
  // her Rescue "Support: Attack +3" x 4 = 18.
  const g = new Game(
    new Player(14, 12, 0),
    new Player(14, 12, 1),
    hand(["Anita", "Aurora", "Lothar", "Sue"], [3, 5, 2, 2]),
    hand(["Magic Alice", "Matriochka", "Miss Ming", "Titus"], [3, 3, 2, 3]),
    Turn.PLAYER_1,
    false,
  );

  g.select(3, 0, false, false); // P1 Sue lv2, 6/3
  g.select(1, 2, false, false); // P2 Matriochka lv3, 8/4

  const [sue, matriochka] = [g.h1[3], g.h2[1]];
  assertEquals(stats(matriochka), [7, 3, 21]);
  assertEquals(stats(sue), [6, 3, 18]);
  assertEquals(matriochka.won, true);
});

Deno.test("Reprisal: Protect. Power And Damage refuses an opposing reduction when it moves second", () => {
  // No captured round shows the Reprisal form live against a reduction: 1131114 r2 is live
  // with nothing to refuse, and 1089830 r1 lets Callie's cut land on a first-moving Fiend.
  // The guard composes the plain form's refusal (1069506 r0) with Reprisal's second move.
  // Sue's "-1 Opp Power And Damage, Min 3" would take Forjoten Ld from 8/5 to 7/4.
  const hands = () => [
    hand(["Spidee", "Sue", "Tina", "Wesley"], [4, 2, 3, 3]),
    hand(["Forjoten Ld", "Tiwi Ld", "Milla", "Pavam Cr"], [4, 3, 3, 4]),
  ];

  // Forjoten moves second: the Reprisal holds and the reduction is refused, 8 x 5 = 40.
  const [h1, h2] = hands();
  const second = new Game(new Player(17, 12, 0), new Player(17, 12, 1), h1, h2, Turn.PLAYER_1, false);
  second.select(1, 5, false, false); // P1 Sue lv2, 6/3
  second.select(0, 4, false, false); // P2 Forjoten Ld lv4, 8/5
  assertEquals(stats(second.h2[0]), [8, 5, 40]);

  // Forjoten moves first: the Reprisal fails and the reduction lands, 7 x 5 = 35.
  const [g1, g2] = hands();
  const first = new Game(new Player(17, 12, 0), new Player(17, 12, 1), g1, g2, Turn.PLAYER_2, false);
  first.select(0, 4, false, false); // P2 Forjoten Ld lv4, 8/5
  first.select(1, 5, false, false); // P1 Sue lv2, 6/3
  assertEquals(stats(first.h2[0]), [7, 4, 35]);
});
