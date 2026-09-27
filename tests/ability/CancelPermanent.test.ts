// An opposing Cancel of a resource deactivates the permanents that write it for the round.
// The server prints it on Babe's "Cancel Opp. Pillz & Life Modif." (1655): "The effects of
// your opponent's poison, heal, regen, toxin, consume and dope abilities will be deactivated
// for the round."
//
// Captured battle 1508676 shows it on Combust, which the list leaves out: Kontrø Ld's
// "Combust 1, Min 0" latches in round zero and takes 1 Life and 1 Pillz at the end of round
// one, but not of round two, where Babe cancels, and again at the end of round three.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

/** 1508676's hands; `babe` replaces Babe's ability. */
const game = (babe?: string) => {
  const h1 = HandGenerator.handOf(
    ["Babe", "BroKen", "Khrull Cr", "Zigal"] as HandOf<string>,
    [4, 4, 4, 2] as HandOf<number | undefined>,
  );
  const h2 = HandGenerator.handOf(
    ["Kontrø Ld", "Maelt Riv", "Molch", "Rhody"] as HandOf<string>,
    [5, 3, 3, 3] as HandOf<number | undefined>,
  );
  if (babe !== undefined) h1[0] = h1[0].withAbility(babe);
  return new Game(new Player(15, 12, 0), new Player(15, 12, 1), h1, h2, Turn.PLAYER_1, false);
};

/** 1508676's rounds, with each round's [life, pillz] for both players. */
const play = (g: Game, rounds = 4) => {
  const after: number[][] = [];
  const note = () => after.push([g.p1.life, g.p1.pillz, g.p2.life, g.p2.pillz]);
  g.select(3, 5, false, false); // P1 Zigal
  g.select(0, 8, false, false); // P2 Kontrø Ld wins, latches Combust
  note();
  g.select(1, 1, false, false); // P2 Maelt Riv wins, latches Consume
  g.select(1, 1, false, false); // P1 BroKen
  note();
  g.select(0, 2, false, false); // P1 Babe cancels
  g.select(2, 3, false, false); // P2 Molch wins
  note();
  if (rounds === 3) return after;
  g.select(3, 0, false, false); // P2 Rhody
  g.select(2, 2, false, false); // P1 Khrull Cr wins
  note();
  return after;
};

Deno.test("A Pillz & Life cancel deactivates a latched Combust for its round (1508676)", () => {
  assertEquals(play(game()), [
    [10, 7, 15, 4],
    [7, 4, 15, 3],
    [4, 2, 15, 0],
    [3, 0, 10, 0],
  ]);
});

Deno.test("Without the cancel the latched Combust pays in round two", () => {
  // 7 - 3 - 1 = 3 Life and 4 - 2 - 1 = 1 Pillz (Consume holds at its Min 2 first), which
  // leaves too little for Khrull Cr's round-three bet.
  assertEquals(play(game("No Ability"), 3)[2], [3, 1, 15, 0]);
});
