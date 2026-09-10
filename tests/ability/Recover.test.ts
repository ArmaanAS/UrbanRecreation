// Recover N Pillz Out Of M — on a Defeat trigger the owner gets back
// max(1, ceil(bet × N / M)) pillz. The bet is the engine's, i.e. without the free pill:
// captured battle 901400 r1 has D-aleq "Defeat: Recover 2 Pillz Out Of 3" lose on a bet of
// 3 and recover 2, where counting the free pill would give ceil(4 × 2/3) = 3. Rounding is
// always up, and a triggered Recover never gives nothing — 877983 r1 has Eebiza "Defeat:
// Recover 1 Pillz Out Of 2" lose on a bet of 0 and still recover 1.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

// Junkz, whose bonus is "Attack +8" and so cannot move anyone's pillz.
// 1: Eebiza lv2, "Defeat: Recover 1 Pillz Out Of 2".
// 2: Sasl Lovelace lv4, "Defeat: Recover 2 Pillz Out Of 3".
const game = () =>
  new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(["Berserkgirl Cr", "Eebiza", "Sasl Lovelace", "Veenyle Cr"] as HandOf<string>, [3, 2, 4, 2] as HandOf<number | undefined>),
    HandGenerator.handOf(["Buck", "Lianah Ld", "Shaun", "Zatapa"] as HandOf<string>, [4, 3, 3, 3] as HandOf<number | undefined>),
    Turn.PLAYER_1,
  );

Deno.test("Recover rounds up", () => {
  const g = game();

  g.select(2, 3, false, false); // p1 Sasl Lovelace, 3 pillz — loses
  g.select(0, 5, false, false); // p2 Buck, 5 pillz

  assertEquals(g.h1[2].won, false);
  assertEquals(g.p1.pillz, 11); // 12 - 3 spent, + ceil(3 × 2/3) = 2
});

Deno.test("Recover gives at least one pillz on a zero bet", () => {
  const g = game();

  g.select(1, 0, false, false); // p1 Eebiza, no pillz — loses
  g.select(0, 5, false, false); // p2 Buck, 5 pillz

  assertEquals(g.h1[1].won, false);
  assertEquals(g.p1.pillz, 13); // nothing spent, and ceil(0 × 1/2) = 0 is floored up to 1
});

Deno.test("Recover does nothing on a win", () => {
  const g = game();

  g.select(2, 3, false, false); // p1 Sasl Lovelace, 3 pillz — wins, so Defeat never fires
  g.select(0, 0, false, false); // p2 Buck, no pillz

  assertEquals(g.h1[2].won, true);
  assertEquals(g.p1.pillz, 9); // 12 - 3 spent, nothing back
});
