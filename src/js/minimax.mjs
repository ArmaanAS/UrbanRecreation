import colors from 'colors';

class Node {
  constructor(name = '', turn = true, score = null, defered = null) {
    this.name = name;
    this.turn = turn;

    this.defer = false;
    this.defered = defered;

    this.score = score;
    this.nodes = [];
  }

  add(name, turn = !this.turn, defered) {
    if (name instanceof Node) {
      this.nodes.push(name);
      return name;
    } else {
      let n = new Node(name, turn, undefined, defered);
      this.nodes.push(n);
      return n;
    }
  }

  get() {
    if (this.score != null) return this.score;
    else if (this.nodes.length) {
      if (this.turn == Minimax.MAX) {
        return Math.max(...this.nodes.map(n => n.get()));
      } else {
        return Math.min(...this.nodes.map(n => n.get()));
      }
    } else return Infinity;
  }

  win() {
    this.score = Minimax.WIN;
    return this;
  }

  tie() {
    this.score = Minimax.TIE;
    return this;
  }

  loss() {
    this.score = Minimax.LOSS;
    return this;
  }

  tree() {
    if (this.nodes.length) {
      return Object.fromEntries(this.nodes.map(n => [n.name, n.tree()]));
    } else {
      return this.score;
    }
  }

  get index() {
    return +this.name.split(' ', 1);
  }

  get pillz() {
    return +this.name.split(' ', 2)[1];
  }

  get fury() {
    return this.name.split(' ', 3)[2] == 'true';
  }

  toString() {
    let g = (this.get() * 100).toFixed(1);
    if (this.turn == Minimax.MIN) {
      if (g > 0) {
        return `[Win ${g}%] ${this.name}`.brightGreen;
      } else if (g < 0) {
        return `[Loss ${-g}%] ${this.name}`.brightRed;
      } else {
        return `[Draw] ${this.name}`.yellow;
      }
    } else {
      if (g > 0) {
        return `[Loss ${g}%] ${this.name}`.brightRed;
      } else if (g < 0) {
        return `[Win ${-g}%] ${this.name}`.brightGreen;
      } else {
        return `[Draw] ${this.name}`.yellow;
      }
    }
  }

  debug(depth = 2) {
    if (this.score != null) {
      return `S ${this.score}` + (this.break ? ' Break' : '');
    } else if (depth == 0) {
      return `G ${this.get()}`;
    } else {
      return Object.fromEntries([
        ...this.nodes.map(n => [n.name, n.debug(depth - 1)]),
        ['nodes', this.nodes.length],
        ['score', this.get()],
        ...(this.defer ? [
          ['max', this.turn],
          ['defer', true]
        ] : [
            ['max', this.turn]
          ])
      ]);
    }
  }
}

export default class Minimax extends Node {
  constructor(turn = Minimax.MAX, name = 'root') {
    super(name, turn);
  }

  best() {
    if (this.defer) {
      let combine = {};
      for (let m of this.nodes) {
        for (let n of m.nodes) {
          if (!combine[n.name]) {
            combine[n.name] = [];
          }
          combine[n.name].push(n);
        }
      }
      combine = Object.fromEntries(
        Object.entries(combine)
          .map(([k, v]) => [k, v.reduce((t, n) => t + n.get(), 0) / v.length])
      );
      console.log('combine', combine);
      let c = combine;
      combine = {};
      Object.entries(c).forEach(([k, v]) => {
        if (combine[v] == undefined) combine[v] = k;
      });
      console.log('combine', combine);
      console.log('Turn: ' + (!this.turn ? 'Max' : 'Min'));

      let s;
      if (this.turn == Minimax.MAX) {
        s = Math.min(...Object.keys(combine))
      } else {
        s = Math.max(...Object.keys(combine));
      }
      return new Node(combine[s], this.turn, +s);

    } else {
      console.log('Turn: ' + (this.turn ? 'Max' : 'Min'));
      let p = Infinity;
      let f = 2;
      if (this.turn == Minimax.MAX) {
        // return this.nodes.reduce((t, n) => n.get() > t.get() ? n : t);
        return this.nodes.reduce((t, n) => {
          let a = n.get();
          let b = t.get();
          if (a < b) {
            return t;
          } else if (a > b) {
            p = n.pillz;
            f = n.fury;
            return n;
          } else {
            if (a == b && (n.pillz < p) && (n.fury < f)) {
              p = n.pillz;
              f = n.fury;
              return n;
            }
            return t;
          }
        });
      } else {
        // return this.nodes.reduce((t, n) => n.get() < t.get() ? n : t);
        return this.nodes.reduce((t, n) => {
          let a = n.get();
          let b = t.get();
          if (a > b) {
            return t;
          } else if (a < b) {
            p = n.pillz;
            f = n.fury;
            return n;
          } else {
            if (a == b && (n.pillz < p) && (n.fury < f)) {
              p = n.pillz;
              f = n.fury;
              return n;
            }
            return t;
          }
        });
      }
    }
  }

  static from(m) {
    for (let n of m.nodes) {
      Minimax.from(n);
    }

    return Object.setPrototypeOf(m, Node.prototype);
  }

  static node(name) {
    return new Node(name);
  }


  static WIN = 1;
  static TIE = 0;
  static LOSS = -1;

  static MIN = false;
  static MAX = true;
}