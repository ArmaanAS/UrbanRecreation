// Strict capture-to-Rust-advisor V3 request normalisation.
//
// This is intentionally separate from both Advisor's private live loop and RustAdvisor's
// process protocol.  It is the one place which is allowed to translate the site-side
// capture orientation into the engine/Rust P1/P2 orientation.
import { reconstruct } from "../../scripts/ExtractBattle.ts";
import Game from "../game/Game.ts";
import { Turn } from "../game/types/Types.ts";
import {
  buildAdvisorRequest,
  type RustAdvisorRequest,
  type RustCardIdentity,
  type RustHistoryRound,
  type RustProvenance,
} from "./RustAdvisor.ts";
import { SearchMode } from "./Search.ts";

type Reconstructed = ReturnType<typeof reconstruct>;
type Side = 0 | 1;
type WirePlayer = "p1" | "p2";

/** This must remain the V1 value in rust/src/effect_registry.rs. */
export const EFFECT_REGISTRY_SCHEMA_VERSION_V1 = 1;
export const RUST_V1_BATTLE_RULE_ID = 10;

export interface RustAdvisorDecisionContext {
  readonly mode: SearchMode;
  /** Search's engine-side requester. */
  readonly us: Turn;
}

/** Everything available at a live decision without reaching into Advisor internals. */
export interface RustAdvisorInputContext {
  readonly rec: Reconstructed;
  readonly game: Game;
  readonly decision: RustAdvisorDecisionContext;
  readonly requestId: string;
  readonly budgetMs: number;
}

/**
 * `supported` establishes only capture/wire eligibility.  The Rust worker remains the
 * strict-history authority: it replays this request and may reject an otherwise eligible
 * capture when its executable model proves the supplied current state inconsistent.
 */
export type RustAdvisorInputResult =
  | { readonly supported: true; readonly request: RustAdvisorRequest }
  | { readonly supported: false; readonly reason: string };

const encoder = new TextEncoder();
const FNV_OFFSET = 0xcbf29ce484222325n;
const FNV_PRIME = 0x100000001b3n;
const U64_MASK = 0xffffffffffffffffn;

function fnv1a64(bytes: Uint8Array): string {
  let hash = FNV_OFFSET;
  for (const byte of bytes) {
    hash = ((hash ^ BigInt(byte)) * FNV_PRIME) & U64_MASK;
  }
  return hash.toString(16).padStart(16, "0");
}

function u64le(value: number): Uint8Array {
  const out = new Uint8Array(8);
  new DataView(out.buffer).setBigUint64(0, BigInt(value), true);
  return out;
}

