import Hand, { HandGenerator } from "./Hand";
import "colors";
import readline from "readline";
import { CardJSON, HandOf } from './types/Types'
import { clone } from "./utils/Utils";
import Player from "./Player";
import Round from "./Round";
import GameRenderer from "./utils/GameRenderer";

let rl: readline.Interface;


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
  i1?: [number, number, boolean] = undefined;
  i2?: [number, number, boolean] = undefined;

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

    for (const hand of [h1, h2]) {
      // for (const card of hand.cards) {
      for (const card of hand) {
        if (card.clan == "Leader") {
          if (hand.getClanCards(card) > 1)
            card.ability_.string = "No Ability";

        } else {
          if (hand.getClanCards(card) == 1)
            card.bonus_.string = "No Bonus";
        }
      }
    }

    GameRenderer.draw(this);

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

    // o.h1 = 
    Hand.from(o.h1);
    // o.h2 = 
    Hand.from(o.h2);

    if (inputs !== undefined)
      o.inputs = inputs;

    if (logs !== undefined)
      o.logs = logs;

    return o;
  }
  static fromClone(o: Game, inputs?: boolean, logs?: boolean) {
    Object.setPrototypeOf(o, Game.prototype);

    o.round.p1 = o.p1;
    o.round.p2 = o.p2;
    o.round.h1 = o.h1;
    o.round.h2 = o.h2;

    o.round.r1.player = o.p1;
    o.round.r1.hand = o.h1;
    o.round.r1.opp = o.p2;
    o.round.r1.oppHand = o.h2;
    o.round.r1.events = o.round.events1;

    o.round.r2.player = o.p2;
    o.round.r2.hand = o.h2;
    o.round.r2.opp = o.p1;
    o.round.r2.oppHand = o.h1;
    o.round.r2.events = o.round.events2;

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


  select(index: number, pillz: number, fury = false) {
    if (typeof index != 'number' || typeof pillz != 'number') //return false;
      throw new Error(`Game.select - index or pillz is not a number 
        index: ${index}, pillz: ${pillz}`)

    if (this.selectedFirst != this.round.first) {
      // if (this.h1.get(index).played)
      // if (this.h1.get(index).won !== undefined)
      // if (this.h1[index].won !== undefined)
      if (this.h1[index].played)
        return false;

      this.i1 = [index, pillz, fury];
      this.h1[index].played = true;
    } else {
      // if (this.h2.get(index).played)
      // if (this.h2.get(index).won !== undefined)
      // if (this.h2[index].won !== undefined)
      if (this.h2[index].played)
        return false;

      this.i2 = [index, pillz, fury];
      // this.h2.get(index).played = true;
      this.h2[index].played = true;
    }

    if (this.selectedFirst) {
      this.battle();
      this.i1 = undefined;
      this.i2 = undefined;
    }

    this.selectedFirst = !this.selectedFirst;

    GameRenderer.draw(this);
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

    const msg = `
         _____      _           _                       _ 
        /  ___|    | |         | |                     | |
        \\ \`--.  ___| | ___  ___| |_    ___ __ _ _ __ __| |
         \`--. \\/ _ \\ |/ _ \\/ __| __|  / __/ _\` | '__/ _\` |
        /\\__/ /  __/ |  __/ (__| |_  | (_| (_| | | | (_| |
        \\____/ \\___|_|\\___|\\___|\\__|  \\___\\__,_|_|  \\__,_| o o o
                                                                                                                                                
    \n`;
    return new Promise((resolve) => {
      rl.question(msg.green, async answer => {
        console.log(`Selected ${answer}`);
        const s = answer.trim().split(' ');

        if (!this.select(+s[0], +(s[1] ?? 0), s[2] == 'true')) {
          resolve(await this.input(repeat));

        } else {
          if (this.hasWinner(repeat)) {
            console.log("\n\nClosing readline...");
            rl.close();
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

  hasWinner(log = false) {
    if (this.round.round > 4 || this.p1.life <= 0 || this.p2.life <= 0) {
      if (log) {
        GameRenderer.draw(this)

        console.log("\n Game over!\n".white.bgRed);

        if (this.p1.life > this.p2.life)
          console.log(` ${` ${this.p1.name} `.white.bgCyan} won the match!\n`);
        else if (this.p1.life < this.p2.life)
          console.log(` ${` ${this.p2.name} `.white.bgCyan} won the match!\n`);
        else
          console.log("Game is a draw!\n".green);
      }

      return true;
    }

    return false;
  }

  getTurn() {
    return this.selectedFirst != this.round.first ? 'Player' : 'Urban Rivals';
  }
}

export class GameGenerator {
  static create(inputs = true, logs?: boolean, repeat?: boolean) {
    const p1 = new Player(12, 12, 0);  // "Player");
    const p2 = new Player(12, 12, 1);  // "Urban Rival");

    const h1 = HandGenerator.generate('Roderick', 'Frank', 'Katsuhkay', 'Oyoh'); // Roderick
    // let h1 = Hand.generate('Frank', 'Katsuhkay', 'Frank', 'Katsuhkay'); // Roderick
    const h2 = HandGenerator.generate('Behemoth Cr', 'Vholt', 'Eyrik', 'Kate');

    // 'Jessie', 'Timber'
    // 'gwen', 'Cassio Cr'

    return new Game(p1, p2, h1, h2, inputs, logs, repeat);
  }

  static createUnique(h1: HandOf<CardJSON>, h2: HandOf<CardJSON>, life: number, pillz: number, name1: string | undefined, name2: string | undefined, first: boolean | undefined) {
    const p1 = new Player(life, pillz, 0);  // name1);
    const p2 = new Player(life, pillz, 1);  // name2);

    return new Game(
      p1, p2,
      HandGenerator.generateRaw(h1),
      HandGenerator.generateRaw(h2),
      false, false, false,
      first
    );
  }
}