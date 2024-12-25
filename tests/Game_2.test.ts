import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";

Deno.test("Protection", () => {
  const h1 = HandGenerator.generate(
    "",
    "",
    "",
    "",
  );
  const h2 = HandGenerator.generate(
    "",
    "",
    "",
    "",
  );
  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);

  const g = new Game(p1, p2, h1, h2, Turn.PLAYER_2);

  assertEquals(g.p1.life, 12);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);

  g.select(3, 2); // Madabook
  g.select(2, 5); // Lianah Ld

  assertEquals(g.p1.life, 9);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 7);
  assertEquals(g.p2.pillz, 10);

  g.select(0, 0); // Dave
  g.select(0, 2); // Cybil

  assertEquals(g.p1.life, 9);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 10);

  g.select(0, 0); // Betelgeuse
  g.select(0, 0); // Dave

  assertEquals(g.p1.life, 9);
  assertEquals(g.p2.life, 11);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 10);
});
