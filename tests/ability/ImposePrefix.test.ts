// A condition prefix in front of an Impose. `Abilities.normalise` moved "Impose" to the front
// of the whole text, prefix included, so "Day: Power Impose" became "Impose Day: Power": an
// unknown condition "Impose Day" over an ability "Power" that compiles to nothing. Every
// prefixed Impose the card list prints (`Reprisal: Damage Impose`, `Unison : Damage Impose`,
// `Day: Power Impose`, `Versus [clan:56][clan:30] : Power Impose`) did nothing. 1517397 r3 is
// the first capture of one live: Zodiack lv3 by day ("The opposing character has equal Power
// to Zodiack", captures/abilities.json 5560) takes Noma lv3 from 9 to 5 Power.
import { Abilities } from "@/game/AbilityParser.ts";
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

const hand = (names: string[], levels: number[]) =>
  HandGenerator.handOf(names as HandOf<string>, levels as HandOf<number | undefined>);

Deno.test("A prefixed Impose keeps its prefix as the condition", () => {
  assertEquals(Abilities.split("Day: Power Impose"), ["Day", "Impose Power"]);
  assertEquals(Abilities.split("Reprisal: Damage Impose"), ["Reprisal", "Impose Damage"]);
  assertEquals(Abilities.split("Unison : Damage Impose"), ["Unison", "Impose Damage"]);
  assertEquals(
    Abilities.split("Versus [clan:56][clan:30] : Power Impose"),
    ["Versus [Clan:56][Clan:30]", "Impose Power"],
  );
  assertEquals(Abilities.split("Power Impose"), ["Impose Power"]);
  assertEquals(Abilities.split("Damage Impose"), ["Impose Damage"]);
});

const play = (night: boolean) => {
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Zodiack", "Angelina", "Mr Dark", "Giacomo"], [3, 2, 3, 2]),
    hand(["Noma", "Natrang", "Sai San", "Gertrud"], [3, 1, 1, 1]),
    Turn.PLAYER_1,
    false,
    night,
  );
  g.select(0, 4, false, false); // P1 Zodiack lv3 (Montana, 5/6), 5 x 5 = 25
  g.select(0, 0, false, false); // P2 Noma lv3 (Tolvack, 9/4) on no pillz
  return g;
};

Deno.test("Day: Power Impose gives the opposing card Zodiack's printed Power (1517397 r3)", () => {
  const g = play(false);
  // 5 x 1 = 5, which Montana's `-12 Opp Attack, Min 8` leaves alone below its Min.
  assertEquals([g.h2[0].power.final, g.h2[0].attack.final], [5, 5]);
  assertEquals(g.h1[0].won, true);
});

Deno.test("By night Zodiack copies the opposing Power instead", () => {
  const g = play(true);
  // `Night: Copy: Opp. Power`: Zodiack fights at Noma's 9, and Noma keeps it.
  assertEquals(g.h2[0].power.final, 9);
  assertEquals(g.h1[0].power.final, 9);
});
