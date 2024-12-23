import data from "@data/data.json" with { type: "json" };
import { wordTree } from "@/utils/Utils.ts";
import { Abilities } from "@/game/AbilityParser.ts";

const abilities = data.map((card) => {
  const split = Abilities.split(card.ability);
  return split.pop()!.replace(/\d+/g, "x");
});
const bonuses = data.map((card) => {
  const split = Abilities.split(card.bonus);
  return split.pop()!.replace(/\d+/g, "x");
});

const tree = wordTree([...abilities, ...bonuses]);

Deno.writeTextFileSync("data/abilityTree.json", JSON.stringify(tree));
