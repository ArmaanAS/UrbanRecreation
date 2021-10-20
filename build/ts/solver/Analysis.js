import Game from "../Game";
import { shiftRange } from '../utils/Utils';
import Bar from "./Bar";
import Minimax, { GameResult } from "./Minimax";
import PromiseMap from '../utils/PromiseMap';
import DistributedAnalysis from "./DistributedAnalysis";
export default class Analysis {
    constructor(game, inputs = false, logs = false, clone = true) {
        if (!clone)
            this.game = game;
        else if (game instanceof Game)
            this.game = game.clone(inputs, logs);
        else
            this.game = Game.from(game, inputs, logs);
    }
    getTurnCards() {
        return this.game.selectedFirst != this.game.round.first ?
            this.game.h1.cards :
            this.game.h2.cards;
    }
    getTurnPlayer() {
        return this.game.selectedFirst != this.game.round.first ?
            this.game.p1 :
            this.game.p2;
    }
    getCardIndex() {
        if (this.game.selectedFirst != this.game.round.first) {
            if (this.game.i2 === undefined)
                throw new Error('this.game.i2 == undefined');
            return this.game.i2[0];
        }
        else {
            if (this.game.i1 === undefined)
                throw new Error('this.game.i1 == undefined');
            return this.game.i1[0];
        }
    }
    getCardHand() {
        return this.game.selectedFirst != this.game.round.first ?
            this.game.h2 :
            this.game.h1;
    }
    getCardIndexes() {
        return this.getTurnCards()
            .filter((c) => !c.played)
            .map((c) => c.index);
    }
    getTurn() {
        return this.game.selectedFirst != this.game.round.first;
    }
    deselect() {
        if (this.game.selectedFirst) {
            let i = this.getCardIndex();
            this.getCardHand().get(i).played = false;
            this.game.selectedFirst = false;
            return i;
        }
        return;
    }
    static async fillTree(game, minimax = new Minimax(), bar = new Bar(), index, pillz, fury = false, fullSearch = false) {
        let a = new Analysis(game);
        if (typeof minimax == "string") {
            minimax = Minimax.node(minimax);
        }
        let indexes, i;
        if (minimax.name != "root") {
            a.game.select(index, pillz, fury);
            if (!a.game.selectedFirst && a.game.winner) {
                if (a.game.winner == "Player") {
                    return minimax.win();
                }
                else if (a.game.winner == "Tie") {
                    return minimax.tie();
                }
                else if (a.game.winner == "Urban Rival") {
                    return minimax.loss();
                }
                throw new Error(`Unknown winner: "${a.game.winner}"`);
            }
        }
        else {
            i = a.deselect();
        }
        if (i != undefined) {
            minimax.playSecond = true;
            indexes = [i];
        }
        else {
            indexes = a.getCardIndexes();
        }
        minimax.turn = a.getTurn();
        if (bar)
            bar.push(a.game, indexes.length);
        for (let p = 0; p <= a.getTurnPlayer().pillz; p++) {
            for (let f of p <= a.getTurnPlayer().pillz - 3 ? [true, false] : [false]) {
                let promises = [];
                for (let i of indexes) {
                    if (a.game.round.round <= 2) {
                        promises.push(DistributedAnalysis.threadedFillTree(a.game, `${i} ${p} ${f}`, bar, i, p, f, minimax.playSecond).then((m) => {
                            minimax.add(m);
                            if (bar)
                                bar.tick();
                        }));
                    }
                    else {
                        let m = await Analysis.fillTree(a.game, `${i} ${p} ${f}`, bar, i, p, f, minimax.playSecond);
                        minimax.add(m);
                        if (!minimax.playSecond && !fullSearch) {
                            if ((m.result == GameResult.WIN && m.turn == Minimax.MAX) ||
                                (m.result == GameResult.LOSS && m.turn == Minimax.MIN)) {
                                process.stdout.write("break\n");
                                break;
                            }
                        }
                    }
                }
                await Promise.allSettled(promises);
            }
        }
        if (bar)
            bar.pop();
        return minimax;
    }
    static async iterTree1(game, child = false) {
        let minimax = new Minimax();
        let timer = `iterTree${Math.random()}`;
        console.time(timer);
        let games = [new Analysis(game)];
        let nodes = [minimax];
        let indexes;
        let depth = 0;
        let _i;
        if (!child && (_i = games[0].deselect()) !== undefined) {
            minimax.playSecond = true;
            indexes = [_i];
        }
        else
            indexes = games[0].getCardIndexes();
        minimax.turn = games[0].getTurn();
        while (games.length) {
            let next = [];
            let layer = [];
            for (let index in games) {
                let a = games[index];
                let n = nodes[index];
                if (indexes === undefined)
                    indexes = a.getCardIndexes();
                let pillz = a.getTurnPlayer().pillz;
                let items = [];
                root: for (let i of indexes) {
                    let breaking = false;
                    card: for (let p of shiftRange(pillz)) {
                        for (let f of p <= pillz - 3 ? [true, false] : [false]) {
                            let c = new Analysis(a.game);
                            c.game.select(i, p, f);
                            let m = n.add(`${i} ${p} ${f}`, c.getTurn(), n.playSecond);
                            if (!c.game.selectedFirst && c.game.winner) {
                                if (c.game.winner == "Player") {
                                    m.win();
                                    if (!n.defered) {
                                        if (m.turn) {
                                            m.break = true;
                                            break root;
                                        }
                                        else if (p == pillz) {
                                            breaking = true;
                                        }
                                        else if (breaking && f && p == pillz - 3) {
                                            m.break = true;
                                            break card;
                                        }
                                    }
                                }
                                else if (c.game.winner == "Tie") {
                                    m.tie();
                                }
                                else if (c.game.winner == "Urban Rival") {
                                    m.loss();
                                    if (!n.defered) {
                                        if (!m.turn) {
                                            m.break = true;
                                            break root;
                                        }
                                        else if (p == pillz) {
                                            breaking = true;
                                        }
                                        else if (breaking && f && p == pillz - 3) {
                                            m.break = true;
                                            break card;
                                        }
                                    }
                                }
                                else
                                    throw new Error(`Unknown winner: "${c.game.winner}"`);
                            }
                            else {
                                if (c.game.round.round == (((((1 + 0))))) && !c.game.selectedFirst) {
                                    n.add(await Analysis.iterTree(c.game, true));
                                }
                                else if (c.game.round.round == 2 && !c.game.selectedFirst) {
                                    items.push(c.game);
                                }
                                else {
                                    next.push(c);
                                    layer.push(m);
                                }
                            }
                        }
                    }
                }
                await PromiseMap.race(items, g => DistributedAnalysis.threadedIterTree(g)
                    .then((x) => n.add(x)), (((((6 - 1))))));
                indexes = undefined;
            }
            process.stdout.write(`[${(depth++).toString().green}`.grey + ']'.grey + ' Finished  ' +
                `Games: ${games.length}\n`.yellow);
            games = next;
            nodes = layer;
            console.timeLog(timer);
        }
        console.timeEnd(timer);
        return minimax;
    }
    static async iterTree(game, child = false) {
        let minimax = new Minimax();
        let timer = `iterTree${Math.random()}`;
        console.time(timer);
        let games = [new Analysis(game)];
        let nodes = [minimax];
        let indexes;
        let depth = 0;
        let _i;
        if (!child && (_i = games[0].deselect()) !== undefined) {
            minimax.playSecond = true;
            indexes = [_i];
        }
        else
            indexes = games[0].getCardIndexes();
        minimax.turn = games[0].getTurn();
        while (games.length) {
            let next = [];
            let layer = [];
            for (let index in games) {
                let a = games[index];
                let n = nodes[index];
                if (indexes === undefined)
                    indexes = a.getCardIndexes();
                let pillz = a.getTurnPlayer().pillz;
                root: for (let i of indexes) {
                    let breaking = false;
                    card: for (let p of shiftRange(pillz)) {
                        for (let f of (p <= pillz - 3 ? [true, false] : [false])) {
                            let c = new Analysis(a.game);
                            c.game.select(i, p, f);
                            let m = n.add(`${i} ${p} ${f}`, c.getTurn(), n.playSecond);
                            if (!c.game.selectedFirst && c.game.winner) {
                                if (c.game.winner == "Player") {
                                    m.win();
                                    if (!n.defered) {
                                        if (m.turn) {
                                            m.break = true;
                                            break root;
                                        }
                                        else if (p == pillz) {
                                            breaking = true;
                                        }
                                        else if (breaking && f && p == pillz - 3) {
                                            m.break = true;
                                            break card;
                                        }
                                    }
                                }
                                else if (c.game.winner == "Tie") {
                                    m.tie();
                                }
                                else if (c.game.winner == "Urban Rival") {
                                    m.loss();
                                    if (!n.defered) {
                                        if (!m.turn) {
                                            m.break = true;
                                            break root;
                                        }
                                        else if (p == pillz) {
                                            breaking = true;
                                        }
                                        else if (breaking && f && p == pillz - 3) {
                                            m.break = true;
                                            break card;
                                        }
                                    }
                                }
                                else
                                    throw new Error(`Unknown winner: "${c.game.winner}"`);
                            }
                            else {
                                if (c.game.round.round == (((((1 + 0))))) && !c.game.selectedFirst) {
                                    n.add(await Analysis.iterTree(c.game, true));
                                }
                                else if (c.game.round.round == 2 && !c.game.selectedFirst) {
                                    n.add(await DistributedAnalysis.iterTree(c.game));
                                }
                                else {
                                    next.push(c);
                                    layer.push(m);
                                }
                            }
                        }
                    }
                }
                indexes = undefined;
            }
            process.stdout.write(`[${(depth++).toString().green}`.grey + ']'.grey + ' Finished  ' +
                `Games: ${games.length}\n`.yellow);
            games = next;
            nodes = layer;
            console.timeLog(timer);
        }
        console.timeEnd(timer);
        return minimax;
    }
}
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiQW5hbHlzaXMuanMiLCJzb3VyY2VSb290IjoiL0M6L1VzZXJzL1N0dWRlbnQvRG9jdW1lbnRzL05vZGVKU1dvcmtzcGFjZS9VcmJhblJlY3JlYXRpb24vc3JjLyIsInNvdXJjZXMiOlsidHMvc29sdmVyL0FuYWx5c2lzLnRzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUFBLE9BQU8sSUFBSSxNQUFNLFNBQVMsQ0FBQTtBQUUxQixPQUFPLEVBQUUsVUFBVSxFQUFFLE1BQU0sZ0JBQWdCLENBQUE7QUFDM0MsT0FBTyxHQUFHLE1BQU0sT0FBTyxDQUFBO0FBQ3ZCLE9BQU8sT0FBTyxFQUFFLEVBQUUsVUFBVSxFQUFRLE1BQU0sV0FBVyxDQUFBO0FBQ3JELE9BQU8sVUFBVSxNQUFNLHFCQUFxQixDQUFBO0FBQzVDLE9BQU8sbUJBQW1CLE1BQU0sdUJBQXVCLENBQUE7QUFFdkQsTUFBTSxDQUFDLE9BQU8sT0FBTyxRQUFRO0lBRTNCLFlBQVksSUFBVSxFQUFFLE1BQU0sR0FBRyxLQUFLLEVBQUUsSUFBSSxHQUFHLEtBQUssRUFBRSxLQUFLLEdBQUcsSUFBSTtRQUNoRSxJQUFJLENBQUMsS0FBSztZQUNSLElBQUksQ0FBQyxJQUFJLEdBQUcsSUFBSSxDQUFDO2FBQ2QsSUFBSSxJQUFJLFlBQVksSUFBSTtZQUMzQixJQUFJLENBQUMsSUFBSSxHQUFHLElBQUksQ0FBQyxLQUFLLENBQUMsTUFBTSxFQUFFLElBQUksQ0FBQyxDQUFDOztZQUVyQyxJQUFJLENBQUMsSUFBSSxHQUFHLElBQUksQ0FBQyxJQUFJLENBQUMsSUFBSSxFQUFFLE1BQU0sRUFBRSxJQUFJLENBQUMsQ0FBQztJQUM5QyxDQUFDO0lBRUQsWUFBWTtRQUNWLE9BQU8sSUFBSSxDQUFDLElBQUksQ0FBQyxhQUFhLElBQUksSUFBSSxDQUFDLElBQUksQ0FBQyxLQUFLLENBQUMsS0FBSyxDQUFDLENBQUM7WUFDdkQsSUFBSSxDQUFDLElBQUksQ0FBQyxFQUFFLENBQUMsS0FBSyxDQUFDLENBQUM7WUFDcEIs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