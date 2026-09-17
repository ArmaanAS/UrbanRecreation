// deno-lint-ignore-file no-control-regex
// Live solver advisor.
//
//   deno task advise                 # start capture automatically and analyse your turns
//   deno task advise --replay 901613 # review a captured battle, decision by decision
//   deno task advise --replay 901613 --budget 5
//   deno task advise --workers 1     # leave solving on the main thread
//   deno task advise --rust=compare  # keep TS authoritative and compare supported decisions
//   deno task advise --rust=use      # show protocol-validated Rust results when supported
//   deno task advise --preview-safe  # inspect the zero-risk shortlist without a live game
//   deno task advise --preview-results # compare the available game-over banner styles
//
// The advisor starts log_server.ts as a quiet child process when its local feed is absent;
// `deno task log` remains available as a capture-only/debug command. The server broadcasts
// every battle snapshot it parses on the local event feed. This process listens,
// rebuilds the game in progress with
// scripts/ExtractBattle.ts's own reconstruction (rather than a second implementation of the
// same snapshot handling), and runs a root-split Search whose ranking is redrawn as it
// firms up. Replay mode does the same against a battle file, which needs no live game and
// is how the whole path gets exercised.
//
// The solver deliberately remains a separate process from log_server.ts: capture has to
// keep up with the game client's polling and serialises its writes, so a multi-second CPU
// burn belongs somewhere it cannot drop a snapshot.
import {
  type CaptureEntry,
  expandEntries,
  loadAbilities,
} from "../../scripts/BattleCapture.ts";
import { reconstruct } from "../../scripts/ExtractBattle.ts";
import Game from "../game/Game.ts";
import Player from "../game/Player.ts";
import { HandGenerator } from "../game/Hand.ts";
import { type HandOf } from "../game/types/CardTypes.ts";
import { Turn } from "../game/types/Types.ts";
import Search from "./Search.ts";
import ParallelSearch from "./ParallelSearch.ts";
import {
  CompletedRustSearch,
  DenoCommandRunner,
  runRustAdvisor,
  RustAdvisorCancelledError,
  type RustAdvisorRunner,
  RustAdvisorTimeoutError,
} from "./RustAdvisor.ts";
import {
  normaliseRustAdvisorInput,
  type RustAdvisorInputResult,
} from "./RustAdvisorInput.ts";
import { SearchMode } from "./Search.ts";
import {
  ALT_SCREEN_OFF,
  ALT_SCREEN_ON,
  autoQueueClick,
  CLEAR_TO_END,
  type ConnectionState,
  consoleSize,
  HOME,
  idle,
  MOUSE_OFF,
  MOUSE_ON,
  opponentReadClick,
  type OpponentReadState,
  type PlayedMove,
  render,
  RESULT_STYLES,
  type ResultStyle,
  resultStylePreview,
  type ViewBoard,
  type ViewOutcome,
  type ViewPhase,
} from "./SolverView.ts";

// Deno.serve is bound to IPv4. On this Windows host `localhost` resolves to ::1 first, so
// use the explicit loopback address for both readiness checks and the live event stream.
const FEED = "http://127.0.0.1:8787/events";
const BATTLE_DIR = "captures/battles";
/** Redraw at most this often; the client polls far faster than a person can read. */
const FRAME_MS = 250;
/** Keep a brief card hover visible even when enter/leave frames arrive almost together. */
const HOVER_HOLD_MS = 350;
/** Keep the chooser outline through the short gap before a committed status arrives. */
const CHOOSER_CLOSE_HOLD_MS = 500;
/** Work for this long between redraws. One unit is ~30ms at round 2. */
const SLICE_MS = 120;

// The engine keeps a process-global CachedCardBattle cache keyed by card-index pair
// (Game.createBattleDataCache), so only one Game may be alive at a time in a process. Every
// position change therefore drops the running search *before* building the next Game.
type Reconstructed = ReturnType<typeof reconstruct>;

interface Position {
  key: string;
  game: Game;
  search: Search;
  battleId: number;
  /** Engine round, 1-based. */
  round: number;
  /** Current/last battle detail for the full hand panel. */
  board: ViewBoard;
  /** Set when the replayed position disagrees with the server's own life / pillz. */
  warning?: string;
  /** One disposable Rust process for this exact capture decision, if requested. */
  rust?: RustDecisionJob;
}

const stdout = Deno.stdout;
const enc = new TextEncoder();
export function writeAllSync(
  writer: { writeSync(data: Uint8Array): number },
  data: Uint8Array,
) {
  let offset = 0;
  while (offset < data.length) {
    const written = writer.writeSync(data.subarray(offset));
    if (written <= 0) throw new Error("terminal stopped accepting output");
    offset += written;
  }
}

// A Windows console write may accept only the first part of a large ANSI frame. Ignoring
// writeSync's return value cut the full-card board off halfway through the first hand and
// left the previous frame's blank area and Auto Queue button below it.
const write = (s: string) => writeAllSync(stdout, enc.encode(s));
let stopControlInput = () => {};
let exitAdvisor: () => void | Promise<void> = () => Deno.exit(0);
// Assigned by liveMode so Ctrl+C kills a disposable Rust child before Deno leaves.
let stopRustDecision = async () => {};

/** The engine logs heavily in the battle path; none of it may reach the screen. */
function silenceEngine() {
  const sink = () => {};
  console.log = sink;
  console.info = sink;
  console.debug = sink;
  console.warn = sink;
}

function controlUrl(feed: string) {
  const url = new URL(feed);
  url.pathname = "/control";
  url.search = "";
  url.hash = "";
  return url.href;
}

async function readAutoQueue(url: string): Promise<boolean> {
  const res = await fetch(url, { headers: { accept: "application/json" } });
  if (!res.ok) throw new Error(`control HTTP ${res.status}`);
  return !!(await res.json()).autoQueue;
}

async function writeAutoQueue(
  url: string,
  autoQueue: boolean,
): Promise<boolean> {
  const res = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ autoQueue }),
  });
  if (!res.ok) throw new Error(`control HTTP ${res.status}`);
  return !!(await res.json()).autoQueue;
}

const MANAGED_LOG_ARGS = [
  "run",
  "--allow-net=0.0.0.0:8787",
  "--allow-read=captures",
  "--allow-write=ur_log.jsonl,captures,data",
  "--allow-env=FORCE_COLOR",
  "--allow-sys=osRelease",
  "./log_server.ts",
];

async function captureServerReady(feed: string) {
  try {
    const res = await fetch(controlUrl(feed), {
      signal: AbortSignal.timeout(300),
    });
    const ready = res.ok;
    await res.body?.cancel();
    return ready;
  } catch {
    return false;
  }
}

/** Start the default local capture feed, unless a separately launched logger owns it. */
async function ensureCaptureServer(
  feed: string,
): Promise<Deno.ChildProcess | undefined> {
  if (feed !== FEED || await captureServerReady(feed)) return undefined;

  const child = new Deno.Command(Deno.execPath(), {
    args: MANAGED_LOG_ARGS,
    stdin: "null",
    stdout: "null",
    stderr: "null",
  }).spawn();
  const exited = child.status;
  for (let attempt = 0; attempt < 30; attempt++) {
    if (await captureServerReady(feed)) return child;
    const early = await Promise.race([
      exited,
      new Promise<undefined>((resolve) =>
        setTimeout(() => resolve(undefined), 100)
      ),
    ]);
    if (early !== undefined) {
      throw new Error(`capture server exited during startup (${early.code})`);
    }
  }
  try {
    child.kill();
  } catch { /* it exited between the final check and cleanup */ }
  throw new Error("capture server did not become ready");
}

function stopCaptureServer(child: Deno.ChildProcess | undefined) {
  if (child === undefined) return;
  try {
    child.kill();
  } catch { /* already stopped */ }
}

