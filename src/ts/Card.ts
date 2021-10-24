import fs from "fs"
import { getN } from "./utils/Utils"
import {
  CardJSON, Clan, Clans, HandOf, MaxStars, Rarity, Stars
} from "./types/Types"
import "colors"
import {
  isMainThread
} from 'worker_threads'


let json: CardJSON[],
  cardIds = {} as { [index: number]: CardJSON },
  cardNames = {} as { [index: string]: CardJSON };
const cardYears = {} as { [index: number]: CardJSON[] },
  cardClans = {} as { [Property in Clan]: CardJSON[] };


// let cbf = () => { };
// fs.readFile(__dirname + "/data.json", (err, data) => {
//   if (err) {
//     throw err;
//   }

//   json = JSON.parse(data.toString());
//   console.log(
//     json.length.toString().brightGreen + " cards loaded!".brightWhite
//   );

//   cardIds = Object.fromEntries(json.map(i => [i.id, i]));
//   cardNames = Object.fromEntries(json.map(i => [i.name.toLowerCase(), i]));
//   cardYears = {};
//   cardClans = {};
//   json.forEach(i => {
//     let year = new Date(i.release_date * 1000).getFullYear();
//     if (!cardYears[year]) {
//       cardYears[year] = [];
//     }

//     cardYears[year].push(i);

//     let clan = i.clan_name;
//     if (!cardClans[clan]) {
//       cardClans[clan] = [];
//     }

//     cardClans[clan].push(i);
//   });

//   // let abilities = json.map(i => new Ability(i.ability));
//   // abilities = [...abilities, ...json.map(i => new Ability(i.bonus))]

//   // let tree = wordTree(abilities.map(i => i.ability.replace(/\d+/g, 'x')));
//   // console.log(JSON.stringify(tree));

//   // let conditions = {};
//   // for (let a of abilities) {
//   //   for (let c of a.conditions) {
//   //     conditions[c] = 0;
//   //   }
//   // }
//   // console.log(Object.keys(conditions));

//   Card.loaded = true;
//   console.log('Calling callback function: ');
//   console.log(cbf.toString());
//   // Card.cb();
//   cbf();
//   // loaded();
// });

interface CardAttr {
  cancel: boolean;
  prot: boolean;
}
// interface CardStat extends CardAttr {
//   base: number;
//   final: number;
// }
interface CardString extends CardAttr {
  string: string;
}
interface BaseCard {
  name: string;
  id: number;
  stars: Stars;
  maxStars: MaxStars;
  release_date: number;
  clan: Clan;
  rarity: Rarity;
  // power: CardStat;  // 18 bits
  // damage: CardStat; // 18 bits
  ability: CardString;
  bonus: CardString;
  // attack: CardStat; // 18 bits
  // life: CardAttr; // 2 bits
  // pillz: CardAttr; // 2 bits
  data: BaseData;
}  // 58 bits + 3 bit index = 61 bits = 2 numbers

const baseCards: { [key: string]: BaseCard } = {};


