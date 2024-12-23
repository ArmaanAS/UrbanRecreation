import Ability from "../Ability.ts";
import CachedEvents from "./CachedEvents.ts";
import Card from "../Card.ts";
import Hand from "../Hand.ts";

export default class CachedBattleData {
  card: Card;
  oppCard: Card;
  events: CachedEvents;
  constructor(hand: Hand, card: Card, oppCard: Card, events: CachedEvents) {
    this.card = card;
    this.oppCard = oppCard;
    this.events = events;

    // Compile leader every round
    const l = hand.getLeader();
    if (l !== undefined) {
      Ability.leader(l, this);
    }

    // Compile currently played card if not leader
    if (this.card.clan != "Leader") {
      Ability.card(this.card, this);
    }
  }
}
