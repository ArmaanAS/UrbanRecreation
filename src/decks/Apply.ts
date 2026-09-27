// Guarded apply, the deck service's half (docs/deck-builder-design.md, phase 4). Deck Lab
// asks for a draft to be saved to the site; this checks it against the captured collection
// and decks and describes what it would change. The service keeps it as the one pending
// write, which the userscript's UR Lab panel on Collection Pro shows. Only the owner's click
// there saves it, through the site's own `savedeck`; the panel then reports what the site
// answered, and every write that reached the site goes into data/deck_history.jsonl, the
// undo log. Nothing here talks to the site: Deck Lab cannot, because the site's session is a
// cookie that only a page on www.urban-rivals.com carries.
import { type DeckCatalog, deckReport } from "./Report.ts";
import { type DeckCard, type SiteDeck, siteDeck } from "./SiteData.ts";

/** The site's `general.config.maxNameLengthDeck`. */
export const MAX_DECK_NAME = 32;
export const MIN_CARDS = 4;
export const MAX_CARDS = 30;
/** `maxDecks` from `general.initPlayer` on 2026-09-23, for when no capture has said. */
const DECK_SLOTS_SEEN = 21;

/** The site's names for a copy's edition (`state`), from Collection Pro's edition filter. */
const EDITIONS: Record<string, string> = {
  "": "classic",
  p: "Prismatic",
  s: "Savage",
  ga: "Golden Aura",
  gs: "Golden Savage",
  m1: "Sapphire Deep Blue",
  a1: "Andromeda",
  k1: "Knight Blaze",
  v1: "Void",
  c1: "Cold",
  g1: "Glam",
  ar1: "Arcade",
  m2: "Frost Platinum",
  m3: "Wonder Spectrum",
  rp: "Rampage",
  i: "Immortal Legacy",
};
const edition = (state: string) => EDITIONS[state] ?? state;

/**
 * pending: waiting for the owner in the panel. applying: the owner clicked Apply and the panel
 * claimed it, so nothing else may take or withdraw it. Then applied (saved and verified),
 * failed, dismissed in the panel, or discarded (withdrawn or replaced in Deck Lab).
 */
export type ApplyState = "pending" | "applying" | "applied" | "failed" | "dismissed" | "discarded";

export interface ApplySummary {
  /** One sentence: "Will overwrite "T2 Skeelz*" (8 cards, 24★): 8 cards replaced by ...". */
  headline: string;
  /** What changes, one fact per line. */
  lines: string[];
  /** What the owner should know before confirming; none of it blocks the request. */
  warnings: string[];
  /** The cards to be saved, in the request's order, with their names. */
  cards: (DeckCard & { name: string })[];
}

/** What Deck Lab asks for, checked, before it is queued. */
export interface ApplyDraft {
  /** 0 creates a new deck (`savedeck id=0`). */
  deckId: number;
  name: string;
  setCurrent: boolean;
  characters: DeckCard[];
  /** The deck being overwritten as last captured, or null for a new deck. */
  before: SiteDeck | null;
  summary: ApplySummary;
}

export interface ApplyResult {
  /** The panel posted `savedeck`, so the site may have changed even when something failed. */
  sent: boolean;
  /** The saved deck as the site's `loaddeck` returned it afterwards. */
  deck: SiteDeck | null;
  /** The site's deck holds exactly the requested cards, order aside: checked here. */
  verified: boolean;
  siteResponse?: unknown;
  error?: string;
}

export interface ApplyRequest extends ApplyDraft {
  id: string;
  createdAt: string;
  state: ApplyState;
  claimedAt?: string;
  finishedAt?: string;
  result?: ApplyResult;
}

/** One line of data/deck_history.jsonl: re-saving `before` undoes the write. */
export interface HistoryEntry {
  t: string;
  requestId: string;
  deckId: number;
  name: string;
  before: SiteDeck | null;
  after: SiteDeck | null;
  verified: boolean;
  siteResponse: unknown;
  error?: string;
}

export interface ApplyContext extends DeckCatalog {
  /** The owner's decks as captured (data/my_decks.json). */
  decks: readonly SiteDeck[];
  maxDecks?: number;
}

// deno-lint-ignore no-explicit-any
type Json = any;

