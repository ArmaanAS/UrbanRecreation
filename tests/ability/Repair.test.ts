// Repair N, Max M — captures/abilities.json 3796 (Wilo Ld): if the card wins its round, its
// owner gains N Life *and* N Pillz at the end of that round and of every following one,
// each capped at M (isPermanent + isImmediatePermanent, currentRoundRequirement "win").
//
// Player 1 alternates who selects first, so within a round the first select() call is the
// first mover's: player 1 in even rounds, player 2 in odd ones.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

// Wilo Ld lv2 (Dominion, power 6 damage 2) has "Repair 1, Max. 14".
const dominion = ["Wilo Ld", "Frogo", "Orka Cr", "Ludicrite"] as HandOf<string>;
const dominionLv = [2, 2, 4, 3] as HandOf<number | undefined>;
const rescue = ["Aurora", "Callie", "Sue", "Wesley"] as HandOf<string>;
const rescueLv = [5, 3, 2, 3] as HandOf<number | undefined>;

const game = () =>
  new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(dominion, dominionLv),
    HandGenerator.handOf(rescue, rescueLv),
    Turn.PLAYER_1,
  );

Deno.test("Repair on a win", () => {
  const g = game();

  g.select(0, 5, false, false); // p1 Wilo Ld, 5 pillz
  g.select(0, 0, false, false); // p2 Aurora, 0 pillz

  assertEquals(g.h1[0].won, true);
  assertEquals(g.p2.life, 10); // Wilo Ld's 2 damage
  assertEquals(g.p1.life, 13); // +1 Life the same round: Repair is immediate
  assertEquals(g.p1.pillz, 8); // 12 - 5 spent, +1 Pillz
});

Deno.test("Repair repeats every following round", () => {
  const g = game();

  g.select(0, 5, false, false); // p1 Wilo Ld, 5 pillz — wins, starting the effect
  g.select(0, 0, false, false); // p2 Aurora
  assertEquals(g.p1.pillz, 8);

  // Round 1: player 2 selects first. Neither side spends a pill, so the only pillz change
  // on player 1's side is Repair, whether or not Orka Cr wins.
  g.select(1, 0, false, false); // p2 Callie
  g.select(2, 0, false, false); // p1 Orka Cr
  assertEquals(g.p1.pillz, 9);

  g.select(3, 0, false, false); // p1 Ludicrite
  g.select(2, 0, false, false); // p2 Sue
  assertEquals(g.p1.pillz, 10);
});

Deno.test("Repair does not start when the card loses", () => {
  const g = game();

  g.select(0, 0, false, false); // p1 Wilo Ld, no pillz — loses
  g.select(0, 5, false, false); // p2 Aurora, 5 pillz

  assertEquals(g.h1[0].won, false);
  assertEquals(g.p1.life, 7); // Aurora's 5 damage, no Repair
  assertEquals(g.p1.pillz, 12); // nothing spent, nothing repaired

  // ...and it stays off in later rounds.
  g.select(1, 0, false, false); // p2 Callie
  g.select(2, 0, false, false); // p1 Orka Cr
  assertEquals(g.p1.pillz, 12);
});
