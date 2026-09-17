// Strict, one-request-per-process bridge for the experimental Rust advisor.
//
// This module deliberately knows nothing about Advisor.Reconstructed.  Callers build a
// fully-normalised DTO from their public snapshot, then hand it to buildFirstRequest().
// Keeping that boundary here makes the wire format reviewable and avoids accidentally
// serialising a private live-site reconstruction into a child process.
import Game from "../game/Game.ts";
import { Turn } from "../game/types/Types.ts";
import Search, {
  type Candidate,
  type Move,
  SearchMode,
  type SearchStats,
} from "./Search.ts";

export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | JsonObject | readonly JsonValue[];
export interface JsonObject {
  readonly [key: string]: JsonValue;
}

export interface RustCardIdentity {
  readonly id: number;
  readonly level: number;
  readonly abilityId: number;
  readonly ability: string;
  readonly bonusId: number;
  readonly bonus: string;
}

/** A public, already-normalised projection; this module intentionally does not build it. */
export type RustWirePlayer = "p1" | "p2";

export interface RustResources {
  readonly life: number;
  readonly pillz: number;
}

export interface RustPlayerState {
  readonly initial: RustResources;
  readonly current: RustResources;
  readonly played: readonly boolean[];
  readonly hand: readonly RustCardIdentity[];
}

export interface RustProvenance {
  readonly effectiveCatalogFingerprintFnv1a64: string;
  readonly effectRegistryFingerprintFnv1a64: string;
  readonly effectRegistrySchemaVersion: number;
}

export interface RustHistoryMove {
  readonly handIndex: number;
  readonly pillz: number;
  readonly fury: boolean;
}

export interface RustHistoryRound {
  readonly firstMover: RustWirePlayer;
  readonly p1: RustHistoryMove;
  readonly p2: RustHistoryMove;
}

export interface RustFirstInput {
  readonly requestId: string;
  readonly us: RustWirePlayer;
  readonly firstMover: RustWirePlayer;
  readonly battleRuleId: number;
  readonly night: boolean;
  /** Versioned catalog/effect fingerprints supplied by the caller's normaliser. */
  readonly provenance: RustProvenance;
  readonly players: {
    readonly p1: RustPlayerState;
    readonly p2: RustPlayerState;
  };
  readonly history: readonly RustHistoryRound[];
  readonly budgetMs: number;
}

export interface RustFirstRequest {
  readonly protocol_version: 1;
  readonly request_id: string;
  readonly mode: "first";
  readonly us: RustWirePlayer;
  readonly first_mover: RustWirePlayer;
  readonly battle_rule_id: number;
  readonly night: boolean;
  readonly provenance: {
    readonly effective_catalog_fingerprint_fnv1a64: string;
    readonly effect_registry_fingerprint_fnv1a64: string;
    readonly effect_registry_schema_version: number;
  };
  readonly players: {
    readonly p1: RustWirePlayerState;
    readonly p2: RustWirePlayerState;
  };
  readonly history: readonly {
    readonly first_mover: RustWirePlayer;
    readonly p1: {
      readonly hand_index: number;
      readonly pillz: number;
      readonly fury: boolean;
    };
    readonly p2: {
      readonly hand_index: number;
      readonly pillz: number;
      readonly fury: boolean;
    };
  }[];
  readonly budget_ms: number;
}

export interface RustWirePlayerState {
  readonly initial: RustResources;
  readonly current: RustResources;
  readonly played: readonly boolean[];
  readonly hand: readonly {
    id: number;
    level: number;
    ability_id: number;
    ability: string;
    bonus_id: number;
    bonus: string;
  }[];
}

export const RUST_ADVISOR_VERSION = 1 as const;
/** Rust permits 32 progress records plus one final, 65,536 bytes each including newlines. */
export const DEFAULT_MAX_JSONL_BYTES = 2_162_721;
export const DEFAULT_MAX_STDERR_BYTES = 64_000;
export const DEFAULT_MAX_JSONL_LINES = 1_024;
export const DEFAULT_MAX_REQUEST_BYTES = 65_536;

export class RustAdvisorProtocolError extends Error {
  constructor(message: string) {
    super(`Rust advisor protocol: ${message}`);
    this.name = "RustAdvisorProtocolError";
  }
}

