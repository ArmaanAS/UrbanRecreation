// Information-aware continuation value for the live advisor.
//
// Deep.ts is ordinary perfect-information minimax: once the first player has selected a
// move, its recursive second player can see the card, pillz and Fury. The live game reveals
// only the card. That made an earlier round read 100% because recursion chose a different
// reply for every hidden bet, then fall to 88% when the real next round forced one reply.
//
// This evaluator computes a conservative pure policy that can actually be followed:
//
//   - our future first-mover decision chooses a move against its worst reply (the opponent
//     is still pessimistically allowed to know our hidden bet);
//   - when the opponent moves first, their card is observable but their bet is not, so one
//     response must survive every hidden pillz/Fury value for that card;
//   - the opponent chooses the card and hidden bet that are worst for us.
//
// Search still averages the opponent's *current* unknown choice for its Win column. Each
// continuation folded into that average is now an executable information-safe result, so
// an exact 100% cannot rely on choosing a future reply after peeking at pillz. The result
// remains one of win/draw/loss in P1's [-1, 1] frame.
import Game, { type CardIndex, Undo, Winner } from "../game/Game.ts";
import type Ability from "../game/Ability.ts";
import type Events from "../game/battle/Events.ts";
import type Hand from "../game/Hand.ts";
import { Turn } from "../game/types/Types.ts";
import { shiftRange } from "../utils/Utils.ts";
import { GameResult } from "./Minimax.ts";

/** Four rounds of two selections, plus slack for future rule variants. */
const MAX_DEPTH = 12;
/** At most four cards times the 23 legal bets available with twelve pillz. */
const MAX_RESPONSES = 92;
const FURY_OR_NOT: readonly boolean[] = [true, false];
const NO_FURY: readonly boolean[] = [false];

/**
 * The most continuation values one cache remembers. The largest exact opening measured
 * (capture 877636's FIRST, 8464 units) stores 273,406 in about 80 MB of heap; a full cache
 * only stops remembering new positions, it never changes a value.
 */
export const CACHE_CAPACITY = 1 << 19;

/**
 * Continuation values already computed in one match, keyed by everything they depend on.
 *
 * `roundValue` is a pure function of the position at the start of a round, the asking
 * player and that round's first mover: the recurrence prunes only on exact endpoints,
 * which cannot change a node's value, and `make` reads nothing but the state keyed below.
 * The exact opening reaches the same position through many lines - a Fury bet of p and a
 * plain bet of p + 3 leave the same pillz - so most positions entering the later rounds
 * are repeats, as in the Rust policy's cache (rust/src/advisor/policy.rs).
 *
 * The key is the whole mutable state at the start of a round, as `Undo` lists it:
 *
 *   - `id`, which fixes the round, the first mover of this and every later round, and that
 *     no card is selected (`i1`/`i2` are always undefined between rounds);
 *   - both players' packed ints (life, pillz, won, wonPrevious and the match-start totals)
 *     and both PlayerRounds' (round, first, day), plus each side's `lastClan`;
 *   - which cards each hand has played. A played slot holds the compiled copy
 *     `CachedCardBattle.play` put there, but nothing a later battle reads differs from the
 *     original: every battle compiles its own cards, and a hand is only asked for clans,
 *     names and its Leader, which the copy shares;
 *   - each side's `Events.repeat`, entry by entry and in order, which is where a latched
 *     permanent (with its won/delayed flags and a Growth permanent's frozen amount) and a
 *     Leader's global ability live between rounds. `Events.events` is always empty then,
 *     and `Events.mask` only decides which times are visited: a stale bit is a no-op.
 *
 * A repeat entry is keyed by structure, not identity - every battle merges fresh clones -
 * so its fixed part (type, text, conditions, and each modifier with its class) is
 * serialised once per object and interned, with its won/delayed flags read live. (Every
 * permanent a battle merges either latches in it or removes itself, and a delayed Poison or
 * Heal clears `delayed` in that same battle, so between rounds a permanent is always won
 * and never delayed and a Leader's entry never has either set; both are keyed anyway.) The
 * fixed part is settled by then: a Growth permanent freezes its amount when it latches.
 * Entries that serialise differently but behave alike only cost a miss; entries that behave
 * differently cannot serialise alike, since that is their whole state.
 *
 * Values are stored only once complete, and the cache forgets everything when it is shown
 * a different match (`Game.matchTables`). It belongs to one `PolicyStack`, so to one
 * `Search` or one worker of a `ParallelSearch`; nothing here is process-global.
 */
