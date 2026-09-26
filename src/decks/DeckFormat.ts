// Deck legality, ported from the site's own validator: `DeckFormat.parseDeck` in
// Collection Pro's collection-pro-bundle.min.js (read 2026-09-26). The site runs it in the
// browser when a game room is picked in the deck panel, so matching it means our verdicts
// match what the site shows. Its quirks are kept on purpose - for example a
// `contained_*_list` criterion fails when *every* used clan, character or ability type is
// in the list - and marked where they look surprising.
//
// One deliberate difference: the site silently ignores a criterion it does not know. Here
// an unknown criterion makes the verdict `null` ("cannot validate"), never a pass, so a new
// rule the site adds shows up instead of being waved through.
import type { DeckFormatCriterion, DeckFormatData } from "./SiteData.ts";

/** What the validator needs to know about one card of a deck, at the level played. */
export interface DeckCharacter {
  id: number;
  level: number;
  level_max: number;
  rarity: string;
  release_date: number;
  clan_id: number;
  efc_banned: boolean;
  /** The ability's type at that level (the night variant's at night); 0 for none. */
  abilityTypeID: number;
}

export interface FormatVerdict {
  formatId: number;
  name: string;
  /** Criteria the deck breaks, with the site's own description. */
  errors: DeckFormatCriterion[];
  /** Criteria this port does not understand. Any of them makes `legal` null. */
  unknown: DeckFormatCriterion[];
  /** Deck positions the site would highlight in red. */
  invalidIndexes: number[];
  /** true legal, false illegal, null cannot tell (an unknown criterion). */
  legal: boolean | null;
}

/** The site treats clan 36 as the Leaders, who never count as a clan. */
export const LEADER_CLAN_ID = 36;

const RARITY_TOTAL: Record<string, string> = {
  c: "commons",
  u: "uncommons",
  r: "rares",
  l: "ld",
  cr: "cr",
  m: "mt",
};
const RARITY_KEYS = ["commons", "uncommons", "rares", "ld", "cr", "mt"];

const KNOWN = new Set([
  "min_characters",
  "max_characters",
  "min_stars",
  "max_stars",
  "no_collectors",
  "min_leaders",
  "max_leaders",
  "min_clans",
  "max_clans",
  "min_character_level",
  "max_character_level",
  "min_evolving_characters",
  "max_evolving_characters",
  "min_maxxed_characters",
  "max_maxxed_characters",
  "min_release_date",
  "max_release_date",
  "no_doubles",
  "exclude_elo_forbidden",
  "authorized_clan_list",
  "contained_clan_list",
  "forbidden_clan_list",
  "authorized_character_list",
  "contained_character_list",
  "forbidden_character_list",
  "forbidden_maxed_character_list",
  "authorized_ability_type_list",
  "contained_ability_type_list",
  "forbidden_ability_type_list",
  "force_balanced_clans",
]);

export function isKnownCriterion(name: string): boolean {
  return KNOWN.has(name) || /^max_level[1-5]_characters$/.test(name) ||
    RARITY_KEYS.some((r) => name === `max_${r}_characters`);
}

