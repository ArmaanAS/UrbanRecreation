import fs from "fs";
import colors from "colors";
import { clone, getN, splitLines } from "./utils/Utils";
import Canvas from "./utils/Canvas";
import {
  CardJSON, Clan, Clans, HandOf, MaxStars, Rarity, Stars
} from "./types/Types";
import {
  isMainThread
} from 'worker_threads';


let cbf = () => { };
let json: CardJSON[],
  cardIds = {} as { [index: number]: CardJSON },
  cardNames = {} as { [index: string]: CardJSON },
  cardYears = {} as { [index: number]: CardJSON[] },
  cardClans = {} as { [Property in Clan]: CardJSON[] },
  abilities;


// fs.readFile(__dirname + "/data.json", (err, data) => {
//   if (err) {
//     throw err;
//   }

//   json = JSON.parse(data.toString());
//   console.log(
//     json.length.toString().brightGreen + " cards loaded!".brightWhite
//   );

//   cardIds = Object.fromEntries(json.map(i => [i.id, i]));
//   cardNames = Object.fromEntries(json.map(i => [i.name.toLowerCase(), i]));
//   cardYears = {};
//   cardClans = {};
//   json.forEach(i => {
//     let year = new Date(i.release_date * 1000).getFullYear();
//     if (!cardYears[year]) {
//       cardYears[year] = [];
//     }

//     cardYears[year].push(i);

//     let clan = i.clan_name;
//     if (!cardClans[clan]) {
//       cardClans[clan] = [];
//     }

//     cardClans[clan].push(i);
//   });

//   // let abilities = json.map(i => new Ability(i.ability));
//   // abilities = [...abilities, ...json.map(i => new Ability(i.bonus))]

//   // let tree = wordTree(abilities.map(i => i.ability.replace(/\d+/g, 'x')));
//   // console.log(JSON.stringify(tree));

//   // let conditions = {};
//   // for (let a of abilities) {
//   //   for (let c of a.conditions) {
//   //     conditions[c] = 0;
//   //   }
//   // }
//   // console.log(Object.keys(conditions));

//   Card.loaded = true;
//   console.log('Calling callback function: ');
//   console.log(cbf.toString());
//   // Card.cb();
//   cbf();
//   // loaded();
// });






// class CardAttr {
//   cancel: boolean = false;
//   prot: boolean = false;

//   // blocked() {
//   //   return !(this.prot || !this.cancel);
//   // }
// }

// class CardStat extends CardAttr {
//   base: number = 0;
//   final: number = 0;

//   constructor(base: number) {
//     super();

//     this.base = base;
//     this.final = base;
//   }
// }

// class CardString extends CardAttr {
//   string: string = '';
//   constructor(string: string) {
//     super();

//     this.string = string;
//   }
// }
interface CardAttr {
  cancel: boolean;
  prot: boolean;
}

interface CardStat extends CardAttr {
  base: number;
  final: number;
}

interface CardString extends CardAttr {
  string: string;
}

export default class Card {
  index: number = -1;
  won?: boolean = undefined;
  played: boolean = false;
  name: string;
  id: number;
  stars: Stars;
  maxStars: MaxStars;
  release_date: number;
  clan: Clan;
  rarity: Rarity;
  power: CardStat;
  damage: CardStat;
  ability: CardString;
  bonus: CardString;
  attack: CardStat;
  life: CardAttr;
  pillz: CardAttr;
  constructor(json: CardJSON) {
    // this.index = -1;
    // this.won = undefined;
    // this.played = false;

    this.name = json.name;
    this.id = json.id;
    this.stars = json.level;
    this.maxStars = json.level_max;
    this.release_date = json.release_date * 1000;
    this.clan = json.clan_name;
    this.rarity = json.rarity;

    // this.power = new CardStat(json.power);
    // this.damage = new CardStat(json.damage);
    // this.ability = new CardString(json.ability);
    // this.bonus = new CardString(json.bonus);

    // this.attack = new CardStat(0);

    // this.life = new CardAttr();
    // this.pillz = new CardAttr();

    this.power = {
      base: json.power, final: json.power, prot: false, cancel: false
    };
    this.damage = {
      base: json.damage, final: json.damage, prot: false, cancel: false
    };
    this.ability = { string: json.ability, prot: false, cancel: false };
    this.bonus = { string: json.bonus, prot: false, cancel: false };

    this.attack = { base: 0, final: 0, prot: false, cancel: false };

    this.life = { prot: false, cancel: false };
    this.pillz = { prot: false, cancel: false };
  }

