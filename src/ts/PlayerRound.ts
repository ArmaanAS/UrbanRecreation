import Card from "./Card";
import Events from "./Events";
import Hand from "./Hand";
import Player from "./Player";

export default class PlayerRound {
  // round: number;
  // day: boolean;
  // first: boolean;
  private a = 0;
  player: Player;
  hand: Hand;
  opp: Player;
  oppHand: Hand;
  events: Events;

  constructor(round: number, day: boolean, first: boolean, p1: Player, h1: Hand, p2: Player, h2: Hand, events: Events) {
    // this.round = round;
    // this.day = day;
    // this.first = first;

    this.player = p1;
    this.hand = h1;

    this.opp = p2;
    this.oppHand = h2;

    this.events = events;
  }

  clone(p1: Player, h1: Hand, p2: Player, h2: Hand, events: Events): PlayerRound {
    // return new PlayerRound(this.round, this.day, this.first, p1, h1, p2, h2, events);
    return Object.setPrototypeOf({
      a: this.a,
      // round: this.round,
      // day: this.day,
      // first: this.first,
      player: p1,
      hand: h1,
      opp: p2,
      oppHand: h2,
      events
    }, PlayerRound.prototype)
  }

  get day() {
    return !!(this.a & 1);
  }
  set day(n: boolean) {
    this.a = (this.a & ~1) | +n;
  }
  get first() {
    return !!(this.a & 2);
  }
  set first(n: boolean) {
    this.a = (this.a & ~2) | (+n << 1);
  }
  get round() {
    return (this.a >> 2) & 0b111;
  }
  set round(n: number) {
    this.a = (this.a & ~0b11100) | (n & 0b111 << 2);
  }

  next(first: boolean) {
    this.round++;

    this.first = first;
  }

  getClanCards(card: Card, opp = false) {
    return (opp ? this.oppHand : this.hand).getClanCards(card);
  }
}