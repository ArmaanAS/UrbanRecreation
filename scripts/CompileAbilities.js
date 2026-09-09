import { Abilities } from "@/game/AbilityParser.ts";
import Ability, { AbilityType } from "@/game/Ability.ts";
import data from "@data/cards.json" with { type: "json" };

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
// for (const card of data) {
//   delete card.url;
//   delete card.clanPictUrl;
//   delete card.characterPictUrl;
//   delete card.characterNewPictUrl;
//   delete card.bonusLongDescription;
//   delete card.abilityLongDescription;
//   delete card.description;
// }
const cards = data.map(c => ({
  id: c.id,
  name: c.name,
  clan_id: c.clan_id,
  clan_name: c.clan_name,
  level: c.level,
  // xp_for_level: c.xp_for_level,
  // level_min: c.level_min,
  level_max: c.level_max,
  power: c.power,
  damage: c.damage,
  rarity: c.rarity,
  ability_id: c.ability_id,
  ability: c.ability,
  // ability_unlock_level: c.ability_unlock_level,
  bonus: c.bonus,
  // has_night_bonus: c.has_night_bonus,
  // bank_price: c.bank_price,
  // distrib: c.distrib,
  // kind: c.kind,
  // state: c.state,
  // offer_at_level: c.offer_at_level,
  release_date: c.release_date,
  // id_artist: c.id_artist,
  // efc_banned: c.efc_banned,
  // efc_max_evo_banned: c.efc_max_evo_banned,
  // efc_temp_banned: c.efc_temp_banned,
  // efc_bonus_low: c.efc_bonus_low,
  // efc_bonus_high: c.efc_bonus_high,
  // tourney_banned: c.tourney_banned,
  // tourney_max_evo_banned: c.tourney_max_evo_banned,
  // penalty: c.penalty,
  // altEvoPictureParam: c.altEvoPictureParam,
  // collector_date: c.collector_date,
  // is_ultra: c.is_ultra,
  // is_meteora: c.is_meteora,
  // is_noel: c.is_noel,
  // is_miss: c.is_miss,
  // id_faction: c.id_faction,
  // pictureURL: c.pictureURL,
  // HDPictureURL: c.HDPictureURL,
  // altPictureURL: c.altPictureURL,
  // ILEPictureURL: c.ILEPictureURL,
  // ILEHDPictureURL: c.ILEHDPictureURL,
  // serial: c.serial,
  // activeBoosterItems: c.activeBoosterItems,
  // min_price: c.min_price,
  // is_first_evo_released: c.is_first_evo_released,
  // characterHDPictUrl: c.characterHDPictUrl,
  // minMarketPrice: c.minMarketPrice,
  bonus_id: c.bonus_id,
}));

Deno.writeTextFileSync("./data/compiled.json", JSON.stringify(obj));
// data/data.json is now built from the site's card database by scripts/BuildCardData.ts
// (`deno task cards`), which has every level of every card. Keep the trimmed max-level
// list around for reference only.
Deno.writeTextFileSync("./data/data.maxlevel.json", JSON.stringify(cards));