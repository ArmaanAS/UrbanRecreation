// Defeat : Heal N, Max M — a permanent that latches on a *loss*, not a win, and then adds
// N Life at the end of every following round (Heal, like Poison, skips the round that
// started it). The engine used to require a win before any permanent could start, which
// discarded this one outright; captured battle 875230 heals from the round after Frogo
// loses. Companion to tests/ability/Repair.test.ts, which covers the win-triggered case.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

// Frogo lv2 (Dominion, power 6 damage 3) has "Defeat : Heal 1 Max. 13".
const game = () =>
  new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(["Frogo", "Wilo Ld", "Orka Cr", "Ludicrite"] as HandOf<string>, [2, 2, 4, 3] as HandOf<number | undefined>),
    HandGenerator.handOf(["Aurora", "Callie", "Sue", "Wesley"] as HandOf<string>, [5, 3, 2, 3] as HandOf<number | undefined>),
    Turn.PLAYER_1,
  );

Deno.test("Defeat Heal starts on a loss", () => {
  const g = game();

  g.select(0, 0, false, false); // p1 Frogo, no pillz — loses
  g.select(0, 5, false, false); // p2 Aurora, 5 pillz

  assertEquals(g.h1[0].won, false);
  assertEquals(g.p1.life, 7); // Aurora's 5 damage; Heal skips the round that started it

  // Round 1: player 2 selects first. Player 1 wins it and so takes no damage, leaving the
  // Heal as the only thing that can move their life.
  g.select(1, 0, false, false); // p2 Callie, no pillz
  g.select(3, 5, false, false); // p1 Ludicrite, 5 pillz
  assertEquals(g.h1[3].won, true);
  assertEquals(g.p1.life, 8);
});

Deno.test("Defeat Heal does not start on a win", () => {
  const g = game();

  g.select(0, 5, false, false); // p1 Frogo, 5 pillz — wins, so Defeat never triggers
  g.select(0, 0, false, false); // p2 Aurora

  assertEquals(g.h1[0].won, true);
  assertEquals(g.p1.life, 12);

  g.select(1, 0, false, false); // p2 Callie
  g.select(3, 5, false, false); // p1 Ludicrite
  assertEquals(g.h1[3].won, true);
  assertEquals(g.p1.life, 12); // still no healing
});
