/**
 * The capture corpus seen through `Situations.ts`: every replay-ready game record in
 * `captures/games/` rebuilt as `tests/replay/Replay.test.ts` does, and every captured round
 * probed with the moves the server actually saw. A round only counts once the engine has
 * reproduced it (card results, Life and Pillz, as the replay suite checks), and a game stops
 * counting at its first round the engine does not reproduce: the keys are the engine's
 * prediction, and a prediction the server contradicted is not evidence of coverage.
 */
import Game from "@/game/Game.ts";
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import { Turn } from "@/game/types/Types.ts";
import type { Clan, HandOf } from "@/game/types/CardTypes.ts";
import cardRows from "@data/data.json" with { type: "json" };
import {
  type CapturedCard,
  type CardResult,
  CLAN_BONUS_IDS,
  LatchLedger,
  type Move,
  RoundProbe,
  slotIds,
} from "./Situations.ts";

interface TestcaseMove {
  s1: [number, number, boolean];
  s2: [number, number, boolean];
  p1life: number;
  p2life: number;
  p1pillz: number;
  p2pillz: number;
  r1?: CardResult;
  r2?: CardResult;
}

export interface Testcase {
  cards: string[];
  levels?: number[];
  night?: boolean;
  life: number;
  pillz: number;
  /** Server-substituted ability per card (Hazard, rebalanced cards); null keeps the printed one. */
  abilities?: (string | null)[];
  moves: TestcaseMove[];
}

/** The parts of a `captures/games/<id>.json` record the scan reads. */
export interface GameRecord {
  id: number;
  firstPlayer: 0 | 1 | null;
  finalStatus: string;
  players: {
    name?: string;
    hand: (CapturedCard & { name: string | null; clan?: Clan | null })[];
  }[];
  testcase: Testcase | null;
}

/** What the replay suite runs: a testcase, a known first mover, a finished battle. */
export function replayReady(rec: GameRecord): boolean {
  return rec.testcase !== null && rec.firstPlayer !== null && rec.finalStatus !== "playing";
}

/** The match at its first position, built exactly as the replay suite builds it. */
export function gameOf(tc: Testcase): Game {
  const lv = tc.levels ?? [];
  const h1 = HandGenerator.handOf(
    tc.cards.slice(0, 4) as HandOf<string>,
    lv.slice(0, 4) as HandOf<number | undefined>,
  );
  const h2 = HandGenerator.handOf(
    tc.cards.slice(4, 8) as HandOf<string>,
    lv.slice(4, 8) as HandOf<number | undefined>,
  );
  tc.abilities?.forEach((ability, i) => {
    if (ability === null) return;
    const hand = i < 4 ? h1 : h2;
    hand[i % 4] = hand[i % 4].withAbility(ability);
  });
  return new Game(
    new Player(tc.life, tc.pillz, 0),
    new Player(tc.life, tc.pillz, 1),
    h1,
    h2,
    Turn.PLAYER_1,
    false,
    tc.night ?? false,
  );
}

export interface ScannedRound {
  /** 0-based round index. */
  round: number;
  keys: string[];
}

export interface ScanResult {
  id: number;
  /** Captured rounds the engine reproduced, with their situation keys. */
  rounds: ScannedRound[];
  /** Captured rounds in the testcase. */
  captured: number;
  /** The first round the engine does not reproduce, if any; it and later ones are left out. */
  mismatch?: number;
  /**
   * Rounds whose probe, with every source live, disagreed with the engine's own `select` of
   * the same moves. It compiles the round separately, so this checks the ablation's baseline
   * against the engine; it should always be zero.
   */
  baselineDisagreements: number;
}

const sameResult = (a: CardResult, b: CardResult) =>
  a.power === b.power && a.damage === b.damage && a.attack === b.attack && a.won === b.won;

