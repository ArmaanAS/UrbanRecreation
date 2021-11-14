import Game, { Winner } from "../game/Game"
import { shiftRange } from '../utils/Utils'
import Bar from "./Bar"
import Minimax, { GameResult, Node } from "./Minimax"
import PromiseMap from '../utils/PromiseMap'
import DistributedAnalysis from "./DistributedAnalysis"
import { Turn } from "../game/types/Types"

export default class Analysis {
  game: Game;
  constructor(game: Game, inputs = false, logs = false, clone = true) {
    if (!clone)
      this.game = game;
    else if (game instanceof Game)
      this.game = game.clone(inputs, logs);
    // this.game = Game.fromClone(Clone(game), inputs, logs);
    else
      (this.game = Game.from(game, inputs, logs))
        .createBattleDataCache();
  }

  get playingHand() {
    // return this.game.selectedFirst !== this.game.first ?
    //   this.game.h1 : this.game.h2;
    return this.game.turn === Turn.PLAYER_1 ?
      this.game.h1 : this.game.h2;
  }

  get playingPlayer() {
    // return this.game.selectedFirst !== this.game.first ?
    //   this.game.p1 : this.game.p2;
    return this.game.turn === Turn.PLAYER_1 ?
      this.game.p1 : this.game.p2;
  }

  get playedCardIndex() {
    // if (this.game.selectedFirst !== this.game.first) {
    if (this.game.turn === Turn.PLAYER_1) {
      if (this.game.i2 === undefined)
        throw new Error('this.game.i2 == undefined ' + this.game.i1 + ' ' + this.game.i2)

      return this.game.i2[0]

    } else {
      if (this.game.i1 === undefined)
        throw new Error('this.game.i1 == undefined')

      return this.game.i1[0]
    }
  }

  get playedHand() {
    // return this.game.selectedFirst !== this.game.first ?
    //   this.game.h2 : this.game.h1;
    return this.game.turn === Turn.PLAYER_1 ?
      this.game.h2 : this.game.h1;
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
    for (let i = 0; i < 4; i++)
      if (!hand[i].played)
        indexes[j++] = i;

    return indexes;
  }

  get turn(): Turn {
    return this.game.turn;
  }

  deselect() {
    if (!this.game.firstHasSelected) return;

    const i = this.playedCardIndex;
    // this.getCardHand()[i].won = undefined;
    this.playedHand[i].played = false;
    this.game.firstHasSelected = false;
    if (this.game.turn === Turn.PLAYER_1)
      this.game.i1 = undefined;
    else
      this.game.i2 = undefined;

    return i;
  }



  static async fillTree(
    game: Game,
    minimax: Node | string = new Minimax(),
    bar = new Bar(),
    index: number,
    pillz: number,
    fury = false,
    fullSearch = false
  ) {
    const a = new Analysis(game);

    if (typeof minimax == "string")
      minimax = Minimax.node(minimax);


    let indexes, i;
    if (minimax.name != "root") {
      a.game.select(index, pillz, fury);

      if (!a.game.firstHasSelected && a.game.winner) {
        if (a.game.winner === Winner.PLAYER_1)
          minimax.result = GameResult.PLAYER_1_WIN;
        else if (a.game.winner === Winner.TIE)
          minimax.result = GameResult.TIE;
        else if (a.game.winner === Winner.PLAYER_2)
          minimax.result = GameResult.PLAYER_2_WIN;
        else
          throw new Error(`Unknown winner: "${a.game.winner}"`);

        return minimax;
      }
    } else
      i = a.deselect();

    if (i != undefined) {
      minimax.playingSecond = true;
      indexes = [i];
    } else {
      indexes = a.unplayedCardIndexes;
    }
    minimax.turn = a.turn;

    if (bar) bar.push(a.game, indexes.length);

    for (let p = 0; p <= a.playingPlayer.pillz; p++) {
      for (const f of p <= a.playingPlayer.pillz - 3 ? [true, false] : [false]) {
        const promises = [];
        for (const i of indexes) {
          if (a.game.round <= 2) {
            promises.push(
              DistributedAnalysis.threadedFillTree(
                a.game,
                `${i} ${p} ${f}`,
                bar,
                i, p, f,
                minimax.playingSecond
              ).then((m: Node) => {
                (minimax as Node).add(m);
                if (bar) bar.tick();
              })
            );
          } else {
            const m = await Analysis.fillTree(
              a.game,
              `${i} ${p} ${f}`,
              bar,
              i,
              p,
              f,
              minimax.playingSecond
            );
            minimax.add(m);

            if (!minimax.playingSecond && !fullSearch) {
              if (
                (m.result == GameResult.PLAYER_1_WIN && m.turn == Minimax.MAX) ||
                (m.result == GameResult.PLAYER_2_WIN && m.turn == Minimax.MIN)
              ) {
                process.stdout.write("break\n");
                break;
              }
            }
          }
        }
        await Promise.allSettled(promises);
      }
    }

    if (bar) bar.pop();

    return minimax;
  }



