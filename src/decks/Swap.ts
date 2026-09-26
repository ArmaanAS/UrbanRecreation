// "Find a better card for this slot": candidates for one slot of a deck, each scored against the
// same opposing hands as the deck itself and ranked by the paired difference. A deck's own hands
// depend only on its size and the seed (sampleHandPairs, sampleFieldPairs), so a variant that
// differs in one slot shares every hand that does not draw that slot: those pairs are identical,
// cost nothing (the cache), and differ by exactly zero, which is what makes a small N enough to
// rank candidates. Phase 7 of docs/deck-builder-design.md, one slot at a time.
import {
  type DeckCardRef,
  type HandPair,
  isRefusedPair,
  type PairOutcome,
  type PairResult,
  solvePairs,
  type SolveOptions,
  summarize,
} from "./Matchup.ts";

export interface SwapCandidateResult<C extends DeckCardRef> {
  card: C;
  /** Pairs scored with the candidate, and pairs refused (mostly: the candidate itself). */
  scored: number;
  refused: number;
  /** The variant deck's mean score in [-1, 1]. */
  mean: number;
  /** Mean of (variant - deck) over the pairs both scored, unchanged pairs counting zero. */
  diff: number;
  diffErr: number;
  /** Pairs whose hand drew the slot, so the candidate was actually played; NaN diff when none scored. */
  changed: number;
}

export interface SwapSearchResult<C extends DeckCardRef> {
  base: ReturnType<typeof summarize>;
  candidates: SwapCandidateResult<C>[];
  cached: number;
  solved: number;
}

const handKey = (hand: readonly DeckCardRef[]) => hand.map((c) => `${c.id}:${c.level}`).sort().join(",");

/**
 * Scores `deck` and every `candidates[k]` in `slot`, each on `pairsFor(variant)`, in one solver
 * batch, and ranks the candidates by the paired difference, best first.
 */
export async function swapSearch<C extends DeckCardRef>(
  deck: readonly DeckCardRef[],
  slot: number,
  candidates: readonly C[],
  pairsFor: (deck: readonly DeckCardRef[]) => HandPair[],
  options: SolveOptions,
): Promise<SwapSearchResult<C>> {
  if (slot < 0 || slot >= deck.length) throw new Error(`slot ${slot} is not in a deck of ${deck.length}`);
  const basePairs = pairsFor(deck);
  const variants = candidates.map((card) => pairsFor(deck.map((c, i) => (i === slot ? card : c))));
  const { outcomes, cached, solved } = await solvePairs([basePairs, ...variants].flat(), options);
  const base = outcomes.slice(0, basePairs.length);
  const ranked = candidates.map((card, k): SwapCandidateResult<C> => {
    const start = basePairs.length * (k + 1);
    const mine: PairOutcome[] = outcomes.slice(start, start + basePairs.length);
    const diffs: number[] = [];
    let changed = 0, changedScored = 0;
    mine.forEach((outcome, i) => {
      const drew = handKey(variants[k][i].a) !== handKey(basePairs[i].a);
      if (drew) changed++;
      const was = base[i];
      if (isRefusedPair(outcome) || isRefusedPair(was)) return;
      if (drew) changedScored++;
      diffs.push((outcome as PairResult).score - (was as PairResult).score);
    });
    const s = summarize(mine);
    // Without one scored hand that holds the candidate there is nothing to compare, not a zero.
    const diff = changedScored ? diffs.reduce((sum, d) => sum + d, 0) / diffs.length : NaN;
    const variance = diffs.length > 1 ? diffs.reduce((sum, d) => sum + (d - diff) ** 2, 0) / (diffs.length - 1) : NaN;
    return {
      card,
      scored: s.scored,
      refused: s.refused,
      mean: s.mean,
      diff,
      diffErr: Math.sqrt(variance / diffs.length),
      changed,
    };
  });
  const order = (x: number) => (Number.isFinite(x) ? x : -Infinity);
  ranked.sort((x, y) => order(y.diff) - order(x.diff));
  return { base: summarize(base), candidates: ranked, cached, solved };
}
