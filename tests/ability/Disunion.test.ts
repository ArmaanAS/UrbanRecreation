// `Disunion:` is Unison's complement: "The ability is only activated if your hand contains at
// least one character from a clan other than the Disunion character. Oculus characters who have
// infiltrated the clan do not count towards the activation of the ability" (captures/abilities.json
// 4388 Bernardite, 5751 Karsen). It was an unknown condition, met unconditionally. Both rounds
// below face the Cosmohnuts `Tune Out`, so the larger bet wins whatever the Attack.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

const hand = (names: string[], levels: number[]) =>
  HandGenerator.handOf(names as HandOf<string>, levels as HandOf<number | undefined>);
const cosmohnuts = () =>
  hand(["Curcan Noel", "Maraval", "Monkovski", "Obyl Ld"], [1, 1, 1, 3]);

Deno.test("Disunion stays off in a one-clan hand (1519333 r0)", () => {
  // Four Rescue: Bernardite wins on 5 pillz and her owner stays on 15.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Bernardite", "Cleo", "Pendelton", "Vinny"], [3, 3, 4, 2]),
    cosmohnuts(),
    Turn.PLAYER_1,
  );
  g.select(0, 5, false, false); // P1 Bernardite lv3
  g.select(3, 0, false, false); // P2 Obyl Ld lv3
  assertEquals(g.h1[0].won, true);
  assertEquals(g.p1.life, 15);
  assertEquals(g.p2.life, 10); // Bernardite's 5 Damage
});

Deno.test("Disunion pays beside another clan (1509142 r0)", () => {
  // Ruru is Frozn, so Bernardite's `Disunion: +1 Life` is live and her owner goes 15 -> 16.
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Ruru", "Bernardite", "Elvira Cr", "Mark"], [4, 3, 3, 3]),
    cosmohnuts(),
    Turn.PLAYER_1,
  );
  g.select(1, 5, false, false); // P1 Bernardite lv3
  g.select(3, 0, false, false); // P2 Obyl Ld lv3
  assertEquals(g.h1[1].won, true);
  assertEquals(g.p1.life, 16);
});
