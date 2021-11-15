import CachedBattleData from "./CachedBattleData";
import CachedEvents from "./CachedEvents";
import Card from "../Card";
import Game from "../Game";
import Hand from "../Hand";
import CardBattle from "./CardBattle";

export default class CachedCardBattle {
  card1: Card;
  card2: Card;
  events1: CachedEvents;
  events2: CachedEvents;
  constructor(
    hand1: Hand,
    card1: Card,
    hand2: Hand,
    card2: Card,
  ) {
    this.card1 = card1.clone();
    this.card2 = card2.clone();

    this.events1 = new CachedEvents();
    this.events2 = new CachedEvents();

    // Compile the cards
    new CachedBattleData(hand1, this.card1, this.card2, this.events1);
    new CachedBattleData(hand2, this.card2, this.card1, this.events2);
  }

  play(
    game: Game,
    pillz1: number,
    fury1: boolean,
    pillz2: number,
    fury2: boolean,
  ) {
    // Merging the states
    this.events1.merge(game.events1);
    this.events2.merge(game.events2);

    // Clone cards
    const card1 = this.card1.clone();
    const card2 = this.card2.clone();

    // Place cloned and compiled cards into hands 
    game.h1[card1.index] = card1;
    game.h2[card2.index] = card2;


    new CardBattle(
      game, game.p1, card1, pillz1, fury1,
      game.p2, card2, pillz2, fury2,
      game.events1, game.events2, false
    );
  }
}