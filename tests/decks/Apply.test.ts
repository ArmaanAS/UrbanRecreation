// The deck service's guarded write queue (/api/apply): what Deck Lab may ask for, what it is
// told, and what reaches the undo log. Nothing here, or anywhere in the service, talks to the
// site; the userscript's half is tests/decks/Userscript.test.ts.
import { assert, assertEquals, assertStringIncludes } from "@std/assert";
import { handle, useDataFiles } from "@/decks/Service.ts";
import type { SiteCard } from "@/decks/SiteData.ts";

const SITE = "https://www.urban-rivals.com";

const card = (id: number, name: string, clan_id: number): SiteCard => ({
  id,
  name,
  clan_id,
  clan_name: `Clan ${clan_id}`,
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
    power: l + 2,
    damage: l,
    ability: { id: 10 + l, typeID: 3, unlockLevel: 1, description: `Power +${l}` },
    nightAbility: [],
  }])),
  bonus: { id: 1, typeID: 1, description: "Attack +8" },
  nightBonus: [],
});

// Cards 101-112 in two clans; the owner has each at level 3, 104 also at 4 as Sapphire (m1).
const ids = Array.from({ length: 12 }, (_, i) => 101 + i);
const cards = ids.map((id) => card(id, `Card ${id}`, id <= 106 ? 1 : 2));
const owned: Record<string, Record<string, Record<string, number>>> = Object.fromEntries(ids.map((id) => [String(id), { "3": { "": 1 } }]));
owned["104"]["4"] = { m1: 1 };
const at3 = (list: number[]) => list.map((id) => ({ id, level: 3, state: "" }));
const skeelz = { id: 5001, name: "T2 Skeelz*", isCurrent: false, characters: at3([101, 102, 103, 104, 105, 106, 107, 108]) };
const lab = { id: 5002, name: "Lab 3", isCurrent: true, characters: at3([101, 102, 103, 104]) };

const dir = await Deno.makeTempDir({ prefix: "deck-apply-" });
const files = {
  formats: `${dir}/deck_formats.json`,
  cards: `${dir}/site_cards.json`,
  collection: `${dir}/my_collection.json`,
  decks: `${dir}/my_decks.json`,
  history: `${dir}/deck_history.jsonl`,
};
const freeFight = {
  id: 57215,
  name: "Free Fight",
  criteria: [
    { name: "min_characters", description: "Your Deck cannot contain fewer than 8 cards.", value: 8 },
    { name: "no_doubles", description: "Your Deck cannot contain any duplicates.", value: true },
  ],
};
await Deno.writeTextFile(files.formats, JSON.stringify({ fetchedAt: "2026-09-26T00:00:00Z", formats: [freeFight] }));
await Deno.writeTextFile(files.cards, JSON.stringify({ fetchedAt: "2026-09-26T00:00:00Z", cards }));
await Deno.writeTextFile(files.collection, JSON.stringify({ fetchedAt: "2026-09-26T00:00:00Z", cards: owned }));
await Deno.writeTextFile(files.decks, JSON.stringify({ fetchedAt: "2026-09-26T00:00:00Z", decks: [skeelz, lab] }));
useDataFiles(files);

const req = (path: string, init: RequestInit & { origin?: string } = {}) => {
  const headers = new Headers(init.headers);
  if (init.origin) headers.set("origin", init.origin);
  return new Request(`http://127.0.0.1:8788${path}`, { ...init, headers });
};
const post = (path: string, body: unknown, origin?: string) =>
  handle(req(path, { method: "POST", body: typeof body === "string" ? body : JSON.stringify(body), origin }));
const current = async () => await (await handle(req("/api/apply"))).json();
const history = async (): Promise<Record<string, unknown>[]> => {
  try {
    return (await Deno.readTextFile(files.history)).split("\n").filter(Boolean).map((l) => JSON.parse(l));
  } catch {
    return [];
  }
};
/** Withdraws whatever the previous test left waiting. */
const reset = async () => await (await handle(req("/api/apply", { method: "DELETE" }))).json();

const rescue = at3([101, 102, 103, 109, 110, 111, 112, 107]);
const overwrite = { characters: rescue, name: "T2 Skeelz*", target: { deckId: 5001 }, setCurrent: false };

