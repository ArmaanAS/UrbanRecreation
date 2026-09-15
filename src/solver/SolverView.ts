// deno-lint-ignore-file no-control-regex
// Terminal view for a running Search.
//
// GameRenderer prints a board and scrolls away; this is the opposite - one screen redrawn
// in place while the solver grinds, so the ranking can be watched settling. Rendering is a
// pure function of (game, search, extras) returning lines, so it can be diffed and tested.
//
// Both recommendation tables precede the live battle board. When the complete original
// 130-column hand canvases, player pillz bars and centre battle strip fit, they are shown
// together; otherwise the whole board is omitted. There is deliberately no squeezed card
// substitute: truncating that UI made it look broken and hid half the information anyway.
//
// Two rules keep it readable, both learned from a real game on an 80-column terminal, where
// the matrix was 116 columns wide:
//
//   Nothing may wrap or scroll. A wrapped line shifts every line after it, so the
//   cursor-home redraw lands in the wrong place and the frame overprints itself into
//   duplicated letters and misaligned columns. `frame()` therefore clips every line to the
//   real terminal width and drops any line past the last row, and the layout sizes its
//   columns to fit first, so clipping is a backstop rather than the normal case.
//
//   Colours come from the terminal's own sixteen, never the 256-colour cube, so the view
//   sits in whatever theme the terminal has instead of importing a palette of its own.
import Game from "../game/Game.ts";
import type Card from "../game/Card.ts";
import { Turn } from "../game/types/Types.ts";
import GameRenderer from "../utils/GameRenderer.ts";
import Search, {
  type Candidate,
  moveCost,
  SearchMode,
  shownPercent,
} from "./Search.ts";

const ESC = "\x1b[";
export const ALT_SCREEN_ON = `${ESC}?1049h${ESC}?25l`;
export const ALT_SCREEN_OFF = `${ESC}?25h${ESC}?1049l`;
/** Home the cursor and clear forward, so a redraw never flickers a blank frame. */
export const HOME = `${ESC}H`;
export const CLEAR_TO_END = `${ESC}0J`;
/** Mouse-motion tracking with unambiguous decimal coordinates. */
export const MOUSE_ON = `${ESC}?1003h${ESC}?1006h`;
export const MOUSE_OFF = `${ESC}?1006l${ESC}?1003l`;
/**
 * Erase from the cursor to the end of the line. Every rendered line ends with this.
 *
 * Without it a redraw only overwrites as far as the new line reaches, so wherever a line
 * got shorter the tail of the previous frame survived - "solved in 0ms" left a stray "=",
 * a four-pillz matrix kept the columns of a twelve-pillz one, and the legend appeared
 * twice. Clearing to the end of the screen at the end of the frame cannot fix that: by
 * then the cursor is past those rows.
 */
export const CLEAR_LINE = `${ESC}K`;

// The terminal's own palette; these are remapped by the user's theme, unlike 38;5;N.
const RED = 31, GREEN = 32, YELLOW = 33, BLUE = 34, MAGENTA = 35, CYAN = 36;
const B_RED = 91, B_GREEN = 92;

const fg = (n: number, s: string) => `${ESC}${n}m${s}${ESC}39m`;
const dim = (s: string) => `${ESC}2m${s}${ESC}22m`;
/**
 * Column headings and other chrome. Bright black is the terminal's own grey, and reads a
 * shade lighter than SGR-2 dim, which most themes render as a very dark version of the
 * foreground - near-black on a dark background.
 */
