import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";

Deno.test("Protection", () => {
  const h1 = HandGenerator.generate(
    "Pr Cushing Cr",
    "Rockwall",
    "Arantxa",
    "Danae",
  );
  const h2 = HandGenerator.generate("Gemini", "Nebula", "Oxo", "TrinmkkT");
  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);

  const g = new Game(p1, p2, h1, h2);

  assertEquals(g.p1.life, 12);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);

  g.select(1, 0); // Rockwall
  g.select(2, 0); // Oxo

  assertEquals(g.p1.life, 9);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 13);
  assertEquals(g.p2.pillz, 12);
});
