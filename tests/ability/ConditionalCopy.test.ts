// A condition printed in front of "Copy: Opp. Ability" / "Copy Opp. Bonus" decides whether
// the copy happens at all; the copied text then still has to meet its own conditions.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

// Captured battle 1091381 round 3: Dr Van Wesel Ld lv1, "Reprisal: Copy Opp. Bonus", against
// Mou lv3, whose Hive bonus is "Equalizer: -3 Opp Attack, Min 5". Both bet 2 (pillzUsed 3).
const vanWesel = (vanWeselFirst: boolean) => {
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(["Aegis Cr", "Hal Gladius", "Lumia Cr", "Mou"] as HandOf<string>, [5, 2, 4, 3] as HandOf<number | undefined>),
    HandGenerator.handOf(["Dark Kupanda", "Agnes", "Dr Van Wesel Ld", "Marlowe"] as HandOf<string>, [2, 1, 1, 2] as HandOf<number | undefined>),
    vanWeselFirst ? Turn.PLAYER_2 : Turn.PLAYER_1,
    false,
    true,
  );
  if (vanWeselFirst) {
    g.select(2, 2, false, false); // Dr Van Wesel Ld
    g.select(3, 2, false, false); // Mou
  } else {
    g.select(3, 2, false, false);
    g.select(2, 2, false, false);
  }
  return { mou: g.h1[3].attack.final, vanWesel: g.h2[2].attack.final };
};

Deno.test("Reprisal Copy copies nothing when its owner moves first", () => {
  // Server: Mou 6 x 3 = 18 with no copied Equalizer; Van Wesel 9 x 3 - 3 x lv1 = 24.
  assertEquals(vanWesel(true), { mou: 18, vanWesel: 24 });
});

Deno.test("Reprisal Copy adopts the opposing bonus when its owner moves second", () => {
  // The copied Equalizer scales with Mou's level 3: 18 - 9 = 9, above the Min of 5.
  assertEquals(vanWesel(false), { mou: 9, vanWesel: 24 });
});

// Captured battle 943231 round 1: Madlocks lv2, "Bet > 3 Pillz: Copy: Opp. Ability", moves
// first against Miyo lv3 ("Stop Opp. Bonus"; Hive bonus "Equalizer: -3 Opp Attack, Min 5").
const madlocks = (bet: number) => {
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(["Aegis Cr", "Hal Gladius", "Miyo", "Nebula"] as HandOf<string>, [5, 2, 3, 3] as HandOf<number | undefined>),
    HandGenerator.handOf(["Akrakk Cr", "Madlocks", "Ryujin Cr", "Zaria"] as HandOf<string>, [4, 2, 2, 3] as HandOf<number | undefined>),
    Turn.PLAYER_2,
    false,
  );
  g.select(1, bet, false, false); // Madlocks
  g.select(2, 0, false, false); // Miyo
  return g.h2[1].attack.final;
};

Deno.test("Bet-gated Copy copies nothing below its bet", () => {
  // pillzUsed 3 is not > 3: Miyo's Stop removes Madlocks' Raptors bonus and her Equalizer
  // lands, 9 x 3 - 3 x lv2 = 21, as the server shows.
  assertEquals(madlocks(2), 21);
});

Deno.test("Bet-gated Copy adopts the opposing ability above its bet", () => {
  // pillzUsed 4: Madlocks copies Miyo's Stop Opp. Bonus, which removes her Equalizer.
  assertEquals(madlocks(3), 36);
});
