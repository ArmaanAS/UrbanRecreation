// Situation coverage of the capture corpus (src/situations/).
//
//   deno task situations                      # writes data/situations.json (gitignored)
//   deno task situations --collection <path>  # the owner's collection, default data/my_collection.json
//
// Every replay-ready capture in captures/games/ is replayed, and every captured round the
// engine reproduces is probed with the server's own moves: which abilities and bonuses fired
// (by ablation), in which coarse context, which latched permanents paid, and which pairs of
// sources interacted. data/situations.json holds the count of every key and the first few
// (battle, round) examples of each; noveltyOf() in src/situations/Situations.ts scores a
// candidate round against it.
//
// With the owner's collection present it also lists the owned cards (each at its highest
// owned level) whose ability, or whose clan's bonus, no capture shows firing, grouped by clan
// - what a Training deck should carry to produce new evidence. That list goes to the terminal
// and to data/analysis/situations-owned.json (gitignored: it describes the owner's cards).
import "colors";
import {
  aggregate,
  type Collection,
  loadRecords,
  neverFired,
  replayReady,
  type ScanResult,
  scanRecord,
} from "../src/situations/Corpus.ts";

const OUT = "data/situations.json";
const OWNED_OUT = "data/analysis/situations-owned.json";

const args = [...Deno.args];
const flag = (name: string) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};
const collectionPath = flag("--collection") ?? "data/my_collection.json";

const started = performance.now();
const records = (await loadRecords("captures/games")).filter(replayReady);
const results: ScanResult[] = [];
for (const rec of records) {
  try {
    results.push(scanRecord(rec));
  } catch (error) {
    console.error(`${rec.id}: ${(error as Error).message}`.red);
  }
}
const situations = aggregate(results);
const seconds = (performance.now() - started) / 1000;
await Deno.writeTextFile(OUT, JSON.stringify(situations, null, 1) + "\n");

const keys = Object.keys(situations.counts);
const plain = (kind: string) =>
  keys.filter((k) => k.startsWith(`fired:${kind}:`) && k.split(":").length === 3);
const firedAbilities = plain("ability").filter((k) => !k.endsWith(":?"));
const firedBonuses = plain("bonus").filter((k) => !k.endsWith(":?") && k !== "fired:bonus:infiltrated");
const context = keys.filter((k) => k.startsWith("fired:") && k.split(":").length === 5);
const paid = keys.filter((k) => k.startsWith("paid:"));
const own = keys.filter((k) => k.startsWith("pair:own:"));
const opp = keys.filter((k) => k.startsWith("pair:opp:"));
const unknown = keys.filter((k) => k.includes("?"));
const disagreements = results.reduce((n, r) => n + r.baselineDisagreements, 0);

console.log(
  (`${situations.games} replay-ready games, ${situations.rounds} rounds reproduced ` +
    `(${seconds.toFixed(1)} s) -> ${OUT}`).white,
);
if (situations.mismatched.length) {
  console.log(
    `  cut short where the engine disagrees with the server: ${
      situations.mismatched.map((m) => `${m.id} r${m.round}`).join(", ")
    }`.yellow,
  );
}
if (disagreements) console.log(`  ${disagreements} rounds whose probe baseline differs from the engine`.red);
console.log(`  ${keys.length} distinct keys`.green);
console.log(`    ${firedAbilities.length} abilities and ${firedBonuses.length} clan bonuses fired`);
console.log(`    ${context.length} fired-in-context keys (win/lose x first/second)`);
console.log(`    ${paid.length} latched permanents paid`);
console.log(`    ${own.length + opp.length} interacting pairs (${opp.length} opposing, ${own.length} same side)`);
if (unknown.length) console.log(`    ${unknown.length} keys with an unresolved id: ${unknown.slice(0, 5).join(", ")}`.yellow);

let collection: Collection | undefined;
try {
  collection = JSON.parse(await Deno.readTextFile(collectionPath));
} catch {
  console.log(`\nno collection at ${collectionPath}: skipping the owned never-fired list`.gray);
}
if (collection !== undefined) {
  const { clans, missing } = neverFired(collection, situations.counts);
  const owned = Object.keys(collection.cards).length;
  const abilityCards = clans.reduce((n, c) => n + c.cards.length, 0);
  const bonusClans = clans.filter((c) => c.bonusNeverFired);
  const bonusCards = bonusClans.reduce((n, c) => n + c.owned, 0);
  // A card counts once even when both its ability and its clan bonus are unfired.
  const listed = clans.reduce(
    (n, c) => n + (c.bonusNeverFired ? c.owned : c.cards.length),
    0,
  );
  console.log(`\nOwned cards never seen firing (${owned} owned, each at its highest level):`.white);
  for (const c of clans) {
    const bonus = c.clan === "Leader"
      ? "no clan bonus".gray
      : c.bonusNeverFired
      ? `bonus "${c.bonus}" never fired (${c.owned} owned${c.owned < 2 ? ", needs 2 in a hand" : ""})`.yellow
      : "bonus fired".gray;
    console.log(`  ${c.clan.bold} - ${bonus}`);
    for (const card of c.cards) {
      const seen = card.textFired ? " (same text fired under another id)".gray : "";
      console.log(`    ${card.name} L${card.level}: ${card.ability} ${`#${card.abilityId}`.gray}${seen}`);
    }
  }
  const newText = clans.reduce((n, c) => n + c.cards.filter((card) => !card.textFired).length, 0);
  console.log(
    (`  ${listed} owned cards listed: ${abilityCards} with a never-fired ability id ` +
      `(${newText} whose text never fired under any id), ${bonusClans.length} clans ` +
      `(${bonusCards} owned cards) whose bonus never fired`).green,
  );
  if (missing.length) {
    console.log(`  ${missing.length} owned card levels missing from data/data.json`.yellow);
  }
  await Deno.mkdir("data/analysis", { recursive: true });
  await Deno.writeTextFile(
    OWNED_OUT,
    JSON.stringify(
      { generatedAt: situations.generatedAt, collectionFetchedAt: collection.fetchedAt, clans, missing },
      null,
      1,
    ) + "\n",
  );
  console.log(`  -> ${OWNED_OUT}`.gray);
}
