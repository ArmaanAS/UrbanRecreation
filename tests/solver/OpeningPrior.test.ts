// The round-one reply prior is a literal in both advisors. These pin the two literals to
// each other and to the rule that produced them, without letting new captures move them:
// the recount stops at the cutoff the committed table records.
import { assertEquals } from "@std/assert";
import {
  countOpeningReplies,
  loadCaptures,
  parseRustCounts,
  replaceBlock,
  rustBlock,
  typeScriptBlock,
} from "../../scripts/OpeningPrior.ts";
import {
  OPENING_REPLY_COUNTS,
  OPENING_REPLY_PROVENANCE,
  openingReplyWeight,
} from "@/solver/Search.ts";

const rustSource = await Deno.readTextFile(
  new URL("../../rust/src/advisor/search.rs", import.meta.url),
);

Deno.test("the TypeScript and Rust opening reply tables are identical", () => {
  assertEquals(parseRustCounts(rustSource), { ...OPENING_REPLY_COUNTS });
});

Deno.test("the opening reply table is the recount of its own capture cutoff", async () => {
  const prior = countOpeningReplies(
    await loadCaptures(),
    OPENING_REPLY_PROVENANCE.until,
  );
  assertEquals(prior.counts, { ...OPENING_REPLY_COUNTS });
  assertEquals(prior.plays, OPENING_REPLY_PROVENANCE.plays);
  assertEquals(prior.captures, OPENING_REPLY_PROVENANCE.captures);
  assertEquals(prior.until, OPENING_REPLY_PROVENANCE.until);
  assertEquals(prior.skipped, []);
});

Deno.test("regenerating either literal from its own recount changes nothing", async () => {
  // Pins the generator's output format too, so `--write` on an unchanged corpus is a no-op.
  const prior = countOpeningReplies(
    await loadCaptures(),
    OPENING_REPLY_PROVENANCE.until,
  );
  const tsSource = await Deno.readTextFile(
    new URL("../../src/solver/Search.ts", import.meta.url),
  );
  const date = tsSource.match(/counted (\d{4}-\d{2}-\d{2}) by/)![1];
  assertEquals(replaceBlock(tsSource, typeScriptBlock(prior, date)), tsSource);
  assertEquals(replaceBlock(rustSource, rustBlock(prior, date)), rustSource);
});

Deno.test("only the opponent's plays are counted, the owner's never", () => {
  const move = (side: number, pillz: number, fury = false) => ({
    side,
    pillz,
    fury,
  });
  const game = (
    id: number,
    mySide: number | null,
    moves: ReturnType<typeof move>[],
  ) => ({
    id,
    capturedAt: `2026-09-0${id}T00:00:00.000Z`,
    myId: mySide === null ? null : 7,
    mySide,
    players: [{ side: 0, id: 7 }, { side: 1, id: 100 + id }],
    rounds: [{ moves }],
  });
  const prior = countOpeningReplies([
    game(1, 0, [move(0, 4), move(1, 2)]),
    game(2, 1, [move(1, 9, true), move(0, 3)]),
    // No mySide: the owner is recognised by the id every other capture carries.
    game(3, null, [move(0, 6), move(1, 3)]),
    game(4, 0, []),
  ]);
  assertEquals(prior.counts, { "2 false": 1, "3 false": 2 });
  assertEquals([prior.plays, prior.captures, prior.read], [3, 3, 4]);
  assertEquals(prior.until, "2026-09-04T00:00:00.000Z");
  // A cutoff reads only captures up to it.
  assertEquals(
    countOpeningReplies(
      [game(1, 0, [move(1, 2)]), game(2, 0, [move(1, 5)])],
      "2026-09-01T00:00:00.000Z",
    ).counts,
    { "2 false": 1 },
  );
});

Deno.test("every legal reply keeps one Laplace observation", () => {
  assertEquals(openingReplyWeight({ index: 0, pillz: 12, fury: false }), 1);
  for (const [key, count] of Object.entries(OPENING_REPLY_COUNTS)) {
    const [pillz, fury] = key.split(" ");
    assertEquals(
      openingReplyWeight({
        index: 2,
        pillz: Number(pillz),
        fury: fury === "true",
      }),
      count + 1,
    );
  }
});