  clone(): Card {
    return Object.setPrototypeOf({
      index: this.index,
      won: this.won,
      played: this.played,

      name: this.name,
      id: this.id,
      stars: this.stars,
      maxStars: this.maxStars,
      release_date: this.release_date,
      clan: this.clan,
      rarity: this.rarity,

      // power: this.power.clone(),
      // damage: this.damage.clone(),
      // ability: this.ability.clone(),
      // bonus: this.bonus.clone(),

      // attack: this.attack.clone(),

      // life: this.life.clone(),
      // pillz: this.pillz.clone(),
      // power: this.power.clone(),
      damage: clone(this.damage),
      ability: clone(this.ability),
      bonus: clone(this.bonus),

      attack: clone(this.attack),

      life: clone(this.life),
      pillz: clone(this.pillz),
      power: clone(this.power),
    }, Card.prototype);
  }

  static from(o: Card) {
    return Object.setPrototypeOf(o, Card.prototype);

    // Object.setPrototypeOf(o.power, CardStat.prototype);
    // Object.setPrototypeOf(o.damage, CardStat.prototype);
    // Object.setPrototypeOf(o.ability, CardString.prototype);
    // Object.setPrototypeOf(o.bonus, CardString.prototype);

    // Object.setPrototypeOf(o.attack, CardStat.prototype);

    // Object.setPrototypeOf(o.life, CardAttr.prototype);
    // Object.setPrototypeOf(o.pillz, CardAttr.prototype);

    // return o;
  }

  getStars() {
    return this.stars;
  }

  getName() {
    return this.name;
  }

  getAbility() {
    return this.ability.string;
  }

  getBonus() {
    return this.bonus.string;
  }

  getClan() {
    return this.clan;
  }

  get year() {
    return new Date(this.release_date).getFullYear().toString();
  }

  get styledName() {
    // let rarity = {
    //   c: 'bgBrightRed',
    //   u: 'bgGrey',
    //   r: 'bgYellow',
    //   cr: 'bgBrightYellow',
    //   l: 'bgMagenta',
    //   m: 'bgBlue'
    // };

    switch (this.rarity) {
      case "c":
        return ` ${this.name} `.bgRed.white;
      case "u":
        return ` ${this.name} `.bgWhite.dim.white;
      case "r":
        return ` ${this.name} `.bgYellow.black;
      case "cr":
        return colors.bold(` ${this.name} `.bgYellow.white);
      case "l":
        return colors.bold(` ${this.name} `.white.bgMagenta);
      case "m":
        return colors.bold(` ${this.name} `.bgBlue.white);
    }
  }

  static styles: {
    [Property in Clan]: (s: String) => string
  } = {
      "All Stars": (s: string) => s.blue,
      Bangers: (s: string) => s.yellow.dim,
      Berzerk: (s: string) => s.red,
      Dominion: (s: string) => s.magenta.dim,
      "Fang Pi Clang": (s: string) => s.red,
      Freaks: (s: string) => s.green,
      Frozn: (s: string) => s.cyan,
      GHEIST: (s: string) => s.red.dim,
      GhosTown: (s: string) => s.blue,
      Hive: (s: string) => s.yellow,
      Huracan: (s: string) => s.red,
      Jungo: (s: string) => s.yellow.dim,
      Junkz: (s: string) => s.yellow,
      Komboka: (s: string) => s.cyan.dim,
      "La Junta": (s: string) => s.yellow,
      Leader: (s: string) => s.red,
      Montana: (s: string) => s.magenta.dim,
      Nightmare: (s: string) => s.black.dim,
      Paradox: (s: string) => s.magenta.dim,
      Piranas: (s: string) => s.yellow,
      Pussycats: (s: string) => s.magenta,
      Raptors: (s: string) => s.yellow.dim,
      Rescue: (s: string) => s.yellow,
      Riots: (s: string) => s.yellow.dim,
      Roots: (s: string) => s.green.dim,
      Sakrohm: (s: string) => s.green,
      Sentinel: (s: string) => s.yellow.dim,
      Skeelz: (s: string) => s.magenta.dim,
      "Ulu Watu": (s: string) => s.green,
      Uppers: (s: string) => s.green,
      Vortex: (s: string) => s.grey,
    };
  get styledClan() {

    let styler = Card.styles[this.clan];
    if (styler) {
      return styler(this.clan);
    } else {
      return this.clan.rainbow.strikethrough;
    }
  }

