import fs from "fs";
// import path from "path";
// const __dirname = path.resolve();
// import Ability from "./ability.mjs";
import Canvas from "./canvas.mjs";
import colors from "colors";
// import Hand from "./hand.mjs";
import {
  isMainThread
} from 'worker_threads';


let cbf = () => { };
let json,
  cardIds = {},
  cardNames = {},
  cardYears = {},
  cardClans = {},
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



function wordTree(lines) {
  let tree = {};

  for (let line of lines) {
    let node = tree;
    for (let word of line.split(" ")) {
      if (!node[word]) {
        node[word] = {};
      }

      node = node[word];
    }
    node["<"] = 0;
  }

  for (let [k, node] of Object.entries(tree)) {
    if (node["<"] != undefined && Object.keys(node).length == 1) {
      tree[k] = 0;
    }
  }

  return tree;
}

function logTree(tree, depth = 0) {
  Object.entries(tree).forEach(([k, v], i) => {
    if (depth == 0) {
      console.log(k);
    } else {
      console.log(" ".repeat(depth * 4) + "+ " + k);
    }
    logTree(v, depth + 1);
  });
}

function getN(arr, n = 1) {
  if (n <= 0) return [];
  if (n == 1) return [arr[(arr.length * Math.random()) | 0]];

  arr = Array.from(arr);

  if (arr.length <= n) return arr;

  let ret = [];
  while (n--) {
    ret.push(arr.splice((arr.length * Math.random()) | 0, 1)[0]);
  }

  return ret;
}

function splitLines(s, len, min = 0) {
  let lines;
  if (min > 0) {
    lines = new Array(min).fill(" ".repeat(len));
  } else {
    lines = [];
  }
  let words = s.trim().split(/(?<= )/g);

  let line = "",
    lineN = 0;
  for (let word of words) {
    if (line.length == 0) {
      line = word;
    } else if (line.length + word.length <= len) {
      line += word;
    } else {
      lines[lineN] = line + " ".repeat(Math.max(len - line.length, 0));
      line = word;
      lineN++;
    }
  }
  lines[lineN] = line + " ".repeat(Math.max(len - line.length, 0));

  return lines;
}

let blocked = function () {
  return !(this.prot || !this.cancel);
};

class CardStat {
  constructor(base) {
    this.base = base;
    this.final = base;
    this.cancel = false;
    this.prot = false;
  }

  clone() {
    let a = new CardStat(this.base);
    a.final = this.final;
    a.cancel = this.cancel;
    a.prot = this.prot;

    return a;
  }

  blocked() {
    return !(this.prot || !this.cancel);
  }
}

class CardString {
  constructor(string) {
    this.string = string;
    this.cancel = false;
    this.prot = false;
  }

  clone() {
    let a = new CardString(this.string);
    a.cancel = this.cancel;
    a.prot = this.prot;

    return a;
  }

  blocked() {
    return !(this.prot || !this.cancel);
  }
}

class CardAttr {
  constructor() {
    this.cancel = false;
    this.prot = false;
  }

  clone() {
    let a = new CardAttr();
    a.cancel = this.cancel;
    a.prot = this.prot;

    return a;
  }

  blocked() {
    return !(this.prot || !this.cancel);
  }
}

export default class Card {
  constructor(json) {
    this.index = -1;
    this.won = undefined;
    this.played = false;

    // this.json = json;
    this.name = json.name;
    this.id = json.id;
    this.stars = json.level;
    this.maxStars = json.level_max;
    this.release_date = json.release_date * 1000;
    this.clan = json.clan_name;
    this.rarity = json.rarity;

    this.power = new CardStat(json.power);
    this.damage = new CardStat(json.damage);
    this.ability = new CardString(json.ability);
    this.bonus = new CardString(json.bonus);

    this.attack = new CardStat(0);

    this.life = new CardAttr();
    this.pillz = new CardAttr();
  }