/** The site's `findCriteriaErrors`, criterion by criterion. */
function criteriaErrors(format: DeckFormatData, deck: DeckCharacter[]): DeckFormatCriterion[] {
  const errors: DeckFormatCriterion[] = [];
  const usedClans = new Map<number, number>();
  const usedCharacters = new Set<number>();
  const maxedCharacters = new Set<number>();
  const usedAbilityTypes = new Set<number>();
  let hasDoubles = false;
  let eloForbidden = 0;
  const levels: Record<string, number> = { totalStars: 0, totalEvolving: 0, totalMaxed: 0 };
  const rarities: Record<string, number> = Object.fromEntries(RARITY_KEYS.map((r) => [r, 0]));
  let leaders = 0, minLevel = 0, maxLevel = 0, minRelease = 0, maxRelease = 0;

  for (const c of deck) {
    levels.totalStars += c.level;
    levels[`total${c.level}`] = (levels[`total${c.level}`] ?? 0) + 1;
    const rarity = RARITY_TOTAL[c.rarity];
    if (rarity) rarities[rarity]++;
    if (c.level >= c.level_max) levels.totalMaxed++;
    else levels.totalEvolving++;
    if (minLevel <= 0 || c.level < minLevel) minLevel = c.level;
    if (c.level > maxLevel) maxLevel = c.level;
    if (minRelease <= 0 || c.release_date < minRelease) minRelease = c.release_date;
    if (c.release_date > maxRelease) maxRelease = c.release_date;
    if (!hasDoubles && usedCharacters.has(c.id)) hasDoubles = true;
    else usedCharacters.add(c.id);
    if (c.level >= c.level_max) maxedCharacters.add(c.id);
    if (c.efc_banned) eloForbidden++;
    if (c.clan_id === LEADER_CLAN_ID) leaders++;
    else usedClans.set(c.clan_id, (usedClans.get(c.clan_id) ?? 0) + 1);
    usedAbilityTypes.add(c.abilityTypeID);
  }

  const clanIds = [...usedClans.keys()];
  const characterIds = [...usedCharacters];
  const typeIds = [...usedAbilityTypes].filter((t) => t !== 0);
  const inList = (list: unknown, id: number) => Array.isArray(list) && list.includes(id);

  for (const criterion of format.criteria) {
    const { name, value } = criterion;
    const fail = () => errors.push(criterion);
    if (name === "min_characters" && deck.length < value) fail();
    if (name === "max_characters" && deck.length > value) fail();
    if (name === "min_stars" && levels.totalStars < value) fail();
    if (name === "max_stars" && levels.totalStars > value) fail();
    const level = /^max_level([1-5])_characters$/.exec(name);
    if (level && (levels[`total${level[1]}`] ?? 0) > value) fail();
    for (const r of RARITY_KEYS) if (name === `max_${r}_characters` && rarities[r] > value) fail();
    if (name === "no_collectors" && rarities.cr > 0) fail();
    if (name === "min_leaders" && leaders < value) fail();
    if (name === "max_leaders" && leaders > value) fail();
    if (name === "min_clans" && clanIds.length < value) fail();
    if (name === "max_clans" && clanIds.length > value) fail();
    if (name === "min_character_level" && minLevel < value) fail();
    if (name === "max_character_level" && maxLevel > value) fail();
    if (name === "min_evolving_characters" && levels.totalEvolving < value) fail();
    if (name === "max_evolving_characters" && levels.totalEvolving > value) fail();
    if (name === "min_maxxed_characters" && levels.totalMaxed < value) fail();
    if (name === "max_maxxed_characters" && levels.totalMaxed > value) fail();
    if (name === "min_release_date" && minRelease < value) fail();
    if (name === "max_release_date" && maxRelease > value) fail();
    if (name === "no_doubles" && hasDoubles) fail();
    if (name === "exclude_elo_forbidden" && eloForbidden > 0) fail();
    if (name === "authorized_clan_list" && clanIds.some((id) => !inList(value, id))) fail();
    // The site's rule, kept as it is: it fails when no used clan lies outside the list.
    if (name === "contained_clan_list" && !clanIds.some((id) => !inList(value, id))) fail();
    if (name === "forbidden_clan_list" && clanIds.some((id) => inList(value, id))) fail();
    if (name === "authorized_character_list" && characterIds.some((id) => !inList(value, id))) fail();
    if (name === "contained_character_list" && !characterIds.some((id) => !inList(value, id))) fail();
    if (name === "forbidden_character_list" && characterIds.some((id) => inList(value, id))) fail();
    if (name === "forbidden_maxed_character_list" && [...maxedCharacters].some((id) => inList(value, id))) fail();
    if (name === "authorized_ability_type_list" && typeIds.some((id) => !inList(value, id))) fail();
    if (name === "contained_ability_type_list" && !typeIds.some((id) => !inList(value, id))) fail();
    if (name === "forbidden_ability_type_list" && typeIds.some((id) => inList(value, id))) fail();
    if (name === "force_balanced_clans" && new Set(usedClans.values()).size > 1) fail();
  }
  return errors;
}

