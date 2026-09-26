// Recount the round-one reply prior both advisors weight the opponent's opening by.
//
//   deno task opening-prior            # print the table the current captures give
//   deno task opening-prior --write    # ...and rewrite the literal in both advisors
//   deno task opening-prior --until 2026-09-23T20:58:52.100Z   # only captures up to then
//
// The table is a literal on purpose: adding captures does not move a recommendation until
// someone reruns this and commits the result. `OPENING_REPLY_PROVENANCE` in Search.ts
// records the capture cutoff of the committed table, and tests/solver/OpeningPrior.test.ts
// recounts the corpus up to that cutoff, so the literal cannot silently drift from its rule.
//
// The rule: every opponent round-one move (rounds[0].moves) of every capture, in every room
// and battle rule, for both movers. "Opponent" is the side that is not the capture's
// mySide; the oldest captures have no mySide, and there the owner is recognised by the
// myId every other capture carries. Each play is keyed by engine pillz (server pillzUsed
// - 1: the free pill and Fury's three pillz are left out) and the Fury flag. Both advisors
// add one Laplace observation to every legal action, so an unseen play stays possible.
//
// The first table (198 plays, 2026-09-13) counted both sides of the first 101 captures, so
// about half of it was the owner's own openings; that was the reason for the recount.

const GAME_DIR = new URL("../captures/games/", import.meta.url);
const TS_TABLE = new URL("../src/solver/Search.ts", import.meta.url);
const RUST_TABLE = new URL("../rust/src/advisor/search.rs", import.meta.url);
const BEGIN = "BEGIN OPENING_REPLY_COUNTS";
const END = "END OPENING_REPLY_COUNTS";

interface CaptureMove {
  side: number;
  pillz: number;
  fury: boolean;
}

interface CaptureRecord {
  id: number;
  capturedAt: string;
  myId: number | null;
  mySide: number | null;
  players: { side: number; id: number }[];
  rounds: { moves: CaptureMove[] }[];
}

export interface OpeningPrior {
  /** `"pillz fury"` → opponent plays, in ascending pillz, plain before Fury. */
  counts: Record<string, number>;
  plays: number;
  /** Captures that contributed at least one opponent play. */
  captures: number;
  /** Captures read, including those without a usable round one. */
  read: number;
  /** capturedAt of the newest capture read. */
  until: string;
  /** Captures whose owner could not be identified, so none of their moves were counted. */
  skipped: number[];
}

export async function loadCaptures(): Promise<CaptureRecord[]> {
  const games: CaptureRecord[] = [];
  for await (const entry of Deno.readDir(GAME_DIR)) {
    if (!entry.isFile || !entry.name.endsWith(".json")) continue;
    games.push(
      JSON.parse(await Deno.readTextFile(new URL(entry.name, GAME_DIR))),
    );
  }
  return games.sort((a, b) =>
    a.capturedAt < b.capturedAt ? -1 : a.capturedAt > b.capturedAt ? 1 : 0
  );
}

/** Count opponent round-one plays over `games` captured at or before `until`. */
export function countOpeningReplies(
  games: CaptureRecord[],
  until?: string,
): OpeningPrior {
  const read = games.filter((g) =>
    until === undefined || g.capturedAt <= until
  );
  const owners = new Set(
    read.flatMap((g) => (g.myId === null ? [] : [g.myId])),
  );
  const counts = new Map<string, number>();
  const skipped: number[] = [];
  let plays = 0, captures = 0;
  for (const game of read) {
    let ownSide = game.mySide;
    if (ownSide === null) {
      const own = game.players.filter((p) => owners.has(p.id));
      if (own.length !== 1) {
        skipped.push(game.id);
        continue;
      }
      ownSide = own[0].side;
    }
    let counted = 0;
    for (const move of game.rounds[0]?.moves ?? []) {
      if (move.side === ownSide) continue;
      const key = `${move.pillz} ${move.fury}`;
      counts.set(key, (counts.get(key) ?? 0) + 1);
      counted++;
    }
    plays += counted;
    if (counted > 0) captures++;
  }
  const order = (key: string) => {
    const [pillz, fury] = key.split(" ");
    return Number(pillz) * 2 + (fury === "true" ? 1 : 0);
  };
  const sorted = [...counts.keys()].sort((a, b) => order(a) - order(b));
  return {
    counts: Object.fromEntries(sorted.map((key) => [key, counts.get(key)!])),
    plays,
    captures,
    read: read.length,
    until: read.at(-1)?.capturedAt ?? "",
    skipped,
  };
}

