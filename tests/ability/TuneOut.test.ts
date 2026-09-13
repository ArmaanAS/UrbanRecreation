import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";

Deno.test("Tune Out", () => {
  const h1 = HandGenerator.generate(
    "Genmaicha",
    "Orka",
    "Sando",
    "Deborah",
  );
  const h2 = HandGenerator.generate(
    "Nathan",
    "El Kuzco",
    "Noon Steevens",
    "Strygia",
  );
  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);

  const g = new Game(p1, p2, h1, h2, Turn.PLAYER_1);

  g.select(0, 4); // Genmaicha
  g.select(2, 5); // Noon Steevens

  assertEquals(g.p1.life, 8);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 8);
  assertEquals(g.p2.pillz, 7);
});

Deno.test("Tune Out ignores Attack modifiers and compares pillz only", () => {
  const hive = HandGenerator.handOf([
    "AI-Lycs",
    "Lumia Cr",
    "Miyo",
    "Nebula",
  ]);
  const cosmohnuts = HandGenerator.handOf([
    "Alana",
    "Colton",
    "Pepe Andrei",
    "Sam & Remi Ld",
  ]);
  const g = new Game(
    new Player(12, 5, 0),
    new Player(4, 10, 1),
    hive,
    cosmohnuts,
    Turn.PLAYER_1,
  );

  // Battle 964404, round 3: Hive's Equalizer -3 Opp Attack incorrectly changed Pepe's
  // pillz-only 7 Attack to 5, making Nebula's 6 look like a certain knockout.
  g.select(3, 5); // Nebula: six effective pillz
  g.select(2, 6); // Pepe Andrei: seven effective pillz

  assertEquals(g.h1[3].power.final, 1);
  assertEquals(g.h2[2].power.final, 1);
  assertEquals(g.h1[3].attack.final, 6);
  assertEquals(g.h2[2].attack.final, 7);
  assertEquals(g.h1[3].won, false);
  assertEquals(g.h2[2].won, true);
  assertEquals(g.p1.life, 7);
  assertEquals(g.p2.life, 4);
});