/** Read A and SGR mouse events without mistaking an arrow-key escape sequence for A. */
function startControlInput(
  toggle: () => void,
  hover: (inside: boolean) => void,
  read: (column: number, row: number, pressed: boolean) => void,
): () => void {
  if (!Deno.stdin.isTerminal()) return () => {};
  let active = true;
  let raw = false;
  let escape = "";
  try {
    // `cbreak` is not supported by Deno on Windows, while basic raw mode is. Ctrl+C is
    // handled explicitly below because raw mode stops the terminal turning it into SIGINT.
    Deno.stdin.setRaw(true);
    raw = true;
    write(MOUSE_ON);
  } catch {
    // Some terminal hosts expose stdin as a TTY but reject raw mode. Reading still works
    // there after Enter; the advisor must remain usable even though clicks are unavailable.
  }

  (async () => {
    const bytes = new Uint8Array(64);
    const decoder = new TextDecoder();
    while (active) {
      const n = await Deno.stdin.read(bytes);
      if (n === null) break;
      for (const ch of decoder.decode(bytes.subarray(0, n))) {
        if (escape) {
          escape += ch;
          // CSI sequences finish with a byte in @..~. SGR mouse presses finish with M.
          if (
            escape.startsWith("\x1b[") && escape.length > 2 && /[@-~]/.test(ch)
          ) {
            const mouse = /^\x1b\[<(\d+);(\d+);(\d+)([Mm])$/.exec(escape);
            if (mouse) {
              const size = consoleSize();
              const inside = autoQueueClick(
                Number(mouse[2]),
                Number(mouse[3]),
                size,
              );
              hover(inside);
              read(
                Number(mouse[2]),
                Number(mouse[3]),
                mouse[1] === "0" && mouse[4] === "M",
              );
              if (mouse[1] === "0" && mouse[4] === "M" && inside) toggle();
            }
            escape = "";
          } else if (!escape.startsWith("\x1b[") && escape.length > 1) {
            escape = "";
          } else if (escape.length > 64) {
            escape = "";
          }
          continue;
        }
        if (ch === "\x1b") escape = ch;
        else if (ch === "a" || ch === "A") toggle();
        else if (ch === "\x03") {
          exitAdvisor();
          return;
        }
      }
    }
  })().catch(() => {});

  return () => {
    if (!active) return;
    active = false;
    if (raw) {
      write(MOUSE_OFF);
      try {
        Deno.stdin.setRaw(false);
      } catch { /* terminal has already closed */ }
    }
  };
}

export interface AdvisorOptions {
  feed: string;
  replay?: number;
  /** Render a synthetic state for visual feedback without the capture server. */
  preview?: "safe" | "results";
  /** Decoration used for the large win/lose/draw lettering. */
  resultStyle: ResultStyle;
  /** Seconds to spend per decision before settling for the partial ranking. */
  budget: number;
  /** Visible recommendations. When omitted, the opening gets 10 and later rounds get 8. */
  top?: number;
  /** CPU workers used by each solve. One keeps the old in-process path. */
  workers: number;
  /** Experimental Rust worker policy. TypeScript remains the default and fallback. */
  rust: "off" | "compare" | "use";
}

export function parseArgs(argv: string[]): AdvisorOptions {
  const opts: AdvisorOptions = {
    feed: FEED,
    budget: 0,
    workers: 3,
    rust: "off",
    resultStyle: "classic",
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--replay") opts.replay = Number(argv[++i]);
    else if (a === "--budget") opts.budget = Number(argv[++i]);
    else if (a === "--feed") opts.feed = argv[++i];
    else if (a === "--top") opts.top = Number(argv[++i]);
    else if (a === "--workers") opts.workers = Number(argv[++i]);
    else if (a === "--rust" || a.startsWith("--rust=")) {
      const value = (a === "--rust" ? argv[++i] : a.slice("--rust=".length)) as
        | AdvisorOptions["rust"]
        | undefined;
      if (value !== "off" && value !== "compare" && value !== "use") {
        throw new Error("--rust must be one of: off, compare, use");
      }
      opts.rust = value;
    } else if (a === "--preview-safe") opts.preview = "safe";
    else if (a === "--preview-results") opts.preview = "results";
    else if (a === "--result-style") {
      const style = argv[++i] as ResultStyle;
      if (!RESULT_STYLES.includes(style)) {
        throw new Error(
          `--result-style must be one of: ${RESULT_STYLES.join(", ")}`,
        );
      }
      opts.resultStyle = style;
    } else throw new Error(`unknown argument: ${a}`);
  }
  if (
    !Number.isInteger(opts.workers) || opts.workers < 1 || opts.workers > 32
  ) {
    throw new Error("--workers must be an integer from 1 to 32");
  }
  if (opts.replay !== undefined && !opts.budget) opts.budget = 20;
  if (opts.replay !== undefined && opts.preview !== undefined) {
    throw new Error("--replay and preview modes cannot be used together");
  }
  return opts;
}

const createSearch = (
  game: Game,
  workers: number,
  blindSecond = false,
): Search =>
  // The shallow opening pass is only a few thousand battle resolutions; worker startup
  // costs more than it saves and needlessly occupies every configured core.
  workers === 1 || game.round === 1
    ? new Search(game, 1, 0, blindSecond)
    : new ParallelSearch(game, workers, 100, blindSecond);

const cancelSearch = (search: Search) => {
  if (search instanceof ParallelSearch) search.cancel();
};

const RUST_WORKER_BASENAME =
  "rust/target/release/urban-recreation-advisor-jsonl";
const RUST_TIMEOUT_GRACE_MS = 750;
const RUST_SCORE_TOLERANCE = 0.005;

/** Resolve only the prebuilt worker.  The advisor must never turn a live decision into cargo. */
export async function rustWorkerCommand(): Promise<string> {
  const exe = `${RUST_WORKER_BASENAME}.exe`;
  const portable = RUST_WORKER_BASENAME;
  // Windows builds conventionally have .exe; retaining the extensionless fallback also
  // makes a checked-out prebuilt worker usable under Deno's non-Windows hosts.
  for (
    const command of Deno.build.os === "windows"
      ? [exe, portable]
      : [portable, exe]
  ) {
    try {
      if ((await Deno.stat(command)).isFile) return command;
    } catch (error) {
      if (!(error instanceof Deno.errors.NotFound)) throw error;
    }
  }
  // Let Deno.Command report the ordinary, concise missing-worker error if neither exists.
  return Deno.build.os === "windows" ? exe : portable;
}

const rustBudgetMs = (opts: AdvisorOptions) =>
  Math.min(30_000, opts.budget > 0 ? opts.budget * 1000 : 1_000);

const boundedRustStatus = (status: string) =>
  status.replace(/[\x00-\x1f\x7f-\x9f]+/g, " ").slice(0, 110);

/**
 * Compare the complete, independently validated Rust answer with the complete TS answer.
 * The tolerance is deliberately below one displayed percentage point, while still allowing
 * harmless floating point fold-order differences.
 */
export function compareRustSearches(ts: Search, rust: Search): string {
  const close = (a: number, b: number) =>
    Math.abs(a - b) <= RUST_SCORE_TOLERANCE;
  const byKey = new Map(
    rust.candidates.map((candidate) => [candidate.key, candidate]),
  );
  if (byKey.size !== ts.candidates.length) return "rust differs";
  for (const candidate of ts.candidates) {
    const other = byKey.get(candidate.key);
    if (
      other === undefined || !close(candidate.average, other.average) ||
      !close(candidate.minimax, other.minimax) ||
      !close(ts.ceiling(candidate), rust.ceiling(other)) ||
      ts.shownPercent(candidate.average) !== rust.shownPercent(other.average) ||
      ts.shownPercent(candidate.minimax) !== rust.shownPercent(other.minimax) ||
      ts.shownPercent(ts.ceiling(candidate)) !==
        rust.shownPercent(rust.ceiling(other)) ||
      !close(ts.koShare(candidate), rust.koShare(other)) ||
      !close(ts.koedShare(candidate), rust.koedShare(other))
    ) return "rust differs";
  }
  return ts.best()?.key === rust.best()?.key ? "rust match" : "rust differs";
}

export interface RustDecisionJobOptions {
  readonly mode: "compare" | "use";
  readonly key: string;
  readonly requestId: string;
  readonly game: Game;
  readonly getSearch: () => Search;
  readonly replaceSearch: (search: Search) => void;
  readonly isCurrent: () => boolean;
  readonly normalise: () => Promise<RustAdvisorInputResult>;
  readonly runner: RustAdvisorRunner | (() => Promise<RustAdvisorRunner>);
  readonly budgetMs: number;
  readonly changed?: () => void;
}

/**
 * Owns one worker for one Position key.  This small boundary keeps process lifetime and
 * late-result checks testable without needing a capture feed or terminal.
 */
export class RustDecisionJob {
  #controller = new AbortController();
  #status = "rust starting";
  #transcript?: Awaited<ReturnType<typeof runRustAdvisor>>;
  #settled = false;
  #cancelled = false;
  #runPromise: Promise<void>;

  constructor(private readonly options: RustDecisionJobOptions) {
    this.#runPromise = this.run();
  }

  get status() {
    return this.#status;
  }

  get waiting() {
    return !this.#settled && !this.#cancelled;
  }

  cancel() {
    if (this.#cancelled) return;
    this.#cancelled = true;
    this.#controller.abort(new RustAdvisorCancelledError());
  }

  async cancelAndWait() {
    this.cancel();
    await this.#runPromise;
  }

  settleCompare(search = this.options.getSearch()) {
    if (
      this.options.mode !== "compare" || this.#transcript === undefined ||
      !search.done || !this.current()
    ) return;
    try {
      this.#status = compareRustSearches(
        search,
        new CompletedRustSearch(this.options.game, this.#transcript.final),
      );
    } catch (error) {
      this.#status = boundedRustStatus(
        `rust rejected; TS fallback: ${(error as Error).message}`,
      );
    }
    this.#settled = true;
    this.changed();
  }

  private current() {
    return !this.#cancelled && !this.#controller.signal.aborted &&
      this.options.isCurrent();
  }

  private changed() {
    this.options.changed?.();
  }

  private setStatus(status: string) {
    if (!this.current()) return;
    this.#status = boundedRustStatus(status);
    this.changed();
  }

  private async run() {
    try {
      const normalised = await this.options.normalise();
      if (!this.current()) return;
      if (!normalised.supported) {
        this.setStatus(`rust rejected: ${normalised.reason}`);
        this.#settled = true;
        return;
      }
      this.setStatus("rust running");
      const runner = typeof this.options.runner === "function"
        ? await this.options.runner()
        : this.options.runner;
      if (!this.current()) return;
      const source = this.options.getSearch();
      const transcript = await runRustAdvisor(
        runner,
        normalised.request,
        source.candidates,
        {
          signal: this.#controller.signal,
          timeoutMs: this.options.budgetMs + RUST_TIMEOUT_GRACE_MS,
        },
      );
      if (!this.current()) return;
      if (this.options.mode === "use") {
        // Construction repeats the full action-set/sample validation before the swap.
        const replacement = new CompletedRustSearch(
          this.options.game,
          transcript.final,
        );
        if (!this.current()) return;
        const previous = this.options.getSearch();
        this.options.replaceSearch(replacement);
        cancelSearch(previous);
        this.#settled = true;
        this.setStatus("rust active");
        return;
      }
      this.#transcript = transcript;
      this.setStatus(source.done ? "rust ready" : "rust ready; TS finishing");
      this.settleCompare();
    } catch (error) {
      if (!this.current()) return;
      const status = error instanceof RustAdvisorTimeoutError
        ? "rust timeout; TS fallback"
        : `rust rejected: ${(error as Error).message}`;
      this.setStatus(status);
      this.#settled = true;
    }
  }
}

function cancelPosition(pos: Position) {
  pos.rust?.cancel();
  cancelSearch(pos.search);
}

export function rustDecisionEnabled(
  rust: AdvisorOptions["rust"],
  _mode: SearchMode,
): rust is Exclude<AdvisorOptions["rust"], "off"> {
  return rust !== "off";
}

function startRustForPosition(
  pos: Position,
  rec: Reconstructed,
  opts: AdvisorOptions,
  changed?: () => void,
  runner: RustAdvisorRunner | (() => Promise<RustAdvisorRunner>) = async () =>
    new DenoCommandRunner({ command: await rustWorkerCommand() }),
) {
  // Off must remain observationally identical. Every enabled mode still passes through
  // the strict capture normaliser and worker protocol before it can replace TypeScript.
  if (!rustDecisionEnabled(opts.rust, pos.search.mode)) return;
  const mode: "compare" | "use" = opts.rust;
  const job: RustDecisionJob = new RustDecisionJob({
    mode,
    key: pos.key,
    requestId: `rust:${pos.key}`,
    game: pos.game,
    getSearch: () => pos.search,
    replaceSearch: (search) => pos.search = search,
    isCurrent: (): boolean => pos.rust === job,
    normalise: () =>
      normaliseRustAdvisorInput({
        rec,
        game: pos.game,
        decision: { mode: pos.search.mode, us: pos.search.us },
        requestId: `rust:${pos.key}`,
        budgetMs: rustBudgetMs(opts),
      }),
    runner,
    budgetMs: rustBudgetMs(opts),
    changed,
  });
  pos.rust = job;
}

// ---------------------------------------------------------------------------------------
// Rebuilding the position
// ---------------------------------------------------------------------------------------

/**
 * Cheap identity for the decision on offer: the round being played plus who has committed
 * what in it. Any change of decision point changes this, so an unchanged key means the
 * position is unchanged and no Game need be built - which matters because the client polls
 * several times a second and building one replays the whole game and rebuilds the engine's
 * process-global battle cache.
 */
export function positionKey(
  rec: Reconstructed,
  battleId: number,
): string | undefined {
  const tc = rec.testcase;
  if (tc === null || rec.firstPlayer === null || rec.mySide === null) {
    return undefined;
  }
  const committed = (rec.rounds[tc.moves.length]?.moves ?? [])
    .map((m) => `${m.side}/${m.index}`)
    .sort()
    .join(",");
  // The result endpoint lands shortly after the first `done` status and contains the
  // authoritative outcome/final life. Give it a distinct key so it repaints the holding
  // screen instead of being mistaken for another poll of the already-seen final state.
  const ending = rec.result?.result ?? rec.finalStatus;
  return `${battleId}:${tc.moves.length}:${committed}:${ending}`;
}

/**
 * Why there is nothing to analyse. `settled` means the answer will not change for this
 * decision point, so it can be remembered; anything else must be retried as more snapshots
 * arrive. Remembering an unsettled answer was a real bug: one transient failure blacklisted
 * the decision permanently and the rest of the game went unanalysed in silence.
 */
interface HoldingState {
  round: number;
  headline: string;
  progress: string;
  you: { life: number; pillz: number };
  them: { life: number; pillz: number };
  board: ViewBoard;
  barFloor: { life: number; pillz: number };
  outcome?: ViewOutcome;
  warning?: string;
}

export type NotOurs = {
  settled: boolean;
  why: string;
  /** The battle is over, so no recommendation, matrix or move history remains current. */
  finished?: boolean;
  /** A real position to display even though there is no decision for us to solve yet. */
  holding?: HoldingState;
  warning?: string;
};
export type Ours = {
  game: Game;
  round: number;
  board: ViewBoard;
  /** Advice calculated before the opponent's first-moving card has been revealed. */
  provisional?: boolean;
  warning?: string;
};
export type Built = Ours | NotOurs;

export const isOurs = (b: Built): b is Ours => "game" in b;

function viewPhase(holding: HoldingState): ViewPhase {
  return holding;
}

/** Prefer the site's revealed totals; fall back to the replay while a round is unresolved. */
function holdingState(
  rec: Reconstructed,
  game: Game,
  ourPlayer: Turn,
  round: number,
  headline: string,
  progress: string,
  warning?: string,
): HoldingState {
  const mine = ourPlayer === Turn.PLAYER_1 ? game.p1 : game.p2;
  const theirs = ourPlayer === Turn.PLAYER_1 ? game.p2 : game.p1;
  const ourHand = ourPlayer === Turn.PLAYER_1 ? game.h1 : game.h2;
  const theirHand = ourPlayer === Turn.PLAYER_1 ? game.h2 : game.h1;
  const resolved = rec.testcase?.moves.length ?? 0;
  const revealed = rec.rounds[resolved - 1];
  const current = rec.rounds[resolved];
  const ourSide = rec.mySide!;
  const theirSide = (1 - ourSide) as 0 | 1;
  const barFloor = {
    life: Math.max(rec.players[0].baseLife, rec.players[1].baseLife),
    pillz: Math.max(rec.players[0].basePillz, rec.players[1].basePillz),
  };
  // Keep both cards from the just-resolved round emphasised until either player commits
  // in the new round. Once that happens only the current selection is highlighted.
  const selectedRound = current?.moves.length ? current : revealed;
  const selected = (side: 0 | 1) =>
    selectedRound?.moves.find((move) => move.side === side)?.index;
  const beforeLastRound = resolved > 0
    ? (() => {
      const prior = rec.rounds[resolved - 2];
      const before = (side: 0 | 1) =>
        prior !== undefined && Number.isFinite(prior.life[side]) &&
          Number.isFinite(prior.pillz[side])
          ? { life: prior.life[side], pillz: prior.pillz[side] }
          : {
            life: rec.players[side].baseLife,
            pillz: rec.players[side].basePillz,
          };
      return { you: before(ourSide), them: before(theirSide) };
    })()
    : undefined;
  const shownBattle = current?.moves.length ? current : revealed;
  const showingCurrentRound = (current?.moves.length ?? 0) > 0;
  const ourCommitted = current?.moves.some((move) => move.side === ourSide) ??
    false;
  const theirCommitted =
    current?.moves.some((move) => move.side === theirSide) ??
      false;
  const turn: ViewBoard["turn"] = rec.result !== null || !game.isPlaying ||
      (ourCommitted && theirCommitted)
    ? undefined
    : ourCommitted
    ? "them"
    : theirCommitted
    ? "you"
    : game.turn === ourPlayer
    ? "you"
    : "them";
  const battleSide = (side: 0 | 1) => {
    const move = shownBattle?.moves.find((candidate) =>
      candidate.side === side
    );
    if (move === undefined) return undefined;
    const result = shownBattle?.resolution[side];
    const betRevealed = (result !== null && result !== undefined) ||
      side === ourSide;
    const card = rec.players[side].hand.find((candidate) =>
      candidate.index === move.index
    );
    const availablePillz = showingCurrentRound
      ? side === ourSide ? mine.pillz : theirs.pillz
      : side === ourSide
      ? beforeLastRound?.you.pillz
      : beforeLastRound?.them.pillz;
    return {
      card: card?.name ?? `card ${move.index}`,
      pillz: betRevealed ? move.pillz : undefined,
      fury: betRevealed ? move.fury : undefined,
      availablePillz,
      attack: result && result.attack >= 0 ? result.attack : undefined,
    };
  };
  const board = {
    you: Array.from(ourHand),
    them: Array.from(theirHand),
    turn,
    selectedYou: selected(ourSide),
    selectedThem: selected(theirSide),
    beforeLastRound,
    battle: shownBattle?.moves.length
      ? {
        round: shownBattle.round + 1,
        you: battleSide(ourSide),
        them: battleSide(theirSide),
      }
      : undefined,
  };
  // battles.result is from our point of view and is the only fresh state sent when a
  // player forfeits mid-round. In that case `revealed` is still the preceding round.
  if (rec.result !== null) {
    return {
      round,
      headline,
      progress,
      warning,
      board,
      barFloor,
      you: {
        life: rec.result.player.life,
        pillz: rec.result.player.pillz,
      },
      them: {
        life: rec.result.opponent.life,
        pillz: rec.result.opponent.pillz,
      },
    };
  }
  if (
    rec.mySide !== null && revealed !== undefined &&
    !Number.isNaN(revealed.life[0])
  ) {
    const other = (1 - rec.mySide) as 0 | 1;
    return {
      round,
      headline,
      progress,
      warning,
      board,
      barFloor,
      you: {
        life: revealed.life[rec.mySide],
        pillz: revealed.pillz[rec.mySide],
      },
      them: { life: revealed.life[other], pillz: revealed.pillz[other] },
    };
  }
  return {
    round,
    headline,
    progress,
    warning,
    board,
    barFloor,
    you: { life: mine.life, pillz: mine.pillz },
    them: { life: theirs.life, pillz: theirs.pillz },
  };
}

/**
 * Only the result endpoint is authoritative. The first `done` status deliberately carries
 * pre-damage life totals, so inferring from it can flash the wrong banner for the fraction
 * of a second before `battles.result` arrives.
 */
function finalOutcome(rec: Reconstructed): ViewOutcome | undefined {
  const result = rec.result?.result;
  if (result === "win" || result === "lose" || result === "draw") {
    return result;
  }
  return undefined;
}

/** A compact account of the most recently resolved round, from our point of view. */
function resolvedSummary(
  rec: Reconstructed,
  round: number,
): string | undefined {
  if (rec.mySide === null || round < 1) return undefined;
  const result = rec.rounds[round - 1];
  const ours = result?.moves.find((m) => m.side === rec.mySide);
  const theirs = result?.moves.find((m) => m.side !== rec.mySide);
  const won = result?.resolution[rec.mySide]?.won;
  if (ours === undefined || theirs === undefined || won === undefined) {
    return undefined;
  }
  const cardName = (side: 0 | 1, index: number) =>
    rec.players[side].hand.find((c) => c.index === index)?.name ??
      `card ${index}`;
  return `round ${round}: ${cardName(rec.mySide, ours.index)} ${
    won ? "beat" : "lost to"
  } ${cardName((1 - rec.mySide) as 0 | 1, theirs.index)}`;
}

/**
 * The engine replays the captured moves to reach the current position, so if it models any
 * ability differently from the server the life and pillz it ends up with drift from the
 * real ones - and every number below is then advice about a game that is not quite the one
 * being played. The server reports both after each resolved round, so say so rather than
 * letting the drift pass silently.
 */
function crossCheck(
  game: Game,
  rec: Reconstructed,
  resolved: number,
): string | undefined {
  const last = rec.rounds[resolved - 1];
  if (resolved < 1 || last === undefined || Number.isNaN(last.life[0])) {
    return undefined;
  }

  // Engine P1 is whoever moved first in round 0; the server indexes by side.
  const p1 = rec.firstPlayer!, p2 = (1 - p1) as 0 | 1;
  const mine = rec.mySide === p1;
  const diffs: string[] = [];
  const check = (
    what: string,
    engine: number,
    server: number,
    ours: boolean,
  ) => {
    if (engine !== server) {
      diffs.push(
        `${ours ? "your" : "their"} ${what} ${engine} vs server ${server}`,
      );
    }
  };
  check("life", game.p1.life, last.life[p1], mine);
  check("life", game.p2.life, last.life[p2], !mine);
  check("pillz", game.p1.pillz, last.pillz[p1], mine);
  check("pillz", game.p2.pillz, last.pillz[p2], !mine);

  return diffs.length
    ? `engine disagrees with the server after round ${resolved}: ${
      diffs.join(", ")
    }` +
      " - advice may be wrong"
    : undefined;
}

/**
 * Keep live advice anchored to the observed game even when an engine rule is still wrong.
 * Replaying remains necessary for card state, permanents and round conditions, but life
 * and pillz after a completed round are authoritative in the server snapshot. Cross-check
 * before calling this so the underlying mismatch stays visible and testable.
 */
function syncObservedResources(
  game: Game,
  rec: Reconstructed,
  resolved: number,
) {
  const last = rec.rounds[resolved - 1];
  if (
    resolved < 1 || last === undefined || Number.isNaN(last.life[0]) ||
    Number.isNaN(last.pillz[0])
  ) return;

  const p1 = rec.firstPlayer!, p2 = (1 - p1) as 0 | 1;
  game.p1.life = last.life[p1];
  game.p2.life = last.life[p2];
  game.p1.pillz = last.pillz[p1];
  game.p2.pillz = last.pillz[p2];
}

/**
 * Replay the resolved rounds through the engine, reconcile its resources with the latest
 * server snapshot, then apply the opponent's card if they have committed this round.
 *
 * Replaying is still essential: permanents, Revenge / Confidence and "After [clan:…]" all
 * depend on how the game got here. The reconciliation prevents one imperfect ability rule
 * from giving the solver an impossible resource count for every later round.
 */
export function buildPosition(rec: Reconstructed): Built {
  const tc = rec.testcase;
  if (tc === null) {
    // The extractor says why far better than a guess would: an unknown card, a level the
    // card DB lacks, a Dojo battle whose rules differ.
    const issue = rec.issues.find((i) =>
      /no testcase|not in data|lacks that level|Dojo|first mover/.test(i)
    );
    return {
      settled: false,
      why: issue ?? "no engine testcase for this battle yet",
    };
  }
  if (rec.mySide === null) {
    return { settled: false, why: "cannot tell which side is yours yet" };
  }
  if (rec.firstPlayer === null) {
    return {
      settled: false,
      why: "cannot tell who moved first in round 0 yet",
    };
  }

  const lv = tc.levels ?? [];
  const h1 = HandGenerator.handOf(
    tc.cards.slice(0, 4) as HandOf<string>,
    lv.slice(0, 4) as HandOf<number | undefined>,
  );
  const h2 = HandGenerator.handOf(
    tc.cards.slice(4, 8) as HandOf<string>,
    lv.slice(4, 8) as HandOf<number | undefined>,
  );
  const game = new Game(
    new Player(tc.life, tc.pillz, 0),
    new Player(tc.life, tc.pillz, 1),
    h1,
    h2,
    Turn.PLAYER_1,
    false,
    tc.night ?? false,
  );

  // The battle API sends the ability actually attached to each card. In particular, EFC
  // can rebalance a semi-evo before our periodic character dump is refreshed. Searching a
  // card that the engine thinks has no ability is unsafe: its unseen base stats can have
  // changed too (battle 1131463 did both). Known versions are supplied through
  // battle_card_overrides.json; fail closed on the next unknown one rather than showing a
  // confident recommendation for a different card.
  const capturedCards = [
    ...rec.players[rec.firstPlayer].hand,
    ...rec.players[(1 - rec.firstPlayer) as 0 | 1].hand,
  ];
  const engineCards = [...game.h1, ...game.h2];
  const unknownLiveDefinition = capturedCards.find((captured, index) => {
    const observed = captured.ability?.description?.trim();
    return observed !== undefined && observed !== "" &&
      !/^No Ability$/i.test(observed) &&
      /^No Ability$/i.test(engineCards[index]?.abilityString ?? "No Ability");
  });
  if (unknownLiveDefinition !== undefined) {
    return {
      settled: false,
      why:
        `${unknownLiveDefinition.name} level ${unknownLiveDefinition.level} has ` +
        `a live ability missing from the card data - advice withheld until the card ` +
        `definition is refreshed`,
    };
  }

  for (const m of tc.moves) {
    game.select(m.s1[0], m.s1[1], m.s1[2], false);
    game.select(m.s2[0], m.s2[1], m.s2[2], false);
  }
  // Engine P1 is whoever moved first in round 0, which is how the testcase is normalised.
  const ourTurn = rec.mySide === rec.firstPlayer
    ? Turn.PLAYER_1
    : Turn.PLAYER_2;
  const round = tc.moves.length; // 0-based index of the round now being played
  const warning = crossCheck(game, rec, round);
  syncObservedResources(game, rec, round);
  const lastRound = resolvedSummary(rec, round);
  // A mid-round forfeit cannot make the replayed engine terminal because there is no
  // second move to resolve. The authoritative result response still ends the live view.
  if (!game.isPlaying || rec.result !== null) {
    const outcome = finalOutcome(rec);
    const finalState = holdingState(
      rec,
      game,
      ourTurn,
      Math.max(1, Math.min(4, rec.rounds.length)),
      "battle over",
      outcome === undefined ? "confirming result..." : "final",
      warning,
    );
    finalState.outcome = outcome;
    return {
      settled: true,
      finished: true,
      // The final board and result banner already say everything useful. Repeating the
      // last round as a diagnostic line made the finished screen look like an error.
      why: "",
      warning,
      holding: finalState,
    };
  }

  const current = rec.rounds[round];

  // Our decision for this round may already be made even when the round has not resolved.
  if (current?.moves.some((m) => m.side === rec.mySide)) {
    return {
      settled: true,
      why: `round ${
        round + 1
      }: your move is locked in · waiting for your opponent`,
      warning,
      holding: holdingState(
        rec,
        game,
        ourTurn,
        round + 1,
        "waiting for opponent",
        "move locked",
        warning,
      ),
    };
  }

  if (game.turn !== ourTurn) {
    // They move first this round. If they have committed, apply it so the search can answer
    // it; the pillz here is a placeholder because the server hides theirs until resolution,
    // and Search re-enumerates every bet they could have made anyway.
    const theirs = current?.moves.find((m) => m.side !== rec.mySide);
    if (theirs === undefined) {
      // From round two onward, start a provisional solve across every card they might
      // choose rather than leaving the table idle. Round two is substantially larger, but
      // its progressive ranking is still useful while they think; the committed-card
      // snapshot gets a new position key and replaces it with exact advice immediately.
      if (round + 1 >= 2) {
        return {
          game,
          round: round + 1,
          board: holdingState(
            rec,
            game,
            ourTurn,
            round + 1,
            "opponent choosing",
            lastRound ? `round ${round} resolved` : "estimating",
            warning,
          ).board,
          provisional: true,
          warning,
        };
      }
      return {
        settled: true,
        why: [
          lastRound,
          `round ${round + 1}: opponent moves first · waiting for their card`,
        ]
          .filter(Boolean).join(" · "),
        warning,
        holding: holdingState(
          rec,
          game,
          ourTurn,
          round + 1,
          "opponent to move",
          lastRound ? `round ${round} resolved` : "waiting",
          warning,
        ),
      };
    }
    game.select(theirs.index, 0, false, false);
    if (game.turn !== ourTurn) {
      // The engine alternates the first mover each round unless a Counter-attack leader is
      // in play. If the real game disagrees, every round looks like it is not ours - which
      // is a whole game with no advice, so say so rather than going quiet.
      return {
        settled: false,
        why: `round ${round + 1}: engine turn order disagrees with the server` +
          " (Counter-attack leader?) - no advice for this battle",
      };
    }
  }

  return {
    game,
    round: round + 1,
    board: holdingState(
      rec,
      game,
      ourTurn,
      round + 1,
      "",
      "",
      warning,
    ).board,
    warning,
  };
}

/** Our move in a resolved round, in the solver's notation, once the server reveals it. */
function ourMove(rec: Reconstructed, round: number): PlayedMove | undefined {
  const r = rec.rounds[round - 1];
  const m = r?.moves.find((x) => x.side === rec.mySide);
  if (m === undefined || Number.isNaN(r.life[0])) return undefined;
  return { index: m.index, pillz: m.pillz, fury: m.fury };
}

/**
 * Where a move sat in the ranking the solver had when it was played. Graded by how many
 * candidates were *strictly* better rather than by list position: several bets often tie,
 * and calling a joint-best move "rank 5 of 5" says the opposite of the truth.
 */
function gradeMove(search: Search, played: PlayedMove): PlayedMove {
  const scored = search.candidates.filter((c) => !Number.isNaN(c.average));
  const key = `${played.index} ${played.pillz} ${played.fury}`;
  const mine = scored.find((c) => c.key === key);
  if (mine === undefined) {
    // Either the search had not reached this move yet, or it was never a candidate.
    const known = search.candidates.some((c) => c.key === key);
    return {
      ...played,
      unevaluated: known ? "not evaluated yet" : "not a legal bet here",
    };
  }
  const percent = search.percent(mine.average);
  const best = Math.max(...scored.map((c) => search.percent(c.average)));
  return {
    ...played,
    percent,
    best,
    better: scored.filter((c) => search.percent(c.average) > percent).length,
    scored: scored.length,
    partial: !search.done,
  };
}

/** Our move at `pos`, graded against the ranking that search produced, once revealed. */
function gradeMoveAt(
  pos: Position,
  rec: Reconstructed,
): PlayedMove | undefined {
  const played = ourMove(rec, pos.round);
  return played && gradeMove(pos.search, played);
}

/** Name of the card a move played, from the hand the search was answering for. */
function cardName(game: Game, us: Turn, index: number) {
  return (us === Turn.PLAYER_1 ? game.h1 : game.h2)[index]?.name ??
    `card ${index}`;
}

// ---------------------------------------------------------------------------------------
// Entry sources
// ---------------------------------------------------------------------------------------

/**
 * Backfill a battle's earlier entries from its capture file.
 *
 * Lines are parsed individually and bad ones skipped: the log server appends to this file
 * while we read it, so the last line can be half-written, and one `JSON.parse` throw used
 * to lose the whole history for that battle - permanently, since the empty result is
 * cached and never retried. The history is what carries `meta` (which side is ours) and
 * the first rounds' move order, so losing it means never analysing that game at all.
 */
async function readBattleFile(id: number): Promise<CaptureEntry[]> {
  const abilities = await loadAbilities();
  const text = await Deno.readTextFile(`${BATTLE_DIR}/${id}.jsonl`);
  const entries: CaptureEntry[] = [];
  for (const line of text.split("\n")) {
    if (!line.trim()) continue;
    try {
      entries.push(JSON.parse(line) as CaptureEntry);
    } catch {
      // Torn final line; the feed delivers this entry live anyway.
    }
  }
  return expandEntries(entries, abilities);
}

interface FeedState {
  connection: ConnectionState;
  error?: string;
}

/** Server-sent events from log_server.ts, reconnecting for as long as we run. */
async function* liveEntries(
  url: string,
  onStatus: (state: FeedState) => void,
): AsyncGenerator<{ battleId: number; entry: CaptureEntry }> {
  for (;;) {
    try {
      onStatus({ connection: "starting" });
      const res = await fetch(url, {
        headers: { accept: "text/event-stream" },
      });
      if (!res.ok || res.body === null) throw new Error(`HTTP ${res.status}`);
      onStatus({ connection: "connected" });
      let buffer = "";
      const reader = res.body.pipeThrough(new TextDecoderStream()).getReader();
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        buffer += value;
        let cut: number;
        while ((cut = buffer.indexOf("\n\n")) >= 0) {
          const frame = buffer.slice(0, cut);
          buffer = buffer.slice(cut + 2);
          for (const line of frame.split("\n")) {
            if (!line.startsWith("data:")) continue; // ": connected" keepalive
            try {
              yield JSON.parse(line.slice(5));
            } catch { /* partial or malformed frame */ }
          }
        }
      }
      onStatus({ connection: "starting", error: "capture feed reconnecting" });
    } catch (e) {
      onStatus({
        connection: "offline",
        error: `capture feed unavailable (${(e as Error).message})`,
      });
    }
    await new Promise((r) => setTimeout(r, 2000));
  }
}

