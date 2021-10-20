import Battle from './battle.mjs';
import Events from './events.mjs';

class PlayerRound {
  constructor(round, day, first, p1, h1, p2, h2, events) {
    this.round = round;
    this.day = day;
    this.first = first;

    this.player = p1;
    this.hand = h1;

    this.opp = p2;
    this.oppHand = h2;

    this.events = events;
  }

  clone(p1, h1, p2, h2, events) {
    return new PlayerRound(this.round, this.day, this.first, p1, h1, p2, h2, events);
  }

  next(first) {
    this.round++;

    this.first = first;
  }

  getClanCards(card, opp = false) {
    return (opp ? this.oppHand : this.hand).getClanCards(card);
  }
}

export default class Round {
  constructor(round, day = true, first = true, p1, h1, p2, h2) {
    this.round = round; // 1, 2, 3, 4
    this.day = day;
    this.first = round % 2 == first;//round % 2 == 1; //first;

    this.p1 = p1
    this.h1 = h1;

    this.p2 = p2;
    this.h2 = h2;

    this.events1 = new Events();
    this.events2 = new Events();

    this.ca1 = false;
    this.ca2 = false;
    let l = this.h1.getLeader();
    if (l && l.ability.string == 'Counter-attack') {
      this.ca1 = true;
    }

    l = this.h2.getLeader();
    if (l && l.ability.string == 'Counter-attack') {
      this.ca2 = true;
    }

    if (this.ca1 == this.ca2) {
      this.first = true;
    } else if (this.ca1) {
      this.first = false;
    } else if (this.ca2) {
      this.first = true;
    }

    this.r1 = new PlayerRound(round, day, first, p1, h1, p2, h2, this.events1);
    this.r2 = new PlayerRound(round, day, !first, p2, h2, p1, h1, this.events2);
  }

  clone(p1, h1, p2, h2) {
    let events1 = this.events1.clone();
    let events2 = this.events2.clone();
    return Object.setPrototypeOf({
      round: this.round,
      day: this.day,
      first: this.first,
      p1: p1,
      h1: h1,
      p2: p2,
      h2: h2,
      events1: events1,
      events2: events2,
      ca1: this.ca1,
      ca2: this.ca2,
      r1: this.r1.clone(p1, h1, p2, h2, events1),
      r2: this.r2.clone(p2, h2, p1, h1, events2)
    }, Round.prototype);
  }

  static from(o) {
    Object.setPrototypeOf(o, Round.prototype);

    o.events1 = Events.from(o.events1);
    o.events2 = Events.from(o.events2);

    Object.setPrototypeOf(o.r1, PlayerRound.prototype);
    Object.setPrototypeOf(o.r2, PlayerRound.prototype);

    return o;
  }

  next() {
    if (this.ca1 == this.ca2) {
      this.first = !this.first;
    } else if (this.ca1) {
      this.first = false;
    } else if (this.ca2) {
      this.first = true;
    }
    this.round++;

    this.r1.next(this.first);
    this.r2.next(!this.first);
  }

  battle(card1index, pillz1, fury1, card2index, pillz2, fury2) {
    let c1 = this.h1.get(card1index);
    let c2 = this.h2.get(card2index);

    let battle = new Battle(this,
      this.p1, c1, pillz1, fury1,
      this.p2, c2, pillz2, fury2,
      this.events1, this.events2
    );
    this.next();
  }
}

// module.exports = {
//   Round: Round,
//   Events: AbilityEvents
// };