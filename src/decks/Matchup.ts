// Deck versus deck on the exact Rust solver (phase 5 of docs/deck-builder-design.md).
//
// The primitive is the Rust batch binary `urban-recreation-matchup` (rust/src/advisor/matchup.rs):
// one JSONL request per line, one response per request, in order. A `solve` is the advisor's own
// round-one FIRST decision for a draw - the exact conservative continuation with the opponent's
// reply weighted by the captured opening prior - and its `value` is the recommended move's
// weighted average in [-1, 1], in the first mover's frame. A draw the strict Rust catalog cannot
// execute comes back `refused`, and is counted, never scored.
//
// deckVsDeck samples hand pairs (four random cards from each deck) with a seeded generator,
// solves each pair with both first movers, and scores deck A on a pair as
//   score = (value(A moves first) - value(B moves first)) / 2,
// each value in its own first mover's frame. It is antisymmetric: swapping the decks (with the
// same hands) negates it. It measures the advisor's conservative policy for whoever moves first,
// not an equilibrium; see the design doc for the caveats.
//
// Results are cached on disk under cache/matchups/, one file per engine and data provenance, so
// rerunning is instant and a data or engine change starts a fresh file.
import { TextLineStream } from "@std/streams/text-line-stream";
import { readRustV1Provenance } from "../solver/RustProvenance.ts";

/** `[card id, level]`, the wire spelling of a card. */
export type CardKey = readonly [number, number];

export interface DeckCardRef {
  readonly id: number;
  readonly level: number;
}

export type FirstMover = "p1" | "p2";

export interface SolveRequest {
  readonly kind: "solve";
  readonly p1: readonly CardKey[];
  readonly p2: readonly CardKey[];
  readonly first: FirstMover;
  readonly night: boolean;
  readonly life?: number;
  readonly pillz?: number;
  readonly budget_ms?: number;
}

export interface ProbeRequest {
  readonly kind: "probe";
  readonly card: CardKey;
  readonly night: boolean;
}

export interface ProvenanceRequest {
  readonly kind: "provenance";
}

export type MatchupRequest = SolveRequest | ProbeRequest | ProvenanceRequest;

export interface AdvisorMoveWire {
  readonly hand_index: number;
  readonly pillz: number;
  readonly fury: boolean;
}

export interface SolvedResponse {
  readonly id: number;
  readonly kind: "solve";
  readonly value: number;
  readonly worst: number;
  readonly best: number;
  readonly best_move: AdvisorMoveWire;
  readonly ko_share: number;
  readonly koed_share: number;
  readonly root_moves: number;
  readonly replies: number;
  readonly ms: number;
}

export interface RefusedResponse {
  readonly id: number;
  readonly kind: "solve";
  readonly refused: string;
}

export interface ErrorResponse {
  readonly id: number | null;
  readonly kind?: string;
  readonly error: string;
}

export type ProbeStatus = "exact" | "bonus_refused" | "bonus_untested" | "refused" | "leader" | "missing";

export interface ProbeCheck {
  /** `null` when the check could not be run. */
  readonly ok: boolean | null;
  readonly reason?: string;
  readonly partner?: CardKey;
}

export interface ProbeResponse {
  readonly id: number;
  readonly kind: "probe";
  readonly card: CardKey;
  readonly night: boolean;
  readonly status: ProbeStatus;
  readonly ability: ProbeCheck;
  readonly bonus: ProbeCheck;
  readonly ability_text: string | null;
  readonly bonus_text: string | null;
}

/** What every result depends on: data fingerprints, semantic revisions, the wire version. */
export interface MatchupProvenance {
  readonly protocol: number;
  readonly effective_catalog_fingerprint_fnv1a64: string;
  readonly effect_registry_fingerprint_fnv1a64: string;
  readonly effect_registry_schema_version: number;
  readonly compiler_policy_semantic_revision: number;
  readonly catalog_context_policy_semantic_revision: number;
  readonly advisor_policy_semantic_revision: number;
  readonly battle_rule_id: number;
  readonly fillers: readonly CardKey[];
}

export type ProvenanceResponse = MatchupProvenance & { readonly id: number; readonly kind: "provenance" };

export type MatchupResponse = SolvedResponse | RefusedResponse | ErrorResponse | ProbeResponse | ProvenanceResponse;

/** Anything that answers a batch of requests in order; tests inject a fake. */
export interface MatchupRunner {
  run(
    requests: readonly MatchupRequest[],
    onResponse?: (response: MatchupResponse, index: number) => void,
  ): Promise<MatchupResponse[]>;
}

