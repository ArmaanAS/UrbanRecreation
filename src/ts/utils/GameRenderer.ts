import Card from "../Card";
import colors from 'colors'
import { Clan } from "../types/Types";
import Canvas from "./Canvas";
import { splitLines } from "./Utils";
import Game from "../Game";
import Player from "../Player";
import Hand from "../Hand";

export default class GameRenderer {
  static draw(game: Game, override = false) {
    if (!override && !game.logs) return;

    if (game.i1 != undefined) {
      this.drawPlayer(game.p1, game.round.round);
      this.drawHand(game.h1, "cyan");
    } else {
      this.drawPlayer(game.p1, game.round.round);
      this.drawHand(game.h1,
        game.selectedFirst != game.round.first ? "yellow" : "white");
    }

    if (game.i2 != undefined) {
      this.drawPlayer(game.p2, game.round.round);
      this.drawHand(game.h2, "cyan");
    } else {
      this.drawPlayer(game.p2, game.round.round);
      this.drawHand(game.h2,
        game.selectedFirst != game.round.first ? "white" : "yellow");
    }
  }

  static drawPlayer(p: Player, r: number) {
    const name = ` ${p.name} `.white.bgCyan.bold;
    const round = r.toString().green;
    const life = Math.max(p.life, 0).toString().red.bold;
    const pillz = Math.max(p.pillz, 0).toString().blue.bold;
    const bar =
      "[" +
      "#".repeat(p.pillz) +
      "-".repeat(Math.max(12 - p.pillz, 0)) +
      "]";
    console.log(
      `\n\n ${name}  ${"Round".green} | ${round}/${"4".green}    ${"Life".red
      } | ${life}    ${"Pillz".blue} | ${pillz}  ${bar.blue.bold}`
    );
  }

  static drawHand(hand: Hand, col: keyof colors.Color = 'cyan') {
    const board = new Canvas(128, 17);
    board.col = col;

    // hand.cards.forEach((c, i) => {
    hand.forEach((c, i) => {
      board.draw(3 + i * 32, 0, GameRenderer.drawCard(c), true);
    });

    board.print();
  }

  static styledName(c: Card) {
    switch (c.rarity) {
      case "c": return ` ${c.name} `.bgRed.white;
      case "u": return ` ${c.name} `.bgWhite.dim.white;
      case "r": return ` ${c.name} `.bgYellow.black;
      case "cr": return colors.bold(` ${c.name} `.bgYellow.white);
      case "l": return colors.bold(` ${c.name} `.white.bgMagenta);
      case "m": return colors.bold(` ${c.name} `.bgBlue.white);
    }
  }

  static STYLES: {
    [Property in Clan]: (s: string) => string
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

  static styledClan(c: Card) {
    return this.STYLES[c.clan]?.(c.clan) ?? c.clan.rainbow.strikethrough;
  }

  static drawCard(card: Card) {
    const width = 24;
    const canvas = new Canvas(width, 15);

    if (card.won === true)
      canvas.col = "green";
    else if (card.won === false)
      canvas.col = "red";
    else if (card.played)
      canvas.col = "yellow";


    const long = card.name.length >= 14;
    const pl =
      Math.floor((width - 2 - card.name.length) / 2) - 1 + (long ? -1 : 0);
    const pr =
      Math.ceil((width - 2 - card.name.length) / 2) - 4 + (long ? +1 : 0);
    let name = card.name.underline;
    name =
      " ".repeat(pl) +
      this.styledName(card) +
      " ".repeat(Math.max(0, pr)) +
      card.year.grey;

    canvas.write(0, name);

    const stars =
      /*' ★'*/
      " $".bold.toString().repeat(card.stars) +
      " ☆".repeat(card.maxStars - card.stars) +
      " ";

    canvas.write(
      2,
      " ".repeat(width - 4 - card.maxStars * 2) +
      stars.yellow.bgMagenta.bold
    );

    let power;
    if (card.power.final != card.power.base) {
      power = `${card.power.final.toString().blue.italic} ${card.power.base.toString().grey.strikethrough
        }`;
    } else {
      power = `${card.power.base.toString().blue}`;
    }
    let damage;
    if (card.damage.final != card.damage.base) {
      damage = `${card.damage.final.toString().red.italic} ${card.damage.base.toString().grey.strikethrough
        }`;
    } else {
      damage = `${card.damage.base.toString().red}`;
    }
    canvas.write(3, " " + " P ".white.bgBlue + ` ${power} `);
    canvas.write(4, " " + " D ".white.bgRed.bold + ` ${damage} `);

    const a = splitLines(card.ability.string, width - 3, 3);
    const b = splitLines(card.bonus.string, width - 3, 2);

    const acol = a[0].startsWith("No") ? "grey" : "blue";
    const bcol = b[0].startsWith("No") ? "grey" : "red";

    canvas.write(
      5,
      " ".repeat(width - 1 - " ability ".length) +
      " Ability ".white.bgCyan.underline.bold
    );
    canvas.write(6, " " + (" " + a[0])[acol].bgWhite);
    canvas.write(7, " " + (" " + a[1])[acol].bgWhite);
    canvas.write(8, " " + (" " + a[2])[acol].bgWhite);

    canvas.write(
      10,
      " ".repeat(width - 1 - " bonus ".length) +
      " Bonus ".white.bgRed.underline.bold
    );
    canvas.write(11, " " + (" " + b[0])[bcol].bgWhite);
    canvas.write(12, " " + (" " + b[1])[bcol].bgWhite);
    // c.write(12, ' ' + b[2].red.bgWhite);

    canvas.write(14, ` ${"Clan".grey.bold} | ` + this.styledClan(card).bold);

    return canvas;
  }
}