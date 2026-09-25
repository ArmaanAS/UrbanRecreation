import "colors";
import { clone, getN } from "../utils/Utils.ts";
import {
  baseCards,
  cardClans,
  cardIds,
  cardLevels,
  cardNames,
  cardYears,
  getBaseKey,
  registerCardJSON,
  registerVariant,
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

interface Infiltration {
  readonly clan: Clan;
  readonly bonus: string;
}

export default class Card {
  private key: number;
  played = false;
  /** Set by Game when Clint City is at night: use the night ability / bonus variant. */
  night = false;
  private data: BaseData;
  /**
   * The clan this card joined and the bonus it took with it (Oculus "Infiltrated"), set by
   * Hand.from for this copy only. It lived on the shared (id, level) base row once, so it
   * outlived its game and an opposing copy of the same card overwrote it. Always assigned,
   * and never mutated once set, so clone() shares the reference and keeps one map.
   */
  private infiltration: Infiltration | undefined = undefined;
  constructor(json: CardJSON) {
    this.key = getBaseKey(json.id, json.level);

    if (this.base === undefined) {
      registerCardJSON(json);
    }

    this.data = clone(this.base.data);
  }

  /**
   * Up to four of these per Hand clone, so per search node, which makes this one of the
   * hottest allocations in the program. Object.create with the fields assigned in
   * declaration order, rather than re-prototyping a literal (~1600x dearer), and `data`
   * written as a fixed two-field literal rather than a spread so every card's packed words
   * share one map instead of going through the object-clone IC.
   */
  clone(): Card {
    const c: Card = Object.create(Card.prototype);
    c.key = this.key;
    c.played = this.played;
    c.night = this.night;
    c.data = { a: this.data.a, b: this.data.b } as BaseData;
    c.infiltration = this.infiltration;
    return c;
  }

  /**
   * A copy of this card that fights with `ability` by day and by night: the text the server
   * substituted for the printed one (Administrator's Hazard). It gets a base row of its own,
   * so the shared (id, level) row, and any opposing copy of the same card, keep theirs. Each
   * call registers a new row; build hands with it, never call it from a search.
   */
  withAbility(ability: string): Card {
    const c = this.clone();
    c.key = registerVariant(this.key, { ability, nightAbility: undefined });
    return c;
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
    return this.infiltration?.clan ?? this.base.clan;
  }
  /** Join `clan` with its `bonus` (Oculus), or leave with `undefined`: this copy only. */
  infiltrate(clan: Clan | undefined, bonus?: string) {
    this.infiltration = clan === undefined
      ? undefined
      : { clan, bonus: bonus! };
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

  /** Unison cards have their own green visual treatment even though rarity remains c/u/r. */
  get hasUnisonAbility() {
    const printedAbility = (this.night ? this.base.nightAbility : undefined) ??
      this.base.ability;
    return /^\s*Unison\s*:/i.test(printedAbility);
  }

  get abilityString(): string {
    if (this.ability.string !== AbilityString.DEFAULT) return "No Ability";
    return (this.night ? this.base.nightAbility : undefined) ??
      this.base.ability;
  }
  get bonusString(): string {
    if (this.bonus.string !== AbilityString.DEFAULT) return "No Bonus";
    if (this.infiltration !== undefined) return this.infiltration.bonus;
    return (this.night ? this.base.nightBonus : undefined) ?? this.base.bonus;
  }

  get ability(): AbilityStat {
    return new AbilityStat(this.data);
  }
  get bonus(): BonusStat {
    return new BonusStat(this.data);
  }
  get power(): PowerStat {
    return new PowerStat(this.data);
  }
  get damage(): DamageStat {
    return new DamageStat(this.data);
  }
  get attack(): AttackStat {
    return new AttackStat(this.data);
  }
  get pillz(): PillzStat {
    return new PillzStat(this.data);
  }
  get life(): LifeStat {
    return new LifeStat(this.data);
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
  /** Look up a card by id or name; `level` selects a specific evolution (default: max level). */
  static get(card: number | string, level?: number) {
    let data: CardJSON | undefined;
    if (typeof card == "number") {
      data = cardIds[card];
    } else {
      data = cardNames[card.toLowerCase()];
    }

    if (data !== undefined && level !== undefined && level !== data.level) {
      data = cardLevels[data.id]?.[level];
    }

    if (data === undefined) {
      return undefined;
    } else {
      return new Card(data);
    }
  }

  static getRandomCard(clan: Clan) {
    const cards = cardClans[clan];
    return new Card(getN(cards, 1)[0]);
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

    return getN(cards, 4).map((c) => new Card(c)) as HandOf<Card>;
  }

  static getRandomHand(cards: HandOf<string | number>) {
    const cardsArr = cards.map((c) => {
      const json = typeof c == "string"
        ? cardNames[c.toLowerCase()]
        : cardIds[c];
      if (json === undefined) {
        throw new Error(`Invalid card ID or Name: ${JSON.stringify(c)}`);
      }
      return new Card(json);
    }) as HandOf<Card>;

    cardsArr.push(
      ...getN(Object.values(cardIds), 4 - cardsArr.length)
        .map((c) => new Card(c)),
    );

    return cardsArr;
  }
}
