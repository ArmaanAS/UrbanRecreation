import cluster from "node:cluster";
import { BaseCard, BaseData, CardJSON, Clan } from "./types/CardTypes.ts";

const DATA_PATH = "./data/data.json";

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

const cardIds: { [index: number]: CardJSON } = {};
const cardNames: { [index: string]: CardJSON } = {};
const cardYears: { [index: number]: CardJSON[] } = {};
const cardClans = {} as { [Property in Clan]: CardJSON[] };
const baseCards: { [key: string]: BaseCard } = {};

export { baseCards, cardClans, cardIds, cardNames, cardYears };

if (cluster.isPrimary) {
  const data = await Deno.readTextFileSync(DATA_PATH);

  const json: CardJSON[] = JSON.parse(data);
  console.log(json.length.toString().green + " cards loaded!".white);

  for (const j of json) {
    cardIds[j.id] = j;
    cardNames[j.name.toLowerCase()] = j;

    const year = new Date(j.release_date * 1000).getFullYear();
    // if (cardYears[year] === undefined)
    //   cardYears[year] = [];
    cardYears[year] ??= [];
    cardYears[year].push(j);

    const clan = j.clan_name;
    // if (cardClans[clan] === undefined)
    //   cardClans[clan] = [];
    cardClans[clan] ??= [];
    cardClans[clan].push(j);

    registerCardJSON(j);
  }
}

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
