import { assertEquals } from "@std/assert";
import Player from "@/game/Player.ts";

Deno.test("Player", () => {
  const p = new Player(12, 12, 0);

  assertEquals(p.name, "Player");
  assertEquals(p.life, 12);
  assertEquals(p.pillz, 12);
  assertEquals(p.won, undefined);
  assertEquals(p.wonPrevious, undefined);

  p.life = 24;
  assertEquals(p.life, 24);

  p.pillz = 14;
  assertEquals(p.life, 24);
  assertEquals(p.pillz, 14);

  p.won = true;
  assertEquals(p.life, 24);
  assertEquals(p.pillz, 14);
  assertEquals(p.won, true);

  p.wonPrevious = false;
  assertEquals(p.life, 24);
  assertEquals(p.pillz, 14);
  assertEquals(p.won, true);
  assertEquals(p.wonPrevious, false);

  p.life = -1;
  assertEquals(p.life, 0);

  // p.pillz = -13248974;
  // assertEquals(p.pillz, 0);
});
