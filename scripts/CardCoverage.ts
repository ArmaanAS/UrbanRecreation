// Which cards the exact Rust solver can score, per card, level, day and night.
//
//   deno task rust:matchup      # build the solver once
//   deno task card-coverage     # writes data/card_coverage.json (gitignored)
//
// Every card at every level in data/site_cards.json is probed by `urban-recreation-matchup`
// in a neutral draw (rust/src/advisor/matchup.rs says exactly which): `exact` means it can be
// scored alone and beside a clan-mate, `bonus_refused` only without a clan-mate, `refused`
// not at all; `leader` and `missing` (not in data/data.json) speak for themselves.
// Each refusal is also classified by what blocks it:
//   uncaptured_text  no battle capture has shown that ability or bonus text at all;
//   uncaptured_id    the text is known, but not under this card's ability id (the registry
//                    keeps printed identity, so a same-text variant waits for its own capture);
//   not_executable   the registry knows it, the strict engine does not execute it.
import "colors";
import {
  assertCurrentBinary,
  matchupProvenance,
  type ProbeRequest,
  type ProbeResponse,
  type ProbeStatus,
  ProcessMatchupRunner,
} from "../src/decks/Matchup.ts";
import type { SiteCard } from "../src/decks/SiteData.ts";

const OUT = "data/card_coverage.json";

const siteCards = JSON.parse(await Deno.readTextFile("data/site_cards.json")).cards as SiteCard[];
const requests: ProbeRequest[] = [];
for (const card of siteCards) {
  const levels = Object.keys(card.evos).map(Number).filter(Number.isInteger).sort((a, b) => a - b);
  for (const level of levels) {
    for (const night of [false, true]) requests.push({ kind: "probe", card: [card.id, level], night });
  }
}

const runner = new ProcessMatchupRunner();
const provenance = await matchupProvenance(runner);
await assertCurrentBinary(provenance);
const started = performance.now();
const encoder = new TextEncoder();
const responses = await runner.run(requests, (_, i) => {
  if (Deno.stderr.isTerminal() && i % 250 === 0) {
    Deno.stderr.writeSync(encoder.encode(`\r  probed ${i}/${requests.length}`));
  }
});
if (Deno.stderr.isTerminal()) Deno.stderr.writeSync(encoder.encode("\r\x1b[K"));
const seconds = (performance.now() - started) / 1000;

type BlockKind = "uncaptured_text" | "uncaptured_id" | "not_executable" | "other";
const BLOCK_KINDS: BlockKind[] = ["uncaptured_text", "uncaptured_id", "not_executable", "other"];

interface Blocker {
  source: "Ability" | "Bonus" | "card";
  text: string;
  kind: BlockKind;
}

