import { BaseCard, BaseData, CardJSON, Clan } from "./types/CardTypes.ts";
import _json from "@data/data.json" with { type: "json" };

export function getBaseKey(id: number, stars: number) {
  if (id < 0 || id > 0xffff) {
    throw new RangeError(`id must be in range 0-${0xffff}: ${id}`);
  }
  if (stars < 1 || stars > 5) {
    throw new RangeError(`stars must be in range 1-5: ${stars}`);
  }

  return (id & 0xffff) | (stars << 16);
}

export function registerCardJSON(j: CardJSON) {
  const data = new BaseData();
  data.power.base = j.power;
  data.power.final = j.power;
  data.damage.base = j.damage;
  data.damage.final = j.damage;

  const key = getBaseKey(j.id, j.level);
  baseCards[key] = {
    name: j.name,
    id: j.id,
    stars: j.level,
    maxStars: j.level_max,
    release_date: j.release_date * 1000,
    clan: j.clan_name,
    rarity: j.rarity,

    ability: j.ability,
    bonus: j.bonus,
    nightAbility: j.night_ability,
    nightBonus: j.night_bonus,
    // ability: { string: j.ability, prot: false, cancel: false },
    // bonus: { string: j.bonus, prot: false, cancel: false },

    // power: {
    //   base: j.power, final: j.power, prot: false, cancel: false
    // },
    // damage: {
    //   base: j.damage, final: j.damage, prot: false, cancel: false
    // },
    // attack: { base: 0, final: 0, prot: false, cancel: false },

    // life: { prot: false, cancel: false },
    // pillz: { prot: false, cancel: false },
    data: data,
  };
}

/** Max-level entry per card id / lower-cased name (the default when no level is given). */
export const cardIds: Record<number, CardJSON> = {};
export const cardNames: Record<string, CardJSON> = {};
/** Every level of every card: cardLevels[id][level]. */
export const cardLevels: Record<number, Partial<Record<number, CardJSON>>> = {};
/** Max-level entries grouped by release year / clan (used for random hands). */
export const cardYears: Record<number, CardJSON[]> = {};
export const cardClans = {} as Record<Clan, CardJSON[]>;
export const baseCards: Record<string, BaseCard> = {};

const json = _json as CardJSON[];

let maxLevelCards = 0;
for (const j of json) {
  (cardLevels[j.id] ??= {})[j.level] = j;
  registerCardJSON(j);

  if (j.level !== j.level_max) continue;
  maxLevelCards++;

  cardIds[j.id] = j;
  cardNames[j.name.toLowerCase()] = j;
  // Cards get a " Cr" suffix when they become collectors; keep the old name as an alias.
  const plain = j.name.toLowerCase().replace(/ cr$/, "");
  if (plain !== j.name.toLowerCase()) cardNames[plain] ??= j;

  const year = new Date(j.release_date * 1000).getFullYear();
  cardYears[year] ??= [];
  cardYears[year].push(j);

  const clan = j.clan_name;
  cardClans[clan] ??= [];
  cardClans[clan].push(j);
}
console.log(
  maxLevelCards.toString().green + " cards loaded!".white +
    (json.length > maxLevelCards ? ` (${json.length} card levels)`.gray : ""),
);

// let abilities = json.map(i => new Ability(i.ability));
// abilities = [...abilities, ...json.map(i => new Ability(i.bonus))]

// let tree = wordTree(abilities.map(i => i.ability.replace(/\d+/g, 'x')));
// console.log(JSON.stringify(tree));

// let conditions = {};
// for (let a of abilities) {
//   for (let c of a.conditions) {
//     conditions[c] = 0;
//   }
// }
// console.log(Object.keys(conditions));
