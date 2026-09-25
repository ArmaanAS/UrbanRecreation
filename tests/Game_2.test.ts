// A whole Ulu Watu against Nightmare game. This file began as a copy of Game_1.test.ts
// with its card names blanked and only the first two rounds' move comments rewritten
// (Madabook against Lianah Ld, then Dave against Cybil); it never ran. The hands keep
// those four cards in the slots the moves pointed at, fill the rest from Game_1, and pin
// every level so later data refreshes cannot move it. Each rule it leans on is one the
// captures show:
// - Nightmare's "Stop Opp. Bonus" cancels the Ulu Watu "Power +2": Lianah Ld fights at
//   8 in 878056 r2, Zatapa at 6 in 878056 r0, Eugene at 7 in 878120 r1.
// - Stop Opp. Ability keeps Heal from latching: in 877733 Lianah Ld beats Pr Balthazar in
//   r0 and her owner never heals, while in 878093 she wins r0 unopposed and heals 12 -> 13
//   at the end of r1.
// - Copy: Power And Damage Opp. takes the opposing card's printed values (1069345 r0).
// - Damage Exchange makes the winner deal the loser's Damage (1065557 r1 Incubus Cr deals
//   Lothar's 5; 901004 r0 Kubrat Cr deals Waldegrin Cr's 1).
// - Defeat: +2 Life pays the loser after the hit (878120 r1 and 1091770 r1, both Eugene).
// Attack = Power x (bet + 1), plus Attack modifiers.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game, { Winner } from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";

const stats = (
  c: { power: { final: number }; damage: { final: number }; attack: { final: number } },
) => [c.power.final, c.damage.final, c.attack.final];

Deno.test("Ulu Watu against Nightmare: Stop Opp. Ability, Copy, Damage Exchange, Defeat Life", () => {
  const h1 = HandGenerator.handOf(
    ["Dave", "Eugene", "Lianah Ld", "Zatapa"],
    [2, 3, 3, 3],
  );
  const h2 = HandGenerator.handOf(
    ["Cybil", "Candy Jack", "Incubus Cr", "Madabook"],
    [2, 3, 4, 5],
  );
  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);

  const g = new Game(p1, p2, h1, h2, Turn.PLAYER_2, false);

  assertEquals(g.p1.life, 12);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);

  // Round 1: Madabook (lv5, 8/5, Stop Opp. Ability) 8 x 3 = 24 against Lianah Ld (lv3, 8/3,
  // Heal 1 Max. 20), 8 x 6 = 48 with her bonus stopped. She wins for 3, but her Heal is
  // stopped and never latches.
  g.select(3, 2); // Madabook
  g.select(2, 5); // Lianah Ld

  assertEquals(stats(g.h2[3]), [8, 5, 24]);
  assertEquals(stats(g.h1[2]), [8, 3, 48]);
  assertEquals(g.p1.life, 12);
  assertEquals(g.p2.life, 9);
  assertEquals(g.p1.pillz, 7);
  assertEquals(g.p2.pillz, 10);

  // Round 2: Dave (lv2, 8/1, +2 Life) 8 x 1 = 8. Cybil (lv2, 7/4) copies his 8/1:
  // 8 x 3 = 24 and deals 1. A latched Heal would have put p1 back on 12 here.
  g.select(0, 0); // Dave
  g.select(0, 2); // Cybil

  assertEquals(stats(g.h1[0]), [8, 1, 8]);
  assertEquals(stats(g.h2[0]), [8, 1, 24]);
  assertEquals(g.p1.life, 11);
  assertEquals(g.p2.life, 9);
  assertEquals(g.p1.pillz, 7);
  assertEquals(g.p2.pillz, 8);

  // Round 3: Incubus Cr (lv4, 7/1, Damage Exchange) 7 x 1 = 7 loses to Zatapa (lv3, 6/4)
  // 6 x 2 = 12, who deals Incubus Cr's 1 instead of her own 4.
  g.select(2, 0); // Incubus Cr
  g.select(3, 1); // Zatapa

  assertEquals(stats(g.h1[3]), [6, 1, 12]);
  assertEquals([g.h2[2].power.final, g.h2[2].attack.final], [7, 7]);
  assertEquals(g.p1.life, 11);
  assertEquals(g.p2.life, 8);
  assertEquals(g.p1.pillz, 6);
  assertEquals(g.p2.pillz, 8);

  // Round 4: Eugene (lv3, 7/4, Defeat: +2 Life) 7 x 1 = 7 loses to Candy Jack (lv3, 3/7)
  // 3 x 3 = 9, so p1 goes 11 - 7 = 4, then + 2 = 6, and P2 wins 8 to 6.
  g.select(1, 0); // Eugene
  g.select(1, 2); // Candy Jack

  assertEquals(stats(g.h1[1]), [7, 4, 7]);
  assertEquals(stats(g.h2[1]), [3, 7, 9]);
  assertEquals(g.p1.life, 6);
  assertEquals(g.p2.life, 8);
  assertEquals(g.p1.pillz, 6);
  assertEquals(g.p2.pillz, 6);
  assertEquals(g.winner, Winner.PLAYER_2);
});
