// Root-split search: the same game tree Analysis.iterTree walks, but reorganised so that
// partial answers exist while it runs.
//
// iterTree is breadth-first over the whole tree, so nothing can be read out of it until it
// has finished - at round 2 that is ~5.2M states and about 48s of silence. The work here is
// cut into units instead, one per *depth-2* state (both players have moved, so the round has
// resolved), and each unit is handed to Policy.ts's information-aware continuation search,
// which walks the subtree with make/unmake and returns a single result. A caller can
// render a ranking that firms up continuously, cancel on the opponent's move, and never
// hold a subtree in memory at all. Deep.ts retains the original perfect-information
// minimax as a reference; Policy.ts prevents live advice from peeking at hidden pillz.
//
// Round one is an explicit exception: recursively solving the three remaining rounds is
// intractable even for four zero-pill probes, so all current-round pairings are evaluated
// with a cheap post-battle position score instead. The view labels that value as an opening
// score, never as a win percentage.
//
//   average   average over the opponent's current unknown choice. The shallow opening uses
//             an empirical prior from captured first-round bets; later rounds use a uniform
//             mean. Every later continuation is a conservative executable policy.
//   minimax   extremum over that same current choice: the visible Worst column.
//
// Both are reported so the mixed model is visible instead of blended. Values are held in
// P1's frame (+1 = P1 wins) and converted to the asking player's side on the way out.
import Game, { Winner } from "../game/Game.ts";
import { shiftRange } from "../utils/Utils.ts";
import policyValue, { newPolicyStack } from "./Policy.ts";
import { GameResult } from "./Minimax.ts";
import { Turn } from "../game/types/Types.ts";

/** A move in the solver's own `"index pillz fury"` notation. */
export interface Move {
  index: number;
  pillz: number;
  fury: boolean;
}

export const moveKey = (m: Move) => `${m.index} ${m.pillz} ${m.fury}`;
/** Pillz actually spent, fury included: what the tie-break in bestChild() compares. */
export const moveCost = (m: Move) => m.pillz + (m.fury ? 3 : 0);

/**
 * Whole-number percentages without inventing certainty at the display boundary. A raw
 * 99.6% used to print as 100%, which is a materially different claim; likewise 0.4% is not
 * zero. Exact endpoints retain their compact labels.
 */
export const shownPercent = (value: number) => {
  const rounded = Math.round(value);
  if (rounded >= 100 && value < 100) return 99;
  if (rounded <= 0 && value > 0) return 1;
  return rounded;
};

export interface Candidate extends Move {
  key: string;
  /** Values in P1's frame, one per opponent hypothesis folded in so far. */
  values: number[];
  /** Statistical weight of each value; round-one replies use the captured opening prior. */
  weights: number[];
  /** Mean of `values`, in P1's frame. NaN until the first unit lands. */
  average: number;
  /** Extremum of `values` from the opponent's side, in P1's frame. NaN until then. */
  minimax: number;
  /** Units done for this candidate, out of `samples`. */
  done: number;
  /**
   * Sampled lines where this bet ends the game outright *this round* by taking the
   * opponent to zero life. Deliberately not a win on life at the end of round 4: "would
   * this finish it now?" is a different question from "would this win eventually", and a
   * 100% win chance can be either.
   */
  kos: number;
  /**
   * Sampled lines where *you* are taken to zero life this round. The mirror of `kos`, and
   * the thing a high `average` hides: a bet can average 95% and still be the one that walks
   * into a one-shot knockout, because the average is over an opponent picking at random.
   */
  koed: number;
}

/**
 * Which player's unknown choice is being averaged over.
 *
 * FIRST  - we move first, so the samples are the opponent's replies to a fully visible
 *          move. They genuinely choose, so `minimax` is a real game-theoretic worst case.
 * SECOND - the opponent has already committed a card and we can see it, but the site hides
 *          their pillz until the round resolves. The samples are their possible bets, which
 *          they are no longer free to pick, so `minimax` is "worst case over what they may
 *          have bet" rather than a min over a choice.
 */
export enum SearchMode {
  FIRST = "first",
  SECOND = "second",
  /** They move first but have not selected a card yet; rank our replies provisionally. */
  BLIND_SECOND = "blind-second",
}

function* legalMoves(game: Game, indexes: number[]): Generator<Move> {
  const pillz = game.playingPlayer.pillz;
  // Same enumeration and same order as Analysis.iterTree: the all-in bet first, then the
  // biggest fury bet, so that the engine's cutoffs get their best shot early.
  for (const index of indexes) {
    for (const p of shiftRange(pillz)) {
      for (const fury of (p <= pillz - 3 ? [true, false] : [false])) {
        yield { index, pillz: p, fury };
      }
    }
  }
}

