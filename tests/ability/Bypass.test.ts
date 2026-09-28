// Robert Cobb's `Bypass` (Leader): "The bonus of your other cards is active even if you don't
// have another card from the same clan. Their bonus can still be blocked by your opponent's
// 'Stop Opp. Bonus' cards" (captures/abilities.json 1546, specialAction `activate_all_bonuses`).
// In all seven captured hands with a lone Robert Cobb the server sends every lone card's clan
// bonus; in 1529161 r2 Stanly, the hand's only Ulu Watu, fights with its `Power +2`.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

const hand = (names: string[], levels: number[]) =>
  HandGenerator.handOf(names as HandOf<string>, levels as HandOf<number | undefined>);
const sentinel = () => hand(["Aurelia", "Josh", "Melissa Cr", "Robin"], [2, 3, 1, 3]);
const stanly = (first: string, level: number) => {
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand([first, "Avola", "Corvine", "Stanly"], [level, 5, 3, 5]),
    sentinel(),
    Turn.PLAYER_1,
  );
  g.select(3, 6, false, false); // P1 Stanly lv5, 6/8, the only Ulu Watu
  g.select(2, 1, false, false); // P2 Melissa Cr lv1
  return g.h1[3];
};

Deno.test("Bypass activates a lone card's clan bonus (1529161 r2)", () => {
  // 6 + 2 = 8 Power, 8 x 7 = 56 Attack.
  const card = stanly("Robert Cobb", 5);
  assertEquals([card.power.final, card.attack.final], [8, 56]);
});

Deno.test("Without Bypass a lone card has no bonus", () => {
  const card = stanly("Mattachione", 2);
  assertEquals([card.power.final, card.attack.final], [6, 42]);
});

Deno.test("A second Leader cancels Bypass", () => {
  // Cancel Leader: "Your Leader Abilities are deactivated if you have more than one Leader in
  // your team" (captures/abilities.json 117).
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Robert Cobb", "Solomon", "Corvine", "Stanly"], [5, 5, 3, 5]),
    sentinel(),
    Turn.PLAYER_1,
  );
  g.select(3, 6, false, false);
  g.select(2, 1, false, false);
  assertEquals(g.h1[3].power.final, 6);
});