// a = power.base,.final, damage.base,.final
// b = attack.base,.final, power.cancel,.prot  damage.cancel,.prot, 
//     attack.cancel,.prot  pillz.cancel,.prot  life.cancel,.prot
//     index  won  played
// a = 00000000 00000000 00000000 00000000     = 32 bits
// b = 00000000 00000000 00 00 00 00 00 000 00 0 = 32 bits
class BaseData {
  a: number;
  b: number;
}
abstract class BaseAttr extends BaseData {
  abstract get cancel(): boolean;
  abstract set cancel(n: boolean);
  abstract get prot(): boolean;
  abstract set prot(n: boolean);
  abstract get blocked(): boolean;
}
abstract class BaseStat extends BaseAttr {
  abstract get base(): number;
  abstract set base(n: number);
  abstract get final(): number;
  abstract set final(n: number);
}
class PowerStat extends BaseStat {
  get base(): number {
    return this.a & 0xff;
  }
  set base(n: number) {
    this.a = (this.a & ~0xff) | (n & 0xff);
  }
  get final(): number {
    return this.a >> 8 & 0xff;
  }
  set final(n: number) {
    this.a = (this.a & ~0xff00) | ((n & 0xff) << 8);
  }
  get cancel(): boolean {
    return !!(this.b >> 16 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x10000) | (+n << 16)
  }
  get prot(): boolean {
    return !!(this.b >> 17 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x20000) | (+n << 17)
  }
  get blocked() {
    // prot || !cancel
    // 10, 11, 00 = true
    // 01 = false
    return (this.b >> 16 & 0b11) !== 0b01;
  }
}
class DamageStat extends BaseStat {
  get base(): number {
    return this.a >> 16 & 0xff;
  }
  set base(n: number) {
    this.a = (this.a & ~0xff0000) | ((n & 0xff) << 16);
  }
  get final(): number {
    return this.a >> 24 & 0xff;
  }
  set final(n: number) {
    this.a = (this.a & ~0xff000000) | ((n & 0xff) << 24);
  }
  get cancel(): boolean {
    return !!(this.b >> 18 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x40000) | (+n << 18)
  }
  get prot(): boolean {
    return !!(this.b >> 19 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x80000) | (+n << 19)
  }
  get blocked() {
    return (this.b >> 18 & 0b11) !== 0b01;
  }
}
class AttackStat extends BaseStat {
  get base(): number {
    return this.b & 0xff;
  }
  set base(n: number) {
    this.b = (this.b & ~0xff) | (n & 0xff);
  }
  get final(): number {
    return this.b >> 8 & 0xff;
  }
  set final(n: number) {
    this.b = (this.b & ~0xff00) | ((n & 0xff) << 8);
  }
  get cancel(): boolean {
    return !!(this.b >> 20 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x100000) | (+n << 20)
  }
  get prot(): boolean {
    return !!(this.b >> 21 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x200000) | (+n << 21)
  }
  get blocked() {
    return (this.b >> 20 & 0b11) !== 0b01;
  }
}
class PillzStat extends BaseAttr {
  get cancel(): boolean {
    return !!(this.b >> 22 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x400000) | (+n << 22)
  }
  get prot(): boolean {
    return !!(this.b >> 23 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x800000) | (+n << 23)
  }
  get blocked() {
    return (this.b >> 22 & 0b11) !== 0b01;
  }
}
class LifeStat extends BaseAttr {
  get cancel(): boolean {
    return !!(this.b >> 24 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x1000000) | (+n << 24)
  }
  get prot(): boolean {
    return !!(this.b >> 25 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x2000000) | (+n << 25)
  }
  get blocked() {
    return (this.b >> 24 & 0b11) !== 0b01;
  }
}

export default class Card {
  private key = -1;
  // index = -1;
  // won?: boolean = undefined;
  // played = false;
  private _ability?: CardString = undefined;
  private _bonus?: CardString = undefined;
  private data: BaseData;
  constructor(json: CardJSON) {
    // this.key = json.id + '_' + json.level;
    this.key = (json.id & 0xffff) | (json.level << 16)

    this.data = { ...this.base.data }
  }

  clone(): Card {
    return Object.setPrototypeOf({
      key: this.key,
      // index: this.index,
      // won: this.won,
      // played: this.played,

      _ability: this._ability && { ...this._ability },
      _bonus: this._bonus && { ...this._bonus },

      data: { ...this.data }

      // _damage: this._damage && { ...this._damage },
      // _power: this._power && { ...this._power },
      // _attack: this._attack && { ...this._attack },

      // _life: this._life && { ...this._life },
      // _pillz: this._pillz && { ...this._pillz },
    }, Card.prototype);
  }

  static from(o: Card): Card {
    return Object.setPrototypeOf(o, Card.prototype);
  }

  get base() {
    return baseCards[this.key];
  }

  get year() {
    return new Date(this.base.release_date).getFullYear().toString();
  }

  get clan() {
    return this.base.clan;
  }

  get stars() {
    return this.base.stars;
  }

  get maxStars() {
    return this.base.maxStars;
  }

  get name() {
    return this.base.name;
  }

  get id() {
    return this.base.id;
  }

  get rarity() {
    return this.base.rarity;
  }

  get ability() {
    return this._ability ?? this.base.ability;
  }

  get bonus() {
    return this._bonus ?? this.base.bonus;
  }

  get power(): PowerStat {
    return Object.setPrototypeOf(this.data, PowerStat.prototype);
  }

  get damage(): DamageStat {
    return Object.setPrototypeOf(this.data, DamageStat.prototype);
  }

  get attack(): AttackStat {
    return Object.setPrototypeOf(this.data, AttackStat.prototype);
  }

  get life(): LifeStat {
    return Object.setPrototypeOf(this.data, LifeStat.prototype);
  }

  get pillz(): PillzStat {
    return Object.setPrototypeOf(this.data, PillzStat.prototype);
  }