// ---------------------------------------------------------------------------------------
// Driving a search
// ---------------------------------------------------------------------------------------

/**
 * Work the search, redrawing as the ranking settles. Stops when the search finishes, the
 * budget runs out, or `interrupted()` says the position has moved on.
 */
async function drive(
  pos: Position,
  opts: AdvisorOptions,
  status: () => string,
  interrupted: () => boolean,
  autoQueue?: () => boolean,
  autoQueueHover?: () => boolean,
  setRepaint?: (paint: () => void) => void,
  connection?: () => ConnectionState,
  opponentRead?: () => OpponentReadState,
) {
  let painted = 0;
  const deadline = opts.budget ? Date.now() + opts.budget * 1000 : Infinity;
  const paint = (extra = "") => {
    painted = Date.now();
    const workerFailure = pos.search instanceof ParallelSearch
      ? pos.search.workerFailure
      : undefined;
    const detail = [
      extra,
      pos.rust?.status,
      workerFailure && `worker pool failed (${workerFailure}); using 1 worker`,
    ]
      .filter(Boolean).join("  ·  ");
    write(
      HOME + render(pos.game, pos.search, {
        status: [status(), detail].filter(Boolean).join("  ·  "),
        connection: connection?.(),
        battleId: pos.battleId,
        autoQueue: autoQueue?.(),
        autoQueueHover: autoQueueHover?.(),
        opponentRead: opponentRead?.(),
        board: pos.board,
        top: opts.top,
      }) + CLEAR_TO_END,
    );
  };

  setRepaint?.(paint);
  paint();
  while ((!pos.search.done || pos.rust?.waiting) && !interrupted()) {
    if (!pos.search.done) await pos.search.workFor(SLICE_MS);
    pos.rust?.settleCompare(pos.search);
    if (Date.now() - painted >= FRAME_MS) paint();
    if (Date.now() > deadline) {
      paint(`stopped at the ${opts.budget}s budget`);
      return;
    }
    await new Promise((r) => setTimeout(r, 0)); // let the feed reader run
  }
  pos.rust?.settleCompare(pos.search);
  paint();
}

