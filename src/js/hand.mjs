import Card from "./card.mjs";
import Canvas from "./canvas.mjs";

function unique(value, index, self) {
  return self.indexOf(value) === index;
}

export default class Hand {
  constructor(c1, c2, c3, c4) {
    if (c1) {
      this.cards = [c1, c2, c3, c4];
      this.cards.forEach((c, i) => (c.index = i));
    }
  }

  clone() {
    return Object.setPrototypeOf({
      cards: this.cards.map(c => c.clone())
    }, Hand.prototype);
  }

  static from(o) {
    Object.setPrototypeOf(o, Hand.prototype);

    o.cards.map(Card.from);

    return o;
  }

  get(index) {
    return this.cards[index];
  }

  draw(col = 'cyan') {
    let board = new Canvas(128, 17);
    board.col = col;

    this.cards.forEach((c, i) => {
      board.draw(3 + i * 32, 0, c.image(), true);
    });

    board.log();
  }

  getClanCards(card) {
    return this.cards
      .filter(c => c.clan == card.clan)
      .map(c => c.name)
      .filter(unique).length;
  }

  getLeader() {
    let leaders = this.cards.filter(c => c.clan == 'Leader');
    if (leaders.length == 1) {
      return leaders[0];
    } else {
      return;
    }
  }

  static generate(card) {
    if (card) {
      return new Hand(...Card.getRandomHand([...arguments].splice(0, 4)));
    } else {
      return new Hand(...Card.getRandomHandYear(2006));
      // return new Hand(...Card.getRandomHandClan());
    }
  }

  static generateRaw(cards) {
    return new Hand(...cards.map(j => new Card(j)));
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


// module.exports = Hand;