Deno.test("a dry run describes the write and queues nothing", async () => {
  await reset();
  const before = await current();
  const res = await post("/api/apply", { ...overwrite, dryRun: true });
  assertEquals(res.status, 200);
  const view = await res.json();
  assertEquals(view.dryRun, true);
  assertEquals(view.deckId, 5001);
  assertEquals(view.before.name, "T2 Skeelz*");
  assertEquals(
    view.summary.headline,
    'Will overwrite "T2 Skeelz*" (8 cards, 24★): 4 cards replaced by Card 109 L3, Card 110 L3, Card 111 L3, Card 112 L3.',
  );
  assert(view.summary.lines.includes("Out: Card 104 L3, Card 105 L3, Card 106 L3, Card 108 L3"), view.summary.lines.join("\n"));
  assert(view.summary.lines.includes("Unchanged: 4 cards"));
  assert(view.summary.lines.includes("Legal in Free Fight"));
  assertEquals(view.summary.cards.map((c: { name: string }) => c.name)[3], "Card 109");
  assertEquals((await current()).id, before.id, "a dry run leaves the queue as it was");
});

Deno.test("a bad request says everything that is wrong and queues nothing", async () => {
  await reset();
  const cases: [unknown, string][] = [
    [{ ...overwrite, characters: rescue.slice(0, 3) }, "4 to 30 cards"],
    [{ ...overwrite, characters: [...rescue.slice(0, 7), { id: 112, level: 6, state: "" }] }, "level from 1 to 5"],
    [{ ...overwrite, characters: [...rescue.slice(0, 7), { id: 112, level: 3, state: "<b>" }] }, "malformed edition"],
    [{ ...overwrite, name: "   " }, "1 to 32 characters"],
    [{ ...overwrite, name: "x".repeat(33) }, "1 to 32 characters"],
    [{ ...overwrite, target: { deckId: 999 } }, "has not been captured"],
    [{ ...overwrite, target: "somewhere" }, 'target must be "new"'],
    [{ ...overwrite, setCurrent: "yes" }, "setCurrent must be true or false"],
    [{ ...overwrite, characters: [...rescue.slice(0, 7), rescue[0]] }, "Card 101 is in the deck twice"],
    // Owned at level 3 classic and level 4 Sapphire, so neither level 4 classic nor level 3 Sapphire.
    [{ ...overwrite, characters: [...rescue.slice(0, 7), { id: 104, level: 4, state: "" }] }, "you do not own Card 104 L4; you have L3 classic, L4 Sapphire Deep Blue"],
    [{ ...overwrite, characters: [...rescue.slice(0, 7), { id: 104, level: 3, state: "m1" }] }, "you do not own Card 104 L3 (Sapphire Deep Blue)"],
    [{ ...overwrite, characters: skeelz.characters }, "already is exactly this deck"],
  ];
  for (const [body, message] of cases) {
    const res = await post("/api/apply", body);
    assertEquals(res.status, 400, JSON.stringify(body));
    assertStringIncludes((await res.json()).error, message);
  }
  const both = await (await post("/api/apply", { ...overwrite, name: "", characters: [...rescue.slice(0, 7), { id: 104, level: 4, state: "" }] })).json();
  assertEquals(both.errors.length, 2, "every problem at once");
  assert((await current()).state !== "pending");
  assertEquals(await (await post("/api/apply", "{")).status, 400);
});

Deno.test("only Deck Lab or a local tool may queue a write; the site may only read and answer", async () => {
  await reset();
  const refused = await post("/api/apply", overwrite, SITE);
  assertEquals(refused.status, 403);
  await refused.body?.cancel();
  assert((await current()).state !== "pending");
  const fromLab = await post("/api/apply", overwrite, "http://127.0.0.1:8788");
  assertEquals(fromLab.status, 201);
  const request = await fromLab.json();
  const seen = await (await handle(req("/api/apply", { origin: SITE }))).json();
  assertEquals(seen.id, request.id, "the panel on the site sees the pending request");
  assertEquals(await (await handle(req("/api/apply", { origin: "https://evil.example" }))).status, 403);
});