  get ability_() {
    return this._ability ?? (this._ability = { ...this.base.ability });
  }

  get bonus_() {
    return this._bonus ?? (this._bonus = { ...this.base.bonus });
  }

  get power_(): PowerStat {
    return Object.setPrototypeOf(this.data, PowerStat.prototype);
  }

  get damage_(): DamageStat {
    return Object.setPrototypeOf(this.data, DamageStat.prototype);
  }

  get attack_(): AttackStat {
    return Object.setPrototypeOf(this.data, AttackStat.prototype);
  }

  get life_(): LifeStat {
    return Object.setPrototypeOf(this.data, LifeStat.prototype);
  }

  get pillz_(): PillzStat {
    return Object.setPrototypeOf(this.data, PillzStat.prototype);
  }

  get index() {
    return this.data.b >> 26 & 0b111;
  }
  set index(n: number) {
    this.data.b = (this.data.b & ~(0b111 << 26)) | ((n & 0b111) << 26)
  }

  get won() {
    // 0 undefined, 0b10 false, 0b11 true
    const a = this.data.b >> 29 & 0b11;
    return a === 0 ? undefined : a === 3;
  }
  set won(n: boolean | undefined) {
    if (n === undefined)
      this.data.b &= ~(0b11 << 29);
    else
      this.data.b = (this.data.b & ~(0b11 << 29)) | (((+n << 1) | 0b1) << 29);
  }

  get played() {
    return !!(this.data.b >> 31 & 1);
  }
  set played(n: boolean) {
    this.data.b = (this.data.b & ~(1 << 31)) | (+n << 31);
  }
}
// interface CardAttr {
//   cancel: boolean;
//   prot: boolean;
// }
// interface CardStat extends CardAttr {
//   base: number;
//   final: number;
// }
// interface CardString extends CardAttr {
//   string: string;
// }
// export default class Card {
//   key = '';
//   index = -1;
//   won?: boolean = undefined;
//   played = false;
//   private _power?: CardStat = undefined;
//   private _damage?: CardStat = undefined;
//   private _ability?: CardString = undefined;
//   private _bonus?: CardString = undefined;
//   private _attack?: CardStat = undefined;
//   private _life?: CardAttr = undefined;
//   private _pillz?: CardAttr = undefined;
//   constructor(json: CardJSON) {
//     this.key = json.id + '_' + json.level;
//   }

//   clone(): Card {
//     return Object.setPrototypeOf({
//       key: this.key,
//       index: this.index,
//       won: this.won,
//       played: this.played,

//       _damage: this._damage && { ...this._damage },
//       _power: this._power && { ...this._power },
//       _attack: this._attack && { ...this._attack },

//       _ability: this._ability && { ...this._ability },
//       _bonus: this._bonus && { ...this._bonus },

//       _life: this._life && { ...this._life },
//       _pillz: this._pillz && { ...this._pillz },
//     }, Card.prototype);
//   }

//   static from(o: Card): Card {
//     return Object.setPrototypeOf(o, Card.prototype);
//   }

//   get base() {
//     return baseCards[this.key];
//   }

//   get year() {
//     return new Date(this.base.release_date).getFullYear().toString();
//   }

//   get clan() {
//     return this.base.clan;
//   }

//   get stars() {
//     return this.base.stars;
//   }

//   get maxStars() {
//     return this.base.maxStars;
//   }

//   get name() {
//     return this.base.name;
//   }

//   get id() {
//     return this.base.id;
//   }

//   get rarity() {
//     return this.base.rarity;
//   }

//   get ability() {
//     return this._ability ?? this.base.ability;
//   }

//   get bonus() {
//     return this._bonus ?? this.base.bonus;
//   }

//   get power() {
//     return this._power ?? this.base.power;
//   }

//   get damage() {
//     return this._damage ?? this.base.damage;
//   }

//   get attack() {
//     return this._attack ?? this.base.attack;
//   }

//   get life() {
//     return this._life ?? this.base.life;
//   }

//   get pillz() {
//     return this._pillz ?? this.base.pillz;
//   }


//   get ability_() {
//     return this._ability ?? (this._ability = { ...this.base.ability });
//   }

//   get bonus_() {
//     return this._bonus ?? (this._bonus = { ...this.base.bonus });
//   }

//   get power_() {
//     return this._power ?? (this._power = { ...this.base.power });
//   }

//   get damage_() {
//     return this._damage ?? (this._damage = { ...this.base.damage });
//   }

