import data from "../../data/data.json";
import { wordTree } from "../../build/ts/utils/Utils.js";
import { Abilities } from "../../build/ts/game/AbilityParser.js";
import { writeFileSync } from "fs";

const abilities = data.map((card) => {
  const split = Abilities.split(card.ability);
  return split.pop().replace(/\d+/g, "x");
});
const bonuses = data.map((card) => {
  const split = Abilities.split(card.bonus);
  return split.pop().replace(/\d+/g, "x");
});

const tree = wordTree([...abilities, ...bonuses]);

writeFileSync("data/abilityTree.json", JSON.stringify(tree));
