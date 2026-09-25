// An Oculus card joins its hand's clan ("Infiltrated": three cards of one clan, or the lone
// card of a two-and-one split) and takes that clan's bonus; its bracketed clan list gates
// its ability on the clan it joined. Engine rules checked against captures:
// - 1090269 r0: Wachtmann + three Ulu Watu fights at 8 + 2 = 10 Power, the Ulu Watu bonus.
// - 1131463 r0 and 1131373 r2: Kusm joins a listed clan (Huracan, then Sakrohm) and its
//   "-12 Opp Attack, Min 3" lands (Smokey Cr 7 x 3 - 12 = 9; Taljion 6 x 2 + 12 Support
//   - 12 = 12), while the Piranas "Stop Opp. Bonus" cancels the bonus it infiltrated
//   (Kusm 6 x 4 = 24 without "+1 Attack Per Life Left"; Taljion keeps its 12 without
//   the "-8 Opp Attack").
// - 1022847 r2: Alekperov joins Tolvack, which is not in his list, and Jairin keeps 8 Power.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";

const stats = (
  c: { power: { final: number }; damage: { final: number }; attack: { final: number } },
) => [c.power.final, c.damage.final, c.attack.final];

Deno.test("Infiltrated", () => {
  // Alekperov (lv3, 6/5) joins Ulu Watu, which his list names, so his "-2 Opp Power, Min 5"
  // takes Tamakuchi (lv4, 7/5) to 5 Power: 5 x 1 + 3 Growth = 8 Attack. The infiltrated
  // "Power +2" falls to Tamakuchi's Nightmare "Stop Opp. Bonus", leaving Alekperov at
  // 6 x 1 = 6, so Tamakuchi still wins and deals 5. This test used to expect Alekperov to
  // win for 4 (p2 on 8), written against the Dec 2024 data (6/4, "Min 1"). He can only win
  // this round if the infiltrated bonus survives Stop Opp. Bonus (8 x 1 against 8, and a
  // tie goes to the card with fewer stars); the captures above show it does not.
  const h1 = HandGenerator.generate(
    "Alekperov",
    "Eugene",
    "Hikiyousan",
    "Zatapa",
  );
  const h2 = HandGenerator.generate(
    "Betelgeuse",
    "Candy Jack",
    "Incubus",
    "Tamakuchi",
  );
  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);

  const g = new Game(p1, p2, h1, h2, Turn.PLAYER_1);

  assertEquals(g.p1.life, 12);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);

  g.select(0, 0); // Alekperov
  g.select(3, 0); // Tamakuchi

  assertEquals(stats(g.h1[0]), [6, 5, 6]);
  assertEquals(stats(g.h2[3]), [5, 5, 8]);
  assertEquals([g.h1[0].won, g.h2[3].won], [false, true]);
  assertEquals(g.p1.life, 7);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);
});

Deno.test("Infiltrated No Ability", () => {
  // Two Ulu Watu and a Riots: Alekperov joins the lone Riots, which his list does not name,
  // so Tamakuchi keeps 7 Power: 7 x 1 + 3 = 10.
  const h1 = HandGenerator.generate(
    "Alekperov",
    "Agnes",
    "Hikiyousan",
    "Zatapa",
  );
  const h2 = HandGenerator.generate(
    "Betelgeuse",
    "Candy Jack",
    "Incubus",
    "Tamakuchi",
  );
  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);

  const g = new Game(p1, p2, h1, h2, Turn.PLAYER_1);

  assertEquals(g.p1.life, 12);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);

  g.select(0, 0); // Alekperov
  g.select(3, 0); // Tamakuchi

  assertEquals(stats(g.h1[0]), [6, 5, 6]);
  assertEquals(stats(g.h2[3]), [7, 5, 10]);
  assertEquals(g.p1.life, 7);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);
});

Deno.test("No Infiltrated", () => {
  const h1 = HandGenerator.generate(
    "Alekperov",
    "Agnes",
    "Hikiyousan",
    "Iris Morana",
  );
  const h2 = HandGenerator.generate(
    "Betelgeuse",
    "Candy Jack",
    "Incubus",
    "Tamakuchi",
  );
  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);

  const g = new Game(p1, p2, h1, h2, Turn.PLAYER_1);

  assertEquals(g.p1.life, 12);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);

  g.select(0, 0); // Alekperov
  g.select(3, 0); // Tamakuchi

  assertEquals(g.p1.life, 7);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);
});
