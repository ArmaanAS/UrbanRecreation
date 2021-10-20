// const Ability = require("./ability");
import Ability from "./ability.mjs";
import Events from "./events.mjs";

class PlayerBattle {
  constructor(round, player1, card1, player2, card2, events) {
    this.round = round;

    this.player = player1;
    this.card = card1;

    this.opp = player2;
    this.oppCard = card2;

    this.events = events;

    let l = round.hand.getLeader();
    if (l) {
      Ability.leader(l, this);
    }

    if (this.card.clan != "Leader") {
      Ability.card(card1, this);
    }
  }

  getClanCards(card, opp = false) {
    return this.round.getClanCards(card, opp);
  }
}

export default class Battle {
  constructor(
    round,
    p1,
    card1,
    pillz1,
    fury1,
    p2,
    card2,
    pillz2,
    fury2,
    events1,
    events2
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

    this.b1 = new PlayerBattle(round.r1, p1, card1, p2, card2, events1);

    this.b2 = new PlayerBattle(round.r2, p2, card2, p1, card1, events2);

    this.play();
  }

  play() {
    this.p1.wonPrevious = this.p1.won;
    this.p2.wonPrevious = this.p2.won;
    // this.events.execute(Events.BEGIN);

    // this.events.execute(Events.PRE_BATTLE);
    this.events1.executePre(this.b1);
    this.events2.executePre(this.b2);

    if (this.fury1) {
      this.card1.damage.final += 2;
    }
    if (this.fury2) {
      this.card2.damage.final += 2;
    }

    this.card1.attack.final = this.card1.power.final * (this.pillz1 + 1);
    this.card2.attack.final = this.card2.power.final * (this.pillz2 + 1);

    // this.events.execute(Events.POST_BATTLE);
    this.events1.executePost(this.b1);
    this.events2.executePost(this.b2);

    console.log(
      `\t\t\t\t\t${this.p1.name}: Attack ${this.card1.attack.final}   |   ${this.p2.name}: Attack ${this.card2.attack.final}`
      .brightWhite
    );

    this.p1.pillz -= this.pillz1 + (this.fury1 ? 3 : 0);
    this.p2.pillz -= this.pillz2 + (this.fury2 ? 3 : 0);

    let c1 = this.card1;
    let c2 = this.card2;
    let a1 = c1.attack.final;
    let a2 = c2.attack.final;
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