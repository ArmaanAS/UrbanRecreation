import Hand from "./Hand";
import readline from "readline";
import { clone } from "./utils/Utils";
import Player from "./Player";
import Round from "./Round";
let rl;
export default class Game {
    constructor(p1, p2, h1, h2, inputs, logs = true, repeat, first = true) {
        this.round = new Round(1, true, first, p1, h1, p2, h2);
        this.inputs = inputs;
        this.logs = logs;
        this.p1 = p1;
        this.p2 = p2;
        this.h1 = h1;
        this.h2 = h2;
        this.selectedFirst = false;
        for (let hand of [h1, h2]) {
            for (let card of hand.cards) {
                if (card.clan == "Leader") {
                    if (hand.getClanCards(card) > 1) {
                        card.ability.string = "No Ability";
                    }
                }
                else {
                    if (hand.getClanCards(card) == 1) {
                        card.bonus.string = "No Bonus";
                    }
                }
            }
        }
        this.log();
        if (inputs) {
            this.input(repeat);
        }
    }
    clone(inputs, logs) {
        const p1 = clone(this.p1);
        const p2 = clone(this.p2);
        const h1 = this.h1.clone();
        const h2 = this.h2.clone();
        return Object.setPrototypeOf({
            round: this.round.clone(p1, h1, p2, h2),
            inputs: inputs !== null && inputs !== void 0 ? inputs : this.inputs,
            logs: logs !== null && logs !== void 0 ? logs : this.logs,
            winner: this.winner,
            p1, p2, h1, h2,
            selectedFirst: this.selectedFirst,
            i1: this.i1,
            i2: this.i2,
        }, Game.prototype);
    }
    static from(o, inputs, logs) {
        Object.setPrototypeOf(o, Game.prototype);
        o.round = Round.from(o.round);
        Object.setPrototypeOf(o.p1, Player.prototype);
        Object.setPrototypeOf(o.p2, Player.prototype);
        o.h1 = Hand.from(o.h1);
        o.h2 = Hand.from(o.h2);
        if (inputs !== undefined)
            o.inputs = inputs;
        if (logs !== undefined)
            o.logs = logs;
        return o;
    }
    log(override = false) {
        if (!override && !this.logs)
            return;
        if (this.i1 != undefined) {
            this.p1.log(this.round.round);
            this.h1.draw("cyan");
        }
        else {
            this.p1.log(this.round.round);
            this.h1.draw(this.selectedFirst != this.round.first ? "yellow" : "white");
        }
        if (this.i2 != undefined) {
            this.p2.log(this.round.round);
            this.h2.draw("cyan");
        }
        else {
            this.p2.log(this.round.round);
            this.h2.draw(this.selectedFirst != this.round.first ? "white" : "yellow");
        }
    }
    select(index, pillz, fury = false) {
        if (typeof index != 'number' || typeof pillz != 'number')
            throw new Error(`Game.select - index or pillz is not a number 
        index: ${index}, pillz: ${pillz}`);
        if (this.selectedFirst != this.round.first) {
            if (this.h1.get(index).played) {
                return false;
            }
            this.i1 = [index, pillz, fury];
            this.h1.get(index).played = true;
        }
        else {
            if (this.h2.get(index).played) {
                return false;
            }
            this.i2 = [index, pillz, fury];
            this.h2.get(index).played = true;
        }
        if (this.selectedFirst) {
            this.battle();
            this.i1 = undefined;
            this.i2 = undefined;
        }
        this.selectedFirst = !this.selectedFirst;
        this.log();
        return true;
    }
    battle() {
        if (this.i1 != undefined && this.i2 != undefined) {
            this.round.battle(...this.i1, ...this.i2);
            if (this.p1.life <= 0 && this.p2.life <= 0) {
                return 'Tie';
            }
            else if (this.p1.life <= 0) {
                this.winner = this.p2.name;
            }
            else if (this.p2.life <= 0) {
                this.winner = this.p1.name;
            }
            else if (this.round.round >= 5) {
                if (this.p1.life > this.p2.life) {
                    this.winner = this.p1.name;
                }
                else if (this.p1.life < this.p2.life) {
                    this.winner = this.p2.name;
                }
                else {
                    this.winner = "Tie";
                }
            }
        }
        return;
    }
    input(repeat = true) {
        if (!rl) {
            rl = readline.createInterface({
                input: process.stdin,
                output: process.stdout,
            });
        }
        let msg = `
         _____      _           _                       _ 
        /  ___|    | |         | |                     | |
        \\ \`--.  ___| | ___  ___| |_    ___ __ _ _ __ __| |
         \`--. \\/ _ \\ |/ _ \\/ __| __|  / __/ _\` | '__/ _\` |
        /\\__/ /  __/ |  __/ (__| |_  | (_| (_| | | | (_| |
        \\____/ \\___|_|\\___|\\___|\\__|  \\___\\__,_|_|  \\__,_| o o o
                                                                                                                                                
    \n`;
        return new Promise((resolve, reject) => {
            rl.question(msg.green, async (answer) => {
                var _a;
                console.log(`Selected ${answer}`);
                let s = answer.trim().split(' ');
                if (!this.select(+s[0], +((_a = s[1]) !== null && _a !== void 0 ? _a : 0), s[2] == 'true')) {
                    resolve(await this.input(repeat));
                }
                else {
                    if (this.checkWinner(repeat)) {
                        console.log("\n\nClosing readline...");
                        rl.close();
                        resolve(s);
                    }
                    else {
                        if (repeat) {
                            resolve(await this.input(true));
                        }
                        else {
                            resolve(s);
                        }
                    }
                }
            });
        });
    }
    checkWinner(log = false) {
        if (this.round.round > 4 || this.p1.life <= 0 || this.p2.life <= 0) {
            if (log) {
                this.log();
                console.log("\n Game over!\n".white.bgRed);
                if (this.p1.life > this.p2.life) {
                    console.log(` ${` ${this.p1.name} `.white.bgCyan} won the match!\n`);
                }
                else if (this.p1.life < this.p2.life) {
                    console.log(` ${` ${this.p2.name} `.white.bgCyan} won the match!\n`);
                }
                else {
                    console.log("Game is a draw!\n".green);
                }
            }
            return true;
        }
        else {
            return false;
        }
    }
    getTurn() {
        return this.selectedFirst != this.round.first ? 'Player' : 'Urban Rivals';
    }
    static create(inputs = true, logs, repeat) {
        let p1 = new Player(12, 12, "Player");
        let p2 = new Player(12, 12, "Urban Rival");
        let h1 = Hand.generate('Roderick', 'Frank', 'Katsuhkay', 'Oyoh');
        let h2 = Hand.generate('Behemoth Cr', 'Vholt', 'Eyrik', 'Kate');
        return new Game(p1, p2, h1, h2, inputs, logs, repeat);
    }
    static createUnique(h1, h2, life, pillz, name1, name2, first) {
        let p1 = new Player(life, pillz, name1);
        let p2 = new Player(life, pillz, name2);
        return new Game(p1, p2, Hand.generateRaw(h1), Hand.generateRaw(h2), false, false, false, first);
    }
}
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiR2FtZS5qcyIsInNvdXJjZVJvb3QiOiIvQzovVXNlcnMvU3R1ZGVudC9Eb2N1bWVudHMvTm9kZUpTV29ya3NwYWNlL1VyYmFuUmVjcmVhdGlvbi9zcmMvIiwic291cmNlcyI6WyJ0cy9HYW1lLnRzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUFBLE9BQU8sSUFBSSxNQUFNLFFBQVEsQ0FBQztBQUUxQixPQUFPLFFBQVEsTUFBTSxVQUFVLENBQUM7QUFFaEMsT0FBTyxFQUFFLEtBQUssRUFBRSxNQUFNLGVBQWUsQ0FBQztBQUN0QyxPQUFPLE1BQU0sTUFBTSxVQUFVLENBQUM7QUFDOUIsT0FBTyxLQUFLLE1BQU0sU0FBUyxDQUFDO0FBRTVCLElBQUksRUFBa0MsQ0FBQztBQUd2QyxNQUFNLENBQUMsT0FBTyxPQUFPLElBQUk7SUFhdkIsWUFDRSxFQUFVLEVBQUUsRUFBVSxFQUFFLEVBQVEsRUFBRSxFQUFRLEVBQzFDLE1BQWUsRUFBRSxJQUFJLEdBQUcsSUFBSSxFQUFFLE1BQTJCLEVBQ3pELEtBQUssR0FBRyxJQUFJO1FBRVosSUFBSSxDQUFDLEtBQUssR0FBRyxJQUFJLEtBQUssQ0FBQyxDQUFDLEVBQUUsSUFBSSxFQUFFLEtBQUssRUFBRSxFQUFFLEVBQUUsRUFBRSxFQUFFLEVBQUUsRUFBRSxFQUFFLENBQUMsQ0FBQztRQUN2RCxJQUFJLENBQUMsTUFBTSxHQUFHLE1BQU0sQ0FBQztRQUNyQixJQUFJLENBQUMsSUFBSSxHQUFHLElBQUksQ0FBQztRQUVqQixJQUFJLENBQUMsRUFBRSxHQUFHLEVBQUUsQ0FBQztRQUNiLElBQUksQ0FBQyxFQUFFLEdBQUcsRUFBRSxDQUFDO1FBR2IsSUFBSSxDQUFDLEVBQUUsR0FBRyxFQUFFLENBQUM7UUFDYixJQUFJLENBQUMsRUFBRSxHQUFHLEVBQUUsQ0FBQztRQUViLElBQUksQ0FBQyxhQUFhLEdBQUcsS0FBSyxDQUFDO1FBSTNCLEtBQUssSUFBSSxJQUFJLElBQUksQ0FBQyxFQUFFLEVBQUUsRUFBRSxDQUFDLEVB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