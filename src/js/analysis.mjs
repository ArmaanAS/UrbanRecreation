import Game from "./game.mjs";
import {
  Worker
} from "worker_threads";
import Minimax from "./minimax.mjs";
import PromiseMap from './promiseMap.mjs';

function* alternateRange(n) {
  let i = 0;
  for (; i < n; i++) {
    yield i;
    yield n--;
  }
  if (i == n) yield i;
}

function* shiftRange(n) {
  yield n;
  if (n >= 3) yield n - 3;
  for (let i = 0; i < n - 3; i++) {
    yield i;
  }
  for (let i = n - 2; i < n; i++) {
    yield i;
  }
}

export default class Analysis {
  constructor(game, inputs = false, logs = false, clone = true) {
    if (!clone) {
      this.game = game;
    } else {
      if (game instanceof Game) {
        this.game = game.clone(inputs, logs);
      } else {
        this.game = Game.from(game, inputs, logs);
      }
    }
  }

  getTurnCards() {
    return this.game.selectedFirst != this.game.round.first ?
      this.game.h1.cards :
      this.game.h2.cards;
  }

  getTurnPlayer() {
    return this.game.selectedFirst != this.game.round.first ?
      this.game.p1 :
      this.game.p2;
  }

  getCardIndex() {
    return this.game.selectedFirst != this.game.round.first ?
      this.game.i2[0] :
      this.game.i1[0];
  }

  getCardHand() {
    return this.game.selectedFirst != this.game.round.first ?
      this.game.h2 :
      this.game.h1;
  }

  getCardIndexes() {
    return this.getTurnCards()
      .filter((c) => !c.played)
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
    if (this.game.selectedFirst) {
      let i = this.getCardIndex();
      this.getCardHand().get(i).played = false;
      this.game.selectedFirst = false;

      return i;
    }
  }

  static threadedMove(data = {}, game, index, pillz, fury = false, per = 100) {
    // function tm() {
    return new Promise((resolve, reject) => {
      const worker = new Worker("./threadMove.mjs", {
        workerData: {
          // args: [game, index, pillz, fury, per],
          args: [...arguments].splice(1),
          game: game,
          threadID: `${index} ${pillz}`, // Analysis.threads++
          ...data,
        },
      });
      worker.on("message", (data) => {
        console.log(data);
        resolve(data);
      });
      worker.on("error", reject);
      worker.on("exit", (code) => {
        if (code !== 0)
          reject(new Error(`Worker stopped with exit code ${code}`));
      });
    });
    // };

    // return tm();
  }

  static threadedFillTree(game, minimax, bar, index, pillz, fury, fullSearch) {
    return new Promise((resolve, reject) => {
      let worker = new Worker("./threadFillTree.mjs", {
        workerData: {
          game: game,
          minimax: minimax,
          bar: null,
          index: index,
          pillz: pillz,
          fury: fury,
          fullSearch: fullSearch,
          threadID: `${index} ${pillz} ${fury}`,
        },
      });
      worker.on("message", (minimax) => {
        // console.log(minimax);
        resolve(Minimax.from(minimax));
      });
      worker.on("error", reject);
      worker.on("exit", (code) => {
        if (code !== 0)
          reject(new Error(`Worker stopped with exit code ${code}`));
      });
    });
  }

  static async fillTree(
    game,
    minimax = new Minimax(),
    bar = new Bar(),
    index,
    pillz,
    fury = false,
    fullSearch = false
  ) {
    let a = new Analysis(game);

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
      minimax.defer = true;
      indexes = [i];
    } else {
      indexes = a.getCardIndexes();
    }
    minimax.turn = a.getTurn();

    if (bar) bar.push(a.game, indexes.length);

