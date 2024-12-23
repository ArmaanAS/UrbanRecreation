import Game, { Winner } from "../game/Game.ts";
import { shiftRange } from "../utils/Utils.ts";
// import Bar from "./Bar.ts"
import Minimax, { GameResult, Node } from "./Minimax.ts";
// import DistributedAnalysis from "./DistributedAnalysis.ts";
import { Turn } from "../game/types/Types.ts";
// import cluster from "node:cluster";
// import process from "node:process";

export default class Analysis {
  game: Game;
  constructor(game: Game, inputs = false, logs = false, clone = true) {
    if (!clone) {
      this.game = game;
    } else if (game instanceof Game) {
      this.game = game.clone(inputs, logs);
    } // this.game = Game.fromClone(Clone(game), inputs, logs);
    else {
      this.game = Game.from(game, inputs, logs);
    }
  }

  get playingHand() {
    // return this.game.selectedFirst !== this.game.first ?
    //   this.game.h1 : this.game.h2;
    return this.game.turn === Turn.PLAYER_1 ? this.game.h1 : this.game.h2;
  }

  get playingPlayer() {
    // return this.game.selectedFirst !== this.game.first ?
    //   this.game.p1 : this.game.p2;
    return this.game.turn === Turn.PLAYER_1 ? this.game.p1 : this.game.p2;
  }

  get playedCardIndex() {
    // if (this.game.selectedFirst !== this.game.first) {
    if (this.game.turn === Turn.PLAYER_1) {
      if (this.game.i2 === undefined) {
        throw new Error(
          "this.game.i2 == undefined " + this.game.i1 + " " + this.game.i2,
        );
      }

      return this.game.i2[0];
    } else {
      if (this.game.i1 === undefined) {
        throw new Error("this.game.i1 == undefined");
      }

      return this.game.i1[0];
    }
  }

  get playedHand() {
    // return this.game.selectedFirst !== this.game.first ?
    //   this.game.h2 : this.game.h1;
    return this.game.turn === Turn.PLAYER_1 ? this.game.h2 : this.game.h1;
  }

  get unplayedCardIndexes() {
    // return this.getTurnCards()
    //   .filter((c) => !c.played)
    //   // .filter((c) => c.won === undefined)
    //   .map((c) => c.index);

    // const indexes = new Array<number>(4 - this.game.round);
    const indexes: number[] = [];
    const hand = this.playingHand;
    let j = 0;
    for (let i = 0; i < 4; i++) {
      if (!hand[i].played) {
        indexes[j++] = i;
      }
    }

    return indexes;
  }

  get turn(): Turn {
    return this.game.turn;
  }

  private deselect() {
    if (!this.game.firstHasSelected) return;

    const i = this.playedCardIndex;

    this.playedHand[i].played = false;
    this.game.id--;

    if (this.game.turn === Turn.PLAYER_1) {
      this.game.i1 = undefined;
    } else {
      this.game.i2 = undefined;
    }

    return i;
  }

  static async iterTree(
    game: Game,
    child = false,
    rootName?: string,
    freeze = true,
  ) {
    const rootNode = new Minimax(rootName, game.turn);
    const rootAnalysis = new Analysis(game);
    let analyses = [rootAnalysis];
    let nodes: Node[] = [rootNode];
    let indexes: number[] | undefined;

    let depth = 0;

    let _i: number | undefined;
    if (!child && (_i = rootAnalysis.deselect()) !== undefined) {
      rootNode.turn = rootAnalysis.turn;
      rootNode.playingSecond = true;
      indexes = [_i];
    } else {
      indexes = rootAnalysis.unplayedCardIndexes;
    }

    while (analyses.length) {
      const roundAnalyses: Analysis[] = [];
      const roundNodes: Node[] = [];
      let counter = 0;

      for (let index = 0; index < analyses.length; index++) {
        const parentAnalysis = analyses[index];
        const parentNode = nodes[index];

        if (indexes === undefined) {
          indexes = parentAnalysis.unplayedCardIndexes;
        }

        const pillz = parentAnalysis.playingPlayer.pillz;
        root: for (const i of indexes) {
          let breaking = false;
          card: for (const p of shiftRange(pillz)) {
            for (const f of (p <= pillz - 3 ? [true, false] : [false])) {
              counter++;

              const analysis = new Analysis(parentAnalysis.game);
              const game = analysis.game;
              game.select(i, p, f);

              if (!game.firstHasSelected && game.winner !== Winner.PLAYING) {
                const node = parentNode.add(
                  `${i} ${p} ${f}`,
                  analysis.turn,
                  parentNode.playingSecond,
                );

                if (game.winner === Winner.PLAYER_1) {
                  node.result = GameResult.PLAYER_1_WIN;
                  if (!parentNode.defered) {
                    if (node.turn === Turn.PLAYER_1) {
                      node.break = true;
                      break root;
                    } else if (p === pillz) {
                      breaking = true;
                    } else if (breaking && f && p === pillz - 3) {
                      node.break = true;
                      break card;
                    }
                  }
                } else if (game.winner === Winner.TIE) {
                  node.result = GameResult.TIE;
                } else if (game.winner === Winner.PLAYER_2) {
                  node.result = GameResult.PLAYER_2_WIN;
                  if (!parentNode.defered) {
                    if (node.turn === Turn.PLAYER_2) {
                      node.break = true;
                      break root;
                    } else if (p === pillz) {
                      breaking = true;
                    } else if (breaking && f && p === pillz - 3) {
                      node.break = true;
                      break card;
                    }
                  }
                }
              } else {
                if (game.round === 1 && !game.firstHasSelected) {
                  parentNode.add(await Analysis.iterTree(game, true));
                } else {
                  const node = parentNode.add(
                    `${i} ${p} ${f}`,
                    analysis.turn,
                    parentNode.playingSecond,
                  );
                  roundAnalyses.push(analysis);
                  roundNodes.push(node);
                }
              }
            }
          }
        }

        indexes = undefined;
      }

      console.log(
        `[${depth++}] Finished  Moves: ${counter}`,
      );
      analyses = roundAnalyses;
      nodes = roundNodes;
    }

    return child && freeze ? rootNode.freeze() : rootNode;
  }
}
