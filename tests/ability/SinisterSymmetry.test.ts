// Sinister Symmetry — captures/abilities.json 4303 (Karkass Cr): if the card wins its round
// against the card *opposite* it (indexRequirement "symmetry"), the opponent's Life drops
// to 0 and the match ends by KO (specialAction "ko"). Captured battle 874399 r3: Karkass Cr
// at index 3 beats Tina at index 3, dealing 4 damage to an opponent on 9 Life, and the
// server then reports a post-round life decrease of exactly the remaining 5.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

// Karkass Cr lv3 (Nightmare, power 9 damage 4) sits at index 3 of player 1's hand.
const game = () =>
  new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(["Gravelsnout", "Nidory", "Gorgon", "Karkass Cr"] as HandOf<string>, [2, 3, 1, 3] as HandOf<number | undefined>),
    HandGenerator.handOf(["Anita", "Aurora", "Sue", "Tina"] as HandOf<string>, [3, 5, 2, 3] as HandOf<number | undefined>),
    Turn.PLAYER_1,
  );

Deno.test("Sinister Symmetry KOs the opponent facing it", () => {
  const g = game();

  g.select(3, 8, false, false); // p1 Karkass Cr (index 3), 8 pillz
  g.select(3, 0, false, false); // p2 Tina (index 3) — same index, so the effect activates

  assertEquals(g.h1[3].won, true);
  assertEquals(g.p2.life, 0); // not 12 - 4 damage: all of it
});

Deno.test("Sinister Symmetry does nothing against another index", () => {
  const g = game();

  g.select(3, 8, false, false); // p1 Karkass Cr (index 3)
  g.select(0, 0, false, false); // p2 Anita (index 0) — no symmetry

  assertEquals(g.h1[3].won, true);
  assertEquals(g.p2.life, 8); // just Karkass Cr's 4 damage
});

Deno.test("Sinister Symmetry does nothing when the card loses", () => {
  const g = game();

  g.select(3, 0, false, false); // p1 Karkass Cr, no pillz — loses
  g.select(3, 8, false, false); // p2 Tina (index 3), 8 pillz

  assertEquals(g.h1[3].won, false);
  assertEquals(g.p2.life, 12);
});
