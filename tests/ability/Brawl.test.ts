// Brawl — the mirror of Support: the modifier is multiplied by the number of the opponent's
// characters belonging to the same clan as the card being fought (the server sends it as
// isAntiSupport, e.g. captures/abilities.json 3948). This is already handled by
// Condition.BRAWL -> BasicModifier.setPer("BRAWL"); the test pins it down because captured
// battle 874590 looked like a missing Brawl and was really the Administrator leader's
// "Hazard", which replaces the other three cards' abilities with random ones.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

// ARN 2000 lv3 (GHEIST, power 7 damage 3) has "Brawl: Damage + 1".
const gheist = () =>
  HandGenerator.handOf(
    ["ARN 2000", "Dr Saw", "Z3r0 D34d", "Methane"] as HandOf<string>,
    [3, undefined, undefined, undefined] as HandOf<number | undefined>,
  );

Deno.test("Brawl counts the whole opposing clan", () => {
  // Four distinct Rescue cards, so Brawl multiplies by 4: damage 3 + 4 = 7.
  const h2 = HandGenerator.handOf(
    ["Aurora", "Callie", "Lothar", "Spidee"] as HandOf<string>,
    [5, 3, 2, 4] as HandOf<number | undefined>,
  );
  const g = new Game(new Player(12, 12, 0), new Player(12, 12, 1), gheist(), h2, Turn.PLAYER_1);

  g.select(0, 0, false, false); // p1 ARN 2000
  g.select(2, 0, false, false); // p2 Lothar (Rescue)

  assertEquals(g.h1[0].damage.final, 7);
});

Deno.test("Brawl counts only the clan of the card being fought", () => {
  // Two Rescue and two Nightmare: fighting a Rescue card multiplies by 2 only.
  const h2 = HandGenerator.handOf(
    ["Aurora", "Callie", "Diabolus", "Nero Cr"] as HandOf<string>,
    [5, 3, 1, 4] as HandOf<number | undefined>,
  );
  const g = new Game(new Player(12, 12, 0), new Player(12, 12, 1), gheist(), h2, Turn.PLAYER_1);

  g.select(0, 0, false, false); // p1 ARN 2000
  g.select(1, 0, false, false); // p2 Callie (Rescue)

  assertEquals(g.h1[0].damage.final, 5);
});