/** The source a refusal names and what blocks it (see the file comment). */
function blocker(reason: string): Blocker {
  const m = /(Ability|Bonus) catalog source [^"]*"((?:[^"\\]|\\.)*)"/.exec(reason);
  const kind: BlockKind = /lookup failed: effect description .* is missing/.test(reason)
    ? "uncaptured_text"
    : /lookup failed: effect id \d+ is missing/.test(reason)
    ? "uncaptured_id"
    : /is not executable by this projection|cannot map to a compact plan/.test(reason)
    ? "not_executable"
    : "other";
  return { source: (m?.[1] as "Ability" | "Bonus" | undefined) ?? "card", text: m ? m[2] : reason, kind };
}

type Entry = { status: ProbeStatus; reason?: string; blocked?: BlockKind; partner?: readonly [number, number] };
const byCard = new Map<number, Record<string, { day?: Entry; night?: Entry }>>();
responses.forEach((raw, i) => {
  if ("error" in raw) throw new Error(`probe ${JSON.stringify(requests[i])} failed: ${raw.error}`);
  const r = raw as ProbeResponse;
  const failed = r.ability.ok === false ? r.ability : r.bonus.ok !== true ? r.bonus : undefined;
  const entry: Entry = { status: r.status };
  if (r.status !== "exact" && failed?.reason) {
    entry.reason = failed.reason;
    if (r.status === "refused" || r.status === "bonus_refused") entry.blocked = blocker(failed.reason).kind;
  }
  if (r.status === "bonus_refused" && r.bonus.partner) entry.partner = r.bonus.partner;
  const [id, level] = r.card;
  const levels = byCard.get(id) ?? {};
  (levels[level] ??= {})[r.night ? "night" : "day"] = entry;
  byCard.set(id, levels);
});

const STATUSES: ProbeStatus[] = ["exact", "bonus_refused", "bonus_untested", "refused", "leader", "missing"];
const tally = (entries: Entry[]) => {
  const counts = Object.fromEntries(STATUSES.map((s) => [s, 0])) as Record<ProbeStatus, number>;
  const blocked = Object.fromEntries(BLOCK_KINDS.map((k) => [k, 0])) as Record<BlockKind, number>;
  for (const e of entries) {
    counts[e.status]++;
    if (e.blocked) blocked[e.blocked]++;
  }
  return { ...counts, blocked };
};
const all = (night: boolean) => [...byCard.values()].flatMap((levels) => Object.values(levels).map((l) => (night ? l.night : l.day)!));
const maxLevel = (night: boolean) =>
  siteCards.map((c) => byCard.get(c.id)?.[c.level_max]?.[night ? "night" : "day"]).filter((e): e is Entry => !!e);

// The sources that stop the most max-level cards by day: what a new capture or slice would buy.
const blockers = new Map<string, Blocker & { cards: number }>();
for (const e of maxLevel(false)) {
  if (!e.reason || !e.blocked) continue;
  const b = blocker(e.reason);
  const key = `${b.source}\u0000${b.text}\u0000${b.kind}`;
  const entry = blockers.get(key) ?? { ...b, cards: 0 };
  entry.cards++;
  blockers.set(key, entry);
}
const topBlockers = [...blockers.values()].sort((x, y) => y.cards - x.cards || x.text.localeCompare(y.text)).slice(0, 25);

const summary = {
  cardLevels: all(false).length,
  cards: siteCards.length,
  day: tally(all(false)),
  night: tally(all(true)),
  maxLevelDay: tally(maxLevel(false)),
  maxLevelNight: tally(maxLevel(true)),
  maxLevelExactDayOrNight: siteCards.filter((c) => {
    const l = byCard.get(c.id)?.[c.level_max];
    return l?.day?.status === "exact" || l?.night?.status === "exact";
  }).length,
  topBlockersMaxLevelDay: topBlockers,
};

const cards = siteCards.map((c) => ({ id: c.id, name: c.name, clan: c.clan_name, levels: byCard.get(c.id) ?? {} }));
await Deno.writeTextFile(
  OUT,
  JSON.stringify(
    {
      generatedAt: new Date().toISOString(),
      method: "neutral probe by urban-recreation-matchup: the card with three level-1 no-ability fillers of other " +
        "clans (ability), then with one clan-mate and two fillers (bonus); see rust/src/advisor/matchup.rs",
      provenance,
      summary,
      cards,
    },
    null,
    1,
  ) + "\n",
);

const pct = (x: number, of: number) => `${((100 * x) / of).toFixed(1)}%`;
const line = (name: string, t: ReturnType<typeof tally>, of: number) =>
  console.log(
    `  ${name.padEnd(18)} exact ${String(t.exact).padStart(5)} (${pct(t.exact, of)})  bonus refused ${t.bonus_refused}  ` +
      `refused ${t.refused}  leader ${t.leader}  missing ${t.missing}  untested ${t.bonus_untested}  | blocked by ` +
      `uncaptured text ${t.blocked.uncaptured_text}, uncaptured id ${t.blocked.uncaptured_id}, ` +
      `not executable ${t.blocked.not_executable}, other ${t.blocked.other}`,
  );
console.log(`${OUT}: ${summary.cards} cards, ${summary.cardLevels} card-levels, ${requests.length} probes in ${seconds.toFixed(1)} s`);
line("all levels, day", summary.day, summary.cardLevels);
line("all levels, night", summary.night, summary.cardLevels);
line("max level, day", summary.maxLevelDay, summary.cards);
line("max level, night", summary.maxLevelNight, summary.cards);
console.log(
  `  max level, exact by day or night: ${summary.maxLevelExactDayOrNight} (${pct(summary.maxLevelExactDayOrNight, summary.cards)})`,
);
console.log("  most common blockers at max level, day:");
for (const b of topBlockers.slice(0, 10)) {
  console.log(`    ${String(b.cards).padStart(4)}  ${b.source} ${JSON.stringify(b.text)} (${b.kind.replace("_", " ")})`.gray);
}
