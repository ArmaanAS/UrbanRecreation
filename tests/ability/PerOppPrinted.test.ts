// Every magnitude per opposing Power or Damage counts the opposing card's printed stat.
// `+N Attack Per Opp. Damage` was settled first (`PerOppDamage.test.ts`); the autoplay runs 5-10
// show `+N Life Per Opp. Damage` and `+N Attack Per Opp. Power` doing the same, against the
// opposing card's own modifiers, Fury, and the converting side's own cut. Every round below is a
// captured one, with the server's numbers.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

const hand = (names: string[], levels: number[]) =>
  HandGenerator.handOf(names as HandOf<string>, levels as HandOf<number | undefined>);
const stats = (c: { power: { final: number }; damage: { final: number }; attack: { final: number } }) =>
  [c.power.final, c.damage.final, c.attack.final];

Deno.test("Life Per Opp. Damage ignores the opposing ability and bonus (1521354 r2)", () => {
  // Sakazuki lv2 prints 6/2 and fights on 6 Damage, his `Damage +2` and the Fang Pi Clang
  // `Damage +2`. Ciminompah lv4's `+1 Life Per Opp. Damage` wins and pays the printed 2.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Dokuja Ld", "Sakazuki", "Sando", "Sung Tsu"], [2, 2, 3, 5]),
    hand(["Ciminompah", "Eliska", "Ace", "Mattachione"], [4, 4, 2, 2]),
    Turn.PLAYER_1,
  );
  g.select(1, 0, false, false); // P1 Sakazuki lv2
  g.select(0, 0, false, false); // P2 Ciminompah lv4
  assertEquals(g.h2[0].won, true);
  assertEquals(g.h1[1].damage.final, 6);
  assertEquals(g.p2.life, 17); // 15 + 2, where the engine paid 6
});

Deno.test("Life Per Opp. Damage ignores Fury and a Courage increase (1525452 r1)", () => {
  // Boomstock Cr lv2 prints 6/3; his Courage `Power And Damage + 2` and Fury take the Damage to
  // 7. AbsorptionBoy lv4's `+1 Life Per Opp. Damage` pays 3: 15 -> 18.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Qubik", "Rowdy Cr", "AbsorptionBoy", "Rainbow"], [4, 4, 4, 5]),
    hand(["Boomstock Cr", "Geo", "Kenjy", "Reeplay"], [2, 2, 2, 3]),
    Turn.PLAYER_1,
    false,
    true,
  );
  g.select(3, 2, false, false); // P1 Rainbow lv5
  g.select(1, 0, false, false); // P2 Geo lv2
  assertEquals([g.p1.life, g.p2.life], [15, 10]);
  g.select(0, 0, true, false); // P2 Boomstock Cr lv2, Fury, moving first
  g.select(2, 6, true, false); // P1 AbsorptionBoy lv4, Fury
  assertEquals(g.h1[2].won, true);
  assertEquals(g.h2[0].damage.final, 7);
  assertEquals(g.p1.life, 18);
});

Deno.test("Attack Per Opp. Power ignores the opposing Power increase (1525823 r0)", () => {
  // Stacey lv2 prints 6 Power and fights on 8 with her `Power +2`. Skinny Bob Ld lv2 (6 + 2
  // Bangers - 2 All Stars = 6) counts the printed 6: 6 x 1 + 2 x 6 = 18 (the engine had 22).
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Davina", "Miss Jessie", "Morrigan", "Stacey"], [3, 2, 5, 2]),
    hand(["Skinny Bob Ld", "Usman", "Arkn", "Vact"], [2, 4, 2, 2]),
    Turn.PLAYER_1,
    false,
    true,
  );
  g.select(3, 1, true, false); // P1 Stacey lv2, Fury
  g.select(0, 0, false, false); // P2 Skinny Bob Ld lv2
  assertEquals(stats(g.h1[3]), [8, 4, 16]);
  assertEquals(stats(g.h2[0]), [6, 3, 18]);
});

Deno.test("Attack Per Opp. Power ignores the converting side's own cut (1522916 r0)", () => {
  // Taurite lv1's `Unison : Copy: Opp. Ability` copies Lakross lv3's `+1 Attack Per Opp.
  // Power`. Taurite's own GhosTown night bonus `-1 Opp Pow. And Damage, Min 1` takes Lakross
  // from 6 to 5 Power, and the copy still counts 6: 7 x 13 + 6 = 97 (the engine had 96).
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Miss Calamity", "Taurite", "Tuco", "Victor Van Dort"], [3, 1, 2, 4]),
    hand(["Lakross", "Kid Teleon", "Kyrioz Ld", "Slatka"], [3, 3, 2, 2]),
    Turn.PLAYER_1,
    false,
    true,
  );
  g.select(1, 12, false, false); // P1 Taurite lv1
  g.select(0, 2, false, false); // P2 Lakross lv3
  assertEquals(stats(g.h2[0]), [5, 4, 22]);
  assertEquals(stats(g.h1[1]), [7, 1, 97]);
});