Deno.test("an applied overwrite is verified here and logged with the deck it replaced", async () => {
  await reset();
  const lines = (await history()).length;
  const created = await post("/api/apply", overwrite);
  assertEquals(created.status, 201);
  const request = await created.json();
  assertEquals(request.state, "pending");
  assertEquals(request.before, skeelz);
  assertEquals((await current()).id, request.id);

  const again = await post("/api/apply", { ...overwrite, name: "Other" });
  assertEquals(again.status, 409, "one pending write at a time");
  assertEquals((await again.json()).pending.id, request.id);

  // The owner clicks Apply: the panel claims the request before it sends anything.
  const claimed = await post(`/api/apply/${request.id}/claim`, {});
  assertEquals(claimed.status, 200);
  assertEquals((await claimed.json()).state, "applying");

  // The site answers with the cards in its own order.
  const siteDeck = { id: 5001, playerID: 1, name: "T2 Skeelz*", Characters: [...rescue].reverse(), isCurrent: false };
  const done = await post(`/api/apply/${request.id}/result`, { ok: true, sent: true, deck: siteDeck, siteResponse: { deck: siteDeck } });
  assertEquals(done.status, 200);
  const view = await done.json();
  assertEquals(view.state, "applied");
  assertEquals(view.result.verified, true);
  assertEquals(view.result.deck.characters.length, 8);

  const log = await history();
  assertEquals(log.length, lines + 1);
  const entry = log.at(-1)!;
  assertEquals(entry.requestId, request.id);
  assertEquals(entry.deckId, 5001);
  assertEquals(entry.name, "T2 Skeelz*");
  assertEquals(entry.before, skeelz);
  assertEquals((entry.after as { characters: unknown[] }).characters.length, 8);
  assertEquals(entry.verified, true);
  assertEquals((entry.siteResponse as { deck: { id: number } }).deck.id, 5001);

  const twice = await post(`/api/apply/${request.id}/result`, { ok: true, sent: true, deck: siteDeck });
  assertEquals(twice.status, 409, "an outcome is recorded once");
  await twice.body?.cancel();
  const listed = await (await handle(req("/api/apply/history"))).json();
  assertEquals(listed[0].requestId, request.id, "newest first");
});

Deno.test("the panel's word is not taken: other cards on the site fail, and are still logged", async () => {
  await reset();
  const lines = (await history()).length;
  const request = await (await post("/api/apply", { ...overwrite, name: "Renamed" })).json();
  assert(request.summary.lines.includes('Renamed to "Renamed"'));
  const wrong = { id: 5001, name: "Renamed", Characters: skeelz.characters, isCurrent: false };
  const view = await (await post(`/api/apply/${request.id}/result`, { ok: true, sent: true, deck: wrong, siteResponse: { deck: wrong } })).json();
  assertEquals(view.state, "failed");
  assertEquals(view.result.verified, false);
  assertEquals(view.result.error, "the saved deck does not hold the requested cards");
  const log = await history();
  assertEquals(log.length, lines + 1, "the site may have changed, so the undo log has it");
  assertEquals(log.at(-1)!.verified, false);
});

Deno.test("dismissed or refused before sending: nothing is logged", async () => {
  await reset();
  const lines = (await history()).length;
  const first = await (await post("/api/apply", overwrite)).json();
  const dismissed = await (await post(`/api/apply/${first.id}/result`, { dismissed: true })).json();
  assertEquals(dismissed.state, "dismissed");
  const second = await (await post("/api/apply", overwrite)).json();
  const stale = await (await post(`/api/apply/${second.id}/result`, {
    ok: false,
    sent: false,
    error: '"T2 Skeelz*" changed on the site since Deck Lab last saw it, so nothing was saved.',
  })).json();
  assertEquals(stale.state, "failed");
  assertStringIncludes(stale.result.error, "changed on the site");
  assertEquals((await history()).length, lines);
  const malformed = await post(`/api/apply/${second.id}/result`, { ok: true });
  assertEquals(malformed.status, 409);
  await malformed.body?.cancel();
  const unknown = await post("/api/apply/00000000-0000-0000-0000-000000000000/result", { sent: false });
  assertEquals(unknown.status, 404);
  await unknown.body?.cancel();
});

