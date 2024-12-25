import { Abilities } from "@/game/AbilityParser.ts";
import Ability, { AbilityType } from "@/game/Ability.ts";
import data from "@data/data.json" with { type: "json" };

const events = new Set();
Set.prototype.addGlobal = Set.prototype.add;

const obj = {};
const ab = {};
function compile(s, id, bonus = false, leader = false) {
  const a = new Ability(s, leader ? AbilityType.GLOBAL : bonus ? AbilityType.BONUS : AbilityType.ABILITY);
  // console.log(a.ability);

  if (a.ability === "Copy Bonus") {
    a.mods = [{
      eventTime: 0,
      copy: 4,
    }];
  } else
    if (a.ability === "Copy Ability") {
      a.mods = [{
        eventTime: 0,
        copy: 3,
      }];
    } else
      if (a.ability === "Infiltrated") {
        a.mods = [{
          eventTime: 0,
          copy: 5,
        }];
      } else {
        a.compile({ events });
      }

  if (a.conditions.length)
    console.log(a.conditions);
  if (a.mods.length)
    console.log(a.mods);

  // const conditions = Abilities.split(s);
  // const ability = conditions.pop();
  const ability = a.ability;
  // const conditions = a.conditions.map((cond) => {
  //   if (cond.stop) {
  //     return `${cond.s} ${cond.stop}`;
  //   } else return cond.s;
  // });
  const conditions = a.conditions.map(({ s }) => s);
  const modifiers = a.mods;

  for (const mod of modifiers) {
    if (mod.constructor.name === "BasicModifier") {
      mod.per = mod.per?.type;
      mod.type = mod.type.name;
      if (mod.min === -Infinity) mod.min = -1000;
      if (mod.max === Infinity) mod.max = 1000;

      if (conditions.includes("Backlash")) {
        mod.opp = false;
      }
    }
  }

  // const tokens = ability.split(" ");

  obj[id] = { ability, conditions, modifiers, ability_type: a.type, delayed: a.delayed, won: a.won, };
}

for (const card of data) {
  const ability = Abilities.normalise(card.ability);
  ab[ability] ??= card.ability_id;
  card.ability_id = ab[ability];

  if (!(card.ability_id in obj)) {
    compile(card.ability, card.ability_id, undefined, card.clan_name === "Leader");
  }
}

let clan = 1;
const bo = {};
for (const card of data) {
  const bonus = Abilities.normalise(card.bonus);
  if (!bo[bonus]) {
    console.log(card.clan_name, clan);
    compile(card.bonus, clan, true);
    bo[bonus] = clan++;
  }

  card.bonus_id = bo[bonus];
}

// Delete long strings and save data.json
for (const card of data) {
  delete card.url;
  delete card.clanPictUrl;
  delete card.characterPictUrl;
  delete card.characterNewPictUrl;
  delete card.bonusLongDescription;
  delete card.abilityLongDescription;
  delete card.description;
}

Deno.writeTextFileSync("./data/compiled.json", JSON.stringify(obj));
Deno.writeTextFileSync("./data/data.json", JSON.stringify(data));