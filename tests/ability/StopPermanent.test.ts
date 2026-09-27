// "Stop: <permanent>" latches only when the opposing card cancels its ability: "If Ardwizz's
// ability is cancelled out by the opposing character, If Ardwizz wins the round, at the end
// of each of the following rounds the player controlling Ardwizz will earn 2 Life point(s)"
// (captures/abilities.json 1629; Giacomo's `Stop: Poison 3, Min 2` is 1147). A permanent is
// GLOBAL_ABILITY by the time its conditions compile, which the Stop condition did not know,
// so it held unconditionally and every won round latched. Three autoplay rounds show the
// unstopped case latching nothing: Ardwizz in 1516740 r2 and Giacomo in 1517397 r1 and
// 1517419 r2. No capture has a Stop: permanent actually stopped yet; that half follows the
// printed text, as the combat-stat `Stop:` abilities (Janice's `Stop: Power +3`) already do.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

const hand = (names: string[], levels: number[]) =>
  HandGenerator.handOf(names as HandOf<string>, levels as HandOf<number | undefined>);

// Four Montana: the bonus `-12 Opp Attack, Min 8` is live on every card.
const montana = () =>
  hand(["Giacomo", "Angelina", "Mr Dark", "Zodiack"], [2, 2, 3, 3]);

Deno.test("An unstopped Stop: Poison latches nothing (1517397 r1, 1517419 r2)", () => {
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    montana(),
    hand(["Natrang", "Sai San", "Gertrud", "Mitch"], [1, 1, 1, 1]),
    Turn.PLAYER_1,
  );
  g.select(0, 2, false, false); // P1 Giacomo lv2, 7 x 3 = 21
  g.select(0, 0, false, false); // P2 Natrang, 3 x 1, under Montana's Min 8
  assertEquals(g.h1[0].won, true);
  assertEquals(g.p2.life, 10); // Giacomo's 2 Damage
  g.select(1, 0, false, false); // P2 Sai San, 1 x 1
  g.select(1, 1, false, false); // P1 Angelina lv2, 4 x 2, copies Sai San's printed 2 Damage
  assertEquals(g.h1[1].won, true);
  assertEquals(g.p2.life, 8); // no Poison: it used to take 3 more, to 5
});

Deno.test("A stopped Stop: Poison latches and pays from the next round", () => {
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    montana(),
    hand(["Angelo", "Natrang", "Sai San", "Gertrud"], [1, 1, 1, 1]),
    Turn.PLAYER_1,
  );
  g.select(0, 2, false, false); // P1 Giacomo lv2, 7 x 3 = 21
  g.select(0, 0, false, false); // P2 Angelo lv1, `Stop Opp. Ability`, 6 x 1
  assertEquals(g.h1[0].won, true);
  assertEquals(g.p2.life, 10); // the Poison waits a round, as every Poison does
  g.select(1, 0, false, false); // P2 Natrang, 3 x 1
  g.select(1, 1, false, false); // P1 Angelina lv2, copies Natrang's printed 2 Damage
  assertEquals(g.h1[1].won, true);
  assertEquals(g.p2.life, 5); // 10 - 2 - 3
});

Deno.test("A Stop: Heal pays only when it was stopped (1516740 r2)", () => {
  // Ardwizz lv3 (Dominion, 7/4) wins round zero; Natrang then loses round one to Gertrud's 5
  // Damage (3 + La Junta's `Damage +2`), 12 - 5 = 7. Only a stopped Ardwizz heals 2 after
  // that round; Steve (Rescue, no ability) stops nothing.
  const play = (stopper: string) => {
    const g = new Game(
      new Player(12, 12, 0),
      new Player(12, 12, 1),
      hand(["Ardwizz", "Gorgorax", "Natrang", "Sai San"], [3, 5, 1, 1]),
      hand([stopper, "Gertrud", "Mitch", "Lea"], [1, 1, 1, 1]),
      Turn.PLAYER_1,
    );
    g.select(0, 3, false, false); // P1 Ardwizz, 7 x 4 = 28
    g.select(0, 0, false, false); // P2's first card on no pillz
    assertEquals(g.h1[0].won, true);
    g.select(1, 5, false, false); // P2 Gertrud, 1 x 6
    g.select(2, 0, false, false); // P1 Natrang, 3 x 1
    assertEquals(g.h2[1].won, true);
    return g.p1.life;
  };
  assertEquals(play("Angelo"), 9); // stopped: 7 + 2
  assertEquals(play("Steve"), 7); // unstopped: the Heal never latched
});
