import Card from "./Card.ts";
import Hand from "./Hand.ts";
import { type Clan } from "./types/CardTypes.ts";

export default class PlayerRound {
  private a = 0;
  hand: Hand;
  oppHand: Hand;
  /**
   * Clan of the card this player played in the previous round, which is what
   * "After [clan:...]" asks about (the server calls it previousClanRequirement).
   * Undefined in round 0, where there is no previous card.
   */
  lastClan?: Clan;

  constructor(
    round: number,
    first: boolean,
    h1: Hand,
    h2: Hand,
    day = true,
  ) {
    this.round = round;
    this.day = day;
    this.first = first;

    this.hand = h1;
    this.oppHand = h2;
  }

  /** Day, first and round are one packed int; `lastClan` is saved alongside it by `Undo`. */
  snapshot(): number {
    return this.a;
  }
  restore(a: number) {
    this.a = a;
  }

  /** Twice per search node; Object.create beats re-prototyping a literal by ~1600x. */
  clone(h1: Hand, h2: Hand): PlayerRound {
    const r: PlayerRound = Object.create(PlayerRound.prototype);
    r.a = this.a;
    r.hand = h1;
    r.oppHand = h2;
    r.lastClan = this.lastClan;
    return r;
  }

  get day() {
    return !!(this.a & 0b1);
  }
  set day(n: boolean) {
    this.a = (this.a & ~0b1) | +n;
  }
  get first() {
    return !!(this.a & 0b10);
  }
  set first(n: boolean) {
    this.a = (this.a & ~0b10) | (+n << 1);
  }
  get round() {
    return (this.a >> 2) & 0b111;
  }
  set round(n: number) {
    this.a = (this.a & ~0b11100) | ((n & 0b111) << 2);
  }

  next(first: boolean) {
    this.round++;

    this.first = first;
  }

  getClanCards(card: Card, opp = false) {
    return (opp ? this.oppHand : this.hand).getClanCards(card);
  }
}
