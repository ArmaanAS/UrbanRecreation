// Search cuts the tree into per-depth-2 units so progress can be read out mid-flight. These
// small fixtures pin the cases where its information-aware policy and the original
// perfect-information tree agree. Policy.test.ts pins their deliberate divergence when a
// future reply would otherwise peek at hidden pillz.
import "colors";
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertAlmostEquals, assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import Analysis from "@/solver/Analysis.ts";
import Search, { openingReplyWeight, SearchMode } from "@/solver/Search.ts";
import type { Node } from "@/solver/Minimax.ts";

const quiet = <T>(f: () => T): T => {
  const log = console.log, info = console.info;
  console.log = () => 0;
  console.info = () => 0;
  try {
    return f();
  } finally {
    console.log = log;
    console.info = info;
  }
};

/** Two rounds played out, so round 3 with 5 pillz each: small enough for iterTree. */
function position(alsoSelectFirst: boolean) {
  return quiet(() => {
    const g = new Game(
      new Player(12, 6, 0),
      new Player(12, 6, 1),
      HandGenerator.handOf(["Genmaicha", "Orka", "Sando", "Deborah"]),
      HandGenerator.handOf(["Nathan", "El Kuzco", "Noon Steevens", "Strygia"]),
      Turn.PLAYER_1,
      false,
    );
    g.select(0, 1, false, false);
    g.select(0, 1, false, false);
    g.select(1, 0, false, false);
    g.select(1, 0, false, false);
    if (alsoSelectFirst) g.select(2, 1, false, false); // p1 commits, p2 must answer
    return g;
  });
}

const runToEnd = (s: Search) =>
  quiet(() => {
    while (s.step());
    return s;
  });

function opening(alsoSelectFirst: boolean, pillz = 12) {
  return quiet(() => {
    const game = new Game(
      new Player(12, pillz, 0),
      new Player(12, pillz, 1),
      HandGenerator.handOf(["Genmaicha", "Orka", "Sando", "Deborah"]),
      HandGenerator.handOf(["Nathan", "El Kuzco", "Noon Steevens", "Strygia"]),
      Turn.PLAYER_1,
      false,
    );
    if (alsoSelectFirst) game.select(0, 0, false, false);
    return game;
  });
}

Deno.test("round one first-mover search covers every legal opening bet", () => {
  const search = new Search(opening(false));

  assertEquals(search.mode, SearchMode.FIRST);
  assertEquals(search.openingPrior, true);
  assertEquals(search.candidates.length, 92);
  assertEquals(search.candidates.some((move) => move.pillz > 0), true);
  assertEquals(search.candidates.some((move) => move.fury), true);
  assertEquals(search.samples, 92);
  assertEquals(search.units, 8464);
});

Deno.test("round one solves every leaf and weights replies by the opening prior", () => {
  // Two pillz each keeps the exact opening small: 12 actions a side, 144 pairings.
  const search = runToEnd(new Search(opening(false, 2)));
  assertEquals(search.units, 144);

  for (const candidate of search.candidates) {
    assertEquals(candidate.done, search.samples);
    // A solved line is a win, draw or loss; nothing is a position score any more.
    for (const value of candidate.values) {
      assertEquals(
        [-1, 0, 1].includes(value),
        true,
        `${candidate.key}: ${value}`,
      );
    }
    // FIRST samples arrive in reply order, each weighted by what opponents open with.
    const weights = search.opponentMoves.map(openingReplyWeight);
    assertEquals(candidate.weights, weights);
    const mean = candidate.values.reduce((sum, v, i) =>
      sum + v * weights[i], 0) /
      weights.reduce((sum, w) => sum + w, 0);
    assertAlmostEquals(candidate.average, mean, 1e-12);
    // The Worst column is the real extremum over the opponent's reply.
    assertEquals(
      candidate.minimax,
      search.us === Turn.PLAYER_1
        ? Math.min(...candidate.values)
        : Math.max(...candidate.values),
    );
  }
  // The prior is not uniform, so it does move the average.
  assertEquals(
    new Set(search.opponentMoves.map(openingReplyWeight)).size > 1,
    true,
  );
});