export class RustAdvisorTimeoutError extends Error {
  constructor(readonly timeoutMs: number) {
    super(`Rust advisor timed out after ${timeoutMs}ms`);
    this.name = "RustAdvisorTimeoutError";
  }
}

export class RustAdvisorCancelledError extends Error {
  constructor() {
    super("Rust advisor cancelled");
    this.name = "RustAdvisorCancelledError";
  }
}

function fail(message: string): never {
  throw new RustAdvisorProtocolError(message);
}

function object(value: unknown, where: string): Record<string, unknown> {
  if (
    value === null || typeof value !== "object" || Array.isArray(value) ||
    Object.getPrototypeOf(value) !== Object.prototype
  ) fail(`${where} must be an object`);
  return value as Record<string, unknown>;
}

function keys(
  value: Record<string, unknown>,
  expected: readonly string[],
  where: string,
) {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (
    actual.length !== wanted.length ||
    actual.some((key, i) => key !== wanted[i])
  ) {
    fail(`${where} has unknown or missing fields`);
  }
}

function string(value: unknown, where: string): string {
  if (typeof value !== "string" || value.length === 0) {
    fail(`${where} must be a non-empty string`);
  }
  return value;
}

function finite(value: unknown, where: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    fail(`${where} must be finite`);
  }
  return value;
}

function integer(value: unknown, where: string, min = 0): number {
  const result = finite(value, where);
  if (!Number.isSafeInteger(result) || result < min) {
    fail(`${where} must be an integer >= ${min}`);
  }
  return result;
}

function boundedInteger(
  value: unknown,
  where: string,
  min: number,
  max: number,
): number {
  const result = integer(value, where, min);
  if (result > max) fail(`${where} must be in ${min}..=${max}`);
  return result;
}

function boundedString(value: unknown, where: string, max: number): string {
  if (typeof value !== "string" || value.length > max) {
    fail(`${where} must be a string no longer than ${max} characters`);
  }
  return value;
}

function score(value: unknown, where: string): number {
  const result = finite(value, where);
  if (result < -1 || result > 1) fail(`${where} must be in [-1, 1]`);
  return result;
}

function evaluationKind(
  value: unknown,
  where: string,
): "opening_estimate" | "exact_continuation_policy" {
  if (value === "opening_estimate" || value === "exact_continuation_policy") {
    return value;
  }
  fail(`${where} is not a supported evaluation_kind`);
}

function json(value: JsonValue, where: string): JsonValue {
  if (typeof value === "number" && !Number.isFinite(value)) {
    fail(`${where} contains a non-finite number`);
  }
  if (
    value === null || typeof value === "string" || typeof value === "boolean" ||
    typeof value === "number"
  ) return value;
  if (Array.isArray(value)) {
    return value.map((entry, index) => json(entry, `${where}[${index}]`));
  }
  const source = object(value, where);
  const out: Record<string, JsonValue> = {};
  for (const [key, entry] of Object.entries(source)) {
    out[key] = json(entry as JsonValue, `${where}.${key}`);
  }
  return out;
}

/**
 * The V1 request builder.  It validates and copies the supplied DTO but makes no attempt
 * to infer its contents from a capture or the private Advisor reconstruction.
 */
