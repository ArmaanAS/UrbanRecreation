// The deck service: answers the userscript's Collection Pro panel with deck reports computed
// from the captured site data, and serves our own deck builder page ("Deck Lab").
//
//   deno task decks        # http://127.0.0.1:8788 - open it in a browser for Deck Lab
//
// It only reads the data files the log server writes (see scripts/DeckCapture.ts), and it
// never talks to the site: nothing here can change the owner's account. Kept out of
// log_server.ts on purpose - that process has to keep up with the game client's polling.
// Deck scoring runs the release `urban-recreation-matchup` binary (`deno task rust:matchup`)
// and caches its solves under cache/matchups/, like `deno task matchup`.
import "colors";
import { readRustV1Provenance } from "../solver/RustProvenance.ts";
import { compactCoverage, type CoverageFile, describeRefusal } from "./Coverage.ts";
import {
  assertCurrentBinary,
  deckVsDeck,
  type DeckVsDeckOptions,
  type DeckVsDeckResult,
  deckVsHands,
  FileMatchupCache,
  type MatchupCache,
  matchupProvenance,
  type MatchupRunner,
  ProcessMatchupRunner,
  provenanceMismatches,
} from "./Matchup.ts";
import { formatHands, formatMeta, type GameRecord } from "./Meta.ts";
import { deckReport } from "./Report.ts";
import type { DeckCard, DeckFormatData, OwnedCopies, SiteCard, SiteDeck, SiteEvo } from "./SiteData.ts";