export class ContinuationCache {
  private tables: readonly [object, object] | undefined = undefined;
  private readonly values = new Map<string, number>();
  /** The interned fixed part of each repeat entry seen, by object. */
  private entryIds = new WeakMap<Ability, number>();
  private readonly signatures = new Map<string, number>();
  hits = 0;
  misses = 0;

  get size() {
    return this.values.size;
  }

  /** Forget every value unless `game` is a position in the match they were computed in. */
  bind(game: Game) {
    const tables = game.matchTables;
    if (
      this.tables === undefined || this.tables[0] !== tables[0] ||
      this.tables[1] !== tables[1]
    ) {
      this.values.clear();
      this.signatures.clear();
      this.entryIds = new WeakMap();
      this.tables = tables;
    }
  }

  key(game: Game, us: Turn): string {
    const h1 = game.h1, h2 = game.h2;
    const played1 = (h1[0].played ? 1 : 0) | (h1[1].played ? 2 : 0) |
      (h1[2].played ? 4 : 0) | (h1[3].played ? 8 : 0);
    const played2 = (h2[0].played ? 1 : 0) | (h2[1].played ? 2 : 0) |
      (h2[2].played ? 4 : 0) | (h2[3].played ? 8 : 0);
    return `${game.id} ${game.winner} ${us} ${game.turn} ` +
      `${game.p1.snapshot()} ${game.p2.snapshot()} ` +
      `${game.r1.snapshot()} ${game.r2.snapshot()} ${played1} ${played2}|` +
      `${game.r1.lastClan ?? ""}|${game.r2.lastClan ?? ""}|` +
      `${this.repeatKey(game.events1)}|${this.repeatKey(game.events2)}`;
  }

  get(key: string): number | undefined {
    const value = this.values.get(key);
    if (value === undefined) this.misses++;
    else this.hits++;
    return value;
  }

  set(key: string, value: number) {
    if (this.values.size < CACHE_CAPACITY) this.values.set(key, value);
  }

  private repeatKey(events: Events): string {
    let key = "";
    for (let t = 0; t < 10; t++) {
      const bucket = events.repeat[t];
      if (bucket.length === 0) continue;
      key += `${t}:`;
      for (let k = 0; k < bucket.length; k++) {
        const a = bucket[k];
        key += `${this.entryId(a)}${flag(a.won)}${flag(a.delayed)},`;
      }
    }
    return key;
  }

  private entryId(a: Ability): number {
    let id = this.entryIds.get(a);
    if (id === undefined) {
      const signature = JSON.stringify(
        [
          a.type,
          a.ability,
          a.conditions,
          a.mods.map((m) => [m.constructor.name, m]),
        ],
        exactNumbers,
      );
      id = this.signatures.get(signature);
      if (id === undefined) {
        id = this.signatures.size;
        this.signatures.set(signature, id);
      }
      this.entryIds.set(a, id);
    }
    return id;
  }
}

const flag = (b: boolean | undefined) => b === undefined ? "u" : b ? "t" : "f";

/** JSON writes every non-finite number as null, but a Min of -Infinity is not +Infinity. */
function exactNumbers(_key: string, value: unknown) {
  return typeof value === "number" && !Number.isFinite(value)
    ? `#${value}`
    : value;
}

