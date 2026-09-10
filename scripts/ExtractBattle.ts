// Turn battle captures into clean, replayable game records.
//
//   deno task extract                 # all captures/battles/*.jsonl → captures/games/<id>.json
//   deno task extract 866431 866440   # only these battle ids
//   deno task extract --raw ur_log.jsonl   # first split a raw userscript log into captures/battles/
//   deno task extract --recompact     # rewrite legacy full-snapshot battle files in the compact format
//
// A game record holds both decks, every move with timing, the per-round resolution the
// server reported (power / damage / attack / winner), life & pillz after each round, the
// server-applied post-round effects, and a `testcase` in the format used by
// tests/rust/TestcaseRunner.ts so the engine can replay it.
import "colors";
import cards from "@data/data.json" with { type: "json" };
import {
  type CaptureEntry,
  type CaptureState,
  compactEntries,
  expandEntries,
  extractFromRecord,
  loadAbilities,
  newCaptureState,
  type RawRecord,
  saveAbilities,
} from "./BattleCapture.ts";

const BATTLE_DIR = "captures/battles";
const GAME_DIR = "captures/games";

type Side = 0 | 1;
// deno-lint-ignore no-explicit-any
type Battle = any;
// deno-lint-ignore no-explicit-any
type Character = any;

interface Move {
  side: Side;
  index: number;
  cardId: number;
  /** Pillz bet, engine convention (excludes the free pill and the 3 fury pillz). */
  pillz: number;
  /** Raw server value: pillz bet + 1 (the free pill). Attack = power × pillzUsed. */
  pillzUsed: number;
  fury: boolean;
  t: number;
}
interface Resolution {
  /** Power / damage as shown when the round resolved (modifiers applied). */
  power: number;
  damage: number;
  /** Damage shown in later snapshots; for the loser this reverts to the unmodified value. */
  damageAfter: number;
  attack: number;
  won: boolean;
}
interface Round {
  round: number;
  first: Side | null;
  moves: Move[];
  resolution: [Resolution | null, Resolution | null];
  life: [number, number];
  pillz: [number, number];
  postRoundAbilities: unknown[];
  durationMs: number | null;
}
/** Server-reported outcome of one card in a round; `r1`/`r2` in a testcase move. */
interface MoveResult {
  power: number;
  damage: number;
  attack: number;
  won: boolean;
}
interface Testcase {
  cards: string[];
  /** Level (stars) each card was played at, same order as `cards`. */
  levels: number[];
  /** Clint City was at night (Night: abilities active, night variants in use). */
  night: boolean;
  flip: boolean;
  life: number;
  pillz: number;
  moves: {
    s1: [number, number, boolean];
    s2: [number, number, boolean];
    p1life: number;
    p2life: number;
    p1pillz: number;
    p2pillz: number;
    r1?: MoveResult;
    r2?: MoveResult;
  }[];
}

/**
 * Clint City switches between day and night every 4 hours. Night is 06:00-10:00,
 * 14:00-18:00 and 22:00-02:00 Paris time (source: Urban Rivals wiki / forum; confirmed by
 * captured games where the server sent the active Day:/Night: ability text).
 */
export function isNight(unixSeconds: number): boolean {
  const hour = Number(new Intl.DateTimeFormat("en-GB", { hour: "2-digit", hour12: false, timeZone: "Europe/Paris" }).format(new Date(unixSeconds * 1000)));
  return (hour >= 6 && hour < 10) || (hour >= 14 && hour < 18) || hour >= 22 || hour < 2;
}

const cardById = new Map<number, typeof cards[number]>();
const cardByIdLevel = new Map<string, typeof cards[number]>();
for (const c of cards) {
  cardByIdLevel.set(`${c.id}:${c.level}`, c);
  if (!cardById.has(c.id) || c.level > cardById.get(c.id)!.level) cardById.set(c.id, c);
}

// ---------------------------------------------------------------------------------------
// --raw: split a raw userscript log into per-battle capture files
// ---------------------------------------------------------------------------------------
async function splitRaw(path: string, state: CaptureState): Promise<number[]> {
  const perBattle = new Map<number, CaptureEntry[]>();
  for (const line of (await Deno.readTextFile(path)).split("\n")) {
    if (!line.trim()) continue;
    let rec: RawRecord;
    try {
      rec = JSON.parse(line);
    } catch {
      continue;
    }
    for (const ev of extractFromRecord(rec, state)) {
      if (!perBattle.has(ev.battleId)) perBattle.set(ev.battleId, []);
      perBattle.get(ev.battleId)!.push(ev.entry);
    }
  }
  await Deno.mkdir(BATTLE_DIR, { recursive: true });
  for (const [id, entries] of perBattle) {
    await Deno.writeTextFile(`${BATTLE_DIR}/${id}.jsonl`, entries.map((e) => JSON.stringify(e)).join("\n") + "\n");
    console.log(`split battle ${id}: ${entries.length} entries`.green);
  }
  return [...perBattle.keys()];
}