  static async iterTree1(game: Game, child = false) {
    const minimax = new Minimax();
    const timer = `iterTree${Math.random()}`;
    console.time(timer);
    let games = [new Analysis(game)];
    let nodes: Node[] = [minimax];
    let indexes: number[] | undefined;

    let depth = 0;

    let _i: number | undefined;
    if (!child && (_i = games[0].deselect()) !== undefined) {
      minimax.playingSecond = true;
      indexes = [_i];
    } else indexes = games[0].unplayedCardIndexes;

    minimax.turn = games[0].turn;

    while (games.length) {
      const next = [];
      const layer = [];

      for (const index in games) {
        const a = games[index];
        const n = nodes[index];

        if (indexes === undefined)
          indexes = a.unplayedCardIndexes;


        const pillz = a.playingPlayer.pillz;
        // let promises = [];
        const items = [];
        root: for (const i of indexes) {
          // card: for (let p = 0; p <= pillz; p++) {
          let breaking = false;
          card: for (const p of shiftRange(pillz)) {
            for (const f of p <= pillz - 3 ? [true, false] : [false]) {
              const c = new Analysis(a.game);
              c.game.select(i, p, f);
              const m = n.add(`${i} ${p} ${f}`, c.turn, n.playingSecond);

              if (!c.game.firstHasSelected && c.game.winner) {
                if (c.game.winner === Winner.PLAYER_1) {
                  // m.win();
                  m.result = GameResult.PLAYER_1_WIN;
                  if (!n.defered) {
                    if (m.turn) {
                      m.break = true;
                      break root;
                    } else if (p == pillz) {
                      breaking = true;
                    } else if (breaking && f && p == pillz - 3) {
                      m.break = true;
                      break card;
                    }
                  }
                } else if (c.game.winner === Winner.TIE) {
                  // m.tie();
                  m.result = GameResult.TIE;
                } else if (c.game.winner == Winner.PLAYER_2) {
                  // m.loss();
                  m.result = GameResult.PLAYER_2_WIN;
                  if (!n.defered) {
                    if (!m.turn) {
                      m.break = true;
                      break root;
                    } else if (p == pillz) {
                      breaking = true;
                    } else if (breaking && f && p == pillz - 3) {
                      m.break = true;
                      break card;
                    }
                  }
                } else throw new Error(`Unknown winner: "${c.game.winner}"`);
              } else {
                if (c.game.round === 1 && !c.game.firstHasSelected) {
                  // n.add(await Analysis.threadedIterTree(c.game));
                  n.add(await Analysis.iterTree1(c.game, true));
                } else if (c.game.round == 2 && !c.game.firstHasSelected) {
                  // promises.push(Analysis.threadedIterTree(c.game).then(i => n.add(i)));
                  items.push(c.game);

                  // n.add(await Analysis.threadedIterTree(c.game));
                  // n.add(await Analysis.iterTree(c.game, true));
                } else {
                  next.push(c);
                  layer.push(m);
                }
              }
            }
          }
        }
        // await Promise.allSettled(promises);
        await PromiseMap.race(items, g =>
          DistributedAnalysis.threadedIterTree(g)
            .then((x: Node) => n.add(x)), (((((6 - 1))))));
        indexes = undefined;
      }

      process.stdout.write(
        `[${(depth++).toString().green}`.grey + ']'.grey + ' Finished  ' +
        `Games: ${games.length}\n`.yellow
      );
      games = next;
      nodes = layer;
      console.timeLog(timer);
    }

    console.timeEnd(timer);
    return minimax;
  }


  static counter = 0;
  static async iterTree(game: Game, child = false, thread = true) {
    // const timer = `iterTree-${this.counter++}`;
    // console.time(timer);

    const minimax = new Minimax();
    const rootAnalysis = new Analysis(game);
    let analyses = [rootAnalysis];
    let nodes: Node[] = [minimax];
    let indexes: number[] | undefined;


    let depth = 0;

    let _i: number | undefined;
    if (!child && (_i = rootAnalysis.deselect()) !== undefined) {
      minimax.playingSecond = true;
      indexes = [_i];
    } else {
      indexes = rootAnalysis.unplayedCardIndexes;
    }

    minimax.turn = rootAnalysis.turn;

    while (analyses.length) {
      const roundAnalyses = [];
      const roundNodes = [];

      for (let index = 0; index < analyses.length; index++) {
        const parentAnalysis = analyses[index];
        const parentNode = nodes[index];

        if (indexes === undefined)
          indexes = parentAnalysis.unplayedCardIndexes;

        const pillz = parentAnalysis.playingPlayer.pillz;
        root: for (const i of indexes) {
          let breaking = false;
          card: for (const p of shiftRange(pillz)) {
            for (const f of (p <= pillz - 3 ? [true, false] : [false])) {

              const analysis = new Analysis(parentAnalysis.game);
              const game = analysis.game;
              game.select(i, p, f);
              const node = parentNode.add(`${i} ${p} ${f}`,
                analysis.turn, parentNode.playingSecond);

              if (!game.firstHasSelected && game.winner !== Winner.PLAYING) {
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
                } else throw new Error(`Unknown winner: "${game.winner}"`);
              } else {
                if (game.round === 1 && !game.firstHasSelected) {
                  parentNode.add(await Analysis.iterTree(game, true));

                } else if (game.round === 2 && !game.firstHasSelected) {
                  // } else if (game.round === 2 && thread) {
                  await DistributedAnalysis.race();
                  DistributedAnalysis.iterTree(game)
                    .then(node => parentNode.add(node))

                } else {
                  roundAnalyses.push(analysis);
                  roundNodes.push(node);
                }
              }
            }
          }
        }

        // process.stdout.write(minimax.toString());

        await DistributedAnalysis.allFinished();
        indexes = undefined;
      }

      process.stdout.write(
        `[${(depth++).toString().green}`.grey + ']'.grey + ' Finished  ' +
        `Games: ${analyses.length}\n`.yellow
      );
      analyses = roundAnalyses;
      nodes = roundNodes;
      // console.timeLog(timer);
    }

    // console.timeEnd(timer);
    return minimax;
  }
}