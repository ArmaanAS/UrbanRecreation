// scripts/DeckCapture.ts folds the collection and deck traffic the userscript mirrors into
// the deck builder's data files. These records are shaped like the ones in the raw log.
import { assertEquals } from "@std/assert";
import { absorbRecord, newDeckStore, rawLogStandIn } from "../../scripts/DeckCapture.ts";

const xhr = (body: string, resp: unknown, t = 1_790_000_000_000) => ({
  t,
  kind: "xhr",
  payload: { m: "POST", u: "/ajax/collection/", body, status: 200, resp: JSON.stringify(resp) },
});
const api = (call: string, params: unknown, data: unknown) => ({
  t: 1_790_000_000_000,
  kind: "fetch",
  payload: {
    m: "POST",
    u: "https://www.urban-rivals.com/api/private/v2/",
    body: "requests=" + encodeURIComponent(JSON.stringify([{ call, params }])),
    status: 200,
    resp: JSON.stringify({ [call]: { data } }),
  },
});

const row = (id: number, collectionData: Record<string, number>) => ({
  id,
  name: `Card ${id}`,
  clan_id: 44,
  clan_name: "Skeelz",
  level_min: 2,
  level_max: 4,
  rarity: "r",
  release_date: 1,
  efc_banned: false,
  tourney_banned: true,
  evos: { "2": { power: 5, damage: 3, ability: { id: 9, typeID: 4, unlockLevel: 2, description: "Power +2" }, nightAbility: [] } },
  bonus: { id: 1, typeID: 1, description: "Protection: Ability" },
  nightBonus: [],
  collectionData: { id, time_last_acquisition: 123, ...collectionData },
  marketData: { id, min_price: 999 },
});

Deno.test("a catalog page becomes public cards and the owner's copies, nothing else", () => {
  const store = newDeckStore();
  const rec = xhr("action=collectiondata&page=0&nbPerPage=500", [row(1, { lvl_2: 1, lvl_3_m1: 2 }), row(2, { lvl_2: 0 })]);
  assertEquals(absorbRecord(rec, store), true);
  assertEquals(store.cards.size, 2);
  assertEquals(store.cards.get(1)!.tourney_banned, true);
  // Market data and acquisition times never reach the store.
  assertEquals("marketData" in store.cards.get(1)!, false);
  assertEquals(store.owned.get(1), { "2": { "": 1 }, "3": { m1: 2 } });
  assertEquals(store.owned.has(2), false);
  assertEquals(
    rawLogStandIn(rec),
    "[collectiondata page 0: 2 rows, kept in data/site_cards.json and data/my_collection.json]",
  );
});

Deno.test("formats and single decks", () => {
  const store = newDeckStore();
  absorbRecord(xhr("action=deckformatsdata", [{ id: 1, name: "EFC", criteria: [{ name: "max_stars", description: "d", value: 25 }] }]), store);
  assertEquals(store.formats!.formats[0].criteria[0].value, 25);
  absorbRecord(xhr("action=loaddeck&id=7", { deck: { id: 7, name: "T1", isCurrent: true, Characters: [{ id: 1, level: 2, state: "" }] } }), store);
  absorbRecord(xhr("action=loaddeck&id=8", { deck: { id: 8, name: "T2", isCurrent: false, Characters: [] } }), store);
  assertEquals([...store.decks.keys()], [7, 8]);
  absorbRecord(xhr("action=setcurrentdeck&id=8", { success: true }), store);
  assertEquals(store.decks.get(7)!.isCurrent, false);
  assertEquals(store.decks.get(8)!.isCurrent, true);
  absorbRecord(xhr("action=deletedeck&id=7", { success: true }), store);
  assertEquals([...store.decks.keys()], [8]);
});

Deno.test("collections.decks: every deck without a format, the server's legal ones with one", () => {
  const store = newDeckStore();
  const decks = [
    { id: 1, name: "A", isCurrent: true, characters: [] },
    { id: 2, name: "B", isCurrent: false, characters: [] },
  ];
  absorbRecord(api("collections.decks", {}, { decks }), store);
  assertEquals(store.decks.size, 2);
  absorbRecord(api("collections.decks", { deckFormatID: 54363 }, { decks: [decks[1]] }), store);
  // A room's answer never shrinks the deck list; it records which decks are legal there.
  assertEquals(store.decks.size, 2);
  assertEquals(store.legalByFormat.get(54363)!.deckIds, [2]);
});

Deno.test("other traffic is left alone", () => {
  const store = newDeckStore();
  assertEquals(absorbRecord({ t: 1, kind: "xhr", payload: { u: "/ajax/news/", body: "action=get", resp: "[]" } }, store), false);
  assertEquals(absorbRecord(xhr("action=evolve&id=445&level=1&state=m1&quantity=1", { error: "x" }), store), false);
  assertEquals(store.dirty.size, 0);
});