// ---------------------------------------------------------------------------------------
// Reconstruction
// ---------------------------------------------------------------------------------------
function describeCard(c: Character, issues: string[]) {
  const known = cardById.get(c.id);
  const atLevel = cardByIdLevel.get(`${c.id}:${c.level}`);
  if (!known) issues.push(`card ${c.id} (level ${c.level}) is not in data/data.json`);
  else if (!atLevel) issues.push(`${known.name} (#${c.id}) played at level ${c.level}, data.json lacks that level`);
  return {
    id: c.id,
    name: known?.name ?? null,
    clan: known?.clan_name ?? null,
    level: c.level,
    index: c.index,
    inBattleId: c.inBattleId,
    state: c.state,
    // Full definitions incl. structured abilityData live in captures/abilities.json.
    ability: c.ability ? { id: c.ability.id, description: c.ability.description } : null,
    bonus: c.bonus ? { id: c.bonus.id, description: c.bonus.description } : null,
  };
}

function describePlayer(side: Battle["player0"], sideIdx: Side, issues: string[]) {
  const p = side.player;
  return {
    side: sideIdx,
    id: p.id,
    name: p.name,
    level: p.level,
    grade: p.grade,
    country: p.country,
    club: p.club ? { id: p.club.id, name: p.club.name } : null,
    registrationTime: p.registrationTime,
    certificationLevel: p.certificationLevel,
    baseLife: side.baseLife,
    basePillz: side.basePillz,
    hand: [...side.characters].sort((a: Character, b: Character) => a.index - b.index).map((c: Character) => describeCard(c, issues)),
  };
}

