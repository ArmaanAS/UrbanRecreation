// Depth-first perfect-information minimax with make/unmake. This is retained as the fast
// reference for the original solver; the live advisor now uses Policy.ts so future choices
// cannot depend on pillz and Fury that would still be hidden in the real game.
//
// `Analysis.iterTree` cloned the game once per node - about 23 objects a time, which left
// GC at 27% of a profile and `Game.clone` at 15% of JS. Depth-first play does not need the
// frontier that forced those clones: one game is mutated forward and walked back, so a
// whole subtree costs no allocation beyond the undo stack, which is preallocated per depth.
//
// It is a value function, not a tree builder: the original `Search` only read `.result` off
// the frozen node `iterTree(game, false)` returned, and this returns exactly that number. The
// enumeration order and both pruning rules are copied deliberately, including the parts
// that are arguably wrong (see AGENTS.md on the domination rule and counter-attack), so
// that the two agree move for move - tests/solver/DeepEquivalence.test.ts pins it.
import Game, { Undo, Winner } from "../game/Game.ts";
import { shiftRange } from "../utils/Utils.ts";
import { GameResult } from "./Minimax.ts";
import { Turn } from "../game/types/Types.ts";

/** Four rounds of two plies, plus slack. */
const MAX_DEPTH = 12;

const FURY_OR_NOT: readonly boolean[] = [true, false];
const NO_FURY: readonly boolean[] = [false];

/**
 * Minimax value of `game` in P1's frame (+1 = P1 wins), for a position where a move is due
 * and the game is still running. `game` is left exactly as it was found.
 */
export default function deepValue(
  game: Game,
  stack: Undo[] = newStack(),
): number {
  return search(game, 0, stack);
}

export function newStack(): Undo[] {
  const stack: Undo[] = new Array(MAX_DEPTH);
  for (let i = 0; i < MAX_DEPTH; i++) stack[i] = new Undo();
  return stack;
}

function search(game: Game, depth: number, stack: Undo[]): number {
  // A game is four rounds of two plies, so MAX_DEPTH is never reached; grow rather than
  // read past the end if that assumption ever changes.
  const undo = stack[depth] ?? (stack[depth] = new Undo());
  const maximising = game.turn === Turn.PLAYER_1;
  let best = maximising ? -Infinity : Infinity;

  const pillz = game.playingPlayer.pillz;
  const indexes = game.unplayedCardIndexes;

  outer: for (let ci = 0; ci < indexes.length; ci++) {
    const i = indexes[ci];
    // Set when the all-in bet loses for the mover; with the biggest fury bet losing too,
    // every other bet for this card is dominated in both attack and damage.
    let breaking = false;

    for (const p of shiftRange(pillz)) {
      const furies = p <= pillz - 3 ? FURY_OR_NOT : NO_FURY;
      for (let fi = 0; fi < furies.length; fi++) {
        const f = furies[fi];
        if (!game.make(i, p, f, undo)) continue;

        // `turn` after the move, which is what iterTree's cutoff tested.
        const after = game.turn;
        const resolved = !game.firstHasSelected;
        const winner = game.winner;

        let value: number;
        if (resolved && winner !== Winner.PLAYING) {
          value = winner === Winner.PLAYER_1
            ? GameResult.PLAYER_1_WIN
            : winner === Winner.PLAYER_2
            ? GameResult.PLAYER_2_WIN
            : GameResult.TIE;
        } else {
          value = search(game, depth + 1, stack);
        }

        game.unmake(undo);

        if (maximising) {
          if (value > best) best = value;
        } else if (value < best) best = value;

        if (resolved && winner !== Winner.PLAYING) {
          if (winner === Winner.PLAYER_1) {
            if (after === Turn.PLAYER_1) break outer; // best possible for the mover
            else if (p === pillz) breaking = true;
            else if (breaking && f && p === pillz - 3) continue outer;
          } else if (winner === Winner.PLAYER_2) {
            if (after === Turn.PLAYER_2) break outer;
            else if (p === pillz) breaking = true;
            else if (breaking && f && p === pillz - 3) continue outer;
          }
        }
      }
    }
  }

  return best;
}
