import Game, { Winner } from "../Game";
import { shiftRange } from '../utils/Utils';
import Bar from "./Bar";
import Minimax, { GameResult } from "./Minimax";
import PromiseMap from '../utils/PromiseMap';
import DistributedAnalysis from "./DistributedAnalysis";
import { Turn } from "../types/Types";
export default class Analysis {
    constructor(game, inputs = false, logs = false, clone = true) {
        if (!clone)
            this.game = game;
        else if (game instanceof Game)
            this.game = game.clone(inputs, logs);
        else
            this.game = Game.from(game, inputs, logs);
    }
    get playingHand() {
        return this.turn === Turn.PLAYER_1 ?
            this.game.h1 : this.game.h2;
    }
    get playingPlayer() {
        return this.turn === Turn.PLAYER_1 ?
            this.game.p1 : this.game.p2;
    }
    get playedCardIndex() {
        if (this.turn === Turn.PLAYER_1) {
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
    get playedHand() {
        return this.turn === Turn.PLAYER_1 ?
            this.game.h2 : this.game.h1;
    }
    get unplayedCardIndexes() {
        const indexes = [];
        const hand = this.playingHand;
        let j = 0;
        for (let i = 0; i < 4; i++)
            if (!hand[i].played)
                indexes[j++] = i;
        return indexes;
    }
    get turn() {
        return this.game.firstHasSelected !== this.game.first;
    }
    deselect() {
        if (!this.game.firstHasSelected)
            return;
        const i = this.playedCardIndex;
        this.playedHand[i].played = false;
        this.game.firstHasSelected = false;
        return i;
    }
    static async fillTree(game, minimax = new Minimax(), bar = new Bar(), index, pillz, fury = false, fullSearch = false) {
        const a = new Analysis(game);
        if (typeof minimax == "string")
            minimax = Minimax.node(minimax);
        let indexes, i;
        if (minimax.name != "root") {
            a.game.select(index, pillz, fury);
            if (!a.game.firstHasSelected && a.game.winner) {
                if (a.game.winner === Winner.PLAYER_1)
                    minimax.result = GameResult.PLAYER_1_WIN;
                else if (a.game.winner === Winner.TIE)
                    minimax.result = GameResult.TIE;
                else if (a.game.winner === Winner.PLAYER_2)
                    minimax.result = GameResult.PLAYER_2_WIN;
                else
                    throw new Error(`Unknown winner: "${a.game.winner}"`);
                return minimax;
            }
        }
        else
            i = a.deselect();
        if (i != undefined) {
            minimax.playSecond = true;
            indexes = [i];
        }
        else {
            indexes = a.unplayedCardIndexes;
        }
        minimax.turn = a.turn;
        if (bar)
            bar.push(a.game, indexes.length);
        for (let p = 0; p <= a.playingPlayer.pillz; p++) {
            for (const f of p <= a.playingPlayer.pillz - 3 ? [true, false] : [false]) {
                const promises = [];
                for (const i of indexes) {
                    if (a.game.round <= 2) {
                        promises.push(DistributedAnalysis.threadedFillTree(a.game, `${i} ${p} ${f}`, bar, i, p, f, minimax.playSecond).then((m) => {
                            minimax.add(m);
                            if (bar)
                                bar.tick();
                        }));
                    }
                    else {
                        const m = await Analysis.fillTree(a.game, `${i} ${p} ${f}`, bar, i, p, f, minimax.playSecond);
                        minimax.add(m);
                        if (!minimax.playSecond && !fullSearch) {
                            if ((m.result == GameResult.PLAYER_1_WIN && m.turn == Minimax.MAX) ||
                                (m.result == GameResult.PLAYER_2_WIN && m.turn == Minimax.MIN)) {
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
        const minimax = new Minimax();
        const timer = `iterTree${Math.random()}`;
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
            indexes = games[0].unplayedCardIndexes;
        minimax.turn = games[0].turn;
        while (games.length) {
            const next = [];
            const layer = [];
            for (const index in games) {
                const a = games[index];
                const n = nodes[index];
                if (indexes === undefined)
                    indexes = a.unplayedCardIndexes;
                const pillz = a.playingPlayer.pillz;
                const items = [];
                root: for (const i of indexes) {
                    let breaking = false;
                    card: for (const p of shiftRange(pillz)) {
                        for (const f of p <= pillz - 3 ? [true, false] : [false]) {
                            const c = new Analysis(a.game);
                            c.game.select(i, p, f);
                            const m = n.add(`${i} ${p} ${f}`, c.turn, n.playSecond);
                            if (!c.game.firstHasSelected && c.game.winner) {
                                if (c.game.winner === Winner.PLAYER_1) {
                                    m.result = GameResult.PLAYER_1_WIN;
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
                                else if (c.game.winner === Winner.TIE) {
                                    m.result = GameResult.TIE;
                                }
                                else if (c.game.winner == Winner.PLAYER_2) {
                                    m.result = GameResult.PLAYER_2_WIN;
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
                                if (c.game.round === 1 && !c.game.firstHasSelected) {
                                    n.add(await Analysis.iterTree1(c.game, true));
                                }
                                else if (c.game.round == 2 && !c.game.firstHasSelected) {
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
        const minimax = new Minimax();
        const timer = `iterTree-${this.counter++}`;
        console.time(timer);
        const rootAnalysis = new Analysis(game);
        let analyses = [rootAnalysis];
        let nodes = [minimax];
        let indexes;
        let depth = 0;
        let _i;
        if (!child && (_i = rootAnalysis.deselect()) !== undefined) {
            minimax.playSecond = true;
            indexes = [_i];
        }
        else {
            indexes = rootAnalysis.unplayedCardIndexes;
        }
        minimax.turn = rootAnalysis.turn;
        while (analyses.length) {
            const roundAnalyses = [];
            const roundNodes = [];
            for (const index in analyses) {
                const parentAnalysis = analyses[index];
                const parentNode = nodes[index];
                if (indexes === undefined)
                    indexes = parentAnalysis.unplayedCardIndexes;
                const pillz = parentAnalysis.playingPlayer.pillz;
                root: for (const i of indexes) {
                    let breaking = false;
                    card: for (const p of shiftRange(pillz)) {
                        for (const f of (p <= pillz - 3 ? [true, false] : [false])) {
                            const analysis = new Analysis(parentAnalysis.game);
                            const game = analysis.game;
                            game.select(i, p, f);
                            const node = parentNode.add(`${i} ${p} ${f}`, analysis.turn, parentNode.playSecond);
                            if (!game.firstHasSelected && game.winner) {
                                if (game.winner === Winner.PLAYER_1) {
                                    node.result = GameResult.PLAYER_1_WIN;
                                    if (!parentNode.defered) {
                                        if (node.turn) {
                                            node.break = true;
                                            break root;
                                        }
                                        else if (p == pillz) {
                                            breaking = true;
                                        }
                                        else if (breaking && f && p === pillz - 3) {
                                            node.break = true;
                                            break card;
                                        }
                                    }
                                }
                                else if (game.winner === Winner.TIE) {
                                    node.result = GameResult.TIE;
                                }
                                else if (game.winner === Winner.PLAYER_2) {
                                    node.result = GameResult.PLAYER_2_WIN;
                                    if (!parentNode.defered) {
                                        if (!node.turn) {
                                            node.break = true;
                                            break root;
                                        }
                                        else if (p === pillz) {
                                            breaking = true;
                                        }
                                        else if (breaking && f && p === pillz - 3) {
                                            node.break = true;
                                            break card;
                                        }
                                    }
                                }
                                else
                                    throw new Error(`Unknown winner: "${game.winner}"`);
                            }
                            else {
                                if (game.round === 1 && !game.firstHasSelected) {
                                    parentNode.add(await Analysis.iterTree(game, true));
                                }
                                else if (game.round === 2 && !game.firstHasSelected) {
                                    await DistributedAnalysis.race();
                                    DistributedAnalysis.iterTree(game)
                                        .then(node => parentNode.add(node));
                                }
                                else {
                                    roundAnalyses.push(analysis);
                                    roundNodes.push(node);
                                }
                            }
                        }
                    }
                }
                await DistributedAnalysis.allFinished();
                indexes = undefined;
            }
            process.stdout.write(`[${(depth++).toString().green}`.grey + ']'.grey + ' Finished  ' +
                `Games: ${analyses.length}\n`.yellow);
            analyses = roundAnalyses;
            nodes = roundNodes;
            console.timeLog(timer);
        }
        console.timeEnd(timer);
        return minimax;
    }
}
Analysis.counter = 0;
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiQW5hbHlzaXMuanMiLCJzb3VyY2VSb290IjoiL0M6L1VzZXJzL1N0dWRlbnQvRG9jdW1lbnRzL05vZGVKU1dvcmtzcGFjZS9VcmJhblJlY3JlYXRpb24vc3JjLyIsInNvdXJjZXMiOlsidHMvc29sdmVyL0FuYWx5c2lzLnRzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUFBLE9BQU8sSUFBSSxFQUFFLEVBQUUsTUFBTSxFQUFFLE1BQU0sU0FBUyxDQUFBO0FBQ3RDLE9BQU8sRUFBRSxVQUFVLEVBQUUsTUFBTSxnQkFBZ0IsQ0FBQTtBQUMzQyxPQUFPLEdBQUcsTUFBTSxPQUFPLENBQUE7QUFDdkIsT0FBTyxPQUFPLEVBQUUsRUFBRSxVQUFVLEVBQVEsTUFBTSxXQUFXLENBQUE7QUFDckQsT0FBTyxVQUFVLE1BQU0scUJBQXFCLENBQUE7QUFDNUMsT0FBTyxtQkFBbUIsTUFBTSx1QkFBdUIsQ0FBQTtBQUN2RCxPQUFPLEVBQUUsSUFBSSxFQUFFLE1BQU0sZ0JBQWdCLENBQUE7QUFFckMsTUFBTSxDQUFDLE9BQU8sT0FBTyxRQUFRO0lBRTNCLFlBQVksSUFBVSxFQUFFLE1BQU0sR0FBRyxLQUFLLEVBQUUsSUFBSSxHQUFHLEtBQUssRUFBRSxLQUFLLEdBQUcsSUFBSTtRQUNoRSxJQUFJLENBQUMsS0FBSztZQUNSLElBQUksQ0FBQyxJQUFJLEdBQUcsSUFBSSxDQUFDO2FBQ2QsSUFBSSxJQUFJLFlBQVksSUFBSTtZQUMzQixJQUFJLENBQUMsSUFBSSxHQUFHLElBQUksQ0FBQyxLQUFLLENBQUMsTUFBTSxFQUFFLElBQUksQ0FBQyxDQUFDOztZQUdyQyxJQUFJLENBQUMsSUFBSSxHQUFHLElBQUksQ0FBQyxJQUFJLENBQUMsSUFBSSxFQUFFLE1BQU0sRUFBRSxJQUFJLENBQUMsQ0FBQztJQUM5QyxDQUFDO0lBRUQsSUFBSSxXQUFXO1FBR2IsT0FBTyxJQUFJLENBQUMsSUFBSSxLQUFLLElBQUksQ0FBQyxRQUFRLENBQUMsQ0FBQztZQUNsQyxJQUFJLENBQUMsSUFB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