Deno.test("round one second-mover search keeps every hidden opponent bet", () => {
  const search = new Search(opening(true));

  assertEquals(search.mode, SearchMode.SECOND);
  assertEquals(search.openingPrior, true);
  assertEquals(search.candidates.length, 92);
  // Their fixed card can carry any of 23 legal hidden bets; none are sampled away.
  assertEquals(search.samples, 23);
  assertEquals(search.units, 2116);
});

Deno.test("the full move set returns after round one", () => {
  const game = opening(false);
  quiet(() => {
    game.select(0, 0, false, false);
    game.select(0, 0, false, false);
  });
  const search = new Search(game);

  assertEquals(search.round, 2);
  assertEquals(search.openingPrior, false);
  assertEquals(search.candidates.length, 69);
  assertEquals(search.candidates.some((move) => move.pillz > 0), true);
  assertEquals(search.candidates.some((move) => move.fury), true);
});

Deno.test("equal displayed win chance ranks Draw worst case above Lose", () => {
  const search = new Search(position(false));
  const [lose, draw] = search.candidates;
  const inOurFrame = (value: number) =>
    search.us === Turn.PLAYER_1 ? value : -value;

  for (const candidate of search.candidates) candidate.average = NaN;
  for (const candidate of [lose, draw]) {
    candidate.average = inOurFrame(0.86); // displayed as a 93% win chance
    candidate.done = search.samples;
  }
  lose.minimax = inOurFrame(-1);
  lose.kos = search.samples; // the old KO-first tie-break put this line first
  draw.minimax = inOurFrame(0);

  assertEquals(search.ranked().slice(0, 2).map((c) => c.key), [
    draw.key,
    lose.key,
  ]);
});

Deno.test("blind-second search ranks replies before the opponent reveals a card", () => {
  const search = new Search(position(false), 1, 0, true);

  assertEquals(search.mode, SearchMode.BLIND_SECOND);
  assertEquals(search.us, Turn.PLAYER_2);
  assertEquals(search.candidates.length, 18);
  assertEquals(search.samples, 18);
  assertEquals(search.units, 324);
  quiet(() => search.step());
  assertEquals(search.best() !== undefined, true);
});

Deno.test("Search retains reference values when we move first and information agrees", () => {
  const search = runToEnd(new Search(position(false)));
  const tree = quiet(() => Analysis.iterTree(position(false)));

  assertEquals(search.mode, SearchMode.FIRST);
  assertEquals(search.candidates.length, tree.nodes.length);
  assertEquals(search.stats.unitsDone, search.units);

  // Every candidate is fully sampled, and its average is iterTree's rating(false).
  const byKey = new Map(tree.nodes.map((n: Node) => [n.name, n]));
  for (const c of search.candidates) {
    assertEquals(c.done, search.samples, `${c.key} was not fully sampled`);
    const node = byKey.get(c.key);
    assertEquals(node !== undefined, true, `iterTree has no node ${c.key}`);
    assertAlmostEquals(
      c.average,
      quiet(() => node!.rating(false)),
      1e-9,
      `average ${c.key}`,
    );
    assertAlmostEquals(
      c.minimax,
      quiet(() => node!.rating()),
      1e-9,
      `minimax ${c.key}`,
    );
  }

  // Same evaluation, so the same value on top. Not necessarily the same *move*:
  // Minimax.best() breaks ties on the cheaper bet, while Search prefers one that wins by
  // knockout this round. That divergence is deliberate - see Search.ranked().
  const best = quiet(() => tree.best());
  const chosen = search.best()!;
  const byMove = new Map(search.candidates.map((c) => [c.key, c]));
  assertAlmostEquals(
    chosen.average,
    byMove.get(best.name)!.average,
    1e-9,
    `best move differs in value: ${chosen.key} vs ${best.name}`,
  );
});

