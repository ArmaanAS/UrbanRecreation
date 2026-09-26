// Improve one of your decks from your own collection, a slot at a time, against the captured
// opponents of a format (phase 7 of docs/deck-builder-design.md).
//
//   deno task rust:matchup                                  # build the solver once
//   deno task deck-search --deck "T1 Rescue"                # Tourney field, 30 hands, 2 passes
//   deno task deck-search --deck 17690254 --scope deck --n 40 --passes 3 --night
//   deno task deck-search --deck "T1 Rescue" --only-format    # only the field's format stays legal
//
// By default every format the deck is legal in now stays legal (a 25-star deck legal in both
// EFC and Tourney keeps EFC's cap and level rules); --only-format keeps only the field's format.
//
// Each pass runs Deck Lab's ⇄ on every slot in turn (src/decks/Swap.ts) and keeps the best
// candidate only when its gain is more than twice its standard error. The seed is fixed, so the
// gains are measured on the same opposing hands throughout, which the search can overfit; the
// result is therefore re-measured against the start on a second seed it never saw. Writes
// data/analysis/deck-search-<deck id>-<format id>-<day|night>.json, which Deck Lab offers as a
// draft to load. Nothing is written to the site.
import "colors";
import { compactCoverage, type CoverageFile } from "../src/decks/Coverage.ts";
import {
  assertCurrentBinary,
  type DeckCardRef,
  FileMatchupCache,
  isRefusedPair,
  matchupProvenance,
  type PairResult,
  ProcessMatchupRunner,
  sampleFieldPairs,
  solvePairs,
  summarize,
} from "../src/decks/Matchup.ts";
import { formatHands, type GameRecord } from "../src/decks/Meta.ts";
import { type DeckCatalog, deckReport } from "../src/decks/Report.ts";
import type { DeckCard, DeckFormatData, OwnedCopies, SiteCard, SiteDeck } from "../src/decks/SiteData.ts";
import { ownedCandidates, swapSearch } from "../src/decks/Swap.ts";

const OWNER_ID = 19309601;
const SEARCH_SEED = 1;
const CHECK_SEED = 2;
const USAGE =
  "usage: deno task deck-search --deck <name or id> [--format Tourney] [--n 30] [--passes 2] [--scope clan|deck] [--night] [--only-format]";

function parse(args: string[]) {
  const options = { deck: "", format: "Tourney", n: 30, passes: 2, scope: "clan" as "clan" | "deck", night: false, onlyFormat: false };
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
      case "--deck":
        options.deck = value();
        break;
      case "--format":
        options.format = value();
        break;
      case "--n":
        options.n = count(value(), 4);
        break;
      case "--passes":
        options.passes = count(value(), 1);
        break;
      case "--scope": {
        const scope = value();
        if (scope !== "clan" && scope !== "deck") throw new Error(`--scope is clan or deck\n${USAGE}`);
        options.scope = scope;
        break;
      }
      case "--night":
        options.night = true;
        break;
      case "--only-format":
        options.onlyFormat = true;
        break;
      default:
        throw new Error(`unknown argument ${flag}\n${USAGE}`);
    }
  }
  if (!options.deck) throw new Error(USAGE);
  return options;
}

const readJson = async (path: string) => JSON.parse(await Deno.readTextFile(path));
async function readOptional(path: string) {
  try {
    return await readJson(path);
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) return undefined;
    throw error;
  }
}

let options: ReturnType<typeof parse>;
try {
  options = parse(Deno.args);
} catch (error) {
  console.error((error as Error).message);
  Deno.exit(2);
}

const [formatsFile, cardsFile, collectionFile, decksFile, coverageFile] = await Promise.all([
  readJson("data/deck_formats.json"),
  readOptional("data/site_cards.json"),
  readOptional("data/my_collection.json"),
  readOptional("data/my_decks.json"),
  readOptional("data/card_coverage.json"),
]);
if (!cardsFile || !collectionFile || !decksFile) {
  console.error("needs data/site_cards.json, data/my_collection.json and data/my_decks.json: open Collection Pro with the log server running");
  Deno.exit(1);
}
const formats = formatsFile.formats as DeckFormatData[];
const format = formats.find((f) => String(f.id) === options.format || f.name.toLowerCase() === options.format.toLowerCase());
if (!format) {
  console.error(`no deck format ${JSON.stringify(options.format)}; known: ${formats.map((f) => `${f.name} (${f.id})`).join(", ")}`);
  Deno.exit(1);
}
const decks = decksFile.decks as SiteDeck[];
const start = decks.find((d) => String(d.id) === options.deck) ??
  decks.find((d) => d.name.toLowerCase() === options.deck.toLowerCase());
if (!start) {
  console.error(`no deck ${JSON.stringify(options.deck)}; your decks: ${decks.map((d) => d.name).join(", ")}`);
  Deno.exit(1);
}
const catalog: DeckCatalog = {
  cards: new Map((cardsFile.cards as SiteCard[]).map((c) => [c.id, c])),
  formats,
  owned: new Map(Object.entries(collectionFile.cards as Record<string, OwnedCopies>).map(([id, o]) => [Number(id), o])),
};
const coverage = coverageFile ? compactCoverage(coverageFile as CoverageFile) : undefined;
if (!coverage) console.log("no data/card_coverage.json: candidates the solver cannot score will be tried and refused".yellow);

