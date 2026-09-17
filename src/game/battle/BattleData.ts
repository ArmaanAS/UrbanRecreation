import Ability from "../Ability.ts";
import Card from "../Card.ts";
import Events from "./Events.ts";
import Player from "../Player.ts";
import PlayerRound from "../PlayerRound.ts";

export default class BattleData {
  round: PlayerRound;
  player: Player;
  card: Card;
  playerPillzUsed: number;
  opp: Player;
  oppCard: Card;
  oppPillzUsed: number;
  /**
   * What a `Bet > N Pillz` condition compares against: the Pillz staked plus the free
   * one, with Fury's three excluded. `pillz1` arrives with Fury already folded in, since
   * the Attack calculation and every Per Pillz effect want the full cost.
   */
  betPillz: number;
  events: Events;
  constructor(
    round: PlayerRound,
    p1: Player,
    card1: Card,
    pillz1: number,
    p2: Player,
    card2: Card,
    pillz2: number,
    events: Events,
    compile = true,
    fury1 = false,
  ) {
    // round is necessary for conditions
    this.round = round;

    this.player = p1;
    this.card = card1;
    this.playerPillzUsed = pillz1;
    this.betPillz = pillz1 - (fury1 ? 3 : 0) + 1;

    this.opp = p2;
    this.oppCard = card2;
    this.oppPillzUsed = pillz2;

    this.events = events;

    if (compile) {
      const l = this.round.hand.getLeader();
      if (l !== undefined) {
        Ability.leader(l, this);
      }

      if (this.card.clan != "Leader") {
        Ability.card(this.card, this);
      }
    }
  }
}