/** The site's `findCharactersIndexesConcernedByCriteriaErrors`. */
function invalidIndexes(deck: DeckCharacter[], errors: DeckFormatCriterion[]): number[] {
  const bad = new Set<number>();
  const capped = (value: number, test: (c: DeckCharacter) => boolean) => {
    let seen = 0;
    deck.forEach((c, i) => {
      if (!test(c)) return;
      if (seen >= value) bad.add(i);
      else seen++;
    });
  };
  for (const { name, value } of errors) {
    if (name === "max_characters") for (let i = value; i < deck.length; i++) bad.add(i);
    const level = /^max_level([1-5])_characters$/.exec(name);
    if (level) capped(value, (c) => c.level === Number(level[1]));
    // The site compares the card's rarity code with these long names here, so this branch
    // never matches a card; kept for fidelity.
    for (const r of RARITY_KEYS) if (name === `max_${r}_characters`) capped(value, (c) => c.rarity === r);
    if (name === "no_collectors") deck.forEach((c, i) => c.rarity === "cr" && bad.add(i));
    if (name === "max_leaders") capped(value, (c) => c.clan_id === LEADER_CLAN_ID);
    if (name === "max_clans") {
      const clans: number[] = [];
      deck.forEach((c, i) => {
        if (!clans.includes(c.clan_id) && clans.length < value) clans.push(c.clan_id);
        else if (!clans.includes(c.clan_id)) bad.add(i);
      });
    }
    if (name === "min_character_level") deck.forEach((c, i) => c.level < value && bad.add(i));
    if (name === "max_character_level") deck.forEach((c, i) => c.level > value && bad.add(i));
    if (name === "max_evolving_characters") capped(value, (c) => c.level < c.level_max);
    if (name === "max_maxxed_characters") capped(value, (c) => c.level >= c.level_max);
    if (name === "min_release_date") deck.forEach((c, i) => c.release_date < value && bad.add(i));
    if (name === "max_release_date") deck.forEach((c, i) => c.release_date > value && bad.add(i));
    if (name === "no_doubles") {
      const ids: number[] = [];
      deck.forEach((c, i) => ids.includes(c.id) ? bad.add(i) : ids.push(c.id));
    }
    if (name === "exclude_elo_forbidden") deck.forEach((c, i) => c.efc_banned && bad.add(i));
    const list = Array.isArray(value) ? value as number[] : [];
    if (name === "authorized_clan_list") deck.forEach((c, i) => !list.includes(c.clan_id) && bad.add(i));
    if (name === "forbidden_clan_list") deck.forEach((c, i) => list.includes(c.clan_id) && bad.add(i));
    if (name === "authorized_character_list") deck.forEach((c, i) => !list.includes(c.id) && bad.add(i));
    if (name === "forbidden_character_list") deck.forEach((c, i) => list.includes(c.id) && bad.add(i));
    if (name === "forbidden_maxed_character_list") {
      deck.forEach((c, i) => list.includes(c.id) && c.level >= c.level_max && bad.add(i));
    }
    if (name === "authorized_ability_type_list") {
      deck.forEach((c, i) => c.abilityTypeID > 0 && !list.includes(c.abilityTypeID) && bad.add(i));
    }
    if (name === "forbidden_ability_type_list") {
      deck.forEach((c, i) => c.abilityTypeID > 0 && list.includes(c.abilityTypeID) && bad.add(i));
    }
  }
  return [...bad].sort((a, b) => a - b);
}

/** Whether `deck` satisfies `format`, as the site's own deck panel would say. */
export function checkDeck(format: DeckFormatData, deck: DeckCharacter[]): FormatVerdict {
  const unknown = format.criteria.filter((c) => !isKnownCriterion(c.name));
  // Like the site: no format (id 0), an empty deck or no criteria has nothing to check.
  const checked = format.id !== 0 && deck.length > 0 && format.criteria.length > 0;
  const errors = checked ? criteriaErrors(format, deck) : [];
  return {
    formatId: format.id,
    name: format.name,
    errors,
    unknown,
    invalidIndexes: checked ? invalidIndexes(deck, errors) : [],
    legal: unknown.length > 0 ? null : errors.length === 0,
  };
}
