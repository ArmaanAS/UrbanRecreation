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

// A permanent that is stopped in the round its card is played never starts at all, and the
// check has to happen then: from the next round on, the ability object outlives its card and
// `data.card` is whichever card its owner plays instead. Captured battle 877733 r0: Pr
// Balthazar's "Stop Opp. Ability" meets Lianah Ld's "Heal 1 Max. 20", and the server never
// heals DashSmashing across the three rounds that follow.
const stopped = () =>
  new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(["Agnes", "Bumdo", "Korakine", "Pr Balthazar"] as HandOf<string>, [1, 2, 2, 3] as HandOf<number | undefined>),
    HandGenerator.handOf(["Eugene", "Fanny", "Lianah Ld", "Shaun"] as HandOf<string>, [3, 2, 3, 3] as HandOf<number | undefined>),
    Turn.PLAYER_1,
  );

Deno.test("Heal stopped in its own round never starts", () => {
  const g = stopped();

  g.select(3, 0, false, false); // p1 Pr Balthazar, "Stop Opp. Ability"
  g.select(2, 4, false, false); // p2 Lianah Ld, "Heal 1 Max. 20" — wins, but is stopped
  assertEquals(g.h2[2].won, true);
  assertEquals(g.p2.life, 12);

  // Round 1: player 2 selects first and wins again, so nothing but a heal could move it.
  g.select(3, 5, false, false); // p2 Shaun
  g.select(0, 0, false, false); // p1 Agnes
  assertEquals(g.p2.life, 12);
});

Deno.test("Heal that is not stopped does start", () => {
  const g = stopped();

  g.select(0, 0, false, false); // p1 Agnes — no Stop this time
  g.select(2, 4, false, false); // p2 Lianah Ld wins with its ability intact
  assertEquals(g.h2[2].won, true);
  assertEquals(g.p2.life, 12); // Heal skips the round that started it

  g.select(3, 5, false, false); // p2 Shaun
  g.select(3, 0, false, false); // p1 Pr Balthazar
  assertEquals(g.p2.life, 13);
});
