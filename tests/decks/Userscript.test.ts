// The userscript's write path, run without a browser or the site: the "deck apply" block of
// ur-logger.user.js is pure, so it is cut out of the file and run against a fake
// /ajax/collection/ that behaves like the captured one (docs/site-api.md 2b-2c). Also checks
// that what the deck service queues is what the panel accepts, and that the panel's report
// is what the service records.
import { assert, assertEquals, assertRejects, assertStringIncludes } from "@std/assert";
import { type ApplyRequest, prepareApply, recordResult } from "@/decks/Apply.ts";
import type { DeckCard, SiteCard, SiteDeck } from "@/decks/SiteData.ts";
import { absorbRecord, newDeckStore } from "../../scripts/DeckCapture.ts";

const SOURCE = await Deno.readTextFile(new URL("../../ur-logger.user.js", import.meta.url));
const START = SOURCE.indexOf("// ---- deck apply (pure:");
const END = SOURCE.indexOf("// ---- end of deck apply ----");
assert(START > 0 && END > START, "the userscript's deck apply block is marked");

// deno-lint-ignore no-explicit-any
type Json = any;
type Post = (action: string, fields: [string, string][]) => Promise<Json>;
interface Apply {
  SITE_DECK_ACTIONS: string[];
  sameCards(a: DeckCard[], b: DeckCard[]): boolean;
  checkApplyRequest(req: Json): string | null;
  savedeckFields(req: Json): [string, string][];
  siteDeckPost(fetchFn: typeof fetch, origin: string): Post;
  applyDeckRequest(req: Json, post: Post): Promise<Json>;
}
const api: Apply = new Function(
  SOURCE.slice(START, END) +
    "\nreturn { SITE_DECK_ACTIONS, sameCards, checkApplyRequest, savedeckFields, siteDeckPost, applyDeckRequest };",
)();

const ORIGIN = "https://www.urban-rivals.com";

/**
 * The site's deck endpoint as captured: loaddeck and savedeck answer `{deck: {..., Characters}}`
 * with the cards in their own order; `id=0` creates a deck. `change` alters what it saves.
 */
function fakeSite(decks: Map<number, SiteDeck>, change: (cards: DeckCard[]) => DeckCard[] = (c) => c) {
  const calls: { url: string; init: RequestInit; form: URLSearchParams }[] = [];
  let nextId = 37646105;
  const reply = (deck: SiteDeck) => ({
    deck: { id: deck.id, playerID: 19309601, name: deck.name, Characters: [...deck.characters].reverse(), isCurrent: deck.isCurrent },
  });
  const fetchFn = (url: string | URL | Request, init?: RequestInit) => {
    const form = new URLSearchParams(String(init?.body));
    calls.push({ url: String(url), init: init!, form });
    const action = form.get("action");
    let body: unknown = { error: "Unknown action" };
    if (action === "loaddeck") {
      const deck = decks.get(Number(form.get("id")));
      body = deck ? reply(deck) : { error: "This deck does not exist" };
    } else if (action === "savedeck") {
      const cards: DeckCard[] = [];
      for (let i = 0; form.has(`characters[${i}][id]`); i++) {
        cards.push({
          id: Number(form.get(`characters[${i}][id]`)),
          level: Number(form.get(`characters[${i}][level]`)),
          state: form.get(`characters[${i}][state]`)!,
        });
      }
      const id = Number(form.get("id")) || nextId++;
      const deck = { id, name: form.get("name")!, isCurrent: form.get("set_current") === "true", characters: change(cards) };
      decks.set(id, deck);
      body = reply(deck);
    }
    return Promise.resolve(new Response(JSON.stringify(body), { headers: { "content-type": "application/json" } }));
  };
  return { calls, fetchFn: fetchFn as typeof fetch, actions: () => calls.map((c) => c.form.get("action")) };
}

