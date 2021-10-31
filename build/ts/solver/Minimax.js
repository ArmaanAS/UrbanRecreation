import { Turn } from "../types/Types";
export var GameResult;
(function (GameResult) {
    GameResult[GameResult["PLAYER_1_WIN"] = 1] = "PLAYER_1_WIN";
    GameResult[GameResult["TIE"] = 0] = "TIE";
    GameResult[GameResult["PLAYER_2_WIN"] = -1] = "PLAYER_2_WIN";
})(GameResult || (GameResult = {}));
export class Node {
    constructor(name = '', turn = Turn.PLAYER_1, result, defered = false) {
        this.playSecond = false;
        this.nodes = [];
        this.break = false;
        this.name = name;
        this.turn = turn;
        this.defered = defered;
        this.result = result;
    }
    add(name, turn = !this.turn, defered) {
        if (name instanceof Node) {
            this.nodes.push(name);
            return name;
        }
        else {
            const n = new Node(name, turn, undefined, defered);
            this.nodes.push(n);
            return n;
        }
    }
    get() {
        if (this.result !== undefined)
            return this.result;
        else if (this.nodes.length) {
            if (this.turn == Minimax.MAX) {
                let max = -Infinity;
                let s;
                for (const n of this.nodes) {
                    s = n.get();
                    if (s > max)
                        max = s;
                }
                return max;
            }
            else {
                let min = Infinity;
                let s;
                for (const n of this.nodes) {
                    s = n.get();
                    if (s < min)
                        min = s;
                }
                return min;
            }
        }
        else
            return Infinity;
    }
    tree() {
        var _a;
        if (this.nodes.length) {
            return Object.fromEntries(this.nodes.map(n => [n.name, n.tree()]));
        }
        else {
            return (_a = this.result) !== null && _a !== void 0 ? _a : -1;
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
        const g = +(this.get() * 100).toFixed(1);
        if (this.turn == Minimax.MIN) {
            if (g > 0) {
                return `[Win ${g}%] ${this.name}`.green;
            }
            else if (g < 0) {
                return `[Loss ${-g}%] ${this.name}`.red;
            }
            else {
                return `[Draw] ${this.name}`.yellow.dim;
            }
        }
        else {
            if (g > 0) {
                return `[Loss ${g}%] ${this.name}`.red;
            }
            else if (g < 0) {
                return `[Win ${-g}%] ${this.name}`.green;
            }
            else {
                return `[Draw] ${this.name}`.yellow.dim;
            }
        }
    }
    debug(depth = 2) {
        if (this.result !== undefined) {
            return `S ${this.result}` + (this.break ? ' Break' : '');
        }
        else if (depth == 0) {
            return `G ${this.get()}`;
        }
        else {
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
            const combine = {};
            for (const m of this.nodes) {
                for (const n of m.nodes) {
                    if (!combine[n.name])
                        combine[n.name] = [];
                    combine[n.name].push(n);
                }
            }
            const combination = Object.fromEntries(Object.entries(combine)
                .map(([k, v]) => [k, v.reduce((t, n) => t + n.get(), 0) / v.length]));
            console.log('combination', combination);
            const revCombo = {};
            for (const [k, v] of Object.entries(combination)) {
                if (revCombo[v] === undefined) {
                    revCombo[v] = k;
                }
                else {
                    const split1 = k.split(' ', 3);
                    const pillz1 = +split1[1] + (split1[2] == 'true' ? 3 : 0);
                    const split2 = revCombo[v].split(' ', 3);
                    const pillz2 = +split2[1] + (split2[2] == 'true' ? 3 : 0);
                    if (pillz1 < pillz2)
                        revCombo[v] = k;
                }
            }
            console.log('revCombo', revCombo);
            console.log('Turn: ' + (!this.turn ? 'Max' : 'Min'));
            let s;
            if (this.turn == Minimax.MAX) {
                s = 2;
                for (const i of Object.keys(revCombo))
                    if (+i < s)
                        s = +i;
            }
            else {
                s = -2;
                for (const i of Object.keys(revCombo))
                    if (+i > s)
                        s = +i;
            }
            return new Node(revCombo[s], this.turn, s);
        }
        else {
            console.log('Turn: ' + (this.turn ? 'Max' : 'Min'));
            let p = Infinity;
            let f = 2;
            if (this.turn == Minimax.MAX) {
                return this.nodes.reduce((t, n) => {
                    const a = n.get();
                    const b = t.get();
                    if (a < b) {
                        return t;
                    }
                    else if (a > b) {
                        p = n.pillz;
                        f = +n.fury;
                        return n;
                    }
                    else {
                        if (a == b && (n.pillz < p) && (+n.fury < f)) {
                            p = n.pillz;
                            f = +n.fury;
                            return n;
                        }
                        return t;
                    }
                });
            }
            else {
                return this.nodes.reduce((t, n) => {
                    const a = n.get();
                    const b = t.get();
                    if (a > b) {
                        return t;
                    }
                    else if (a < b) {
                        p = n.pillz;
                        f = +n.fury;
                        return n;
                    }
                    else {
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
    static from(m) {
        for (const n of m.nodes) {
            Minimax.from(n);
        }
        return Object.setPrototypeOf(m, Node.prototype);
    }
    static node(name) {
        return new Node(name);
    }
}
Minimax.MIN = false;
Minimax.MAX = true;
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiTWluaW1heC5qcyIsInNvdXJjZVJvb3QiOiIvQzovVXNlcnMvU3R1ZGVudC9Eb2N1bWVudHMvTm9kZUpTV29ya3NwYWNlL1VyYmFuUmVjcmVhdGlvbi9zcmMvIiwic291cmNlcyI6WyJ0cy9zb2x2ZXIvTWluaW1heC50cyJdLCJuYW1lcyI6W10sIm1hcHBpbmdzIjoiQUFBQSxPQUFPLEVBQUUsSUFBSSxFQUFFLE1BQU0sZ0JBQWdCLENBQUM7QUFPdEMsTUFBTSxDQUFOLElBQVksVUFJWDtBQUpELFdBQVksVUFBVTtJQUNwQiwyREFBZ0IsQ0FBQTtJQUNoQix5Q0FBTyxDQUFBO0lBQ1AsNERBQWlCLENBQUE7QUFDbkIsQ0FBQyxFQUpXLFVBQVUsS0FBVixVQUFVLFFBSXJCO0FBRUQsTUFBTSxPQUFPLElBQUk7SUFRZixZQUNFLElBQUksR0FBRyxFQUFFLEVBQ1QsT0FBYSxJQUFJLENBQUMsUUFBUSxFQUMxQixNQUFtQixFQUNuQixPQUFPLEdBQUcsS0FBSztRQVRqQixlQUFVLEdBQUcsS0FBSyxDQUFDO1FBR25CLFVBQUssR0FBVyxFQUFFLENBQUM7UUFDbkIsVUFBSyxHQUFHLEtBQUssQ0FBQztRQU9aLElBQUksQ0FBQyxJQUFJLEdBQUcsSUFBSSxDQUFDO1FBQ2pCLElBQUksQ0FBQyxJQUFJLEdBQUcsSUFBSSxDQUFDO1FBR2pCLElBQUksQ0FBQyxPQUFPLEdBQUcsT0FBTyxDQUFDO1FBRXZCLElBQUksQ0FBQyxNQUFNLEdBQUcsTUFBTSxDQUFDO0lBRXZCLENBQUM7SUFJRCxHQUFHLENBQUMsSUFBbUIsRUFBRSxJQUFJLEdBQUcsQ0FBQyxJQUFJLENBQUMsSUFBSSxFQUFFLE9BQWlCO1FBQzNELElBQUksSUFBSSxZQUFZLElBQUksRUFBRTtZQUN4QixJQUFJLENBQUMsS0FBSyxDQUFDLElBQUksQ0FBQyxJQUFJLENBQUMsQ0FBQztZQUN0QixPQUFPLElBQUksQ0FBQztTQUNiO2FBQU07WUFDTCxNQUFNLENBQUMsR0FBRyxJQUFJLElBQUksQ0FBQyxJQUFJLEVB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