export interface PolicyStack {
  undo: Undo[];
  /** Worst result per possible response, reused for one hidden-bet information set. */
  worst: Float64Array[];
  /** A response is inactive once one hidden bet has proved its worst possible result. */
  active: Uint8Array[];
  /** Completed round values of the current match; undefined recomputes every position. */
  cache: ContinuationCache | undefined;
}

export function newPolicyStack(cache = true): PolicyStack {
  const undo = new Array<Undo>(MAX_DEPTH);
  const worst = new Array<Float64Array>(MAX_DEPTH / 2);
  const active = new Array<Uint8Array>(MAX_DEPTH / 2);
  for (let i = 0; i < MAX_DEPTH; i++) undo[i] = new Undo();
  for (let i = 0; i < worst.length; i++) {
    worst[i] = new Float64Array(MAX_RESPONSES);
    active[i] = new Uint8Array(MAX_RESPONSES);
  }
  return {
    undo,
    worst,
    active,
    cache: cache ? new ContinuationCache() : undefined,
  };
}

/** Conservative eventual result under an observable-information policy for `us`. */
export default function policyValue(
  game: Game,
  us: Turn,
  stack: PolicyStack = newPolicyStack(),
): number {
  if (game.firstHasSelected) {
    throw new Error("policyValue requires a position at the start of a round");
  }
  stack.cache?.bind(game);
  return roundValue(game, us, 0, stack);
}

function terminalValue(game: Game): number | undefined {
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
}

function continuation(
  game: Game,
  us: Turn,
  depth: number,
  stack: PolicyStack,
): number {
  return terminalValue(game) ?? roundValue(game, us, depth, stack);
}

function unplayed(hand: Hand): CardIndex[] {
  const indexes: CardIndex[] = [];
  for (let i = 0; i < 4; i++) if (!hand[i].played) indexes.push(i);
  return indexes;
}

const better = (candidate: number, best: number, us: Turn) =>
  us === Turn.PLAYER_1 ? candidate > best : candidate < best;
const worse = (candidate: number, worst: number, us: Turn) =>
  us === Turn.PLAYER_1 ? candidate < worst : candidate > worst;
const bestPossible = (us: Turn) =>
  us === Turn.PLAYER_1 ? GameResult.PLAYER_1_WIN : GameResult.PLAYER_2_WIN;
const worstPossible = (us: Turn) =>
  us === Turn.PLAYER_1 ? GameResult.PLAYER_2_WIN : GameResult.PLAYER_1_WIN;

function actionsPerCard(pillz: number): number {
  return pillz < 3 ? pillz + 1 : pillz * 2 - 1;
}

function roundValue(
  game: Game,
  us: Turn,
  depth: number,
  stack: PolicyStack,
): number {
  const cache = stack.cache;
  if (cache === undefined) {
    return game.turn === us
      ? ourFirstValue(game, us, depth, stack)
      : opponentFirstValue(game, us, depth, stack);
  }
  const key = cache.key(game, us);
  const known = cache.get(key);
  if (known !== undefined) return known;
  const value = game.turn === us
    ? ourFirstValue(game, us, depth, stack)
    : opponentFirstValue(game, us, depth, stack);
  cache.set(key, value);
  return value;
}