export function buildFirstRequest(input: RustFirstInput): RustFirstRequest {
  const requestId = string(input.requestId, "requestId");
  if (requestId.length > 128 || !/^[\x20-\x7e]+$/.test(requestId)) {
    fail("requestId must be 1..=128 printable ASCII bytes");
  }
  const resources = (value: RustResources, field: string): RustResources => {
    keys(object(value, field), ["life", "pillz"], field);
    return {
      life: boundedInteger(value.life, `${field}.life`, 1, 255),
      pillz: boundedInteger(value.pillz, `${field}.pillz`, 0, 30),
    };
  };
  const player = (
    side: RustPlayerState,
    where: string,
  ): RustWirePlayerState => {
    keys(object(side, where), ["initial", "current", "played", "hand"], where);
    if (
      !Array.isArray(side.played) || side.played.length !== 4 ||
      side.played.some((value) => typeof value !== "boolean")
    ) fail(`${where}.played must be exactly four booleans`);
    if (!Array.isArray(side.hand) || side.hand.length !== 4) {
      fail(`${where}.hand must contain exactly four cards`);
    }
    const hand = side.hand.map((card, index) => {
      const cardWhere = `${where}.hand[${index}]`;
      keys(object(card, cardWhere), [
        "id",
        "level",
        "abilityId",
        "ability",
        "bonusId",
        "bonus",
      ], cardWhere);
      return {
        id: boundedInteger(card.id, `${cardWhere}.id`, 1, 0xffffffff),
        level: boundedInteger(card.level, `${cardWhere}.level`, 1, 255),
        ability_id: boundedInteger(
          card.abilityId,
          `${cardWhere}.abilityId`,
          0,
          0xffffffff,
        ),
        ability: boundedString(card.ability, `${cardWhere}.ability`, 256),
        bonus_id: boundedInteger(
          card.bonusId,
          `${cardWhere}.bonusId`,
          0,
          0xffffffff,
        ),
        bonus: boundedString(card.bonus, `${cardWhere}.bonus`, 256),
      };
    });
    return {
      initial: resources(side.initial, `${where}.initial`),
      current: resources(side.current, `${where}.current`),
      played: [...side.played],
      hand,
    };
  };
  if (input.us !== "p1" && input.us !== "p2") fail("us must be p1 or p2");
  if (input.firstMover !== "p1" && input.firstMover !== "p2") {
    fail("firstMover must be p1 or p2");
  }
  if (input.us !== input.firstMover) {
    fail("V1 first mode requires us and firstMover to match");
  }
  if (input.battleRuleId !== 10) fail("V1 supports battleRuleId 10 only");
  if (typeof input.night !== "boolean") fail("night must be boolean");
  keys(object(input.provenance, "provenance"), [
    "effectiveCatalogFingerprintFnv1a64",
    "effectRegistryFingerprintFnv1a64",
    "effectRegistrySchemaVersion",
  ], "provenance");
  const fingerprint = (value: string, field: string) => {
    if (!/^[0-9a-f]{16}$/.test(value)) {
      fail(`${field} must be 16 lowercase hexadecimal characters`);
    }
    return value;
  };
  if (!Array.isArray(input.history) || input.history.length > 3) {
    fail("history must contain at most three rounds");
  }
  const move = (value: RustHistoryMove, where: string) => {
    keys(object(value, where), ["handIndex", "pillz", "fury"], where);
    if (typeof value.fury !== "boolean") fail(`${where}.fury must be boolean`);
    return {
      hand_index: boundedInteger(value.handIndex, `${where}.handIndex`, 0, 3),
      pillz: boundedInteger(value.pillz, `${where}.pillz`, 0, 30),
      fury: value.fury,
    };
  };
  let expectedHistoryFirst: RustWirePlayer | undefined;
  const history = input.history.map((round, index) => {
    const where = `history[${index}]`;
    keys(object(round, where), ["firstMover", "p1", "p2"], where);
    if (round.firstMover !== "p1" && round.firstMover !== "p2") {
      fail(`${where}.firstMover must be p1 or p2`);
    }
    if (
      expectedHistoryFirst !== undefined &&
      round.firstMover !== expectedHistoryFirst
    ) fail(`${where}.firstMover does not alternate`);
    expectedHistoryFirst = round.firstMover === "p1" ? "p2" : "p1";
    return {
      first_mover: round.firstMover,
      p1: move(round.p1, `${where}.p1`),
      p2: move(round.p2, `${where}.p2`),
    };
  });
  if (
    expectedHistoryFirst !== undefined &&
    expectedHistoryFirst !== input.firstMover
  ) fail("firstMover must follow history alternation");
  return {
    protocol_version: RUST_ADVISOR_VERSION,
    request_id: requestId,
    mode: "first",
    us: input.us,
    first_mover: input.firstMover,
    battle_rule_id: 10,
    night: input.night,
    provenance: {
      effective_catalog_fingerprint_fnv1a64: fingerprint(
        input.provenance.effectiveCatalogFingerprintFnv1a64,
        "provenance.effectiveCatalogFingerprintFnv1a64",
      ),
      effect_registry_fingerprint_fnv1a64: fingerprint(
        input.provenance.effectRegistryFingerprintFnv1a64,
        "provenance.effectRegistryFingerprintFnv1a64",
      ),
      effect_registry_schema_version: boundedInteger(
        input.provenance.effectRegistrySchemaVersion,
        "provenance.effectRegistrySchemaVersion",
        0,
        65535,
      ),
    },
    players: {
      p1: player(input.players.p1, "players.p1"),
      p2: player(input.players.p2, "players.p2"),
    },
    history,
    budget_ms: boundedInteger(input.budgetMs, "budgetMs", 1, 30_000),
  };
}