const terminalValue = (game: Game): number | undefined => {
  switch (game.winner) {
    case Winner.PLAYER_1:
      return GameResult.PLAYER_1_WIN;
    case Winner.PLAYER_2:
      return GameResult.PLAYER_2_WIN;
    case Winner.TIE:
      return GameResult.TIE;
    default:
      return undefined;
  }
};

/**
 * Cheap round-one leaf value in P1's frame. It deliberately describes the position after
 * this battle rather than pretending to know the eventual winner: life matters most,
 * pillz retain most of that value, and preserving stronger unplayed cards breaks otherwise
 * wasteful ties. `tanh` keeps the public score on Search's normal [-1, 1] scale.
 */
function openingPositionValue(game: Game): number {
  const handValue = (hand: Game["h1"]) => {
    let value = 0;
    for (const card of hand) {
      if (!card.played) value += card.power.final + card.damage.final;
    }
    return value;
  };
  const life = game.p1.life - game.p2.life;
  // Pillz have diminishing marginal value but a steep reserve value near zero. A linear
  // term made twelve-to-zero look only slightly dearer than six-to-zero and recommended
  // opening all-ins because it could not represent three future rounds without resources.
  const pillz = 4 * (Math.sqrt(game.p1.pillz) - Math.sqrt(game.p2.pillz));
  const cards = handValue(game.h1) - handValue(game.h2);
  return Math.tanh((life + pillz + 0.12 * cards) / 12);
}

// 198 captured round-one plays as of 2026-09-13. The previous uniform model treated an
// all-in as just as likely as a four-pill bet; in the actual sample, costs 9–11 occurred
// zero times and a twelve-pill spend occurred twice (1%), while the median was four.
// Add one Laplace observation to every legal action so an unseen play stays possible.
const OPENING_REPLY_COUNTS: Readonly<Record<string, number>> = {
  "0 false": 38,
  "1 false": 7,
  "2 false": 20,
  "3 false": 28,
  "4 false": 53,
  "4 true": 1,
  "5 false": 29,
  "6 false": 12,
  "7 false": 7,
  "8 false": 1,
  "9 true": 2,
};

const openingReplyWeight = (move: Move) =>
  (OPENING_REPLY_COUNTS[`${move.pillz} ${move.fury}`] ?? 0) + 1;

export interface SearchStats {
  units: number;
  unitsDone: number;
  /** Depth-2 states that were already decided, so no subtree had to be built. */
  terminal: number;
  ms: number;
}

export default class Search {
  readonly mode: SearchMode;
  /** The side the answer is for: the player whose turn it is. */
  readonly us: Turn;
  readonly round: number;
  /** Round one uses a fast post-battle position estimate instead of recursive minimax. */
  readonly openingEstimate: boolean;
  /** The card the opponent has already committed, in SECOND mode. */
  readonly oppIndex?: number;
  readonly candidates: Candidate[];
  /**
   * Opponent hypotheses per candidate - constant, since neither side's options depend on
   * the other's choice within a round. This counts them all, not just this slice's, so with
   * a stride above 1 a candidate's `done` only reaches `samples` once the slices are merged.
   */
  readonly samples: number;
  readonly units: number;

  private readonly root: Game;
  /** Outer loop: the move that gets applied first. */
  private readonly outer: Move[];
  /** Inner loop: the reply. */
  private readonly inner: Move[];
  private oi = 0;
  private ii = 0;
  /** `root` with `outer[oi]` applied, reused across the inner loop as iterTree does. */
  private outerGame?: Game;
  /** Reused across every unit, so exploring a subtree allocates no policy buffers. */
  private readonly policyStack = newPolicyStack();
  private startedAt = 0;
  private elapsed = 0;
  private terminalUnits = 0;
  private unitsDone = 0;

  /**
   * Units are independent, so a search can be split across processes by giving each one a
   * different `offset` of the same `stride` and merging the candidates afterwards. One
   * process is `stride: 1, offset: 0`.
   */
  readonly stride: number;
  readonly offset: number;
  /** Units this instance will actually evaluate, given its stride. */
  readonly ownUnits: number;

  /** Parallel implementations override this so the view can show what is actually active. */
  get workerCount() {
    return 1;
  }

