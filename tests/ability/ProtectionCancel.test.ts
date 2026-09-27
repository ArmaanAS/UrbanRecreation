// A stat Protection does not shield its card's own modifiers of that stat from an opposing
// `Cancel Opp. <stat> Modif.` ("Any modifier of the opposing character affecting attack will be
// deactivated", captures/abilities.json 1163). The engine let the Protection win. Every round
// below is a captured one, with the server's numbers; attack = power x (bet + 1) plus the Attack
// modifiers.
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

Deno.test("Cancel Opp. Attack Modif. beats Protection: Attack (1519871 r0)", () => {
  // Gemini lv3's `Protection: Attack` beside its Hive bonus `Equalizer: -3 Opp Attack, Min 5`.
  // The Raptors bonus `Cancel Opp. Attack Modif.` still switches the Equalizer off: Sauropsite
  // fights at 8 x 7 = 56, not 56 - 3 x 3 = 47.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Horace", "Sauropsite", "Chel", "Fuzzy June"], [3, 3, 3, 3]),
    hand(["Delta", "Gemini", "Sight Ld", "VeeXn"], [2, 3, 1, 4]),
    Turn.PLAYER_1,
  );
  g.select(1, 6, false, false); // P1 Sauropsite lv3, 8/4
  g.select(1, 4, false, false); // P2 Gemini lv3, 7/4
  const [sauropsite, gemini] = [g.h1[1], g.h2[1]];
  assertEquals(stats(sauropsite), [8, 4, 56]);
  assertEquals(stats(gemini), [7, 4, 35]);
  assertEquals(sauropsite.won, true);
});

Deno.test("Cancel Opp. Damage Modif. beats Protection : Damage (1520579 r1)", () => {
  // Davis lv3's `Protection : Damage` beside his La Junta `Damage +2`. Lenora's `Cancel Opp.
  // Damage Modif.` switches the bonus off: Davis wins on his printed 3 Damage, not 5.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Jamtiax", "Mozaert", "Davis", "Sgt Stormblade"], [2, 4, 3, 2]),
    hand(["Grouchy", "Harmonia", "Lenora", "Octana"], [2, 2, 4, 5]),
    Turn.PLAYER_1,
  );
  g.select(2, 4, false, false); // P1 Davis lv3, 8/3
  g.select(2, 2, false, false); // P2 Lenora lv4
  const [davis, lenora] = [g.h1[2], g.h2[2]];
  assertEquals(stats(davis), [8, 3, 40]);
  assertEquals(stats(lenora), [8, 5, 24]);
  assertEquals(g.p2.life, 12); // 15 - 3
});

Deno.test("Protection: Attack still refuses an opposing Attack reduction", () => {
  // Only the Cancel half changed: without a Cancel, the guard keeps Gemini's Attack whole
  // against the Montana bonus `-12 Opp Attack, Min 8` (the printed "The attack of Gemini cannot
  // be reduced by an opposing character", 1340; no capture shows this half yet).
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Delta", "Gemini", "Sight Ld", "VeeXn"], [2, 3, 1, 4]),
    hand(["Giacomo", "Angelina", "Mr Dark", "Zodiack"], [2, 2, 3, 3]),
    Turn.PLAYER_1,
  );
  g.select(1, 2, false, false); // P1 Gemini lv3, 7 x 3
  g.select(0, 0, false, false); // P2 Giacomo lv2
  assertEquals(g.h1[1].attack.final, 21);
});