export interface RustRankedEntry {
  readonly handIndex: number;
  readonly pillz: number;
  readonly fury: boolean;
  /** Values are in Rust's `our` frame: +1 means the player asking for advice wins. */
  readonly score: number;
  readonly worst: number;
  readonly best: number;
  readonly samples: number;
  readonly kos: number;
  readonly koed: number;
}

export interface RustAdvisorUpdate {
  readonly kind: "progress" | "final";
  readonly sequence: number;
  readonly evaluationKind: "opening_estimate" | "exact_continuation_policy";
  readonly complete: boolean;
  readonly unitsDone: number;
  readonly unitsTotal: number;
  readonly elapsedMs: number;
  readonly ranked: readonly RustRankedEntry[];
}

export interface RustAdvisorFinal extends RustAdvisorUpdate {
  readonly kind: "final";
}

export interface RustAdvisorTranscript {
  readonly progress: readonly RustAdvisorUpdate[];
  readonly final: RustAdvisorFinal;
}

export interface ResponseExpectation {
  readonly version: 1;
  readonly requestId: string;
  /** Every update must describe this exact action set, once each. */
  readonly candidates?: readonly Pick<Move, "index" | "pillz" | "fury">[];
}

function normaliseRanked(value: unknown, where: string): RustRankedEntry {
  const entry = object(value, where);
  const base = [
    "hand_index",
    "pillz",
    "fury",
    "score",
    "worst",
    "best",
    "samples",
  ];
  keys(entry, [...base, "ko_share", "loss_share"], where);
  const samples = integer(entry.samples, `${where}.samples`, 1);
  const scoreValue = score(entry.score, `${where}.score`);
  const worst = score(entry.worst, `${where}.worst`);
  const best = score(entry.best, `${where}.best`);
  if (worst > scoreValue || scoreValue > best) {
    fail(`${where} score must fall between worst and best`);
  }
  const share = (field: "ko_share" | "loss_share") => {
    const result = finite(entry[field], `${where}.${field}`);
    if (result < 0 || result > 1) fail(`${where}.${field} must be in [0, 1]`);
    const count = result * samples;
    if (Math.abs(count - Math.round(count)) > 1e-9) {
      fail(`${where}.${field} is not a whole-sample share`);
    }
    return Math.round(count);
  };
  const kos = share("ko_share");
  const koed = share("loss_share");
  if (kos > samples || koed > samples) {
    fail(`${where} KO count exceeds samples`);
  }
  if (typeof entry.fury !== "boolean") fail(`${where}.fury must be boolean`);
  return {
    handIndex: boundedInteger(entry.hand_index, `${where}.hand_index`, 0, 3),
    pillz: boundedInteger(entry.pillz, `${where}.pillz`, 0, 30),
    fury: entry.fury,
    score: scoreValue,
    worst,
    best,
    samples,
    kos,
    koed,
  };
}

function checkCandidates(
  ranked: readonly RustRankedEntry[],
  expected?: ResponseExpectation["candidates"],
  exact = false,
) {
  const key = (move: Pick<Move, "index" | "pillz" | "fury">) =>
    `${move.index}:${move.pillz}:${move.fury}`;
  const seen = new Set<string>();
  for (const entry of ranked) {
    const action = key({
      index: entry.handIndex,
      pillz: entry.pillz,
      fury: entry.fury,
    });
    if (seen.has(action)) fail(`duplicate ranked candidate ${action}`);
    seen.add(action);
  }
  if (expected !== undefined) {
    const wanted = new Set(expected.map(key));
    if (
      wanted.size !== expected.length ||
      [...seen].some((entry) => !wanted.has(entry))
    ) {
      fail("ranked candidate is not in the requested action set");
    }
    if (exact && wanted.size !== seen.size) {
      fail(
        "final ranked candidates do not exactly match the requested action set",
      );
    }
  }
}

