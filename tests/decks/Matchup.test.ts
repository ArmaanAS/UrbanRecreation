import { assert, assertAlmostEquals, assertEquals, assertNotEquals, assertRejects } from "@std/assert";
import {
  assertCurrentBinary,
  canonicalHand,
  type CardKey,
  deckVsDeck,
  FileMatchupCache,
  isRefused,
  matchupBinaryPath,
  matchupProvenance,
  type MatchupProvenance,
  type MatchupRequest,
  type MatchupResponse,
  type MatchupRunner,
  MemoryMatchupCache,
  type ProbeResponse,
  ProcessMatchupRunner,
  sampleHandPairs,
  type SolveRequest,
} from "@/decks/Matchup.ts";

const deck = (base: number, size = 8) => Array.from({ length: size }, (_, i) => ({ id: base + i, level: 1 + (i % 5) }));
const A = deck(100);
const B = deck(200);

/**
 * Answers solves from a fixed table of first-mover values: a hand's strength is the sum of its
 * card ids, so whoever holds the stronger hand wins by the difference. Any hand holding a card
 * in `refuse` is refused, as the strict catalog would.
 */
class FakeRunner implements MatchupRunner {
  readonly batches: SolveRequest[][] = [];
  constructor(
    private readonly value: (first: readonly CardKey[], second: readonly CardKey[]) => number,
    private readonly refuse = new Set<number>(),
    private readonly failOn?: number,
  ) {}

  run(requests: readonly MatchupRequest[], onResponse?: (r: MatchupResponse, i: number) => void) {
    const solves = requests as SolveRequest[];
    this.batches.push([...solves]);
    return Promise.resolve(solves.map((r, id) => {
      assertEquals(r.kind, "solve");
      assertEquals(r.first, "p1", "the first mover's hand is always sent as p1");
      let response: MatchupResponse;
      if ([...r.p1, ...r.p2].some(([card]) => card === this.failOn)) {
        response = { id, kind: "solve", error: "budget reached" };
      } else if ([...r.p1, ...r.p2].some(([card]) => this.refuse.has(card))) {
        response = { id, kind: "solve", refused: "P1 slot 0 contains unsupported Leader" };
      } else {
        response = {
          id,
          kind: "solve",
          value: this.value(r.p1, r.p2),
          worst: -1,
          best: 1,
          best_move: { hand_index: 0, pillz: 3, fury: false },
          ko_share: 0,
          koed_share: 0,
          root_moves: 92,
          replies: 92,
          ms: 1,
        };
      }
      onResponse?.(response, id);
      return response;
    }));
  }

  get solves(): SolveRequest[] {
    return this.batches.flat();
  }
}

const strength = (hand: readonly CardKey[]) => hand.reduce((s, [id]) => s + id, 0);
const byStrength = (first: readonly CardKey[], second: readonly CardKey[]) =>
  Math.max(-1, Math.min(1, (strength(first) - strength(second)) / 1000));

Deno.test("sampling is seeded, draws four distinct cards a side, and keeps B's hands whatever A is", () => {
  const one = sampleHandPairs(A, B, 30, 7);
  assertEquals(sampleHandPairs(A, B, 30, 7), one);
  assertNotEquals(sampleHandPairs(A, B, 30, 8), one);
  for (const { a, b } of one) {
    assertEquals(new Set(a.map((c) => c.id)).size, 4);
    assertEquals(new Set(b.map((c) => c.id)).size, 4);
    assert(a.every((c) => A.includes(c)) && b.every((c) => B.includes(c)));
  }
  // Common random numbers: another A deck of the same size meets exactly the same B hands, and
  // a one-card swap changes only the A hands that drew the swapped position.
  const swapped = A.map((c, i) => (i === 3 ? { id: 999, level: 2 } : c));
  const other = sampleHandPairs(swapped, B, 30, 7);
  assertEquals(other.map((p) => p.b), one.map((p) => p.b));
  one.forEach((pair, i) => {
    if (!pair.a.includes(A[3])) assertEquals(other[i].a, pair.a);
    else assert(other[i].a.some((c) => c.id === 999));
  });
  // Different streams per side: the same deck on both sides does not mirror its hands.
  const mirror = sampleHandPairs(A, A, 30, 7);
  assert(mirror.some(({ a, b }) => JSON.stringify(canonicalHand(a)) !== JSON.stringify(canonicalHand(b))));
});

