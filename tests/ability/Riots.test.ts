// Riots' Victory Or Defeat bonus gives its Pillz even after the owner is KO'd.
// Captured battle 1058366 r2 has Kenjy Noel lose to Anagone, reach zero Life,
// and still go from 8 to 9 Pillz.
import { HandGenerator } from "@/game/Hand.ts";
import Ability, { AbilityType } from "@/game/Ability.ts";
import BattleData from "@/game/battle/BattleData.ts";
import BasicModifier from "@/game/modifiers/BasicModifier.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type Clan, type HandOf } from "@/game/types/CardTypes.ts";

const postKoPillzFlag = (clan: Clan, bonusString: string) => {
  const ability = new Ability(bonusString, AbilityType.BONUS);
  ability.compile({
    card: { clan, bonusString },
    events: { add() {} },
  } as unknown as BattleData);
  return (ability.mods[0] as BasicModifier).postKoPillz;
};

Deno.test("Riots gets its Victory Or Defeat Pillz after a KO", () => {
  const game = new Game(
    new Player(8, 8, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(
      ["Kenjy Noel", "Astromos Cr", "Archimedes", "Argos"] as HandOf<string>,
      [2, 3, 2, 1] as HandOf<number | undefined>,
    ),
    HandGenerator.handOf(
      ["Aneta", "Anagone", "Leonaparte", "Nantosuelte"] as HandOf<string>,
      [2, 5, 3, 3] as HandOf<number | undefined>,
    ),
    Turn.PLAYER_1,
    false,
    true,
  );

  game.select(0, 0, false, false); // Kenjy Noel: 6 Attack
  game.select(1, 9, false, false); // Anagone: 70 Attack, 10 Damage

  assertEquals(game.h1[0].won, false);
  assertEquals(game.p1.life, 0);
  assertEquals(game.p1.pillz, 9); // 8 - 0 + Riots 1
});

Deno.test("the post-KO Pillz exception is exact to the Riots bonus", () => {
  assertEquals(
    postKoPillzFlag("Riots", "Victory Or Defeat : +1 Pillz"),
    true,
  );
  assertEquals(
    postKoPillzFlag("Junkz", "Victory Or Defeat : +1 Pillz"),
    false,
  );
  assertEquals(
    postKoPillzFlag("Riots", "Victory Or Defeat : +2 Pillz"),
    false,
  );
});
