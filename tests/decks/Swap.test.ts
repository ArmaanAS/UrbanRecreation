import { assert, assertAlmostEquals, assertEquals } from "@std/assert";
import { type MatchupRequest, type MatchupResponse, type MatchupRunner, sampleFieldPairs, type SolveRequest } from "@/decks/Matchup.ts";
import { swapSearch } from "@/decks/Swap.ts";

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
