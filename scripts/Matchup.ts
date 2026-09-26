// Score one of your decks against another on the exact Rust solver.
//
//   deno task rust:matchup                                  # build the solver once
//   deno task matchup --a "T1 Rescue" --b "T1 Riots"        # N=100 hand pairs, seed 1, day
//   deno task matchup --a 17690254 --b "T1 Riots" --n 40 --seed 7 --night
//
// Decks come from data/my_decks.json (written by the log server when Collection Pro loads, or
// by `deno task deck-data`), by id or by name. Every hand pair is solved with both first movers;
// see src/decks/Matchup.ts for the score. Solves are cached under cache/matchups/, so a rerun,
// another seed's overlap or a comparison sharing hands costs only the new solves.
// Other flags: --threads N (default every hardware thread), --no-cache, --worst K.
import "colors";
import {
  assertCurrentBinary,
  type CardKey,
  deckVsDeck,
  FileMatchupCache,
  type MatchupCache,
  matchupProvenance,
  MemoryMatchupCache,
  ProcessMatchupRunner,
} from "../src/decks/Matchup.ts";
import type { SiteCard, SiteDeck } from "../src/decks/SiteData.ts";

const USAGE =
  "usage: deno task matchup --a <deck name or id> --b <deck name or id> [--n 100] [--seed 1] [--night] [--threads N] [--worst 5] [--no-cache]";

function parse(args: string[]) {
  const options = { a: "", b: "", n: 100, seed: 1, night: false, threads: 0, worst: 5, cache: true };
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
      case "--a":
        options.a = value();
        break;
      case "--b":
        options.b = value();
        break;
      case "--n":
        options.n = count(value(), 1);
        break;
      case "--seed":
        options.seed = count(value(), 0);
        break;
      case "--threads":
        options.threads = count(value(), 1);
        break;
      case "--worst":
        options.worst = count(value(), 0);
        break;
      case "--night":
        options.night = true;
        break;
      case "--no-cache":
        options.cache = false;
        break;
      default:
        throw new Error(`unknown argument ${flag}\n${USAGE}`);
    }
  }
  if (!options.a || !options.b) throw new Error(USAGE);
  return options;
}

function findDeck(decks: SiteDeck[], query: string): SiteDeck {
  const byId = decks.find((d) => String(d.id) === query);
  if (byId) return byId;
  const lower = query.toLowerCase();
  const exact = decks.filter((d) => d.name.toLowerCase() === lower);
  const matches = exact.length ? exact : decks.filter((d) => d.name.toLowerCase().includes(lower));
  if (matches.length === 1) return matches[0];
  const names = decks.map((d) => `  ${d.id}  ${d.name} (${d.characters.length})`).join("\n");
  throw new Error(
    `${matches.length ? "several decks match" : "no deck matches"} ${JSON.stringify(query)}; your decks:\n${names}`,
  );
}

async function readJson(path: string) {
  try {
    return JSON.parse(await Deno.readTextFile(path));
  } catch {
    return undefined;
  }
}

const options = (() => {
  try {
    return parse(Deno.args);
  } catch (error) {
    console.error((error as Error).message);
    Deno.exit(2);
  }
})();

const decksFile = await readJson("data/my_decks.json");
if (!decksFile?.decks?.length) {
  console.error("data/my_decks.json has no decks: open Collection Pro with the log server running, or run `deno task deck-data`.");
  Deno.exit(1);
}
const decks = decksFile.decks as SiteDeck[];
let deckA: SiteDeck, deckB: SiteDeck;
try {
  deckA = findDeck(decks, options.a);
  deckB = findDeck(decks, options.b);
} catch (error) {
  console.error((error as Error).message);
  Deno.exit(1);
}

const siteCards = await readJson("data/site_cards.json");
const names = new Map<number, string>((siteCards?.cards as SiteCard[] | undefined)?.map((c) => [c.id, c.name]) ?? []);
const cardText = ([id, level]: CardKey) => `${names.get(id) ?? `#${id}`} L${level}`;
const handText = (hand: readonly CardKey[]) => hand.map(cardText).join(", ");
const signed = (x: number) => (x >= 0 ? "+" : "") + x.toFixed(3);

const runner = new ProcessMatchupRunner(options.threads ? { threads: options.threads } : {});
let cache: MatchupCache;
try {
  const provenance = await matchupProvenance(runner);
  await assertCurrentBinary(provenance);
  cache = options.cache ? await FileMatchupCache.open("cache/matchups", provenance) : new MemoryMatchupCache();
} catch (error) {
  console.error((error as Error).message);
  Deno.exit(1);
}

console.log(
  `${deckA.name.bold} (${deckA.characters.length} cards) vs ${deckB.name.bold} (${deckB.characters.length} cards), ` +
    `${options.night ? "night" : "day"}, ${options.n} hand pairs, seed ${options.seed}`,
);
const started = performance.now();
const result = await deckVsDeck(deckA.characters, deckB.characters, {
  runner,
  cache,
  n: options.n,
  seed: options.seed,
  night: options.night,
  worst: options.worst,
  onProgress: (done, total) => {
    if (Deno.stderr.isTerminal()) Deno.stderr.writeSync(new TextEncoder().encode(`\r  solving ${done}/${total}`));
  },
});
if (Deno.stderr.isTerminal()) Deno.stderr.writeSync(new TextEncoder().encode("\r\x1b[K"));
const seconds = (performance.now() - started) / 1000;

if (result.scored) {
  const halfWidth = Number.isFinite(result.stderr) ? ` ± ${result.stderr.toFixed(3)}` : "";
  const percentHalfWidth = Number.isFinite(result.stderr) ? ` ± ${(result.stderr * 50).toFixed(1)}` : "";
  console.log(
    `  ${deckA.name} scores ${signed(result.mean)}${halfWidth} ` +
      `(${result.percent.toFixed(1)}%${percentHalfWidth} on the advisor's scale; 50% is even)`,
  );
} else {
  console.log("  nothing could be scored".red);
}
console.log(
  `  scored ${result.scored} pairs, refused ${result.refused}` +
    (result.refused ? ` (the strict catalog's reasons; P1 is ${deckA.name}'s hand)` : ""),
);
if (result.refused) {
  const reasons = new Map<string, number>();
  for (const pair of result.refusedPairs) reasons.set(pair.reason, (reasons.get(pair.reason) ?? 0) + 1);
  for (const [reason, count] of [...reasons].sort((x, y) => y[1] - x[1]).slice(0, 5)) {
    console.log(`    ${count} x ${reason}`.gray);
  }
}
console.log(`  ${result.solved} new solves, ${result.cached} from cache, ${seconds.toFixed(1)} s`);
if (result.worst.length) {
  console.log(`  worst hands for ${deckA.name}:`);
  for (const pair of result.worst) {
    console.log(
      `    ${signed(pair.score)}  ${handText(pair.a)}  vs  ${handText(pair.b)}  ` +
        `(A first ${signed(pair.aFirst)}, B first ${signed(pair.bFirst)})`.gray,
    );
  }
}
console.log(
  "  Conservative advisor policy for the first mover, opening reply weighted by the captured prior; not an equilibrium."
    .gray,
);
