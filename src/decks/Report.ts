// Everything the deck panel shows about one deck: every card's full details at the level
// played, the star count, clan counts and how often each card's clan bonus is live, and a
// verdict for every format. Pure: the caller supplies the site's cards, formats and (if it
// has them) the owner's copies.
import { checkDeck, type DeckCharacter, type FormatVerdict, LEADER_CLAN_ID } from "./DeckFormat.ts";
import type { DeckCard, DeckFormatData, OwnedCopies, SiteAbility, SiteCard } from "./SiteData.ts";

export interface DeckCatalog {
  cards: ReadonlyMap<number, SiteCard>;
  formats: readonly DeckFormatData[];
  /** The owner's copies by card id; absent when no collection has been captured yet. */
  owned?: ReadonlyMap<number, OwnedCopies>;
}

export interface CardDetail extends DeckCard {
  /** False when the site's card list has no such card, or no such level of it. */
  known: boolean;
  name?: string;
  clan?: string;
  clanId?: number;
  rarity?: string;
  levelMin?: number;
  levelMax?: number;
  power?: number;
  damage?: number;
  /** The ability in force (the night variant at night); a locked one reads "No Ability". */
  ability?: string;
  abilityLocked?: boolean;
  abilityUnlockLevel?: number;
  bonus?: string;
  bans?: {
    tourney: boolean;
    tourneyMaxLevel: boolean;
    elo: boolean;
    efcMaxLevel: boolean;
    efcTemporary: boolean;
  };
  /** Copies owned at this level, every edition, and at this exact edition. */
  ownedAtLevel?: number;
  ownedExact?: number;
  /**
   * Share of this deck's four-card hands holding this card in which its clan bonus is
   * live, i.e. at least one other card of its clan was drawn too. Leaders have none.
   */
  bonusLiveShare?: number;
}

export interface DeckReport {
  cards: CardDetail[];
  stars: number;
  clans: { clanId: number; clan: string; count: number }[];
  leaders: number;
  formats: FormatVerdict[];
  /** Whether the owner owns every card at its level and edition; null without a collection. */
  allOwned: boolean | null;
  notes: string[];
}

const text = (a: SiteAbility | []): string | undefined => Array.isArray(a) ? undefined : a.description;

/** n choose k, exact for the small numbers a deck produces. */
function choose(n: number, k: number): number {
  if (k < 0 || k > n) return 0;
  let r = 1;
  for (let i = 1; i <= k; i++) r = (r * (n - k + i)) / i;
  return r;
}

export function deckReport(characters: DeckCard[], catalog: DeckCatalog, night = false): DeckReport {
  const notes: string[] = [];
  const details: CardDetail[] = characters.map((c) => {
    const card = catalog.cards.get(c.id);
    const evo = card?.evos[String(c.level)];
    if (!card || !evo) return { ...c, known: false };
    const nightAbility = night ? evo.nightAbility : [];
    const live = Array.isArray(nightAbility) ? evo.ability : nightAbility;
    const unlock = live.unlockLevel ?? 0;
    const locked = live.id === 0 || unlock > c.level;
    const owned = catalog.owned?.get(c.id)?.[String(c.level)];
    return {
      ...c,
      known: true,
      name: card.name,
      clan: card.clan_name,
      clanId: card.clan_id,
      rarity: card.rarity,
      levelMin: card.level_min,
      levelMax: card.level_max,
      power: evo.power,
      damage: evo.damage,
      ability: locked ? "No Ability" : live.description,
      abilityLocked: locked,
      ...(unlock ? { abilityUnlockLevel: unlock } : {}),
      bonus: (night ? text(card.nightBonus) : undefined) ?? card.bonus.description,
      bans: {
        tourney: card.tourney_banned,
        tourneyMaxLevel: card.tourney_max_evo_banned,
        elo: card.efc_banned,
        efcMaxLevel: card.efc_max_evo_banned,
        efcTemporary: card.efc_temp_banned,
      },
      ...(catalog.owned
        ? {
          ownedAtLevel: Object.values(owned ?? {}).reduce((a, b) => a + b, 0),
          ownedExact: owned?.[c.state] ?? 0,
        }
        : {}),
    };
  });

  const unknownCards = details.filter((d) => !d.known);
  if (unknownCards.length) {
    notes.push(
      `${unknownCards.length} card(s) are not in the captured card list at the level played ` +
        `(${unknownCards.map((d) => `#${d.id} L${d.level}`).join(", ")}): open Collection Pro to refresh it.`,
    );
  }

  const clanCounts = new Map<number, { clan: string; count: number }>();
  let leaders = 0;
  for (const d of details) {
    if (!d.known) continue;
    if (d.clanId === LEADER_CLAN_ID) leaders++;
    else {
      const entry = clanCounts.get(d.clanId!) ?? { clan: d.clan!, count: 0 };
      entry.count++;
      clanCounts.set(d.clanId!, entry);
    }
  }
  const n = details.length;
  for (const d of details) {
    if (!d.known || d.clanId === LEADER_CLAN_ID || n < 4) continue;
    const mates = details.filter((o) => o !== d && o.known && o.clanId === d.clanId && o.id !== d.id).length;
    d.bonusLiveShare = 1 - choose(n - 1 - mates, 3) / choose(n - 1, 3);
  }
  if (details.some((d) => d.clan === "Oculus")) {
    notes.push("Oculus infiltration is not modelled in the bonus shares: an Oculus card joins a clan only in some hands.");
  }

  const deck: DeckCharacter[] = details.filter((d) => d.known).map((d) => {
    const card = catalog.cards.get(d.id)!;
    const evo = card.evos[String(d.level)];
    const live = night && !Array.isArray(evo.nightAbility) ? evo.nightAbility : evo.ability;
    return {
      id: d.id,
      level: d.level,
      level_max: card.level_max,
      rarity: card.rarity,
      release_date: card.release_date,
      clan_id: card.clan_id,
      efc_banned: card.efc_banned,
      abilityTypeID: live.typeID,
    };
  });
  const formats = catalog.formats.map((f) => {
    const verdict = checkDeck(f, deck);
    // A card the validator could not see may break a rule it would otherwise pass.
    return unknownCards.length && verdict.legal ? { ...verdict, legal: null } : verdict;
  });

  return {
    cards: details,
    stars: details.reduce((sum, d) => sum + d.level, 0),
    clans: [...clanCounts.entries()].map(([clanId, v]) => ({ clanId, ...v })).sort((a, b) => b.count - a.count),
    leaders,
    formats,
    allOwned: catalog.owned ? details.every((d) => (d.ownedExact ?? 0) > 0) : null,
    notes,
  };
}
