// Pepo Brahms' "Growth: Opp. Attack +N" raises the *opposing* card's Attack by N x the round
// number (captures/abilities.json 5210 and 5214: sideAffected "opponent", increase). The
// text normalised to "Opp +1 Attack", which neither numeric branch of compileAbility read,
// so it compiled to nothing. Both captured rounds are here, with the server's numbers.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";
import { Abilities } from "@/game/AbilityParser.ts";

const hand = (names: string[], levels: number[]) =>
  HandGenerator.handOf(names as HandOf<string>, levels as HandOf<number | undefined>);
const stats = (c: { power: { final: number }; damage: { final: number }; attack: { final: number } }) =>
  [c.power.final, c.damage.final, c.attack.final];

Deno.test("Growth: Opp. Attack +N reads as an increase to the opposing Attack", () => {
  assertEquals(Abilities.split("Growth: Opp. Attack +1"), ["Growth", "+1 Opp Attack"]);
  assertEquals(Abilities.split("Growth: Opp. Attack +2"), ["Growth", "+2 Opp Attack"]);
  // The shape it now shares, unchanged.
  assertEquals(Abilities.split("Defeat: +1 Opp. Life"), ["Defeat", "+1 Opp Life"]);
});

Deno.test("Growth: Opp. Attack +1 adds 3 to the opposing Attack in round three", () => {
  // Captured battle 1414749 r2, at night. Schredder (Fury, two pillz) fights at 6 x 3 = 18,
  // plus Pepo Brahms' +1 x 3: the server has 21. Pepo is 7 x 5 = 35 and wins.
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    hand(["Kephren", "Plunk", "Sandro Cr", "Schredder"], [4, 3, 3, 4]),
    hand(["Bugamon", "Okiku", "Pepo Brahms", "SpineHeadMan"], [2, 3, 4, 2]),
    Turn.PLAYER_1,
    false,
    true,
  );

  g.select(0, 0, false, false); // P1 Kephren
  g.select(3, 6, false, false); // P2 SpineHeadMan
  g.select(0, 2, false, false); // P2 Bugamon
  g.select(2, 5, false, false); // P1 Sandro Cr
  g.select(3, 2, true, false); // P1 Schredder, Fury
  g.select(2, 4, false, false); // P2 Pepo Brahms lv4

  const [schredder, pepo] = [g.h1[3], g.h2[2]];
  assertEquals(stats(schredder), [6, 3, 21]);
  assertEquals(stats(pepo), [7, 7, 35]);
  assertEquals(pepo.won, true);
});

Deno.test("Growth: Opp. Attack +2 adds 8 to the opposing Attack in round four", () => {
  // Captured battle 1507713 r3. Filomena bets nine: 4 x 10 = 40, plus Pepo Brahms' +2 x 4 =
  // 48 on the server. Pepo is 7 x 2 = 14, cut to 8 by Filomena's Montana "-12 Opp Attack,
  // Min 8".
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Karl", "BroKen", "Khrull Cr", "Pepo Brahms"], [5, 4, 4, 5]),
    hand(["Twyh", "Filomena", "Ricardo", "Copper Cr"], [3, 2, 2, 3]),
    Turn.PLAYER_1,
    false,
  );

  g.select(0, 4, false, false); // P1 Karl
  g.select(2, 0, false, false); // P2 Ricardo
  g.select(0, 3, false, false); // P2 Twyh
  g.select(2, 0, false, false); // P1 Khrull Cr
  g.select(1, 1, false, false); // P1 BroKen
  g.select(3, 0, false, false); // P2 Copper Cr
  g.select(1, 9, false, false); // P2 Filomena lv2
  g.select(3, 1, false, false); // P1 Pepo Brahms lv5

  const [pepo, filomena] = [g.h1[3], g.h2[1]];
  assertEquals(stats(filomena), [4, 5, 48]);
  assertEquals(stats(pepo), [7, 9, 8]);
  assertEquals(filomena.won, true);
});
