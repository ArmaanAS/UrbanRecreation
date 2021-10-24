import BattleData from "./BattleData";
import Card from "./Card";
import Events from "./Events";
import Player from "./Player";
import Round from "./Round";


export default class CardBattle {
  round: Round;
  p1: Player;
  card1: Card;
  pillz1: number;
  fury1: boolean;
  p2: Player;
  card2: Card;
  pillz2: number;
  fury2: boolean;
  events1: Events;
  events2: Events;
  b1: BattleData;
  b2: BattleData;
  constructor(
    round: Round,
    p1: Player,
    card1: Card,
    pillz1: number,
    fury1: boolean,
    p2: Player,
    card2: Card,
    pillz2: number,
    fury2: boolean,
    events1: Events,
    events2: Events
  ) {
    this.round = round;

    this.p1 = p1;
    this.card1 = card1;
    this.pillz1 = pillz1;
    this.fury1 = fury1;

    this.p2 = p2;
    this.card2 = card2;
    this.pillz2 = pillz2;
    this.fury2 = fury2;

    this.events1 = events1;
    this.events2 = events2;

    this.b1 = new BattleData(round.r1, p1, card1, p2, card2, events1);
    this.b2 = new BattleData(round.r2, p2, card2, p1, card1, events2);
  }

  play() {
    this.p1.wonPrevious = this.p1.won;
    this.p2.wonPrevious = this.p2.won;

    this.events1.executePre(this.b1);
    this.events2.executePre(this.b2);

    if (this.fury1)
      this.card1.damage_.final += 2;

    if (this.fury2)
      this.card2.damage_.final += 2;


    // this.card1.attack_.final = this.card1.power.final * (this.pillz1 + 1);
    // this.card2.attack_.final = this.card2.power.final * (this.pillz2 + 1);
    const attack1 = this.card1.power.final * (this.pillz1 + 1);
    const attack2 = this.card2.power.final * (this.pillz2 + 1);
    this.card1.attack_.final = attack1;
    this.card2.attack_.final = attack2;

    this.events1.executePost(this.b1);
    this.events2.executePost(this.b2);

    console.log(
      `\t\t\t\t\t${this.p1.name}: Attack ${this.card1.attack.final}\
      |     ${this.p2.name}: Attack ${this.card2.attack.final}`
        .white
    );

    this.p1.pillz -= this.pillz1 + (this.fury1 ? 3 : 0);
    this.p2.pillz -= this.pillz2 + (this.fury2 ? 3 : 0);

    const c1 = this.card1;
    const c2 = this.card2;
    const a1 = c1.attack.final;
    const a2 = c2.attack.final;
    if (
      a1 > a2 ||
      (a1 == a2 && c1.stars < c2.stars) ||
      (c1.stars == c2.stars && this.round.first && a1 == a2)
    ) {
      // console.log(`Life -${this.card1.damage.final}`);
      this.p1.won = this.card1.won = true;
      this.p2.won = this.card2.won = false;
      this.p2.life -= this.card1.damage.final;
    } else {
      // console.log(`Life -${this.card2.damage.final}`);
      this.p1.won = this.card1.won = false;
      this.p2.won = this.card2.won = true;
      this.p1.life -= this.card2.damage.final;
    }

    this.events1.executeEnd(this.b1);
    this.events2.executeEnd(this.b2);

    this.card1.played = true;
    this.card2.played = true;
  }
}