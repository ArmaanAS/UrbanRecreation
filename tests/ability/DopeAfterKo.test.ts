// A latched Dope keeps paying its owner, up to its Max, in the round that knocks that owner
// out - unlike Heal and Regen, and unlike a one-off Defeat gain such as Kubra's. Both games
// are replayed move for move from their captured testcases (the first mover of round 0 is
// player 1; s1 is whoever moves first in that round).
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

type Move = [number, number, boolean];

const play = (life: number, cards: string[], levels: number[], night: boolean, moves: [Move, Move][]) => {
  const g = new Game(
    new Player(life, 12, 0),
    new Player(life, 12, 1),
    HandGenerator.handOf(cards.slice(0, 4) as HandOf<string>, levels.slice(0, 4) as HandOf<number | undefined>),
    HandGenerator.handOf(cards.slice(4) as HandOf<string>, levels.slice(4) as HandOf<number | undefined>),
    Turn.PLAYER_1,
    false,
    night,
  );
  for (const [s1, s2] of moves) {
    g.select(s1[0], s1[1], s1[2], false);
    g.select(s2[0], s2[1], s2[2], false);
  }
  return { p1life: g.p1.life, p2life: g.p2.life, p1pillz: g.p1.pillz, p2pillz: g.p2.pillz };
};

Deno.test("Defeat: Dope pays a knocked-out owner", () => {
  // Captured battle 956902 (a 15-Life room): Shao Xue's "Defeat: Dope 1, Max. 13" latches
  // on the round-0 loss. In round 2 its owner bets 4 of 10 and is knocked out, and the
  // server still posts a permanent +1: 10 - 4 + 1 = 7.
  assertEquals(
    play(
      15,
      ["Doshu Sensei", "HU0-M31", "Shao Xue", "Murphy", "Aegis Cr", "Hal Gladius", "Lumia Cr", "Mou"],
      [4, 4, 4, 4, 5, 2, 4, 3],
      false,
      [
        [[2, 0, false], [3, 0, false]],
        [[1, 3, false], [3, 4, false]],
        [[1, 4, false], [0, 9, false]],
      ],
    ),
    { p1life: 0, p2life: 15, p1pillz: 7, p2pillz: 0 },
  );
});

Deno.test("Dope pays a knocked-out owner", () => {
  // Captured battle 924853: Talhia's "Dope 3, Max. 4" latches on the round-0 win. In round
  // 3 its owner bets all 4, loses the 36-36 tie and goes to 0 Life; the server posts a
  // permanent +3: 0 + 3 = 3.
  assertEquals(
    play(
      12,
      ["Goran", "Mighty Kyrioz", "Omurtag", "Talhia", "Aegis Cr", "Hal Gladius", "Miyo", "Mou"],
      [3, 3, 3, 4, 5, 2, 3, 3],
      true,
      [
        [[3, 8, false], [3, 6, false]],
        [[1, 0, false], [2, 0, false]],
        [[1, 1, false], [0, 1, false]],
        [[2, 5, false], [0, 4, false]],
      ],
    ),
    { p1life: 0, p2life: 8, p1pillz: 3, p2pillz: 0 },
  );
});