  constructor(
    game: Game,
    stride = 1,
    offset = 0,
    blindSecond = false,
  ) {
    this.stride = stride;
    this.offset = offset;
    this.root = game.clone();
    this.round = this.root.round;
    this.openingEstimate = this.round === 1;

    if (this.root.firstHasSelected) {
      if (blindSecond) {
        throw new Error(
          "blind-second search cannot start after a card is selected",
        );
      }
      // The opponent has moved and we are answering: drop their selection so the tree can
      // re-enumerate their hidden pillz, exactly as iterTree does for this case.
      this.mode = SearchMode.SECOND;
      this.oppIndex = this.root.deselect(false)!;
      this.us = this.root.turn === Turn.PLAYER_1
        ? Turn.PLAYER_2
        : Turn.PLAYER_1;
      this.outer = [...legalMoves(this.root, [this.oppIndex])];
      this.inner = this.repliesTo(this.outer[0]);
    } else if (blindSecond) {
      // The opponent is still choosing. Treat every one of their cards and hidden bets as
      // a hypothesis, then group the results by our possible response. When their real card
      // arrives the advisor discards this provisional search and starts precise SECOND mode.
      this.mode = SearchMode.BLIND_SECOND;
      this.us = this.root.turn === Turn.PLAYER_1
        ? Turn.PLAYER_2
        : Turn.PLAYER_1;
      this.outer = [
        ...legalMoves(this.root, this.root.unplayedCardIndexes),
      ];
      this.inner = this.repliesTo(this.outer[0]);
    } else {
      this.mode = SearchMode.FIRST;
      this.us = this.root.turn;
      this.outer = [
        ...legalMoves(this.root, this.root.unplayedCardIndexes),
      ];
      this.inner = this.repliesTo(this.outer[0]);
    }

    const keys = this.mode === SearchMode.FIRST ? this.outer : this.inner;
    this.candidates = keys.map((m) => ({
      ...m,
      key: moveKey(m),
      values: [],
      weights: [],
      average: NaN,
      minimax: NaN,
      done: 0,
      kos: 0,
      koed: 0,
    }));
    this.samples = this.mode === SearchMode.FIRST
      ? this.inner.length
      : this.outer.length;
    this.units = this.outer.length * this.inner.length;
    this.ownUnits = Math.ceil((this.units - offset) / stride);
  }

  /** The reply set after `move`; the same for every `move`, so this is called once. */
  private repliesTo(move: Move): Move[] {
    const g = this.root.clone();
    g.select(move.index, move.pillz, move.fury, false);
    return [...legalMoves(g, g.unplayedCardIndexes)];
  }

  get done() {
    return this.oi >= this.outer.length;
  }

  get stats(): SearchStats {
    return {
      units: this.ownUnits,
      unitsDone: this.unitsDone,
      terminal: this.terminalUnits,
      ms: this.elapsed + (this.startedAt ? Date.now() - this.startedAt : 0),
    };
  }

  /**
   * Evaluate one depth-2 state. Returns false once every unit is done.
   *
   * In FIRST mode a candidate is only final when all its samples are in, so candidates
   * settle one at a time. In SECOND mode every candidate gains one sample per opponent
   * hypothesis, so the whole ranking refines together.
   */
  step(): boolean {
    if (this.done) return false;
    if (!this.startedAt) this.startedAt = Date.now();

    if (
      ((this.oi * this.inner.length) + this.ii - this.offset) % this.stride !==
        0
    ) {
      this.advance();
      return !this.done;
    }

    const outerMove = this.outer[this.oi];
    if (this.outerGame === undefined) {
      this.outerGame = this.root.clone();
      this.outerGame.select(
        outerMove.index,
        outerMove.pillz,
        outerMove.fury,
        false,
      );
    }

    const innerMove = this.inner[this.ii];
    const game = this.outerGame.clone();
    game.select(innerMove.index, innerMove.pillz, innerMove.fury, false);

    // The round has resolved. Either it decided the game, or minimax the rest of it.
    const terminal = terminalValue(game);
    let value: number;
    let koNow = false;
    let koedNow = false;
    if (terminal === undefined) {
      value = this.openingEstimate
        ? openingPositionValue(game)
        : policyValue(game, this.us, this.policyStack);
    } else {
      value = terminal;
      this.terminalUnits++;
      // Taken to zero, not merely ahead on life when the rounds run out - a knockout is a
      // different fact from a win, in both directions.
      const oppLife = this.us === Turn.PLAYER_1 ? game.p2.life : game.p1.life;
      const ourLife = this.us === Turn.PLAYER_1 ? game.p1.life : game.p2.life;
      const ourScore = this.us === Turn.PLAYER_1 ? value : -value;
      koNow = ourScore === 1 && oppLife <= 0;
      koedNow = ourLife <= 0; // counts a double knockout too: you still died
    }

    const candidate = this.mode === SearchMode.FIRST
      ? this.candidates[this.oi]
      : this.candidates[this.ii];
    candidate.values.push(value);
    candidate.weights.push(
      this.openingEstimate
        ? openingReplyWeight(
          this.mode === SearchMode.FIRST ? innerMove : outerMove,
        )
        : 1,
    );
    candidate.done++;
    if (koNow) candidate.kos++;
    if (koedNow) candidate.koed++;
    this.fold(candidate);

    this.unitsDone++;
    this.advance();
    return true;
  }

