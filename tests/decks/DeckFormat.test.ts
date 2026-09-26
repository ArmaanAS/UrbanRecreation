// src/decks/DeckFormat.ts is a port of the site's own deck validator. The unit tests pin
// each criterion on small synthetic decks; the oracle test compares every verdict with the
// server's, which the game client reveals by asking for a room's decks (`collections.decks
// {deckFormatID}` returns only the legal ones). The oracle needs the owner's gitignored
// data/my_decks.json and data/site_cards.json, so it is skipped where they are absent.
import { assertEquals } from "@std/assert";
import { checkDeck, type DeckCharacter, isKnownCriterion } from "@/decks/DeckFormat.ts";
import { deckReport } from "@/decks/Report.ts";
import type { DeckFormatData, SiteCard, SiteDeck } from "@/decks/SiteData.ts";

const card = (id: number, over: Partial<DeckCharacter> = {}): DeckCharacter => ({
  id,
  level: 3,
  level_max: 4,
  rarity: "r",
  release_date: 1_600_000_000,
  clan_id: 10 + (id % 2),
  efc_banned: false,
  abilityTypeID: 7,
  ...over,
});
const deckOf = (n: number, over: (i: number) => Partial<DeckCharacter> = () => ({})) =>
  Array.from({ length: n }, (_, i) => card(i + 1, over(i)));
const format = (...criteria: [string, unknown][]): DeckFormatData => ({
  id: 99,
  name: "Test",
  criteria: criteria.map(([name, value]) => ({ name, description: name, value })),
});
const errors = (f: DeckFormatData, deck: DeckCharacter[]) => checkDeck(f, deck).errors.map((c) => c.name);

Deno.test("card count, stars and levels", () => {
  const eight = deckOf(8);
  assertEquals(errors(format(["min_characters", 8]), eight), []);
  assertEquals(errors(format(["min_characters", 8]), eight.slice(0, 7)), ["min_characters"]);
  assertEquals(errors(format(["max_stars", 24]), eight), []); // 8 cards at level 3
  assertEquals(errors(format(["max_stars", 23]), eight), ["max_stars"]);
  const oneFive = deckOf(8, (i) => ({ level: i === 0 ? 5 : 3, level_max: 5 }));
  assertEquals(errors(format(["max_level5_characters", 1]), oneFive), []);
  const twoFives = deckOf(8, (i) => ({ level: i < 2 ? 5 : 3, level_max: 5 }));
  assertEquals(errors(format(["max_level5_characters", 1]), twoFives), ["max_level5_characters"]);
  assertEquals(checkDeck(format(["max_level5_characters", 1]), twoFives).invalidIndexes, [1]);
  assertEquals(errors(format(["max_level1_characters", 0]), deckOf(8, (i) => ({ level: i ? 3 : 1 }))), [
    "max_level1_characters",
  ]);
});

Deno.test("doubles, bans and maxed-only bans", () => {
  const doubled = [...deckOf(7), card(3)];
  assertEquals(errors(format(["no_doubles", true]), doubled), ["no_doubles"]);
  assertEquals(checkDeck(format(["no_doubles", true]), doubled).invalidIndexes, [7]);
  assertEquals(errors(format(["forbidden_character_list", [5]]), deckOf(8)), ["forbidden_character_list"]);
  // Banned only when played at its maximum level.
  const maxed = format(["forbidden_maxed_character_list", [5]]);
  assertEquals(errors(maxed, deckOf(8)), []);
  assertEquals(errors(maxed, deckOf(8, (i) => (i === 4 ? { level: 4 } : {}))), ["forbidden_maxed_character_list"]);
  assertEquals(errors(format(["exclude_elo_forbidden", true]), deckOf(8, (i) => ({ efc_banned: i === 2 }))), [
    "exclude_elo_forbidden",
  ]);
});

Deno.test("clans, leaders and ability types", () => {
  const leaders = deckOf(8, (i) => (i < 2 ? { clan_id: 36 } : {}));
  assertEquals(errors(format(["max_leaders", 1]), leaders), ["max_leaders"]);
  // Leaders never count as a clan.
  assertEquals(errors(format(["max_clans", 2]), leaders), []);
  assertEquals(errors(format(["max_clans", 1]), deckOf(8)), ["max_clans"]);
  assertEquals(errors(format(["force_balanced_clans", true]), deckOf(8)), []);
  // Card 2 moves from clan 10 to 11: five against three.
  assertEquals(errors(format(["force_balanced_clans", true]), deckOf(8, (i) => (i === 1 ? { clan_id: 11 } : {}))), [
    "force_balanced_clans",
  ]);
  assertEquals(errors(format(["authorized_clan_list", [10]]), deckOf(8)), ["authorized_clan_list"]);
  assertEquals(errors(format(["forbidden_ability_type_list", [7]]), deckOf(8)), ["forbidden_ability_type_list"]);
  // Type 0 (no ability) is never checked against a type list.
  assertEquals(errors(format(["authorized_ability_type_list", [3]]), deckOf(8, () => ({ abilityTypeID: 0 }))), []);
  // The site's own rule, kept: `contained_*` fails when every used clan is in the list.
  assertEquals(errors(format(["contained_clan_list", [10, 11]]), deckOf(8)), ["contained_clan_list"]);
});

Deno.test("an unknown criterion makes the verdict null, never a pass", () => {
  assertEquals(isKnownCriterion("max_level3_characters"), true);
  assertEquals(isKnownCriterion("max_cr_characters"), true);
  assertEquals(isKnownCriterion("min_moon_phase"), false);
  const verdict = checkDeck(format(["min_characters", 8], ["min_moon_phase", 3]), deckOf(8));
  assertEquals(verdict.legal, null);
  assertEquals(verdict.unknown.map((c) => c.name), ["min_moon_phase"]);
});

Deno.test("no format and an empty deck check nothing, as on the site", () => {
  assertEquals(checkDeck({ ...format(["min_characters", 8]), id: 0 }, deckOf(2)).legal, true);
  assertEquals(checkDeck(format(["min_characters", 8]), []).legal, true);
});

async function readJson(path: string) {
  try {
    return JSON.parse(await Deno.readTextFile(path));
  } catch {
    return undefined;
  }
}
const [siteCards, formats, myDecks] = await Promise.all([
  readJson("data/site_cards.json"),
  readJson("data/deck_formats.json"),
  readJson("data/my_decks.json"),
]);

Deno.test({
  name: "every verdict matches the server's for the owner's decks",
  ignore: !siteCards || !formats || !myDecks?.legalByFormat,
  fn: () => {
    const catalog = {
      cards: new Map((siteCards.cards as SiteCard[]).map((c) => [c.id, c])),
      formats: formats.formats as DeckFormatData[],
    };
    let compared = 0;
    for (const deck of myDecks.decks as SiteDeck[]) {
      for (const verdict of deckReport(deck.characters, catalog).formats) {
        const legal = myDecks.legalByFormat[verdict.formatId];
        if (!legal) continue;
        assertEquals(verdict.legal, legal.deckIds.includes(deck.id), `${deck.name} in ${verdict.name}`);
        compared++;
      }
    }
    // 19 decks x 4 formats when the fixture was first taken (2026-09-26).
    assertEquals(compared > 0, true);
  },
});
