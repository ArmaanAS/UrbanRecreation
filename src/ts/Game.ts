import Hand from "./Hand";
import * as _ from "colors";
import readline from "readline";
import { CardJSON, HandOf } from './types/Types'
import { clone } from "./utils/Utils";
import Player from "./Player";
import Round from "./Round";

let rl: readline.Interface | undefined;


export default class Game {
  round: Round;
  inputs: boolean;
  logs: boolean;
  p1: Player;
  p2: Player;
  winner: string | undefined;
  h1: Hand;
  h2: Hand;
  selectedFirst: boolean;
  i1: [number, number, boolean] | undefined;
  i2: [number, number, boolean] | undefined;

  constructor(
    p1: Player, p2: Player, h1: Hand, h2: Hand,
    inputs: boolean, logs = true, repeat: boolean | undefined,
    first = true
  ) {
    this.round = new Round(1, true, first, p1, h1, p2, h2);
    this.inputs = inputs;
    this.logs = logs;

    this.p1 = p1;
    this.p2 = p2;
    // this.winner = undefined;

    this.h1 = h1;
    this.h2 = h2;

    this.selectedFirst = false;
    // this.i1 = undefined;
    // this.i2 = undefined;

    for (let hand of [h1, h2]) {
      for (let card of hand.cards) {
        if (card.clan == "Leader") {
          if (hand.getClanCards(card) > 1) {
            card.ability.string = "No Ability";
          }
        } else {
          if (hand.getClanCards(card) == 1) {
            card.bonus.string = "No Bonus";
          }
        }
      }
    }

    this.log();

    if (inputs) {
      this.input(repeat);
    }
    // this.select(0, 3);
    // this.select(0, 3);
  }

  clone(inputs?: boolean, logs?: boolean) {
    const p1 = clone(this.p1);
    const p2 = clone(this.p2);
    const h1 = this.h1.clone();
    const h2 = this.h2.clone();

    return Object.setPrototypeOf({
      round: this.round.clone(p1, h1, p2, h2),
      inputs: inputs ?? this.inputs,
      logs: logs ?? this.logs,
      winner: this.winner,
      p1, p2, h1, h2,
      selectedFirst: this.selectedFirst,
      i1: this.i1,
      i2: this.i2,
    }, Game.prototype);
  }

  static from(o: Game, inputs?: boolean, logs?: boolean) {
    Object.setPrototypeOf(o, Game.prototype);

    o.round = Round.from(o.round);

    Object.setPrototypeOf(o.p1, Player.prototype);
    Object.setPrototypeOf(o.p2, Player.prototype);

    o.h1 = Hand.from(o.h1);
    o.h2 = Hand.from(o.h2);

    if (inputs !== undefined)
      o.inputs = inputs;

    if (logs !== undefined)
      o.logs = logs;

    return o;
  }

  log(override = false) {
    if (!override && !this.logs) return;

    if (this.i1 != undefined) {
      this.p1.log(this.round.round);
      this.h1.draw("cyan");
    } else {
      this.p1.log(this.round.round);
      this.h1.draw(this.selectedFirst != this.round.first ? "yellow" : "white");
    }

    if (this.i2 != undefined) {
      this.p2.log(this.round.round);
      this.h2.draw("cyan");
    } else {
      this.p2.log(this.round.round);
      this.h2.draw(this.selectedFirst != this.round.first ? "white" : "yellow");
    }
  }

  select(index: number, pillz: number, fury = false) {
    if (typeof index != 'number' || typeof pillz != 'number') //return false;
      throw new Error(`Game.select - index or pillz is not a number 
        index: ${index}, pillz: ${pillz}`)

    if (this.selectedFirst != this.round.first) {
      if (this.h1.get(index).played) {
        // console.debug(index, pillz, this.h1.get(index))
        return false;
      }

      this.i1 = [index, pillz, fury];
      this.h1.get(index).played = true;
    } else {
      if (this.h2.get(index).played) {
        // console.debug(index, pillz, this.h2.get(index))
        return false;
      }

      this.i2 = [index, pillz, fury];
      this.h2.get(index).played = true;
    }

    if (this.selectedFirst) {
      this.battle();
      this.i1 = undefined;
      this.i2 = undefined;
    }

    this.selectedFirst = !this.selectedFirst;

    this.log();
    return true;
  }

