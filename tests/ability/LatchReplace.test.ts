// Same-family permanents replace instead of stacking. The server prints "If two poisons or
// toxins are applied, the second will replace the first as soon as the latter takes effect"
// on every Toxin, and the same note for two Heal or Regen, two Dope and two Consume.
//
// Captured battle 1506438 settles it for two identical Poisons: the opposing Freaks bonus
// "Poison 2, Min 3" latches in rounds zero and one, and round two takes 2 (7 -> 5), where
// stacking took 4; round three posts a single entry of 2 (5 -> 3). In round one the newer
// latch is still in its delayed round, so the older one pays its 2 (12 - 3 - 2 = 7), as in
// 1131208 r2, 1092369 r2 and 926420 r3.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game, { Undo } from "@/game/Game.ts";
import { assertEquals, assertNotEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";
import { ContinuationCache } from "@/solver/Policy.ts";

/** 1506438's hands; `zera` and `olga` replace those cards' abilities. */
const game = (zera?: string, olga?: string, life = 15) => {
  const h1 = HandGenerator.handOf(
    ["Hammer Cr", "Keanew", "Emma", "Wanda Cr"] as HandOf<string>,
    [5, 3, 5, 2] as HandOf<number | undefined>,
  );
  const h2 = HandGenerator.handOf(
    ["Olga Cr", "Schaap", "Zera", "Shayna"] as HandOf<string>,
    [3, 4, 4, 4] as HandOf<number | undefined>,
  );
  if (zera !== undefined) h2[2] = h2[2].withAbility(zera);
  if (olga !== undefined) h2[0] = h2[0].withAbility(olga);
  return new Game(new Player(life, 12, 0), new Player(life, 12, 1), h1, h2, Turn.PLAYER_1, false);
};

/** Rounds zero and one of 1506438: Zera, then Olga Cr, win for the Freaks hand. */
const firstTwoRounds = (g: Game, lives: number[][] = []) => {
  g.select(3, 0, false, false); // P1 Wanda Cr
  g.select(2, 6, false, false); // P2 Zera wins, deals 3
  lives.push([g.p1.life, g.p2.life]);
  g.select(0, 3, false, false); // P2 Olga Cr
  g.select(1, 0, false, false); // P1 Keanew, Olga wins, deals 3
  lives.push([g.p1.life, g.p2.life]);
};

const poisons = (g: Game) => g.events2.repeat.flat().map((a) => a.won);

Deno.test("A second Poison replaces the first once it starts paying (1506438)", () => {
  const g = game();
  const lives: number[][] = [];
  firstTwoRounds(g, lives);
  g.select(2, 3, false, false); // P1 Emma wins, deals 6
  g.select(3, 3, false, false); // P2 Shayna
  lives.push([g.p1.life, g.p2.life]);
  g.select(1, 0, false, false); // P2 Schaap
  g.select(0, 1, true, false); // P1 Hammer Cr, Fury, wins and knocks P2 out
  lives.push([g.p1.life, g.p2.life]);

  // 12 - 3 - 2 = 7 with the newer latch still delayed; then 2 a round, not 4.
  assertEquals(lives, [[12, 15], [7, 15], [5, 9], [3, 0]]);
  assertEquals(poisons(g), [false, true]);
});

Deno.test("Of three latches only the newest pays", () => {
  // The Freaks bonus latches three times, with Zera, Olga Cr and Schaap, on 25 Life so the
  // target stays above the Min 3: 25 - 3 = 22, 22 - 3 - 2 = 17, 17 - 5 - 2 = 10 as Schaap
  // wins, then 10 - 2 = 8 as Emma wins. Stacking took 8 and then 4.
  const g = game(undefined, undefined, 25);
  const lives: number[][] = [];
  firstTwoRounds(g, lives);
  g.select(0, 0, false, false); // P1 Hammer Cr
  g.select(1, 2, false, false); // P2 Schaap wins, deals 5
  lives.push([g.p1.life, g.p2.life]);
  assertEquals(poisons(g), [false, true, true]);
  g.select(3, 0, false, false); // P2 Shayna
  g.select(2, 2, false, false); // P1 Emma wins, deals 6
  lives.push([g.p1.life, g.p2.life]);

  assertEquals(lives, [[22, 25], [17, 25], [10, 25], [8, 19]]);
  assertEquals(poisons(g), [false, false, true]);
});

Deno.test("An immediate newcomer replaces the older latch in its own round", () => {
  // Unobserved, from the printed text: a Toxin pays in the round it latches, so the latched
  // Poison stops that very round. Olga Cr prints "Toxin 1, Min 0" here; her Freaks bonus
  // Poison, merged just before it, is replaced before it ever latches. 12 - 3 - 1 = 8, where
  // stacking took 12 - 3 - 2 - 1 = 6; then the Toxin's 1 a round.
  const g = game(undefined, "Toxin 1, Min 0");
  const lives: number[][] = [];
  firstTwoRounds(g, lives);
  g.select(2, 3, false, false); // P1 Emma wins, deals 6
  g.select(3, 3, false, false); // P2 Shayna
  lives.push([g.p1.life, g.p2.life]);

  assertEquals(lives, [[12, 15], [8, 15], [7, 9]]);
  assertEquals(poisons(g), [false, false, true]);
});

Deno.test("A latch aimed at the other player is not replaced", () => {
  // Zera prints "Backlash: Poison 1, Min 3" here, a Poison on its own owner beside the Freaks
  // bonus on the opponent. Both latch in round zero and pay from round one; Olga's bonus
  // Poison then replaces only the bonus one on P1, and the Backlash keeps taking 1 from P2.
  const g = game("Backlash: Poison 1, Min 3");
  const lives: number[][] = [];
  firstTwoRounds(g, lives);
  g.select(2, 3, false, false); // P1 Emma wins, deals 6
  g.select(3, 3, false, false); // P2 Shayna
  lives.push([g.p1.life, g.p2.life]);

  assertEquals(lives, [[12, 15], [7, 14], [5, 7]]);
  assertEquals(poisons(g), [false, true, true]);
});

Deno.test("make/unmake walks a replacement back exactly, cache key included", () => {
  const g = game();
  firstTwoRounds(g);
  const cache = new ContinuationCache();
  cache.bind(g);
  const state = () =>
    JSON.stringify([
      g.id,
      g.p1.snapshot(),
      g.p2.snapshot(),
      g.r1.snapshot(),
      g.r2.snapshot(),
      poisons(g),
      g.events1.mask,
      g.events2.mask,
    ]);
  const before = state();
  const key = cache.key(g, Turn.PLAYER_1);
  assertEquals(poisons(g), [true, true]);

  const u1 = new Undo(), u2 = new Undo();
  g.make(2, 3, false, u1); // P1 Emma
  g.make(3, 3, false, u2); // P2 Shayna
  assertEquals([g.p1.life, g.p2.life], [5, 9]);
  assertEquals(poisons(g), [false, true]);
  assertNotEquals(cache.key(g, Turn.PLAYER_1), key);

  g.unmake(u2);
  g.unmake(u1);
  assertEquals(state(), before);
  assertEquals(cache.key(g, Turn.PLAYER_1), key);
});

Deno.test("A latch from the other side replaces one on the same player (1519318)", () => {
  // The owner's own Gork `Backlash: Poison 2, Min 4` and the opposing Obyl Ld's `Poison 1, Min
  // 0` both land on P1, from two `Events`. The newer, latched in round one, replaces the older
  // from round two, when it starts paying: P1 loses 2 a round there, not 3. Both hands are the
  // capture's; the Cosmohnuts `Tune Out` decides every round by the bet.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    HandGenerator.handOf(
      ["Gork", "Mudster", "Akirale", "Pantherine"] as HandOf<string>,
      [5, 2, 3, 3] as HandOf<number | undefined>,
    ),
    HandGenerator.handOf(
      ["Colton", "Maraval", "Obyl Ld", "Sam & Remi Ld"] as HandOf<string>,
      [1, 1, 3, 1] as HandOf<number | undefined>,
    ),
    Turn.PLAYER_1,
    false,
  );
  const lives: number[][] = [];
  g.select(3, 3, false, false); // P1 Pantherine
  g.select(2, 12, false, false); // P2 Obyl Ld wins; its Poison latches on P1
  lives.push([g.p1.life, g.p2.life]);
  g.select(3, 0, false, false); // P2 Sam & Remi Ld
  g.select(0, 5, false, false); // P1 Gork wins; its Backlash Poison latches on P1 too
  lives.push([g.p1.life, g.p2.life]);
  g.select(1, 0, false, false); // P1 Mudster
  g.select(1, 0, false, false); // P2 Maraval wins the tied bet
  lives.push([g.p1.life, g.p2.life]);
  g.select(0, 0, false, false); // P2 Colton wins
  g.select(2, 0, false, false); // P1 Akirale
  lives.push([g.p1.life, g.p2.life]);

  // 15 - 4; 11 - Obyl's 1 (the Backlash is in its delayed round); 10 - 1 - 2, Mudster's
  // `Victory Or Defeat: - 1 Opp. Life` on P2; 7 - 1 - 2, held at the Backlash's Min 4.
  assertEquals(lives, [[11, 15], [10, 7], [7, 6], [4, 6]]);
});