  clone() {
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

      power: this.power.clone(),
      damage: this.damage.clone(),
      ability: this.ability.clone(),
      bonus: this.bonus.clone(),

      attack: this.attack.clone(),

      life: this.life.clone(),
      pillz: this.pillz.clone(),
    },
      Card.prototype
    );
  }

  static from(o) {
    Object.setPrototypeOf(o, Card.prototype);

    Object.setPrototypeOf(o.power, CardStat.prototype);
    Object.setPrototypeOf(o.damage, CardStat.prototype);
    Object.setPrototypeOf(o.ability, CardString.prototype);
    Object.setPrototypeOf(o.bonus, CardString.prototype);

    Object.setPrototypeOf(o.attack, CardStat.prototype);

    Object.setPrototypeOf(o.life, CardAttr.prototype);
    Object.setPrototypeOf(o.pillz, CardAttr.prototype);

    return o;
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
        return ` ${this.name} `.bgBrightRed.brightWhite;
        break;
      case "u":
        return ` ${this.name} `.bgGrey.white;
        break;
      case "r":
        return ` ${this.name} `.bgYellow.black;
        break;
      case "cr":
        return ` ${this.name} `.bgYellow.brightWhite.bold;
        break;
      case "l":
        return ` ${this.name} `.brightWhite.bgMagenta.bold;
        break;
      case "m":
        return ` ${this.name} `.bgBrightBlue.white.bold;
        break;
    }
  }

  get styledClan() {
    let styles = {
      "All Stars": (s) => s.brightBlue,
      Bangers: (s) => s.yellow,
      Berzerk: (s) => s.brightRed,
      Dominion: (s) => s.magenta,
      "Fang Pi Clang": (s) => s.brightRed,
      Freaks: (s) => s.brightGreen,
      Frozn: (s) => s.brightCyan,
      GHEIST: (s) => s.red,
      GhosTown: (s) => s.brightBlue,
      Hive: (s) => s.brightYellow,
      Huracan: (s) => s.brightRed,
      Jungo: (s) => s.yellow,
      Junkz: (s) => s.brightYellow,
      Komboka: (s) => s.cyan,
      "La Junta": (s) => s.yellow,
      Leader: (s) => s.red,
      Montana: (s) => s.magenta,
      Nightmare: (s) => s.black,
      Paradox: (s) => s.magenta,
      Piranas: (s) => s.brightYellow,
      Pussycats: (s) => s.brightMagenta,
      Raptors: (s) => s.yellow,
      Rescue: (s) => s.brightYellow,
      Riots: (s) => s.yellow,
      Roots: (s) => s.green,
      Sakrohm: (s) => s.brightGreen,
      Sentinel: (s) => s.yellow,
      Skeelz: (s) => s.magenta,
      "Ulu Watu": (s) => s.brightGreen,
      Uppers: (s) => s.brightGreen,
      Vortex: (s) => s.grey,
    };

    let styler = styles[this.clan];
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
      c.col = "brightGreen";
    } else if (this.won == false) {
      c.col = "brightRed";
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
      " $".bold.repeat(this.stars) +
      " ☆".repeat(this.maxStars - this.stars) +
      " ";

    c.write(
      2,
      " ".repeat(width - 4 - this.maxStars * 2) +
      stars.brightYellow.bgBrightMagenta.bold
    );

    let power;
    if (this.power.final != this.power.base) {
      power = `${this.power.final.toString().brightBlue.italic} ${this.power.base.toString().grey.strikethrough
        }`;
    } else {
      power = `${this.power.base.toString().brightBlue}`;
    }
    let damage;
    if (this.damage.final != this.damage.base) {
      damage = `${this.damage.final.toString().red.italic} ${this.damage.base.toString().grey.strikethrough
        }`;
    } else {
      damage = `${this.damage.base.toString().red}`;
    }
    c.write(3, " " + " P ".white.bgBrightBlue + ` ${power} `);
    c.write(4, " " + " D ".white.bgBrightRed.bold + ` ${damage} `);

    let a = splitLines(this.ability.string, width - 3, 3);
    let b = splitLines(this.bonus.string, width - 3, 2);

    let acol = a[0].startsWith("No") ? "grey" : "brightBlue";
    let bcol = b[0].startsWith("No") ? "grey" : "brightRed";

    c.write(
      5,
      " ".repeat(width - 1 - " ability ".length) +
      " Ability ".brightWhite.bgCyan.underline.bold
    );
    c.write(6, " " + (" " + a[0])[acol].bgBrightWhite);
    c.write(7, " " + (" " + a[1])[acol].bgBrightWhite);
    c.write(8, " " + (" " + a[2])[acol].bgBrightWhite);

    c.write(
      10,
      " ".repeat(width - 1 - " bonus ".length) +
      " Bonus ".brightWhite.bgBrightRed.underline.bold
    );
    c.write(11, " " + (" " + b[0])[bcol].bgBrightWhite);
    c.write(12, " " + (" " + b[1])[bcol].bgBrightWhite);
    // c.write(12, ' ' + b[2].red.bgWhite);

    c.write(14, ` ${"Clan".grey.bold} | ` + this.styledClan.bold);

    return c;
  }

  static get(card) {
    let data = cardIds[card] || cardNames[card];
    if (data == undefined) return undefined;
    else return new Card(data);
  }

  static getRandomHandYear(year = 2006) {
    let card = getN(cardYears[year])[0];
    let cards = cardYears[year].filter((j) => j.clan_name == card.clan_name);

    return getN(cards, 4).map((c) => new Card(c));
  }

  static getRandomHandClan(clan) {
    let cards;
    if (clan) {
      cards = cardClans[clan];
    } else {
      cards = cardClans[getN(Object.keys(cardClans))[0]];
    }

    return getN(cards, 4).map((c) => new Card(c));
  }

  static getRandomHand(cards) {
    let cardsArr = [];
    for (let c of cards) {
      if (typeof c == "string") {
        cardsArr.push(new Card(cardNames[c.toLowerCase()]));
      } else if (typeof c == "number") {
        cardsArr.push(new Card(cardIds[c]));
      }
    }

    cardsArr.push(
      ...getN(Object.values(cardIds), 4 - cardsArr.length).map(
        (c) => new Card(c)
      )
    );

    return cardsArr;
  }
}

if (isMainThread || true) {
  let data = fs.readFileSync("./data.json");

  json = JSON.parse(data);
  console.log(
    json.length.toString().brightGreen + " cards loaded!".brightWhite
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