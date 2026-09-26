import { assert, assertAlmostEquals, assertEquals } from "@std/assert";
import { type MatchupRequest, type MatchupResponse, type MatchupRunner, sampleFieldPairs, type SolveRequest } from "@/decks/Matchup.ts";
import type { OwnedCopies, SiteCard } from "@/decks/SiteData.ts";
import { ownedCandidates, swapSearch } from "@/decks/Swap.ts";

/** A hand's strength is the sum of its card ids, and the stronger hand wins by the difference. */
const runner: MatchupRunner = {
  run(requests: readonly MatchupRequest[], onResponse?: (r: MatchupResponse, i: number) => void) {
    return Promise.resolve((requests as SolveRequest[]).map((r, id) => {
      const strength = (hand: readonly (readonly [number, number])[]) => hand.reduce((s, [card]) => s + card, 0);
      const response: MatchupResponse = r.p1.some(([card]) => card === 666)
        ? { id, kind: "solve", refused: "P1 slot 0 Ability catalog source" }
        : {
          id,
          kind: "solve",
          value: Math.tanh((strength(r.p1) - strength(r.p2)) / 400),
          worst: -1,
          best: 1,
          best_move: { hand_index: 0, pillz: 0, fury: false },
          ko_share: 0,
          koed_share: 0,
          root_moves: 1,
          replies: 1,
          ms: 1,
        };
      onResponse?.(response, id);
      return response;
    }));
  },
};

const deck = [100, 110, 120, 130, 140, 150, 160, 170].map((id) => ({ id, level: 3 }));
const field = Array.from({ length: 12 }, (_, i) => [0, 1, 2, 3].map((k) => ({ id: 120 + 10 * i + k, level: 2 })));
const pairsFor = (cards: readonly { id: number; level: number }[]) => sampleFieldPairs(cards, field, 12, 1);

Deno.test("candidates are ranked by the paired difference, unchanged hands counting zero", async () => {
  const candidates = [{ id: 90, level: 3 }, { id: 300, level: 3 }, { id: 666, level: 3 }, { id: 200, level: 3 }];
  const result = await swapSearch(deck, 0, candidates, pairsFor, { runner });
  assertEquals(result.candidates.map((c) => c.card.id), [300, 200, 90, 666], "stronger first, a refused card last");
  const drew = pairsFor(deck).filter((p) => p.a.some((c) => c.id === 100)).length;
  assert(drew > 0 && drew < 12);
  for (const c of result.candidates) assertEquals(c.changed, drew);
  const best = result.candidates[0];
  assert(best.diff > 0 && best.scored === 12);
  // The difference is the variant's mean minus the deck's, over the same twelve pairs.
  assertAlmostEquals(best.diff, best.mean - result.base.mean, 1e-12);
  const refused = result.candidates[3];
  assertEquals(refused.refused, drew, "only the hands holding the refused card are refused");
  assertEquals(refused.scored + refused.refused, 12);
  assert(result.candidates[2].diff < 0, "a weaker card loses points");
});

Deno.test("a slot outside the deck is an error", async () => {
  let threw = false;
  try {
    await swapSearch(deck, 8, [], pairsFor, { runner });
  } catch {
    threw = true;
  }
  assert(threw);
});

const siteCard = (id: number, clan_id: number, name = `Card ${id}`): SiteCard => ({
  id,
  name,
  clan_id,
  clan_name: `Clan ${clan_id}`,
  level_min: 1,
  level_max: 4,
  rarity: "u",
  release_date: 1,
  efc_banned: false,
  efc_max_evo_banned: false,
  efc_temp_banned: false,
  efc_bonus_low: false,
  efc_bonus_high: false,
  tourney_banned: false,
  tourney_max_evo_banned: false,
  evos: Object.fromEntries([1, 2, 3, 4].map((l) => [String(l), {
    power: id % 10 + l,
    damage: l,
    ability: { id: 0, typeID: 0, unlockLevel: 0, description: "No Ability" },
    nightAbility: [],
  }])),
  bonus: { id: 1, typeID: 0, description: "Power +1" },
  nightBonus: [],
});

Deno.test("owned candidates keep every format the deck is legal in, at the highest level that does", () => {
  const cards = [1, 2, 3, 4, 5, 6, 7].map((id) => siteCard(id, id <= 6 ? 10 : 20));
  cards.push(siteCard(8, 10, "Card 1 Cr")); // the same character as card 1
  const catalog = {
    cards: new Map(cards.map((c) => [c.id, c])),
    formats: [
      { id: 1, name: "Cap 10", criteria: [{ name: "max_stars", value: 10, description: "10 stars at most" }] },
      { id: 2, name: "Any", criteria: [] },
    ],
    owned: new Map<number, OwnedCopies>([
      [5, { "4": { "": 1 }, "2": { p: 1 } }],
      [6, { "3": { "": 2 } }],
      [7, { "4": { "": 1 } }],
      [8, { "4": { "": 1 } }],
    ]),
  };
  const deck = [1, 2, 3, 4].map((id) => ({ id, level: 2, state: "" })); // 8 stars, legal in both
  const both = ownedCandidates(deck, 3, catalog, { scope: "clan", keepFormats: [1, 2], night: false });
  // Card 5 at level 4 would make 10 stars: fine. Card 6 at 3 makes 9. Card 7 is another clan and
  // card 8 is card 1 again.
  assertEquals(both.map((c) => [c.id, c.level, c.state]).sort(), [[5, 4, ""], [6, 3, ""]]);
  const tight = [1, 2, 3, 4].map((id) => ({ id, level: id === 1 ? 4 : 2, state: "" })); // 10 stars
  const capped = ownedCandidates(tight, 3, catalog, { scope: "clan", keepFormats: [1, 2], night: false });
  assertEquals(capped.map((c) => [c.id, c.level, c.state]).sort(), [[5, 2, "p"]], "level 2 is all the cap leaves");
  const loose = ownedCandidates(tight, 3, catalog, { scope: "clan", keepFormats: [2], night: false });
  assertEquals(loose.map((c) => [c.id, c.level]).sort(), [[5, 4], [6, 3]], "only the other format is kept");
  const wide = ownedCandidates(deck, 3, catalog, { scope: "deck", keepFormats: [1, 2], night: false });
  assertEquals(wide.length, 2, "the deck's clans are still clan 10 only");
  const refused = ownedCandidates(deck, 3, catalog, {
    scope: "clan",
    keepFormats: [1, 2],
    night: false,
    coverage: { cards: { "5": { "4": ["r", "r", -1, -1] } } },
  });
  assertEquals(refused.map((c) => [c.id, c.level]).sort(), [[5, 2], [6, 3]], "a level the solver refuses is skipped");
});
