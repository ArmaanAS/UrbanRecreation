import { Turn } from "../game/types/Types";

// export enum GameResult {
//   WIN = 1,
//   TIE = 0,
//   LOSS = -1,
// }
export enum GameResult {
  PLAYER_1_WIN = 1,
  TIE = 0,
  PLAYER_2_WIN = -1,
}

export class Node {
  name: string;
  turn: Turn;
  average = false;
  playingSecond = false;
  defered: boolean;
  result?: GameResult; // This could be any number, e.g. 0.87832732451 !!...!!
  nodes: Node[] = [];
  break = false;
  constructor(
    name = '',
    turn: Turn = Turn.PLAYER_1,
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
  add(name: string, turn: Turn, defered: boolean): Node;
  add(name: string | Node, turn?: Turn, defered?: boolean) {
    if (name instanceof Node) {
      this.nodes.push(name);
      return name;
    } else {
      if (turn === undefined)
        turn = this.turn === Turn.PLAYER_1 ? Turn.PLAYER_2 : Turn.PLAYER_1;
      const n = new Node(name, turn, undefined, defered);
      this.nodes.push(n);
      return n;
    }
  }

  rating(minimax = true): number {
    if (this.result !== undefined)
      return this.result;
    else if (this.nodes.length) {
      if (!minimax) {
        this.average = true;
        let avg = 0;
        for (const n of this.nodes)
          avg += n.rating();

        return avg / this.nodes.length;

      } else if (this.turn === Minimax.MAX) {
        // return Math.max(...this.nodes.map(n => n.get()));
        let max = -Infinity;
        for (const n of this.nodes) {
          const s = n.rating();
          if (s > max)
            max = s
        }

        return max;
      } else {
        // return Math.min(...this.nodes.map(n => n.get()));
        let min = Infinity;
        for (const n of this.nodes) {
          const s = n.rating();
          if (s < min)
            min = s;
        }
        return min
      }
    } else return Infinity;
  }

  // win() {
  //   this.result = GameResult.WIN;
  //   return this;
  // }

  // tie() {
  //   this.result = GameResult.TIE;
  //   return this;
  // }

  // loss() {
  //   this.result = GameResult.LOSS;
  //   return this;
  // }

  tree(): number | object {
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

  get totalPillz() {
    return this.pillz + (this.fury ? 3 : 0);
  }

  toString() {
    const rating = this.rating();
    if (this.average &&
      !(this.turn === Minimax.MIN ? rating === 1 : rating === -1)) {
      let p1wins = 0,
        draws = 0,
        p2wins = 0;

      for (const n of this.nodes) {
        const rating = n.rating();
        if (rating === GameResult.PLAYER_1_WIN)
          p1wins++;
        else if (rating === GameResult.TIE)
          draws++;
        else if (rating === GameResult.PLAYER_2_WIN)
          p2wins++;
      }

      if (this.turn === Minimax.MIN) {
        const s = `[${p1wins} Win | ${draws} Draw | ${p2wins} Loss] ${this.name}`;
        // if (p1wins > p2wins && p1wins > draws)
        if (p2wins === 0 && draws === 0)
          return s.green;
        // else if (p2wins > p1wins && p2wins > draws)
        else if (p1wins === 0 && p2wins > 0)
          return s.red;
        else
          return s.yellow;
      } else {
        const s = `[${p2wins} Win | ${draws} Draw | ${p1wins} Loss] ${this.name}`;

        // if (p1wins > p2wins && p1wins > draws)
        //   return s.red;
        if (p1wins === 0 && draws === 0)
          return s.green;
        // else if (p2wins > p1wins && p2wins > draws)
        //   return s.green;
        else if (p2wins === 0 && p1wins > 0)
          return s.red;
        else
          return s.yellow;
      }

    } else {
      const g = +(rating * 100).toFixed(1);
      // if (this.turn === Minimax.MIN) {
      //   if (g > 0)
      //     return `[Win ${g}%] ${this.name}`.green;
      //   else if (g < 0)
      //     return `[Loss ${-g}%] ${this.name}`.red;
      //   else
      //     return `[Draw] ${this.name}`.yellow;
      // } else {
      //   if (g > 0)
      //     return `[Loss ${g}%] ${this.name}`.red;
      //   else if (g < 0)
      //     return `[Win ${-g}%] ${this.name}`.green;
      //   else
      //     return `[Draw] ${this.name}`.yellow;
      // }
      if (this.turn === Minimax.MIN) {
        if (g === 50)
          return `[50/50] ${this.name}`.yellow;
        else if (g === 0)
          return `[Loss] ${this.name}`.red;
        else if (g < 50)
          return `[Win ${g}%] ${this.name}`.red;
        else
          return `[Win ${g}%] ${this.name}`.green;
      } else {
        if (g === 50)
          return `[50/50] ${this.name}`.yellow;
        else if (g === 100)
          return `[Loss] ${this.name}`.red;
        else if (g < 50)
          return `[Win ${100 - g}%] ${this.name}`.green;
        else
          return `[Win ${100 - g}%] ${this.name}`.red;
      }
    }
  }

  debug(depth = 2): string | object {
    if (this.result !== undefined) {
      return `S ${this.result}` + (this.break ? ' Break' : '');
    } else if (depth === 0) {
      return `G ${this.rating()}`;
    } else {
      return Object.fromEntries([
        ...this.nodes.map(n => [n.name, n.debug(depth - 1)]),
        ['nodes', this.nodes.length],
        ['score', this.rating()],
        ...(this.playingSecond ? [
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
    if (this.playingSecond) { // TODO: Convert to sets and maps!! 
      const combine: { [key: string]: Node[] } = {};
      for (const m of this.nodes) {
        for (const n of m.nodes) {
          if (!combine[n.name])
            combine[n.name] = [];

          combine[n.name].push(n);
        }
      }

      // const combination = Object.fromEntries(
      //   Object.entries(combine)
      //     .map(([k, v]) => [k, v.reduce<number>(
      //       (t, n) => t + n.rating(), 0) / v.length])
      // );
      const combination: { [key: string]: number } = {};
      for (const [k, v] of Object.entries(combine)) {
        let total = 0;
        for (const n of v)
          total += n.rating();

        combination[k] = (total / v.length + 1) / 2;
      }
      console.log('combination', combination);

      const revCombo: { [key: number]: string } = {};
      for (const [k, v] of Object.entries(combination)) {
        if (revCombo[v] === undefined) {
          revCombo[v] = k;
        } else {
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
      if (this.turn === Minimax.MAX) {
        s = 2
        for (const i of Object.keys(revCombo))
          if (+i < s)
            s = +i

        // if (s > -1 && revCombo[0])
        //   console.log(`[DRAW] ${revCombo[0]}`.yellow)

      } else {
        s = -2
        for (const i of Object.keys(revCombo))
          if (+i > s)
            s = +i

        // if (s < 1 && revCombo[0])
        //   console.log(`[DRAW] ${revCombo[0]}`.yellow)
      }

      return new Node(revCombo[s], this.turn, s);

    } else {

      const combo: {
        [index: string]: { [index: number]: string } | number;
      } = {};
      for (const node of this.nodes) {
        console.info(node.toString());
        if (node.nodes.length === 0)
          combo[node.name] = node.rating();
        else {
          const childCombo: { [index: number]: string } = {};
          combo[node.name] = childCombo;

          for (const childNode of node.nodes) {
            childCombo[childNode.rating()] = childNode.name;
          }

          const keys = Object.keys(childCombo);
          if (keys.length === 1)
            combo[node.name] = keys[0];
        }
      }
      console.log("Combo");
      console.log(combo);

      // if (this.nodes.length === 0) throw new Error('Minimax nodes is empty');
      if (this.nodes.length === 0) return this;

      console.log('Turn: ' + (this.turn ? 'Max' : 'Min'));
      console.log('Child Turn: ' + (this.nodes[0].turn ? 'Max' : 'Min'));

      let bestNode = this.nodes[0];
      let minPillz = this.nodes[0].totalPillz;
      if (this.turn === Minimax.MAX) {

        let maxRating = this.nodes[0].rating(false);
        for (const node of this.nodes.slice(1)) {
          const a = node.rating(false);
          if (a > maxRating || (a === maxRating && node.totalPillz < minPillz)) {
            minPillz = node.totalPillz;
            maxRating = a;
            bestNode = node;
          }
        }

      } else {

        let minRating = this.nodes[0].rating(false);
        for (const node of this.nodes.slice(1)) {
          const a = node.rating(false);
          if (a < minRating || (a === minRating && node.totalPillz < minPillz)) {
            minPillz = node.totalPillz;
            minRating = a;
            bestNode = node;
          }
        }

      }

      return bestNode;
    }
  }

  static from(m: Node): Node {
    for (const n of m.nodes) {
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

  // static MIN = false;
  // static MAX = true;

  static MIN = 0;
  static MAX = 1;
}