const cardKeys = (cards: readonly DeckCard[]) => cards.map((c) => `${c.id}:${c.level}:${c.state}`).sort();
/** The same cards at the same levels and editions, in any order: the site reorders a saved deck. */
export function sameCards(a: readonly DeckCard[], b: readonly DeckCard[]): boolean {
  const x = cardKeys(a), y = cardKeys(b);
  return x.length === y.length && x.every((k, i) => k === y[i]);
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;
const stars = (cards: readonly DeckCard[]) => cards.reduce((sum, c) => sum + c.level, 0);

/** Checks a request from Deck Lab and describes it, or says everything that is wrong with it. */
export function prepareApply(body: Json, ctx: ApplyContext): { draft: ApplyDraft } | { errors: string[] } {
  const errors: string[] = [];
  const characters: DeckCard[] = [];
  const raw = body?.characters;
  if (!Array.isArray(raw) || raw.length < MIN_CARDS || raw.length > MAX_CARDS) {
    return { errors: [`a deck is ${MIN_CARDS} to ${MAX_CARDS} cards`] };
  }
  for (const c of raw) {
    const id = Number(c?.id), level = Number(c?.level);
    const state = c?.state === undefined ? "" : c.state;
    if (!Number.isInteger(id) || id <= 0 || !Number.isInteger(level) || level < 1 || level > 5) {
      return { errors: ["every card needs an id and a level from 1 to 5"] };
    }
    if (typeof state !== "string" || !/^[a-z0-9]{0,4}$/.test(state)) return { errors: [`card ${id} has a malformed edition`] };
    characters.push({ id, level, state });
  }
  const nameOf = (c: DeckCard) => ctx.cards.get(c.id)?.name ?? `#${c.id}`;
  const label = (c: DeckCard) => `${nameOf(c)} L${c.level}${c.state ? ` (${edition(c.state)})` : ""}`;

  const seen = new Set<number>();
  for (const c of characters) {
    if (seen.has(c.id)) errors.push(`${nameOf(c)} is in the deck twice`);
    seen.add(c.id);
  }

  const name = typeof body?.name === "string" ? body.name.trim() : "";
  if (!name || [...name].length > MAX_DECK_NAME) errors.push(`the name must be 1 to ${MAX_DECK_NAME} characters`);

  let before: SiteDeck | null = null;
  const target = body?.target;
  if (target === "new") before = null;
  else if (Number.isInteger(target?.deckId) && target.deckId > 0) {
    before = ctx.decks.find((d) => d.id === target.deckId) ?? null;
    if (!before) errors.push("that deck has not been captured: open it in Collection Pro first");
  } else errors.push('target must be "new" or {deckId}');

  if (body?.setCurrent !== undefined && typeof body.setCurrent !== "boolean") errors.push("setCurrent must be true or false");
  // Collection Pro's own Save sends the loaded deck's current flag, so saving the current deck
  // keeps it current. `set_current=false` on it is a request nobody has seen answered.
  const setCurrent = body?.setCurrent === true || before?.isCurrent === true;

  // Owned means owned at that level in that edition: the site has no per-copy identity to
  // substitute, and a deck may not name a copy the account lacks.
  if (!ctx.owned) errors.push("no collection has been captured yet: open Collection Pro so the log server records it");
  else {
    for (const c of characters) {
      const byLevel = ctx.owned.get(c.id) ?? {};
      if ((byLevel[String(c.level)]?.[c.state] ?? 0) > 0) continue;
      const others = Object.entries(byLevel).flatMap(([level, byState]) =>
        Object.entries(byState).filter(([, n]) => n > 0).map(([state]) => `L${level} ${edition(state)}`)
      );
      errors.push(`you do not own ${label(c)}${others.length ? `; you have ${others.join(", ")}` : ""}`);
    }
  }
  if (errors.length) return { errors };

  if (before && sameCards(before.characters, characters) && before.name === name && before.isCurrent === setCurrent) {
    return { errors: [`"${before.name}" already is exactly this deck`] };
  }

  const lines: string[] = [];
  const warnings: string[] = [];
  let headline: string;
  if (!before) {
    headline = `Will create a new deck "${name}" with ${plural(characters.length, "card")} (${stars(characters)}★)` +
      (setCurrent ? " and make it your current deck." : ".");
    lines.push(`Cards: ${characters.map(label).join(", ")}`);
    const slots = ctx.maxDecks ?? DECK_SLOTS_SEEN;
    if (ctx.decks.length >= slots) {
      warnings.push(
        `You already have ${ctx.decks.length} decks and the site allowed ${slots}${ctx.maxDecks ? "" : " on 2026-09-23"}: ` +
          "it may refuse a new one. Overwriting a deck you no longer use is the safer choice.",
      );
    }
  } else {
    const old = new Map(before.characters.map((c) => [c.id, c]));
    const next = new Map(characters.map((c) => [c.id, c]));
    const added = characters.filter((c) => !old.has(c.id));
    const removed = before.characters.filter((c) => !next.has(c.id));
    const changed = characters.filter((c) => old.has(c.id) && (old.get(c.id)!.level !== c.level || old.get(c.id)!.state !== c.state));
    const kept = characters.length - added.length - changed.length;
    const parts: string[] = [];
    if (removed.length && removed.length === added.length) {
      parts.push(`${plural(removed.length, "card")} replaced by ${added.map(label).join(", ")}`);
    } else {
      if (removed.length) parts.push(`${plural(removed.length, "card")} out`);
      if (added.length) parts.push(`${plural(added.length, "card")} in (${added.map(label).join(", ")})`);
    }
    if (changed.length) parts.push(`${changed.length} re-levelled`);
    if (!parts.length) parts.push("same cards");
    headline = `Will overwrite "${before.name}" (${plural(before.characters.length, "card")}, ${stars(before.characters)}★): ` +
      `${parts.join(", ")}.`;
    if (removed.length) lines.push(`Out: ${removed.map(label).join(", ")}`);
    if (added.length) lines.push(`In: ${added.map(label).join(", ")}`);
    if (changed.length) lines.push(`Changed: ${changed.map((c) => `${label(old.get(c.id)!)} → ${label(c)}`).join(", ")}`);
    if (kept) lines.push(`Unchanged: ${plural(kept, "card")}`);
    lines.push(`Stars: ${stars(before.characters)} → ${stars(characters)}`);
    if (name !== before.name) lines.push(`Renamed to "${name}"`);
    if (before.isCurrent) lines.push("It is your current deck and stays current.");
    else if (setCurrent) lines.push("It becomes your current deck.");
    warnings.push(`The cards now in "${before.name}" are kept in data/deck_history.jsonl, so the overwrite can be undone.`);
  }

  const report = deckReport(characters, ctx);
  const legal = report.formats.filter((f) => f.legal === true).map((f) => f.name);
  const illegal = report.formats.filter((f) => f.legal === false).map((f) => f.name);
  const unknown = report.formats.filter((f) => f.legal === null).map((f) => f.name);
  lines.push(
    [legal.length && `Legal in ${legal.join(", ")}`, illegal.length && `not legal in ${illegal.join(", ")}`, unknown.length && `cannot tell for ${unknown.join(", ")}`]
      .filter(Boolean).join("; ") || "No format captured to check it against",
  );
  if (!legal.length) warnings.push("It is legal in no captured format, so only rooms without a format can use it.");

  return {
    draft: {
      deckId: before?.id ?? 0,
      name,
      setCurrent,
      characters,
      before,
      summary: { headline, lines, warnings, cards: characters.map((c) => ({ ...c, name: nameOf(c) })) },
    },
  };
}

/** A site answer kept in the undo log: whole unless absurdly large (an HTML error page). */
function keepable(response: unknown): unknown {
  const text = typeof response === "string" ? response : JSON.stringify(response ?? null);
  return text.length > 20_000 ? `[${text.length} chars: ${text.slice(0, 2_000)}…]` : response;
}

/**
 * Folds what the panel reports into the request. Returns an error for a malformed report or
 * one for a request that already has an outcome; otherwise the history line to append when
 * the site was asked to save (null when nothing was sent).
 */
export function recordResult(request: ApplyRequest, body: Json): { error: string } | { history: HistoryEntry | null } {
  if (request.result || request.state === "dismissed") return { error: "this request already has an outcome" };
  if (!body || typeof body !== "object") return { error: "expected {ok, sent, deck, siteResponse} or {dismissed: true}" };
  const now = new Date().toISOString();
  if (body.dismissed === true) {
    // A request Deck Lab withdrew stays withdrawn; either way nothing reached the site.
    if (request.state === "pending" || request.state === "applying") request.state = "dismissed";
    request.finishedAt = now;
    return { history: null };
  }
  if (typeof body.sent !== "boolean") return { error: "sent must be true or false" };
  const deck = body.deck && typeof body.deck === "object" ? siteDeck(body.deck) : null;
  // The panel's own verdict is not taken on trust: the site's cards must equal the request's.
  const verified = body.sent && deck !== null && deck.id > 0 && (request.deckId === 0 || deck.id === request.deckId) &&
    sameCards(deck.characters, request.characters);
  const error = verified ? undefined : typeof body.error === "string" && body.error
    ? body.error.slice(0, 500)
    : deck
    ? "the saved deck does not hold the requested cards"
    : "the site's deck could not be read back";
  request.result = {
    sent: body.sent,
    deck,
    verified,
    ...(body.siteResponse !== undefined ? { siteResponse: keepable(body.siteResponse) } : {}),
    ...(error ? { error } : {}),
  };
  // Withdrawn in Deck Lab and then refused by the panel: still nothing sent, still withdrawn.
  if (verified || body.sent || request.state !== "discarded") request.state = verified ? "applied" : "failed";
  request.finishedAt = now;
  if (!body.sent) return { history: null };
  const saved = Number(body.siteResponse?.deck?.id);
  return {
    history: {
      t: now,
      requestId: request.id,
      deckId: deck?.id || (Number.isInteger(saved) && saved > 0 ? saved : request.deckId),
      name: request.name,
      before: request.before,
      after: deck,
      verified,
      siteResponse: keepable(body.siteResponse ?? null),
      ...(error ? { error } : {}),
    },
  };
}