/** Probe every round of one replay-ready record with the moves the server saw. */
export function scanRecord(rec: GameRecord): ScanResult {
  const tc = rec.testcase!;
  const first = rec.firstPlayer!;
  const game = gameOf(tc);
  const captured = [...rec.players[first].hand, ...rec.players[1 - first].hand];
  const ids = slotIds(game, captured);
  const ledger = new LatchLedger();
  const result: ScanResult = {
    id: rec.id,
    rounds: [],
    captured: tc.moves.length,
    baselineDisagreements: 0,
  };

  for (let round = 0; round < tc.moves.length; round++) {
    const move = tc.moves[round];
    // s1 is the round's first mover.
    const [m1, m2]: [Move, Move] = game.playingFirst === Turn.PLAYER_1
      ? [move.s1, move.s2]
      : [move.s2, move.s1];
    const probed = new RoundProbe(game, { ids, ledger }).analyse(m1, m2);
    ledger.play(game, m1, m2, ids);

    const c1 = game.h1[m1[0]], c2 = game.h2[m2[0]];
    const r1 = { power: c1.power.final, damage: c1.damage.final, attack: c1.attack.final, won: !!c1.won };
    const r2 = { power: c2.power.final, damage: c2.damage.final, attack: c2.attack.final, won: !!c2.won };
    if (
      !sameResult(probed.p1, r1) || !sameResult(probed.p2, r2) ||
      probed.life[0] !== game.p1.life || probed.life[1] !== game.p2.life ||
      probed.pillz[0] !== game.p1.pillz || probed.pillz[1] !== game.p2.pillz
    ) {
      result.baselineDisagreements++;
    }

    const reproduced = (move.r1 === undefined || sameResult(r1, move.r1)) &&
      (move.r2 === undefined || sameResult(r2, move.r2)) &&
      game.p1.life === move.p1life && game.p2.life === move.p2life &&
      game.p1.pillz === move.p1pillz && game.p2.pillz === move.p2pillz;
    if (!reproduced) {
      result.mismatch = round;
      break;
    }
    result.rounds.push({ round, keys: [...probed.keys].sort() });
    if (!game.isPlaying) break;
  }
  return result;
}

/** Up to this many (battle, round) examples are kept per key. */
export const EXAMPLES_PER_KEY = 3;

export interface SituationCounts {
  generatedAt: string;
  /** Replay-ready games scanned. */
  games: number;
  /** Rounds that counted (reproduced by the engine). */
  rounds: number;
  /** Games cut short at a round the engine does not reproduce, with that round. */
  mismatched: { id: number; round: number }[];
  counts: Record<string, number>;
  examples: Record<string, { battle: number; round: number }[]>;
}

/** Fold scanned games into key counts, keys sorted, first examples in battle-id order. */
export function aggregate(results: ScanResult[], generatedAt = new Date()): SituationCounts {
  const sorted = [...results].sort((a, b) => a.id - b.id);
  const counts: Record<string, number> = {};
  const examples: Record<string, { battle: number; round: number }[]> = {};
  let rounds = 0;
  for (const r of sorted) {
    for (const round of r.rounds) {
      rounds++;
      for (const key of round.keys) {
        counts[key] = (counts[key] ?? 0) + 1;
        const list = examples[key] ??= [];
        if (list.length < EXAMPLES_PER_KEY) list.push({ battle: r.id, round: round.round });
      }
    }
  }
  const keys = Object.keys(counts).sort();
  return {
    generatedAt: generatedAt.toISOString(),
    games: sorted.length,
    rounds,
    mismatched: sorted.filter((r) => r.mismatch !== undefined).map((r) => ({
      id: r.id,
      round: r.mismatch!,
    })),
    counts: Object.fromEntries(keys.map((k) => [k, counts[k]])),
    examples: Object.fromEntries(keys.map((k) => [k, examples[k]])),
  };
}

// ---------------------------------------------------------------------------------------
// The owner's cards that no capture has shown firing
// ---------------------------------------------------------------------------------------

interface CatalogRow {
  id: number;
  name: string;
  level: number;
  clan_name: Clan;
  ability_id: number;
  ability: string;
  bonus: string;
}

const catalog = new Map<number, CatalogRow>();
/** The printed (day) text of every ability id the card data knows. */
const abilityText = new Map<number, string>();
for (const r of cardRows as CatalogRow[]) {
  catalog.set(r.id * 8 + r.level, r);
  abilityText.set(r.ability_id, r.ability);
}

/** `data/my_collection.json`: copies per card id, level and edition. */
export interface Collection {
  fetchedAt?: string;
  cards: Record<string, Record<string, Record<string, number>>>;
}