  image() {
    let width = 24;
    let c = new Canvas(width, 15);
    if (this.won == true) {
      c.col = "green";
    } else if (this.won == false) {
      c.col = "red";
    } else if (this.played) {
      c.col = "yellow";
    }

    let long = this.name.length >= 14;
    let pl =
      Math.floor((width - 2 - this.name.length) / 2) - 1 + (long ? -1 : 0);
    let pr =
      Math.ceil((width - 2 - this.name.length) / 2) - 4 + (long ? +1 : 0);
    let name = this.name.underline;
    name =
      " ".repeat(pl) +
      this.styledName +
      " ".repeat(Math.max(0, pr)) +
      this.year.grey;

    c.write(0, name);

    let stars =
      /*' ★'*/
      " $".bold.toString().repeat(this.stars) +
      " ☆".repeat(this.maxStars - this.stars) +
      " ";

    c.write(
      2,
      " ".repeat(width - 4 - this.maxStars * 2) +
      stars.yellow.bgMagenta.bold
    );

    let power;
    if (this.power.final != this.power.base) {
      power = `${this.power.final.toString().blue.italic} ${this.power.base.toString().grey.strikethrough
        }`;
    } else {
      power = `${this.power.base.toString().blue}`;
    }
    let damage;
    if (this.damage.final != this.damage.base) {
      damage = `${this.damage.final.toString().red.italic} ${this.damage.base.toString().grey.strikethrough
        }`;
    } else {
      damage = `${this.damage.base.toString().red}`;
    }
    c.write(3, " " + " P ".white.bgBlue + ` ${power} `);
    c.write(4, " " + " D ".white.bgRed.bold + ` ${damage} `);

    let a = splitLines(this.ability.string, width - 3, 3);
    let b = splitLines(this.bonus.string, width - 3, 2);

    const acol = a[0].startsWith("No") ? "grey" : "blue";
    const bcol = b[0].startsWith("No") ? "grey" : "red";

    c.write(
      5,
      " ".repeat(width - 1 - " ability ".length) +
      " Ability ".white.bgCyan.underline.bold
    );
    c.write(6, " " + (" " + a[0])[acol].bgWhite);
    c.write(7, " " + (" " + a[1])[acol].bgWhite);
    c.write(8, " " + (" " + a[2])[acol].bgWhite);

    c.write(
      10,
      " ".repeat(width - 1 - " bonus ".length) +
      " Bonus ".white.bgRed.underline.bold
    );
    c.write(11, " " + (" " + b[0])[bcol].bgWhite);
    c.write(12, " " + (" " + b[1])[bcol].bgWhite);
    // c.write(12, ' ' + b[2].red.bgWhite);

    c.write(14, ` ${"Clan".grey.bold} | ` + this.styledClan.bold);

    return c;
  }

  static get(card: number | string) {
    let data: CardJSON | undefined;
    if (typeof card == 'number')
      data = cardIds[card];
    else
      data = cardNames[card];

    if (data == undefined) return undefined;
    else return new Card(data);
  }

  static getRandomHandYear(year = 2006) {
    let card = getN(cardYears[year])[0];
    let cards = cardYears[year].filter((j) => j.clan_name == card.clan_name);

    return getN(cards, 4).map((c) => new Card(c)) as HandOf<Card>;
  }

  static getRandomHandClan(clan?: Clan) {
    let cards: CardJSON[];
    if (clan !== undefined) {
      cards = cardClans[clan];
    } else {
      // cards = cardClans[getN(Object.keys(cardClans) as Clan[])[0]];
      cards = cardClans[getN(Clans as unknown as Clan[])[0]];
    }

    return getN(cards, 4).map((c) => new Card(c));
  }

  static getRandomHand(cards: HandOf<string | number>) {
    let cardsArr = cards.map(c => {
      if (typeof c == "string")
        return new Card(cardNames[c.toLowerCase()]);
      else
        return new Card(cardIds[c]);
    }) as HandOf<Card>;

    cardsArr.push(
      ...getN(Object.values(cardIds), 4 - cardsArr.length).map(
        (c) => new Card(c)
      )
    );

    return cardsArr;
  }
}

if (isMainThread || true) {
  let data = fs.readFileSync("../../data/data.json").toString();

  json = JSON.parse(data);
  console.log(
    json.length.toString().green + " cards loaded!".white
  );

  cardIds = Object.fromEntries(json.map(i => [i.id, i]));
  cardNames = Object.fromEntries(json.map(i => [i.name.toLowerCase(), i]));
  json.forEach(i => {
    let year = new Date(i.release_date * 1000).getFullYear();
    if (!cardYears[year]) {
      cardYears[year] = [];
    }

    cardYears[year].push(i);

    let clan = i.clan_name;
    if (!cardClans[clan]) {
      cardClans[clan] = [];
    }

    cardClans[clan].push(i);
  });
}


// let abilities = json.map(i => new Ability(i.ability));
// abilities = [...abilities, ...json.map(i => new Ability(i.bonus))]

// let tree = wordTree(abilities.map(i => i.ability.replace(/\d+/g, 'x')));
// console.log(JSON.stringify(tree));

// let conditions = {};
// for (let a of abilities) {
//   for (let c of a.conditions) {
//     conditions[c] = 0;
//   }
// }
// console.log(Object.keys(conditions));

// Card.loaded = true;
// console.log('Calling callback function: ');
// console.log(Card.cb.toString());
// Card.cb();


// module.exports = Card;