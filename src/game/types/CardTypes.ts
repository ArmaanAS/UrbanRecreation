export const Clans = [
  "All Stars",
  "Bangers",
  "Berzerk",
  "Dominion",
  "Fang Pi Clang",
  "Freaks",
  "Frozn",
  "GHEIST",
  "GhosTown",
  "Hive",
  "Huracan",
  "Jungo",
  "Junkz",
  "Komboka",
  "La Junta",
  "Leader",
  "Montana",
  "Nightmare",
  "Oculus",
  "Paradox",
  "Piranas",
  "Pussycats",
  "Raptors",
  "Rescue",
  "Riots",
  "Roots",
  "Sakrohm",
  "Sentinel",
  "Skeelz",
  "Ulu Watu",
  "Uppers",
  "Vortex",
] as const;
export type Clan = typeof Clans[number];

export type Rarity = "c" | "u" | "r" | "cr" | "m" | "l";

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
  ability: string;
  bonus: string;
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
// b = attack.base,.final, power.cancel,.prot  damage.cancel,.prot,
//     attack.cancel,.prot  pillz.cancel,.prot  life.cancel,.prot
//     index  won  played
// a = 00000 00000 00000 00000 000 000    = 26 bits
// b = 00000000 00000000 00 00 00 00 00 000 00 0 = 32 bits
export class BaseData {
  a = 0;
  b = 0;
  get power(): PowerStat {
    return Object.setPrototypeOf(this, PowerStat.prototype);
  }
  get damage(): DamageStat {
    return Object.setPrototypeOf(this, DamageStat.prototype);
  }
  get attack(): AttackStat {
    return Object.setPrototypeOf(this, AttackStat.prototype);
  }
  get pillz(): PillzStat {
    return Object.setPrototypeOf(this, PillzStat.prototype);
  }
  get life(): LifeStat {
    return Object.setPrototypeOf(this, LifeStat.prototype);
  }
}
abstract class BaseAttr extends BaseData {
  abstract get cancel(): boolean;
  abstract set cancel(n: boolean);
  abstract get prot(): boolean;
  abstract set prot(n: boolean);
  /**
   * `protected || !cancelled`
   */
  abstract get blocked(): boolean;
}
abstract class BaseStat extends BaseAttr {
  abstract get base(): number;
  abstract set base(n: number);
  abstract get final(): number;
  abstract set final(n: number);
}
export enum AbilityString {
  DEFAULT = 0,
  NO_ABILITY = 1,
}
abstract class BaseString extends BaseAttr {
  abstract get string(): AbilityString;
  abstract set string(n: AbilityString);
}
export class AbilityStat extends BaseString {
  get string(): AbilityString {
    return this.a >> 20 & 1;
  }
  set string(n: AbilityString) {
    this.a = (this.a & ~0x100000) | (n << 20);
  }
  get cancel(): boolean {
    return !!(this.a >> 21 & 1);
  }
  set cancel(n: boolean) {
    this.a = (this.a & ~0x200000) | (+n << 21);
  }
  get prot(): boolean {
    return !!(this.a >> 22 & 1);
  }
  set prot(n: boolean) {
    this.a = (this.a & ~0x400000) | (+n << 22);
  }
  get blocked() {
    return (this.a >> 21 & 0b11) === 0b01;
  }
}
export class BonusStat extends BaseString {
  get string(): AbilityString {
    return this.a >> 23 & 1;
  }
  set string(n: AbilityString) {
    this.a = (this.a & ~0x800000) | (n << 23);
  }
  get cancel(): boolean {
    return !!(this.a >> 24 & 1);
  }
  set cancel(n: boolean) {
    this.a = (this.a & ~0x1000000) | (+n << 24);
  }
  get prot(): boolean {
    return !!(this.a >> 25 & 1);
  }
  set prot(n: boolean) {
    this.a = (this.a & ~0x2000000) | (+n << 25);
  }
  get blocked() {
    return (this.a >> 24 & 0b11) === 0b01;
  }
}
export class PowerStat extends BaseStat {
  get base(): number {
    return this.a & 0x1f;
  }
  set base(n: number) {
    this.a = (this.a & ~0x1f) | (n & 0x1f);
  }
  get final(): number {
    return this.a >> 5 & 0x1f;
  }
  set final(n: number) {
    this.a = (this.a & ~0x3e0) | ((n & 0x1f) << 5);
  }
  get cancel(): boolean {
    return !!(this.b >> 16 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x10000) | (+n << 16);
  }
  get prot(): boolean {
    return !!(this.b >> 17 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x20000) | (+n << 17);
  }
  get blocked() {
    // prot || !cancel
    // 10, 11, 00 = true
    // 01 = false
    // Cancelled && !prot || prot
    // 01 = true
    // 10, 11, 00 = false,
    return (this.b >> 16 & 0b11) === 0b01;
  }
}
export class DamageStat extends BaseStat {
  get base(): number {
    return this.a >> 10 & 0x1f;
  }
  set base(n: number) {
    this.a = (this.a & ~0x7c00) | ((n & 0x1f) << 10);
  }
  get final(): number {
    return this.a >> 15 & 0x1f;
  }
  set final(n: number) {
    this.a = (this.a & ~0xf8000) | ((n & 0x1f) << 15);
  }
  get cancel(): boolean {
    return !!(this.b >> 18 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x40000) | (+n << 18);
  }
  get prot(): boolean {
    return !!(this.b >> 19 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x80000) | (+n << 19);
  }
  get blocked() {
    return (this.b >> 18 & 0b11) === 0b01;
  }
}
export class AttackStat extends BaseStat {
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
    this.b = (this.b & ~0x100000) | (+n << 20);
  }
  get prot(): boolean {
    return !!(this.b >> 21 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x200000) | (+n << 21);
  }
  get blocked() {
    return (this.b >> 20 & 0b11) === 0b01;
  }
}
export class PillzStat extends BaseAttr {
  get cancel(): boolean {
    return !!(this.b >> 22 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x400000) | (+n << 22);
  }
  get prot(): boolean {
    return !!(this.b >> 23 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x800000) | (+n << 23);
  }
  get blocked() {
    return (this.b >> 22 & 0b11) === 0b01;
  }
}
export class LifeStat extends BaseAttr {
  get cancel(): boolean {
    return !!(this.b >> 24 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x1000000) | (+n << 24);
  }
  get prot(): boolean {
    return !!(this.b >> 25 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x2000000) | (+n << 25);
  }
  get blocked() {
    return (this.b >> 24 & 0b11) === 0b01;
  }
}
