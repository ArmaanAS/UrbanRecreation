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

export interface PolicyStack {
  undo: Undo[];
  /** Worst result per possible response, reused for one hidden-bet information set. */
  worst: Float64Array[];
  /** A response is inactive once one hidden bet has proved its worst possible result. */
  active: Uint8Array[];
}

export function newPolicyStack(): PolicyStack {
  const undo = new Array<Undo>(MAX_DEPTH);
  const worst = new Array<Float64Array>(MAX_DEPTH / 2);
  const active = new Array<Uint8Array>(MAX_DEPTH / 2);
  for (let i = 0; i < MAX_DEPTH; i++) undo[i] = new Undo();
  for (let i = 0; i < worst.length; i++) {
    worst[i] = new Float64Array(MAX_RESPONSES);
    active[i] = new Uint8Array(MAX_RESPONSES);
  }
  return { undo, worst, active };
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
  return game.turn === us
    ? ourFirstValue(game, us, depth, stack)
    : opponentFirstValue(game, us, depth, stack);
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
