// Recover N Pillz Out Of M — when it triggers, the owner gets back
// max(1, floor(placed × N / M)) pillz, where placed is the bet plus the free pill plus
// Fury's three, as the server's long description says ("of the Pillz placed on him, rounded
// down ... with a minimum of 1"). Captured battle 901400 r1 has D-aleq "Defeat: Recover 2
// Pillz Out Of 3" lose on a bet of 3 and recover floor(4 × 2/3) = 2; 877983 r1 has Eebiza
// "Defeat: Recover 1 Pillz Out Of 2" lose on a bet of 0 and still recover 1. For 1/2 and 2/3
// this equals ceil(bet × N / M); only 1/3 separates them (947010 r0, below).
// The plain form pays only when its card wins, and "Defeat:" only when it loses.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

// Junkz, whose bonus is "Attack +8" and so cannot move anyone's pillz.
// 1: Eebiza lv2, "Defeat: Recover 1 Pillz Out Of 2".
// 2: Sasl Lovelace lv4, "Defeat: Recover 2 Pillz Out Of 3".
const game = () =>
  new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(["Berserkgirl Cr", "Eebiza", "Sasl Lovelace", "Veenyle Cr"] as HandOf<string>, [3, 2, 4, 2] as HandOf<number | undefined>),
    HandGenerator.handOf(["Buck", "Lianah Ld", "Shaun", "Zatapa"] as HandOf<string>, [4, 3, 3, 3] as HandOf<number | undefined>),
    Turn.PLAYER_1,
  );

Deno.test("Defeat: Recover pays two thirds of the placed pillz", () => {
  const g = game();

  g.select(2, 3, false, false); // p1 Sasl Lovelace, 3 pillz — loses
  g.select(0, 5, false, false); // p2 Buck, 5 pillz

  assertEquals(g.h1[2].won, false);
  assertEquals(g.p1.pillz, 11); // 12 - 3 spent, + floor(4 × 2/3) = 2
});

Deno.test("Recover gives at least one pillz on a zero bet", () => {
  const g = game();

  g.select(1, 0, false, false); // p1 Eebiza, no pillz — loses
  g.select(0, 5, false, false); // p2 Buck, 5 pillz

  assertEquals(g.h1[1].won, false);
  assertEquals(g.p1.pillz, 13); // nothing spent, and floor(1 × 1/2) = 0 is raised to 1
});

Deno.test("Recover does nothing on a win", () => {
  const g = game();

  g.select(2, 3, false, false); // p1 Sasl Lovelace, 3 pillz — wins, so Defeat never fires
  g.select(0, 0, false, false); // p2 Buck, no pillz

  assertEquals(g.h1[2].won, true);
  assertEquals(g.p1.pillz, 9); // 12 - 3 spent, nothing back
});

// Captured battle 946810 r0: Costello lv3, "Recover 1 Pillz Out Of 3", bets 5 and loses to
// Uuber. The server posts no pillz increase: 12 - 5 = 7.
Deno.test("Plain Recover pays nothing on a loss", () => {
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(["AI-Lycs", "Lumia Cr", "Miyo", "Uuber"] as HandOf<string>, [3, 4, 3, 2] as HandOf<number | undefined>),
    HandGenerator.handOf(["Costello", "Frau Vanda", "Maelt Riv", "Tør"] as HandOf<string>, [3, 2, 3, 3] as HandOf<number | undefined>),
    Turn.PLAYER_1,
    false,
    true,
  );

  g.select(3, 4, false, false); // p1 Uuber
  g.select(0, 5, false, false); // p2 Costello

  assertEquals(g.h2[0].won, false);
  assertEquals(g.p2.pillz, 7);
});

// Captured battle 947010 r0: Kyrioz Ld lv2, "Recover 1 Pillz Out Of 3", bets 7 and wins.
// The server recovers floor(8 / 3) = 2 of the 8 placed pillz (ceil(7 / 3) would be 3), and
// Kyrioz's Bet > 3 bonus and Uuber's -1 Life move nothing else: 12 - 7 + 2 = 7.
Deno.test("Plain Recover rounds the placed pillz down on a win", () => {
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(["Aegis Cr", "Hal Gladius", "Lumia Cr", "Uuber"] as HandOf<string>, [5, 2, 4, 2] as HandOf<number | undefined>),
    HandGenerator.handOf(["Dayo", "Kyrioz Ld", "Omurtag", "Phelemon"] as HandOf<string>, [4, 2, 3, 4] as HandOf<number | undefined>),
    Turn.PLAYER_1,
    false,
    true,
  );

  g.select(3, 4, false, false); // p1 Uuber
  g.select(1, 7, false, false); // p2 Kyrioz Ld

  assertEquals(g.h2[1].won, true);
  assertEquals(g.p2.pillz, 7);
});