export const MATCHUP_PROTOCOL_VERSION = 1;
export const DEFAULT_LIFE = 12;
export const DEFAULT_PILLZ = 12;
export const HAND_SIZE = 4;

/**
 * The release binary built by `deno task rust:matchup`, relative to the repository root that
 * `deno task` runs in (the tasks' `--allow-run` names exactly this path).
 */
export function matchupBinaryPath(): string {
  const base = "rust/target/release/urban-recreation-matchup";
  return Deno.build.os === "windows" ? `${base}.exe` : base;
}

export const isError = (r: MatchupResponse): r is ErrorResponse => "error" in r;
export const isRefused = (r: MatchupResponse): r is RefusedResponse => "refused" in r;
export const isSolved = (r: MatchupResponse): r is SolvedResponse => r.kind === "solve" && "value" in r;

/** Spawns the binary once per batch, streams the requests in and the responses out. */
export class ProcessMatchupRunner implements MatchupRunner {
  constructor(private readonly options: { binary?: string; threads?: number } = {}) {}

  async run(
    requests: readonly MatchupRequest[],
    onResponse?: (response: MatchupResponse, index: number) => void,
  ): Promise<MatchupResponse[]> {
    if (!requests.length) return [];
    const binary = this.options.binary ?? matchupBinaryPath();
    try {
      await Deno.stat(binary);
    } catch {
      throw new Error(`${binary} is missing: build it with \`deno task rust:matchup\``);
    }
    const child = new Deno.Command(binary, {
      args: this.options.threads ? ["--threads", String(this.options.threads)] : [],
      stdin: "piped",
      stdout: "piped",
      stderr: "piped",
    }).spawn();
    const encoder = new TextEncoder();
    const write = (async () => {
      const writer = child.stdin.getWriter();
      try {
        for (let i = 0; i < requests.length; i += 256) {
          const chunk = requests.slice(i, i + 256).map((r, j) => JSON.stringify({ id: i + j, ...r }) + "\n").join("");
          await writer.write(encoder.encode(chunk));
        }
      } finally {
        await writer.close().catch(() => {});
      }
    })();
    const stderr = new Response(child.stderr).text();
    const responses: MatchupResponse[] = [];
    let failure: Error | undefined;
    const lines = child.stdout.pipeThrough(new TextDecoderStream()).pipeThrough(new TextLineStream());
    for await (const line of lines) {
      if (failure || !line.trim()) continue;
      let response: MatchupResponse;
      try {
        response = JSON.parse(line);
      } catch {
        failure = new Error(`matchup binary wrote a line that is not JSON: ${line.slice(0, 200)}`);
        continue;
      }
      const index = responses.length;
      if (response.id !== index) {
        failure = new Error(`matchup response ${index} carries id ${response.id}`);
        continue;
      }
      responses.push(response);
      onResponse?.(response, index);
    }
    const [status, errors] = await Promise.all([child.status, stderr]);
    // A binary that exits early closes its stdin; the exit status below says why.
    await write.catch(() => {});
    if (failure) throw failure;
    if (!status.success || responses.length !== requests.length) {
      throw new Error(
        `matchup binary exited ${status.code} after ${responses.length} of ${requests.length} responses` +
          (errors.trim() ? `: ${errors.trim()}` : ""),
      );
    }
    return responses;
  }
}

/** Reads the binary's provenance, refusing a binary that speaks another protocol version. */
export async function matchupProvenance(runner: MatchupRunner): Promise<MatchupProvenance> {
  const [response] = await runner.run([{ kind: "provenance" }]);
  if (!response || isError(response) || response.kind !== "provenance") {
    throw new Error(`matchup binary did not report its provenance: ${JSON.stringify(response)}`);
  }
  const { id: _id, kind: _kind, ...provenance } = response as ProvenanceResponse;
  if (provenance.protocol !== MATCHUP_PROTOCOL_VERSION) {
    throw new Error(`matchup binary speaks protocol ${provenance.protocol}, this host ${MATCHUP_PROTOCOL_VERSION}`);
  }
  return provenance;
}

/**
 * Refuses a binary built from other data or other engine revisions than the checkout it would
 * be describing. The fingerprints are the advisor worker's (src/solver/RustProvenance.ts).
 */