function joinBytes(parts: readonly Uint8Array[]): Uint8Array {
  const out = new Uint8Array(
    parts.reduce((total, part) => total + part.length, 0),
  );
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

/**
 * Read and fingerprint the exact raw worker inputs.  In particular, do not parse and
 * re-serialise JSON: whitespace changes are deliberately part of Rust's provenance.
 */
export async function readRustV1Provenance(): Promise<RustProvenance> {
  const root = new URL("../../", import.meta.url);
  const [
    registry,
    catalog,
    overrides,
    registrySource,
    compilerSource,
    catalogMatchSource,
    advisorSearchSource,
  ] = await Promise.all([
    Deno.readFile(new URL("captures/abilities.json", root)),
    Deno.readFile(new URL("data/data.json", root)),
    Deno.readFile(new URL("data/battle_card_overrides.json", root)),
    Deno.readTextFile(new URL("rust/src/effect_registry.rs", root)),
    Deno.readTextFile(
      new URL("rust/src/engine/combat_stat_compiler.rs", root),
    ),
    Deno.readTextFile(new URL("rust/src/engine/catalog_match.rs", root)),
    Deno.readTextFile(new URL("rust/src/advisor/search.rs", root)),
  ]);
  const version = /pub const EFFECT_REGISTRY_SCHEMA_VERSION:\s*u16\s*=\s*(\d+);/
    .exec(registrySource)?.[1];
  if (
    version === undefined ||
    Number(version) !== EFFECT_REGISTRY_SCHEMA_VERSION_V1
  ) {
    throw new Error(
      "Rust V1 effect-registry schema version is missing or unsupported",
    );
  }
  const semanticRevision = (
    source: string,
    name: string,
  ): number => {
    const revision = new RegExp(
      `(?:pub(?:\\(crate\\))?\\s+)?const\\s+${name}:\\s*u16\\s*=\\s*(\\d+);`,
    ).exec(source)?.[1];
    if (revision === undefined) {
      throw new Error(`Rust semantic revision ${name} is missing`);
    }
    return Number(revision);
  };
  const effective = joinBytes([
    encoder.encode("urban-recreation-effective-catalog-v1\0"),
    u64le(catalog.length),
    catalog,
    u64le(overrides.length),
    overrides,
  ]);
  return {
    effectiveCatalogFingerprintFnv1a64: fnv1a64(effective),
    effectRegistryFingerprintFnv1a64: fnv1a64(registry),
    effectRegistrySchemaVersion: Number(version),
    compilerPolicySemanticRevision: semanticRevision(
      compilerSource,
      "COMBAT_STAT_COMPILER_POLICY_SEMANTIC_REVISION_V1",
    ),
    catalogContextPolicySemanticRevision: semanticRevision(
      catalogMatchSource,
      "CATALOG_CONTEXT_POLICY_SEMANTIC_REVISION_V1",
    ),
    advisorPolicySemanticRevision: semanticRevision(
      advisorSearchSource,
      "ADVISOR_POLICY_SEMANTIC_REVISION_V1",
    ),
  };
}

let cachedProvenance: ReturnType<typeof readRustV1Provenance> | undefined;

/**
 * Cache the expensive byte-for-byte hashes for the process lifetime.  A catalog or effect
 * registry change requires restarting this host; until then the independently loaded Rust
 * worker will reject our stale provenance rather than accept a mismatched request.  Leave
 * `readRustV1Provenance` exported for tests and tooling which need to read them afresh.
 */
export function rustV1Provenance(): ReturnType<typeof readRustV1Provenance> {
  if (cachedProvenance === undefined) {
    cachedProvenance = readRustV1Provenance().catch((error: unknown) => {
      cachedProvenance = undefined;
      throw error;
    });
  }
  return cachedProvenance;
}

function unsupported(reason: string): RustAdvisorInputResult {
  return { supported: false, reason: `Rust advisor V3 unsupported: ${reason}` };
}

function integer(value: unknown, where: string, min = 0): number | string {
  if (
    typeof value !== "number" || !Number.isSafeInteger(value) || value < min
  ) return `${where} must be a finite integer >= ${min}`;
  return value;
}

function side(value: unknown, where: string): Side | string {
  return value === 0 || value === 1
    ? value
    : `${where} must be server side 0 or 1`;
}

function wire(value: Turn): WirePlayer {
  return value === Turn.PLAYER_1 ? "p1" : "p2";
}

function opposite(value: Side): Side {
  return (1 - value) as Side;
}

function resources(
  life: unknown,
  pillz: unknown,
  where: string,
): { life: number; pillz: number } | string {
  const validLife = integer(life, `${where}.life`, 1);
  const validPillz = integer(pillz, `${where}.pillz`);
  if (typeof validLife === "string") return validLife;
  if (typeof validPillz === "string") return validPillz;
  if (validPillz > 30) return `${where}.pillz exceeds Rust V3's limit of 30`;
  return { life: validLife, pillz: validPillz };
}

function observation(
  value: unknown,
  fallback: { id: number; description: string },
  where: string,
): { id: number; description: string } | string {
  if (value === null) return fallback;
  if (
    value === undefined || typeof value !== "object" || Array.isArray(value)
  ) {
    return `${where} must be an observed identity or null`;
  }
  const candidate = value as Record<string, unknown>;
  const id = integer(candidate.id, `${where}.id`);
  if (typeof id === "string") return id;
  if (
    typeof candidate.description !== "string" ||
    candidate.description.length === 0
  ) {
    return `${where}.description must be a non-empty string`;
  }
  return { id, description: candidate.description };
}

function cards(
  rec: Reconstructed,
  serverSide: Side,
  game: Game,
  engineSide: Turn,
): readonly RustCardIdentity[] | string {
  const player = rec.players[serverSide];
  if (
    player === undefined || !Array.isArray(player.hand) ||
    player.hand.length !== 4
  ) {
    return `server side ${serverSide} must have exactly four cards`;
  }
  const engineHand = engineSide === Turn.PLAYER_1 ? game.h1 : game.h2;
  const result: RustCardIdentity[] = [];
  for (let slot = 0; slot < 4; slot++) {
    const card = player.hand[slot];
    if (card === undefined || card.index !== slot) {
      return `server side ${serverSide} hand must be ordered by explicit indexes 0..3`;
    }
    const id = integer(card.id, `players.${serverSide}.hand[${slot}].id`, 1);
    const level = integer(
      card.level,
      `players.${serverSide}.hand[${slot}].level`,
      1,
    );
    if (typeof id === "string") return id;
    if (typeof level === "string") return level;
    if (engineHand[slot]?.id !== id || engineHand[slot]?.stars !== level) {
      return `engine hand does not match server side ${serverSide} card ${slot}`;
    }
    const ability = observation(card.ability, {
      id: 0,
      description: "No Ability",
    }, `players.${serverSide}.hand[${slot}].ability`);
    const bonus = observation(
      card.bonus,
      { id: 0, description: "No Bonus" },
      `players.${serverSide}.hand[${slot}].bonus`,
    );
    if (typeof ability === "string") return ability;
    if (typeof bonus === "string") return bonus;
    result.push({
      id,
      level,
      abilityId: ability.id,
      ability: ability.description,
      bonusId: bonus.id,
      bonus: bonus.description,
    });
  }
  return result;
}

type CurrentRound =
  | { readonly kind: "empty"; readonly round: number }
  | {
    readonly kind: "first-selected";
    readonly round: number;
    readonly index: number;
  }
  | undefined;

interface CaptureHistory {
  readonly completed: readonly RustHistoryRound[];
  readonly current: CurrentRound;
}

/** Strictly transform completed site rounds into Rust's engine-oriented history. */
function history(
  rec: Reconstructed,
  first: Side,
): CaptureHistory | string {
  const result: RustHistoryRound[] = [];
  let expectedFirst = first;
  for (const [roundIndex, round] of rec.rounds.entries()) {
    if (round.moves.length === 0) {
      // Live `reconstruct()` already knows the scheduled mover before either card is
      // committed.  Accept that evidence when it agrees with the completed history, but
      // reject a stale or impossible mover just as strictly as a populated tail.
      if (round.first !== null && round.first !== expectedFirst) {
        return `empty current round ${roundIndex} has a contradictory first mover`;
      }
      if (
        rec.rounds.slice(roundIndex + 1).some((tail) => tail.moves.length !== 0)
      ) {
        return `round ${roundIndex} is empty but a later round is populated`;
      }
      return {
        completed: result,
        current: { kind: "empty", round: roundIndex },
      };
    }
    if (round.moves.length === 1) {
      if (round.first === null || round.round !== roundIndex) {
        return `round ${roundIndex} first mover does not follow capture alternation`;
      }
      if (round.first !== expectedFirst) {
        return `round ${roundIndex} first mover does not follow capture alternation`;
      }
      if (
        rec.rounds.slice(roundIndex + 1).some((tail) => tail.moves.length !== 0)
      ) {
        return `round ${roundIndex} is partial but a later round is populated`;
      }
      const move = round.moves[0]!;
      const moveSide = side(move.side, `round ${roundIndex}.move.side`);
      if (typeof moveSide === "string") return moveSide;
      if (moveSide !== expectedFirst) {
        return `round ${roundIndex} partial move is not from the scheduled first mover`;
      }
      const index = integer(move.index, `round ${roundIndex}.move.index`);
      const pillz = integer(move.pillz, `round ${roundIndex}.move.pillz`);
      if (typeof index === "string" || index > 3) {
        return typeof index === "string"
          ? index
          : `round ${roundIndex}.move.index must be 0..3`;
      }
      if (typeof pillz === "string" || pillz > 30) {
        return typeof pillz === "string"
          ? pillz
          : `round ${roundIndex}.move.pillz exceeds Rust V3's limit of 30`;
      }
      if (typeof move.fury !== "boolean") {
        return `round ${roundIndex}.move.fury must be boolean`;
      }
      if (rec.players[moveSide]?.hand[index]?.id !== move.cardId) {
        return `round ${roundIndex}.move card identity does not match its hand slot`;
      }
      return {
        completed: result,
        current: { kind: "first-selected", round: roundIndex, index },
      };
    }
    if (round.moves.length !== 2 || round.first === null) {
      return `round ${roundIndex} is not a completely resolved two-player round`;
    }
    if (round.round !== roundIndex || round.first !== expectedFirst) {
      return `round ${roundIndex} first mover does not follow capture alternation`;
    }
    const current = resources(
      round.life[0],
      round.pillz[0],
      `round ${roundIndex}.side0`,
    );
    const other = resources(
      round.life[1],
      round.pillz[1],
      `round ${roundIndex}.side1`,
    );
    if (typeof current === "string") return current;
    if (typeof other === "string") return other;
    const bySide = new Map<Side, typeof round.moves[number]>();
    for (const move of round.moves) {
      const moveSide = side(move.side, `round ${roundIndex}.move.side`);
      if (typeof moveSide === "string") return moveSide;
      if (bySide.has(moveSide)) {
        return `round ${roundIndex} has duplicate side ${moveSide} moves`;
      }
      const index = integer(move.index, `round ${roundIndex}.move.index`);
      const pillz = integer(move.pillz, `round ${roundIndex}.move.pillz`);
      if (typeof index === "string" || index > 3) {
        return typeof index === "string"
          ? index
          : `round ${roundIndex}.move.index must be 0..3`;
      }
      if (typeof pillz === "string" || pillz > 30) {
        return typeof pillz === "string"
          ? pillz
          : `round ${roundIndex}.move.pillz exceeds Rust V3's limit of 30`;
      }
      if (typeof move.fury !== "boolean") {
        return `round ${roundIndex}.move.fury must be boolean`;
      }
      const hand = rec.players[moveSide]?.hand;
      if (hand?.[index]?.id !== move.cardId) {
        return `round ${roundIndex}.move card identity does not match its hand slot`;
      }
      bySide.set(moveSide, move);
    }
    const p1 = bySide.get(first);
    const p2 = bySide.get(opposite(first));
    if (p1 === undefined || p2 === undefined) {
      return `round ${roundIndex} must contain one move per side`;
    }
    result.push({
      firstMover: roundIndex % 2 === 0 ? "p1" : "p2",
      p1: { handIndex: p1.index, pillz: p1.pillz, fury: p1.fury },
      p2: { handIndex: p2.index, pillz: p2.pillz, fury: p2.fury },
    });
    expectedFirst = opposite(expectedFirst);
  }
  if (result.length > 3) return "Rust V3 accepts at most three resolved rounds";
  return { completed: result, current: undefined };
}

/**
 * Return a V3 request for the exact live information set. Server resources are read directly
 * from the latest resolved capture round; the potentially reconciled TS Game is used solely
 * to establish decision orientation and independently check card/mask state.
 */
export async function normaliseRustAdvisorInput(
  context: RustAdvisorInputContext,
): Promise<RustAdvisorInputResult> {
  const { rec, game, decision } = context;
  const mode = decision.mode === SearchMode.FIRST
    ? "first"
    : decision.mode === SearchMode.SECOND
    ? "second"
    : decision.mode === SearchMode.BLIND_SECOND
    ? "blind_second"
    : undefined;
  if (mode === undefined) {
    return unsupported(`unknown TS Search mode ${decision.mode}`);
  }
  if (rec.testcase == null) return unsupported("capture has no testcase");
  const first = side(rec.firstPlayer, "firstPlayer");
  const mine = side(rec.mySide, "mySide");
  if (typeof first === "string") return unsupported(first);
  if (typeof mine === "string") return unsupported(mine);
  if (rec.result !== null || !game.isPlaying) {
    return unsupported("battle is already complete");
  }
  const captureHistory = history(rec, first);
  if (typeof captureHistory === "string") return unsupported(captureHistory);
  const completed = captureHistory.completed;
  // These state bits are the authoritative distinction between a blind snapshot and a
  // revealed-card snapshot. Check them before turn orientation so contradictions report the
  // actual missing/extra commitment rather than its downstream turn consequence.
  if (mode === "second" && !game.firstHasSelected) {
    return unsupported("second mode requires a selected first-moving card");
  }
  if (mode === "blind_second" && game.firstHasSelected) {
    return unsupported(
      "blind-second mode cannot have a selected first-moving card",
    );
  }
  if (mode === "blind_second" && completed.length === 0) {
    return unsupported(
      "blind-second mode requires at least one completed round",
    );
  }
  const expectedFirstTurn = completed.length % 2 === 0
    ? Turn.PLAYER_1
    : Turn.PLAYER_2;
  if (game.round !== completed.length + 1) {
    return unsupported(
      "engine round does not match the completed capture history",
    );
  }
  const expectedRequester = mode === "first"
    ? expectedFirstTurn
    : expectedFirstTurn === Turn.PLAYER_1
    ? Turn.PLAYER_2
    : Turn.PLAYER_1;
  const expectedGameTurn = mode === "blind_second"
    ? expectedFirstTurn
    : expectedRequester;
  if (decision.us !== expectedRequester || game.turn !== expectedGameTurn) {
    return unsupported(
      mode === "first"
        ? "decision requester does not equal the engine's current first mover"
        : "decision requester does not equal the scheduled first mover's opponent",
    );
  }
  const expectedMine = expectedRequester === Turn.PLAYER_1
    ? first
    : opposite(first);
  if (mine !== expectedMine) {
    return unsupported(
      mode === "first"
        ? "our server side is not the current first mover"
        : "our server side is not the scheduled first mover's opponent",
    );
  }
  let opponentHandIndex: number | undefined;
  if (mode === "first") {
    if (game.firstHasSelected) {
      return unsupported("a first-moving card is already selected");
    }
    if (captureHistory.current?.kind === "first-selected") {
      return unsupported(
        "first-mode capture has a selected current-round card",
      );
    }
  } else if (mode === "second") {
    if (!game.firstHasSelected) {
      return unsupported("second mode requires a selected first-moving card");
    }
    if (captureHistory.current?.kind !== "first-selected") {
      return unsupported(
        "second-mode capture requires exactly one current first-side move",
      );
    }
    opponentHandIndex = captureHistory.current.index;
    let selected: number;
    try {
      selected = game.playedCardIndex;
    } catch {
      return unsupported("engine has no readable selected first-moving card");
    }
    if (selected !== opponentHandIndex) {
      return unsupported(
        "engine selected card does not match the capture first-side move",
      );
    }
  } else {
    if (game.firstHasSelected) {
      return unsupported(
        "blind-second mode cannot have a selected first-moving card",
      );
    }
    if (captureHistory.current?.kind === "first-selected") {
      return unsupported(
        "blind-second capture cannot have a selected current-round card",
      );
    }
  }
  const battleRuleId = integer(rec.battleRuleId, "battleRuleId", 0);
  if (typeof battleRuleId === "string") return unsupported(battleRuleId);
  if (battleRuleId !== RUST_V1_BATTLE_RULE_ID) {
    return unsupported(
      `battle rule ${battleRuleId} is not Rust V3 rule ${RUST_V1_BATTLE_RULE_ID}`,
    );
  }
  if (typeof rec.night !== "boolean") {
    return unsupported("night must be boolean");
  }
  const budget = integer(context.budgetMs, "budgetMs", 1);
  if (typeof budget === "string" || budget > 30_000) {
    return unsupported(
      typeof budget === "string"
        ? budget
        : "budgetMs exceeds Rust V3's limit of 30000",
    );
  }
  if (!/^[ -~]{1,128}$/.test(context.requestId)) {
    return unsupported("requestId must be 1..128 printable ASCII bytes");
  }

  const p1Cards = cards(rec, first, game, Turn.PLAYER_1);
  const p2Cards = cards(rec, opposite(first), game, Turn.PLAYER_2);
  if (typeof p1Cards === "string") return unsupported(p1Cards);
  if (typeof p2Cards === "string") return unsupported(p2Cards);
  const p1Initial = resources(
    rec.players[first]?.baseLife,
    rec.players[first]?.basePillz,
    "players.p1.initial",
  );
  const p2Initial = resources(
    rec.players[opposite(first)]?.baseLife,
    rec.players[opposite(first)]?.basePillz,
    "players.p2.initial",
  );
  if (typeof p1Initial === "string") return unsupported(p1Initial);
  if (typeof p2Initial === "string") return unsupported(p2Initial);
  const last = completed.length === 0
    ? undefined
    : rec.rounds[completed.length - 1];
  const p1Current = last === undefined
    ? p1Initial
    : resources(last.life[first], last.pillz[first], "players.p1.current");
  const p2Current = last === undefined ? p2Initial : resources(
    last.life[opposite(first)],
    last.pillz[opposite(first)],
    "players.p2.current",
  );
  if (typeof p1Current === "string") return unsupported(p1Current);
  if (typeof p2Current === "string") return unsupported(p2Current);
  const played = (side: Side) => {
    const marks = [false, false, false, false];
    for (const round of rec.rounds.slice(0, completed.length)) {
      const move = round.moves.find((candidate) => candidate.side === side)!;
      marks[move.index] = true;
    }
    return marks;
  };
  const p1Played = played(first), p2Played = played(opposite(first));
  // The worker replays only completed rounds, so its wire masks must describe that clean
  // root.  The live TS Game additionally marks the currently revealed first card; account
  // for that transient mark only while cross-checking the host state, never on the wire.
  const p1EnginePlayed = [...p1Played], p2EnginePlayed = [...p2Played];
  if (mode === "second") {
    const marks = expectedFirstTurn === Turn.PLAYER_1
      ? p1EnginePlayed
      : p2EnginePlayed;
    marks[opponentHandIndex!] = true;
  }
  const masksMatch = (hand: Game["h1"], expected: readonly boolean[]) =>
    expected.every((value, index) => hand[index]?.played === value);
  if (
    !masksMatch(game.h1, p1EnginePlayed) ||
    !masksMatch(game.h2, p2EnginePlayed)
  ) {
    return unsupported(
      `engine played masks ${
        game.h1.map((card) => Number(card.played)).join("")
      }/` +
        `${game.h2.map((card) => Number(card.played)).join("")} do not match ` +
        `capture ${p1EnginePlayed.map(Number).join("")}/${
          p2EnginePlayed.map(Number).join("")
        }`,
    );
  }
  const provenance = await rustV1Provenance();
  const input = {
    requestId: context.requestId,
    us: wire(decision.us),
    firstMover: wire(expectedFirstTurn),
    battleRuleId,
    night: rec.night,
    provenance,
    players: {
      p1: {
        initial: p1Initial,
        current: p1Current,
        played: p1Played,
        hand: p1Cards,
      },
      p2: {
        initial: p2Initial,
        current: p2Current,
        played: p2Played,
        hand: p2Cards,
      },
    },
    history: completed,
    budgetMs: budget,
  };
  return {
    supported: true,
    request: mode === "second"
      ? buildAdvisorRequest({
        ...input,
        mode,
        opponentHandIndex: opponentHandIndex!,
      })
      : buildAdvisorRequest({ ...input, mode }),
  };
}

/** US spelling for callers outside the existing British-spelling source tree. */
export const normalizeRustAdvisorInput = normaliseRustAdvisorInput;