const cards = (list: [number, number, string?][]): DeckCard[] => list.map(([id, level, state]) => ({ id, level, state: state ?? "" }));
const pirhanas = cards([[566, 4], [727, 3], [516, 3, "m1"], [528, 3], [530, 2], [531, 3], [532, 4], [533, 3]]);
const draft = cards([[566, 4], [727, 3], [516, 3, "m1"], [528, 3], [540, 3], [541, 2], [542, 4], [543, 3]]);
const skeelz: SiteDeck = { id: 17990965, name: "T2 Skeelz*", isCurrent: false, characters: pirhanas };
const request = (over: Json = {}) => ({
  id: "3f1c2e0a-0000-4000-8000-000000000001",
  state: "pending",
  deckId: 0,
  name: "T1 Pirhanas v2",
  setCurrent: false,
  characters: draft,
  before: null,
  summary: { headline: "", lines: [], warnings: [], cards: [] },
  ...over,
});

Deno.test("the userscript's version is the same in its header and its code", () => {
  const header = /\/\/ @version\s+(\S+)/.exec(SOURCE)?.[1];
  const code = /const VERSION = '([^']+)'/.exec(SOURCE)?.[1];
  assertEquals(header, "0.10.0");
  assertEquals(code, header);
});

Deno.test("a new deck is one savedeck with id 0, made like the site's own, then read back", async () => {
  const site = fakeSite(new Map());
  const outcome = await api.applyDeckRequest(request(), api.siteDeckPost(site.fetchFn, ORIGIN));
  assertEquals(outcome.ok, true, outcome.error);
  assertEquals(outcome.sent, true);
  assertEquals(outcome.deck.id, 37646105);
  assertEquals(site.actions(), ["savedeck", "loaddeck"]);

  const [save, load] = site.calls;
  assertEquals(save.url, `${ORIGIN}/ajax/collection/`);
  assertEquals(save.init.method, "POST");
  assertEquals(save.init.credentials, "same-origin");
  const headers = new Headers(save.init.headers);
  assertEquals(headers.get("x-requested-with"), "XMLHttpRequest");
  assertEquals(headers.get("content-type"), "application/x-www-form-urlencoded; charset=UTF-8");
  // Same fields in the same order as the captured save (docs/site-api.md 2c), PHP array syntax.
  assertEquals([...save.form.keys()].slice(0, 7), [
    "action",
    "id",
    "name",
    "set_current",
    "characters[0][id]",
    "characters[0][level]",
    "characters[0][state]",
  ]);
  assert(
    String(save.init.body).startsWith(
      "action=savedeck&id=0&name=T1+Pirhanas+v2&set_current=false&characters%5B0%5D%5Bid%5D=566&characters%5B0%5D%5Blevel%5D=4&characters%5B0%5D%5Bstate%5D=&",
    ),
    String(save.init.body),
  );
  assertEquals(save.form.get("characters[2][state]"), "m1");
  assertEquals(save.form.getAll("action"), ["savedeck"]);
  assertEquals(String(load.init.body), "action=loaddeck&id=37646105");
});

Deno.test("an overwrite re-reads its target first, saves into that id and verifies", async () => {
  const site = fakeSite(new Map([[skeelz.id, skeelz]]));
  const req = request({ deckId: skeelz.id, name: "T2 Skeelz*", setCurrent: true, before: skeelz });
  const outcome = await api.applyDeckRequest(req, api.siteDeckPost(site.fetchFn, ORIGIN));
  assertEquals(outcome.ok, true, outcome.error);
  assertEquals(site.actions(), ["loaddeck", "savedeck", "loaddeck"]);
  assertEquals(site.calls[1].form.get("id"), String(skeelz.id));
  assertEquals(site.calls[1].form.get("set_current"), "true");
  assertEquals(outcome.deck.isCurrent, true);
});

