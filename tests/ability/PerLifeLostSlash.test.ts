// Wachtmann lv3 prints "[clan...] +1 Dam./ Life Lost Max. 6" (ability 5113). The slash is
// "per", not "and": the server adds 1 Damage for every Life its owner has lost since the
// start of the match and caps the resulting Damage at 6. Hands from captured battle 925818,
// where Wachtmann (an Oculus infiltrating Roots) is at 8 Life in round 2 and deals 1 + 4 = 5;
// 924890/3 has him at 5 Life, where 1 + 7 is capped to 6, and 1090269/0 at full Life deals 1.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";
import { Abilities } from "@/game/AbilityParser.ts";

const wachtmannDamage = (life: number) => {
  const p1 = new Player(12, 12, 0);
  p1.life = life;
  const game = new Game(
    p1,
    new Player(12, 12, 1),
    HandGenerator.handOf(["Wachtmann", "Avani", "Gandelf", "Kalija"] as HandOf<string>, [3, 3, 3, 4] as HandOf<number | undefined>),
    HandGenerator.handOf(["Hal Gladius", "Lumia Cr", "Nebula", "Uuber"] as HandOf<string>, [2, 4, 3, 2] as HandOf<number | undefined>),
    Turn.PLAYER_1,
    false,
  );
  game.select(0, 1, false, false); // Wachtmann
  game.select(3, 0, false, false); // Uuber
  return game.h1[0].damage.final;
};

Deno.test("Slash before Life Lost reads as Per", () => {
  assertEquals(
    Abilities.split("[clan:47][clan:43][clan:54][clan:29][clan:10][clan:59] +1 Dam./ Life Lost Max. 6").pop(),
    "+1 Damage Per Life Lost Max 6",
  );
  // The only other slash in the card list still means "and".
  assertEquals(Abilities.split("Cancel Opp. Pow/dam Mod.").pop(), "Cancel Power&Damage");
});

Deno.test("Wachtmann adds nothing at full Life", () => {
  assertEquals(wachtmannDamage(12), 1);
});

Deno.test("Wachtmann adds one Damage per Life lost", () => {
  assertEquals(wachtmannDamage(8), 5); // 925818/2
  assertEquals(wachtmannDamage(7), 6); // 1092066/3
});

Deno.test("Wachtmann's Max caps the resulting Damage", () => {
  assertEquals(wachtmannDamage(5), 6); // 924890/3: 1 + 7, capped at 6
});