/** We choose a full move; pessimistically, the opponent finds its worst reply. */
function ourFirstValue(
  game: Game,
  us: Turn,
  depth: number,
  stack: PolicyStack,
): number {
  const firstUndo = stack.undo[depth];
  const secondUndo = stack.undo[depth + 1];
  const firstIndexes = unplayed(game.playingHand);
  const secondHand = game.turn === Turn.PLAYER_1 ? game.h2 : game.h1;
  const secondPlayer = game.turn === Turn.PLAYER_1 ? game.p2 : game.p1;
  const secondIndexes = unplayed(secondHand);
  const firstPillz = game.playingPlayer.pillz;
  const secondPillz = secondPlayer.pillz;
  const win = bestPossible(us);
  const loss = worstPossible(us);
  let best = loss;

  for (const firstIndex of firstIndexes) {
    for (const firstBet of shiftRange(firstPillz)) {
      const firstFuries = firstBet <= firstPillz - 3 ? FURY_OR_NOT : NO_FURY;
      for (const firstFury of firstFuries) {
        if (!game.make(firstIndex, firstBet, firstFury, firstUndo)) continue;
        let replyWorst = win;
        responses: for (const secondIndex of secondIndexes) {
          for (const secondBet of shiftRange(secondPillz)) {
            const secondFuries = secondBet <= secondPillz - 3
              ? FURY_OR_NOT
              : NO_FURY;
            for (const secondFury of secondFuries) {
              if (
                !game.make(secondIndex, secondBet, secondFury, secondUndo)
              ) continue;
              const value = continuation(game, us, depth + 2, stack);
              game.unmake(secondUndo);
              if (worse(value, replyWorst, us)) replyWorst = value;
              if (replyWorst === loss) break responses;
            }
          }
        }
        game.unmake(firstUndo);

        if (better(replyWorst, best, us)) best = replyWorst;
        if (best === win) return best;
      }
    }
  }
  return best;
}

/**
 * They choose a card and hidden bet. We may condition our response on the visible card,
 * but the same response has to be used for every pillz/Fury hypothesis for that card.
 */
function opponentFirstValue(
  game: Game,
  us: Turn,
  depth: number,
  stack: PolicyStack,
): number {
  const firstUndo = stack.undo[depth];
  const secondUndo = stack.undo[depth + 1];
  const firstIndexes = unplayed(game.playingHand);
  const secondHand = game.turn === Turn.PLAYER_1 ? game.h2 : game.h1;
  const secondPlayer = game.turn === Turn.PLAYER_1 ? game.p2 : game.p1;
  const secondIndexes = unplayed(secondHand);
  const firstPillz = game.playingPlayer.pillz;
  const secondPillz = secondPlayer.pillz;
  const responseWorst = stack.worst[depth >> 1];
  const active = stack.active[depth >> 1];
  const responses = secondIndexes.length * actionsPerCard(secondPillz);
  const win = bestPossible(us);
  const loss = worstPossible(us);
  let result = win;

  for (const firstIndex of firstIndexes) {
    responseWorst.fill(win, 0, responses);
    active.fill(1, 0, responses);
    let activeResponses = responses;

    hidden: for (const firstBet of shiftRange(firstPillz)) {
      const firstFuries = firstBet <= firstPillz - 3 ? FURY_OR_NOT : NO_FURY;
      for (const firstFury of firstFuries) {
        if (!game.make(firstIndex, firstBet, firstFury, firstUndo)) continue;
        let response = 0;
        for (const secondIndex of secondIndexes) {
          for (const secondBet of shiftRange(secondPillz)) {
            const secondFuries = secondBet <= secondPillz - 3
              ? FURY_OR_NOT
              : NO_FURY;
            for (const secondFury of secondFuries) {
              if (
                active[response] &&
                game.make(secondIndex, secondBet, secondFury, secondUndo)
              ) {
                const value = continuation(game, us, depth + 2, stack);
                if (worse(value, responseWorst[response], us)) {
                  responseWorst[response] = value;
                }
                if (responseWorst[response] === loss) {
                  active[response] = 0;
                  activeResponses--;
                }
                game.unmake(secondUndo);
              }
              response++;
            }
          }
        }
        game.unmake(firstUndo);
        if (activeResponses === 0) break hidden;
      }
    }

    let bestResponse = loss;
    for (let response = 0; response < responses; response++) {
      if (better(responseWorst[response], bestResponse, us)) {
        bestResponse = responseWorst[response];
      }
    }
    if (worse(bestResponse, result, us)) result = bestResponse;
    if (result === loss) return result;
  }

  return result;
}
