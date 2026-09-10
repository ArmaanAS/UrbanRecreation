// After [clan:…] — captures/abilities.json 5585, the Tolvack clan bonus: "This effect only
// activates if the player of Tolvack played an Oculus or Tolvack character in the previous
// round" (the server's previousClanRequirement). It is a per-round look-back, not "at any
// point earlier in the game", and round 0 has no previous card so it never activates there.
// Captured battle 876464 is the reference: Tør lv4 fights round 0 on its base power of 6,
// then Drava lv3 (base 7) has 10 in round 1 and Maelt Riv lv3 (base 8) has 11 in round 2.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

// Four Tolvack, all carrying "After [clan:56][clan:60] : Power +3" as their clan bonus.
// The opposing Ulu Watu hand only ever modifies its own power, so it cannot skew this.
const game = () =>
  new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(["Dorga", "Drava", "Maelt Riv", "Tør"] as HandOf<string>, [3, 3, 3, 4] as HandOf<number | undefined>),
    HandGenerator.handOf(["Eugene", "Fanny", "Lianah Ld", "Shaun"] as HandOf<string>, [3, 2, 3, 3] as HandOf<number | undefined>),
    Turn.PLAYER_1,
  );

Deno.test("After is inactive in round 0", () => {
  const g = game();

  g.select(3, 0, false, false); // p1 Tør
  g.select(0, 0, false, false); // p2 Eugene

  assertEquals(g.h1[3].power.final, 6); // base power, no +3: nothing was played before it
});

Deno.test("After activates once a listed clan played the round before", () => {
  const g = game();

  g.select(3, 0, false, false); // p1 Tør — Tolvack, clan 60
  g.select(0, 0, false, false); // p2 Eugene

  // Round 1: player 2 selects first.
  g.select(1, 0, false, false); // p2 Fanny
  g.select(1, 0, false, false); // p1 Drava
  assertEquals(g.h1[1].power.final, 10); // base 7 + 3

  g.select(2, 0, false, false); // p1 Maelt Riv
  g.select(2, 0, false, false); // p2 Lianah Ld
  assertEquals(g.h1[2].power.final, 11); // base 8 + 3
});
