// `+N Attack Per Opp. Damage` counts the opposing card's printed Damage, before that card's own
// modifiers. The engine counted the modified Damage. Every round below is a captured one, with
// the server's numbers; attack = power x (bet + 1) plus the Attack modifiers. The printed value
// also leaves Fury out, which `FuryDamage.test.ts` pins with Goran (1130726 r3).
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

const hand = (names: string[], levels: number[]) =>
  HandGenerator.handOf(names as HandOf<string>, levels as HandOf<number | undefined>);
const stats = (c: { power: { final: number }; damage: { final: number }; attack: { final: number } }) =>
  [c.power.final, c.damage.final, c.attack.final];

Deno.test("Per Opp. Damage ignores the opposing ability and bonus (1518765 r1)", () => {
  // Kinjo Cr lv5 prints 6/5 and fights on 10 Damage, his `Damage +3` and the Fang Pi Clang
  // `Damage +2`. Adytia Ld lv2's `+2 Attack Per Opp. Damage` counts the printed 5: 8 x 1 + 2 x
  // 5 = 18 (the engine had 28).
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Adytia Ld", "Duygu", "Emel", "Carnibox"], [2, 4, 4, 2]),
    hand(["Chan", "Khann", "Kinjo Cr", "Muntendon"], [1, 5, 5, 2]),
    Turn.PLAYER_1,
  );
  g.select(0, 0, false, false); // P1 Adytia Ld lv2, 8/2
  g.select(2, 1, false, false); // P2 Kinjo Cr lv5, 6/5
  const [adytia, kinjo] = [g.h1[0], g.h2[2]];
  assertEquals(stats(kinjo), [6, 10, 12]);
  assertEquals(stats(adytia), [8, 2, 18]);
});

Deno.test("Per Opp. Damage ignores the opposing Support (1518052 r0)", () => {
  // Coby Cr lv4 prints 8/3 and fights on 7, his `Support: Damage +1` over four Sentinel.
  // Shawnia lv4's `+3 Attack Per Opp. Damage` counts the 3: 7 x 4 + 3 x 3 = 37 (the engine had
  // 49). Coby Cr is 8 x 3 + 8 (Sentinel `Attack +8`) - 8 (Sakrohm `-8 Opp Attack, Min 3`).
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Anakrohm", "Baxter", "Shawnia", "Wakai"], [1, 3, 4, 2]),
    hand(["Coby Cr", "Earl", "Kamakura", "Owen"], [4, 1, 4, 5]),
    Turn.PLAYER_1,
  );
  g.select(2, 3, false, false); // P1 Shawnia lv4, 7/5
  g.select(0, 2, false, false); // P2 Coby Cr lv4, 8/3
  const [shawnia, coby] = [g.h1[2], g.h2[0]];
  assertEquals(stats(coby), [8, 7, 24]);
  assertEquals(stats(shawnia), [7, 5, 37]);
});

Deno.test("Per Opp. Damage ignores an opposing Copy (1519829 r1)", () => {
  // Baxter lv3 prints 6/3 and copies Cobretti's printed 7/7. Cobretti lv5's `+2 Attack Per Opp.
  // Damage` still counts Baxter's 3: 7 x 1 + 2 x 3 + 8 (Sentinel) - 8 (Sakrohm) = 13 (the
  // engine had 21).
  const g = new Game(
    new Player(15, 12, 0),
    new Player(15, 12, 1),
    hand(["Hilal", "Cobretti", "Katja", "Valentina Ld"], [3, 5, 3, 3]),
    hand(["Ackh", "Anakrohm", "Baxter", "Betwixt"], [5, 1, 3, 2]),
    Turn.PLAYER_1,
  );
  g.select(1, 0, false, false); // P1 Cobretti lv5, 7/7
  g.select(2, 0, false, false); // P2 Baxter lv3, 6/3
  const [cobretti, baxter] = [g.h1[1], g.h2[2]];
  assertEquals(stats(baxter), [7, 7, 7]);
  assertEquals(stats(cobretti), [7, 7, 13]);
});
