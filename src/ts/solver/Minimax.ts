import colors from 'colors';

export enum GameResult {
  WIN = 1,
  TIE = 0,
  LOSS = -1,
}

type Turn = boolean
export const Turn = {
  PLAYER: true,
  OPPONENT: false,
} as const

export class Node {
  name: string;
  turn: Turn;
  playSecond: boolean = false;
  defered: boolean;
  result?: GameResult; // This could be any number, e.g. 0.87832732451 !!...!!
  nodes: Node[] = [];
  break = false;
  constructor(
    name = '',
    turn: Turn = Turn.PLAYER,
    result?: GameResult,
    defered = false
  ) {
    this.name = name;
    this.turn = turn;

    // this.playSecond = false;
    this.defered = defered;

    this.result = result;
    // this.nodes = [];
  }

  add(name: Node): Node;
  add(name: string, turn?: boolean, defered?: boolean): Node;
  add(name: string | Node, turn = !this.turn, defered?: boolean) {
    if (name instanceof Node) {
      this.nodes.push(name);
      return name;
    } else {
      let n = new Node(name, turn, undefined, defered);
      this.nodes.push(n);
      return n;
    }
  }

  get(): number {
    if (this.result !== undefined)
      return this.result;
    else if (this.nodes.length) {
      if (this.turn == Minimax.MAX) {
        // return Math.max(...this.nodes.map(n => n.get()));
        let max = -Infinity;
        let s: number;
        for (const n of this.nodes) {
          s = n.get();
          if (s > max)
            max = s
        }

        return max;
      } else {
        // return Math.min(...this.nodes.map(n => n.get()));
        let min = Infinity;
        let s: number;
        for (const n of this.nodes) {
          s = n.get();
          if (s < min)
            min = s;
        }
        return min
      }
    } else return Infinity;
  }

  win() {
    this.result = GameResult.WIN;
    return this;
  }

  tie() {
    this.result = GameResult.TIE;
    return this;
  }

  loss() {
    this.result = GameResult.LOSS;
    return this;
  }

  tree(): number | Object {
    if (this.nodes.length) {
      return Object.fromEntries(this.nodes.map(n => [n.name, n.tree()]));
    } else {
      return this.result ?? -1;
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
    let g = +(this.get() * 100).toFixed(1);
    if (this.turn == Minimax.MIN) {
      if (g > 0) {
        return `[Win ${g}%] ${this.name}`.green;
      } else if (g < 0) {
        return `[Loss ${-g}%] ${this.name}`.red;
      } else {
        return `[Draw] ${this.name}`.yellow.dim;
      }
    } else {
      if (g > 0) {
        return `[Loss ${g}%] ${this.name}`.red;
      } else if (g < 0) {
        return `[Win ${-g}%] ${this.name}`.green;
      } else {
        return `[Draw] ${this.name}`.yellow.dim;
      }
    }
  }

  debug(depth = 2): string | object {
    if (this.result !== undefined) {
      return `S ${this.result}` + (this.break ? ' Break' : '');
    } else if (depth == 0) {
      return `G ${this.get()}`;
    } else {
      return Object.fromEntries([
        ...this.nodes.map(n => [n.name, n.debug(depth - 1)]),
        ['nodes', this.nodes.length],
        ['score', this.get()],
        ...(this.playSecond ? [
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
    if (this.playSecond) {
      const combine: { [key: string]: Node[] } = {};
      for (let m of this.nodes) {
        for (let n of m.nodes) {
          if (!combine[n.name])
            combine[n.name] = [];

          combine[n.name].push(n);
        }
      }

      const combination = Object.fromEntries(
        Object.entries(combine)
          .map(([k, v]) => [k, v.reduce<number>(
            (t, n) => t + n.get(), 0) / v.length])
      );
      console.log('combination', combination);

      const revCombo: { [key: number]: string } = {};
      for (const [k, v] of Object.entries(combination)) {
        if (!revCombo.hasOwnProperty(v))
          revCombo[v] = k;
        else {
          const split1 = k.split(' ', 3)
          const pillz1 = +split1[1] + (split1[2] == 'true' ? 3 : 0)
          const split2 = revCombo[v].split(' ', 3)
          const pillz2 = +split2[1] + (split2[2] == 'true' ? 3 : 0)

          if (pillz1 < pillz2)
            revCombo[v] = k
        }
      }

      console.log('revCombo', revCombo);
      console.log('Turn: ' + (!this.turn ? 'Max' : 'Min'));

      let s: number;
      if (this.turn == Minimax.MAX) {
        // s = Math.min(...Object.keys(revCombo))
        s = 2
        for (const i of Object.keys(revCombo))
          if (+i < s)
            s = +i

      } else {
        // s = Math.max(...Object.keys(revCombo));
        s = -2
        for (const i of Object.keys(revCombo))
          if (+i > s)
            s = +i
      }

      return new Node(revCombo[s], this.turn, s);

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
            f = +n.fury;
            return n;
          } else {
            if (a == b && (n.pillz < p) && (+n.fury < f)) {
              p = n.pillz;
              f = +n.fury;
              return n;
            }
            return t;
          }
        });
        // let max: Node;
        // for (const n of this.nodes) {
        //   let a = n.get();
        // }
      } else {
        // return this.nodes.reduce((t, n) => n.get() < t.get() ? n : t);
        return this.nodes.reduce((t, n) => {
          let a = n.get();
          let b = t.get();
          if (a > b) {
            return t;
          } else if (a < b) {
            p = n.pillz;
            f = +n.fury;
            return n;
          } else {
            if (a == b && (n.pillz < p) && (+n.fury < f)) {
              p = n.pillz;
              f = +n.fury;
              return n;
            }
            return t;
          }
        });
      }
    }
  }

  static from(m: Node): Node {
    for (let n of m.nodes) {
      Minimax.from(n);
    }

    return Object.setPrototypeOf(m, Node.prototype);
  }

  static node(name: string) {
    return new Node(name);
  }


  // static WIN = 1;
  // static TIE = 0;
  // static LOSS = -1;

  static MIN = false;
  static MAX = true;
}