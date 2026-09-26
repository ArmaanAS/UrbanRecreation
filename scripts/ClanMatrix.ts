// Rank the clans of one format against each other on the exact Rust solver, from the hands
// actually played there, next to how they did in the captured games.
//
//   deno task rust:matchup                            # build the solver once
//   deno task clan-matrix                             # Tourney, day, 16 hand pairs per clan pair
//   deno task clan-matrix --format EFC --night --per-cell 30 --min 5
//
// Writes data/analysis/clan-matrix-<format id>-<day|night>.json, which Deck Lab shows. Solves are
// cached under cache/matchups/ like `deno task matchup`, so a rerun or a larger --per-cell only
// pays for the new pairs. See src/decks/ClanMatrix.ts for what the numbers mean.
import "colors";
import { type ClanMatrixResult, clanGroups, clanMatrix, fieldHands, type MatrixGameRecord } from "../src/decks/ClanMatrix.ts";
import {
  assertCurrentBinary,
  FileMatchupCache,
  type MatchupCache,
  matchupProvenance,
  MemoryMatchupCache,
  ProcessMatchupRunner,
} from "../src/decks/Matchup.ts";

/** The owner's player id, for the few old captures that do not say which side was theirs. */
const OWNER_ID = 19309601;
const USAGE =
  "usage: deno task clan-matrix [--format Tourney] [--night] [--per-cell 16] [--min 7] [--seed 1] [--threads N] [--no-cache]";

function parse(args: string[]) {
  const options = { format: "Tourney", night: false, perCell: 16, min: 7, seed: 1, threads: 0, cache: true };
  for (let i = 0; i < args.length; i++) {
    const flag = args[i];
    const value = () => {
      const v = args[++i];
      if (v === undefined) throw new Error(`${flag} needs a value\n${USAGE}`);
      return v;
    };
    const count = (v: string, min: number) => {
      const x = Number(v);
      if (!Number.isInteger(x) || x < min) throw new Error(`${flag} must be an integer >= ${min}`);
      return x;
    };
    switch (flag) {
      case "--format":
        options.format = value();
        break;
      case "--night":
        options.night = true;
        break;
      case "--per-cell":
        options.perCell = count(value(), 1);
        break;
      case "--min":
        options.min = count(value(), 1);
        break;
      case "--seed":
        options.seed = count(value(), 0);
        break;
      case "--threads":
        options.threads = count(value(), 1);
        break;
      case "--no-cache":
        options.cache = false;
        break;
      default:
        throw new Error(`unknown argument ${flag}\n${USAGE}`);
    }
  }
  return options;
}

let options: ReturnType<typeof parse>;
try {
  options = parse(Deno.args);
} catch (error) {
  console.error((error as Error).message);
  Deno.exit(2);
}

const formats = JSON.parse(await Deno.readTextFile("data/deck_formats.json")).formats as { id: number; name: string }[];
const format = formats.find((f) => String(f.id) === options.format || f.name.toLowerCase() === options.format.toLowerCase());
if (!format) {
  console.error(`no deck format ${JSON.stringify(options.format)}; known: ${formats.map((f) => `${f.name} (${f.id})`).join(", ")}`);
  Deno.exit(1);
}

const games: MatrixGameRecord[] = [];
for await (const entry of Deno.readDir("captures/games")) {
  if (entry.isFile && entry.name.endsWith(".json")) {
    games.push(JSON.parse(await Deno.readTextFile(`captures/games/${entry.name}`)));
  }
}
const hands = fieldHands(games, format.id, OWNER_ID);
const groups = clanGroups(hands, options.min);
if (groups.length < 2) {
  console.error(`only ${groups.length} clans have ${options.min}+ captured ${format.name} hands; lower --min`);
  Deno.exit(1);
}

const runner = new ProcessMatchupRunner(options.threads ? { threads: options.threads } : {});
let cache: MatchupCache;
let provenance;
try {
  provenance = await matchupProvenance(runner);
  await assertCurrentBinary(provenance);
  cache = options.cache ? await FileMatchupCache.open("cache/matchups", provenance) : new MemoryMatchupCache();
} catch (error) {
  console.error((error as Error).message);
  Deno.exit(1);
}

