export const Clans = {
  "All Stars": 38,
  "Bangers": 31,
  "Berzerk": 46,
  "Cosmohnuts": 58,
  "Dominion": 53,
  "Fang Pi Clang": 25,
  "Freaks": 40,
  "Frozn": 47,
  "GHEIST": 32,
  "GhosTown": 52,
  "Hive": 51,
  "Huracan": 48,
  "Jungo": 43,
  "Junkz": 26,
  "Komboka": 54,
  "La Junta": 27,
  "Leader": 36,
  "Montana": 3,
  "Nightmare": 37,
  "Oblivion": 57,
  "Oculus": 56,
  "Paradox": 55,
  "Piranas": 42,
  "Pussycats": 4,
  "Raptors": 50,
  "Rescue": 41,
  "Riots": 49,
  "Roots": 29,
  "Sakrohm": 30,
  "Sentinel": 33,
  "Skeelz": 44,
  "Tolvack": 60,
  "Ulu Watu": 10,
  "Uppers": 28,
  "Vortex": 45,
  "Zenith": 59,
} as const;
export const ClanNames = Object.keys(Clans) as (keyof typeof Clans)[];
export const ClanIdMap = Object.fromEntries(
  Object.entries(Clans).map(([k, v]) => [v, k]),
) as Record<ClanId, Clan>;
export type Clan = keyof typeof Clans;
export type ClanId = typeof Clans[Clan];

/** Compact, stable labels for places where full clan names cannot fit, such as card faces. */
export const ClanAbbreviations: Record<Clan, string> = {
  "All Stars": "AS",
  Bangers: "BA",
  Berzerk: "BZ",
  Cosmohnuts: "CO",
  Dominion: "DO",
  "Fang Pi Clang": "FP",
  Freaks: "FR",
  Frozn: "FZ",
  GHEIST: "GH",
  GhosTown: "GT",
  Hive: "HI",
  Huracan: "HU",
  Jungo: "JG",
  Junkz: "JZ",
  Komboka: "KO",
  "La Junta": "LJ",
  Leader: "LE",
  Montana: "MO",
  Nightmare: "NM",
  Oblivion: "OB",
  Oculus: "OC",
  Paradox: "PA",
  Piranas: "PI",
  Pussycats: "PC",
  Raptors: "RA",
  Rescue: "RE",
  Riots: "RI",
  Roots: "RO",
  Sakrohm: "SA",
  Sentinel: "SE",
  Skeelz: "SK",
  Tolvack: "TO",
  "Ulu Watu": "UW",
  Uppers: "UP",
  Vortex: "VO",
  Zenith: "ZE",
};

// export type Rarity = "c" | "u" | "r" | "cr" | "m" | "l";
export type Rarity = "c" | "u" | "r" | "cr" | "l";

export type Stars = 1 | 2 | 3 | 4 | 5;
export type MaxStars = 2 | 3 | 4 | 5;
export type Power = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9;
export type Damage = Power;

export interface CardJSON {
  name: string;
  id: number;
  level: Stars;
  level_max: MaxStars;
  release_date: number;
  clan_name: Clan;
  rarity: Rarity;
  power: Power;
  damage: Damage;
  ability_id: number;
  ability: string;
  bonus: string;
  /** Night variants (GhosTown and a few others); undefined = same as day. */
  night_ability?: string;
  night_bonus?: string;
}

export type HandOf<T> = [T, T, T, T];

// interface CardAttr {
//   cancel: boolean;
//   prot: boolean;
// }
// interface CardStat extends CardAttr {
//   base: number;
//   final: number;
// }
// export interface CardString extends CardAttr {
//   string: string;
// }
export interface BaseCard {
  name: string;
  id: number;
  stars: Stars;
  maxStars: MaxStars;
  release_date: number;
  clan: Clan;
  rarity: Rarity;
  ability: string;
  bonus: string;
  nightAbility?: string;
  nightBonus?: string;
  // ability: CardString; // 3 bits
  // bonus: CardString; // 3 bits
  // power: CardStat;  // 18 bits
  // damage: CardStat; // 18 bits
  // attack: CardStat; // 18 bits
  // life: CardAttr; // 2 bits
  // pillz: CardAttr; // 2 bits
  data: BaseData;
} // 58 bits + 3 bit index = 61 bits = 2 numbers