//   get attack_() {
//     return this._attack ?? (this._attack = { ...this.base.attack });
//   }

//   get life_() {
//     return this._life ?? (this._life = { ...this.base.life });
//   }

//   get pillz_() {
//     return this._pillz ?? (this._pillz = { ...this.base.pillz });
//   }
// }

export class CardGenerator {
  static get(card: number | string) {
    let data: CardJSON | undefined;
    if (typeof card == 'number')
      data = cardIds[card];
    else
      data = cardNames[card];

    // if (data == undefined)
    //   return undefined;
    // else
    //   return new Card(data);
    return data && new Card(data);
  }

  static getRandomHandYear(year = 2006) {
    const card = getN(cardYears[year])[0];
    const cards = cardYears[year].filter((j) => j.clan_name == card.clan_name);

    return getN(cards, 4).map((c) => new Card(c)) as HandOf<Card>;
  }

  static getRandomHandClan(clan?: Clan) {
    let cards: CardJSON[];
    if (clan !== undefined)
      cards = cardClans[clan];
    else
      // cards = cardClans[getN(Object.keys(cardClans) as Clan[])[0]];
      cards = cardClans[getN(Clans as unknown as Clan[])[0]];

    return getN(cards, 4).map((c) => new Card(c));
  }

  static getRandomHand(cards: HandOf<string | number>) {
    const cardsArr = cards.map(c => {
      if (typeof c == "string")
        return new Card(cardNames[c.toLowerCase()]);
      else
        return new Card(cardIds[c]);
    }) as HandOf<Card>;

    cardsArr.push(
      ...getN(Object.values(cardIds), 4 - cardsArr.length)
        .map(c => new Card(c))
    );

    return cardsArr;
  }
}

// class CardClassic {
//   index: number = -1;
//   won?: boolean = undefined;
//   played: boolean = false;
//   name: string;
//   id: number;
//   stars: Stars;
//   maxStars: MaxStars;
//   release_date: number;
//   clan: Clan;
//   rarity: Rarity;
//   power: CardStat;
//   damage: CardStat;
//   ability: CardString;
//   bonus: CardString;
//   attack: CardStat;
//   life: CardAttr;
//   pillz: CardAttr;
//   constructor(json: CardJSON) {
//     // this.index = -1;
//     // this.won = undefined;
//     // this.played = false;

//     this.name = json.name;
//     this.id = json.id;
//     this.stars = json.level;
//     this.maxStars = json.level_max;
//     this.release_date = json.release_date * 1000;
//     this.clan = json.clan_name;
//     this.rarity = json.rarity;

//     // this.power = new CardStat(json.power);
//     // this.damage = new CardStat(json.damage);
//     // this.ability = new CardString(json.ability);
//     // this.bonus = new CardString(json.bonus);

//     // this.attack = new CardStat(0);

//     // this.life = new CardAttr();
//     // this.pillz = new CardAttr();

//     this.power = {
//       base: json.power, final: json.power, prot: false, cancel: false
//     };
//     this.damage = {
//       base: json.damage, final: json.damage, prot: false, cancel: false
//     };
//     this.ability = { string: json.ability, prot: false, cancel: false };
//     this.bonus = { string: json.bonus, prot: false, cancel: false };

//     this.attack = { base: 0, final: 0, prot: false, cancel: false };

//     this.life = { prot: false, cancel: false };
//     this.pillz = { prot: false, cancel: false };
//   }

//   clone(): Card {
//     return Object.setPrototypeOf({
//       index: this.index,
//       won: this.won,
//       played: this.played,

//       name: this.name,
//       id: this.id,
//       stars: this.stars,
//       maxStars: this.maxStars,
//       release_date: this.release_date,
//       clan: this.clan,
//       rarity: this.rarity,

//       // power: this.power.clone(),
//       // damage: this.damage.clone(),
//       // ability: this.ability.clone(),
//       // bonus: this.bonus.clone(),

//       // attack: this.attack.clone(),

//       // life: this.life.clone(),
//       // pillz: this.pillz.clone(),
//       // power: this.power.clone(),

//       // damage: clone(this.damage),
//       // ability: clone(this.ability),
//       // bonus: clone(this.bonus),

//       // attack: clone(this.attack),

//       // life: clone(this.life),
//       // pillz: clone(this.pillz),
//       // power: clone(this.power),
//       damage: { ...this.damage },
//       ability: { ...this.ability },
//       bonus: { ...this.bonus },