function reconstruct(id: number, entries: CaptureEntry[]) {
  const issues: string[] = [];
  const meta = entries.find((e) => e.kind === "meta") as Extract<CaptureEntry, { kind: "meta" }> | undefined;
  const statuses = entries.filter((e) => e.kind === "status") as Extract<CaptureEntry, { kind: "status" }>[];
  const result = entries.find((e) => e.kind === "result") as Extract<CaptureEntry, { kind: "result" }> | undefined;
  if (statuses.length === 0) throw new Error(`battle ${id}: no status snapshots`);
  statuses.sort((a, b) => a.t - b.t);

  const first = statuses[0].battle;
  const last = statuses[statuses.length - 1].battle;
  const sides = ["player0", "player1"] as const;
  const players = sides.map((k, i) => describePlayer(last[k], i as Side, issues));
  const myId = meta?.myId ?? 0;
  const mySide: Side | null = myId ? (last.player0.player.id === myId ? 0 : last.player1.player.id === myId ? 1 : null) : null;
  const sideOf = (playerId: number): Side | null => last.player0.player.id === playerId ? 0 : last.player1.player.id === playerId ? 1 : null;

  const finished = last.status !== "playing"; // "done", "timeout", ...
  const nRounds = last.round + 1;
  const rounds: Round[] = [];
  for (let r = 0; r < nRounds; r++) {
    rounds.push({ round: r, first: null, moves: [], resolution: [null, null], life: [NaN, NaN], pillz: [NaN, NaN], postRoundAbilities: [], durationMs: null });
  }

  // Walk the snapshots in order to recover move order, timing and transient resolution values.
  const seen = new Set<string>();
  const roundStart = new Map<number, number>();
  for (const { t, battle } of statuses) {
    if (!roundStart.has(battle.round)) roundStart.set(battle.round, t);
    for (const side of [0, 1] as Side[]) {
      for (const c of battle[sides[side]].characters as Character[]) {
        if (c.roundPlayed < 0) continue;
        const round = rounds[c.roundPlayed];
        if (!round) continue;
        const key = `${side}:${c.index}`;
        if (!seen.has(key)) {
          seen.add(key);
          // pillz/fury are hidden until the round resolves; filled in from the final snapshot below.
          round.moves.push({ side, index: c.index, cardId: c.id, pillz: Math.max(0, c.pillzUsed - 1), pillzUsed: c.pillzUsed, fury: !!c.isFury, t });
        }
        if (c.roundAttack >= 0) {
          round.resolution[side] = { power: c.roundPower, damage: c.roundDamage, damageAfter: c.roundDamage, attack: c.roundAttack, won: !!c.roundWon };
        } else if (round.resolution[side]) {
          round.resolution[side]!.damageAfter = c.roundDamage;
        }
      }
    }
    // Life / pillz after round r = first snapshot whose round has advanced past r (or the final one).
    for (let r = 0; r < battle.round && r < nRounds; r++) {
      const round = rounds[r];
      if (Number.isNaN(round.life[0])) {
        round.life = [battle.player0.life, battle.player1.life];
        round.pillz = [battle.player0.pillz, battle.player1.pillz];
        round.postRoundAbilities = [...(battle.player0.postRoundAbilities ?? []), ...(battle.player1.postRoundAbilities ?? [])];
        round.durationMs = t - (roundStart.get(r) ?? t);
      }
    }
  }
  // Final round (game done): take life/pillz from the last snapshot.
  const lastRound = rounds[nRounds - 1];
  if (lastRound && Number.isNaN(lastRound.life[0]) && finished) {
    lastRound.life = [last.player0.life, last.player1.life];
    lastRound.pillz = [last.player0.pillz, last.player1.pillz];
    lastRound.postRoundAbilities = [...(last.player0.postRoundAbilities ?? []), ...(last.player1.postRoundAbilities ?? [])];
    lastRound.durationMs = statuses[statuses.length - 1].t - (roundStart.get(nRounds - 1) ?? 0);
  }

  // Fill final pillz/fury/win values and the per-round first mover.
  for (const round of rounds) {
    for (const m of round.moves) {
      const c = (last[sides[m.side]].characters as Character[]).find((c) => c.index === m.index);
      if (c) {
        m.pillzUsed = c.pillzUsed;
        m.pillz = Math.max(0, c.pillzUsed - 1);
        m.fury = !!c.isFury;
        if (!round.resolution[m.side]) {
          issues.push(`round ${round.round}: no resolution snapshot captured for side ${m.side} (attack unknown)`);
          round.resolution[m.side] = { power: c.roundPower, damage: c.roundDamage, damageAfter: c.roundDamage, attack: -1, won: !!c.roundWon };
        }
      }
    }
    round.moves.sort((a, b) => a.t - b.t);
    if (round.moves.length === 2 && round.moves[0].t !== round.moves[1].t) round.first = round.moves[0].side;
    else if (round.moves.length) {
      // Both first seen in the same snapshot: use whose turn it was at the start of the round.
      const snap = statuses.find((s) => s.battle.round === round.round);
      round.first = snap ? sideOf(snap.battle.turnPlayerId) : null;
      if (round.first === null) issues.push(`round ${round.round}: could not determine who played first`);
    }
    if (round.moves.length !== 2) issues.push(`round ${round.round}: expected 2 moves, saw ${round.moves.length}`);
  }
  if (rounds.length && rounds[0].first === null) rounds[0].first = sideOf(first.turnPlayerId);

  // The final "done" snapshot is taken before the last round's damage is applied, so
  // use battles.result (which is from my point of view) for the closing life totals.
  if (result && lastRound && finished) {
    if (mySide !== null) {
      const opp = (1 - mySide) as Side;
      lastRound.life[mySide] = result.result.player.life;
      lastRound.life[opp] = result.result.opponent.life;
      lastRound.pillz[mySide] = result.result.player.pillz;
      lastRound.pillz[opp] = result.result.opponent.pillz;
    } else {
      issues.push("final round life/pillz may be stale: result could not be attributed to a side");
    }
  }

  const levelMismatch = issues.some((i) => /played at level|is not in data\/data.json/.test(i));
  if (levelMismatch) issues.push("engine lacks stats for one or more cards at the level played: no testcase generated (run __ur.dumpCharacters() then `deno task cards`)");
  const isDojo = first.battleRuleId === 6 || /dojo/i.test(String((meta?.room as { name?: string } | undefined)?.name ?? ""));
  if (isDojo) issues.push("Dojo (tutorial) battle: rules differ from PvP, no testcase generated");
  const night = isNight(first.creationTime);
  // Sanity check: the server sends the *active* variant of Day:/Night: abilities.
  for (const p of players) {
    for (const c of p.hand) {
      for (const k of ["ability", "bonus"] as const) {
        const m = /^\s*(Day|Night)\s*:/.exec(c[k]?.description ?? "");
        if (m && (m[1] === "Night") !== night) issues.push(`${c.name}: server sent "${m[1]}:" text but schedule says ${night ? "night" : "day"} — check isNight()`);
      }
    }
  }
  const testcase = isDojo || levelMismatch ? null : buildTestcase(players, rounds, issues, night);

  return {
    id,
    capturedAt: new Date(statuses[0].t).toISOString(),
    creationTime: first.creationTime,
    room: meta?.room ?? null,
    battleRuleId: first.battleRuleId,
    night,
    myId: myId || null,
    mySide,
    firstPlayer: rounds[0]?.first ?? null,
    players,
    rounds,
    result: result ? { ...result.result, t: result.t } : null,
    finalStatus: last.status,
    snapshots: statuses.length,
    issues,
    testcase,
  };
}

