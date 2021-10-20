import Game from "../Game"

export const Clans = [
  "All Stars", "Bangers", "Berzerk", "Dominion", "Fang Pi Clang",
  "Freaks", "Frozn", "GHEIST", "GhosTown", "Hive", "Huracan", "Jungo",
  "Junkz", "Komboka", "La Junta", "Leader", "Montana", "Nightmare",
  "Paradox", "Piranas", "Pussycats", "Raptors", "Rescue", "Riots", "Roots",
  "Sakrohm", "Sentinel", "Skeelz", "Ulu Watu", "Uppers", "Vortex"
] as const
export type Clan = typeof Clans[number]

export type Rarity = "c" | "u" | "r" | "cr" | "m" | "l"

export type Stars = 1 | 2 | 3 | 4 | 5
export type MaxStars = 2 | 3 | 4 | 5
export type Power = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9
export type Damage = Power

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

export type HandOf<T> = [T, T, T, T]


export interface WorkerSolverData {
  id: number;
  game: Game;
}