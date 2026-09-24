// Naja Ld prints "Victory Or Defeat : +3 Players Pillz" (captures/abilities.json 5511): both
// players get 3 Pillz after the bet, whatever the outcome - and the server pays her owner
// even in the round that knocks that owner out. Captured battle 1024592 round 2: Aneta
// (bet 3) beats Naja Ld (bet 0) for 6 against 6 Life; the server posts +3 Pillz for both
// players and ends Naja's owner on 5 - 0 + 3 = 8. 1024732 r3 pays a knocked-out owner 0 -> 3.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

Deno.test("Naja Ld's Players Pillz pays a knocked-out owner", () => {
  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);
  p1.life = 7;
  p1.pillz = 9;
  p2.life = 6;
  p2.pillz = 5;
  const g = new Game(
    p1,
    p2,
    HandGenerator.handOf(["Aneta", "Jairin", "Nantosuelte", "Ramak"] as HandOf<string>, [2, 4, 3, 4] as HandOf<number | undefined>),
    HandGenerator.handOf(["Arnie", "Naja Ld", "Spade", "McLayton"] as HandOf<string>, [4, 1, 2, 4] as HandOf<number | undefined>),
    Turn.PLAYER_1,
    false,
    true,
  );

  g.select(0, 3, false, false); // Aneta
  g.select(1, 0, false, false); // Naja Ld

  assertEquals(g.h1[0].damage.final, 6);
  assertEquals(g.h1[0].won, true);
  assertEquals(
    { p1life: g.p1.life, p2life: g.p2.life, p1pillz: g.p1.pillz, p2pillz: g.p2.pillz },
    { p1life: 7, p2life: 0, p1pillz: 9, p2pillz: 8 },
  );
});