// ---------------------------------------------------------------------------------------
// Modes
// ---------------------------------------------------------------------------------------

/**
 * A deterministic round-four position whose high-win moves all carry immediate-KO risk,
 * while three deliberately lower-win moves are proven safe. This exercises the real view
 * and sizing logic without depending on a rare live-game state or the capture server.
 */
function safePreview(): { game: Game; search: Search; board: ViewBoard } {
  const game = new Game(
    new Player(50, 12, 0),
    new Player(50, 12, 1),
    HandGenerator.handOf(["Genmaicha", "Orka", "Sando", "Deborah"]),
    HandGenerator.handOf([
      "Nathan",
      "El Kuzco",
      "Noon Steevens",
      "Strygia",
    ]),
    Turn.PLAYER_1,
    false,
  );
  // Reach round four normally so the board carries authentic played/won card states.
  for (let index = 0; index < 3; index++) {
    game.select(index, 0, false, false);
    game.select(index, 0, false, false);
  }
  game.p1.life = 5;
  game.p2.life = 7;
  game.p1.pillz = 7;
  game.p2.pillz = 7;

  const search = new Search(game);
  while (search.step()) { /* finish the small round-four preview search */ }

  // Candidate values are stored in P1's frame even when the preview's asking side is P2.
  const valueForUs = (value: number) =>
    search.us === Turn.PLAYER_1 ? value : -value;

  // Give every ordinary move a reasonable average but some immediate-KO exposure. Then
  // place three lower-win zero-risk alternatives below the normal recommendation podium.
  for (const candidate of search.candidates) {
    candidate.average = 0;
    candidate.minimax = valueForUs(-1);
    candidate.kos = 0;
    candidate.koed = 1;
  }
  const score = (
    pillz: number,
    fury: boolean,
    average: number,
    koed: number,
    kos = 0,
  ) => {
    const candidate = search.candidates.find((move) =>
      move.pillz === pillz && move.fury === fury
    );
    if (candidate === undefined) throw new Error("invalid preview move");
    candidate.average = valueForUs(average);
    candidate.minimax = valueForUs(-1);
    candidate.koed = koed;
    candidate.kos = kos;
  };
  score(7, false, 0.84, 6, 3);
  score(4, true, 0.72, 4, 2);
  score(6, false, 0.62, 2);
  score(3, false, -0.4, 0);
  score(1, false, -0.6, 0);
  score(0, false, -0.8, 0);
  const ourHand = search.us === Turn.PLAYER_1 ? game.h1 : game.h2;
  const theirHand = search.us === Turn.PLAYER_1 ? game.h2 : game.h1;
  const board: ViewBoard = {
    you: Array.from(ourHand),
    them: Array.from(theirHand),
    battle: {
      round: 3,
      you: {
        card: ourHand[2].name,
        pillz: 0,
        fury: false,
        attack: ourHand[2].attack.final,
      },
      them: {
        card: theirHand[2].name,
        pillz: 0,
        fury: false,
        attack: theirHand[2].attack.final,
      },
    },
  };
  return { game, search, board };
}