function decodeLine(
  line: string,
  expectation: ResponseExpectation,
  lineNumber: number,
): RustAdvisorUpdate {
  let parsed: unknown;
  try {
    parsed = JSON.parse(line);
  } catch {
    fail(`line ${lineNumber} is not JSON`);
  }
  const message = object(parsed, `line ${lineNumber}`);
  const common = ["protocol_version", "request_id", "sequence", "kind"];
  const kind = string(message.kind, `line ${lineNumber}.kind`);
  if (kind !== "progress" && kind !== "final") {
    fail(`line ${lineNumber} has unknown kind ${kind}`);
  }
  keys(message, [
    ...common,
    "score_frame",
    "evaluation_kind",
    "complete",
    "units_done",
    "units_total",
    "elapsed_ms",
    "ranked_moves",
  ], `line ${lineNumber}`);
  if (
    message.protocol_version !== expectation.version ||
    message.request_id !== expectation.requestId
  ) {
    fail(`line ${lineNumber} does not echo this request`);
  }
  if (message.score_frame !== "requester") {
    fail(`line ${lineNumber} has unsupported score frame`);
  }
  integer(message.sequence, `line ${lineNumber}.sequence`, 0);
  if (typeof message.complete !== "boolean") {
    fail(`line ${lineNumber}.complete must be boolean`);
  }
  if (kind === "progress" && message.complete) {
    fail(`line ${lineNumber} progress cannot be complete`);
  }
  if (!Array.isArray(message.ranked_moves)) {
    fail(`line ${lineNumber}.ranked_moves must be an array`);
  }
  const ranked = message.ranked_moves.map((entry, index) =>
    normaliseRanked(entry, `line ${lineNumber}.ranked_moves[${index}]`)
  );
  checkCandidates(
    ranked,
    expectation.candidates,
    kind === "final" && message.complete === true,
  );
  const unitsDone = integer(
    message.units_done,
    `line ${lineNumber}.units_done`,
    0,
  );
  const unitsTotal = integer(
    message.units_total,
    `line ${lineNumber}.units_total`,
    0,
  );
  if (unitsDone > unitsTotal) {
    fail(`line ${lineNumber}.unitsDone exceeds unitsTotal`);
  }
  if (message.complete === true && unitsDone !== unitsTotal) {
    fail(`line ${lineNumber} complete response has unfinished units`);
  }
  const update: RustAdvisorUpdate = {
    kind: kind as "progress" | "final",
    sequence: integer(message.sequence, `line ${lineNumber}.sequence`, 0),
    evaluationKind: evaluationKind(
      message.evaluation_kind,
      `line ${lineNumber}.evaluation_kind`,
    ),
    complete: message.complete,
    unitsDone,
    unitsTotal,
    elapsedMs: integer(message.elapsed_ms, `line ${lineNumber}.elapsed_ms`, 0),
    ranked,
  };
  return update;
}

