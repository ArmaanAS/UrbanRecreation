import Card from "../game/Card.ts";
import colors from "colors";
import {
  type Clan,
  ClanAbbreviations,
  type ClanId,
  ClanIdMap,
} from "../game/types/CardTypes.ts";
import Canvas from "./Canvas.ts";
import { splitLines } from "./Utils.ts";
import Game from "../game/Game.ts";
import Player from "../game/Player.ts";
import Hand from "../game/Hand.ts";
import { Turn } from "../game/types/Types.ts";

const CLAN_MARKER = "\uE000";
const clanMarker = (id: ClanId) =>
  CLAN_MARKER + String.fromCharCode(0xE100 + id);

function replaceClanTags(
  description: string,
  replacement: (clan: Clan | undefined, id: ClanId) => string,
) {
  return description.replace(
    /(?:\[clan:\d+\])+/gi,
    (group) =>
      Array.from(group.matchAll(/\[clan:(\d+)\]/gi), ([, rawId]) => {
        const id = Number(rawId) as ClanId;
        const clan = ClanIdMap[id];
        return replacement(clan, id);
      }).join("/"),
  );
}

/** Replace site-only clan image tags with fixed-width terminal labels. */
export function compactClanTags(description: string) {
  return replaceClanTags(
    description,
    (clan, id) => clan === undefined ? `C${id}` : ClanAbbreviations[clan],
  );
}

/** Preserve the identity of genuine clan tags while the plain text is line-wrapped. */
function markedClanTags(description: string) {
  return replaceClanTags(
    description,
    (clan, id) => clan === undefined ? `C${id}` : clanMarker(id),
  );
}

export default class GameRenderer {
  static draw(game: Game) {
    if (game.i2 !== undefined) {
      this.drawPlayer(game.p2, game.round);
      this.drawHand(game.h2, "cyan");
    } else {
      this.drawPlayer(game.p2, game.round);
      this.drawHand(game.h2, game.turn === Turn.PLAYER_2 ? "yellow" : "white");
    }

    if (game.i1 !== undefined) {
      this.drawPlayer(game.p1, game.round);
      this.drawHand(game.h1, "cyan");
    } else {
      this.drawPlayer(game.p1, game.round);
      this.drawHand(game.h1, game.turn === Turn.PLAYER_1 ? "yellow" : "white");
    }
  }

  static drawPlayer(p: Player, r: number) {
    const name = ` ${p.name} `.white.bgCyan.bold;
    const round = r.toString().green;
    const life = Math.max(p.life, 0).toString().red.bold;
    const pillz = Math.max(p.pillz, 0).toString().blue.bold;
    const bar = "[" +
      "O".repeat(p.pillz) +
      "-".repeat(Math.max(12 - p.pillz, 0)) +
      "]";
    console.log(
      `\n\n ${name}  ${"Round".green} | ${round}/${"4".green}    ${"Life".red} | ${life}    ${"Pillz".blue} | ${pillz}  ${bar.blue.bold}`,
    );
  }

  static drawHand(hand: Hand, col: keyof colors.Color = "cyan") {
    for (const line of this.handLines(hand, col)) console.log(" " + line);
  }

  /**
   * The complete original hand canvas as lines: four bordered 24x15 faces at the original
   * x positions on a 128-column canvas. The optional outer border belongs to the original
   * standalone renderer; fixed-screen callers can omit it without changing the cards.
   */
  static handLines(
    hand: readonly Card[],
    col: keyof colors.Color = "cyan",
    selected?: number,
    outerBorder = true,
    hovered?: number,
    choosing?: number,
  ) {
    const board = new Canvas(128, 17);
    board.col = col;

    hand.forEach((c, i) => {
      const isSelected = i === selected;
      const isHovered = i === hovered;
      const isChoosing = i === choosing;
      board.draw(
        3 + i * 32,
        0,
        GameRenderer.drawCard(c, isSelected, isHovered, isChoosing),
        isSelected || isHovered || isChoosing ? "double" : "single",
      );
    });
    return outerBorder
      ? board.borderedLines("rounded")
      : Array.from(board.lines);
  }

  static styledName(c: Card) {
    // Unison is presented as a green card treatment by the live game, independently of
    // the ordinary rarity value it sends (Musardine, for example, is still rarity "r").
    if (c.hasUnisonAbility) return ` ${c.name} `.bgGreen.black;

    switch (c.rarity) {
      case "c":
        return ` ${c.name} `.bgRed.white;
      case "u":
        return ` ${c.name} `.bgWhite.white.dim;
      case "r":
        return ` ${c.name} `.bgYellow.black;
      case "cr":
        return colors.bold(` ${c.name} `.bgYellow.white);
      case "l":
        return colors.bold(` ${c.name} `.white.bgMagenta);
        // case "m":
        //   return colors.bold(` ${c.name} `.bgBlue.white);
    }
  }

  static STYLES: Record<Clan, (s: string) => string> = {
    "All Stars": (s: string) => s.blue,
    Bangers: (s: string) => s.yellow.dim,
    Berzerk: (s: string) => s.red,
    Cosmohnuts: (s: string) => s.green.bgRed,
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
    Oblivion: (s: string) => s.gray.dim,
    Oculus: (s: string) => s.red.dim,
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
    Tolvack: (s: string) => s.cyan,
    "Ulu Watu": (s: string) => s.green,
    Uppers: (s: string) => s.green,
    Vortex: (s: string) => s.grey,
    Zenith: (s: string) => s.white.bgBlue,
  };

