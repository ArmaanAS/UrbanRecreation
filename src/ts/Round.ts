import CardBattle from './CardBattle';
import Events from './Events';
import Hand from './Hand';
import Player from './Player';
import PlayerRound from './PlayerRound';


export default class Round {
  round: number;
  day: boolean;
  first: boolean;
  h1: Hand;
  h2: Hand;
  p1: Player;
  p2: Player;
  events1: Events;
  events2: Events;
  ca1: boolean;
  ca2: boolean;
  r1: PlayerRound;
  r2: PlayerRound;
  hand: any;
  constructor(round: number, day = true, first = true, p1: Player, h1: Hand, p2: Player, h2: Hand) {
    this.round = round; // 1, 2, 3, 4
    this.day = day;
    this.first = round % 2 == +first;//round % 2 == 1; //first;

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

  clone(p1: Player, h1: Hand, p2: Player, h2: Hand) {
    let events1 = this.events1.clone();
    let events2 = this.events2.clone();
    return Object.setPrototypeOf({
      round: this.round,
      day: this.day,
      first: this.first,
      p1, h1, p2, h2,
      events1,
      events2,
      ca1: this.ca1,
      ca2: this.ca2,
      r1: this.r1.clone(p1, h1, p2, h2, events1),
      r2: this.r2.clone(p2, h2, p1, h1, events2)
    }, Round.prototype);
  }

  static from(o: Round) {
    Object.setPrototypeOf(o, Round.prototype);

    o.events1 = Events.from(o.events1);
    o.events2 = Events.from(o.events2);

    Object.setPrototypeOf(o.r1, PlayerRound.prototype);
    Object.setPrototypeOf(o.r2, PlayerRound.prototype);

    return o;
  }

  next() {
    if (this.ca1 == this.ca2)
      this.first = !this.first;
    else if (this.ca1)
      this.first = false;
    else if (this.ca2)
      this.first = true;

    this.round++;

    this.r1.next(this.first);
    this.r2.next(!this.first);
  }

  battle(card1index: number, pillz1: number, fury1: boolean, card2index: number, pillz2: number, fury2: boolean) {
    let c1 = this.h1.get(card1index);
    let c2 = this.h2.get(card2index);

    new CardBattle(this,
      this.p1, c1, pillz1, fury1,
      this.p2, c2, pillz2, fury2,
      this.events1, this.events2
    ).play();

    this.next();
  }
}