function buildTestcase(players: ReturnType<typeof describePlayer>[], rounds: Round[], issues: string[], night: boolean): Testcase | null {
  const p1: Side | null = rounds[0]?.first ?? null;
  if (p1 === null) return null;
  const p2 = (1 - p1) as Side;
  if (players.some((p) => p.hand.some((c) => c.name === null))) return null;
  const tc: Testcase = {
    cards: [...players[p1].hand.map((c) => c.name!), ...players[p2].hand.map((c) => c.name!)],
    levels: [...players[p1].hand.map((c) => c.level), ...players[p2].hand.map((c) => c.level)],
    night,
    flip: false,
    life: players[p1].baseLife,
    pillz: players[p1].basePillz,
    moves: [],
  };
  for (const r of rounds) {
    if (r.moves.length !== 2 || r.first === null || Number.isNaN(r.life[0])) break;
    const m1 = r.moves.find((m) => m.side === r.first)!;
    const m2 = r.moves.find((m) => m.side !== r.first)!;
    if (r.first !== (r.round % 2 === 0 ? p1 : p2)) {
      issues.push(`round ${r.round}: first mover does not alternate as the engine assumes`);
    }
    const res = (side: Side): MoveResult | undefined => {
      const x = r.resolution[side];
      return x && x.attack >= 0 ? { power: x.power, damage: x.damage, attack: x.attack, won: x.won } : undefined;
    };
    tc.moves.push({
      s1: [m1.index, m1.pillz, m1.fury],
      s2: [m2.index, m2.pillz, m2.fury],
      p1life: r.life[p1],
      p2life: r.life[p2],
      p1pillz: r.pillz[p1],
      p2pillz: r.pillz[p2],
      r1: res(p1),
      r2: res(p2),
    });
  }
  return tc;
}

// ---------------------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------------------
const args = [...Deno.args];
const state = newCaptureState(await loadAbilities());
let ids: number[] = [];
const rawIdx = args.indexOf("--raw");
if (rawIdx >= 0) {
  const [, path] = args.splice(rawIdx, 2);
  ids = await splitRaw(path, state);
}
const recompact = args.includes("--recompact");
if (recompact) args.splice(args.indexOf("--recompact"), 1);
if (args.length) ids = args.map(Number);
if (ids.length === 0) {
  for await (const f of Deno.readDir(BATTLE_DIR)) {
    if (f.name.endsWith(".jsonl")) ids.push(Number(f.name.replace(".jsonl", "")));
  }
}
await Deno.mkdir(GAME_DIR, { recursive: true });

for (const id of ids.sort((a, b) => a - b)) {
  const path = `${BATTLE_DIR}/${id}.jsonl`;
  const text = await Deno.readTextFile(path);
  let entries = text.split("\n").filter((l) => l.trim()).map((l) => JSON.parse(l) as CaptureEntry);
  if (recompact && entries.some((e) => e.kind === "status")) {
    const compact = compactEntries(entries, state);
    await Deno.writeTextFile(path, compact.map((e) => JSON.stringify(e)).join("\n") + "\n");
    console.log(`recompacted ${id}: ${text.length} → ${JSON.stringify(compact).length} bytes`.gray);
  }
  entries = expandEntries(entries, state.abilities);
  try {
    const game = reconstruct(id, entries);
    await Deno.writeTextFile(`${GAME_DIR}/${id}.json`, JSON.stringify(game, null, 2));
    const p = game.players;
    const res = game.result ? `${game.result.result}${game.result.byKo ? " (KO)" : ""}` : game.finalStatus;
    console.log(
      `battle ${id}`.cyan + `  ${p[0].name} vs ${p[1].name}  ${game.rounds.length} rounds  ${res}` +
        (game.testcase ? `  testcase ✓ (${game.testcase.moves.length} moves)`.green : "  no testcase".yellow) +
        (game.issues.length ? `\n  ${game.issues.length} issue(s):\n  - ${game.issues.join("\n  - ")}`.yellow : ""),
    );
  } catch (e) {
    console.error(`battle ${id}: ${(e as Error).message}`.red);
  }
}

if (state.abilitiesDirty) await saveAbilities(state.abilities);