export async function assertCurrentBinary(provenance: MatchupProvenance): Promise<void> {
  const source = await readRustV1Provenance();
  const pairs: [string, unknown, unknown][] = [
    ["effective catalog", provenance.effective_catalog_fingerprint_fnv1a64, source.effectiveCatalogFingerprintFnv1a64],
    ["effect registry", provenance.effect_registry_fingerprint_fnv1a64, source.effectRegistryFingerprintFnv1a64],
    ["registry schema", provenance.effect_registry_schema_version, source.effectRegistrySchemaVersion],
    ["compiler revision", provenance.compiler_policy_semantic_revision, source.compilerPolicySemanticRevision],
    [
      "catalog-context revision",
      provenance.catalog_context_policy_semantic_revision,
      source.catalogContextPolicySemanticRevision,
    ],
    ["advisor policy revision", provenance.advisor_policy_semantic_revision, source.advisorPolicySemanticRevision],
  ];
  const stale = pairs.filter(([, binary, checkout]) => binary !== checkout);
  if (stale.length) {
    throw new Error(
      `the matchup binary does not match this checkout (${
        stale.map(([name, binary, checkout]) => `${name} ${binary} vs ${checkout}`).join(", ")
      }); rebuild it with \`deno task rust:matchup\``,
    );
  }
}

// ---------------------------------------------------------------------------------------------
// Sampling

/** mulberry32: a small, fast, seedable 32-bit generator. Returns floats in [0, 1). */
export function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** murmur3's finaliser over a few words: independent seeds for independent streams. */
function mix(...words: number[]): number {
  let h = 0x9e3779b9;
  for (const word of words) {
    h = Math.imul(h ^ (word >>> 0), 0x85ebca6b);
    h ^= h >>> 13;
    h = Math.imul(h, 0xc2b2ae35);
    h ^= h >>> 16;
  }
  return h >>> 0;
}

/** Four distinct positions of a deck of `size`, in draw order (a partial Fisher-Yates). */
export function sampleHandPositions(size: number, random: () => number): number[] {
  const positions = Array.from({ length: size }, (_, i) => i);
  for (let k = 0; k < HAND_SIZE; k++) {
    const j = k + Math.floor(random() * (size - k));
    [positions[k], positions[j]] = [positions[j], positions[k]];
  }
  return positions.slice(0, HAND_SIZE);
}

export interface HandPair {
  readonly a: readonly DeckCardRef[];
  readonly b: readonly DeckCardRef[];
}

/**
 * `n` hand pairs for a seed. Each side draws from its own stream, keyed by the seed, the side
 * and the sample index only, so the B hands are the same whatever deck A is (common random
 * numbers), and two A decks of the same size that differ in one position share every hand
 * that does not draw that position.
 */
export function sampleHandPairs(
  deckA: readonly DeckCardRef[],
  deckB: readonly DeckCardRef[],
  n: number,
  seed: number,
): HandPair[] {
  for (const [name, deck] of [["A", deckA], ["B", deckB]] as const) {
    if (deck.length < HAND_SIZE) throw new Error(`deck ${name} has ${deck.length} cards; a hand needs ${HAND_SIZE}`);
  }
  return Array.from({ length: n }, (_, i) => ({
    a: sampleHandPositions(deckA.length, mulberry32(mix(seed, 0xa, i))).map((p) => deckA[p]),
    b: sampleHandPositions(deckB.length, mulberry32(mix(seed, 0xb, i))).map((p) => deckB[p]),
  }));
}

/** A hand in canonical order, `(id, level)` ascending: the order it is solved and cached in. */
export function canonicalHand(hand: readonly DeckCardRef[]): CardKey[] {
  return hand.map((c) => [c.id, c.level] as const).sort((x, y) => x[0] - y[0] || x[1] - y[1]);
}

const handText = (hand: readonly CardKey[]) => hand.map(([id, level]) => `${id}:${level}`).join(",");

export interface SolveContext {
  readonly night: boolean;
  readonly life: number;
  readonly pillz: number;
}

/** The cache key of a solve with `first` moving first: night, resources, both sorted hands. */
export function solveKey(first: readonly CardKey[], second: readonly CardKey[], context: SolveContext): string {
  return `${context.night ? "n" : "d"}|${context.life}|${context.pillz}|${handText(first)}|${handText(second)}`;
}

// ---------------------------------------------------------------------------------------------
// Cache

export type CachedSolve =
  | {
    readonly value: number;
    readonly worst: number;
    readonly best_move: AdvisorMoveWire;
    readonly ko_share: number;
    readonly koed_share: number;
    readonly ms: number;
  }
  | { readonly refused: string };

export interface MatchupCache {
  get(key: string): CachedSolve | undefined;
  set(key: string, value: CachedSolve): void;
  /** Persists everything `set` since the last flush. */
  flush(): Promise<void>;
}

