// The reductions of one stat resolve across both sides by descending Min (`Events.executeCuts`),
// not one side and then the other. It decides the rounds where both sides reduce one card: the
// owner's own half of a `Cards` reduction against the opposing card's cut. Every round below is a
// captured one, with the server's numbers; the seat is the one the capture had.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

const hand = (names: string[], levels: number[]) =>
  HandGenerator.handOf(names as HandOf<string>, levels as HandOf<number | undefined>);

Deno.test("An opposing Min 2 goes before the owner's own Cards Min 0 (1526067 r1)", () => {
  // Pere Fourrure lv4 (8/6, P1) and his `Support: -1 Cards Damage, Min 0` x 2 Jungo meet Niva
  // lv4's `Courage: -4 Opp. Dmg, Min 2` (P2, moving first): 6 -> 2 -> 0. P1 first gave 2.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Buba", "Pere Fourrure", "Ciro", "Yamaboshi"], [4, 4, 5, 3]),
    hand(["Elea Cr", "Niva", "Rodney", "Wendy"], [2, 4, 3, 1]),
    Turn.PLAYER_1,
    false,
    true,
  );
  g.select(3, 2, false, false); // P1 Yamaboshi lv3
  g.select(2, 0, false, false); // P2 Rodney lv3
  g.select(1, 0, false, false); // P2 Niva lv4, first: Courage
  g.select(1, 0, false, false); // P1 Pere Fourrure lv4
  assertEquals(g.h1[1].damage.final, 0);
  assertEquals(g.h2[1].damage.final, 2); // 4 - 2, the Cards' opposing half
});

Deno.test("The owner's own Cards Min 4 goes before an opposing Min 3 (1079078 r3)", () => {
  // Rajesh lv2 (5/6, P2) and his `-2 Cards Damage, Min 4` meet Sue lv2's `-1 Opp Power And
  // Damage, Min 3` (P1): 6 -> 4 -> 3. P1 first gave 6 -> 5, then 3 held at Min 4, so 4.
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    hand(["Anita", "Aurora", "Sue", "Tina"], [3, 5, 2, 3]),
    hand(["Catherine", "Droult", "Glenn", "Rajesh"], [4, 2, 5, 2]),
    Turn.PLAYER_1,
  );
  g.select(2, 0, false, false); // P1 Sue lv2, 6/3
  g.select(3, 0, false, false); // P2 Rajesh lv2
  assertEquals([g.h2[3].power.final, g.h2[3].damage.final], [4, 3]);
  assertEquals(g.h1[2].damage.final, 3); // already at or below the Cards' Min 4
});

Deno.test("An opposing Attack Min 6 goes before the owner's own Cards Min 0 (1525903 r1)", () => {
  // Miss Denna lv1 (5/4) on 1 pill is 10; Magenta lv4's `-6 Opp Attack, Min 6` holds her at
  // 6, and her own `-7 Cards Attack, Min 0` then takes her to 0. Played with Miss Denna as P1,
  // where running P1 first would cut her to 3 and leave Magenta's Min 6 nothing to do.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Meow", "Micciui", "Miss Denna", "Svelthlana"], [5, 4, 1, 3]),
    hand(["Adler", "Karconte", "Magenta", "Phillips"], [5, 2, 4, 4]),
    Turn.PLAYER_1,
    false,
    true,
  );
  g.select(2, 1, false, false); // P1 Miss Denna lv1
  g.select(2, 5, false, false); // P2 Magenta lv4
  assertEquals(g.h1[2].attack.final, 0);
  assertEquals(g.h2[2].attack.final, 29); // 6 x 6 - 7
});
