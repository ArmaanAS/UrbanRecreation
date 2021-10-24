import Game from "../Game"
import { shiftRange } from '../utils/Utils'
import Bar from "./Bar"
import Minimax, { GameResult, Node } from "./Minimax"
import PromiseMap from '../utils/PromiseMap'
import DistributedAnalysis from "./DistributedAnalysis"
// import { clone as Clone } from '../utils/Utils'

export default class Analysis {
  game: Game;
  constructor(game: Game, inputs = false, logs = false, clone = true) {
    if (!clone)
      this.game = game;
    else if (game instanceof Game)
      this.game = game.clone(inputs, logs);
    // this.game = Game.fromClone(Clone(game), inputs, logs);
    else
      this.game = Game.from(game, inputs, logs);
  }

  getTurnCards() {
    return this.game.selectedFirst != this.game.round.first ?
      // this.game.h1.cards :
      // this.game.h2.cards;
      this.game.h1 : this.game.h2;
  }

  getTurnPlayer() {
    return this.game.selectedFirst != this.game.round.first ?
      this.game.p1 :
      this.game.p2;
  }

  getCardIndex() {
    if (this.game.selectedFirst != this.game.round.first) {
      if (this.game.i2 === undefined)
        throw new Error('this.game.i2 == undefined')

      return this.game.i2[0]

    } else {
      if (this.game.i1 === undefined)
        throw new Error('this.game.i1 == undefined')

      return this.game.i1[0]
    }
  }

  getCardHand() {
    return this.game.selectedFirst != this.game.round.first ?
      this.game.h2 :
      this.game.h1;
  }

  getCardIndexes() {
    return this.getTurnCards()
      .filter((c) => !c.played)
      // .filter((c) => c.won === undefined)
      .map((c) => c.index);
  }

  getTurn() {
    return this.game.selectedFirst != this.game.round.first;
  }

  // getTotalMoves(cards) {
  //   let p = this.getTurnPlayer().pillz + 1;
  //   return cards * (p + Math.max(0, p - 3));
  // }

