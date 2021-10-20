// const colors = require('colors');
import colors from 'colors';

function insert(s, c, i, rep = 0) {
  // return s.substr(0, i) + c + s.substr(i + rep);
  return trim(s, i) + c + s.substr(trim(s, i + rep).length);
}

function getLength(s) {
  return s.replace(/\u001b\[\d\d?\d?m/g, '').length;
}

function trim(s, len) {
  while (getLength(s) > len) {
    if (/\u001b\[\d\d?\d?m$/.test(s)) {
      s = s.substr(0, s.lastIndexOf('\u001b['))
    } else {
      s = s.substr(0, s.length - 1);
    }
  }

  return s;
}


export default class Canvas {
  constructor(width, height) {
    this.width = width;
    this.height = height;
    this.lines = new Array(height)
      .fill('').map(() => '' + ' '.repeat(width) + '');
    this.col = 'brightWhite';
  }

  draw(x, y, canvas, border = false) {
    let i = Array.from(canvas.lines);

    if (border) {
      i.forEach((s, i, a) => a[i] = '║' [canvas.col] + s + '║' [canvas.col]);

      i.splice(0, 0, `╔${'═'.repeat(canvas.width)}╗` [canvas.col]);
      i.splice(i.length, 0, `╚${'═'.repeat(canvas.width)}╝` [canvas.col]);
    }

    for (let h = y; h < this.height && h >= 0 && (h - y) < i.length; h++) {
      this.write(x, h, i[h - y]);
    }

    return this;
  }

  write(x, y, t) {
    if (y >= this.height) return;
    if (t == undefined && typeof y == 'string') {
      t = y;
      y = x
      x = 0;
    }
    let length = getLength(t);
    if (length >= this.width - x) {
      length = this.width - x;
      t = trim(t, length);
    }
    this.lines[y] = insert(this.lines[y], t, x, length);

    return this;
  }

  log() {
    let i = Array.from(this.lines);

    i.forEach((s, i, a) => a[i] = '║' [this.col] + s + '║' [this.col]);

    i.splice(0, 0, `╔${'═'.repeat(this.width)}╗` [this.col]);
    i.splice(i.length, 0, `╚${'═'.repeat(this.width)}╝` [this.col]);

    for (let line of i) {
      console.log(' ' + line);
    }

    return this;
  }
}

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