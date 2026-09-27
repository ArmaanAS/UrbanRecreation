// The corpus scan (src/situations/Corpus.ts) on real captures: keys that are known to be
// there, the probe's baseline agreeing with the engine on every captured round, and the clan
// bonus id table agreeing with every captured hand.
import { assert, assertEquals } from "@std/assert";
import cardRows from "@data/data.json" with { type: "json" };
import {
  aggregate,
  type GameRecord,
  loadRecords,
  neverFired,
  replayReady,
  scanRecord,
} from "@/situations/Corpus.ts";
import { CLAN_BONUS_IDS } from "@/situations/Situations.ts";
import type { Clan } from "@/game/types/CardTypes.ts";

const GAMES = new URL("../../captures/games/", import.meta.url);
const record = async (id: number): Promise<GameRecord> =>
  JSON.parse(await Deno.readTextFile(new URL(`${id}.json`, GAMES)));
const keysOf = async (id: number, round: number) => {
  const scan = scanRecord(await record(id));
  assertEquals(scan.baselineDisagreements, 0);
  const r = scan.rounds.find((r) => r.round === round);
  assert(r !== undefined, `${id} round ${round} was not reproduced`);
  return new Set(r.keys);
};

Deno.test("1024673 r0: Lyse Teria Cr's Stop Opp. Ability stops Nantosuelte's ability", async () => {
  const keys = await keysOf(1024673, 0);
  assert(keys.has("fired:ability:73"));
  assert(keys.has("fired:ability:73:win:second"));
  assert(keys.has("pair:opp:ability:2535|ability:73"));
  assert(!keys.has("fired:ability:2535"));
});

Deno.test("1506438 r2: the newer Freaks Poison replaces the older one", async () => {
  // Events.executeRepeat: the opposing Freaks bonus latches in rounds zero and one, and
  // round two takes 2 Life, not 4.
  const keys = await keysOf(1506438, 2);
  assert(keys.has("paid:bonus:206"));
  assert(keys.has("pair:own:paid:bonus:206|paid:bonus:206"));
});

Deno.test("1508676 r2: an opposing Cancel keeps a latched Combust from paying", async () => {
  // Babe's "Cancel Opp. Pillz & Life Modif." (1655) against Kontrø Ld's "Combust 1, Min 0".
  const keys = await keysOf(1508676, 2);
  assert(keys.has("fired:ability:1655"));
  assert(keys.has("pair:opp:ability:1655|paid:ability:5684"));
});

Deno.test("Leaders, Tie-break and Copy are sources too", async () => {
  // Timber's "Team: +1 Damage" in every round it is live, not only when Timber is played.
  for (const round of [0, 1, 2]) assert((await keysOf(1508957, round)).has("fired:ability:121"));
  // Solomon's Tie-break wins a tied round (Game.tieBreaker).
  assert((await keysOf(1506259, 3)).has("fired:ability:1135:win:second"));
  // Kunglaba's "Copy: Opp. Bonus" (3194) copies the Paradox bonus, and pairs with it.
  const copy = await keysOf(1023608, 3);
  assert(copy.has("fired:ability:3194"));
  assert(copy.has("pair:opp:ability:3194|bonus:1844"));
});

Deno.test("the probe's baseline is the engine's own round on every captured round", async () => {
  const records = (await loadRecords(GAMES)).filter(replayReady);
  const scans = records.map(scanRecord);
  assertEquals(scans.reduce((n, s) => n + s.baselineDisagreements, 0), 0);
  const counts = aggregate(scans);
  assert(counts.rounds > 1000);
  assertEquals(counts.games, records.length);
  for (const [key, n] of Object.entries(counts.counts)) {
    assert(n > 0 && counts.examples[key].length === Math.min(n, 3), key);
  }
});

Deno.test("CLAN_BONUS_IDS is the id every captured hand shows for its clan's bonus", async () => {
  const printed = new Map<Clan, { day: string; night?: string }>();
  for (const r of cardRows as { clan_name: Clan; bonus: string; night_bonus?: string }[]) {
    printed.set(r.clan_name, { day: r.bonus, night: r.night_bonus });
  }
  let checked = 0;
  for (const rec of await loadRecords(GAMES)) {
    for (const player of rec.players) {
      for (const card of player.hand) {
        if (card.clan == null || card.bonus == null || card.clan === "Oculus") continue;
        const text = printed.get(card.clan)!;
        const ids = CLAN_BONUS_IDS[card.clan];
        assert(ids !== undefined, `no bonus id for ${card.clan}`);
        // A copied bonus (Oblivion's own bonus is a Copy) shows what it copied instead.
        if (card.bonus.description === text.day) assertEquals(card.bonus.id, ids.day, card.clan);
        else if (card.bonus.description === text.night) assertEquals(card.bonus.id, ids.night, card.clan);
        else continue;
        checked++;
      }
    }
  }
  assert(checked > 1000);
});

Deno.test("owned cards are listed by clan when their ability id never fired", () => {
  // Aurora owned at levels 4 and 5 (a Prismatic copy of the 5): the highest level is read,
  // so the list names level 5's "+3 Life", not level 4's "Bet > 5 Pillz: +3 Life".
  const aurora = (cardRows as { id: number; name: string; level: number; ability_id: number }[])
    .filter((r) => r.name === "Aurora");
  const id = aurora[0].id;
  const l5 = aurora.find((r) => r.level === 5)!.ability_id;
  const collection = { cards: { [id]: { "4": { "": 1 }, "5": { "": 0, p: 1 } } } };

  const unseen = neverFired(collection, {});
  assertEquals(unseen.clans.length, 1);
  assertEquals(unseen.clans[0].clan, "Rescue");
  assertEquals(unseen.clans[0].bonusNeverFired, true);
  assertEquals(unseen.clans[0].cards.map((c) => [c.level, c.abilityId]), [[5, l5]]);

  const seen = neverFired(collection, { [`fired:ability:${l5}`]: 2, "fired:bonus:266": 1 });
  assertEquals(seen.clans, []);
});