  deselect() {
    if (!this.game.selectedFirst) return;

    const i = this.getCardIndex();
    // this.getCardHand().get(i).played = false;
    // this.getCardHand().get(i).won = undefined;
    // this.getCardHand()[i].won = undefined;
    this.getCardHand()[i].played = false;
    this.game.selectedFirst = false;

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

    if (typeof minimax == "string") {
      minimax = Minimax.node(minimax);
    }

    let indexes, i;
    if (minimax.name != "root") {
      a.game.select(index, pillz, fury);

      if (!a.game.selectedFirst && a.game.winner) {
        if (a.game.winner == "Player") {
          return minimax.win();
        } else if (a.game.winner == "Tie") {
          return minimax.tie();
        } else if (a.game.winner == "Urban Rival") {
          return minimax.loss();
        }
        throw new Error(`Unknown winner: "${a.game.winner}"`);
      }
    } else {
      i = a.deselect();
    }

    if (i != undefined) {
      minimax.playSecond = true;
      indexes = [i];
    } else {
      indexes = a.getCardIndexes();
    }
    minimax.turn = a.getTurn();

    if (bar) bar.push(a.game, indexes.length);

    for (let p = 0; p <= a.getTurnPlayer().pillz; p++) {
      for (const f of p <= a.getTurnPlayer().pillz - 3 ? [true, false] : [false]) {
        const promises = [];
        for (const i of indexes) {
          if (a.game.round.round <= 2) {
            promises.push(
              DistributedAnalysis.threadedFillTree(
                a.game,
                `${i} ${p} ${f}`,
                bar,
                i, p, f,
                minimax.playSecond
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
              minimax.playSecond
            );
            minimax.add(m);

            if (!minimax.playSecond && !fullSearch) {
              if (
                (m.result == GameResult.WIN && m.turn == Minimax.MAX) ||
                (m.result == GameResult.LOSS && m.turn == Minimax.MIN)
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
      minimax.playSecond = true;
      indexes = [_i];
    } else indexes = games[0].getCardIndexes();

    minimax.turn = games[0].getTurn();

    while (games.length) {
      const next = [];
      const layer = [];

      for (const index in games) {
        const a = games[index];
        const n = nodes[index];

        if (indexes === undefined)
          indexes = a.getCardIndexes();


        const pillz = a.getTurnPlayer().pillz;
        // let promises = [];
        const items = [];
        root: for (const i of indexes) {
          // card: for (let p = 0; p <= pillz; p++) {
          let breaking = false;
          card: for (const p of shiftRange(pillz)) {
            for (const f of p <= pillz - 3 ? [true, false] : [false]) {
              const c = new Analysis(a.game);
              c.game.select(i, p, f);
              const m = n.add(`${i} ${p} ${f}`, c.getTurn(), n.playSecond);

              if (!c.game.selectedFirst && c.game.winner) {
                if (c.game.winner == "Player") {
                  m.win();
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
                } else if (c.game.winner == "Tie") {
                  m.tie();
                } else if (c.game.winner == "Urban Rival") {
                  m.loss();
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
                if (c.game.round.round == (((((1 + 0))))) && !c.game.selectedFirst) {
                  // n.add(await Analysis.threadedIterTree(c.game));
                  n.add(await Analysis.iterTree(c.game, true));
                } else if (c.game.round.round == 2 && !c.game.selectedFirst) {
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



  static async iterTree(game: Game, child = false) {
    const minimax = new Minimax();
    const timer = `iterTree${Math.random()}`;
    console.time(timer);
    let games = [new Analysis(game)];
    let nodes: Node[] = [minimax];
    let indexes: number[] | undefined;

    let depth = 0;

    let _i: number | undefined;
    if (!child && (_i = games[0].deselect()) !== undefined) {
      minimax.playSecond = true;
      indexes = [_i];
    } else indexes = games[0].getCardIndexes();

    minimax.turn = games[0].getTurn();

    while (games.length) {
      const next = [];
      const layer = [];

      for (const index in games) {
        const a = games[index];
        const n = nodes[index];

        if (indexes === undefined)
          indexes = a.getCardIndexes();


        const pillz = a.getTurnPlayer().pillz;
        root: for (const i of indexes) {
          let breaking = false;
          card: for (const p of shiftRange(pillz)) {
            for (const f of (p <= pillz - 3 ? [true, false] : [false])) {
              const c = new Analysis(a.game);
              c.game.select(i, p, f);
              const m = n.add(`${i} ${p} ${f}`, c.getTurn(), n.playSecond);

              if (!c.game.selectedFirst && c.game.winner) {
                if (c.game.winner == "Player") {
                  m.win();
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
                } else if (c.game.winner == "Tie") {
                  m.tie();
                } else if (c.game.winner == "Urban Rival") {
                  m.loss();
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
                if (c.game.round.round == (((((1 + 0))))) && !c.game.selectedFirst) {
                  n.add(await Analysis.iterTree(c.game, true));
                } else if (c.game.round.round == 2 && !c.game.selectedFirst) {
                  await DistributedAnalysis.race();
                  DistributedAnalysis.iterTree(c.game)
                    .then(node => n.add(node))
                } else {
                  next.push(c);
                  layer.push(m);
                }
              }
            }
          }
        }

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



  static async asyncIterTree(game: Game, child = false) {
    const minimax = new Minimax();
    const timer = `iterTree${Math.random()}`;
    console.time(timer);
    let games = [new Analysis(game)];
    let nodes: Node[] = [minimax];
    let indexes: number[] | undefined;

    let depth = 0;

    let _i: number | undefined;
    if (!child && (_i = games[0].deselect()) !== undefined) {
      minimax.playSecond = true;
      indexes = [_i];
    } else indexes = games[0].getCardIndexes();

    minimax.turn = games[0].getTurn();

    while (games.length) {
      const next = [];
      const layer = [];

      for (const index in games) {
        const a = games[index];
        const n = nodes[index];

        if (indexes === undefined)
          indexes = a.getCardIndexes();


        const pillz = a.getTurnPlayer().pillz;
        // let promises = [];
        // let items = [];
        root: for (const i of indexes) {
          // card: for (let p = 0; p <= pillz; p++) {
          let breaking = false;
          card: for (const p of shiftRange(pillz)) {
            for (const f of (p <= pillz - 3 ? [true, false] : [false])) {
              const c = new Analysis(a.game);
              c.game.select(i, p, f);
              const m = n.add(`${i} ${p} ${f}`, c.getTurn(), n.playSecond);

              if (!c.game.selectedFirst && c.game.winner) {
                if (c.game.winner == "Player") {
                  m.win();
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
                } else if (c.game.winner == "Tie") {
                  m.tie();
                } else if (c.game.winner == "Urban Rival") {
                  m.loss();
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
                if (c.game.round.round == 1 && !c.game.selectedFirst) {
                  // n.add(await Analysis.threadedIterTree(c.game));
                  n.add(await Analysis.iterTree(c.game, true));
                } else if (c.game.round.round == (((((2))))) && !c.game.selectedFirst) {
                  // promises.push(Analysis.threadedIterTree(c.game).then(i => n.add(i)));
                  // items.push(c.game);

                  // n.add(await Analysis.threadedIterTree(c.game));
                  // n.add(await Analysis.iterTree(c.game, true));

                  // n.add(await DistributedAnalysis.iterTree(c.game));
                  await DistributedAnalysis.race();
                  DistributedAnalysis.iterTree(c.game)
                    .then(node => n.add(node))
                } else {
                  next.push(c);
                  layer.push(m);
                }
              }
            }
          }
        }
        // await Promise.allSettled(promises);
        // await PromiseMap.race(items, g =>
        //   DistributedAnalysis.threadedIterTree(g)
        //     .then((x: Node) => n.add(x)), (((((6 - 1))))));
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
}