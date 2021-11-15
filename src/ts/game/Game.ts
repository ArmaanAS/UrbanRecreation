import Hand, { HandGenerator } from "./Hand";
import "colors";
import readline from "readline";
import { AbilityString, CardJSON, HandOf } from './types/CardTypes'
import { clone } from "../utils/Utils";
import Player from "./Player";
import GameRenderer from "../utils/GameRenderer";
import Events from "./battle/Events";
import PlayerRound from "./PlayerRound";
import CardBattle from "./battle/CardBattle";
import { Turn } from "./types/Types";
import CachedCardBattle from "./battle/CachedCardBattle";

export let counter = 0;

let rl: readline.Interface;

export enum Winner {
  PLAYING = 0,
  PLAYER_1 = 1,
  PLAYER_2 = 2,
  TIE = 3,
}

type CardIndex = 0 | 1 | 2 | 3 | number;
type Selection = [CardIndex, number, boolean];

export default class Game {
  id: number;
  inputs: boolean;
  logs: boolean;
  p1: Player;
  p2: Player;
  winner = Winner.PLAYING;
  h1: Hand;
  h2: Hand;

  i1?: Selection = undefined;
  i2?: Selection = undefined;

  events1 = new Events();
  events2 = new Events();
  r1: PlayerRound;
  r2: PlayerRound;

  constructor(
    p1: Player, p2: Player, h1: Hand, h2: Hand,
    inputs: boolean, logs = true, repeat?: boolean,
    first: Turn = Turn.PLAYER_1
  ) {
    this.id = 0;
    this.inputs = inputs;
    this.logs = logs;

    this.p1 = p1;
    this.p2 = p2;

    this.h1 = h1;
    this.h2 = h2;

    this.id = this.createBaseGameCache(first);

    this.r1 = new PlayerRound(
      1, this.day, first === Turn.PLAYER_1, p1, h1, p2, h2, this.events1);
    this.r2 = new PlayerRound(
      1, this.day, first === Turn.PLAYER_2, p2, h2, p1, h1, this.events2);

    for (const hand of [h1, h2]) {
      for (const card of hand) {
        if (card.clan == "Leader") {
          if (hand.getClanCards(card) > 1)
            card.ability.string = AbilityString.NO_ABILITY;

        } else {
          if (hand.getClanCards(card) === 1)
            card.bonus.string = AbilityString.NO_ABILITY;
        }
      }
    }

    this.createBattleDataCache();

    GameRenderer.draw(this);

    if (inputs)
      this.input(repeat);
  }

  clone(inputs?: boolean, logs?: boolean): Game {
    const p1 = clone(this.p1);
    const p2 = clone(this.p2);
    const h1 = this.h1.clone();
    const h2 = this.h2.clone();
    const events1 = this.events1.clone();
    const events2 = this.events2.clone();

    return Object.setPrototypeOf({
      id: this.id,
      inputs: inputs ?? this.inputs,
      logs: logs ?? this.logs,
      winner: this.winner,
      p1, p2, h1, h2,
      i1: this.i1,
      i2: this.i2,
      events1,
      events2,
      r1: this.r1.clone(p1, h1, p2, h2, events1),
      r2: this.r2.clone(p2, h2, p1, h1, events2)
    }, Game.prototype);
  }

  static from(o: Game, inputs?: boolean, logs?: boolean) {
    Object.setPrototypeOf(o, Game.prototype);

    Object.setPrototypeOf(o.p1, Player.prototype);
    Object.setPrototypeOf(o.p2, Player.prototype);

    Hand.from(o.h1);
    Hand.from(o.h2);

    Events.from(o.events1);
    Events.from(o.events2);

    Object.setPrototypeOf(o.r1, PlayerRound.prototype);
    Object.setPrototypeOf(o.r2, PlayerRound.prototype);

    if (inputs !== undefined)
      o.inputs = inputs;

    if (logs !== undefined)
      o.logs = logs;

    o.createBaseGameCache();
    o.createBattleDataCache();

    return o;
  }

  get base() {
    return baseGames[this.id];
  }

  get day() {
    return true;
  }

  get ca1() {
    return this.base.ca1;
  }

  get ca2() {
    return this.base.ca2;
  }

  get firstHasSelected() {
    return this.base.firstHasSelected;
  }

  get playingFirst() {
    return this.base.playingFirst;
  }

  get round() {
    return this.base.round;
  }

  get turn() {
    return this.base.turn;
  }


  select(index: CardIndex, pillz: number, fury = false) {
    if (typeof index != 'number' || typeof pillz != 'number') //return false;
      throw new Error(`Game.select - index or pillz is not a number 
        index: ${index}, pillz: ${pillz}`)

    // if (this.firstHasSelected != this.first) {
    if (this.turn === Turn.PLAYER_1) {
      // if (this.h1.get(index).won !== undefined)
      // if (this.h1[index].won !== undefined)
      if (this.h1[index].played)
        return false;

      this.i1 = [index, pillz, fury];
      this.h1[index].played = true;
    } else {
      // if (this.h2.get(index).won !== undefined)
      // if (this.h2[index].won !== undefined)
      if (this.h2[index].played)
        return false;

      this.i2 = [index, pillz, fury];
      this.h2[index].played = true;
    }

    if (this.firstHasSelected) {
      this.battle();
      this.i1 = undefined;
      this.i2 = undefined;
    } else {
      this.id++;
    }

    // this.firstHasSelected = !this.firstHasSelected;

    GameRenderer.draw(this);
    return true;
  }

