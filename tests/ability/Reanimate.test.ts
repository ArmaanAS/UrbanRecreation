// Reanimate is an immediate Defeat life gain and is explicitly allowed to prevent a KO.
// Battle 1130654 r1 is the non-lethal ground truth: Lobo loses on 7 Life, takes Miyo's
// 5 Damage, then gains 2 Life, leaving 4 rather than 2.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game, { Winner } from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

const game = (theirLife: number) =>
  new Game(
    new Player(14, 4, 0),
    new Player(theirLife, 6, 1),
    HandGenerator.handOf(
      ["Lumia Cr", "Miyo", "Mou", "Nebula"] as HandOf<string>,
      [4, 3, 3, 3] as HandOf<number | undefined>,
    ),
    HandGenerator.handOf(
      ["Bernardite", "Dashiell", "Donald", "Lobo"] as HandOf<string>,
      [3, 4, 3, 3] as HandOf<number | undefined>,
    ),
    Turn.PLAYER_1,
  );

const playMiyoIntoLobo = (g: Game) => {
  g.select(1, 1, false, false); // Miyo: 12 Attack, 5 Damage
  g.select(3, 0, false, false); // Lobo: 5 Attack, Reanimate +2 Life
};

Deno.test("Reanimate gains life after a non-lethal defeat", () => {
  const g = game(7);
  playMiyoIntoLobo(g);

  assertEquals(g.h2[3].won, false);
  assertEquals(g.p2.life, 4); // 7 - 5 Damage + 2 Reanimate
});

Deno.test("Reanimate can prevent a lethal damage KO", () => {
  const g = game(4);
  playMiyoIntoLobo(g);

  assertEquals(g.p2.life, 2); // max(0, 4 - 5 Damage) + 2 Reanimate
  assertEquals(g.winner, Winner.PLAYING);
});

Deno.test("a stopped Reanimate does not gain life", () => {
  const g = game(14);

  g.select(0, 4, false, false); // Lumia Cr: Stop Opp. Ability
  g.select(3, 0, false, false); // Lobo's Reanimate is blocked

  assertEquals(g.h2[3].won, false);
  assertEquals(g.p2.life, 14 - g.h1[0].damage.final);
});