Deno.test("Deck Lab can withdraw or replace a waiting request; a late write still reaches the log", async () => {
  await reset();
  const first = await (await post("/api/apply", overwrite)).json();
  const withdrawn = await (await handle(req("/api/apply", { method: "DELETE" }))).json();
  assertEquals(withdrawn.id, first.id);
  assertEquals(withdrawn.state, "discarded");

  const second = await (await post("/api/apply", overwrite)).json();
  const replaced = await post("/api/apply", { ...overwrite, name: "Replacement", replace: true });
  assertEquals(replaced.status, 201);
  const third = await replaced.json();
  assertEquals((await current()).id, third.id);

  // The panel had already clicked Apply on the replaced one: the site changed, so it is logged.
  const lines = (await history()).length;
  const siteDeck = { id: 5001, name: "T2 Skeelz*", Characters: rescue, isCurrent: false };
  const late = await (await post(`/api/apply/${second.id}/result`, { ok: true, sent: true, deck: siteDeck })).json();
  assertEquals(late.state, "applied");
  assertEquals((await history()).length, lines + 1);
  assertEquals((await current()).id, third.id, "the newer request is still the one waiting");
});

Deno.test("once the panel claims a request, no other tab, withdrawal or new request can race it", async () => {
  await reset();
  const request = await (await post("/api/apply", overwrite)).json();
  assertEquals((await (await post(`/api/apply/${request.id}/claim`, {})).json()).state, "applying");

  const secondTab = await post(`/api/apply/${request.id}/claim`, {});
  assertEquals(secondTab.status, 409);
  assertStringIncludes((await secondTab.json()).error, "no longer waiting (applying)");
  const withdraw = await handle(req("/api/apply", { method: "DELETE" }));
  assertEquals(withdraw.status, 409);
  assertEquals((await withdraw.json()).state, "applying");
  const another = await post("/api/apply", { ...overwrite, name: "Another" });
  assertEquals(another.status, 409);
  assertStringIncludes((await another.json()).error, "saving another deck right now");
  assertEquals((await current()).state, "applying");

  const siteDeck = { id: 5001, name: "T2 Skeelz*", Characters: rescue, isCurrent: false };
  assertEquals((await (await post(`/api/apply/${request.id}/result`, { ok: true, sent: true, deck: siteDeck })).json()).state, "applied");

  // A withdrawn or replaced request cannot be claimed at all.
  const withdrawn = await (await post("/api/apply", overwrite)).json();
  await reset();
  const late = await post(`/api/apply/${withdrawn.id}/claim`, {});
  assertEquals(late.status, 409);
  assertStringIncludes((await late.json()).error, "(discarded)");
  const unknown = await post("/api/apply/00000000-0000-0000-0000-000000000000/claim", {});
  assertEquals(unknown.status, 404);
  await unknown.body?.cancel();
});

Deno.test("a new deck is described as one, and the current deck stays current", async () => {
  await reset();
  const created = await (await post("/api/apply", { characters: rescue.slice(0, 4), name: "T1 Rescue v2", target: "new", setCurrent: true })).json();
  assertEquals(created.deckId, 0);
  assertEquals(created.before, null);
  assertEquals(created.setCurrent, true);
  assertEquals(created.summary.headline, 'Will create a new deck "T1 Rescue v2" with 4 cards (12★) and make it your current deck.');
  assert(created.summary.lines.includes("not legal in Free Fight"));
  assert(created.summary.warnings.some((w: string) => w.includes("legal in no captured format")));
  const saved = { id: 7777, name: "T1 Rescue v2", Characters: rescue.slice(0, 4), isCurrent: true };
  assertEquals((await (await post(`/api/apply/${created.id}/result`, { ok: true, sent: true, deck: saved })).json()).state, "applied");

  // Collection Pro's own save keeps the current deck current, so this does too.
  const onCurrent = await (await post("/api/apply", { ...overwrite, target: { deckId: 5002 }, name: "Lab 3", setCurrent: false })).json();
  assertEquals(onCurrent.setCurrent, true);
  assert(onCurrent.summary.lines.includes("It is your current deck and stays current."));
  assertStringIncludes(onCurrent.summary.headline, '"Lab 3" (4 cards, 12★): 1 card out, 5 cards in (Card 109 L3,');
});
