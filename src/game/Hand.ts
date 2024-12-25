import { CardJSON, HandOf } from "./types/CardTypes.ts";
import Card, { CardGenerator } from "./Card.ts";

// function unique(value: any, index: any, self: string | any[]) {
//   return self.indexOf(value) === index;
// }

export default class Hand extends Array<Card> {
  // constructor(cards: HandOf<Card>) {
  //   super(4);

  //   this[0] = cards[0];
  //   this[1] = cards[1];
  //   this[2] = cards[2];
  //   this[3] = cards[3];
  //   cards[0].index = 0;
  //   cards[1].index = 1;
  //   cards[2].index = 2;
  //   cards[3].index = 3;
  // }

  clone(): Hand {
    return Object.setPrototypeOf([
      this[0].won === undefined && !this[0].played ? this[0].clone() : this[0],
      this[1].won === undefined && !this[1].played ? this[1].clone() : this[1],
      this[2].won === undefined && !this[2].played ? this[2].clone() : this[2],
      this[3].won === undefined && !this[3].played ? this[3].clone() : this[3],
      // this[0].won === undefined ? this[0].clone() : this[0],
      // this[1].won === undefined ? this[1].clone() : this[1],
      // this[2].won === undefined ? this[2].clone() : this[2],
      // this[3].won === undefined ? this[3].clone() : this[3],
      // this[0].clone(),
      // this[1].clone(),
      // this[2].clone(),
      // this[3].clone(),
    ], Hand.prototype);
  }

  static override from(o: Hand) {
    Card.from(o[0]);
    Card.from(o[1]);
    Card.from(o[2]);
    Card.from(o[3]);

    return Object.setPrototypeOf(o, Hand.prototype);
  }

  // get(index: number) {
  //   return this[index];
  // }

  getClanCards(card: Card) {
    let i = 0;
    if (this[0].clan == card.clan) i++;
    if (
      this[1].clan == card.clan &&
      this[1].name != this[0].name
    ) i++;
    if (
      this[2].clan == card.clan &&
      this[2].name != this[0].name &&
      this[2].name != this[1].name
    ) i++;
    if (
      this[3].clan == card.clan &&
      this[3].name != this[0].name &&
      this[3].name != this[1].name &&
      this[3].name != this[2].name
    ) i++;

    return i;
  }

  getLeader() {
    // const leaders = this.filter(c => c.clan == 'Leader');
    // if (leaders.length == 1)
    //   return leaders[0];
    // else return;
    if (this[0].clan == "Leader") {
      if (
        this[1].clan == "Leader" ||
        this[2].clan == "Leader" ||
        this[3].clan == "Leader"
      ) return;

      return this[0];
    } else if (this[1].clan == "Leader") {
      if (
        this[2].clan == "Leader" ||
        this[3].clan == "Leader"
      ) return;

      return this[1];
    } else if (this[2].clan == "Leader") {
      if (this[3].clan == "Leader") return;

      return this[2];
    } else if (this[3].clan == "Leader") {
      return this[3];
    }

    return;
  }
}

export class HandGenerator {
  static from(hand: HandOf<Card>): Hand {
    hand[0].index = 0;
    hand[1].index = 1;
    hand[2].index = 2;
    hand[3].index = 3;

    return Object.setPrototypeOf(hand, Hand.prototype);
  }

  static generate(...cards: HandOf<string | number> | []) {
    if (cards.length === 4) {
      return this.from(CardGenerator.getRandomHand(cards));
    } else {
      return this.from(CardGenerator.getRandomHandYear(2006));
    }
  }

  static generateRaw(cards: HandOf<CardJSON>) {
    return this.from(cards.map((j) => new Card(j)) as HandOf<Card>);
  }

  static handOf(cards: HandOf<number | string>) {
    return this.from(cards.map((c) => {
      const card = CardGenerator.get(c);
      if (card !== undefined) {
        return card;
      } else {
        throw new Error(`Invalid card ID or Name: ${c}`);
      }
    }) as HandOf<Card>);
  }
}
