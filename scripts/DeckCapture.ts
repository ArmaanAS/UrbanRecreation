// Turns the collection and deck traffic the userscript mirrors into the deck builder's data
// files, the way scripts/BattleCapture.ts does for battles. The log server feeds it every
// record as it arrives; `deno task deck-data` feeds it an existing raw log.
//
//   data/deck_formats.json   committed  every format's criteria, from action=deckformatsdata
//   data/site_cards.json     gitignored every character at every level, from collectiondata
//   data/my_collection.json  gitignored the owner's copies per level and edition
//   data/my_decks.json       gitignored the owner's decks, from loaddeck/savedeck and
//                                       the game client's collections.decks, plus which of
//                                       them the server itself calls legal in each format
//
// Nothing personal beyond card ids, levels, editions, copy counts and deck names is kept:
// market offers, acquisition times and every auth field are dropped by the parsers.
import {
  deckFormat,
  type DeckFormatData,
  ownedCopies,
  type OwnedCopies,
  siteCard,
  type SiteCard,
  siteDeck,
  type SiteDeck,
} from "@/decks/SiteData.ts";

export const DECK_FILES = {
  formats: "data/deck_formats.json",
  cards: "data/site_cards.json",
  collection: "data/my_collection.json",
  decks: "data/my_decks.json",
} as const;

export interface DeckStore {
  formats?: { fetchedAt: string; formats: DeckFormatData[] };
  cards: Map<number, SiteCard>;
  cardsAt?: string;
  owned: Map<number, OwnedCopies>;
  ownedAt?: string;
  decks: Map<number, SiteDeck>;
  decksAt?: string;
  maxDecks?: number;
  /**
   * The game client asks `collections.decks {deckFormatID}` for a room's decks, and the
   * server answers with only the decks legal there (Tourney 7 of 19 on 2026-09-26). That
   * is the server's own verdict, the oracle for src/decks/DeckFormat.ts.
   */
  legalByFormat: Map<number, { at: string; deckIds: number[] }>;
  /** Which files changed since the last save. */
  dirty: Set<keyof typeof DECK_FILES>;
}

export function newDeckStore(): DeckStore {
  return { cards: new Map(), owned: new Map(), decks: new Map(), legalByFormat: new Map(), dirty: new Set() };
}

// deno-lint-ignore no-explicit-any
type Json = any;

const iso = (t: number) => new Date(t).toISOString();

/** The `action=` of a Collection Pro form post, and its other fields. */
function formFields(body: unknown): URLSearchParams | null {
  if (typeof body !== "string" || !body.includes("action=")) return null;
  try {
    return new URLSearchParams(body);
  } catch {
    return null;
  }
}

function parse(resp: unknown): Json {
  if (typeof resp !== "string") return undefined;
  try {
    return JSON.parse(resp);
  } catch {
    return undefined;
  }
}

/** Describes a record as collection traffic, or returns null if it is anything else. */
export function collectionAction(rec: Json): { action: string; form: URLSearchParams } | null {
  const p = rec?.payload;
  if (!p || typeof p !== "object" || !String(p.u ?? "").includes("/ajax/collection")) return null;
  const form = formFields(p.body);
  const action = form?.get("action");
  return action ? { action, form: form! } : null;
}

/**
 * Folds one raw log record into the store. Returns true when it was collection or deck
 * traffic the store used.
 */
export function absorbRecord(rec: Json, store: DeckStore): boolean {
  const t = Number(rec?.t) || Date.now();
  const collection = collectionAction(rec);
  if (collection) {
    const resp = parse(rec.payload.resp);
    if (resp === undefined) return false;
    switch (collection.action) {
      case "collectiondata": {
        if (!Array.isArray(resp)) return false;
        for (const row of resp) {
          const card = siteCard(row);
          store.cards.set(card.id, card);
          const owned = ownedCopies(row.collectionData);
          if (Object.keys(owned).length) store.owned.set(card.id, owned);
          else store.owned.delete(card.id);
        }
        store.cardsAt = store.ownedAt = iso(t);
        store.dirty.add("cards");
        store.dirty.add("collection");
        return true;
      }
      case "deckformatsdata": {
        if (!Array.isArray(resp)) return false;
        store.formats = { fetchedAt: iso(t), formats: resp.map(deckFormat) };
        store.dirty.add("formats");
        return true;
      }
      case "loaddeck":
      case "savedeck": {
        if (!resp?.deck) return false;
        const deck = siteDeck(resp.deck);
        if (deck.isCurrent) for (const other of store.decks.values()) other.isCurrent = false;
        store.decks.set(deck.id, deck);
        store.decksAt = iso(t);
        store.dirty.add("decks");
        return true;
      }
      case "deletedeck": {
        const id = Number(collection.form.get("id"));
        if (!resp?.success || !store.decks.delete(id)) return false;
        store.decksAt = iso(t);
        store.dirty.add("decks");
        return true;
      }
      case "setcurrentdeck": {
        const id = Number(collection.form.get("id"));
        if (!resp?.success || !store.decks.has(id)) return false;
        for (const deck of store.decks.values()) deck.isCurrent = deck.id === id;
        store.decksAt = iso(t);
        store.dirty.add("decks");
        return true;
      }
      default:
        return false;
    }
  }

  // The game client lists decks through the private API: all of them without a format,
  // or only the ones the server finds legal in the format it names.
  const p = rec?.payload;
  if (p && typeof p === "object" && String(p.u ?? "").includes("/api/private/v2/")) {
    const resp = parse(p.resp);
    const data = resp?.["collections.decks"]?.data;
    if (!data) return false;
    const decks = Array.isArray(data) ? data : data.decks;
    if (!Array.isArray(decks)) return false;
    const formatId = Number(privateApiParams(p.body, "collections.decks")?.deckFormatID ?? 0);
    if (formatId !== 0) {
      store.legalByFormat.set(formatId, { at: iso(t), deckIds: decks.map((d: Json) => Number(d.id)).sort((a, b) => a - b) });
    } else {
      store.decks = new Map(decks.map((d: Json) => [Number(d.id), siteDeck(d)]));
      if (typeof data.maxDecks === "number") store.maxDecks = data.maxDecks;
      store.decksAt = iso(t);
    }
    store.dirty.add("decks");
    return true;
  }
  return false;
}