const games: GameRecord[] = [];
for await (const entry of Deno.readDir("captures/games")) {
  if (entry.isFile && entry.name.endsWith(".json")) games.push(await readJson(`captures/games/${entry.name}`));
}
const hands = formatHands(games, format.id, OWNER_ID);
if (hands.length < options.n) {
  console.error(`only ${hands.length} captured ${format.name} opposing hands; lower --n`);
  Deno.exit(1);
}

const runner = new ProcessMatchupRunner();
const provenance = await matchupProvenance(runner);
await assertCurrentBinary(provenance);
const cache = await FileMatchupCache.open("cache/matchups", provenance);
const solve = { runner, cache, night: options.night };
const name = ({ id, level }: DeckCardRef) => `${catalog.cards.get(id)?.name ?? `#${id}`} L${level}`;
const pp = (x: number) => `${x >= 0 ? "+" : "-"}${Math.abs(x * 50).toFixed(1)}`;

console.log(
  `${start.name.bold}: ${start.characters.map(name).join(", ")}\n` +
    `against ${options.n} of the ${hands.length} captured ${format.name} opponents, ${options.night ? "night" : "day"}, ` +
    `candidates from the ${options.scope === "clan" ? "slot's clan" : "deck's clans"}, up to ${options.passes} passes`,
);
const keepFormats = options.onlyFormat ? [format.id] : formats.map((f) => f.id);
const legalNow = deckReport(start.characters, catalog, options.night).formats.filter((v) => v.legal === true)
  .filter((v) => keepFormats.includes(v.formatId)).map((v) => v.name);
console.log(`  stays legal in: ${legalNow.length ? legalNow.join(", ") : "nothing (the deck is legal in none of them now)"}`);
const started = performance.now();
let deck: DeckCard[] = start.characters.map((c) => ({ ...c }));
const swaps: { pass: number; slot: number; out: DeckCard; in: DeckCard; gain: number; gainErr: number; tried: number }[] = [];
for (let pass = 1; pass <= options.passes; pass++) {
  let changed = false;
  for (let slot = 0; slot < deck.length; slot++) {
    const candidates = ownedCandidates(deck, slot, catalog, { scope: options.scope, keepFormats, night: options.night, coverage });
    if (!candidates.length) continue;
    const found = await swapSearch(deck, slot, candidates, (cards) => sampleFieldPairs(cards, hands, options.n, SEARCH_SEED), solve);
    const best = found.candidates[0];
    const minutes = ((performance.now() - started) / 60000).toFixed(0);
    const keep = best && Number.isFinite(best.diff) && best.diff - 2 * best.diffErr > 0 && best.refused <= found.base.refused;
    console.log(
      `  pass ${pass}, ${name(deck[slot]).padEnd(18)} deck ${((found.base.mean + 1) * 50).toFixed(1)}%, best of ` +
        `${candidates.length}: ${best ? `${name(best.card)} ${pp(best.diff)} ±${(best.diffErr * 50).toFixed(1)}` : "none"}` +
        `${keep ? "  -> swapped".green : ""}  (${minutes} min)`,
    );
    if (keep) {
      swaps.push({ pass, slot, out: deck[slot], in: best.card, gain: best.diff, gainErr: best.diffErr, tried: candidates.length });
      deck = deck.map((c, i) => (i === slot ? best.card : c));
      changed = true;
    }
  }
  if (!changed) break;
}

// The search chose on seed 1's hands; measure the change on hands it never saw.
const [before, after] = [start.characters, deck].map((cards) => sampleFieldPairs(cards, hands, options.n, CHECK_SEED));
const { outcomes } = await solvePairs([...before, ...after], solve);
const was = outcomes.slice(0, before.length), now = outcomes.slice(before.length);
const diffs = now.flatMap((o, i) => (isRefusedPair(o) || isRefusedPair(was[i]) ? [] : [(o as PairResult).score - (was[i] as PairResult).score]));
const gain = diffs.reduce((s, d) => s + d, 0) / Math.max(1, diffs.length);
const gainErr = Math.sqrt(diffs.reduce((s, d) => s + (d - gain) ** 2, 0) / Math.max(1, diffs.length - 1) / Math.max(1, diffs.length));
const check = { seed: CHECK_SEED, before: summarize(was).mean, after: summarize(now).mean, gain, gainErr, pairs: diffs.length };

const out = `data/analysis/deck-search-${start.id}-${format.id}-${options.night ? "night" : "day"}${
  options.onlyFormat ? "-only-format" : ""
}.json`;
await Deno.mkdir("data/analysis", { recursive: true });
await Deno.writeTextFile(
  out,
  JSON.stringify(
    {
      generatedAt: new Date().toISOString(),
      format: { id: format.id, name: format.name },
      night: options.night,
      n: options.n,
      scope: options.scope,
      keptLegal: legalNow,
      start: { id: start.id, name: start.name, characters: start.characters },
      characters: deck,
      swaps,
      check,
      provenance,
    },
    null,
    1,
  ) + "\n",
);
console.log(`${out}, ${((performance.now() - started) / 60000).toFixed(0)} min`);
if (!swaps.length) console.log("  no swap gained more than twice its error: the deck stays as it is");
for (const s of swaps) console.log(`  ${name(s.out)} -> ${name(s.in)}: ${pp(s.gain)} ±${(s.gainErr * 50).toFixed(1)} on the search hands`);
console.log(
  `  on ${check.pairs} hands the search never saw: ${((check.before + 1) * 50).toFixed(1)}% -> ${((check.after + 1) * 50).toFixed(1)}%, ` +
    `change ${pp(check.gain)} ±${(check.gainErr * 50).toFixed(1)}`,
);
console.log(`  final: ${deck.map(name).join(", ")}`);