async function previewMode(opts: AdvisorOptions) {
  if (opts.preview === "results") {
    write(HOME + resultStylePreview(consoleSize()) + CLEAR_TO_END);
    if (Deno.stdin.isTerminal()) {
      await Deno.stdin.read(new Uint8Array(1));
    }
    return;
  }
  const { game, search, board } = safePreview();
  write(
    HOME + render(game, search, {
      status: "PREVIEW · synthetic zero-risk example · press Enter to exit",
      top: opts.top,
      board,
    }) + CLEAR_TO_END,
  );
  // Cooked terminal input keeps the preview portable on Windows: Enter exits, while the
  // normal SIGINT handler still restores the alternate screen for Ctrl+C.
  if (Deno.stdin.isTerminal()) {
    await Deno.stdin.read(new Uint8Array(1));
  }
}

async function replayMode(opts: AdvisorOptions) {
  const id = opts.replay!;
  const all = await readBattleFile(id);

  // Feed the snapshots in order and stop at each position that was ours to decide.
  const statuses = all.filter((e) => e.kind === "status");
  let lastKey = "";
  let decisions = 0;
  for (let i = 1; i <= statuses.length; i++) {
    const prefix = [
      ...all.filter((e) => e.kind === "meta"),
      ...statuses.slice(0, i),
    ];
    let rec: Reconstructed;
    try {
      rec = reconstruct(id, prefix);
    } catch {
      continue; // not enough snapshots yet to reconstruct anything
    }
    const key = positionKey(rec, id);
    if (key === undefined || key === lastKey) continue;
    lastKey = key;
    const built = buildPosition(rec);
    if (!isOurs(built)) continue; // not ours to decide
    decisions++;

    const search = createSearch(built.game, opts.workers, built.provisional);
    const pos: Position = {
      key,
      game: built.game,
      search,
      battleId: id,
      round: built.round,
      board: built.board,
    };
    startRustForPosition(pos, rec, opts);
    stopRustDecision = async () => {
      pos.rust?.cancel();
      cancelSearch(pos.search);
      await pos.rust?.cancelAndWait();
    };
    const label = `replay ${id} · decision ${decisions} · round ${built.round}`;
    await drive(pos, opts, () => label, () => false);

    // The full battle file knows what was actually played; grade it against the ranking.
    const full = reconstruct(id, all);
    const played = ourMove(full, built.round);
    write(
      HOME + render(pos.game, pos.search, {
        status: label,
        board: pos.board,
        top: opts.top,
        played: played && gradeMove(pos.search, played),
      }) + CLEAR_TO_END,
    );
    // Leave the frame up long enough to read before moving to the next decision.
    await new Promise((r) => setTimeout(r, 1500));
    await stopRustDecision();
    stopRustDecision = async () => {};
  }
  if (decisions === 0) {
    write(ALT_SCREEN_OFF);
    console.error(
      `battle ${id}: no decision of yours could be reconstructed ` +
        `(needs a testcase and a known side; check captures/games/${id}.json issues)`,
    );
  }
}

