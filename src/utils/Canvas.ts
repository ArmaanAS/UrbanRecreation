// deno-lint-ignore-file no-control-regex
import colors from "colors";

function insert(s: string, c: string | number | object, i: number, rep = 0) {
  // return s.substr(0, i) + c + s.substr(i + rep);
  return trim(s, i) + c + s.substring(trim(s, i + rep).length);
}

function getLength(s: string) {
  return s.replace(/\u001b\[\d\d?\d?m/g, "").length;
}

function trim(s: string, len: number) {
  while (getLength(s) > len) {
    if (/\u001b\[\d\d?\d?m$/.test(s)) {
      s = s.substring(0, s.lastIndexOf("\u001b["));
    } else {
      s = s.substring(0, s.length - 1);
    }
  }

  return s;
}

export default class Canvas {
  col: keyof colors.Color;
  width: number;
  height: number;
  lines: string[];
  constructor(width: number, height: number) {
    this.width = width;
    this.height = height;
    this.lines = new Array(height)
      .fill("").map(() => "" + " ".repeat(width) + "");
    this.col = "white";
  }

  draw(
    x: number,
    y: number,
    canvas: Canvas,
    border: boolean | BorderStyle = false,
  ) {
    const i = Array.from(canvas.lines);

    if (border) {
      const chars = BORDERS[border === true ? "double" : border];
      i.forEach((s, i, a) =>
        a[i] = chars.vertical[canvas.col] + s + chars.vertical[canvas.col]
      );

      i.splice(
        0,
        0,
        `${chars.topLeft}${
          chars.horizontal.repeat(canvas.width)
        }${chars.topRight}`[
          canvas.col
        ] as string,
      );
      i.splice(
        i.length,
        0,
        `${chars.bottomLeft}${
          chars.horizontal.repeat(canvas.width)
        }${chars.bottomRight}`[
          canvas.col
        ] as string,
      );
    }

    for (let h = y; h < this.height && h >= 0 && (h - y) < i.length; h++) {
      this.write(x, h, i[h - y]);
    }

    return this;
  }

  write(x: number, y: string | number, t?: string) {
    if (+y >= this.height) return;
    if (t === undefined && typeof y == "string") {
      t = y;
      y = x;
      x = 0;
    }
    let length = getLength(t as string);
    if (length >= this.width - x) {
      const clipped = length > this.width - x;
      length = this.width - x;
      t = trim(t as string, length);
      // trim() necessarily removes trailing colour-closing sequences before printable
      // characters. Restore a full reset so clipped backgrounds cannot bleed past a
      // canvas (the long raw [clan:...] requirements exposed this on card faces).
      if (clipped && (t as string).includes("\u001b[")) t += "\u001b[0m";
    }
    this.lines[y as number] = insert(
      this.lines[y as number],
      t!,
      x,
      length,
    );

    return this;
  }

  print() {
    for (const line of this.borderedLines()) {
      console.log(" " + line);
    }
  }

  /** The same bordered output as `print()`, without writing it to stdout. */
  borderedLines(style: BorderStyle = "double"): string[] {
    const chars = BORDERS[style];
    const lines = Array.from(this.lines);
    lines.forEach((s, i, a) =>
      a[i] = chars.vertical[this.col] + s + chars.vertical[this.col]
    );
    lines.splice(
      0,
      0,
      `${chars.topLeft}${chars.horizontal.repeat(this.width)}${chars.topRight}`[
        this.col
      ] as string,
    );
    lines.push(
      `${chars.bottomLeft}${
        chars.horizontal.repeat(this.width)
      }${chars.bottomRight}`[
        this.col
      ] as string,
    );
    return lines;
  }
}

export type BorderStyle = "double" | "single" | "rounded";

const BORDERS: Record<BorderStyle, {
  topLeft: string;
  topRight: string;
  bottomLeft: string;
  bottomRight: string;
  horizontal: string;
  vertical: string;
}> = {
  double: {
    topLeft: "╔",
    topRight: "╗",
    bottomLeft: "╚",
    bottomRight: "╝",
    horizontal: "═",
    vertical: "║",
  },
  single: {
    topLeft: "┌",
    topRight: "┐",
    bottomLeft: "└",
    bottomRight: "┘",
    horizontal: "─",
    vertical: "│",
  },
  rounded: {
    topLeft: "╭",
    topRight: "╮",
    bottomLeft: "╰",
    bottomRight: "╯",
    horizontal: "─",
    vertical: "│",
  },
};

// im = new Canvas(25, 10);
// for (let i in new Array(10).fill()) {
//   im.write(i, i % 10, `${i} text`);
// }
// im.log();

// c = new Canvas(125, 15);
// c.draw(1, 1, im);
// c.draw(10, 1, im);
// c.log();

// module.exports = Canvas;