/** Decode the complete, bounded stdout of one worker process.  Any ambiguity fails closed. */
export function decodeRustJsonl(
  stdout: string,
  expectation: ResponseExpectation,
  options: { maxBytes?: number; maxLines?: number } = {},
): RustAdvisorTranscript {
  const maxBytes = options.maxBytes ?? DEFAULT_MAX_JSONL_BYTES;
  const maxLines = options.maxLines ?? DEFAULT_MAX_JSONL_LINES;
  if (new TextEncoder().encode(stdout).byteLength > maxBytes) {
    fail("stdout exceeds byte limit");
  }
  const lines = stdout.split("\n");
  if (lines.at(-1) === "") lines.pop();
  if (
    lines.length === 0 || lines.length > maxLines ||
    lines.some((line) => line.length === 0)
  ) fail("stdout must contain bounded non-empty JSONL lines");
  const progress: RustAdvisorUpdate[] = [];
  let final: RustAdvisorFinal | undefined;
  let previousUnits = -1;
  let previousElapsed = -1;
  let unitsTotal: number | undefined;
  for (const [index, line] of lines.entries()) {
    if (final !== undefined) fail("message received after final");
    const update = decodeLine(
      line.endsWith("\r") ? line.slice(0, -1) : line,
      expectation,
      index + 1,
    );
    if (update.sequence !== index) {
      fail(
        `line ${index + 1} sequence must start at zero and increment by one`,
      );
    }
    if (update.unitsDone < previousUnits) {
      fail(`line ${index + 1} units_done is not monotonic`);
    }
    if (update.elapsedMs < previousElapsed) {
      fail(`line ${index + 1} elapsed_ms is not monotonic`);
    }
    if (unitsTotal !== undefined && update.unitsTotal !== unitsTotal) {
      fail(`line ${index + 1} units_total changed`);
    }
    previousUnits = update.unitsDone;
    previousElapsed = update.elapsedMs;
    unitsTotal = update.unitsTotal;
    if (update.kind === "final") final = update as RustAdvisorFinal;
    else progress.push(update);
  }
  if (final === undefined) {
    fail("worker did not return exactly one final response");
  }
  if (!final.complete) {
    fail("worker final response is incomplete");
  }
  return { progress, final };
}

export interface RustRunnerOutput {
  readonly code: number;
  readonly stdout: string;
  readonly stderr: string;
}

export interface RustAdvisorRunner {
  run(
    request: string,
    options: {
      signal: AbortSignal;
      maxRequestBytes: number;
      maxStdoutBytes: number;
      maxStderrBytes: number;
    },
  ): Promise<RustRunnerOutput>;
}

export interface DenoCommandRunnerOptions {
  readonly command: string;
  readonly args?: readonly string[];
}

async function readBounded(
  stream: ReadableStream<Uint8Array>,
  limit: number,
  name: string,
  stop: () => void,
): Promise<Uint8Array> {
  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      total += value.byteLength;
      if (total > limit) {
        stop();
        fail(`${name} exceeds byte limit`);
      }
      chunks.push(value);
    }
  } finally {
    reader.releaseLock();
  }
  const output = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    output.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return output;
}

function decodeUtf8(bytes: Uint8Array, where: string): string {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    fail(`${where} is not valid UTF-8`);
  }
}

/** Production runner; tests should inject a fake rather than spawn a process. */
export class DenoCommandRunner implements RustAdvisorRunner {
  constructor(private readonly command: DenoCommandRunnerOptions) {}

  async run(
    request: string,
    options: {
      signal: AbortSignal;
      maxRequestBytes: number;
      maxStdoutBytes: number;
      maxStderrBytes: number;
    },
  ): Promise<RustRunnerOutput> {
    if (options.signal.aborted) {
      throw options.signal.reason ?? new RustAdvisorCancelledError();
    }
    if (
      new TextEncoder().encode(request).byteLength > options.maxRequestBytes
    ) {
      fail("request exceeds byte limit");
    }
    const child = new Deno.Command(this.command.command, {
      args: [...(this.command.args ?? [])],
      stdin: "piped",
      stdout: "piped",
      stderr: "piped",
    }).spawn();
    let killed = false;
    const stop = () => {
      if (!killed) {
        killed = true;
        try {
          child.kill("SIGTERM");
        } catch { /* process may already have exited */ }
      }
    };
    const abort = () => stop();
    options.signal.addEventListener("abort", abort, { once: true });
    // Close the narrow race between the pre-spawn check and listener registration.
    if (options.signal.aborted) stop();
    // Begin draining both pipes before stdin is written.  A worker that eagerly reports a
    // request error cannot then block behind a full pipe while the parent waits on write.
    const status = child.status;
    const stdout = readBounded(
      child.stdout,
      options.maxStdoutBytes,
      "stdout",
      stop,
    );
    const stderr = readBounded(
      child.stderr,
      options.maxStderrBytes,
      "stderr",
      stop,
    );
    try {
      const writer = child.stdin.getWriter();
      await writer.write(new TextEncoder().encode(request));
      await writer.close();
      const [exit, out, err] = await Promise.all([status, stdout, stderr]);
      if (options.signal.aborted) {
        throw options.signal.reason ?? new RustAdvisorCancelledError();
      }
      return {
        code: exit.code,
        stdout: decodeUtf8(out, "stdout"),
        stderr: decodeUtf8(err, "stderr"),
      };
    } catch (error) {
      stop();
      // Do not return a timeout/cancel result while a child is still live.  In particular,
      // wait for status and both pipe pumps after kill so process-per-decision stays true.
      await Promise.allSettled([status, stdout, stderr]);
      if (options.signal.aborted) {
        throw options.signal.reason ?? new RustAdvisorCancelledError();
      }
      throw error;
    } finally {
      options.signal.removeEventListener("abort", abort);
      stop();
    }
  }
}