Deno.test("an overwrite whose target changed on the site since Deck Lab saw it sends nothing", async () => {
  const onSite = { ...skeelz, characters: [...pirhanas.slice(0, 7), { id: 599, level: 3, state: "" }] };
  const site = fakeSite(new Map([[skeelz.id, onSite]]));
  const outcome = await api.applyDeckRequest(request({ deckId: skeelz.id, before: skeelz }), api.siteDeckPost(site.fetchFn, ORIGIN));
  assertEquals(outcome.ok, false);
  assertEquals(outcome.sent, false);
  assertStringIncludes(outcome.error, "changed on the site");
  assertEquals(site.actions(), ["loaddeck"]);
});

Deno.test("what the site refuses or saves differently is reported, with savedeck marked as sent", async () => {
  const refusing = fakeSite(new Map());
  const refuse = (url: string | URL | Request, init?: RequestInit) =>
    new URLSearchParams(String(init?.body)).get("action") === "savedeck"
      ? Promise.resolve(new Response('{"error":"Your deck name is too long"}'))
      : refusing.fetchFn(url, init);
  const refused = await api.applyDeckRequest(request(), api.siteDeckPost(refuse as typeof fetch, ORIGIN));
  assertEquals([refused.ok, refused.sent], [false, true]);
  assertEquals(refused.siteResponse, { error: "Your deck name is too long" });
  assertStringIncludes(refused.error, "the site did not save it");

  const dropping = fakeSite(new Map(), (c) => c.slice(0, 7));
  const partial = await api.applyDeckRequest(request(), api.siteDeckPost(dropping.fetchFn, ORIGIN));
  assertEquals([partial.ok, partial.sent], [false, true]);
  assertStringIncludes(partial.error, "does not hold the requested cards");

  // A logged-out session answers with the login page.
  const html = () => Promise.resolve(new Response("<!doctype html><title>Login</title>", { status: 200 }));
  const loggedOut = await api.applyDeckRequest(request(), api.siteDeckPost(html as typeof fetch, ORIGIN));
  assertEquals([loggedOut.ok, loggedOut.sent], [false, true]);
  assertStringIncludes(loggedOut.siteResponse.error, "not JSON");

  const offline = () => Promise.reject(new TypeError("Failed to fetch"));
  const lost = await api.applyDeckRequest(request(), api.siteDeckPost(offline as typeof fetch, ORIGIN));
  assertEquals([lost.ok, lost.sent, lost.error], [false, true, "Failed to fetch"]);
});

Deno.test("nothing is sent for a malformed request, and nothing off the allowlist ever", async () => {
  const bad = [
    request({ name: "" }),
    request({ name: "x".repeat(33) }),
    request({ characters: draft.slice(0, 3) }),
    request({ characters: [...draft.slice(0, 7), draft[0]] }),
    request({ characters: [...draft.slice(0, 7), { id: 543, level: 6, state: "" }] }),
    request({ characters: [...draft.slice(0, 7), { id: 543, level: 3, state: "a&b" }] }),
    request({ setCurrent: "true" }),
    request({ deckId: skeelz.id, before: null }),
    request({ deckId: -1 }),
  ];
  for (const req of bad) {
    const site = fakeSite(new Map());
    const outcome = await api.applyDeckRequest(req, api.siteDeckPost(site.fetchFn, ORIGIN));
    assertEquals([outcome.ok, outcome.sent], [false, false], JSON.stringify(req));
    assertStringIncludes(outcome.error, "not sent: ");
    assertEquals(site.calls.length, 0);
  }

  assertEquals(api.SITE_DECK_ACTIONS, ["loaddeck", "savedeck"]);
  const site = fakeSite(new Map());
  for (const action of ["deletedeck", "setcurrentdeck", "evolve", "sell", "purchase"]) {
    await assertRejects(() => api.siteDeckPost(site.fetchFn, ORIGIN)(action, [["id", "1"]]), Error, `never sends ${action}`);
  }
  assertEquals(site.calls.length, 0);
  // And no other code in the script names one of those actions.
  const code = SOURCE.split("\n").filter((line) => !line.trim().startsWith("//")).join("\n");
  for (const action of ["deletedeck", "setcurrentdeck", "evolve", "devolve", "purchase"]) {
    assert(!new RegExp(`['"\`=]${action}\\b`).test(code), action);
  }
});