    for (let p = 0; p <= a.getTurnPlayer().pillz; p++) {
      for (let f of p <= a.getTurnPlayer().pillz - 3 ? [true, false] : [false]) {
        let promises = [];
        for (let i of indexes) {
          if (a.game.round.round <= 2) {
            promises.push(
              Analysis.threadedFillTree(
                a.game,
                `${i} ${p} ${f}`,
                bar,
                i,
                p,
                f,
                minimax.defer
              ).then((m) => {
                minimax.add(m);
                if (bar) bar.tick();
              })
            );
          } else {
            let m = await Analysis.fillTree(
              a.game,
              `${i} ${p} ${f}`,
              bar,
              i,
              p,
              f,
              minimax.defer
            );
            minimax.add(m);

            if (!minimax.defer && !fullSearch) {
              if (
                (m.score == Minimax.WIN && m.turn == Minimax.MAX) ||
                (m.score == Minimax.LOSS && m.turn == Minimax.MIN)
              ) {
                console._stdout.write("break\n");
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

  static threadedIterTree(game) {
    return new Promise((resolve, reject) => {
      let worker = new Worker("./threadIterTree.mjs", {
        workerData: game,
      });
      worker.on("message", (minimax) => {
        // console.log(minimax);
        resolve(Minimax.from(minimax));
      });
      worker.on("error", reject);
      worker.on("exit", (code) => {
        if (code !== 0)
          reject(new Error(`Worker stopped with exit code ${code}`));
      });
    });
  }

  static async iterTree(game, child = false) {
    let minimax = new Minimax();
    let timer = `iterTree${Math.random()}`;
    console.time(timer);
    let games = [new Analysis(game)];
    let nodes = [minimax];
    let indexes;

    let depth = 0;

    let _i;
    if (!child && (_i = games[0].deselect()) != undefined) {
      minimax.defer = true;
      indexes = [_i];
    } else {
      indexes = games[0].getCardIndexes();
    }
    minimax.turn = games[0].getTurn();

    while (games.length) {
      let next = [];
      let layer = [];

      for (let index in games) {
        let a = games[index];
        let n = nodes[index];

        if (indexes === undefined) {
          indexes = a.getCardIndexes();
        }

        let pillz = a.getTurnPlayer().pillz;
        let promises = [];
        let items = [];
        root: for (let i of indexes) {
          // card: for (let p = 0; p <= pillz; p++) {
          let breaking = false;
          card: for (let p of shiftRange(pillz)) {
            for (let f of p <= pillz - 3 ? [true, false] : [false]) {
              let c = new Analysis(a.game);
              c.game.select(i, p, f);
              let m = n.add(`${i} ${p} ${f}`, c.getTurn(), n.defer);

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
                } else {
                  throw new Error(`Unknown winner: "${c.game.winner}"`);
                }
              } else {
                if (c.game.round.round == 1 && !c.game.selectedFirst) {
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
          Analysis.threadedIterTree(g)
            .then(x => n.add(x)), 6);
        indexes = undefined;
      }

      console._stdout.write(
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

class Bar {
  constructor(interval = 0.25) {
    this.step = 0;
    this.interval = interval;
    this.inverse = 1 / interval;

    this.total = 0;
    this.value = [100];
    this.num = [1];

    this.start = +new Date();
    this.t = +new Date();
  }

  push(game, cards) {
    let a = new Analysis(game, undefined, undefined, false);
    let num;

    let p = a.getTurnPlayer().pillz + 1;
    num = (p + Math.max(0, p - 3)) * cards; //a.getCardIndexes().length;

    this.num.push(num);
    this.value.push(this.value[this.value.length - 1] / num);
  }

  pop() {
    this.total += this.value.pop() * this.num.pop();
    this.num[this.num.length - 1]--;

    this.check();
  }

  tick() {
    this.total += this.value[this.value.length - 1];
    this.num[this.num.length - 1]--;
    // console.log('Tick');

    this.check();
  }

  check() {
    // if (this.value[this.value.length - 1] > 1.9) {
    //   console._stdout.write(`Total: ${this.total}%\n`);
    // }
    if (this.total >= this.step) {
      // console._stdout.write(`${this.value[this.value.length - 1]}\n`)
      this.step = Math.floor(this.total * this.inverse) / this.inverse;
      let elapsed = (+new Date() - this.start) / 1000;
      let remaining = (elapsed / this.total) * (100 - this.total);
      console._stdout.write(
        "\r " +
        ` Complete: ${this.step.toFixed(2)}% `.bgGreen.brightWhite +
        ` Tick: ${((+new Date() - this.t) / 1000).toFixed(2)}s  `
          .brightGreen +
        `Elapsed: ${elapsed.toFixed(2)}s  `.yellow +
        `Remaining: ${remaining.toFixed(2)}s  `.brightYellow +
        `                `
      );
      this.step += this.interval;
      this.t = +new Date();
    }
  }
}