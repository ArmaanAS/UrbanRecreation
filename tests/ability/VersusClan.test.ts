import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";

Deno.test("Versus Riots", () => {
  const h1 = HandGenerator.generate(
    "Ashara",
    "Ali",
    "Hikiyousan",
    "Zatapa",
  );
  const h2 = HandGenerator.generate(
    "Betelgeuse",
    "Molder",
    "Incubus",
    "Tamakuchi",
  );
  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);

  const g = new Game(p1, p2, h1, h2, Turn.PLAYER_1);

  assertEquals(g.p1.life, 12);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);

  g.select(0, 1); // Ashara
  g.select(3, 0); // Tamakuchi

  assertEquals(g.p1.life, 14);
  assertEquals(g.p2.life, 8);
  assertEquals(g.p1.pillz, 11);
  assertEquals(g.p2.pillz, 12);
});

Deno.test("Versus Unmet", () => {
  const h1 = HandGenerator.generate(
    "Ashara",
    "Ali",
    "Hikiyousan",
    "Zatapa",
  );
  const h2 = HandGenerator.generate(
    "Betelgeuse",
    "Candy Jack",
    "Incubus",
    "Tamakuchi",
  );
  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);

  const g = new Game(p1, p2, h1, h2, Turn.PLAYER_1);

  assertEquals(g.p1.life, 12);
  assertEquals(g.p2.life, 12);
  assertEquals(g.p1.pillz, 12);
  assertEquals(g.p2.pillz, 12);

  g.select(0, 1); // Ashara
  g.select(3, 0); // Tamakuchi

  assertEquals(g.p1.life, 12);
  assertEquals(g.p2.life, 8);
  assertEquals(g.p1.pillz, 11);
  assertEquals(g.p2.pillz, 12);
});

// Versus reads the opposing hand's printed clans, so an infiltrated Oculus still counts as an
// Oculus. 1517121 r1: Predtr Ld's `Versus [clan:51][clan:56] : Stop Opp. Ability` ("only
// activates if there's a Hive or Oculus in your opponent's hand") faces a hand whose Dark
// Sentogan fights as a Montana; the Stop still lands, so Nobutomo's `Equalizer: -1 Opp
// Damage, Min 3` never cuts Predtr Ld's 4 Damage.
Deno.test("Versus counts an infiltrated Oculus as an Oculus (1517121 r1)", () => {
  const play = (second: string, level: number) => {
    const g = new Game(
      new Player(15, 12, 0),
      new Player(15, 12, 1),
      HandGenerator.handOf(
        ["Cristalys", "Izsobahd", "Predtr Ld", "Sarah"] as HandOf<string>,
        [5, 3, 3, 3] as HandOf<number | undefined>,
      ),
      HandGenerator.handOf(
        ["Avola", second, "Judge Scare", "Nobutomo"] as HandOf<string>,
        [5, level, 5, 2] as HandOf<number | undefined>,
      ),
      Turn.PLAYER_1,
    );
    g.select(2, 0, false, false); // P1 Predtr Ld lv3 (9/4)
    g.select(3, 0, false, false); // P2 Nobutomo lv2
    return g;
  };
  const oculus = play("Dark Sentogan", 2);
  assertEquals([oculus.h2[1].baseClan, oculus.h2[1].clan], ["Oculus", "Montana"]);
  assertEquals(oculus.h1[2].damage.final, 4);
  // A Rescue card in the same slot: no Hive or Oculus, so the Equalizer cuts 4 - 3 to Min 3.
  assertEquals(play("Lea", 1).h1[2].damage.final, 3);
});