Deno.test("Search retains reference values when we move second and information agrees", () => {
  const search = runToEnd(new Search(position(true)));
  const tree = quiet(() => Analysis.iterTree(position(true)));

  assertEquals(search.mode, SearchMode.SECOND);
  assertEquals(search.oppIndex, 2);
  assertEquals(search.stats.unitsDone, search.units);

  // bestTimeline() groups the grandchildren by our reply and averages over the opponent's
  // hidden pillz; Search folds the same values into each candidate, unrescaled.
  const combined = new Map<string, number[]>();
  quiet(() => {
    for (const opp of tree.nodes) {
      for (const reply of opp.nodes) {
        const list = combined.get(reply.name) ?? [];
        list.push(reply.rating());
        combined.set(reply.name, list);
      }
    }
  });

  assertEquals(search.candidates.length, combined.size);
  for (const c of search.candidates) {
    const values = combined.get(c.key);
    assertEquals(
      values !== undefined,
      true,
      `bestTimeline has no reply ${c.key}`,
    );
    assertEquals(c.done, values!.length, `sample count ${c.key}`);
    const mean = values!.reduce((a, b) => a + b, 0) / values!.length;
    assertAlmostEquals(c.average, mean, 1e-9, `average ${c.key}`);
  }

  // Same evaluation, so the same value on top. Not necessarily the same *move*:
  // Minimax.best() breaks ties on the cheaper bet, while Search prefers one that wins
  // by knockout this round. That divergence is deliberate - see Search.ranked().
  const best = quiet(() => tree.best());
  const chosen = search.best()!;
  const byMove = new Map(search.candidates.map((c) => [c.key, c]));
  assertAlmostEquals(
    chosen.average,
    byMove.get(best.name)!.average,
    1e-9,
    `best move differs in value: ${chosen.key} vs ${best.name}`,
  );
});

Deno.test("Search reports progress before it has finished", () => {
  const search = new Search(position(true));
  assertEquals(search.units > 0, true);
  assertEquals(search.best(), undefined); // nothing known yet

  // One unit is enough to rank something, which is the whole point of the split.
  quiet(() => search.step());
  assertEquals(search.stats.unitsDone, 1);
  assertEquals(search.best() !== undefined, true);
  assertEquals(search.done, false);
});

Deno.test("striding partitions the work exactly once", () => {
  const whole = runToEnd(new Search(position(true)));

  // Four slices, as a worker pool would run them, merged back together.
  const stride = 4;
  const slices = [0, 1, 2, 3].map((offset) =>
    runToEnd(new Search(position(true), stride, offset))
  );
  assertEquals(
    slices.reduce((n, s) => n + s.stats.unitsDone, 0),
    whole.units,
    "slices must cover every unit exactly once",
  );
  for (const s of slices) assertEquals(s.stats.unitsDone, s.ownUnits);

  // Merging the per-candidate samples reproduces the single-process averages.
  for (const [i, c] of whole.candidates.entries()) {
    const merged = slices.flatMap((s) => s.candidates[i].values);
    assertEquals(slices.every((s) => s.candidates[i].key === c.key), true);
    assertEquals(merged.length, c.values.length, `sample count ${c.key}`);
    const mean = merged.reduce((a, b) => a + b, 0) / merged.length;
    assertAlmostEquals(mean, c.average, 1e-9, `merged average ${c.key}`);
  }
});

Deno.test("slices are dealt whole card pairs", () => {
  // Four cards a side make sixteen pairs of 23 x 23 units each. Three slices take six, five
  // and five pairs, so no pair's units are split between two continuation caches.
  const owned = [0, 1, 2].map((offset) =>
    new Search(opening(false), 3, offset).ownUnits
  );
  assertEquals(owned, [6 * 529, 5 * 529, 5 * 529]);
});