Deno.test("the panel's site calls reach the deck files the way the site's own do", async () => {
  // The panel posts through the page's patched fetch, which mirrors each call to the log
  // server as a `fetch` record; scripts/DeckCapture.ts must fold those in like the page's XHRs,
  // so a refused stale overwrite leaves Deck Lab with the site's version of the deck.
  const site = fakeSite(new Map([[skeelz.id, skeelz]]));
  const store = newDeckStore();
  let t = 1_790_000_000_000;
  const mirrored = (async (url: string | URL | Request, init?: RequestInit) => {
    const res = await site.fetchFn(url, init);
    const rec = { t: t++, kind: "fetch", payload: { m: "POST", u: String(url), body: String(init?.body), status: 200, resp: await res.clone().text() } };
    assert(absorbRecord(rec, store), String(init?.body));
    return res;
  }) as typeof fetch;
  const req = request({ deckId: skeelz.id, name: "T2 Skeelz*", before: skeelz });
  assertEquals((await api.applyDeckRequest(req, api.siteDeckPost(mirrored, ORIGIN))).ok, true);
  assert(api.sameCards(store.decks.get(skeelz.id)!.characters, draft));
  assertEquals(store.dirty.has("decks"), true);
});

// ---- the deck service and the panel speak the same language ------------------------------
const siteCard = (id: number): SiteCard => ({
  id,
  name: `Card ${id}`,
  clan_id: 1,
  clan_name: "Piranas",
  level_min: 1,
  level_max: 5,
  rarity: "u",
  release_date: 1,
  efc_banned: false,
  efc_max_evo_banned: false,
  efc_temp_banned: false,
  efc_bonus_low: false,
  efc_bonus_high: false,
  tourney_banned: false,
  tourney_max_evo_banned: false,
  evos: Object.fromEntries([1, 2, 3, 4, 5].map((l) => [String(l), {
    power: 5,
    damage: 3,
    ability: { id: 1, typeID: 3, unlockLevel: 1, description: "Power +2" },
    nightAbility: [],
  }])),
  bonus: { id: 1, typeID: 1, description: "Attack +8" },
  nightBonus: [],
});

Deno.test("a request the service queues is one the panel sends, and its report is what the service logs", async () => {
  const all = [...pirhanas, ...draft];
  const owned = new Map<number, Record<string, Record<string, number>>>();
  for (const c of all) owned.set(c.id, { ...owned.get(c.id), [String(c.level)]: { [c.state]: 1 } });
  const prepared = prepareApply(
    { characters: draft, name: "T2 Skeelz*", target: { deckId: skeelz.id }, setCurrent: false },
    { cards: new Map(all.map((c) => [c.id, siteCard(c.id)])), formats: [], owned, decks: [skeelz] },
  );
  assert("draft" in prepared, JSON.stringify(prepared));
  const queued: ApplyRequest = { id: crypto.randomUUID(), createdAt: new Date().toISOString(), state: "pending", ...prepared.draft };
  // Through JSON, as the log server's proxy hands it to the panel.
  const seen = JSON.parse(JSON.stringify(queued));
  assertEquals(api.checkApplyRequest(seen), null);

  const site = fakeSite(new Map([[skeelz.id, skeelz]]));
  const outcome = await api.applyDeckRequest(seen, api.siteDeckPost(site.fetchFn, ORIGIN));
  assertEquals(outcome.ok, true, outcome.error);
  const recorded = recordResult(queued, JSON.parse(JSON.stringify(outcome)));
  assert("history" in recorded && recorded.history, JSON.stringify(recorded));
  assertEquals(queued.state, "applied");
  assertEquals(recorded.history.before, skeelz);
  assertEquals(recorded.history.after?.characters.length, 8);
  assert(api.sameCards(recorded.history.after!.characters, draft));
  assertEquals(recorded.history.deckId, skeelz.id);
});
