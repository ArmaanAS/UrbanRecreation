// Build data/data.json (the card list the engine loads) from the site's own card database.
//
//   deno task cards
//
// Input:  data/site_characters.jsonl  — written by log_server.ts when you run
//         `__ur.dumpCharacters()` in the browser console on urban-rivals.com
//         data/site_clans.json         — optional, from `__ur.dumpClans()`; falls back to the
//         clan names / bonuses already present in data/cards.json
// Output: data/data.json with one row per card per level (power, damage and ability differ
//         by level), in the same shape the engine's CardJSON expects.
import "colors";
import { ClanIdMap } from "@/game/types/CardTypes.ts";

const SITE_CHARACTERS = "./data/site_characters.jsonl";
const SITE_CLANS = "./data/site_clans.json";
const LEGACY_CARDS = "./data/cards.json";
const LEGACY_DATA = "./data/data.json";
const OUT = "./data/data.json";

interface SiteAbility {
  id: number;
  unlockLevel: number;
  description: Record<string, string>;
}
interface SiteEvo {
  power: number;
  damage: number;
  ability: SiteAbility;
  nightAbility: SiteAbility | [];
}
interface SiteCharacter {
  id: number;
  name: string;
  clanId: number;
  levelMin: number;
  levelMax: number;
  rarity: string;
  timestampAvailable: number;
  evos: Record<string, SiteEvo>;
}
interface ClanInfo {
  id: number;
  name: string;
  bonus: string;
  bonusId: number;
}

// ---- clan map ---------------------------------------------------------------------------
const clans = new Map<number, ClanInfo>();
try {
  const legacy: { clan_id: number; clan_name: string; bonus: string }[] = JSON.parse(await Deno.readTextFile(LEGACY_CARDS));
  for (const c of legacy) {
    if (!clans.has(c.clan_id)) clans.set(c.clan_id, { id: c.clan_id, name: c.clan_name, bonus: c.bonus, bonusId: 0 });
  }
  const legacyData: { clan_id: number; bonus_id?: number }[] = JSON.parse(await Deno.readTextFile(LEGACY_DATA));
  for (const c of legacyData) {
    const clan = clans.get(c.clan_id);
    if (clan && c.bonus_id) clan.bonusId = c.bonus_id;
  }
} catch (e) {
  console.warn(`Could not read legacy card data for clan bonuses: ${(e as Error).message}`.yellow);
}
try {
  // Shape written by log_server.ts from clans.get:
  //   [{ id, name, codeName, bonusId: { id }, bonus: { description: {en,...} }, nightBonus, ... }]
  // deno-lint-ignore no-explicit-any
  const site: any[] = JSON.parse(await Deno.readTextFile(SITE_CLANS));
  for (const c of site) {
    const name = typeof c.name === "string" ? c.name : c.name?.en;
    const bonus = typeof c.bonus === "string" ? c.bonus : c.bonus?.description?.en ?? c.bonus?.en;
    if (!name || !bonus) continue;
    const bonusId = c.bonusId?.id ?? c.bonusId ?? c.bonus?.id ?? clans.get(c.id)?.bonusId ?? 0;
    clans.set(c.id, { id: c.id, name, bonus, bonusId: typeof bonusId === "number" ? bonusId : 0 });
  }
  console.log(`Loaded ${site.length} clans from ${SITE_CLANS}`.green);
} catch {
  console.log(`No ${SITE_CLANS}; using clan bonuses from ${LEGACY_CARDS}`.gray);
}

// Names of clans the engine knows (CardTypes.Clans).
const engineClanNames = new Set<string>(Object.values(ClanIdMap));

// ---- characters -------------------------------------------------------------------------
const text = await Deno.readTextFile(SITE_CHARACTERS);
const characters: SiteCharacter[] = text.split("\n").filter((l) => l.trim()).map((l) => JSON.parse(l));
console.log(`${characters.length} characters in ${SITE_CHARACTERS}`.green);

const rows = [];
const unknownClans = new Map<number, string[]>();
for (const c of characters.sort((a, b) => a.id - b.id)) {
  const clan = clans.get(c.clanId);
  if (!clan) {
    unknownClans.set(c.clanId, [...(unknownClans.get(c.clanId) ?? []), c.name]);
    continue;
  }
  if (!engineClanNames.has(clan.name)) {
    // Engine has a hardcoded clan list; a new clan needs adding to src/game/types/CardTypes.ts
    unknownClans.set(c.clanId, [...(unknownClans.get(c.clanId) ?? []), c.name]);
    continue;
  }
  for (let level = c.levelMin; level <= c.levelMax; level++) {
    const evo = c.evos[String(level)];
    if (!evo) continue;
    const unlocked = evo.ability.unlockLevel <= level;
    rows.push({
      id: c.id,
      name: c.name,
      clan_id: c.clanId,
      clan_name: clan.name,
      level,
      level_min: c.levelMin,
      level_max: c.levelMax,
      power: evo.power,
      damage: evo.damage,
      rarity: c.rarity,
      ability_id: unlocked ? evo.ability.id : 0,
      ability: unlocked ? evo.ability.description.en : "No Ability",
      ability_unlock_level: evo.ability.unlockLevel,
      bonus: clan.bonus,
      bonus_id: clan.bonusId,
      release_date: c.timestampAvailable,
    });
  }
}

for (const [clanId, names] of unknownClans) {
  console.warn(
    `Skipped ${names.length} cards of unknown clan ${clanId} (${names.slice(0, 4).join(", ")}${names.length > 4 ? ", …" : ""}). ` +
      `Run __ur.dumpClans() in the browser and add the clan to src/game/types/CardTypes.ts.`.yellow,
  );
}

await Deno.writeTextFile(OUT, JSON.stringify(rows));
const cards = new Set(rows.map((r) => r.id)).size;
console.log(`Wrote ${rows.length} rows (${cards} cards, all levels) to ${OUT}`.green);