export interface RunRustAdvisorOptions {
  readonly timeoutMs?: number;
  readonly signal?: AbortSignal;
  readonly maxRequestBytes?: number;
  readonly maxStdoutBytes?: number;
  readonly maxStderrBytes?: number;
  readonly maxLines?: number;
}

/** Runs one disposable worker and accepts only a clean exit plus one valid final response. */
export async function runRustAdvisor(
  runner: RustAdvisorRunner,
  request: RustFirstRequest,
  expectedCandidates: readonly Pick<Move, "index" | "pillz" | "fury">[],
  options: RunRustAdvisorOptions = {},
): Promise<RustAdvisorTranscript> {
  if (options.signal?.aborted) throw new RustAdvisorCancelledError();
  const controller = new AbortController();
  const onCancel = () => controller.abort(new RustAdvisorCancelledError());
  options.signal?.addEventListener("abort", onCancel, { once: true });
  if (options.signal?.aborted) onCancel();
  const timeoutMs = options.timeoutMs ?? 15_000;
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1) {
    throw new RangeError("timeoutMs must be a positive integer");
  }
  const timer = setTimeout(
    () => controller.abort(new RustAdvisorTimeoutError(timeoutMs)),
    timeoutMs,
  );
  try {
    const encodedRequest = `${JSON.stringify(request)}\n`;
    const maxRequestBytes = options.maxRequestBytes ??
      DEFAULT_MAX_REQUEST_BYTES;
    if (new TextEncoder().encode(encodedRequest).byteLength > maxRequestBytes) {
      throw new RustAdvisorProtocolError("request exceeds byte limit");
    }
    let output: RustRunnerOutput;
    try {
      output = await runner.run(encodedRequest, {
        signal: controller.signal,
        maxRequestBytes,
        maxStdoutBytes: options.maxStdoutBytes ?? DEFAULT_MAX_JSONL_BYTES,
        maxStderrBytes: options.maxStderrBytes ?? DEFAULT_MAX_STDERR_BYTES,
      });
    } catch (error) {
      if (controller.signal.aborted) throw controller.signal.reason;
      throw error;
    }
    if (controller.signal.aborted) throw controller.signal.reason;
    if (output.code !== 0) {
      const diagnostic = Array.from(output.stderr, (character) => {
        const code = character.charCodeAt(0);
        return code <= 0x1f || (code >= 0x7f && code <= 0x9f) ? " " : character;
      }).join("")
        .replace(/\s+/g, " ")
        .trim()
        .slice(0, 512);
      throw new RustAdvisorProtocolError(
        `worker exited with code ${output.code}${
          diagnostic.length === 0 ? "" : `: ${diagnostic}`
        }`,
      );
    }
    return decodeRustJsonl(output.stdout, {
      version: request.protocol_version,
      requestId: request.request_id,
      candidates: expectedCandidates,
    }, { maxBytes: options.maxStdoutBytes, maxLines: options.maxLines });
  } finally {
    clearTimeout(timer);
    options.signal?.removeEventListener("abort", onCancel);
  }
}

/**
 * A completed FIRST-mode Search backed by a Rust final response.  Search's view API stays
 * authoritative: action identity is checked against the TS legal move set before values
 * are copied, and only the score frame is translated (Rust `ours` -> TS P1).
 */