/** The params of `call` in a private-API body (`requests=` + URL-encoded JSON array). */
function privateApiParams(body: unknown, call: string): Json {
  if (typeof body !== "string" || !body.startsWith("requests=")) return undefined;
  try {
    const requests = JSON.parse(decodeURIComponent(body.slice("requests=".length)));
    return (requests as Json[]).find((r) => r?.call === call)?.params ?? {};
  } catch {
    return undefined;
  }
}

/** A short line standing in for a collectiondata body in the raw log (2.7 MB per page). */
export function rawLogStandIn(rec: Json): string | null {
  const collection = collectionAction(rec);
  if (collection?.action !== "collectiondata") return null;
  const rows = parse(rec.payload.resp);
  if (!Array.isArray(rows)) return null;
  return `[collectiondata page ${collection.form.get("page")}: ${rows.length} rows, kept in ` +
    `${DECK_FILES.cards} and ${DECK_FILES.collection}]`;
}

export async function loadDeckStore(): Promise<DeckStore> {
  const store = newDeckStore();
  // Only a missing file means "nothing captured yet". Anything else - a permission the
  // process lacks, a half-written file - must stop here: starting empty would overwrite the
  // owner's captured decks with whatever the next record holds.
  const read = async (path: string): Promise<Json> => {
    try {
      return JSON.parse(await Deno.readTextFile(path));
    } catch (e) {
      if (e instanceof Deno.errors.NotFound) return undefined;
      throw new Error(`cannot load ${path}: ${(e as Error).message}`);
    }
  };
  const formats = await read(DECK_FILES.formats);
  if (formats?.formats) store.formats = formats;
  const cards = await read(DECK_FILES.cards);
  if (cards?.cards) {
    store.cardsAt = cards.fetchedAt;
    for (const c of cards.cards as SiteCard[]) store.cards.set(c.id, c);
  }
  const collection = await read(DECK_FILES.collection);
  if (collection?.cards) {
    store.ownedAt = collection.fetchedAt;
    for (const [id, owned] of Object.entries(collection.cards)) store.owned.set(Number(id), owned as OwnedCopies);
  }
  const decks = await read(DECK_FILES.decks);
  if (decks?.decks) {
    store.decksAt = decks.fetchedAt;
    store.maxDecks = decks.maxDecks;
    for (const d of decks.decks as SiteDeck[]) store.decks.set(d.id, d);
    for (const [id, legal] of Object.entries(decks.legalByFormat ?? {})) {
      store.legalByFormat.set(Number(id), legal as { at: string; deckIds: number[] });
    }
  }
  return store;
}

/** Writes the files that changed. The card list is sorted by id, so diffs stay readable. */
export async function saveDeckStore(store: DeckStore): Promise<string[]> {
  const written: string[] = [];
  const write = async (key: keyof typeof DECK_FILES, value: unknown, indent?: number) => {
    await Deno.writeTextFile(DECK_FILES[key], JSON.stringify(value, null, indent) + "\n");
    written.push(DECK_FILES[key]);
  };
  const byId = <T extends { id: number }>(m: Map<number, T>) => [...m.values()].sort((a, b) => a.id - b.id);
  if (store.dirty.has("formats") && store.formats) await write("formats", store.formats, 1);
  if (store.dirty.has("cards")) await write("cards", { fetchedAt: store.cardsAt, cards: byId(store.cards) });
  if (store.dirty.has("collection")) {
    const cards = Object.fromEntries([...store.owned.entries()].sort((a, b) => a[0] - b[0]));
    await write("collection", { fetchedAt: store.ownedAt, cards }, 1);
  }
  if (store.dirty.has("decks")) {
    const legalByFormat = Object.fromEntries([...store.legalByFormat.entries()].sort((a, b) => a[0] - b[0]));
    await write("decks", { fetchedAt: store.decksAt, maxDecks: store.maxDecks, decks: byId(store.decks), legalByFormat }, 1);
  }
  store.dirty.clear();
  return written;
}