const label = (s: string) => `${ESC}90m${s}${ESC}39m`;
const bold = (s: string) => `${ESC}1m${s}${ESC}22m`;
const plain = (s: string) => s.replace(/\x1b\[[0-9;]*m/g, "");

/** Zero is a neutral commitment; positive bets keep their normal emphasis colour. */
const pillzLabel = (pillz: number, colour = BLUE) =>
  pillz === 0 ? label("0 pillz") : fg(colour, `${pillz} pillz`);
const betPillzColour = (
  pillz: number,
  fury: boolean,
  availablePillz: number | undefined,
) =>
  availablePillz !== undefined && pillz + (fury ? 3 : 0) === availablePillz
    ? MAGENTA
    : BLUE;

/**
 * Printable width. Terminals draw CJK and most emoji two cells wide, and miscounting one
 * shifts every column after it, so those count as 2 and the matrix stays aligned.
 */
const width = (s: string) => {
  let n = 0;
  for (const ch of plain(s)) {
    const c = ch.codePointAt(0)!;
    const wide = (c >= 0x1100 && c <= 0x115f) || c === 0x2329 || c === 0x232a ||
      (c >= 0x2e80 && c <= 0xa4cf) || (c >= 0xac00 && c <= 0xd7a3) ||
      (c >= 0xf900 && c <= 0xfaff) || (c >= 0xfe30 && c <= 0xfe6f) ||
      (c >= 0xff00 && c <= 0xff60) || (c >= 0xffe0 && c <= 0xffe6) ||
      (c >= 0x1f300 && c <= 0x1faff) || c === 0x26a1;
    n += wide ? 2 : 1;
  }
  return n;
};
const padEnd = (s: string, n: number) =>
  s + " ".repeat(Math.max(0, n - width(s)));
const padStart = (s: string, n: number) =>
  " ".repeat(Math.max(0, n - width(s))) + s;

/** Cut to `n` printable columns, keeping escape sequences intact and colours closed. */
function clip(s: string, n: number): string {
  if (width(s) <= n) return s;
  let out = "", used = 0, i = 0;
  while (i < s.length) {
    if (s[i] === "\x1b") {
      const end = s.indexOf("m", i);
      if (end < 0) break;
      out += s.slice(i, end + 1);
      i = end + 1;
      continue;
    }
    const ch = String.fromCodePoint(s.codePointAt(i)!);
    const w = width(ch);
    if (used + w > n) break;
    out += ch;
    used += w;
    i += ch.length;
  }
  return out + `${ESC}0m`;
}

/** Win chance to one of the terminal's own colours. */
function heat(pct: number) {
  if (pct >= 80) return B_GREEN;
  if (pct >= 60) return GREEN;
  if (pct >= 45) return YELLOW;
  if (pct >= 25) return RED;
  return B_RED;
}

const pct = (n: number) => `${Math.round(n)}`;

/**
 * Once three complete answers exist, reserve the visible podium for complete answers.
 * Partial estimates may still move around underneath it, but a briefly optimistic sample
 * can no longer evict a move whose every opponent reply has actually been searched.
 */
export function displayRanked(search: Search, locked = 3): Candidate[] {
  const ranked = search.ranked().filter((c) => !Number.isNaN(c.average));
  const complete = ranked.filter((c) => search.settled(c));
  if (complete.length < locked) return ranked;

  const podium = complete.slice(0, locked);
  const keys = new Set(podium.map((c) => c.key));
  return [...podium, ...ranked.filter((c) => !keys.has(c.key))];
}

/**
 * Fully proven moves that cannot be answered by an immediate knockout. A partial move
 * with no knockout observed yet is not safe: an unsearched reply may still kill us.
 */
export function displaySafeRanked(search: Search): Candidate[] {
  return search.ranked().filter((c) =>
    search.settled(c) && c.koed === 0 &&
    search.shownPercent(c.average) > 0
  );
}

export interface OpponentReadResult {
  candidate: Candidate;
  value: number;
  ko: boolean;
  koed: boolean;
}

/** Best responses under one explicit hypothesis about the opponent's hidden wager. */
export function opponentReadRanked(
  search: Search,
  pillz: number,
  fury: boolean,
): OpponentReadResult[] {
  const results: OpponentReadResult[] = [];
  for (const candidate of search.candidates) {
    const outcome = search.outcome(candidate, { pillz, fury });
    if (outcome !== undefined) results.push({ candidate, ...outcome });
  }
  return results.sort((a, b) => {
    const av = search.percent(a.value), bv = search.percent(b.value);
    if (av !== bv) return bv - av;
    if (a.ko !== b.ko) return a.ko ? -1 : 1;
    if (a.koed !== b.koed) return a.koed ? 1 : -1;
    const cost = moveCost(a.candidate) - moveCost(b.candidate);
    if (cost !== 0) return cost;
    return a.candidate.index - b.candidate.index;
  });
}

interface RelativeReadTarget {
  key: string;
  column: number;
  endColumn: number;
  row: number;
}

function opponentReadPanel(
  search: Search,
  ourHand: Game["h1"],
  opponentName: string,
  ourPillz: number,
  theirPillz: number,
  cols: number,
  state?: OpponentReadState,
): { lines: string[]; targets: RelativeReadTarget[] } {
  const moves = search.opponentMoves;
  const plainMoves = moves.filter((move) => !move.fury).sort((a, b) =>
    a.pillz - b.pillz
  );
  const furyMoves = moves.filter((move) => move.fury).sort((a, b) =>
    a.pillz - b.pillz
  );
  const available = [...plainMoves, ...furyMoves];
  const selected =
    available.find((move) =>
      opponentReadKey(move.pillz, move.fury) === state?.selected
    ) ?? moves[0];
  const selectedKey = opponentReadKey(selected.pillz, selected.fury);
  const targets: RelativeReadTarget[] = [];

  const button = (pillz: number, fury: boolean) => {
    const key = opponentReadKey(pillz, fury);
    const allIn = pillz + (fury ? 3 : 0) === theirPillz;
    const number = pillz === 0
      ? label("0")
      : fg(allIn ? MAGENTA : BLUE, String(pillz));
    const content = fury ? `${number}${fg(RED, "+F")}` : number;
    const rendered = `[${content}]`;
    if (key === selectedKey) return bold(fg(CYAN, rendered));
    return key === state?.hovered ? `${ESC}7m${rendered}${ESC}27m` : rendered;
  };

  const selector = (
    title: string,
    row: number,
    options: typeof moves,
  ) => {
    let line = `  ${padEnd(label(title), 7)}`;
    if (options.length === 0) return line + dim("unavailable");
    for (const move of options) {
      const rendered = button(move.pillz, move.fury);
      line += " ";
      const column = width(line) + 1;
      line += rendered;
      const endColumn = width(line);
      if (column <= cols) {
        targets.push({
          key: opponentReadKey(move.pillz, move.fury),
          column,
          endColumn: Math.min(cols, endColumn),
          row,
        });
      }
    }
    return line;
  };

  const lines = [
    "",
    "",
    `  ${label("OPP READ")} ${dim("—")} ${bold(opponentName)}  ${
      dim("hypothetical")
    }`,
    selector("Plain", 3, plainMoves),
    selector("Fury", 4, furyMoves),
    "",
  ];
  const assumed = pillzLabel(
    selected.pillz,
    betPillzColour(selected.pillz, selected.fury, theirPillz),
  ) + (selected.fury ? fg(RED, " + Fury") : "");
  const results = opponentReadRanked(
    search,
    selected.pillz,
    selected.fury,
  );
  const nameW = Math.max(30, Math.min(52, cols - 25));
  lines.push(
    padEnd(`  ${label("Best replies if OPP used")} ${assumed}`, nameW) +
      padStart(label(search.openingEstimate ? "Score" : "Result"), 9) +
      padStart(label("Now"), 7) +
      padStart(dim(`${results.length}/${search.candidates.length}`), 8),
  );
  const rows = results.slice(0, 3).map((result, index) => {
    const score = search.percent(result.value);
    const resultText = search.openingEstimate
      ? fg(heat(score), pct(score))
      : score > 50
      ? fg(GREEN, "Win")
      : score < 50
      ? fg(RED, "Lose")
      : fg(YELLOW, "Draw");
    const now = result.ko
      ? bold(fg(B_GREEN, "KO"))
      : result.koed
      ? bold(fg(B_RED, "KO'd"))
      : dim("-");
    return padEnd(
      clip(
        ` ${dim(`${index + 1}.`)} ${
          moveLabel(ourHand, result.candidate, nameW - 20, ourPillz)
        }`,
        nameW,
      ),
      nameW,
    ) + padStart(resultText, 9) + padStart(now, 7);
  });
  if (rows.length === 0) {
    rows.push(`  ${dim("calculating this assumption…")}`);
  }
  while (rows.length < 3) rows.push("");
  lines.push(...rows);
  return { lines, targets };
}

function recommendationHeading(
  cols: number,
  title: string,
  openingEstimate = false,
): string {
  const nameW = Math.max(14, Math.min(34, cols - (cols >= 78 ? 35 : 23)));
  const koCol = cols >= 78 ? 6 : 0;
  return padEnd(label(`  ${title}`), nameW) +
    padStart(label(openingEstimate ? "Avg" : "Win"), 6) +
    padStart(label(openingEstimate ? "Range" : "Worst"), 8) +
    (koCol
      ? padStart(label("KO"), koCol) + padStart(label("Risk"), koCol)
      : "") +
    padStart(label("Progress"), 9);
}

/** What the player actually committed, once the round has revealed it. */
export interface PlayedMove {
  index: number;
  pillz: number;
  fury: boolean;
  /** Win chance the solver gave this exact bet. */
  percent?: number;
  /** Win chance of the solver's best bet, for the gap. */
  best?: number;
  /** How many candidates scored strictly better - 0 means it matched the best. */
  better?: number;
  /** Candidates the solver had scored when this was played. */
  scored?: number;
  /** Set instead of the numbers when the solver had no verdict on this bet. */
  unevaluated?: string;
  /** The search had not finished when this was graded, so the numbers are provisional. */
  partial?: boolean;
}

export type ConnectionState = "starting" | "connected" | "offline";

export interface ViewExtras {
  /** Short contextual detail; connection and battle identity have dedicated fields. */
  status?: string;
  connection?: ConnectionState;
  battleId?: number;
  played?: PlayedMove;
  /** How an earlier round went, kept on screen after the position has moved on. */
  history?: { round: number; card: string; move: PlayedMove }[];
  /** Show the live auto-queue control in its current state. */
  autoQueue?: boolean;
  /** The pointer is currently over the auto-queue control. */
  autoQueueHover?: boolean;
  /** Interactive hypothesis used when answering an opponent's committed card. */
  opponentRead?: OpponentReadState;
  /** Live state after the displayed search has ceased to be the current decision. */
  phase?: ViewPhase;
  /** Direct board override, primarily for deterministic visual previews. */
  board?: ViewBoard;
  top?: number;
  /** Override the detected terminal size; for tests. */
  size?: { columns: number; rows: number };
}

export interface OpponentReadTarget {
  key: string;
  /** One-based terminal coordinates, inclusive. */
  column: number;
  endColumn: number;
  row: number;
}

export interface OpponentReadState {
  /** `${pillz} ${fury}`; undefined defaults to the opponent's plain all-in. */
  selected?: string;
  hovered?: string;
  /** Rebuilt by render for the live mouse handler. */
  targets?: OpponentReadTarget[];
}

export const opponentReadKey = (pillz: number, fury: boolean) =>
  `${pillz} ${fury}`;

export function opponentReadClick(
  targets: readonly OpponentReadTarget[],
  column: number,
  row: number,
): string | undefined {
  return targets.find((target) =>
    target.row === row && column >= target.column &&
    column <= target.endColumn
  )?.key;
}

export interface ViewPhase {
  /** The round currently visible on the site, 1-based. */
  round: number;
  /** Short, prominent replacement for "you move first". */
  headline: string;
  /** Current server-equivalent resources after replaying every resolved round. */
  you: { life: number; pillz: number };
  them: { life: number; pillz: number };
  /** Right-hand header label, such as "round 1 resolved". */
  progress: string;
  /** Final result from our point of view; present only once the battle is over. */
  outcome?: ViewOutcome;
  /** Current full card state while the displayed search belongs to the previous decision. */
  board?: ViewBoard;
  /** Shared game-start resource scale retained while the advisor is waiting. */
  barFloor?: { life: number; pillz: number };
}

export type ViewOutcome = "win" | "lose" | "draw";
export type ResultStyle =
  | "classic"
  | "slant"
  | "solid"
  | "shadow"
  | "framed";
export const RESULT_STYLES: readonly ResultStyle[] = [
  "classic",
  "slant",
  "solid",
  "shadow",
  "framed",
];

export interface ViewBoard {
  you: Card[];
  them: Card[];
  /** Side currently choosing a card; omitted while resolving or after the battle. */
  turn?: "you" | "them";
  selectedYou?: number;
  selectedThem?: number;
  /** Transient remote mouse position; cyan/double, never treated as a committed play. */
  hoveredYou?: number;
  hoveredThem?: number;
  /** Card whose pillz chooser is open; magenta/double until submitted or cancelled. */
  choosingYou?: number;
  choosingThem?: number;
  /** Resources immediately before the most recently resolved round. */
  beforeLastRound?: {
    you: { life: number; pillz: number };
    them: { life: number; pillz: number };
  };
  /** Current selection or the most recently resolved round, shown between the hands. */
  battle?: ViewBattle;
}

export interface ViewBattleSide {
  card: string;
  /** Undefined while the opponent's committed bet is still hidden. */
  pillz?: number;
  fury?: boolean;
  /** Stack at the start of this round, used to distinguish an all-in bet. */
  availablePillz?: number;
  /** Undefined until the server has revealed the round result. */
  attack?: number;
}

export interface ViewBattle {
  round: number;
  you?: ViewBattleSide;
  them?: ViewBattleSide;
}

const AUTO_QUEUE_COL = 2; // one-based terminal coordinate, after the line's leading space
const AUTO_QUEUE_WIDTH = 18;
const autoQueueText = (enabled: boolean) =>
  `[ AUTO-QUEUE ${enabled ? "ON " : "OFF"} ]`;
const autoQueueButton = (enabled: boolean, hovered = false) => {
  const button = enabled
    ? bold(fg(GREEN, autoQueueText(true)))
    : label(autoQueueText(false));
  return hovered ? `${ESC}7m${button}${ESC}27m` : button;
};

function connectionBadge(state: ConnectionState | undefined) {
  if (state === "connected") {
    return bold(fg(B_GREEN, "●")) + " " + fg(GREEN, "LIVE");
  }
  if (state === "offline") {
    return bold(fg(B_RED, "●")) + " " + fg(RED, "OFFLINE");
  }
  if (state === "starting") return fg(YELLOW, "◌ STARTING");
  return "";
}

function footerLine(
  cols: number,
  options: {
    status?: string;
    connection?: ConnectionState;
    battleId?: number;
    autoQueue?: boolean;
    autoQueueHover?: boolean;
  },
) {
  const left = [
    options.autoQueue === undefined
      ? ""
      : autoQueueButton(options.autoQueue, options.autoQueueHover),
    connectionBadge(options.connection),
    options.status ? dim(options.status) : "",
  ].filter(Boolean).join("  ");
  const right = options.battleId === undefined
    ? ""
    : dim(`Battle ${options.battleId}`);
  if (!right) return " " + clip(left, Math.max(0, cols - 1));
  const leftRoom = Math.max(0, cols - width(right) - 2);
  const shownLeft = clip(left, leftRoom);
  const gap = Math.max(1, cols - 1 - width(shownLeft) - width(right));
  return " " + shownLeft + " ".repeat(gap) + right;
}

/** Whether an SGR mouse event landed on the fixed-width control on the terminal's last row. */
export function autoQueueClick(
  column: number,
  row: number,
  size = consoleSize(),
): boolean {
  return row === size.rows && column >= AUTO_QUEUE_COL &&
    column < AUTO_QUEUE_COL + AUTO_QUEUE_WIDTH;
}

/**
 * Progress, and the rule under the header at the same time. Once complete it becomes a
 * quiet line rather than a solid bar: "solved in 869ms" already says it is finished, and a
 * full-width block of colour draws the eye away from the ranking underneath.
 */
/** Rows the played log always occupies: this round plus the three the advisor keeps. */
const PLAYED_SLOTS = 4;

function bar(done: number, total: number, cells: number) {
  if (cells <= 0) return "";
  if (total === 0) return dim("-".repeat(cells));
  if (done >= total) return dim("-".repeat(cells));
  const filled = Math.round((done / total) * cells);
  return fg(GREEN, "#".repeat(filled)) +
    dim("-".repeat(Math.max(0, cells - filled)));
}

function duration(ms: number) {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const m = Math.floor(ms / 60_000);
  return `${m}m${String(Math.round((ms % 60_000) / 1000)).padStart(2, "0")}s`;
}

/**
 * Fit `lines` to the terminal: clip each to the width, drop any past the last row. This is
 * what guarantees the redraw stays aligned, whatever the layout above computed.
 */
function frame(lines: string[], cols: number, rows: number): string {
  const out: string[] = [];
  for (const line of lines) {
    if (out.length >= rows) break;
    out.push(clip(line, cols) + CLEAR_LINE);
  }
  // No trailing newline: writing one on the last available row scrolls the screen up, and
  // then the next frame's cursor-home lands one line off and the whole thing walks.
  return out.join("\n");
}

export function render(
  game: Game,
  search: Search,
  extras: ViewExtras = {},
): string {
  const { columns: cols, rows } = extras.size ?? consoleSize();
  if (extras.opponentRead?.targets !== undefined) {
    extras.opponentRead.targets.length = 0;
  }
  const us = search.us === Turn.PLAYER_1 ? game.p1 : game.p2;
  const them = search.us === Turn.PLAYER_1 ? game.p2 : game.p1;
  const ourHand = search.us === Turn.PLAYER_1 ? game.h1 : game.h2;
  const theirHand = search.us === Turn.PLAYER_1 ? game.h2 : game.h1;
  const ourMove = search.us === Turn.PLAYER_1 ? game.i1 : game.i2;
  const theirMove = search.us === Turn.PLAYER_1 ? game.i2 : game.i1;
  const ourSelection = ourMove?.[0];
  const theirSelection = theirMove?.[0];
  const currentBattle: ViewBattle | undefined = ourMove || theirMove
    ? {
      round: search.round,
      you: ourMove
        ? {
          card: ourHand[ourMove[0]].name,
          pillz: ourMove[1],
          fury: ourMove[2],
          availablePillz: us.pillz,
        }
        : undefined,
      them: theirMove
        ? {
          card: theirHand[theirMove[0]].name,
          // SECOND mode applies a zero-pillz placeholder for a bet the site still hides.
          pillz: search.mode === SearchMode.SECOND ? undefined : theirMove[1],
          fury: search.mode === SearchMode.SECOND ? undefined : theirMove[2],
          availablePillz: them.pillz,
        }
        : undefined,
    }
    : undefined;
  const board: ViewBoard = extras.phase?.board ?? extras.board ?? {
    you: Array.from(ourHand),
    them: Array.from(theirHand),
    turn: game.isPlaying ? game.turn === search.us ? "you" : "them" : undefined,
    selectedYou: ourSelection,
    selectedThem: theirSelection,
    battle: currentBattle,
  };
  const stats = search.stats;
  const done = search.done;

  // ---- header -------------------------------------------------------------------------
  // Deliberately light: one line and an optional rule that doubles as the progress bar.
  // Both sit between the complete hand display and the recommendations.
  const oppCard = search.oppIndex !== undefined
    ? theirHand[search.oppIndex]
    : undefined;
  const headline = extras.phase?.headline ??
    (search.mode === SearchMode.SECOND
      ? `answering ${bold(oppCard?.name ?? "")}`
      : search.mode === SearchMode.BLIND_SECOND
      ? "opponent choosing  ·  initial estimate"
      : search.openingEstimate
      ? "you move first  ·  fast opening estimate"
      : "you move first");
  const rate = stats.ms > 0 ? (stats.unitsDone / (stats.ms / 1000)) : 0;
  const eta = rate > 0 && !done
    ? duration(((stats.units - stats.unitsDone) / rate) * 1000)
    : "";
  const workers = search.workerCount > 1 ? `${search.workerCount}w ` : "";
  const progress = extras.phase !== undefined
    ? fg(CYAN, extras.phase.progress)
    : done
    ? fg(
      GREEN,
      `${search.openingEstimate ? "estimated" : "solved"} in ${
        duration(stats.ms)
      }`,
    )
    : `${workers}${stats.unitsDone}/${stats.units}` +
      dim(eta ? ` eta ${eta}` : "");

  const head: string[] = [];
  const title = ` ${bold("UR ADVISOR")} ${label("│")} ${
    bold(fg(CYAN, `ROUND ${extras.phase?.round ?? search.round}/4`))
  } ${label("│")} ${headline}`;
  head.push(title + padStart(progress, Math.max(1, cols - width(title) - 1)));
  const shownUs = extras.phase?.you ?? us;
  const shownThem = extras.phase?.them ?? them;
  // This is both search progress and the permanent visual divider under the title.
  head.push(" " + bar(stats.unitsDone, stats.units, cols - 2));
  if (rows > 60) head.push("");

  const hasStatus = extras.status !== undefined ||
    extras.connection !== undefined || extras.battleId !== undefined ||
    extras.autoQueue !== undefined;
  const desiredTop = extras.top ?? (search.openingEstimate ? 10 : 8);
  // Four rows below the main table are permanently reserved for the defensive shortlist.
  // The optional battle panel disappears when space is tight; it must never shrink either
  // recommendation table into a different shape.
  const availableTop = rows - head.length - 4 - (hasStatus ? 1 : 0) - 1;
  const topN = Math.max(1, Math.min(desiredTop, availableTop));

  // ---- the ranking, first, because it is what gets read --------------------------------
  const ranked = displayRanked(search);
  const table: string[] = [];
  // Wide enough for "Noon Steevens 12 pillz + Fury" where the terminal allows it. The
  // numeric columns take 35 cells with the knockout pair and 23 without.
  const nameW = Math.max(14, Math.min(34, cols - (cols >= 78 ? 35 : 23)));
  const koCol = cols >= 78 ? 6 : 0;

  const recommendationRow = (
    c: Candidate,
    rank: number,
  ) => {
    const settled = search.settled(c);
    const share = (n: number) => `${Math.round(n * 100)}%`;
    // Certain and merely possible should not look alike: a bet that always finishes it
    // is bold and bright, one that only sometimes does is plain.
    const koS = search.koShare(c), riskS = search.koedShare(c);
    const ko = c.kos === 0
      ? dim("-")
      : koS >= 1
      ? bold(fg(B_GREEN, share(koS)))
      : fg(GREEN, share(koS));
    const risk = c.koed === 0
      ? bold(fg(B_GREEN, "-"))
      : riskS >= 1
      ? bold(fg(B_RED, share(riskS)))
      : fg(RED, share(riskS));

    // Only `win` is provisional. `worst` can only fall as more of the opponent's options
    // are tried, and a knockout already seen is a fact about a line that exists - so
    // those are shown in full even while the search runs, which is when they matter most.
    const winPct = search.openingEstimate
      ? pct(search.percent(c.average))
      : `${search.shownPercent(c.average)}`;
    const shownScore = search.openingEstimate ? winPct : `${winPct}%`;
    const win = settled
      ? fg(heat(search.percent(c.average)), shownScore)
      : dim(`~${shownScore}`);
    // A full subtree evaluates to exactly win, draw or loss, while the shallow opening
    // estimate is continuous and therefore shows its observed floor-to-ceiling range.
    const worstPct = search.percent(c.minimax);
    const worst = search.openingEstimate
      ? (() => {
        const ceiling = search.percent(search.ceiling(c));
        return fg(heat(worstPct), pct(worstPct)) + dim("–") +
          fg(heat(ceiling), pct(ceiling));
      })()
      : worstPct > 50
      ? fg(GREEN, "Win")
      : worstPct < 50
      ? fg(RED, "Lose")
      : fg(YELLOW, "Draw");
    return (
      // Clipped as well as padded: padEnd alone leaves a long label to push every column
      // right of it out of line with its heading.
      padEnd(
        clip(
          ` ${dim(`${rank}.`)} ${moveLabel(ourHand, c, nameW - 20, us.pillz)}`,
          nameW,
        ),
        nameW,
      ) +
      padStart(win, 6) +
      padStart(worst, 8) +
      (koCol ? padStart(ko, koCol) + padStart(risk, koCol) : "") +
      padStart(
        settled ? fg(GREEN, "Done") : `${c.done}/${search.samples}`,
        9,
      )
    );
  };

  {
    // The header's name field is exactly as wide as the rows', indent included, or every
    // column below it sits two cells left of its own heading.
    const title = extras.phase
      ? "Previous advice"
      : search.openingEstimate
      ? "Opening estimates"
      : search.mode === SearchMode.BLIND_SECOND
      ? "Initial estimates"
      : "Best bets";
    table.push(recommendationHeading(cols, title, search.openingEstimate));

    const body: string[] = [];
    for (const [i, c] of ranked.slice(0, topN).entries()) {
      body.push(recommendationRow(c, i + 1));
    }
    if (body.length === 0) body.push("  " + dim("no moves evaluated yet..."));
    // Always `topN` rows, blank ones included: the ranking fills in as the search runs, and
    // without this everything under it walks down the screen a line at a time.
    while (body.length < topN) body.push("");
    table.push(...body);
  }

  // A high average can hide the safest useful options, but risk is only trustworthy once
  // every opponent reply has been searched. Do not flash a defensive shortlist midway
  // through a search, and do not repeat it when the main table's top five already contains
  // a useful zero-risk move. The four blank rows remain allocated so the board never jumps
  // when the need appears or disappears.
  const safeTable: string[] = [];
  const provenSafe = displaySafeRanked(search);
  const shownSafe = provenSafe.slice(0, 3);
  const topFiveKeys = new Set(
    ranked.slice(0, 5).map((candidate) => candidate.key),
  );
  const topFiveHasSafe = provenSafe.some((candidate) =>
    topFiveKeys.has(candidate.key)
  );
  const needsSafe = search.done && !topFiveHasSafe && shownSafe.length > 0;
  if (needsSafe) {
    safeTable.push(
      recommendationHeading(
        cols,
        "─ Safe bets (0% Risk)",
        search.openingEstimate,
      ),
    );
    safeTable.push(
      ...shownSafe.map((candidate, index) =>
        recommendationRow(candidate, index + 1)
      ),
    );
  }
  while (safeTable.length < 4) safeTable.push("");

  // ---- the matrix, second, as background -----------------------------------------------
  const bets = [...new Set(search.candidates.map((c) => c.pillz))].sort((
    a,
    b,
  ) => a - b);
  const byMove = new Map(search.candidates.map((c) => [c.key, c]));
  const best = search.best();

  const budget = cols - 1;
  let mNameCol = 18, statCol = 8, cell = 6;
  const total = () => mNameCol + statCol + bets.length * cell;
  while (total() > budget && cell > 4) cell--;
  while (total() > budget && mNameCol > 11) mNameCol--;
  while (total() > budget && statCol > 0) statCol--;

  // Two blank rows, not one: the grid is a separate thing from the ranking above it and
  // wants a clear gap rather than looking like more of the same table.
  const matrix: string[] = ["", ""];
  matrix.push(
    padEnd(
      label(
        search.openingEstimate
          ? "  Opening score by pillz"
          : "  Win % by pillz",
      ),
      Math.max(0, mNameCol + statCol),
    ) +
      bets.map((p) => padStart(label(String(p)), cell)).join(""),
  );
  for (const index of [...new Set(search.candidates.map((c) => c.index))]) {
    const card = ourHand[index];
    const marker = best !== undefined && best.index === index
      ? fg(GREEN, ">")
      : " ";
    for (const fury of [false, true]) {
      const row = bets.map((p) => byMove.get(`${index} ${p} ${fury}`));
      if (row.every((c) => c === undefined)) continue;
      const label = fury
        ? "    " + fg(RED, "+ Fury")
        : ` ${marker} ${bold(card.name.slice(0, Math.max(3, mNameCol - 4)))}`;
      const stat = fury || statCol === 0
        ? ""
        : dim(`${card.power.final}/${card.damage.final}`);
      const rowBest = Math.max(
        ...row.map((
          c,
        ) => (c && !Number.isNaN(c.average) ? search.percent(c.average) : -1)),
      );
      matrix.push(
        padEnd(label, mNameCol) + padEnd(stat, statCol) +
          row.map((c) => padStart(formatCell(search, c, rowBest), cell)).join(
            "",
          ),
      );
    }
  }
  const readPanel = search.mode === SearchMode.SECOND
    ? opponentReadPanel(
      search,
      ourHand,
      oppCard?.name ?? "opponent card",
      us.pillz,
      them.pillz,
      cols,
      extras.opponentRead,
    )
    : undefined;
  const decisionPanel = readPanel?.lines ?? matrix;
  // ---- what was played, and the status line ---------------------------------------------
  const playedFooter: string[] = [];
  const played: {
    round?: number;
    card: string;
    move: PlayedMove;
    now: boolean;
  }[] = [];
  if (extras.played !== undefined) {
    played.push({
      card: ourHand[extras.played.index]?.name ?? `card ${extras.played.index}`,
      move: extras.played,
      now: true,
    });
  }
  for (const h of extras.history ?? []) {
    played.push({ round: h.round, card: h.card, move: h.move, now: false });
  }

  {
    // Same column grid as the ranking above, so the two read as one page rather than a
    // table followed by a list of loose sentences. Always drawn, always the same height:
    // the log grows a row per round, and if it were sized to fit its heading would climb
    // the screen as the game went on.
    playedFooter.push("");
    playedFooter.push(
      padEnd(label("  Played"), nameW) + padStart(label("Win"), 6) + "  " +
        label("Verdict"),
    );
    const rowsOut: string[] = [];
    for (const p of played) {
      const round = p.round ?? search.round;
      const bet =
        `${p.card} ${
          p.move.pillz === 0 ? label("0 pillz") : `${p.move.pillz} pillz`
        }` +
        (p.move.fury ? fg(RED, " + Fury") : "");
      const shown = p.now ? bold(bet) : bet;
      const win = p.move.percent === undefined
        ? dim("-")
        : fg(heat(p.move.percent), `${pct(p.move.percent)}%`);
      rowsOut.push(
        padEnd(clip(` ${dim(`r${round}`)} ${shown}`, nameW), nameW) +
          padStart(win, 6) + "  " + verdict(p.move),
      );
    }
    if (rowsOut.length === 0) rowsOut.push("  " + dim("nothing played yet"));
    while (rowsOut.length < PLAYED_SLOTS) rowsOut.push("");
    playedFooter.push(...rowsOut.slice(0, PLAYED_SLOTS));
  }
  const statusFooter: string[] = [];
  if (hasStatus) {
    statusFooter.push(footerLine(cols, extras));
  }

  // The complete original board is the visual anchor at the top of the screen. It is
  // admitted only when both dimensions fit; there is no partial/compact board.
  const completeBoard = cols >= FULL_BOARD_COLUMNS
    ? battleBoard(
      board,
      shownUs,
      shownThem,
      {
        // Both sides share one visual scale even if a mode starts them asymmetrically.
        life: Math.max(us.baseLife, them.baseLife),
        pillz: Math.max(us.basePillz, them.basePillz),
      },
      cols,
    )
    : [];
  const boardRoom = rows - head.length - table.length - safeTable.length -
    statusFooter.length;
  const cards = completeBoard.length > 0 && boardRoom >= completeBoard.length
    ? completeBoard
    : [];
  const room = rows - head.length - table.length - safeTable.length -
    cards.length -
    statusFooter.length;
  const shown: string[] = [];
  // The conditional read panel replaces the aggregate grid after the opponent commits a
  // card. It needs enough room to include at least one actual response; a clipped set of
  // buttons with no answer underneath would look interactive while being useless.
  const panelMinimum = readPanel === undefined ? 4 : 8;
  if (room >= panelMinimum) {
    shown.push(...decisionPanel.slice(0, room));
  }
  if (readPanel !== undefined && extras.opponentRead?.targets !== undefined) {
    const rowOffset = cards.length + head.length + table.length +
      safeTable.length;
    for (const target of readPanel.targets) {
      if (target.row >= shown.length) continue;
      extras.opponentRead.targets.push({
        ...target,
        row: rowOffset + target.row + 1,
      });
    }
  }
  const logRoom = room - shown.length;
  if (
    shown.length === decisionPanel.length && logRoom >= playedFooter.length
  ) {
    shown.push(...playedFooter);
  }
  const filler = new Array(Math.max(0, room - shown.length)).fill("");

  return frame(
    [
      ...cards,
      ...head,
      ...table,
      ...safeTable,
      ...shown,
      ...filler,
      ...statusFooter,
    ],
    cols,
    rows,
  );
}

/** How a played bet compares with what the solver had at the time. */
function verdict(p: PlayedMove) {
  if (p.unevaluated !== undefined) return dim(p.unevaluated);
  const caveat = p.partial ? dim(" (partial)") : "";
  if (p.better === 0) return fg(B_GREEN, "best move") + caveat;
  return dim(`${p.better} better, best ${pct(p.best ?? 0)}%`) + caveat;
}

function formatCell(search: Search, c: Candidate | undefined, rowBest: number) {
  if (c === undefined) return dim("."); // that bet cannot afford fury
  if (Number.isNaN(c.average)) return dim("-"); // not sampled yet
  const p = search.percent(c.average);
  const text = search.openingEstimate ? pct(p) : `${shownPercent(p)}`;
  // Dim anything still being sampled, so a settled number is visibly different.
  if (c.done < search.samples) return dim(text);
  return p === rowBest ? bold(fg(heat(p), text)) : fg(heat(p), text);
}

/**
 * The bet, never the total spend. `c.pillz` excludes the three pillz fury costs, which is
 * how the site's own slider reads and how the matrix columns and the played-move line are
 * labelled. Printing `moveCost()` here rendered a 4-pillz fury bet as "7pz+f", which reads
 * as seven pillz *and* fury - ten in total, and not even legal on a stack of eight.
 */
function moveLabel(
  hand: { [i: number]: { name: string } },
  c: Candidate,
  room: number,
  availablePillz: number,
) {
  const name = hand[c.index]?.name ?? `card ${c.index}`;
  // An all-in Fury bet leaves three fewer pillz on the slider but costs the same whole
  // stack. Give both forms the same warning colour so neither reads like an ordinary bet.
  const pillzColour = betPillzColour(c.pillz, c.fury, availablePillz);
  return `${bold(name.slice(0, Math.max(3, room)))} ${
    pillzLabel(c.pillz, pillzColour)
  }` +
    (c.fury ? fg(RED, " + Fury") : "");
}

/** The complete 128-column hand canvas, with a little room around the central UI. */
const FULL_BOARD_COLUMNS = 131;

const BOARD_CENTRE_WIDTH = 7;

function centredCell(value: string, cellWidth = BOARD_CENTRE_WIDTH) {
  const gap = Math.max(0, cellWidth - width(value));
  // Bias an indivisible half-cell to the left of the content. This puts one-digit values
  // directly under the middle letter and two-digit values visually across the centre.
  return " ".repeat(Math.ceil(gap / 2)) + value +
    " ".repeat(Math.floor(gap / 2));
}

function playerPanelLine(
  who: "YOU" | "OPP",
  player: { life: number; pillz: number },
  cols: number,
  before?: { life: number; pillz: number },
  active = false,
  barFloor = { life: 12, pillz: 12 },
) {
  const life = Math.max(0, player.life);
  const pillz = Math.max(0, player.pillz);
  const resourceBar = (
    value: number,
    previous: number | undefined,
    minimum: number,
    marker: string,
    colour: number,
    spentColour: number,
  ) => {
    const old = previous === undefined ? value : Math.max(0, previous);
    const kept = Math.min(value, old);
    const gained = Math.max(0, value - old);
    const spent = Math.max(0, old - value);
    const length = Math.max(12, minimum, value, old);
    const empty = Math.max(0, length - kept - gained - spent);
    return fg(colour, "[") +
      bold(fg(colour, marker.repeat(kept))) +
      (gained ? bold(fg(B_GREEN, marker.repeat(gained))) : "") +
      (spent ? bold(fg(spentColour, "-".repeat(spent))) : "") +
      label("-".repeat(empty)) + fg(colour, "]");
  };
  const lifeBar = resourceBar(
    life,
    before?.life,
    barFloor.life,
    "♥",
    RED,
    B_RED,
  );
  const pillBar = resourceBar(
    pillz,
    before?.pillz,
    barFloor.pillz,
    "O",
    BLUE,
    YELLOW,
  );
  const left = `${lifeBar} ${fg(RED, "Life")} ${
    padStart(fg(RED, String(life)), 2)
  }`;
  const right = `${padEnd(fg(BLUE, String(pillz)), 2)} ${
    fg(BLUE, "Pillz")
  } ${pillBar}`;
  const centreStart = Math.max(
    0,
    Math.floor((cols - BOARD_CENTRE_WIDTH) / 2),
  );
  const leftRoom = Math.max(0, centreStart - 2);
  const rightRoom = Math.max(
    0,
    cols - centreStart - BOARD_CENTRE_WIDTH - 2,
  );
  const playerLabel = active
    ? bold(fg(YELLOW, `▶ ${who} ◀`))
    : bold(fg(CYAN, who));
  return padStart(clip(left, leftRoom), leftRoom) + "  " +
    centredCell(playerLabel) + "  " + clip(right, rightRoom);
}

function battleDetails(side: ViewBattleSide | undefined) {
  if (side === undefined) return dim("no card selected");
  const pillz = side.pillz === undefined ? "" : ` ${
    pillzLabel(
      side.pillz,
      betPillzColour(side.pillz, side.fury ?? false, side.availablePillz),
    )
  }`;
  const fury = side.fury ? fg(RED, " + Fury") : "";
  return `${bold(side.card)}${pillz}${fury}`;
}

function battleAttackValue(
  side: ViewBattleSide | undefined,
  colour?: number,
) {
  if (side?.attack === undefined) return "";
  const value = String(side.attack);
  return bold(colour === undefined ? value : fg(colour, value));
}

/**
 * Complete original two-hand display, with the player/pillz lines that surrounded those
 * canvases and a fixed centre strip for the current or most recently revealed attacks.
 */
function battleBoard(
  board: ViewBoard,
  you: { life: number; pillz: number },
  them: { life: number; pillz: number },
  barFloor: { life: number; pillz: number },
  cols: number,
): string[] {
  const centre = (lines: string[]) =>
    lines.map((line) =>
      " ".repeat(Math.max(0, Math.floor((cols - width(line)) / 2))) + line
    );
  const theirHand = centre(GameRenderer.handLines(
    board.them,
    board.selectedThem === undefined ? "white" : "cyan",
    board.selectedThem,
    false,
    board.hoveredThem,
    board.choosingThem,
  ));
  const ourHand = centre(GameRenderer.handLines(
    board.you,
    board.selectedYou === undefined ? "white" : "cyan",
    board.selectedYou,
    false,
    board.hoveredYou,
    board.choosingYou,
  ));
  const battle = board.battle;
  let theirColour: number | undefined;
  let ourColour: number | undefined;
  if (battle?.them?.attack !== undefined && battle.you?.attack !== undefined) {
    if (battle.them.attack === battle.you.attack) {
      theirColour = ourColour = YELLOW;
    } else if (battle.them.attack > battle.you.attack) {
      theirColour = GREEN;
      ourColour = RED;
    } else {
      theirColour = RED;
      ourColour = GREEN;
    }
  }
  const centreStart = Math.max(
    0,
    Math.floor((cols - BOARD_CENTRE_WIDTH) / 2),
  );
  const theirAttack = centredCell(
    battleAttackValue(battle?.them, theirColour),
  );
  const ourAttack = centredCell(
    battleAttackValue(battle?.you, ourColour),
  );
  const detailRoom = Math.max(0, centreStart - 2);
  const attackLine = (details: string, attack: string) =>
    padStart(clip(details, detailRoom), detailRoom) + "  " + attack + "  " +
    label("Attack");
  const theirLine = attackLine(battleDetails(battle?.them), theirAttack);
  const ourLine = attackLine(battleDetails(battle?.you), ourAttack);

  return [
    ...theirHand,
    playerPanelLine(
      "OPP",
      them,
      cols,
      board.beforeLastRound?.them,
      board.turn === "them",
      barFloor,
    ),
    "",
    theirLine,
    ourLine,
    "",
    playerPanelLine(
      "YOU",
      you,
      cols,
      board.beforeLastRound?.you,
      board.turn === "you",
      barFloor,
    ),
    ...ourHand,
  ];
}

const RESULT_GLYPHS: Record<string, string[]> = {
  " ": ["     ", "     ", "     ", "     ", "     "],
  A: [" ███ ", "█   █", "█████", "█   █", "█   █"],
  D: ["████ ", "█   █", "█   █", "█   █", "████ "],
  E: ["█████", "█    ", "████ ", "█    ", "█████"],
  I: ["█████", "  █  ", "  █  ", "  █  ", "█████"],
  L: ["█    ", "█    ", "█    ", "█    ", "█████"],
  N: ["█   █", "██  █", "█ █ █", "█  ██", "█   █"],
  O: [" ███ ", "█   █", "█   █", "█   █", " ███ "],
  R: ["████ ", "█   █", "████ ", "█  █ ", "█   █"],
  S: [" ████", "█    ", " ███ ", "    █", "████ "],
  U: ["█   █", "█   █", "█   █", "█   █", " ███ "],
  W: ["█   █", "█   █", "█ █ █", "██ ██", "█   █"],
  Y: ["█   █", " █ █ ", "  █  ", "  █  ", "  █  "],
};

/** A variable-width FIGlet-like alphabet for the words used by the result screen. */
const RESULT_ASCII_GLYPHS: Record<string, string[]> = {
  " ": ["    ", "    ", "    ", "    ", "    ", "    "],
  A: [
    "   /\\   ",
    "  /  \\  ",
    " / /\\ \\ ",
    "/ ____ \\",
    "| |  | |",
    "|_|  |_|",
  ],
  D: [
    " _____  ",
    "|  __ \\ ",
    "| |  | |",
    "| |  | |",
    "| |__| |",
    "|_____/ ",
  ],
  E: [
    " ______ ",
    "|  ____|",
    "| |__   ",
    "|  __|  ",
    "| |____ ",
    "|______|",
  ],
  I: [
    " _____ ",
    "|_   _|",
    "  | |  ",
    "  | |  ",
    " _| |_ ",
    "|_____|",
  ],
  L: [
    " _      ",
    "| |     ",
    "| |     ",
    "| |     ",
    "| |____ ",
    "|______|",
  ],
  N: [
    " _   _ ",
    "| \\ | |",
    "|  \\| |",
    "| . ' |",
    "| |\\  |",
    "|_| \\_|",
  ],
  O: [
    "  ____  ",
    " / __ \\ ",
    "| |  | |",
    "| |  | |",
    "| |__| |",
    " \\____/ ",
  ],
  R: [
    " _____  ",
    "|  __ \\ ",
    "| |__) |",
    "|  _  / ",
    "| | \\ \\ ",
    "|_|  \\_\\",
  ],
  S: [
    "  _____ ",
    " / ____|",
    "| (___  ",
    " \\___ \\ ",
    " ____) |",
    "|_____/ ",
  ],
  U: [
    " _    _ ",
    "| |  | |",
    "| |  | |",
    "| |  | |",
    "| |__| |",
    " \\____/ ",
  ],
  W: [
    "__          __",
    "\\ \\        / /",
    " \\ \\  /\\  / / ",
    "  \\ \\/  \\/ /  ",
    "   \\  /\\  /   ",
    "    \\/  \\/    ",
  ],
  Y: [
    "__     __",
    "\\ \\   / /",
    " \\ \\_/ / ",
    "  \\   /  ",
    "   | |   ",
    "   |_|   ",
  ],
};

function resultArt(message: string): string[] {
  return new Array(5).fill("").map((_, row) =>
    [...message].map((character) => RESULT_GLYPHS[character][row]).join(" ")
  );
}

function asciiResultArt(message: string): string[] {
  const glyphs = [...message].map((character) =>
    RESULT_ASCII_GLYPHS[character]
  );
  const widths = glyphs.map((glyph) =>
    Math.max(...glyph.map((line) => line.length))
  );
  return new Array(6).fill("").map((_, row) =>
    glyphs.map((glyph, index) => glyph[row].padEnd(widths[index])).join(" ")
  );
}

function shadowed(art: string[]): string[] {
  const columns = Math.max(...art.map((line) => line.length));
  return new Array(art.length + 1).fill("").map((_, row) => {
    let line = "";
    for (let column = 0; column <= columns; column++) {
      const solid = art[row]?.[column] === "█";
      const shadow = row > 0 && column > 0 &&
        art[row - 1]?.[column - 1] === "█";
      line += solid ? "█" : shadow ? "▓" : " ";
    }
    return line.trimEnd();
  });
}

function framed(art: string[]): string[] {
  const columns = Math.max(...art.map((line) => line.length));
  return [
    `╔${"═".repeat(columns + 2)}╗`,
    ...art.map((line) => `║ ${line.padEnd(columns)} ║`),
    `╚${"═".repeat(columns + 2)}╝`,
  ];
}

/** Large terminal-native result lettering, with selectable decoration. */
export function resultBanner(
  outcome: ViewOutcome,
  cols: number,
  style: ResultStyle = "classic",
): string[] {
  const message = outcome === "win"
    ? "YOU WIN"
    : outcome === "lose"
    ? "YOU LOSE"
    : "DRAW";
  const colour = outcome === "win"
    ? B_GREEN
    : outcome === "lose"
    ? B_RED
    : YELLOW;
  const complex = style === "classic" || style === "slant";
  const base = complex ? asciiResultArt(message) : resultArt(message);
  const art = style === "slant"
    ? base.map((line, row) => " ".repeat(base.length - row - 1) + line)
    : style === "shadow"
    ? shadowed(base)
    : style === "framed"
    ? framed(base)
    : base;
  const canvasWidth = Math.max(...art.map((line) => width(line)));
  return art.map((line) => {
    const decorated = complex
      ? bold(fg(colour, line.trimEnd()))
      : [...line].map((character) =>
        character === "█"
          ? bold(fg(colour, character))
          : character === "▓"
          ? dim(fg(colour, character))
          : "╔═╗║╚╝".includes(character)
          ? fg(colour, character)
          : character
      ).join("");
    return " ".repeat(Math.max(0, Math.floor((cols - canvasWidth) / 2))) +
      decorated;
  });
}

/** All result styles on one non-scrolling screen for choosing a preferred default. */
export function resultStylePreview(size = consoleSize()): string {
  const lines = [
    ` ${bold("RESULT BANNER STYLES")}  ${dim("use --result-style <name>")}`,
  ];
  const shown = size.rows >= 40
    ? RESULT_STYLES
    : RESULT_STYLES.filter((style) => style === "classic" || style === "slant");
  for (const style of shown) {
    lines.push(` ${bold(style.toUpperCase())}`);
    lines.push(...resultBanner("win", size.columns, style));
  }
  if (shown.length < RESULT_STYLES.length) {
    lines.push(` ${dim("Also available: solid, shadow, framed")}`);
  }
  lines.push(` ${dim("Press Enter to exit")}`);
  return frame(lines, size.columns, size.rows);
}

/** Holding screen for when there is nothing of ours to decide. */
export function idle(
  status: string,
  note: string,
  size = consoleSize(),
  autoQueue?: boolean,
  autoQueueHover = false,
  phase?: ViewPhase,
  resultStyle: ResultStyle = "classic",
  connection?: ConnectionState,
  battleId?: number,
): string {
  const title = phase
    ? ` ${bold("UR ADVISOR")} ${label("│")} ${
      bold(fg(CYAN, `ROUND ${phase.round}/4`))
    } ${label("│")} ${phase.headline}`
    : ` ${bold("UR ADVISOR")} ${label("│")} ${dim("idle")}`;
  const header = [
    title,
    " " + bar(0, 0, Math.max(0, size.columns - 2)),
  ];
  const waiting = phase && phase.headline !== "battle over";
  const result = phase?.outcome === undefined
    ? []
    : ["", ...resultBanner(phase.outcome, size.columns, resultStyle), ""];
  const advice = waiting
    ? [
      recommendationHeading(size.columns, "Best bets"),
      `  ${dim("waiting for the opponent's card before solving...")}`,
      ...new Array(7).fill(""),
    ]
    : [];
  // Match render()'s permanently allocated safety block so everything below the board
  // keeps its rows when the opponent's card arrives and solving begins.
  const safeTable = waiting ? new Array(4).fill("") : [];
  const completeBoard = phase?.board && size.columns >= FULL_BOARD_COLUMNS
    ? battleBoard(
      phase.board,
      phase.you,
      phase.them,
      phase.barFloor ?? {
        life: Math.max(12, phase.you.life, phase.them.life),
        pillz: Math.max(12, phase.you.pillz, phase.them.pillz),
      },
      size.columns,
    )
    : [];
  const hasFooter = Boolean(status) || connection !== undefined ||
    battleId !== undefined || autoQueue !== undefined;
  const tailRows = 2 + (hasFooter ? 1 : 0);
  const cards = completeBoard.length > 0 &&
      header.length + result.length + advice.length + safeTable.length +
            completeBoard.length + tailRows <= size.rows
    ? completeBoard
    : [];
  const lines = [
    ...cards,
    ...header,
    ...result,
    ...advice,
    ...safeTable,
    `  ${dim(phase?.headline === "battle over" ? "" : note)}`,
    "",
  ];
  if (hasFooter) {
    while (lines.length < size.rows - 1) lines.push("");
    if (lines.length > size.rows - 1) lines.length = Math.max(0, size.rows - 1);
    lines.push(footerLine(size.columns, {
      status,
      connection,
      battleId,
      autoQueue,
      autoQueueHover,
    }));
  }
  return frame(lines, size.columns, size.rows);
}

export function consoleSize(): { columns: number; rows: number } {
  try {
    return usableConsoleSize(Deno.consoleSize());
  } catch {
    return { columns: 80, rows: 24 }; // not a tty (piped output, or a worker)
  }
}

export function usableConsoleSize(size: { columns: number; rows: number }) {
  return {
    columns: size.columns > 20 ? size.columns : 80,
    rows: size.rows > 6 ? size.rows : 24,
  };
}
