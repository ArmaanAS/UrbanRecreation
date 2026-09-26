// Which cards the exact Rust solver can score, from data/card_coverage.json (written by
// `deno task card-coverage`), in the words and the compact shape Deck Lab shows.
import type { CardKey, MatchupProvenance, ProbeStatus } from "./Matchup.ts";

/**
 * What stops a refused card:
 *   uncaptured_text  no battle capture has shown that ability or bonus text at all;
 *   uncaptured_id    the text is known, but not under this card's ability id (the registry
 *                    keeps printed identity, so a same-text variant waits for its own capture);
 *   not_executable   the registry knows it, the strict engine does not execute it.
 */
export type BlockKind = "uncaptured_text" | "uncaptured_id" | "not_executable" | "other";
export const BLOCK_KINDS: BlockKind[] = ["uncaptured_text", "uncaptured_id", "not_executable", "other"];

export interface Blocker {
  source: "Ability" | "Bonus" | "card";
  text: string;
  kind: BlockKind;
}

/** The source a refusal names and what blocks it, read from the strict catalog's reason. */
export function blocker(reason: string): Blocker {
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

/** One card level at one time of day, as data/card_coverage.json stores it. */
export interface CoverageEntry {
  status: ProbeStatus;
  reason?: string;
  blocked?: BlockKind;
  partner?: CardKey;
}

export interface CoverageFile {
  generatedAt: string;
  provenance: MatchupProvenance;
  cards: { id: number; levels: Record<string, { day?: CoverageEntry; night?: CoverageEntry }> }[];
}

const WHY: Record<Exclude<BlockKind, "other">, string> = {
  uncaptured_text: "has not been seen in a captured battle yet",
  uncaptured_id: "is known, but not on this card: it needs a capture of this card",
  not_executable: "is not modelled by the exact engine yet",
};

/** A strict-catalog refusal in a few words: which ability or bonus, and what it waits for. */
export function describeRefusal(reason: string): string {
  const b = blocker(reason);
  if (/unsupported Leader/.test(reason)) return "Leaders are not modelled by the exact engine";
  // The seat and slot say nothing to a reader, and would split one card's refusals in two.
  if (b.kind === "other" || b.source === "card") return reason.replace(/^P[12] slot \d+ /, "");
  return `${b.source === "Bonus" ? "clan bonus" : "ability"} "${b.text}" ${WHY[b.kind]}`;
}

/** One line saying why the solver cannot score a card, or undefined when it can. */
export function whyNot(entry: CoverageEntry): string | undefined {
  switch (entry.status) {
    case "exact":
      return undefined;
    case "leader":
      return "Leaders are not modelled by the exact engine";
    case "missing":
      return "not in the engine's card data";
    case "bonus_untested":
      return "its clan bonus could not be tested";
  }
  if (!entry.reason) return entry.status === "bonus_refused" ? "its clan bonus is not modelled" : "not modelled";
  return describeRefusal(entry.reason);
}

export type CoverageCode = "e" | "b" | "u" | "r" | "l" | "m";
const CODES: Record<ProbeStatus, CoverageCode> = {
  exact: "e",
  bonus_refused: "b",
  bonus_untested: "u",
  refused: "r",
  leader: "l",
  missing: "m",
};

/**
 * Coverage for a browser: per card id and level, `[day, night, day why, night why]` where a
 * status is one letter (`e` exact, `b` bonus refused, `u` bonus untested, `r` refused, `l`
 * Leader, `m` missing, `null` not probed) and a why indexes `reasons` (-1 for none).
 */
export interface CompactCoverage {
  generatedAt: string;
  reasons: string[];
  cards: Record<string, Record<string, [CoverageCode | null, CoverageCode | null, number, number]>>;
}

export function compactCoverage(file: CoverageFile): CompactCoverage {
  const reasons: string[] = [];
  const index = new Map<string, number>();
  const why = (entry?: CoverageEntry) => {
    const text = entry && whyNot(entry);
    if (!text) return -1;
    let i = index.get(text);
    if (i === undefined) index.set(text, i = reasons.push(text) - 1);
    return i;
  };
  const cards: CompactCoverage["cards"] = {};
  for (const card of file.cards) {
    const levels: CompactCoverage["cards"][string] = {};
    for (const [level, { day, night }] of Object.entries(card.levels)) {
      levels[level] = [day ? CODES[day.status] : null, night ? CODES[night.status] : null, why(day), why(night)];
    }
    cards[card.id] = levels;
  }
  return { generatedAt: file.generatedAt, reasons, cards };
}