const cellCount = (groups.length * (groups.length - 1)) / 2;
console.log(
  `${format.name}, ${options.night ? "night" : "day"}: ${groups.length} clans with ${options.min}+ of the ${hands.length} ` +
    `clan hands captured, ${cellCount} clan pairs x ${options.perCell} hand pairs, seed ${options.seed}`,
);
const started = performance.now();
const encoder = new TextEncoder();
let result: ClanMatrixResult;
try {
  result = await clanMatrix(groups, {
    runner,
    cache,
    perCell: options.perCell,
    seed: options.seed,
    night: options.night,
    onProgress: (done, total) => {
      if (Deno.stderr.isTerminal() && (done % 25 === 0 || done === total)) {
        const eta = ((performance.now() - started) / 1000 / done) * (total - done);
        Deno.stderr.writeSync(encoder.encode(`\r  solving ${done}/${total}, about ${Math.ceil(eta / 60)} min left `));
      }
    },
  });
} finally {
  await cache.flush();
}
if (Deno.stderr.isTerminal()) Deno.stderr.writeSync(encoder.encode("\r\x1b[K"));
const seconds = (performance.now() - started) / 1000;

const out = `data/analysis/clan-matrix-${format.id}-${options.night ? "night" : "day"}.json`;
await Deno.mkdir("data/analysis", { recursive: true });
await Deno.writeTextFile(
  out,
  JSON.stringify(
    {
      generatedAt: new Date().toISOString(),
      format: { id: format.id, name: format.name },
      minHands: options.min,
      handsCaptured: hands.length,
      games: new Set(hands.map((h) => h.gameId)).size,
      provenance,
      ...result,
    },
    null,
    1,
  ) + "\n",
);

const pct = (x: number) => (Number.isFinite(x) ? `${((x + 1) * 50).toFixed(1)}%` : "  -  ");
const err = (x: number) => (Number.isFinite(x) ? `±${(x * 50).toFixed(1)}` : "");
console.log(`${out}: ${result.solved} new solves, ${result.cached} from cache, ${(seconds / 60).toFixed(1)} min`);
console.log(`  ${"clan".padEnd(12)} ${"vs field".padStart(14)} ${"vs clans".padStart(14)}  hands (owner's)  solved  practice`);
for (const c of result.clans) {
  const practice = c.practice.games ? `${c.practice.score}/${c.practice.games} games` : "";
  console.log(
    `  ${c.clan.padEnd(12)} ${`${pct(c.vsField)} ${err(c.vsFieldErr)}`.padStart(14)} ${
      `${pct(c.vsClans)} ${err(c.vsClansErr)}`.padStart(14)
    }  ${String(c.hands).padStart(4)} (${String(c.ownerHands).padStart(2)})       ${
      `${Math.round((100 * c.scored) / Math.max(1, c.scored + c.refused))}%`.padStart(4)
    }    ${practice}`,
  );
}
const lopsided = result.cells.filter((c) => c.scored >= options.perCell / 2)
  .sort((x, y) => Math.abs(y.mean) - Math.abs(x.mean)).slice(0, 10);
console.log("  most one-sided clan pairs:");
for (const c of lopsided) {
  const [winner, loser, mean] = c.mean >= 0 ? [c.a, c.b, c.mean] : [c.b, c.a, -c.mean];
  console.log(`    ${winner} over ${loser}: ${pct(mean)} ${err(c.stderr)} (${c.scored} pairs)`.gray);
}
const refused = result.cells.reduce((s, c) => s + c.refused, 0);
if (refused) {
  console.log(`  ${refused} hand pairs refused; most common reasons:`);
  for (const r of result.refusals.slice(0, 5)) console.log(`    ${r.count} x ${r.reason}`.gray);
}
console.log(
  "  Exact solves with both first movers, the advisor's conservative policy; hands from the owner's own matches.".gray,
);
