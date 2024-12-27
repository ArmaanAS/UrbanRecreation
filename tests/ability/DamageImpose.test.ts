import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";

Deno.test("Versus Riots", () => {
  const h1 = HandGenerator.generate(
    "Kochar",
    "Ashara",
    "Yamaboshi",
    "Dark Kaizerin",
  );
  const h2 = HandGenerator.generate(
    "Betelgeuse",
    "Molder",
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

  g.select(0, 0); // Kochar
  g.select(3, 0); // Tamakuchi

  assertEquals(g.p1.life, 10);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);
});