  /**
   * Give this search up to `ms` of compute time. Keeping this on the search abstraction
   * lets the advisor drive an in-process search and a worker pool through the same path.
   */
  workFor(ms: number): Promise<void> {
    const until = Date.now() + ms;
    while (Date.now() < until && this.step());
    return Promise.resolve();
  }

  private advance() {
    if (++this.ii >= this.inner.length) {
      this.ii = 0;
      this.oi++;
      this.outerGame = undefined;
      if (this.startedAt) {
        this.elapsed += Date.now() - this.startedAt;
        this.startedAt = 0;
      }
    }
  }

  /** Recompute a candidate's two summary numbers from the samples seen so far. */
  private fold(c: Candidate) {
    let total = 0, weight = 0;
    // The opponent is P2 exactly when we are P1, and P2 minimises in P1's frame.
    let extreme = this.us === Turn.PLAYER_1 ? Infinity : -Infinity;
    for (let i = 0; i < c.values.length; i++) {
      const v = c.values[i], w = c.weights[i] ?? 1;
      total += v * w;
      weight += w;
      if (this.us === Turn.PLAYER_1 ? v < extreme : v > extreme) extreme = v;
    }
    c.average = total / weight;
    c.minimax = extreme;
  }

  /** `value` (P1's frame) on the public 0..100 scale from our point of view. */
  percent(value: number) {
    const ours = this.us === Turn.PLAYER_1 ? value : -value;
    return ((ours + 1) / 2) * 100;
  }

  /** Public score rounded exactly as the TUI presents it. */
  shownPercent(value: number) {
    return shownPercent(this.percent(value));
  }

  /** Share of this candidate's sampled lines that end the game outright this round. */
  koShare(c: Candidate) {
    return c.done > 0 ? c.kos / c.done : 0;
  }

  /** Share of sampled lines where this bet gets *you* knocked out this round. */
  koedShare(c: Candidate) {
    return c.done > 0 ? c.koed / c.done : 0;
  }

  /** True once every opponent choice has been folded into this candidate. */
  settled(c: Candidate) {
    return c.done >= this.samples;
  }

  /** Best sampled response for us, in P1's frame; the counterpart to `minimax`. */
  ceiling(c: Candidate) {
    let extreme = this.us === Turn.PLAYER_1 ? -Infinity : Infinity;
    for (const value of c.values) {
      if (this.us === Turn.PLAYER_1 ? value > extreme : value < extreme) {
        extreme = value;
      }
    }
    return extreme;
  }

  /**
   * Candidates best first: by win chance, worst case, knockout, then the cheaper bet.
   *
   * Win chance still dominates, but two moves displaying the same chance are ordered by
   * the same guarantee shown in the Worst column. Only once their displayed chance and
   * worst case agree do we favour a move that ends the game *now*. Opening estimates show
   * a continuous Range rather than a game-theoretic Worst, so they retain the KO tie-break.
   */
  ranked(): Candidate[] {
    // Compared as *displayed*, to a whole percent. Sorting on the raw mean put rows in an
    // order the screen could not explain - two bets both reading 90% with the higher one
    // above for a difference of half a point, which is far inside the error of either the
    // captured opening prior or the later uniform-opponent model. Equal on screen now means
    // equal here, so the tie-breaks below are what decides, and they are all visible.
    const shown = (c: Candidate) =>
      Number.isNaN(c.average) ? -Infinity : this.shownPercent(c.average);
    const worst = (c: Candidate) =>
      Number.isNaN(c.minimax) ? -Infinity : this.shownPercent(c.minimax);

    return [...this.candidates].sort((a, b) => {
      const av = shown(a), bv = shown(b);
      if (av !== bv) return bv - av;
      // In a full search the visible floor is categorical: Win, Draw or Lose. A Draw
      // guarantee must beat a possible loss when the displayed average is identical.
      if (!this.openingEstimate) {
        const aw = worst(a), bw = worst(b);
        if (aw !== bw) return bw - aw;
      }
      const ak = this.koShare(a), bk = this.koShare(b);
      if (ak !== bk) return bk - ak;
      // Then the one least likely to get you knocked out on the spot. Equal win rates are
      // equal *on average*; they are not equally survivable, and a line that ends you now
      // does not care that the average said 90%.
      const ar = this.koedShare(a), br = this.koedShare(b);
      if (ar !== br) return ar - br;
      return moveCost(a) - moveCost(b);
    });
  }

  best(): Candidate | undefined {
    const [top] = this.ranked();
    return top !== undefined && Number.isNaN(top.average) ? undefined : top;
  }
}
