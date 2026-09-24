// A reduction that names no opponent lowers its owner's own card. Bugamon lv2's "Growth: -1
// Power And Damage, Min 4" is a drawback (captures/abilities.json 1676, sideAffected
// "player"); the engine used to aim every "-N <stat>" at the opposing card because
// normalising drops the "Opp" it would have looked for. Captured battles 1088641 r0 and
// 1414749 r1 both show Bugamon falling by the round number while the opposing card is
// untouched.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

// The opening of captured battle 1088641. Bugamon is its hand's only Dominion card, so it
// has no bonus, and the Nightmare "Stop Opp. Bonus" never reaches the Rescue Support.
const game = () =>
  new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(["Aurora", "Lothar", "Spidee", "Sue"] as HandOf<string>, [5, 2, 4, 2] as HandOf<number | undefined>),
    HandGenerator.handOf(["Bugamon", "Bapho Ld", "Nistarok", "Senestra"] as HandOf<string>, [2, 2, 5, 3] as HandOf<number | undefined>),
    Turn.PLAYER_1,
    false,
    true,
  );

Deno.test("Growth -1 Power And Damage lowers its own card", () => {
  const g = game();

  g.select(0, 6, false, false); // p1 Aurora lv5, 7/5
  g.select(0, 4, false, false); // p2 Bugamon lv2, 8/7

  const [aurora, bugamon] = [g.h1[0], g.h2[0]];
  assertEquals([bugamon.power.final, bugamon.damage.final, bugamon.attack.final], [7, 6, 35]);
  assertEquals([aurora.power.final, aurora.damage.final, aurora.attack.final], [7, 5, 61]);
  assertEquals(aurora.won, true);
  assertEquals([g.p1.life, g.p2.life], [15, 7]); // Aurora's +3 Life, then her 5 Damage
});
