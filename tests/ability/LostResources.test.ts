import "colors";
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

Deno.test("Attack Per Pillz Lost uses the match-start pillz total", () => {
  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);
  // Nolegs reached the captured final round with no pillz, having started on twelve.
  p1.pillz = 0;
  const h1 = HandGenerator.handOf(
    ["Nolegs", "Olga", "Grudj Cr", "Zaveli"] as HandOf<string>,
    [1, undefined, undefined, undefined] as HandOf<number | undefined>,
  );
  const h2 = HandGenerator.handOf(
    ["Natrang", "Sai San", "Windy Mor", "Wardog"] as HandOf<string>,
  );
  const game = new Game(p1, p2, h1, h2, Turn.PLAYER_1, false);

  game.select(0, 0, false, false);
  game.select(0, 0, false, false);

  // 6 base Attack plus 2 for each of the 12 pillz already lost.
  assertEquals(game.h1[0].attack.final, 30);
});

Deno.test("Per Life Lost retains its Max clause", () => {
  const p1 = new Player(12, 12, 0);
  p1.life = 1;
  const h1 = HandGenerator.handOf(
    ["Razor", "Natrang", "Akiko", "Wanda Cr"] as HandOf<string>,
    [4, undefined, undefined, undefined] as HandOf<number | undefined>,
  );
  const h2 = HandGenerator.handOf(
    ["Sai San", "Windy Mor", "Wardog", "Ottavia"] as HandOf<string>,
  );
  const game = new Game(
    p1,
    new Player(12, 12, 1),
    h1,
    h2,
    Turn.PLAYER_1,
    false,
  );

  game.select(0, 0, false, false);
  game.select(0, 0, false, false);

  // Razor starts at 5 Power and has lost 11 Life, but its written Max is 9.
  assertEquals(game.h1[0].power.final, 9);
});
