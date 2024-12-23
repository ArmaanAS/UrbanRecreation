import Game from "../game/Game.ts";
import Analysis from "./Analysis.ts";
import process from "node:process";

export default class Bar {
  step: number;
  interval: number;
  inverse: number;
  total: number;
  value: number[];
  num: number[];
  start: number;
  t: number;
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

  push(game: Game, cards: number) {
    const a = new Analysis(game, undefined, undefined, false);

    const p = a.playingPlayer.pillz + 1;
    const num = (p + Math.max(0, p - 3)) * cards; //a.getCardIndexes().length;

    this.num.push(num);
    this.value.push(this.value[this.value.length - 1] / num);
  }

  pop() {
    this.total += this.value.pop()! * this.num.pop()!;
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
      const elapsed = (+new Date() - this.start) / 1000;
      const remaining = (elapsed / this.total) * (100 - this.total);
      process.stdout.write(
        "\r " +
          ` Complete: ${this.step.toFixed(2)}% `.bgGreen.dim.white +
          ` Tick: ${((+new Date() - this.t) / 1000).toFixed(2)}s  `
            .green +
          `Elapsed: ${elapsed.toFixed(2)}s  `.yellow.dim +
          `Remaining: ${remaining.toFixed(2)}s  `.yellow +
          `                `,
      );
      this.step += this.interval;
      this.t = +new Date();
    }
  }
}
