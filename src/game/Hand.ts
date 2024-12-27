import { CardJSON, HandOf } from "./types/CardTypes.ts";
import Card, { CardGenerator } from "./Card.ts";
import type { Clan } from "@/game/types/CardTypes.ts";

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

  static override from(o: Hand | HandOf<Card>): Hand {
    Card.from(o[0]);
    Card.from(o[1]);
    Card.from(o[2]);
    Card.from(o[3]);

    const hand: Hand = Object.setPrototypeOf(o, Hand.prototype);

    // Change Oculus cards clan
    // If only one other clan is present in the draw, the Oculus card is considered to be a card of that clan.
    // If two other clans are present, the Oculus card will belong to the clan of the sole card, thus activating its bonus.
    // If three other clans are present in the draw, or if you have more than one Oculus in your hand, the Infiltrated bonus has no effect.
    let oculus: Card | undefined;
    let oculusIndex: number | undefined;
    for (let i = 0; i < 4; i++) {
      const card = hand[i];
      if (card.baseClan === "Oculus") {
        if (oculus !== undefined) return hand;
        oculus = card;
        oculusIndex = i;
        break;
      }
    }

    // console.log({ oculusIndex });

    if (oculus) {
      // Count clans in hand
      const clansCount = new Map<Clan, number>();
      for (let i = 0; i < 4; i++) {
        if (i === oculusIndex) continue;
        const card = hand[i];
        if (card.baseClan !== "Oculus") {
          clansCount.set(card.clan, (clansCount.get(card.clan) ?? 0) + 1);
        }
      }

      // Find clan to Infiltrate
      let clan: Clan | undefined;
      if (clansCount.size === 1) {
        clan = clansCount.keys().next().value!;
      } else if (clansCount.size === 2) {
        for (const [newClan, count] of clansCount) {
          if (count === 1) {
            clan = newClan;
            break;
          }
        }
      }
      // console.log({ clan });

      if (clan) {
        oculus.clan = clan;
        oculus.bonusString = hand.find((c) => c.baseClan === clan)!.bonusString;
      }
    }

    return hand;
  }

  // get(index: number) {
  //   return this[index];
  // }

  getClanCards(card: Card) {
    let i = 0;
    if (this[0].clan === card.clan) i++;
    if (
      this[1].clan === card.clan &&
      this[1].name !== this[0].name
    ) i++;
    if (
      this[2].clan === card.clan &&
      this[2].name !== this[0].name &&
      this[2].name !== this[1].name
    ) i++;
    if (
      this[3].clan === card.clan &&
      this[3].name !== this[0].name &&
      this[3].name !== this[1].name &&
      this[3].name !== this[2].name
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

    // return Object.setPrototypeOf(hand, Hand.prototype);
    return Hand.from(hand);
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
