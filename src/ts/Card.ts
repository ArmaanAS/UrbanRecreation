import "colors"
import { clone, getN } from "./utils/Utils"
import {
  baseCards, cardIds, cardNames,
  cardYears, cardClans, getBaseKey, registerCardJSON
} from "./CardLoader";
import {
  // CardString, 
  CardJSON, HandOf, Clan, Clans,
  BaseData,
  AttackStat,
  DamageStat,
  LifeStat,
  PillzStat,
  PowerStat,
  AbilityStat,
  BonusStat,
  AbilityString
} from "./types/CardTypes";


export default class Card {
  private key = -1;
  played = false;
  private data: BaseData;
  constructor(json: CardJSON) {
    this.key = getBaseKey(json.id, json.level);

    if (this.base === undefined)
      registerCardJSON(json);

    this.data = clone(this.base.data);
  }

  clone(): Card {
    return Object.setPrototypeOf({
      key: this.key,
      played: this.played,

      // _ability: this._ability && { ...this._ability },
      // _bonus: this._bonus && { ...this._bonus },

      data: { ...this.data }
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

  // get ability() {
  //   return this._ability ?? this.base.ability;
  // }

  // get bonus() {
  //   return this._bonus ?? this.base.bonus;
  // }

  get abilityString(): string {
    return this.ability.string === AbilityString.DEFAULT ?
      this.base.ability : 'No Ability';
  }
  get bonusString(): string {
    return this.bonus.string === AbilityString.DEFAULT ?
      this.base.bonus : 'No Bonus';
  }

  get ability(): AbilityStat {
    return Object.setPrototypeOf(this.data, AbilityStat.prototype);
  }
  get bonus(): BonusStat {
    return Object.setPrototypeOf(this.data, BonusStat.prototype);
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
  get pillz(): PillzStat {
    return Object.setPrototypeOf(this.data, PillzStat.prototype);
  }
  get life(): LifeStat {
    return Object.setPrototypeOf(this.data, LifeStat.prototype);
  }


  // get ability_() {
  //   return this._ability ?? (this._ability = { ...this.base.ability });
  // }

  // get bonus_() {
  //   return this._bonus ?? (this._bonus = { ...this.base.bonus });
  // }

  // get power_(): PowerStat {
  //   return Object.setPrototypeOf(this.data, PowerStat.prototype);
  // }
  // get damage_(): DamageStat {
  //   return Object.setPrototypeOf(this.data, DamageStat.prototype);
  // }
  // get attack_(): AttackStat {
  //   return Object.setPrototypeOf(this.data, AttackStat.prototype);
  // }
  // get pillz_(): PillzStat {
  //   return Object.setPrototypeOf(this.data, PillzStat.prototype);
  // }
  // get life_(): LifeStat {
  //   return Object.setPrototypeOf(this.data, LifeStat.prototype);
  // }

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

  // get played() {
  //   return !!(this.data.b >> 31 & 1);
  // }
  // set played(n: boolean) {
  //   this.data.b = (this.data.b & ~(1 << 31)) | (+n << 31);
  // }
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

    if (data === undefined)
      return undefined;
    else
      return new Card(data);
    // return data && new Card(data);
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