  battle() {
    if (this.i1 != undefined && this.i2 != undefined) {
      this.round.battle(...this.i1, ...this.i2);

      if (this.p1.life <= 0 && this.p2.life <= 0) {
        return 'Tie';
      } else if (this.p1.life <= 0) {
        this.winner = this.p2.name;
      } else if (this.p2.life <= 0) {
        this.winner = this.p1.name;
      } else if (this.round.round >= 5) {
        if (this.p1.life > this.p2.life) {
          this.winner = this.p1.name;
        } else if (this.p1.life < this.p2.life) {
          this.winner = this.p2.name;
        } else {
          this.winner = "Tie";
        }
      }
    }
    return;
  }

  input(repeat = true) {
    if (!rl) {
      rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout,
      });
    }

    let msg = `
         _____      _           _                       _ 
        /  ___|    | |         | |                     | |
        \\ \`--.  ___| | ___  ___| |_    ___ __ _ _ __ __| |
         \`--. \\/ _ \\ |/ _ \\/ __| __|  / __/ _\` | '__/ _\` |
        /\\__/ /  __/ |  __/ (__| |_  | (_| (_| | | | (_| |
        \\____/ \\___|_|\\___|\\___|\\__|  \\___\\__,_|_|  \\__,_| o o o
                                                                                                                                                
    \n`;
    return new Promise((resolve, reject) => {
      rl!.question(msg.green, async answer => {
        console.log(`Selected ${answer}`);
        let s = answer.trim().split(' ');

        if (!this.select(+s[0], +(s[1] ?? 0), s[2] == 'true')) {
          resolve(await this.input(repeat));

        } else {
          if (this.checkWinner(repeat)) {
            console.log("\n\nClosing readline...");
            rl!.close();
            resolve(s);

          } else {
            if (repeat) {
              resolve(await this.input(true));

            } else {
              // play(this);
              resolve(s);
            }
          }
        }
      });
    });
  }

  checkWinner(log = false) {
    if (this.round.round > 4 || this.p1.life <= 0 || this.p2.life <= 0) {
      if (log) {
        this.log();

        console.log("\n Game over!\n".white.bgRed);

        if (this.p1.life > this.p2.life) {
          console.log(` ${` ${this.p1.name} `.white.bgCyan} won the match!\n`);
        } else if (this.p1.life < this.p2.life) {
          console.log(` ${` ${this.p2.name} `.white.bgCyan} won the match!\n`);
        } else {
          console.log("Game is a draw!\n".green);
        }
      }

      return true;
    } else {
      return false;

    }
  }

  getTurn() {
    return this.selectedFirst != this.round.first ? 'Player' : 'Urban Rivals';
  }

  static create(inputs = true, logs: boolean | undefined, repeat: boolean | undefined) {
    let p1 = new Player(12, 12, "Player");
    let p2 = new Player(12, 12, "Urban Rival");

    // let h1 = Hand.generate('Oon Cr');
    // let h2 = Hand.generate('Schwarz');
    let h1 = Hand.generate('Roderick', 'Frank', 'Katsuhkay', 'Oyoh'); // Roderick
    // let h1 = Hand.generate('Frank', 'Katsuhkay', 'Frank', 'Katsuhkay'); // Roderick
    let h2 = Hand.generate('Behemoth Cr', 'Vholt', 'Eyrik', 'Kate');
    // 1023 Lizbeth Mt, 1934 Lady Ametia Cr,
    // 942 Gerald, 1890 Anne Derya, 1932 Mel-T

    // 'Jessie', 'Timber'
    // 'gwen', 'Cassio Cr'

    return new Game(p1, p2, h1, h2, inputs, logs, repeat);
  }

  static createUnique(h1: HandOf<CardJSON>, h2: HandOf<CardJSON>, life: number, pillz: number, name1: string | undefined, name2: string | undefined, first: boolean | undefined) {
    let p1 = new Player(life, pillz, name1);
    let p2 = new Player(life, pillz, name2);

    return new Game(
      p1, p2,
      Hand.generateRaw(h1),
      Hand.generateRaw(h2),
      false, false, false,
      first
    );
  }
}