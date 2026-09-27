// A capped increase of a card's own stat is measured before the card's clan bonus lands:
// Razor level 4 (Ulu Watu, 5 Power, `+1 Power Per Life Lost Max. 9`, bonus `Power +2`)
// fights at 5 + lost, capped at 9, + 2. The engine used to run the bonus first and cap the
// total, 5 + 2 + lost capped at 9. Five autoplay rounds pin it (1516811 r1 and 1516846 r1 on
// 5 lost, 1516832 r1 on 7, 1516906 r2 on 6, 1517271 r1 on 4: 11 in every one), and 1508932
// r2 the Damage form, P. Steevens Cr lv4's `+1 Damage Per Life Lost Max. 7` beside La Junta's
// `Damage +2`: 2 + 9 capped at 7, + 2 = 9. See `Ability.capsOwnStat`.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

const hand = (names: string[], levels: number[]) =>
  HandGenerator.handOf(names as HandOf<string>, levels as HandOf<number | undefined>);

const razor = (life: number) => {
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Razor", "Fuzzy June", "Gaia Noel", "Janice"], [4, 3, 3, 3]),
    hand(["Natrang", "Sai San", "Gertrud", "Mitch"], [1, 1, 1, 1]),
    Turn.PLAYER_1,
  );
  g.p1.life = life; // Life lost is measured from the match-start 15
  g.select(0, 0, false, false); // P1 Razor on no pillz
  g.select(1, 0, false, false); // P2 Sai San, 1 x 1
  return g.h1[0];
};

Deno.test("A binding cap is measured before the same-stat bonus (Razor, 1516811 r1)", () => {
  // 5 lost: 5 + 5 = 10, capped at 9, then Ulu Watu's +2. Bonus-first gave 9.
  const card = razor(10);
  assertEquals([card.power.final, card.attack.final], [11, 11]);
  // 7 lost (1516832 r1): 12 capped at 9, + 2 = 11 as well.
  assertEquals(razor(8).power.final, 11);
});

Deno.test("A cap that does not bind is the same either way", () => {
  // 2 lost: 5 + 2 + 2 = 9 in any order.
  assertEquals(razor(13).power.final, 9);
});

Deno.test("The Damage form: P. Steevens Cr beside La Junta's Damage +2 (1508932 r2)", () => {
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Chiro", "No Nam", "P. Steevens Cr", "Pilzken Cr"], [4, 5, 4, 5]),
    hand(["Natrang", "Sai San", "Gertrud", "Mitch"], [1, 1, 1, 1]),
    Turn.PLAYER_1,
  );
  g.p1.life = 6; // 9 lost
  g.select(2, 0, false, false); // P1 P. Steevens Cr lv4 (8/2) on no pillz
  g.select(1, 0, false, false); // P2 Sai San, 1 x 1
  // 2 + 9 = 11, capped at 7, then + 2 = 9. Bonus-first gave 2 + 2 + 9 capped at 7.
  assertEquals(g.h1[2].damage.final, 9);
});
