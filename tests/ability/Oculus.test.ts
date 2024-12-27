import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";

Deno.test("Infiltrated", () => {
  const h1 = HandGenerator.generate(
    "Alekperov",
    "Eugene",
    "Hikiyousan",
    "Zatapa",
  );
  const h2 = HandGenerator.generate(
    "Betelgeuse",
    "Candy Jack",
    "Incubus",
    "Tamakuchi",
  );
  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);

  const g = new Game(p1, p2, h1, h2, Turn.PLAYER_1);

  assertEquals(g.p1.life, 12);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);

  g.select(0, 0); // Alekperov
  g.select(3, 0); // Tamakuchi

  assertEquals(g.p1.life, 12);
  assertEquals(g.p2.life, 8);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);
});

Deno.test("Infiltrated No Ability", () => {
  const h1 = HandGenerator.generate(
    "Alekperov",
    "Agnes",
    "Hikiyousan",
    "Zatapa",
  );
  const h2 = HandGenerator.generate(
    "Betelgeuse",
    "Candy Jack",
    "Incubus",
    "Tamakuchi",
  );
  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);

  const g = new Game(p1, p2, h1, h2, Turn.PLAYER_1);

  assertEquals(g.p1.life, 12);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);

  g.select(0, 0); // Alekperov
  g.select(3, 0); // Tamakuchi

  assertEquals(g.p1.life, 7);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);
});

Deno.test("No Infiltrated", () => {
  const h1 = HandGenerator.generate(
    "Alekperov",
    "Agnes",
    "Hikiyousan",
    "Iris Morana",
  );
  const h2 = HandGenerator.generate(
    "Betelgeuse",
    "Candy Jack",
    "Incubus",
    "Tamakuchi",
  );
  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);

  const g = new Game(p1, p2, h1, h2, Turn.PLAYER_1);

  assertEquals(g.p1.life, 12);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);

  g.select(0, 0); // Alekperov
  g.select(3, 0); // Tamakuchi

  assertEquals(g.p1.life, 7);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);
});
