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