export interface UnfiredCard {
  id: number;
  name: string;
  /** The highest level owned, which is what the ability is read at. */
  level: number;
  abilityId: number;
  ability: string;
  /**
   * Another id printed with the same text has fired (e.g. one of the many "Stop Opp.
   * Ability"s), so the TypeScript engine has seen the rule if not this card's id. The Rust
   * engine admits abilities id by id, so the id alone still counts as unseen.
   */
  textFired: boolean;
}

export interface UnfiredClan {
  clan: Clan;
  /** Owned cards of the clan (a bonus needs two of them in one hand). */
  owned: number;
  bonus: string;
  /** The clan bonus's battle ids (GhosTown has a day and a night one); none for Oculus. */
  bonusIds: number[];
  /** No capture shows the clan bonus firing. */
  bonusNeverFired: boolean;
  /** Owned cards whose ability (at their highest owned level) no capture shows firing. */
  cards: UnfiredCard[];
}

/**
 * The owner's cards, each at its highest owned level, grouped by clan: which clan bonuses
 * and which abilities have never fired in the corpus (`counts` from `aggregate`). Only clans
 * with something unfired are listed. An ability is identified by the card data's id for its
 * day text; Leaders have no clan bonus, and Oculus's "Infiltrated" counts as fired once any
 * infiltrated bonus has (`fired:bonus:infiltrated`).
 */
export function neverFired(
  collection: Collection,
  counts: Readonly<Record<string, number>>,
): { clans: UnfiredClan[]; missing: { id: number; level: number }[] } {
  const fired = (key: string) => (counts[key] ?? 0) > 0;
  const firedTexts = new Set<string>();
  for (const key of Object.keys(counts)) {
    const m = /^fired:ability:(\d+)$/.exec(key);
    const text = m && abilityText.get(Number(m[1]));
    if (text) firedTexts.add(text);
  }
  const byClan = new Map<Clan, UnfiredClan>();
  const missing: { id: number; level: number }[] = [];
  for (const [idText, levels] of Object.entries(collection.cards)) {
    const id = Number(idText);
    const owned = Object.entries(levels)
      .filter(([, editions]) => Object.values(editions).some((n) => n > 0))
      .map(([level]) => Number(level));
    if (owned.length === 0) continue;
    const level = Math.max(...owned);
    const row = catalog.get(id * 8 + level);
    if (row === undefined) {
      missing.push({ id, level });
      continue;
    }
    const clan = row.clan_name;
    let entry = byClan.get(clan);
    if (entry === undefined) {
      const ids = CLAN_BONUS_IDS[clan];
      const bonusIds = clan === "Oculus" || ids === undefined
        ? []
        : [ids.day, ...(ids.night === undefined ? [] : [ids.night])];
      entry = {
        clan,
        owned: 0,
        bonus: row.bonus,
        bonusIds,
        bonusNeverFired: clan === "Leader"
          ? false
          : clan === "Oculus"
          ? !fired("fired:bonus:infiltrated")
          : !bonusIds.some((b) => fired(`fired:bonus:${b}`)),
        cards: [],
      };
      byClan.set(clan, entry);
    }
    entry.owned++;
    if (row.ability_id !== 0 && row.ability !== "No Ability" && !fired(`fired:ability:${row.ability_id}`)) {
      entry.cards.push({
        id,
        name: row.name,
        level,
        abilityId: row.ability_id,
        ability: row.ability,
        textFired: firedTexts.has(row.ability),
      });
    }
  }
  const clans = [...byClan.values()]
    .filter((c) => c.bonusNeverFired || c.cards.length > 0)
    .sort((a, b) => a.clan.localeCompare(b.clan));
  for (const c of clans) c.cards.sort((a, b) => a.name.localeCompare(b.name));
  return { clans, missing };
}

/** Every record in `captures/games/`, sorted by battle id. */
export async function loadRecords(dir: URL | string): Promise<GameRecord[]> {
  const base = typeof dir === "string" ? dir.replace(/[\\/]?$/, "/") : dir.href.replace(/\/?$/, "/");
  const records: GameRecord[] = [];
  for await (const f of Deno.readDir(dir)) {
    if (!f.isFile || !f.name.endsWith(".json")) continue;
    const path = typeof dir === "string" ? base + f.name : new URL(f.name, base);
    records.push(JSON.parse(await Deno.readTextFile(path)));
  }
  return records.sort((a, b) => a.id - b.id);
}
