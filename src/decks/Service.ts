// The deck service: answers the userscript's Collection Pro panel (and, later, our own deck
// builder page) with deck reports computed from the captured site data.
//
//   deno task decks        # http://127.0.0.1:8788
//
// It only reads the data files the log server writes (see scripts/DeckCapture.ts), and it
// never talks to the site: nothing here can change the owner's account. Kept out of
// log_server.ts on purpose - that process has to keep up with the game client's polling.
import "colors";
import { deckReport } from "./Report.ts";
import type { DeckCard, DeckFormatData, OwnedCopies, SiteCard, SiteDeck } from "./SiteData.ts";

const PORT = 8788;
const SITE_ORIGIN = "https://www.urban-rivals.com";
const cors = {
  "Access-Control-Allow-Origin": SITE_ORIGIN,
  "Access-Control-Allow-Headers": "content-type",
  "Vary": "Origin",
};

const FILES = {
  formats: "data/deck_formats.json",
  cards: "data/site_cards.json",
  collection: "data/my_collection.json",
  decks: "data/my_decks.json",
};

// deno-lint-ignore no-explicit-any
type Json = any;

/** Re-reads a data file only when the log server has rewritten it. */
const cache = new Map<string, { mtime: number; value: Json }>();
async function readData(path: string): Promise<Json> {
  try {
    const mtime = (await Deno.stat(path)).mtime?.getTime() ?? 0;
    const hit = cache.get(path);
    if (hit && hit.mtime === mtime) return hit.value;
    const value = JSON.parse(await Deno.readTextFile(path));
    cache.set(path, { mtime, value });
    return value;
  } catch {
    return undefined;
  }
}

const indexes = new WeakMap<object, Map<number, SiteCard>>();
async function catalog() {
  const [formats, cards, collection] = await Promise.all([
    readData(FILES.formats),
    readData(FILES.cards),
    readData(FILES.collection),
  ]);
  let byId = cards && indexes.get(cards);
  if (cards && !byId) {
    byId = new Map((cards.cards as SiteCard[]).map((c) => [c.id, c]));
    indexes.set(cards, byId);
  }
  return {
    cards: byId ?? new Map<number, SiteCard>(),
    formats: (formats?.formats ?? []) as DeckFormatData[],
    owned: collection
      ? new Map(Object.entries(collection.cards as Record<string, OwnedCopies>).map(([id, o]) => [Number(id), o]))
      : undefined,
    fetchedAt: { formats: formats?.fetchedAt, cards: cards?.fetchedAt, collection: collection?.fetchedAt },
  };
}

function validDeck(characters: unknown): DeckCard[] | null {
  if (!Array.isArray(characters) || characters.length > 30) return null;
  const deck: DeckCard[] = [];
  for (const c of characters) {
    const id = Number(c?.id), level = Number(c?.level);
    if (!Number.isInteger(id) || !Number.isInteger(level) || level < 1 || level > 5) return null;
    deck.push({ id, level, state: typeof c?.state === "string" ? c.state : "" });
  }
  return deck;
}

const json = (value: unknown, status = 200) => Response.json(value, { status, headers: cors });

export async function handle(r: Request): Promise<Response> {
  const origin = r.headers.get("origin");
  if (origin !== null && origin !== SITE_ORIGIN) return new Response(null, { status: 403 });
  if (r.method === "OPTIONS") return new Response(null, { status: 204, headers: cors });
  const path = new URL(r.url).pathname;

  if (r.method === "GET" && path === "/api/status") {
    const [c, decks] = await Promise.all([catalog(), readData(FILES.decks)]);
    return json({
      cards: c.cards.size,
      formats: c.formats.map((f) => ({ id: f.id, name: f.name })),
      owned: c.owned?.size ?? null,
      decks: decks?.decks?.length ?? null,
      fetchedAt: { ...c.fetchedAt, decks: decks?.fetchedAt },
    });
  }
  if (r.method === "GET" && path === "/api/formats") return json((await catalog()).formats);
  if (r.method === "GET" && path === "/api/decks") return json((await readData(FILES.decks)) ?? { decks: [] });
  const deckId = /^\/api\/deck\/(\d+)$/.exec(path);
  if (r.method === "GET" && deckId) {
    const deck = ((await readData(FILES.decks))?.decks as SiteDeck[] | undefined)?.find((d) => d.id === Number(deckId[1]));
    if (!deck) return json({ error: "no such deck captured; open it in Collection Pro" }, 404);
    return json({ deck, report: deckReport(deck.characters, await catalog()) });
  }
  const cardId = /^\/api\/card\/(\d+)$/.exec(path);
  if (r.method === "GET" && cardId) {
    const c = await catalog();
    const card = c.cards.get(Number(cardId[1]));
    if (!card) return json({ error: "no such card captured" }, 404);
    return json({ card, owned: c.owned?.get(card.id) ?? {} });
  }
  if (r.method === "POST" && path === "/api/report") {
    let body: Json;
    try {
      body = await r.json();
    } catch {
      return json({ error: "expected a JSON body" }, 400);
    }
    const deck = validDeck(body?.characters);
    if (!deck) return json({ error: "characters must be up to 30 {id, level 1-5, state}" }, 400);
    return json(deckReport(deck, await catalog(), body?.night === true));
  }
  return json({ error: "not found" }, 404);
}

if (import.meta.main) {
  Deno.serve({
    hostname: "127.0.0.1",
    port: PORT,
    onListen: () => console.log(`deck service on http://127.0.0.1:${PORT} (reads ${Object.values(FILES).join(", ")})`.green),
  }, handle);
}