  battle() {
    if (this.i1 !== undefined && this.i2 !== undefined) {
      const card1 = this.h1[this.i1[0]];
      const card2 = this.h2[this.i2[0]];
      const pillz1 = this.i1[1];
      const pillz2 = this.i2[1];
      const fury1 = this.i1[2];
      const fury2 = this.i2[2];

      // new CachedCardBattle(
      //   this.h1, card1, pillz1, fury1,
      //   this.h2, card2, pillz2, fury2
      // ).play(
      //   this, this.p1, this.p2,
      //   this.events1, this.events2
      // );
      const bc = battleCache[`${this.i1[0]} ${this.i2[0]}`];
      if (bc === undefined) {
        new CardBattle(this,
          this.p1, card1, pillz1, fury1,
          this.p2, card2, pillz2, fury2,
          this.events1, this.events2
        ).play();

        console.error(`CardBattle "${this.i1[0]} ${this.i2[0]}" is not cached`);
        // throw new Error(`CardBattle "${this.i1} ${this.i2}" is not cached`);
      } else {
        bc.play(this,
          this.p1, pillz1, fury1,
          this.p2, pillz2, fury2,
          this.events1, this.events2
        );
      }

      counter++;

      this.nextRound();

      if (this.p1.life <= 0 && this.p2.life <= 0)
        return 'Tie';
      else if (this.p1.life <= 0)
        this.winner = Winner.PLAYER_2;
      else if (this.p2.life <= 0)
        this.winner = Winner.PLAYER_1;
      else if (this.round >= 5) {
        if (this.p1.life > this.p2.life)
          this.winner = Winner.PLAYER_1;
        else if (this.p1.life < this.p2.life)
          this.winner = Winner.PLAYER_2;
        else
          this.winner = Winner.TIE;
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

        const index = +s[0];
        const pillz = +(s[1] ?? 0);
        const fury = s[2] == 'true';
        if (index < 0 || index > 3 || pillz < 0) {
          resolve(await this.input(repeat));
          return;
        }

        if (!this.select(index, pillz, fury)) {
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
    if (this.round > 4 || this.p1.life <= 0 || this.p2.life <= 0) {
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


  private nextRound() {
    this.id++;

    this.r1.next(this.playingFirst === Turn.PLAYER_1);
    this.r2.next(this.playingFirst === Turn.PLAYER_2);
  }


  createBattleDataCache() {
    const log = console.log;
    console.log = () => 1;

    let counter = 0;
    for (let ci1 = 0; ci1 < 4; ci1++) {
      const c1 = this.h1[ci1];
      if (c1.won !== undefined) continue;

      for (let ci2 = 0; ci2 < 4; ci2++) {
        const c2 = this.h2[ci2];
        if (c2.won !== undefined) continue;

        counter++;
        battleCache[`${ci1} ${ci2}`] = new CachedCardBattle(
          this.h1, c1, this.h2, c2,
        );
      }
    }

    console.log = log;
    console.log(
      `Cached ${`${counter}`.green} CardBattles `.white + `(${Object.keys(battleCache).length} keys)`.green.dim
    );
  }

  createBaseGameCache(first = Turn.PLAYER_1) {
    let ca1 = false;
    const l1 = this.h1.getLeader();
    if (l1?.abilityString == 'Counter-attack')
      ca1 = true;

    let ca2 = false;
    const l2 = this.h2.getLeader();
    if (l2?.abilityString == 'Counter-attack')
      ca2 = true;

    let start = 0;
    for (const f of [Turn.PLAYER_1, Turn.PLAYER_2]) {
      let playingFirst = f;

      for (let i = 0; i < 8; i++) {
        const firstHasSelected = i % 2 === 1;
        const turn = playingFirst === Turn.PLAYER_1 ?
          (!firstHasSelected ? Turn.PLAYER_1 : Turn.PLAYER_2) :
          (!firstHasSelected ? Turn.PLAYER_2 : Turn.PLAYER_1);

        baseGames[start + i] = {
          day: true,
          round: Math.floor(i / 2) + 1,
          ca1, ca2,
          playingFirst,
          firstHasSelected,
          turn,
        };

        if (i % 2 === 1)
          if (ca1 && !ca2)
            playingFirst = Turn.PLAYER_2;
          else if (ca2 && !ca1)
            playingFirst = Turn.PLAYER_1;
          else
            playingFirst = playingFirst === Turn.PLAYER_1 ?
              Turn.PLAYER_2 : Turn.PLAYER_1;
      }

      start += 8;
    }

    return first === Turn.PLAYER_1 ? 0 : 8;
  }
}

const battleCache: { [key: string]: CachedCardBattle } = {};

interface BaseGame {
  playingFirst: Turn;
  firstHasSelected: boolean;
  turn: Turn;
  ca1: boolean;
  ca2: boolean;
  day: boolean;
  round: number;
}

const baseGames: { [key: number]: BaseGame } = {};


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

  static createUnique(
    h1: HandOf<CardJSON>, h2: HandOf<CardJSON>,
    life: number, pillz: number,
    first?: Turn
  ) {
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