// a = power.base,.final, damage.base,.final,
//     ability.string,.cancel,.prot  bonus.string,.cancel,.prot
//     power.guard  damage.guard
// b = attack.base,.final, power.cancel,.prot  damage.cancel,.prot,
//     attack.cancel,.prot  pillz.cancel,.prot  life.cancel,.prot
//     index  won  played
// a = 00000 00000 00000 00000 000 000 00 = 28 bits
// b = 00000000 00000000 00 00 00 00 00 000 00 0 = 32 bits
export class BaseData {
  a = 0;
  b = 0;
  get power(): PowerStat {
    return new PowerStat(this);
  }
  get damage(): DamageStat {
    return new DamageStat(this);
  }
  get attack(): AttackStat {
    return new AttackStat(this);
  }
  get pillz(): PillzStat {
    return new PillzStat(this);
  }
  get life(): LifeStat {
    return new LifeStat(this);
  }
}

/**
 * A stat view over a card's two packed words.
 *
 * These used to be selected by swapping the packed object's prototype
 * (`Object.setPrototypeOf(this, PowerStat.prototype)`), which kept allocation at one
 * object per card - the thing that makes a clone cheap enough to search millions of
 * states. That allocation was never the problem; the dispatch was. Each access was an
 * un-inlinable runtime call that changed the packed object's map, so it churned through
 * map transitions and every `.final` site went megamorphic. A throwaway view costs
 * nothing by comparison, but only while TurboFan can inline its constructor so escape
 * analysis drops the allocation, and that needs each view class to stand alone. They
 * used to share an abstract `BaseAttr -> BaseStat` chain whose constructor stored `d`,
 * which defeated it twice over on V8 15 (tests/CardAccess.bench.ts): every `super()`
 * call went through the `FindNonDefaultConstructorOrConstruct` builtin, which TurboFan
 * did not inline, and the one `d` store in the shared constructor saw all seven view
 * maps, more than a polymorphic site holds. Either alone kept the allocation, and a real
 * object per access cost the search a fifth of `deno task time-search` and an eighth of
 * `deno task time`. So each view owns its constructor and `d`, and the shape they share
 * is the type-only interfaces below. Do not give them a runtime base class again.
 */