function provenance(prior: OpeningPrior, date: string): string[] {
  return [
    `// ${prior.plays} captured opponent round-one plays from ${prior.captures} of the ${prior.read} captures`,
    `// up to ${prior.until}, counted ${date} by \`deno task opening-prior\`: every`,
    "// opponent round-one move, every room and battle rule, both movers, keyed by engine pillz",
    "// (server pillzUsed - 1) and the Fury flag. Rerun that task to refresh it; adding",
    "// captures does not change it by itself.",
  ];
}

/** The generated block for Search.ts, between (and excluding) the marker lines. */
export function typeScriptBlock(prior: OpeningPrior, date: string): string[] {
  return [
    ...provenance(prior, date),
    "export const OPENING_REPLY_PROVENANCE = {",
    `  plays: ${prior.plays},`,
    `  captures: ${prior.captures},`,
    `  until: "${prior.until}",`,
    "} as const;",
    "export const OPENING_REPLY_COUNTS: Readonly<Record<string, number>> = {",
    ...Object.entries(prior.counts).map(([key, n]) => `  "${key}": ${n},`),
    "};",
  ];
}

/** The generated block for search.rs, between (and excluding) the marker lines. */
export function rustBlock(prior: OpeningPrior, date: string): string[] {
  return [
    ...provenance(prior, date),
    "const OPENING_REPLY_COUNTS: &[((u16, bool), u16)] = &[",
    ...Object.entries(prior.counts).map(([key, n]) => {
      const [pillz, fury] = key.split(" ");
      return `    ((${pillz}, ${fury}), ${n}),`;
    }),
    "];",
  ];
}

/** Replace the lines between the two marker comments, keeping the file's line endings. */
export function replaceBlock(source: string, block: string[]): string {
  const eol = source.includes("\r\n") ? "\r\n" : "\n";
  const lines = source.split(eol);
  const begin = lines.findIndex((line) => line.includes(BEGIN));
  const end = lines.findIndex((line) => line.includes(END));
  if (begin < 0 || end <= begin) {
    throw new Error(`no ${BEGIN} ... ${END} block`);
  }
  return [...lines.slice(0, begin + 1), ...block, ...lines.slice(end)].join(
    eol,
  );
}

/** Parse the committed Rust literal back into the TypeScript record shape. */
export function parseRustCounts(source: string): Record<string, number> {
  const start = source.indexOf(BEGIN), stop = source.indexOf(END);
  if (start < 0 || stop <= start) throw new Error(`no ${BEGIN} block`);
  const counts: Record<string, number> = {};
  for (
    const [, pillz, fury, n] of source.slice(start, stop).matchAll(
      /\(\((\d+), (true|false)\), (\d+)\)/g,
    )
  ) counts[`${pillz} ${fury}`] = Number(n);
  return counts;
}

if (import.meta.main) {
  const args = [...Deno.args];
  const write = args.includes("--write");
  const untilAt = args.indexOf("--until");
  const until = untilAt >= 0 ? args[untilAt + 1] : undefined;
  const prior = countOpeningReplies(await loadCaptures(), until);
  const date = new Date().toISOString().slice(0, 10);
  const shares = (n: number) => {
    // 13 plain and 10 Fury actions with the corpus's twelve pillz.
    const legal = 23;
    return ((n + 1) / (prior.plays + legal) * 100).toFixed(1);
  };
  console.log(
    `${prior.plays} opponent plays from ${prior.captures} of ${prior.read} captures up to ${prior.until}`,
  );
  if (prior.skipped.length > 0) {
    console.log(`owner unknown, skipped: ${prior.skipped.join(", ")}`);
  }
  for (const [key, n] of Object.entries(prior.counts)) {
    const [pillz, fury] = key.split(" ");
    console.log(
      `${pillz.padStart(3)} ${fury === "true" ? "fury" : "    "} ${
        String(n).padStart(4)
      }  ${shares(n).padStart(5)}%`,
    );
  }
  if (write) {
    for (
      const [url, block] of [
        [TS_TABLE, typeScriptBlock(prior, date)],
        [RUST_TABLE, rustBlock(prior, date)],
      ] as const
    ) {
      await Deno.writeTextFile(
        url,
        replaceBlock(await Deno.readTextFile(url), block),
      );
      console.log(`rewrote ${url.pathname}`);
    }
  }
}