Deno.test("every pair is solved with both first movers and scored (A first - B first) / 2", async () => {
  const runner = new FakeRunner(byStrength);
  const result = await deckVsDeck(A, B, { runner, n: 12, seed: 3 });
  assertEquals(result.scored, 12);
  assertEquals(result.refused, 0);
  const pairs = sampleHandPairs(A, B, 12, 3).map(({ a, b }) => ({ a: canonicalHand(a), b: canonicalHand(b) }));
  const keys = new Set(runner.solves.map((r) => JSON.stringify([r.p1, r.p2])));
  pairs.forEach(({ a, b }, i) => {
    assert(keys.has(JSON.stringify([a, b])), "A moves first with its hand as p1");
    assert(keys.has(JSON.stringify([b, a])), "B moves first with its hand as p1");
    const pair = result.pairs[i];
    assertEquals(pair.a, a);
    assertEquals(pair.b, b);
    assertEquals(pair.aFirst, byStrength(a, b));
    assertEquals(pair.bFirst, byStrength(b, a));
    assertEquals(pair.score, (pair.aFirst - pair.bFirst) / 2);
  });
  const scores = result.pairs.map((p) => p.score);
  const mean = scores.reduce((s, x) => s + x, 0) / scores.length;
  const sd = Math.sqrt(scores.reduce((s, x) => s + (x - mean) ** 2, 0) / (scores.length - 1));
  assertAlmostEquals(result.mean, mean, 1e-12);
  assertAlmostEquals(result.stderr, sd / Math.sqrt(scores.length), 1e-12);
  assertAlmostEquals(result.percent, (mean + 1) * 50, 1e-9);
  // A's cards are all weaker, so every score is negative, and the worst list is sorted.
  assert(result.mean < 0);
  assertEquals(result.worst.length, 5);
  for (let i = 1; i < result.worst.length; i++) assert(result.worst[i - 1].score <= result.worst[i].score);
  assertEquals(result.worst[0].score, Math.min(...scores));

  // Swapping the decks resamples (each side keeps its own stream), so only the sign is checked;
  // on the same pairs B's score is -score by the definition.
  const swapped = await deckVsDeck(B, A, { runner: new FakeRunner(byStrength), n: 12, seed: 3 });
  assert(swapped.mean > 0);
});

Deno.test("night, life and pillz reach the requests and the cache key", async () => {
  const runner = new FakeRunner(byStrength);
  const cache = new MemoryMatchupCache();
  await deckVsDeck(A, B, { runner, cache, n: 4, seed: 1, night: true, pillz: 10 });
  assert(runner.solves.every((r) => r.night && r.pillz === 10 && r.life === undefined));
  assert([...cache.entries.keys()].every((k) => k.startsWith("n|12|10|")));
  await deckVsDeck(A, B, { runner, cache, n: 4, seed: 1 });
  assertEquals(runner.batches.length, 2, "a day evaluation does not reuse night solves");
});

Deno.test("solved pairs are cached: a rerun sends nothing and returns the same numbers", async () => {
  const runner = new FakeRunner(byStrength);
  const cache = new MemoryMatchupCache();
  const first = await deckVsDeck(A, B, { runner, cache, n: 20, seed: 5 });
  const sent = runner.solves.length;
  assert(sent <= 40);
  assertEquals(first.solved, sent);
  assertEquals(first.cached, 0);
  const again = await deckVsDeck(A, B, { runner, cache, n: 20, seed: 5 });
  assertEquals(runner.solves.length, sent, "nothing new to solve");
  assertEquals(again.cached, sent);
  assertEquals(again.solved, 0);
  assertEquals(again.pairs, first.pairs);
  assertEquals(again.mean, first.mean);

  // A one-card swap reuses every solve whose A hand did not draw the swapped card.
  const swapped = A.map((c, i) => (i === 3 ? { id: 999, level: 2 } : c));
  const partial = await deckVsDeck(swapped, B, { runner, cache, n: 20, seed: 5 });
  assert(partial.cached > 0 && partial.solved > 0);
});