interface Attr {
  cancel: boolean;
  prot: boolean;
  /**
   * `protected || !cancelled`
   */
  readonly blocked: boolean;
}
interface Stat extends Attr {
  base: number;
  final: number;
}
export enum AbilityString {
  DEFAULT = 0,
  NO_ABILITY = 1,
}
interface StringAttr extends Attr {
  string: AbilityString;
}
export class AbilityStat implements StringAttr {
  constructor(private readonly d: BaseData) {}
  get string(): AbilityString {
    return this.d.a >> 20 & 1;
  }
  set string(n: AbilityString) {
    this.d.a = (this.d.a & ~0x100000) | (n << 20);
  }
  get cancel(): boolean {
    return !!(this.d.a >> 21 & 1);
  }
  set cancel(n: boolean) {
    this.d.a = (this.d.a & ~0x200000) | (+n << 21);
  }
  get prot(): boolean {
    return !!(this.d.a >> 22 & 1);
  }
  set prot(n: boolean) {
    this.d.a = (this.d.a & ~0x400000) | (+n << 22);
  }
  get blocked() {
    return (this.d.a >> 21 & 0b11) === 0b01;
  }
}
export class BonusStat implements StringAttr {
  constructor(private readonly d: BaseData) {}
  get string(): AbilityString {
    return this.d.a >> 23 & 1;
  }
  set string(n: AbilityString) {
    this.d.a = (this.d.a & ~0x800000) | (n << 23);
  }
  get cancel(): boolean {
    return !!(this.d.a >> 24 & 1);
  }
  set cancel(n: boolean) {
    this.d.a = (this.d.a & ~0x1000000) | (+n << 24);
  }
  get prot(): boolean {
    return !!(this.d.a >> 25 & 1);
  }
  set prot(n: boolean) {
    this.d.a = (this.d.a & ~0x2000000) | (+n << 25);
  }
  get blocked() {
    return (this.d.a >> 24 & 0b11) === 0b01;
  }
}
export class PowerStat implements Stat {
  constructor(private readonly d: BaseData) {}
  get base(): number {
    return this.d.a & 0x1f;
  }
  set base(n: number) {
    this.d.a = (this.d.a & ~0x1f) | (n & 0x1f);
  }
  get final(): number {
    return this.d.a >> 5 & 0x1f;
  }
  set final(n: number) {
    this.d.a = (this.d.a & ~0x3e0) | ((n & 0x1f) << 5);
  }
  get cancel(): boolean {
    return !!(this.d.b >> 16 & 1);
  }
  set cancel(n: boolean) {
    this.d.b = (this.d.b & ~0x10000) | (+n << 16);
  }
  get prot(): boolean {
    return !!(this.d.b >> 17 & 1);
  }
  set prot(n: boolean) {
    this.d.b = (this.d.b & ~0x20000) | (+n << 17);
  }
  get blocked() {
    // prot || !cancel
    // 10, 11, 00 = true
    // 01 = false
    // Cancelled && !prot || prot
    // 01 = true
    // 10, 11, 00 = false,
    return (this.d.b >> 16 & 0b11) === 0b01;
  }
  /**
   * Refuses the opposing card's reductions of this stat (`Protection: Power And Damage`).
   * Separate from `prot`, which only resists a Cancel: the single-stat Protections set
   * `prot` too, but no capture shows one refusing a reduction of the stat it names.
   */
  get guard(): boolean {
    return !!(this.d.a >> 26 & 1);
  }
  set guard(n: boolean) {
    this.d.a = (this.d.a & ~0x4000000) | (+n << 26);
  }
}
export class DamageStat implements Stat {
  constructor(private readonly d: BaseData) {}
  get base(): number {
    return this.d.a >> 10 & 0x1f;
  }
  set base(n: number) {
    this.d.a = (this.d.a & ~0x7c00) | ((n & 0x1f) << 10);
  }
  get final(): number {
    return this.d.a >> 15 & 0x1f;
  }
  set final(n: number) {
    this.d.a = (this.d.a & ~0xf8000) | ((n & 0x1f) << 15);
  }
  get cancel(): boolean {
    return !!(this.d.b >> 18 & 1);
  }
  set cancel(n: boolean) {
    this.d.b = (this.d.b & ~0x40000) | (+n << 18);
  }
  get prot(): boolean {
    return !!(this.d.b >> 19 & 1);
  }
  set prot(n: boolean) {
    this.d.b = (this.d.b & ~0x80000) | (+n << 19);
  }
  get blocked() {
    return (this.d.b >> 18 & 0b11) === 0b01;
  }
  /** See `PowerStat.guard`. */
  get guard(): boolean {
    return !!(this.d.a >> 27 & 1);
  }
  set guard(n: boolean) {
    this.d.a = (this.d.a & ~0x8000000) | (+n << 27);
  }
}
export class AttackStat implements Stat {
  constructor(private readonly d: BaseData) {}
  get base(): number {
    return this.d.b & 0xff;
  }
  set base(n: number) {
    this.d.b = (this.d.b & ~0xff) | (n & 0xff);
  }
  get final(): number {
    return this.d.b >> 8 & 0xff;
  }
  set final(n: number) {
    this.d.b = (this.d.b & ~0xff00) | ((n & 0xff) << 8);
  }
  get cancel(): boolean {
    return !!(this.d.b >> 20 & 1);
  }
  set cancel(n: boolean) {
    this.d.b = (this.d.b & ~0x100000) | (+n << 20);
  }
  get prot(): boolean {
    return !!(this.d.b >> 21 & 1);
  }
  set prot(n: boolean) {
    this.d.b = (this.d.b & ~0x200000) | (+n << 21);
  }
  get blocked() {
    return (this.d.b >> 20 & 0b11) === 0b01;
  }
}
export class PillzStat implements Attr {
  constructor(private readonly d: BaseData) {}
  get cancel(): boolean {
    return !!(this.d.b >> 22 & 1);
  }
  set cancel(n: boolean) {
    this.d.b = (this.d.b & ~0x400000) | (+n << 22);
  }
  get prot(): boolean {
    return !!(this.d.b >> 23 & 1);
  }
  set prot(n: boolean) {
    this.d.b = (this.d.b & ~0x800000) | (+n << 23);
  }
  get blocked() {
    return (this.d.b >> 22 & 0b11) === 0b01;
  }
}
export class LifeStat implements Attr {
  constructor(private readonly d: BaseData) {}
  get cancel(): boolean {
    return !!(this.d.b >> 24 & 1);
  }
  set cancel(n: boolean) {
    this.d.b = (this.d.b & ~0x1000000) | (+n << 24);
  }
  get prot(): boolean {
    return !!(this.d.b >> 25 & 1);
  }
  set prot(n: boolean) {
    this.d.b = (this.d.b & ~0x2000000) | (+n << 25);
  }
  get blocked() {
    return (this.d.b >> 24 & 0b11) === 0b01;
  }
}
