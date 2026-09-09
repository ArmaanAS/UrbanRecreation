// Fetch every card at every level (data/data.json only has max-level stats, but cards are
// routinely played below max level).
//
//   deno run --env --allow-env=API_KEY,API_SECRET,NODE_EXTRA_CA_CERTS,FORCE_COLOR -RWN --allow-sys=osRelease ./scripts/RequestAllCardLevels.ts
//
// Strategy 1: characters.getCharacters with maxLevels:false — if the API honours it, one
// request returns every level of every card.
// Strategy 2 (fallback): characters.getCharacterLevels per card, batched through
// multipleQueries (see urban-rivals-oauth README).
import callAPI, { urApi } from "./UR_API.ts";
import "colors";
import cards from "@data/cards.json" with { type: "json" };

const OUT = "./data/cardsAllLevels.json";
const BATCH = 25;

console.info("Strategy 1: characters.getCharacters { maxLevels: false }".yellow);
console.time("Request");
let all: unknown[] = [];
try {
  const { items } = await callAPI("characters.getCharacters", { maxLevels: false });
  all = items;
  console.info(`  → ${items.length} rows (max-level list has ${cards.length})`.green);
} catch (e) {
  console.error("  failed:".red, e);
}
console.timeEnd("Request");

const distinctLevels = new Set(all.map((c) => `${(c as { id: number }).id}:${(c as { level: number }).level}`));
if (distinctLevels.size <= cards.length) {
  console.info("Strategy 1 returned only max levels. Strategy 2: characters.getCharacterLevels per card".yellow);
  console.time("Request levels");
  all = [];
  for (let i = 0; i < cards.length; i += BATCH) {
    const batch = cards.slice(i, i + BATCH);
    const result = await urApi.multipleQueries(
      ...batch.map((c) => ({ call: "characters.getCharacterLevels", params: { characterID: c.id } })),
    );
    // multipleQueries indexes by call name, so identical calls collapse; fall back to one-by-one if so.
    const rows = Object.values(result as Record<string, { items?: unknown[] }>).flatMap((r) => r.items ?? []);
    if (rows.length < batch.length) {
      for (const c of batch) {
        const { items } = await callAPI("characters.getCharacterLevels", { characterID: c.id });
        all.push(...items);
      }
    } else {
      all.push(...rows);
    }
    console.info(`  ${Math.min(i + BATCH, cards.length)} / ${cards.length}`.gray);
  }
  console.timeEnd("Request levels");
}

console.info(`Writing ${all.length} rows to ${OUT}`.yellow);
await Deno.writeTextFile(OUT, JSON.stringify(all));
console.info("Done".green);