  static styledClan(c: Card) {
    return this.STYLES[c.clan]?.(c.clan) ?? c.clan.rainbow.strikethrough;
  }

  /**
   * Colour marked clan abbreviations independently of the surrounding ability/bonus.
   * The private two-character markers have the same width as their final abbreviations,
   * so splitLines() can still lay out the fixed-size card before ANSI styling is applied.
   */
  static styledDescriptionLine(
    line: string,
    baseColour: "blue" | "red" | "grey",
  ) {
    const base = (text: string) =>
      (baseColour === "blue"
        ? text.blue
        : baseColour === "red"
        ? text.red
        : text.grey).bgWhite;
    let out = "";
    let plain = "";
    const flush = () => {
      if (plain.length === 0) return;
      out += base(plain);
      plain = "";
    };

    for (let i = 0; i < line.length;) {
      if (line[i] !== CLAN_MARKER || i + 1 >= line.length) {
        plain += line[i++];
        continue;
      }
      const id = line.charCodeAt(i + 1) - 0xE100 as ClanId;
      const clan = ClanIdMap[id];
      if (clan === undefined) {
        plain += line[i++];
        continue;
      }

      flush();
      const abbreviation = ClanAbbreviations[clan];
      // Each segment owns its background. A clan style with its own background ends in
      // ANSI 49 (default background), so relying on one outer bgWhite would leave every
      // later code and the trailing padding transparent.
      out += (this.STYLES[clan]?.(abbreviation) ?? abbreviation).bgWhite;
      i += 2;
      // Only separators introduced between real clan tags receive the neutral colour.
      if (line[i] === "/" && line[i + 1] === CLAN_MARKER) {
        out += "/".grey.bgWhite;
        i++;
      }
    }
    flush();
    return out;
  }

  /** The original full card face as lines, for fixed-screen views as well as stdout. */
  static cardLines(card: Card, selected = false) {
    return this.drawCard(card, selected).borderedLines(
      selected ? "double" : "single",
    );
  }

  static drawCard(
    card: Card,
    selected = false,
    hovered = false,
    choosing = false,
  ) {
    const width = 24;
    const canvas = new Canvas(width, 15);

    if (card.won === true) {
      canvas.col = "green";
    } else if (card.won === false) {
      canvas.col = "red";
    } else if (selected || card.played) {
      canvas.col = "yellow";
    } else if (choosing) {
      canvas.col = "magenta";
    } else if (hovered) {
      canvas.col = "cyan";
    }

    const long = card.name.length >= 14;
    const pl = Math.floor((width - 2 - card.name.length) / 2) - 1 +
      (long ? -1 : 0);
    const pr = Math.ceil((width - 2 - card.name.length) / 2) - 4 +
      (long ? +1 : 0);
    let name = card.name.underline;
    name = " ".repeat(pl) +
      this.styledName(card) +
      " ".repeat(Math.max(0, pr)) +
      card.year.grey;

    canvas.write(0, name);

    const stars = " ★".bold.toString().repeat(card.stars) +
      // " $".bold.toString().repeat(card.stars) +
      " ☆".repeat(card.maxStars - card.stars) +
      " ";

    canvas.write(
      2,
      " ".repeat(width - 4 - card.maxStars * 2) +
        stars.yellow.bgMagenta.bold,
    );

    let power;
    if (card.power.final != card.power.base) {
      power =
        `${card.power.final.toString().blue.italic} ${card.power.base.toString().grey.strikethrough}`;
    } else {
      power = `${card.power.base.toString().blue}`;
    }
    let damage;
    if (card.damage.final != card.damage.base) {
      damage =
        `${card.damage.final.toString().red.italic} ${card.damage.base.toString().grey.strikethrough}`;
    } else {
      damage = `${card.damage.base.toString().red}`;
    }
    canvas.write(3, " " + " P ".white.bgBlue + ` ${power} `);
    canvas.write(4, " " + " D ".white.bgRed.bold + ` ${damage} `);

    // const a = splitLines(card.ability.string, width - 3, 3);
    // const b = splitLines(card.bonus.string, width - 3, 2);
    const a = splitLines(markedClanTags(card.abilityString), width - 3, 3);
    const b = splitLines(markedClanTags(card.bonusString), width - 3, 2);

    const acol = a[0].startsWith("No") ? "grey" : "blue";
    const bcol = b[0].startsWith("No") ? "grey" : "red";

    canvas.write(
      5,
      " ".repeat(width - 1 - " ability ".length) +
        " Ability ".white.bgCyan.underline.bold,
    );
    canvas.write(
      6,
      " " + this.styledDescriptionLine(" " + a[0], acol),
    );
    canvas.write(
      7,
      " " + this.styledDescriptionLine(" " + a[1], acol),
    );
    canvas.write(
      8,
      " " + this.styledDescriptionLine(" " + a[2], acol),
    );

    canvas.write(
      10,
      " ".repeat(width - 1 - " bonus ".length) +
        " Bonus ".white.bgRed.underline.bold,
    );
    canvas.write(
      11,
      " " + this.styledDescriptionLine(" " + b[0], bcol),
    );
    canvas.write(
      12,
      " " + this.styledDescriptionLine(" " + b[1], bcol),
    );
    // c.write(12, ' ' + b[2].red.bgWhite);

    canvas.write(14, ` ${"Clan".grey.bold} | ` + this.styledClan(card).bold);

    return canvas;
  }
}