//       attack: { ...this.attack },

//       life: { ...this.life },
//       pillz: { ...this.pillz },
//       power: { ...this.power },
//     }, Card.prototype);
//   }

//   static from(o: Card) {
//     return Object.setPrototypeOf(o, Card.prototype);

//     // Object.setPrototypeOf(o.power, CardStat.prototype);
//     // Object.setPrototypeOf(o.damage, CardStat.prototype);
//     // Object.setPrototypeOf(o.ability, CardString.prototype);
//     // Object.setPrototypeOf(o.bonus, CardString.prototype);

//     // Object.setPrototypeOf(o.attack, CardStat.prototype);

//     // Object.setPrototypeOf(o.life, CardAttr.prototype);
//     // Object.setPrototypeOf(o.pillz, CardAttr.prototype);

//     // return o;
//   }

//   get year() {
//     return new Date(this.release_date).getFullYear().toString();
//   }



//   static get(card: number | string) {
//     let data: CardJSON | undefined;
//     if (typeof card == 'number')
//       data = cardIds[card];
//     else
//       data = cardNames[card];

//     if (data == undefined) return undefined;
//     else return new Card(data);
//   }

//   static getRandomHandYear(year = 2006) {
//     let card = getN(cardYears[year])[0];
//     let cards = cardYears[year].filter((j) => j.clan_name == card.clan_name);

//     return getN(cards, 4).map((c) => new Card(c)) as HandOf<Card>;
//   }

//   static getRandomHandClan(clan?: Clan) {
//     let cards: CardJSON[];
//     if (clan !== undefined) {
//       cards = cardClans[clan];
//     } else {
//       // cards = cardClans[getN(Object.keys(cardClans) as Clan[])[0]];
//       cards = cardClans[getN(Clans as unknown as Clan[])[0]];
//     }

//     return getN(cards, 4).map((c) => new Card(c));
//   }

//   static getRandomHand(cards: HandOf<string | number>) {
//     let cardsArr = cards.map(c => {
//       if (typeof c == "string")
//         return new Card(cardNames[c.toLowerCase()]);
//       else
//         return new Card(cardIds[c]);
//     }) as HandOf<Card>;

//     cardsArr.push(
//       ...getN(Object.values(cardIds), 4 - cardsArr.length).map(
//         (c) => new Card(c)
//       )
//     );

//     return cardsArr;
//   }
// }

if (isMainThread || !false) {
  const data = fs.readFileSync("../../data/data.json").toString();

  json = JSON.parse(data);
  console.log(
    json.length.toString().green + " cards loaded!".white
  );

  cardIds = Object.fromEntries(json.map(i => [i.id, i]));
  cardNames = Object.fromEntries(json.map(i => [i.name.toLowerCase(), i]));

  // json.forEach(i => {
  for (const j of json) {
    const year = new Date(j.release_date * 1000).getFullYear();
    if (!cardYears[year]) {
      cardYears[year] = [];
    }

    cardYears[year].push(j);

    const clan = j.clan_name;
    if (!cardClans[clan]) {
      cardClans[clan] = [];
    }

    cardClans[clan].push(j);

    const o = { a: 0, b: 0 };
    const power: PowerStat = Object.setPrototypeOf(o, PowerStat.prototype);
    power.base = j.power;
    power.final = j.power;
    const damage: DamageStat = Object.setPrototypeOf(o, DamageStat.prototype);
    damage.base = j.damage;
    damage.final = j.damage;

    const key = (j.id & 0xffff) | (j.level << 16);
    baseCards[key] = {
      name: j.name,
      id: j.id,
      stars: j.level,
      maxStars: j.level_max,
      release_date: j.release_date * 1000,
      clan: j.clan_name,
      rarity: j.rarity,

      ability: { string: j.ability, prot: false, cancel: false },
      bonus: { string: j.bonus, prot: false, cancel: false },

      // power: {
      //   base: j.power, final: j.power, prot: false, cancel: false
      // },
      // damage: {
      //   base: j.damage, final: j.damage, prot: false, cancel: false
      // },
      // attack: { base: 0, final: 0, prot: false, cancel: false },

      // life: { prot: false, cancel: false },
      // pillz: { prot: false, cancel: false },
      data: o,
    }
  }
  // });
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

// Card.loaded = true;
// console.log('Calling callback function: ');
// console.log(Card.cb.toString());
// Card.cb();


// module.exports = Card;