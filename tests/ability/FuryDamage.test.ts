// Fury's +2 Damage is settled where the Damage is dealt, so an Attack modifier reading the
// opponent's Damage sees the printed value. Battle 1130726 r3: Goran's "+2 Attack Per Opp.
// Damage" is worth +4 against a Fury Uuber, not +8, and the round ends 28 Attack to 24.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

const game = () =>
  new Game(
    new Player(6, 6, 0),
    new Player(23, 3, 1),
    HandGenerator.handOf(
      ["AI-Lycs", "Aegis Cr", "Mou", "Uuber"] as HandOf<string>,
      [3, 5, 3, 2] as HandOf<number | undefined>,
    ),
    HandGenerator.handOf(
      ["Goran", "NBara", "Talhia", "Tervel"] as HandOf<string>,
      [4, 2, 4, 3] as HandOf<number | undefined>,
    ),
    Turn.PLAYER_2,
  );

Deno.test("Per Opp. Damage ignores the opponent's Fury", () => {
  const g = game();

  g.select(0, 3, false, false); // Goran: 8 x 4, +2 per Uuber's printed 2 Damage
  g.select(3, 3, true, false); // Uuber: 7 x 4, Fury

  assertEquals(g.h2[0].attack.final, 24); // 32 + 4 - Hive Equalizer 3 x 4
  assertEquals(g.h1[3].attack.final, 28);
  assertEquals(g.h1[3].won, true);
});

Deno.test("Fury still reaches the Damage dealt", () => {
  const g = game();

  g.select(0, 3, false, false);
  g.select(3, 3, true, false);

  assertEquals(g.h1[3].damage.final, 4); // 2 printed + 2 Fury
  assertEquals(g.p2.life, 18); // 23 - 4 - 1 from Victory Or Defeat
});