const PORT = 8788;
/** The owner's player id, for the few old captures that do not say which side was theirs. */
const OWNER_ID = 19309601;
const SITE_ORIGIN = "https://www.urban-rivals.com";
const cors = {
  "Access-Control-Allow-Origin": SITE_ORIGIN,
  "Access-Control-Allow-Headers": "content-type",
  "Access-Control-Allow-Methods": "GET, POST, DELETE, OPTIONS",
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
  let byId: Map<number, SiteCard> | undefined = cards ? indexes.get(cards) : undefined;
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

/** The captured games, re-read only when a capture is added or rewritten. */
const GAMES_DIR = "captures/games";
let gamesCache: { key: string; games: GameRecord[] } | undefined;
async function games(): Promise<GameRecord[]> {
  const files: { name: string; mtime: number }[] = [];
  for await (const entry of Deno.readDir(GAMES_DIR)) {
    if (!entry.isFile || !entry.name.endsWith(".json")) continue;
    files.push({ name: entry.name, mtime: (await Deno.stat(`${GAMES_DIR}/${entry.name}`)).mtime?.getTime() ?? 0 });
  }
  const key = `${files.length}:${Math.max(0, ...files.map((f) => f.mtime))}`;
  if (gamesCache?.key === key) return gamesCache.games;
  const loaded = await Promise.all(files.map(async (f) => JSON.parse(await Deno.readTextFile(`${GAMES_DIR}/${f.name}`)) as GameRecord));
  gamesCache = { key, games: loaded };
  return loaded;
}

// Deck Lab's static files, served from src/decks/ui/.
const UI_DIR = new URL("./ui/", import.meta.url);
const UI_FILES: Record<string, string> = {
  "/": "index.html",
  "/app.js": "app.js",
  "/style.css": "style.css",
};
const UI_TYPES: Record<string, string> = {
  html: "text/html; charset=utf-8",
  js: "text/javascript; charset=utf-8",
  css: "text/css; charset=utf-8",
};

/**
 * Every card, compact, with the owner's copies: what Deck Lab's collection browser needs in
 * one request. Per level: [power, damage, ability, unlock level, night ability, picture].
 */
async function collection() {
  const c = await catalog();
  return {
    fetchedAt: c.fetchedAt,
    formats: c.formats,
    cards: [...c.cards.values()].map((card) => ({
      id: card.id,
      name: card.name,
      clan: card.clan_name,
      clanId: card.clan_id,
      rarity: card.rarity,
      levelMin: card.level_min,
      levelMax: card.level_max,
      release: card.release_date,
      bans: {
        tourney: card.tourney_banned,
        tourneyMaxLevel: card.tourney_max_evo_banned,
        elo: card.efc_banned,
        efcMaxLevel: card.efc_max_evo_banned,
        efcTemporary: card.efc_temp_banned,
      },
      bonus: card.bonus.description,
      nightBonus: Array.isArray(card.nightBonus) ? undefined : card.nightBonus.description,
      evos: Object.fromEntries(Object.entries(card.evos as Record<string, SiteEvo>).map(([level, evo]) => [level, [
        evo.power,
        evo.damage,
        evo.ability.description,
        evo.ability.unlockLevel ?? 0,
        Array.isArray(evo.nightAbility) ? null : evo.nightAbility.description,
        evo.pictureURL ?? null,
      ]])),
      owned: c.owned?.get(card.id) ?? {},
    })),
  };
}

// ---- which cards the exact solver can score (data/card_coverage.json) --------------------
const COVERAGE_FILE = "data/card_coverage.json";
let coverageView: { file: CoverageFile; view: Json } | undefined;
async function coverage(): Promise<Json> {
  const file = (await readData(COVERAGE_FILE)) as CoverageFile | undefined;
  if (!file) return { missing: true };
  if (coverageView?.file !== file) {
    let stale: string | null = null;
    try {
      const mismatches = provenanceMismatches(file.provenance, await readRustV1Provenance());
      if (mismatches.length) stale = `it was made from other engine or card data (${mismatches.join(", ")})`;
    } catch (error) {
      stale = `it cannot be checked against this checkout: ${(error as Error).message}`;
    }
    coverageView = { file, view: { ...compactCoverage(file), stale } };
  }
  return coverageView.view;
}

// ---- deck scoring on the exact Rust solver (src/decks/Matchup.ts) --------------------------
export interface Solver {
  runner: MatchupRunner;
  cache: MatchupCache;
}

const solveCaches = new Map<string, FileMatchupCache>();
/** Checks the release binary against this checkout on every job, so a rebuild is picked up. */
async function releaseSolver(): Promise<Solver> {
  const runner = new ProcessMatchupRunner();
  const provenance = await matchupProvenance(runner);
  await assertCurrentBinary(provenance);
  const key = JSON.stringify(provenance);
  let cache = solveCaches.get(key);
  if (!cache) solveCaches.set(key, cache = await FileMatchupCache.open("cache/matchups", provenance));
  return { runner, cache };
}
let solver: () => Promise<Solver> = releaseSolver;
/** For tests: score with another solver. */
export function useSolver(factory: () => Promise<Solver>) {
  solver = factory;
}

const MAX_PAIRS = 400;
/** One seed for every job, so two drafts scored against the same opponent meet the same hands. */
const SEED = 1;

interface MatchupJob {
  id: number;
  state: "running" | "done" | "failed" | "cancelled";
  /** The draft that was scored, as [id, level], and what it was scored against. */
  deck: [number, number][];
  against: string;
  night: boolean;
  done: number;
  total: number;
  startedAt: string;
  seconds?: number;
  result?: Json;
  error?: string;
}
let job: { view: MatchupJob; controller: AbortController } | undefined;
let jobCount = 0;

/** What Deck Lab shows of a result: the numbers, the worst pairs, and why pairs were refused. */
function summary(result: DeckVsDeckResult) {
  const refusals = new Map<string, { side?: string; card?: readonly [number, number]; why: string; count: number }>();
  for (const pair of result.refusedPairs) {
    const why = describeRefusal(pair.reason);
    const key = `${pair.side}|${pair.card?.join(":")}|${why}`;
    const entry = refusals.get(key) ?? { side: pair.side, card: pair.card, why, count: 0 };
    entry.count++;
    refusals.set(key, entry);
  }
  return {
    pairs: result.n,
    scored: result.scored,
    refused: result.refused,
    mean: result.mean,
    stderr: result.stderr,
    percent: result.percent,
    cached: result.cached,
    solved: result.solved,
    worst: result.worst,
    refusals: [...refusals.values()].sort((x, y) => y.count - x.count).slice(0, 8),
  };
}

async function startMatchup(body: Json): Promise<Response> {
  const deck = validDeck(body?.characters);
  if (!deck || deck.length < 4) return json({ error: "characters must be 4 to 30 {id, level 1-5, state}" }, 400);
  const n = body?.n === undefined ? 40 : Number(body.n);
  if (!Number.isInteger(n) || n < 1 || n > MAX_PAIRS) return json({ error: `n must be 1 to ${MAX_PAIRS} hand pairs` }, 400);
  const night = body?.night === true;
  const opponent = body?.opponent;
  let against: string;
  let run: (options: DeckVsDeckOptions) => Promise<DeckVsDeckResult>;
  if (Number.isInteger(opponent?.format)) {
    const hands = formatHands(await games(), opponent.format, OWNER_ID);
    if (!hands.length) return json({ error: "no opposing hands of that format have been captured" }, 400);
    const name = (await catalog()).formats.find((f) => f.id === opponent.format)?.name ?? `format ${opponent.format}`;
    against = `${Math.min(n, hands.length)} of the ${hands.length} captured ${name} opponents`;
    run = (options) => deckVsHands(deck, hands, options);
  } else if (Number.isInteger(opponent?.deck)) {
    const other = ((await readData(FILES.decks))?.decks as SiteDeck[] | undefined)?.find((d) => d.id === opponent.deck);
    if (!other) return json({ error: "no such deck captured; open it in Collection Pro" }, 404);
    if (other.characters.length < 4) return json({ error: `"${other.name}" has fewer than 4 cards` }, 400);
    against = `your deck "${other.name}"`;
    run = (options) => deckVsDeck(deck, other.characters, options);
  } else {
    return json({ error: "opponent must be {format: id} or {deck: id}" }, 400);
  }

  job?.controller.abort();
  const controller = new AbortController();
  const view: MatchupJob = {
    id: ++jobCount,
    state: "running",
    deck: deck.map((c) => [c.id, c.level]),
    against,
    night,
    done: 0,
    total: 0,
    startedAt: new Date().toISOString(),
  };
  job = { view, controller };
  const started = performance.now();
  (async () => {
    try {
      const { runner, cache } = await solver();
      controller.signal.throwIfAborted();
      const result = await run({
        runner,
        cache,
        n,
        seed: SEED,
        night,
        signal: controller.signal,
        onProgress: (done, total) => {
          view.done = done;
          view.total = total;
        },
      });
      view.result = summary(result);
      view.state = "done";
    } catch (error) {
      if (controller.signal.aborted) view.state = "cancelled";
      else {
        view.state = "failed";
        view.error = (error as Error).message;
      }
    } finally {
      view.seconds = (performance.now() - started) / 1000;
    }
  })();
  return json(view, 202);
}

// Deck Lab's own page, and the site (the userscript panel, via the log server's proxy or
// directly). Every other origin is refused, so no other page can read the owner's decks.
const OWN_ORIGINS = new Set([`http://127.0.0.1:${PORT}`, `http://localhost:${PORT}`]);

export async function handle(r: Request): Promise<Response> {
  const origin = r.headers.get("origin");
  if (origin !== null && origin !== SITE_ORIGIN && !OWN_ORIGINS.has(origin)) {
    return new Response(null, { status: 403 });
  }
  if (r.method === "OPTIONS") return new Response(null, { status: 204, headers: cors });
  const path = new URL(r.url).pathname;

  const uiFile = UI_FILES[path];
  if (r.method === "GET" && uiFile) {
    const body = await Deno.readFile(new URL(uiFile, UI_DIR));
    return new Response(body, {
      headers: { "content-type": UI_TYPES[uiFile.split(".").pop()!], "cache-control": "no-store" },
    });
  }
  if (r.method === "GET" && path === "/api/collection") return json(await collection());
  if (r.method === "GET" && path === "/api/coverage") return json(await coverage());
  if (path === "/api/matchup") {
    if (r.method === "GET") return json(job?.view ?? { state: "none" });
    if (r.method === "DELETE") {
      job?.controller.abort();
      return json(job?.view ?? { state: "none" });
    }
    if (r.method === "POST") {
      let body: Json;
      try {
        body = await r.json();
      } catch {
        return json({ error: "expected a JSON body" }, 400);
      }
      return startMatchup(body);
    }
  }
  if (r.method === "GET" && path === "/api/meta") {
    const formatId = Number(new URL(r.url).searchParams.get("format"));
    if (!Number.isInteger(formatId)) return json({ error: "format must be a deck format id" }, 400);
    return json(formatMeta(await games(), formatId, OWNER_ID));
  }
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
    onListen: () =>
      console.log(
        `deck service on http://127.0.0.1:${PORT} - open it for Deck Lab (reads ${Object.values(FILES).join(", ")})`.green,
      ),
  }, handle);
}