export class MemoryMatchupCache implements MatchupCache {
  readonly entries = new Map<string, CachedSolve>();
  get(key: string) {
    return this.entries.get(key);
  }
  set(key: string, value: CachedSolve) {
    this.entries.set(key, value);
  }
  async flush() {}
}

async function provenanceHash(provenance: MatchupProvenance): Promise<string> {
  const canonical = JSON.stringify(provenance, Object.keys(provenance).sort());
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(canonical));
  return [...new Uint8Array(digest)].slice(0, 8).map((b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * An append-only JSONL file per provenance: `<dir>/v<protocol>-<hash>.jsonl`, one `{k, r}` line
 * per solve, beside a `.provenance.json` saying what the hash stands for.
 */
export class FileMatchupCache implements MatchupCache {
  private readonly entries = new Map<string, CachedSolve>();
  private pending: string[] = [];

  private constructor(readonly path: string) {}

  static async open(dir: string, provenance: MatchupProvenance): Promise<FileMatchupCache> {
    const base = `${dir.replace(/[\\/]+$/, "")}/v${provenance.protocol}-${await provenanceHash(provenance)}`;
    const cache = new FileMatchupCache(`${base}.jsonl`);
    await Deno.mkdir(dir, { recursive: true });
    try {
      await Deno.stat(`${base}.provenance.json`);
    } catch {
      await Deno.writeTextFile(`${base}.provenance.json`, JSON.stringify(provenance, null, 2) + "\n");
    }
    let text = "";
    try {
      text = await Deno.readTextFile(cache.path);
    } catch (error) {
      if (!(error instanceof Deno.errors.NotFound)) throw error;
    }
    for (const line of text.split("\n")) {
      if (!line.trim()) continue;
      try {
        const { k, r } = JSON.parse(line);
        if (typeof k === "string" && r && typeof r === "object") cache.entries.set(k, r);
      } catch {
        // A line cut short by an interrupted run; the solve will simply be redone.
      }
    }
    return cache;
  }

  get size(): number {
    return this.entries.size;
  }

  get(key: string) {
    return this.entries.get(key);
  }

  set(key: string, value: CachedSolve) {
    if (this.entries.has(key)) return;
    this.entries.set(key, value);
    this.pending.push(JSON.stringify({ k: key, r: value }));
  }

  async flush() {
    if (!this.pending.length) return;
    const lines = this.pending.join("\n") + "\n";
    this.pending = [];
    await Deno.writeTextFile(this.path, lines, { append: true });
  }
}

// ---------------------------------------------------------------------------------------------
// Deck versus deck

export interface DeckVsDeckOptions {
  readonly runner: MatchupRunner;
  readonly cache?: MatchupCache;
  /** Hand pairs to sample (default 100). */
  readonly n?: number;
  readonly seed?: number;
  readonly night?: boolean;
  readonly life?: number;
  readonly pillz?: number;
  /** How many of A's worst pairs to return (default 5). */
  readonly worst?: number;
  /** Called once per fresh solve as its response arrives. */
  readonly onProgress?: (done: number, total: number) => void;
}

export interface PairResult {
  readonly a: readonly CardKey[];
  readonly b: readonly CardKey[];
  /** A's value when A moves first, in A's frame. */
  readonly aFirst: number;
  /** B's value when B moves first, in B's frame. */
  readonly bFirst: number;
  /** (aFirst - bFirst) / 2, A's score on this pair. */
  readonly score: number;
}

export interface RefusedPair {
  readonly a: readonly CardKey[];
  readonly b: readonly CardKey[];
  readonly reason: string;
}

export interface DeckVsDeckResult {
  readonly n: number;
  readonly seed: number;
  readonly night: boolean;
  /** Pairs with both first movers solved. */
  readonly scored: number;
  /** Pairs the strict catalog refused, excluded from the mean. */
  readonly refused: number;
  /** Mean score for A in [-1, 1]; NaN when nothing was scored. */
  readonly mean: number;
  /** Standard error of the mean over scored pairs; NaN below two. */
  readonly stderr: number;
  /** The mean on the advisor's percent scale, (mean + 1) / 2 * 100. */
  readonly percent: number;
  readonly pairs: readonly PairResult[];
  readonly worst: readonly PairResult[];
  readonly refusedPairs: readonly RefusedPair[];
  /** Solves answered by the cache and solves run now. */
  readonly cached: number;
  readonly solved: number;
}

/** Everything a set of hand pairs needs solved, as cache keys and requests, deduplicated. */
function solvesFor(pairs: readonly { a: readonly CardKey[]; b: readonly CardKey[] }[], context: SolveContext) {
  const keys = new Map<string, SolveRequest>();
  const perPair = pairs.map(({ a, b }) => {
    const aFirst = solveKey(a, b, context);
    const bFirst = solveKey(b, a, context);
    for (const [key, first, second] of [[aFirst, a, b], [bFirst, b, a]] as const) {
      if (!keys.has(key)) {
        keys.set(key, {
          kind: "solve",
          p1: first,
          p2: second,
          first: "p1",
          night: context.night,
          ...(context.life !== DEFAULT_LIFE ? { life: context.life } : {}),
          ...(context.pillz !== DEFAULT_PILLZ ? { pillz: context.pillz } : {}),
        });
      }
    }
    return { aFirst, bFirst };
  });
  return { keys, perPair };
}

/** Solves every key the cache does not hold, through one runner batch. */
async function resolve(
  keys: ReadonlyMap<string, SolveRequest>,
  cache: MatchupCache,
  runner: MatchupRunner,
  onProgress?: (done: number, total: number) => void,
): Promise<{ cached: number; solved: number }> {
  const missing = [...keys].filter(([key]) => cache.get(key) === undefined);
  if (missing.length) {
    let done = 0;
    const responses = await runner.run(missing.map(([, request]) => request), () => onProgress?.(++done, missing.length));
    if (responses.length !== missing.length) {
      throw new Error(`matchup runner answered ${responses.length} of ${missing.length} solves`);
    }
    try {
      responses.forEach((response, i) => {
        const [key] = missing[i];
        if (isError(response)) throw new Error(`solve ${key} failed: ${response.error}`);
        if (isRefused(response)) cache.set(key, { refused: response.refused });
        else if (isSolved(response)) {
          const { value, worst, best_move, ko_share, koed_share, ms } = response;
          cache.set(key, { value, worst, best_move, ko_share, koed_share, ms });
        } else throw new Error(`solve ${key} got an unexpected response ${JSON.stringify(response)}`);
      });
    } finally {
      await cache.flush();
    }
  }
  return { cached: keys.size - missing.length, solved: missing.length };
}

function mean(values: readonly number[]): number {
  return values.length ? values.reduce((s, v) => s + v, 0) / values.length : NaN;
}

function standardError(values: readonly number[]): number {
  if (values.length < 2) return NaN;
  const m = mean(values);
  const variance = values.reduce((s, v) => s + (v - m) ** 2, 0) / (values.length - 1);
  return Math.sqrt(variance / values.length);
}

/** Scores deck A against deck B on sampled hand pairs; see the file comment for the definition. */
export async function deckVsDeck(
  deckA: readonly DeckCardRef[],
  deckB: readonly DeckCardRef[],
  options: DeckVsDeckOptions,
): Promise<DeckVsDeckResult> {
  const n = options.n ?? 100;
  const seed = options.seed ?? 1;
  const context: SolveContext = {
    night: options.night ?? false,
    life: options.life ?? DEFAULT_LIFE,
    pillz: options.pillz ?? DEFAULT_PILLZ,
  };
  const cache = options.cache ?? new MemoryMatchupCache();
  const pairs = sampleHandPairs(deckA, deckB, n, seed).map(({ a, b }) => ({ a: canonicalHand(a), b: canonicalHand(b) }));
  const { keys, perPair } = solvesFor(pairs, context);
  const { cached, solved } = await resolve(keys, cache, options.runner, options.onProgress);

  const results: PairResult[] = [];
  const refusedPairs: RefusedPair[] = [];
  pairs.forEach(({ a, b }, i) => {
    const aFirst = cache.get(perPair[i].aFirst)!;
    const bFirst = cache.get(perPair[i].bFirst)!;
    if ("refused" in aFirst || "refused" in bFirst) {
      refusedPairs.push({ a, b, reason: "refused" in aFirst ? aFirst.refused : (bFirst as { refused: string }).refused });
      return;
    }
    results.push({ a, b, aFirst: aFirst.value, bFirst: bFirst.value, score: (aFirst.value - bFirst.value) / 2 });
  });
  const scores = results.map((r) => r.score);
  const m = mean(scores);
  return {
    n,
    seed,
    night: context.night,
    scored: results.length,
    refused: refusedPairs.length,
    mean: m,
    stderr: standardError(scores),
    percent: ((m + 1) / 2) * 100,
    pairs: results,
    worst: [...results].sort((x, y) => x.score - y.score).slice(0, options.worst ?? 5),
    refusedPairs,
    cached,
    solved,
  };
}
