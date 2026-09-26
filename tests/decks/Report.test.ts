import { assertAlmostEquals, assertEquals } from "@std/assert";
import { deckReport } from "@/decks/Report.ts";
import type { SiteCard } from "@/decks/SiteData.ts";

const card = (id: number, clan_id: number, over: Partial<SiteCard> = {}): SiteCard => ({
  id,
  name: `Card ${id}`,
  clan_id,
  clan_name: `Clan ${clan_id}`,
  level_min: 1,
  level_max: 3,
  rarity: "u",
  release_date: 1,
  efc_banned: false,
  efc_max_evo_banned: false,
  efc_temp_banned: false,
  efc_bonus_low: false,
  efc_bonus_high: false,
  tourney_banned: false,
  tourney_max_evo_banned: false,
  evos: {
    "1": { power: 4, damage: 2, ability: { id: 0, typeID: 0, unlockLevel: 2, description: "Ability at Level 2" }, nightAbility: [] },
    "2": { power: 5, damage: 3, ability: { id: 11, typeID: 3, unlockLevel: 2, description: "Power +2" }, nightAbility: [] },
    "3": {
      power: 6,
      damage: 3,
      ability: { id: 12, typeID: 3, unlockLevel: 2, description: "Power +3" },
      nightAbility: { id: 13, typeID: 3, unlockLevel: 2, description: "Night: Power +5" },
    },
  },
  bonus: { id: 1, typeID: 1, description: "Attack +8" },
  nightBonus: [],
  ...over,
});

// Clan 1: cards 1-2, clan 2: cards 3-8 (so card 1 has one clan-mate, card 3 has five).
const cards = new Map([1, 2, 3, 4, 5, 6, 7, 8].map((id) => [id, card(id, id <= 2 ? 1 : 2)]));
const deck = [1, 2, 3, 4, 5, 6, 7, 8].map((id) => ({ id, level: 2, state: "" }));

Deno.test("each card's bonus is live in the share of its hands that draw a clan-mate", () => {
  const report = deckReport(deck, { cards, formats: [] });
  // Holding card 1, the other three cards come from seven, one of them its clan-mate.
  assertAlmostEquals(report.cards[0].bonusLiveShare!, 1 - 20 / 35, 1e-12);
  // Card 3 has five clan-mates among the seven: only hands of the two others miss.
  assertAlmostEquals(report.cards[2].bonusLiveShare!, 1, 1e-12);
  assertEquals(report.clans.map((c) => [c.clanId, c.count]), [[2, 6], [1, 2]]);
  assertEquals(report.stars, 16);
});

Deno.test("a locked ability, a night variant and the owner's copies", () => {
  const owned = new Map([[1, { "1": { "": 1 }, "2": { p: 2 } }]]);
  const locked = deckReport([{ id: 1, level: 1, state: "" }], { cards, formats: [], owned });
  assertEquals(locked.cards[0].ability, "No Ability");
  assertEquals(locked.cards[0].abilityLocked, true);
  const night = deckReport([{ id: 1, level: 3, state: "" }], { cards, formats: [], owned }, true);
  assertEquals(night.cards[0].ability, "Night: Power +5");
  const copies = deckReport([{ id: 1, level: 2, state: "" }], { cards, formats: [], owned });
  // Two prismatic copies at level 2, none of the classic edition the deck asks for.
  assertEquals([copies.cards[0].ownedAtLevel, copies.cards[0].ownedExact, copies.allOwned], [2, 0, false]);
});

Deno.test("an unknown card is reported, and no format can call the deck legal", () => {
  const formats = [{ id: 7, name: "Any", criteria: [{ name: "min_characters", description: "", value: 1 }] }];
  const report = deckReport([{ id: 999, level: 2, state: "" }, deck[0]], { cards, formats });
  assertEquals(report.cards[0].known, false);
  assertEquals(report.formats[0].legal, null);
  assertEquals(report.notes.length, 1);
});