function validateCompletedFinal(final: RustAdvisorFinal) {
  const value = object(final, "final");
  keys(value, [
    "kind",
    "sequence",
    "evaluationKind",
    "complete",
    "unitsDone",
    "unitsTotal",
    "elapsedMs",
    "ranked",
  ], "final");
  if (value.kind !== "final" || value.complete !== true) {
    fail("final must be a complete final update");
  }
  integer(value.sequence, "final.sequence", 0);
  evaluationKind(value.evaluationKind, "final.evaluationKind");
  const done = integer(value.unitsDone, "final.unitsDone", 0);
  if (done !== integer(value.unitsTotal, "final.unitsTotal", 0)) {
    fail("final has unfinished units");
  }
  integer(value.elapsedMs, "final.elapsedMs", 0);
  if (!Array.isArray(value.ranked)) fail("final.ranked must be an array");
  for (const [index, entry] of value.ranked.entries()) {
    const where = `final.ranked[${index}]`;
    const row = object(entry, where);
    keys(row, [
      "handIndex",
      "pillz",
      "fury",
      "score",
      "worst",
      "best",
      "samples",
      "kos",
      "koed",
    ], where);
    boundedInteger(row.handIndex, `${where}.handIndex`, 0, 3);
    boundedInteger(row.pillz, `${where}.pillz`, 0, 30);
    if (typeof row.fury !== "boolean") fail(`${where}.fury must be boolean`);
    const samples = integer(row.samples, `${where}.samples`, 1);
    const rowScore = score(row.score, `${where}.score`),
      worst = score(row.worst, `${where}.worst`),
      best = score(row.best, `${where}.best`);
    if (worst > rowScore || rowScore > best) {
      fail(`${where} score must fall between worst and best`);
    }
    const kos = integer(row.kos, `${where}.kos`, 0),
      koed = integer(row.koed, `${where}.koed`, 0);
    if (kos > samples || koed > samples) {
      fail(`${where} KO count exceeds samples`);
    }
  }
}

export class CompletedRustSearch extends Search {
  readonly #rustStats: SearchStats;
  readonly #ceilings = new Map<string, number>();

  constructor(game: Game, final: RustAdvisorFinal) {
    super(game);
    validateCompletedFinal(final);
    if (this.mode !== SearchMode.FIRST) {
      throw new RustAdvisorProtocolError(
        "completed Rust adapter only supports FIRST mode",
      );
    }
    if (!final.complete || final.unitsDone !== final.unitsTotal) {
      throw new RustAdvisorProtocolError(
        "completed Rust adapter requires a complete final response",
      );
    }
    if (
      final.evaluationKind !==
        (this.openingEstimate
          ? "opening_estimate"
          : "exact_continuation_policy")
    ) {
      throw new RustAdvisorProtocolError(
        "final evaluation_kind does not match the TS search phase",
      );
    }
    if (final.unitsTotal !== this.units) {
      throw new RustAdvisorProtocolError(
        "final unit count does not match the TS FIRST-mode matrix",
      );
    }
    const byAction = new Map(
      final.ranked.map((
        entry,
      ) => [`${entry.handIndex}:${entry.pillz}:${entry.fury}`, entry]),
    );
    checkCandidates(final.ranked, this.candidates, true);
    for (const candidate of this.candidates) {
      const entry = byAction.get(
        `${candidate.index}:${candidate.pillz}:${candidate.fury}`,
      )!;
      if (entry.samples !== this.samples) {
        throw new RustAdvisorProtocolError(
          `candidate ${candidate.key} samples do not match TS FIRST-mode replies`,
        );
      }
      const p1 = (value: number) => this.us === Turn.PLAYER_1 ? value : -value;
      candidate.values = [p1(entry.score)];
      candidate.weights = [1];
      candidate.average = p1(entry.score);
      candidate.minimax = p1(entry.worst);
      candidate.done = entry.samples;
      candidate.kos = entry.kos;
      candidate.koed = entry.koed;
      this.#ceilings.set(candidate.key, p1(entry.best));
    }
    this.#rustStats = {
      units: final.unitsTotal,
      unitsDone: final.unitsDone,
      terminal: 0,
      ms: final.elapsedMs,
    };
  }

  override get done() {
    return true;
  }

  override get stats(): SearchStats {
    return this.#rustStats;
  }

  override step(): boolean {
    return false;
  }

  override workFor(_ms: number): Promise<void> {
    return Promise.resolve();
  }

  override ceiling(candidate: Candidate): number {
    return this.#ceilings.get(candidate.key) ?? candidate.average;
  }
}
