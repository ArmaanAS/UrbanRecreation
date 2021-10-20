import Canvas from "./utils/Canvas";
import colors from 'colors'
import { HandOf, CardJSON } from "./types/Types";
import Card from "./Card";

// function unique(value: any, index: any, self: string | any[]) {
//   return self.indexOf(value) === index;
// }

export default class Hand {
  cards: HandOf<Card>;
  constructor(cards: HandOf<Card>) {
    this.cards = cards;
    // this.cards.forEach((c, i) => (c.index = i));
    cards[0].index = 0;
    cards[1].index = 1;
    cards[2].index = 2;
    cards[3].index = 3;
  }

  clone(): Hand {
    // return Object.setPrototypeOf({
    //   cards: this.cards.map(c => c.clone())
    // }, Hand.prototype);
    return Object.setPrototypeOf({
      cards: [
        this.cards[0].clone(),
        this.cards[1].clone(),
        this.cards[2].clone(),
        this.cards[3].clone(),
      ]
    }, Hand.prototype);
  }

  static from(o: Hand) {
    Object.setPrototypeOf(o, Hand.prototype);

    // o.cards.map(Card.from);
    Card.from(o.cards[0]);
    Card.from(o.cards[1]);
    Card.from(o.cards[2]);
    Card.from(o.cards[3]);

    return o;
  }

  get(index: number) {
    return this.cards[index];
  }

  draw(col: keyof colors.Color = 'cyan') {
    let board = new Canvas(128, 17);
    board.col = col;

    this.cards.forEach((c, i) => {
      board.draw(3 + i * 32, 0, c.image(), true);
    });

    board.log();
  }

  getClanCards(card: Card) {
    // return this.cards
    //   .filter(c => c.clan == card.clan)
    //   .map(c => c.name)
    //   .filter(unique).length;
    let i = 0;
    if (this.cards[0].clan == card.clan) i++;
    if (this.cards[1].clan == card.clan &&
      this.cards[1].name != this.cards[0].name) i++;
    if (this.cards[2].clan == card.clan &&
      this.cards[2].name != this.cards[0].name &&
      this.cards[2].name != this.cards[1].name) i++;
    if (this.cards[3].clan == card.clan &&
      this.cards[3].name != this.cards[0].name &&
      this.cards[3].name != this.cards[1].name &&
      this.cards[3].name != this.cards[2].name) i++;

    return i;
  }

  getLeader() {
    let leaders = this.cards.filter(c => c.clan == 'Leader');
    if (leaders.length == 1)
      return leaders[0];
    else return;
  }

  static generate(...cards: HandOf<string> | []) {
    if (cards.length === 4) {
      return new Hand(Card.getRandomHand(cards));
    } else {
      return new Hand(Card.getRandomHandYear(2006));
      // return new Hand(...Card.getRandomHandClan());
    }
  }

  static generateRaw(cards: HandOf<CardJSON>) {
    return new Hand(cards.map(j => new Card(j)) as HandOf<Card>);
  }

  // static of (c1, c2, c3, c4) {
  //   cards = [];
  //   for (c of [c1, c2, c3, c4]) {
  //     card = Card.get(c);
  //     if (card) {
  //       cards.push(card);
  //     } else {
  //       throw new Error("Invalid card ID or Name: " + c);
  //     }
  //   }

  //   return new Hand(...cards);
  // }
}