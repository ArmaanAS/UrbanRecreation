import BattleData from "./BattleData";
import Card from "../Card";
import Events from "./Events";
import Game from "../Game";
import Player from "../Player";
import { Turn } from "../types/Types";

export default class CardBattle {
  constructor(
    game: Game,
    p1: Player,
    card1: Card,
    pillz1: number,
    fury1: boolean,
    p2: Player,
    card2: Card,
    pillz2: number,
    fury2: boolean,
    events1: Events,
    events2: Events,
    compile = true
  ) {
    const totalPillz1 = pillz1 + (fury1 ? 3 : 0);
    const totalPillz2 = pillz2 + (fury2 ? 3 : 0);
    // this.
    const b1 = new BattleData(
      game.r1, p1, card1, totalPillz1,
      p2, card2, totalPillz2, events1, compile);
    // this.
    const b2 = new BattleData(
      game.r2, p2, card2, totalPillz2,
      p1, card1, totalPillz1, events2, compile);

    // CardBattle.battle(
    //   game, p1, card1, pillz1, fury1,
    //   p2, card2, pillz2, fury2,
    //   events1, events2, b1, b2
    // )
    p1.wonPrevious = p1.won;
    p2.wonPrevious = p2.won;

    events1.executePre(b1);
    events2.executePre(b2);

    if (fury1)
      card1.damage.final += 2;

    if (fury2)
      card2.damage.final += 2;


    const a1 = card1.power.final * (pillz1 + 1);
    const a2 = card2.power.final * (pillz2 + 1);
    card1.attack.final = a1;
    card2.attack.final = a2;

    events1.executePost(b1);
    events2.executePost(b2);

    console.log(
      `\t\t\t\t\t${` ${p2.name} `.bgBlue.white} Attack ${card2.attack.final}\
      |       ${` ${p1.name} `.bgBlue.white} Attack ${card1.attack.final}`
        .white
    );

    p1.pillz -= totalPillz1;
    p2.pillz -= totalPillz2;

    const attack1 = card1.attack.final;
    const attack2 = card2.attack.final;
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