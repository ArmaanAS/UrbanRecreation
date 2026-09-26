// The site's own collection, deck and format records, as Collection Pro receives them from
// `POST /ajax/collection/` (and the game client from `collections.decks`). See
// docs/site-api.md for the transports and docs/deck-builder-design.md for how they are used.
//
// These types describe the parts a deck builder reads. The site sends more, and the parsers
// keep only what is typed here, so a personal field (market offers, acquisition times) never
// reaches a file by accident.

/** An ability or bonus as the site describes it at one level. */
export interface SiteAbility {
  id: number;
  typeID: number;
  unlockLevel?: number;
  description: string;
  longDescription?: string;
}

/** One level of a character. `nightAbility` is `[]` when the card has no night variant. */
export interface SiteEvo {
  power: number;
  damage: number;
  ability: SiteAbility;
  nightAbility: SiteAbility | [];
  pictureURL?: string;
}

/** A character from `action=collectiondata`, without the owner's copies or market data. */
export interface SiteCard {
  id: number;
  name: string;
  clan_id: number;
  clan_name: string;
  level_min: number;
  level_max: number;
  rarity: string;
  kind?: string;
  release_date: number;
  is_clan_leader?: boolean;
  efc_banned: boolean;
  efc_max_evo_banned: boolean;
  efc_temp_banned: boolean;
  efc_bonus_low: boolean;
  efc_bonus_high: boolean;
  tourney_banned: boolean;
  tourney_max_evo_banned: boolean;
  evos: Record<string, SiteEvo>;
  bonus: SiteAbility;
  nightBonus: SiteAbility | [];
}

/** Copies owned, by level then edition (`""` classic, `p`, `s`, `m1`, `rp`, `i`, ...). */
export type OwnedCopies = Record<string, Record<string, number>>;

export interface DeckFormatCriterion {
  name: string;
  description: string;
  // deno-lint-ignore no-explicit-any
  value: any;
}

export interface DeckFormatData {
  id: number;
  name: string;
  isOfficial?: boolean;
  criteria: DeckFormatCriterion[];
}

export interface DeckCard {
  id: number;
  level: number;
  state: string;
}

export interface SiteDeck {
  id: number;
  name: string;
  isCurrent: boolean;
  characters: DeckCard[];
}

// deno-lint-ignore no-explicit-any
type Json = any;

const ability = (a: Json): SiteAbility => ({
  id: Number(a?.id ?? 0),
  typeID: Number(a?.typeID ?? 0),
  ...(a?.unlockLevel !== undefined ? { unlockLevel: Number(a.unlockLevel) } : {}),
  description: String(a?.description ?? ""),
  ...(a?.longDescription ? { longDescription: String(a.longDescription) } : {}),
});

const maybeAbility = (a: Json): SiteAbility | [] =>
  a && !Array.isArray(a) && typeof a === "object" && "description" in a ? ability(a) : [];

/** The public part of one `collectiondata` row. */
export function siteCard(row: Json): SiteCard {
  const evos: Record<string, SiteEvo> = {};
  for (const [level, evo] of Object.entries(row.evos ?? {}) as [string, Json][]) {
    evos[level] = {
      power: Number(evo.power),
      damage: Number(evo.damage),
      ability: ability(evo.ability),
      nightAbility: maybeAbility(evo.nightAbility),
      ...(evo.pictureURL ? { pictureURL: String(evo.pictureURL) } : {}),
    };
  }
  return {
    id: Number(row.id),
    name: String(row.name),
    clan_id: Number(row.clan_id),
    clan_name: String(row.clan_name),
    level_min: Number(row.level_min),
    level_max: Number(row.level_max),
    rarity: String(row.rarity),
    ...(row.kind !== undefined ? { kind: String(row.kind) } : {}),
    release_date: Number(row.release_date),
    ...(row.is_clan_leader !== undefined ? { is_clan_leader: !!row.is_clan_leader } : {}),
    efc_banned: !!row.efc_banned,
    efc_max_evo_banned: !!row.efc_max_evo_banned,
    efc_temp_banned: !!row.efc_temp_banned,
    efc_bonus_low: !!row.efc_bonus_low,
    efc_bonus_high: !!row.efc_bonus_high,
    tourney_banned: !!row.tourney_banned,
    tourney_max_evo_banned: !!row.tourney_max_evo_banned,
    evos,
    bonus: ability(row.bonus),
    nightBonus: maybeAbility(row.nightBonus),
  };
}

/**
 * The owner's copies from a row's `collectionData`: `lvl_<L>` for the classic edition and
 * `lvl_<L>_<state>` for every other one. Zero counts are dropped, and an unowned card
 * yields `{}`.
 */
export function ownedCopies(collectionData: Json): OwnedCopies {
  const owned: OwnedCopies = {};
  for (const [key, value] of Object.entries(collectionData ?? {})) {
    const m = /^lvl_(\d)(?:_([a-z0-9]+))?$/.exec(key);
    const count = Number(value);
    if (!m || !(count > 0)) continue;
    (owned[m[1]] ??= {})[m[2] ?? ""] = count;
  }
  return owned;
}

export function deckFormat(row: Json): DeckFormatData {
  return {
    id: Number(row.id),
    name: String(row.name),
    ...(row.isOfficial !== undefined ? { isOfficial: !!row.isOfficial } : {}),
    criteria: (row.criteria ?? []).map((c: Json) => ({
      name: String(c.name),
      description: String(c.description ?? ""),
      value: c.value,
    })),
  };
}

/** A deck from `loaddeck`/`savedeck` (`Characters`) or `collections.decks` (`characters`). */
export function siteDeck(row: Json): SiteDeck {
  const characters = (row.characters ?? row.Characters ?? []) as Json[];
  return {
    id: Number(row.id),
    name: String(row.name ?? ""),
    isCurrent: !!row.isCurrent,
    characters: characters.map((c) => ({
      id: Number(c.id),
      level: Number(c.level),
      state: String(c.state ?? ""),
    })),
  };
}