async function liveMode(opts: AdvisorOptions) {
  const battles = new Map<number, CaptureEntry[]>();
  /** Active absolute server-side card slots for each battle. */
  const hovers = new Map<number, Set<number>>();
  /** When each active hover became visible, used to guarantee a readable minimum dwell. */
  const hoverEnteredAt = new Map<number, Map<number, number>>();
  /** Delayed leave timers, one per absolute card slot. */
  const hoverLeaves = new Map<
    number,
    Map<number, ReturnType<typeof setTimeout>>
  >();
  /** Absolute server-side card slot whose pillz chooser is open for each battle. */
  const selecting = new Map<number, number>();
  /** Delayed chooser-close timers, one per battle. */
  const selectingLeaves = new Map<
    number,
    ReturnType<typeof setTimeout>
  >();
  let feedState: FeedState = { connection: "starting" };
  let pos: Position | undefined;
  stopRustDecision = async () => {
    if (pos === undefined) return;
    pos.rust?.cancel();
    cancelSearch(pos.search);
    await pos.rust?.cancelAndWait();
  };
  let pending: { rec: Reconstructed; battleId: number } | undefined;
  let holding: HoldingState | undefined;
  let lastPlayed: PlayedMove | undefined;
  /** Why nothing is being analysed, shown on the idle screen instead of a silent wait. */
  let diagnosis = "";
  /** Decision points already found to be none of ours, so they are not rebuilt each poll. */
  const skipped = new Set<string>();
  /** Graded moves from earlier rounds, newest first. */
  const history: { round: number; card: string; move: PlayedMove }[] = [];
  const autoQueueUrl = controlUrl(opts.feed);
  let autoQueue = false;
  let autoQueueHover = false;
  const opponentRead: OpponentReadState = { targets: [] };
  let controlError = "";
  /** The currently visible frame; controls can repaint it without waiting for the feed loop. */
  let repaint = () => {};

  const applyCardInteractions = (
    board: ViewBoard | undefined,
    battleId: number,
    mySide: number | null | undefined,
  ) => {
    if (board === undefined || (mySide !== 0 && mySide !== 1)) return;
    board.hoveredYou = undefined;
    board.hoveredThem = undefined;
    board.choosingYou = undefined;
    board.choosingThem = undefined;
    for (const slot of hovers.get(battleId) ?? []) {
      const side = slot < 4 ? 0 : 1;
      const index = slot % 4;
      if (side === mySide) board.hoveredYou = index;
      else board.hoveredThem = index;
    }
    const selectingSlot = selecting.get(battleId);
    if (selectingSlot !== undefined) {
      const side = selectingSlot < 4 ? 0 : 1;
      const index = selectingSlot % 4;
      if (side === mySide) board.choosingYou = index;
      else board.choosingThem = index;
    }
  };

  const applyVisibleCardInteractions = (
    battleId: number,
    mySide: number | null | undefined,
  ) => {
    if (pos?.battleId === battleId) {
      applyCardInteractions(pos.board, battleId, mySide);
    }
    if (pending?.battleId === battleId) {
      applyCardInteractions(holding?.board, battleId, mySide);
    }
  };

  const clearHoverBattle = (battleId: number) => {
    for (const timer of hoverLeaves.get(battleId)?.values() ?? []) {
      clearTimeout(timer);
    }
    hoverLeaves.delete(battleId);
    hoverEnteredAt.delete(battleId);
    hovers.delete(battleId);
  };

  const clearSelectingBattle = (battleId: number) => {
    const timer = selectingLeaves.get(battleId);
    if (timer !== undefined) clearTimeout(timer);
    selectingLeaves.delete(battleId);
    selecting.delete(battleId);
  };

  const updateHover = (
    battleId: number,
    entry: Extract<CaptureEntry, { kind: "hover" }>,
    holdLeave = false,
    mySide?: number | null,
  ) => {
    let active = hovers.get(battleId);
    if (active === undefined) {
      active = new Set();
      hovers.set(battleId, active);
    }
    let enteredAt = hoverEnteredAt.get(battleId);
    if (enteredAt === undefined) {
      enteredAt = new Map();
      hoverEnteredAt.set(battleId, enteredAt);
    }
    const slot = entry.side * 4 + entry.index;

    if (entry.active) {
      // A browser pointer can only be over one card. Replacing the prior slot also heals a
      // missed leave frame instead of leaving several cards with hover outlines forever.
      if (!active.has(slot)) {
        clearHoverBattle(battleId);
        active = new Set([slot]);
        hovers.set(battleId, active);
        enteredAt = new Map([[slot, Date.now()]]);
        hoverEnteredAt.set(battleId, enteredAt);
      } else {
        const timer = hoverLeaves.get(battleId)?.get(slot);
        if (timer !== undefined) {
          clearTimeout(timer);
          hoverLeaves.get(battleId)?.delete(slot);
        }
      }
      return;
    }

    if (!active.has(slot)) return;
    const remove = () => {
      hoverLeaves.get(battleId)?.delete(slot);
      active.delete(slot);
      enteredAt.delete(slot);
      applyVisibleCardInteractions(battleId, mySide);
      repaint();
    };
    const remaining = holdLeave
      ? Math.max(0, HOVER_HOLD_MS - (Date.now() - (enteredAt.get(slot) ?? 0)))
      : 0;
    if (remaining === 0) {
      remove();
      return;
    }
    let leaves = hoverLeaves.get(battleId);
    if (leaves === undefined) {
      leaves = new Map();
      hoverLeaves.set(battleId, leaves);
    }
    if (leaves.has(slot)) return;
    leaves.set(slot, setTimeout(remove, remaining));
  };

  const updateSelecting = (
    battleId: number,
    entry: Extract<CaptureEntry, { kind: "selecting" }>,
    holdClose = false,
    mySide?: number | null,
  ) => {
    const slot = entry.side * 4 + entry.index;
    const pendingClose = selectingLeaves.get(battleId);

    if (entry.active) {
      if (pendingClose !== undefined) clearTimeout(pendingClose);
      selectingLeaves.delete(battleId);
      selecting.set(battleId, slot);
      return;
    }
    if (selecting.get(battleId) !== slot) return;

    const remove = () => {
      selectingLeaves.delete(battleId);
      if (selecting.get(battleId) === slot) selecting.delete(battleId);
      applyVisibleCardInteractions(battleId, mySide);
      repaint();
    };
    if (!holdClose) {
      remove();
      return;
    }
    if (pendingClose !== undefined) return;
    selectingLeaves.set(
      battleId,
      setTimeout(remove, CHOOSER_CLOSE_HOLD_MS),
    );
  };
  /** Wake the main loop as soon as the feed changes, with a timeout only as a safety net. */
  let revision = 0;
  let wake: (() => void) | undefined;
  const signal = () => {
    revision++;
    const resolve = wake;
    wake = undefined;
    resolve?.();
  };
  const waitForSignal = async (seen: number) => {
    if (revision !== seen) return;
    await new Promise<void>((resolve) => wake = resolve);
  };
  try {
    autoQueue = await readAutoQueue(autoQueueUrl);
  } catch (e) {
    controlError = `auto-queue unavailable: ${(e as Error).message}`;
  }
  let toggleWork: Promise<void> = Promise.resolve();
  const toggleAutoQueue = () => {
    toggleWork = toggleWork.then(async () => {
      try {
        autoQueue = await writeAutoQueue(autoQueueUrl, !autoQueue);
        controlError = "";
      } catch (e) {
        controlError = `auto-queue unavailable: ${(e as Error).message}`;
      }
      repaint();
    });
  };
  stopControlInput = startControlInput(toggleAutoQueue, (inside) => {
    if (autoQueueHover === inside) return;
    autoQueueHover = inside;
    repaint();
  }, (column, row, pressed) => {
    const key = opponentReadClick(
      opponentRead.targets ?? [],
      column,
      row,
    );
    let changed = opponentRead.hovered !== key;
    opponentRead.hovered = key;
    if (pressed && key !== undefined && opponentRead.selected !== key) {
      opponentRead.selected = key;
      changed = true;
    }
    if (changed) repaint();
  });
  const status = () =>
    [feedState.error, controlError].filter(Boolean).join(" · ");

  const clearDecisionState = () => {
    if (pos !== undefined) cancelPosition(pos);
    pos = undefined;
    lastPlayed = undefined;
    history.length = 0;
  };

  const feed = liveEntries(opts.feed, (state) => {
    feedState = state;
    signal();
  });

  // The feed reader runs independently of the search so a move by the opponent lands even
  // while a solve is mid-flight; `pending` is the newest position the reader has seen.
  (async () => {
    for await (const { battleId, entry } of feed) {
      let entries = battles.get(battleId);
      if (entries === undefined) {
        // Joined mid-battle: backfill the earlier snapshots from the capture file, or the
        // first rounds' move order (and so the first mover) cannot be recovered.
        try {
          entries = await readBattleFile(battleId);
        } catch {
          entries = [];
        }
        battles.set(battleId, entries);
        for (const previous of entries) {
          if (previous.kind === "hover") updateHover(battleId, previous);
          else if (previous.kind === "selecting") {
            updateSelecting(battleId, previous);
          }
        }
      }
      entries.push(entry);
      if (entry.kind === "hover" || entry.kind === "selecting") {
        const mySide = pending?.battleId === battleId
          ? pending.rec.mySide
          : undefined;
        if (entry.kind === "hover") {
          updateHover(battleId, entry, true, mySide);
        } else {
          updateSelecting(battleId, entry, true, mySide);
        }
        applyVisibleCardInteractions(battleId, mySide);
        repaint();
        continue;
      }
      if (
        entry.kind === "result" ||
        (entry.kind === "status" && entry.battle?.status !== "playing")
      ) {
        clearHoverBattle(battleId);
        clearSelectingBattle(battleId);
      }
      // `battles.result` usually arrives a fraction of a second after the first `done`
      // status—well before the site's result animation finishes. Reconstruct on it too so
      // the banner uses that definitive win/lose/draw response immediately.
      if (entry.kind !== "status" && entry.kind !== "result") continue;
      try {
        pending = { rec: reconstruct(battleId, entries), battleId };
        applyVisibleCardInteractions(battleId, pending.rec.mySide);
        signal();
      } catch { /* too early to reconstruct */ }
    }
  })();

  /** The decision the newest snapshot is offering, or undefined if there is none. */
  const currentKey = () =>
    pending === undefined
      ? undefined
      : positionKey(pending.rec, pending.battleId);

  for (;;) {
    const seen = revision;
    const next = pending;
    const key = currentKey();

    // A new decision point: grade whatever we played at the previous one, then switch.
    // A key of undefined means the battle cannot even be identified yet; buildPosition
    // returns early and cheaply in that case, and says why.
    const fresh = next !== undefined &&
      (key === undefined || (key !== pos?.key && !skipped.has(key)));
    if (fresh) {
      // A battle transition can arrive without us ever seeing the previous battle's final
      // snapshot (advisor restart, feed reconnect, rapid auto-queue). Clear before building
      // the new Game, and never grade an old move against the new battle's capture.
      if (pos !== undefined && next!.battleId !== pos.battleId) {
        clearDecisionState();
      }
      const built = buildPosition(next!.rec);
      applyCardInteractions(
        isOurs(built) ? built.board : built.holding?.board,
        next!.battleId,
        next!.rec.mySide,
      );
      if (!isOurs(built)) {
        // A settled non-decision (our move is locked, their turn, or battle over) makes the
        // previous solve obsolete. Transient incomplete snapshots do not: cancelling for
        // one of those would recreate the old "never gets anywhere" failure.
        if (built.settled && pos !== undefined && key !== pos.key) {
          cancelPosition(pos);
        }
        diagnosis = built.why;
        holding = built.holding;
        if (built.finished) clearDecisionState();
        // Only remember a settled answer. Blacklisting a transient one - data that had not
        // arrived yet - used to silence the advisor for the rest of the battle.
        if (built.settled && key !== undefined) skipped.add(key);
      } else {
        // Grade what we played at the decision we are leaving and keep it on screen; the
        // ranking it is graded against disappears with the old search.
        if (pos !== undefined) {
          const played = lastPlayed ?? gradeMoveAt(pos, next.rec);
          // One entry per round: a round can be graded more than once as the search
          // advances, and the later grading is the better-informed one.
          if (
            played !== undefined && !history.some((h) => h.round === pos!.round)
          ) {
            history.unshift({
              round: pos.round,
              card: cardName(pos.game, pos.search.us, played.index),
              move: played,
            });
            history.splice(3);
          }
          cancelPosition(pos);
        }
        // buildPosition has already taken the engine's process-global battle cache, so the
        // previous search must not be stepped again. Nothing else does: the feed reader
        // never builds a Game, and only this loop steps a search.
        // isOurs implies positionKey found a testcase and a side, so `key` is defined.
        pos = {
          key: key!,
          game: built.game,
          search: createSearch(
            built.game,
            opts.workers,
            built.provisional,
          ),
          battleId: next!.battleId,
          round: built.round,
          board: built.board,
          warning: built.warning,
        };
        startRustForPosition(pos, next!.rec, opts, signal);
        opponentRead.selected = undefined;
        opponentRead.hovered = undefined;
        opponentRead.targets!.length = 0;
        holding = undefined;
        diagnosis = "";
        lastPlayed = undefined;
        history.length = Math.min(history.length, 3);
      }
    }

    if (pos !== undefined && !pos.search.done && key === pos.key) {
      // Keep working the current decision. The interrupt only fires when the *decision*
      // changes, not on every snapshot - the client polls several times a second, and
      // restarting on each one meant the search never got anywhere.
      const here = pos;
      await drive(
        here,
        opts,
        () => here.warning ?? status(),
        () => currentKey() !== here.key,
        () => autoQueue,
        () => autoQueueHover,
        (paint) => repaint = paint,
        () => feedState.connection,
        () => opponentRead,
      );
      continue;
    }

    const paintHolding = () => {
      if (pos !== undefined) {
        // Solved, or our turn is over: retain the previous advice and verdict, but put the
        // site's current round and resources in the header so the screen never looks stale.
        if (lastPlayed === undefined && next !== undefined) {
          lastPlayed = gradeMoveAt(pos, next.rec);
        }
        const over = key !== pos.key;
        opponentRead.targets!.length = 0;
        if (over) opponentRead.hovered = undefined;
        const phase = holding === undefined ? undefined : viewPhase(holding);
        const detail = phase?.headline === "battle over"
          ? (holding?.warning ?? pos.warning ?? "")
          : holding?.warning ?? pos.warning ??
            (diagnosis ||
              (over
                ? "waiting for the next turn"
                : "solved, waiting for you to play"));
        write(
          HOME + render(pos.game, pos.search, {
            status: [status(), detail].filter(Boolean).join(" · "),
            connection: feedState.connection,
            battleId: pos.battleId,
            autoQueue,
            autoQueueHover,
            opponentRead: over ? undefined : opponentRead,
            phase,
            board: pos.board,
            played: lastPlayed,
            // The round on screen is already shown by `played`; listing it again in the
            // history printed the same move twice, and with two different verdicts when the
            // two gradings happened at different points in the search.
            history: history.filter((h) => h.round !== pos!.round),
            top: opts.top,
          }) + CLEAR_TO_END,
        );
      } else {
        opponentRead.targets!.length = 0;
        opponentRead.hovered = undefined;
        const phase = holding === undefined ? undefined : viewPhase(holding);
        const note = phase?.headline === "battle over"
          ? (holding?.warning ?? "")
          : diagnosis;
        write(
          HOME + idle(
            status(),
            note,
            undefined,
            autoQueue,
            autoQueueHover,
            phase,
            opts.resultStyle,
            feedState.connection,
            pending?.battleId,
          ) + CLEAR_TO_END,
        );
      }
    };
    repaint = paintHolding;
    paintHolding();
    await waitForSignal(seen);
  }
}

// ---------------------------------------------------------------------------------------

if (import.meta.main) {
  const opts = parseArgs(Deno.args);
  silenceEngine();
  let managedCapture: Deno.ChildProcess | undefined;
  let alternateScreen = false;
  const restore = async () => {
    stopControlInput();
    await stopRustDecision();
    if (alternateScreen) write(ALT_SCREEN_OFF);
    stopCaptureServer(managedCapture);
    Deno.exit(0);
  };
  exitAdvisor = restore;
  Deno.addSignalListener("SIGINT", restore);
  try {
    if (opts.preview === undefined && opts.replay === undefined) {
      managedCapture = await ensureCaptureServer(opts.feed);
    }
    write(ALT_SCREEN_ON);
    alternateScreen = true;
    if (opts.preview !== undefined) await previewMode(opts);
    else if (opts.replay !== undefined) await replayMode(opts);
    else await liveMode(opts);
  } finally {
    stopControlInput();
    await stopRustDecision();
    if (alternateScreen) write(ALT_SCREEN_OFF);
    stopCaptureServer(managedCapture);
  }
}
