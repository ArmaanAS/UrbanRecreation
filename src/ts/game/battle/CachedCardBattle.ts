import BattleData from "./BattleData";
import CachedBattleData from "./CachedBattleData";
import CachedEvents from "./CachedEvents";
import Card from "../Card";
import Events from "./Events";
import Game from "../Game";
import Hand from "../Hand";
import Player from "../Player";
import { Turn } from "../types/Types";
import GameRenderer from "../../utils/GameRenderer";

export default class CachedCardBattle {
  card1: Card;
  card2: Card;
  events1: CachedEvents;
  events2: CachedEvents;
  constructor(
    hand1: Hand,
    card1: Card,
    hand2: Hand,
    card2: Card,
  ) {
    this.card1 = card1.clone();
    this.card2 = card2.clone();

    this.events1 = new CachedEvents();
    this.events2 = new CachedEvents();

    // Compile the cards
    new CachedBattleData(hand1, this.card1, this.card2, this.events1);
    new CachedBattleData(hand2, this.card2, this.card1, this.events2);
  }

  play(
    game: Game,
    p1: Player,
    pillz1: number,
    fury1: boolean,
    p2: Player,
    pillz2: number,
    fury2: boolean,
    events1: Events,
    events2: Events
  ) {
    // Merging the states
    this.events1.merge(events1);
    this.events2.merge(events2);

    // Clone cards
    const card1 = this.card1.clone();
    const card2 = this.card2.clone();

    // Create BattleData from current game data
    const totalPillz1 = pillz1 + (fury1 ? 3 : 0);
    const totalPillz2 = pillz2 + (fury2 ? 3 : 0);
    const b1 = new BattleData(
      game.r1, p1, card1, totalPillz1, p2,
      card2, totalPillz2, events1, false);
    const b2 = new BattleData(
      game.r2, p2, card2, totalPillz2, p1,
      card1, totalPillz1, events2, false);

    // Place cloned and compiled cards into hands 
    game.h1[card1.index] = card1;
    game.h2[card2.index] = card2;


    // Playing the game
    p1.wonPrevious = p1.won;
    p2.wonPrevious = p2.won;

    events1.executePre(b1);
    events2.executePre(b2);

    if (fury1)
      card1.damage.final += 2;

    if (fury2)
      card2.damage.final += 2;


    const attack1 = card1.power.final * (pillz1 + 1);
    const attack2 = card2.power.final * (pillz2 + 1);
    card1.attack.final = attack1;
    card2.attack.final = attack2;

    events1.executePost(b1);
    events2.executePost(b2);

    console.log(
      `\t\t\t\t\t${` ${p2.name} `.bgBlue.white} Attack ${card2.attack.final}\
      |       ${` ${p1.name} `.bgBlue.white} Attack ${card1.attack.final}`
        .white
    );

    if (p1.pillz - totalPillz1 < 0) {
      process.stdout.write(`p1.pillz: ${p1.pillz}, totalPillz1: ${totalPillz1}, pillz1: ${pillz1}, fury1: ${fury1}\n`);
      process.stdout.write(`${game.i1} | ${game.i2}`);
      const log = console.log;
      console.log = console.info;
      GameRenderer.draw(game, true);
      console.log = log;
    }
    if (p2.pillz - totalPillz2 < 0) {
      process.stdout.write(`p2.pillz: ${p2.pillz}, totalPillz2: ${totalPillz2}, pillz2: ${pillz2}, fury2: ${fury2}\n`);
      process.stdout.write(`${game.i1} | ${game.i2}`);
      const log = console.log;
      console.log = console.info;
      GameRenderer.draw(game, true);
      console.log = log;
    }

    p1.pillz -= totalPillz1;
    p2.pillz -= totalPillz2;

    if (
      attack1 > attack2 ||
      (attack1 === attack2 && (card1.stars < card2.stars ||
        (card1.stars === card2.stars &&
          game.playingFirst === Turn.PLAYER_1)))
    ) {
      // console.log(`Life -${card1.damage.final}`);
      p1.won = card1.won = true;
      p2.won = card2.won = false;
      p2.life -= card1.damage.final;
    } else {
      // console.log(`Life -${card2.damage.final}`);
      p1.won = card1.won = false;
      p2.won = card2.won = true;
      p1.life -= card2.damage.final;
    }

    events1.executeEnd(b1);
    events2.executeEnd(b2);

    card1.played = true;
    card2.played = true;
  }
}