import "colors";
import { clone, getN } from "../utils/Utils.ts";
import {
  baseCards,
  cardClans,
  cardIds,
  cardNames,
  cardYears,
  getBaseKey,
  registerCardJSON,
} from "./CardLoader.ts";
import {
  AbilityStat,
  AbilityString,
  AttackStat,
  BaseData,
  BonusStat,
  // CardString,
  CardJSON,
  Clan,
  ClanNames,
  DamageStat,
  HandOf,
  LifeStat,
  PillzStat,
  PowerStat,
} from "./types/CardTypes.ts";

export default class Card {
  private key: number;
  played = false;
  private data: BaseData;
  constructor(json: CardJSON) {
    this.key = getBaseKey(json.id, json.level);

    if (this.base === undefined) {
      registerCardJSON(json);
    }

    this.data = clone(this.base.data);
  }

  clone(): Card {
    return Object.setPrototypeOf({
      key: this.key,
      played: this.played,
      data: { ...this.data },
    }, Card.prototype);
  }

  static from(o: Card): Card {
    return Object.setPrototypeOf(o, Card.prototype);
  }

  private get base() {
    return baseCards[this.key];
  }

  get year() {
    return new Date(this.base.release_date).getFullYear().toString();
  }

  get clan() {
    return this.base.infiltratedClan ?? this.base.clan;
  }
  set clan(clan: Clan) {
    this.base.infiltratedClan = clan;
  }
  get baseClan() {
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

  get abilityString(): string {
    return this.ability.string === AbilityString.DEFAULT
      ? this.base.ability
      : "No Ability";
  }
  get bonusString(): string {
    return this.bonus.string === AbilityString.DEFAULT
      ? this.base.infiltratedBonus ?? this.base.bonus
      : "No Bonus";
  }
  set bonusString(text: string) {
    this.base.infiltratedBonus = text;
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

  get index() {
    return this.data.b >> 26 & 0b111;
  }
  set index(n: number) {
    this.data.b = (this.data.b & ~(0b111 << 26)) | ((n & 0b111) << 26);
  }

  get won() {
    // 0 undefined, 0b10 false, 0b11 true
    const a = this.data.b >> 29 & 0b11;
    return a === 0 ? undefined : a === 3;
  }
  set won(n: boolean | undefined) {
    if (n === undefined) {
      this.data.b &= ~(0b11 << 29);
    } else {
      this.data.b = (this.data.b & ~(0b11 << 29)) | (((+n << 1) | 0b1) << 29);
    }
  }

  // get played() {
  //   return !!(this.data.b >> 31 & 1);
  // }
  // set played(n: boolean) {
  //   this.data.b = (this.data.b & ~(1 << 31)) | (+n << 31);
  // }
}

export class CardGenerator {
  static get(card: number | string) {
    let data: CardJSON | undefined;
    if (typeof card == "number") {
      data = cardIds[card];
    } else {
      data = cardNames[card];
    }

    if (data === undefined) {
      return undefined;
    } else {
      return new Card(data);
    }
    // return data && new Card(data);
  }

  static getRandomHandYear(year = 2006) {
    const card = getN(cardYears[year])[0];
    const cards = cardYears[year].filter((j) => j.clan_name == card.clan_name);

    return getN(cards, 4).map((c) => new Card(c)) as HandOf<Card>;
  }

  static getRandomHandClan(clan?: Clan) {
    let cards: CardJSON[];
    if (clan !== undefined) {
      cards = cardClans[clan];
    } // cards = cardClans[getN(Object.keys(cardClans) as Clan[])[0]];
    else {
      cards = cardClans[getN(ClanNames)[0]];
    }

    return getN(cards, 4).map((c) => new Card(c));
  }

  static getRandomHand(cards: HandOf<string | number>) {
    const cardsArr = cards.map((c) => {
      if (typeof c == "string") {
        return new Card(cardNames[c.toLowerCase()]);
      } else {
        return new Card(cardIds[c]);
      }
    }) as HandOf<Card>;

    cardsArr.push(
      ...getN(Object.values(cardIds), 4 - cardsArr.length)
        .map((c) => new Card(c)),
    );

    return cardsArr;
  }
}