Deno.test("the file cache persists across opens and is separate per provenance", async () => {
  const dir = await Deno.makeTempDir();
  try {
    const provenance: MatchupProvenance = {
      protocol: 1,
      effective_catalog_fingerprint_fnv1a64: "0123456789abcdef",
      effect_registry_fingerprint_fnv1a64: "fedcba9876543210",
      effect_registry_schema_version: 1,
      compiler_policy_semantic_revision: 76,
      catalog_context_policy_semantic_revision: 7,
      advisor_policy_semantic_revision: 3,
      battle_rule_id: 10,
      fillers: [[123, 1]],
    };
    const runner = new FakeRunner(byStrength, new Set([104]));
    const cache = await FileMatchupCache.open(dir, provenance);
    const first = await deckVsDeck(A, B, { runner, cache, n: 10, seed: 2 });
    const sent = runner.solves.length;

    const reopened = await FileMatchupCache.open(dir, provenance);
    assertEquals(reopened.size, sent);
    const again = await deckVsDeck(A, B, { runner, cache: reopened, n: 10, seed: 2 });
    assertEquals(runner.solves.length, sent, "every solve and refusal came from the file");
    assertEquals(again.pairs, first.pairs);
    assertEquals(again.refused, first.refused);

    const newer = await FileMatchupCache.open(dir, { ...provenance, compiler_policy_semantic_revision: 77 });
    assertEquals(newer.size, 0, "an engine revision starts a fresh file");
    assertNotEquals(newer.path, reopened.path);
    const files = [...Deno.readDirSync(dir)].map((e) => e.name).sort();
    assertEquals(files.filter((f) => f.endsWith(".provenance.json")).length, 2);
  } finally {
    await Deno.remove(dir, { recursive: true });
  }
});

Deno.test("refused pairs are counted and left out of the mean", async () => {
  // Card 103 is in half of A's hands; every pair holding it is refused.
  const runner = new FakeRunner(byStrength, new Set([103]));
  const result = await deckVsDeck(A, B, { runner, n: 40, seed: 11 });
  const pairs = sampleHandPairs(A, B, 40, 11);
  const refusedCount = pairs.filter(({ a }) => a.some((c) => c.id === 103)).length;
  assert(refusedCount > 0 && refusedCount < 40);
  assertEquals(result.refused, refusedCount);
  assertEquals(result.scored, 40 - refusedCount);
  assertEquals(result.refusedPairs.length, refusedCount);
  assert(result.refusedPairs.every((p) => p.a.some(([id]) => id === 103) && p.reason.includes("Leader")));
  assert(result.pairs.every((p) => !p.a.some(([id]) => id === 103)));
  const mean = result.pairs.reduce((s, p) => s + p.score, 0) / result.pairs.length;
  assertAlmostEquals(result.mean, mean, 1e-12);

  const none = await deckVsDeck(A, B, { runner: new FakeRunner(byStrength, new Set(A.map((c) => c.id))), n: 5 });
  assertEquals(none.scored, 0);
  assertEquals(none.refused, 5);
  assert(Number.isNaN(none.mean) && Number.isNaN(none.stderr));
});

Deno.test("a solve that fails is an error, not a score, and is not cached", async () => {
  const cache = new MemoryMatchupCache();
  await assertRejects(
    () => deckVsDeck(A, B, { runner: new FakeRunner(byStrength, new Set(), 105), cache, n: 30, seed: 4 }),
    Error,
    "budget reached",
  );
  assert([...cache.entries.keys()].every((k) => !k.includes("105:")));
  await assertRejects(() => deckVsDeck(deck(1, 3), B, { runner: new FakeRunner(byStrength), n: 1 }), Error, "3 cards");
});

const binaryAvailable = await (async () => {
  try {
    return (await Deno.stat(matchupBinaryPath())).isFile;
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) return false;
    throw error;
  }
})();

Deno.test({
  name: "the release matchup binary answers in order and matches this checkout's provenance",
  ignore: !binaryAvailable,
  async fn() {
    const runner = new ProcessMatchupRunner({ threads: 2 });
    const provenance = await matchupProvenance(runner);
    await assertCurrentBinary(provenance);
    const demo2: CardKey[] = [[441, 1], [444, 1], [445, 1], [447, 1]];
    const responses = await runner.run([
      { kind: "probe", card: [123, 1], night: false },
      { kind: "solve", p1: [[269, 5], [124, 1], [138, 1], [139, 1]], p2: demo2, first: "p1", night: false },
      { kind: "probe", card: [269, 1], night: true },
    ]);
    assertEquals(responses.map((r) => r.id), [0, 1, 2]);
    assertEquals((responses[0] as ProbeResponse).status, "exact");
    assert(isRefused(responses[1]) && responses[1].refused.includes("Leader"));
    assertEquals((responses[2] as ProbeResponse).status, "leader");
  },
});
