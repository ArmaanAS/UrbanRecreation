// Cards <stat> +N — captures/abilities.json 3295: "The Damage points of both characters are
// increased by 2 points" (sideAffected "both"). "Cards" marks a modifier that lands once on
// each side, the way "Players" already did for Pillz; the negative shape "-2 Cards Damage,
// Min 1" (4957) was already handled. Captured battle 874795 r0 is the reference: El
// Resbaladizo lv4 (base damage 6) fights at 8 and Aurora lv5 (base 5) at 7, in one round.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

Deno.test("Cards Damage raises both sides", () => {
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(["El Resbaladizo", "La Chispa", "La Picaflor", "Vola Dolores"] as HandOf<string>, [4, 2, 2, 5] as HandOf<number | undefined>),
    HandGenerator.handOf(["Anita", "Aurora", "Lothar", "Sue"] as HandOf<string>, [3, 5, 2, 2] as HandOf<number | undefined>),
    Turn.PLAYER_1,
  );

  g.select(0, 6, false, false); // p1 El Resbaladizo, "Cards Damage +2"
  g.select(1, 5, false, false); // p2 Aurora, whose only ability is "+3 Life"

  assertEquals(g.h1[0].damage.final, 8); // base 6 + 2
  assertEquals(g.h2[1].damage.final, 7); // base 5 + 2, the opponent gets it too
});
