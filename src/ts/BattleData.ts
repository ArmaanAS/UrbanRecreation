import Ability from "./Ability";
import Card from "./Card";
import Events from "./Events";
import Player from "./Player";
import PlayerRound from "./PlayerRound";

export default class BattleData {
  round: PlayerRound;
  player: Player;
  card: Card;
  playerPillzUsed: number;
  opp: Player;
  oppCard: Card;
  oppPillzUsed: number;
  events: Events;
  constructor(
    round: PlayerRound,
    p1: Player,
    card1: Card,
    pillz1: number,
    p2: Player,
    card2: Card,
    pillz2: number,
    events: Events
  ) {
    // round is necessary for conditions
    this.round = round;

    this.player = p1;
    this.card = card1;
    this.playerPillzUsed = pillz1;

    this.opp = p2;
    this.oppCard = card2;
    this.oppPillzUsed = pillz2;

    this.events = events;

    const l = round.hand.getLeader();
    if (l !== undefined)
      Ability.leader(l, this);

    if (this.card.clan != "Leader")
      Ability.card(card1, this);
  }

  getClanCards(card: Card, opp = false) {
    return this.round.getClanCards(card, opp);
  }
}