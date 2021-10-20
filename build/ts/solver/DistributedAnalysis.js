var _a;
import { Worker } from "worker_threads";
import cluster from 'cluster';
import Minimax from "./Minimax";
export default class DistributedAnalysis {
    static threadedFillTree(game, minimax, bar, index, pillz, fury, fullSearch) {
        return new Promise((resolve, reject) => {
            let worker = new Worker("./solver/ThreadFillTree.js", {
                workerData: {
                    game, minimax,
                    bar: null,
                    index, pillz, fury,
                    fullSearch,
                    threadID: `${index} ${pillz} ${fury}`,
                },
            });
            worker.on("message", (minimax) => {
                resolve(Minimax.from(minimax));
            });
            worker.on("error", reject);
            worker.on("exit", (code) => {
                if (code !== 0)
                    reject(new Error(`Worker stopped with exit code ${code}`));
            });
        });
    }
    static threadedIterTree(game) {
        return new Promise((resolve, reject) => {
            let worker = new Worker("./solver/ThreadIterTree.js", {
                workerData: game,
            });
            worker.on("message", (minimax) => {
                resolve(Minimax.from(minimax));
            });
            worker.on("error", reject);
            worker.on("exit", (code) => {
                if (code !== 0)
                    reject(new Error(`Worker stopped with exit code ${code}`));
            });
        });
    }
    static async race() {
        if (this.promises.size >= this.threads)
            await Promise.race(this.promises);
    }
    static async iterTree(game) {
        await this.race();
        const id = this.id++;
        const worker = this.workers.pop();
        const promise = new Promise((resolve) => {
        });
        promise.then(() => this.promises.delete(promise));
        this.promises.add(promise);
        worker.send({ game, id });
        return promise;
    }
}
_a = DistributedAnalysis;
DistributedAnalysis.threads = 5;
DistributedAnalysis.workers = [];
DistributedAnalysis.promises = new Set();
DistributedAnalysis.id = 0;
(() => {
    if (cluster.isMaster) {
        cluster.setupMaster({
            exec: './solver/ProcSolver.js',
            execArgv: [
                "--experimental-specifier-resolution=node",
                "--enable-source-maps",
                "--max-old-space-size=6350",
                "--inspect",
            ]
        });
        for (let i = 0; i < _a.threads; i++) {
            const worker = cluster.fork();
            worker.on("online", () => {
                process.stdout.write(`Worker ${worker.id} is online\n`.green);
            });
            worker.on("error", (err) => {
                process.stdout.write(`Cluster process error: ${err}\n`.red);
            });
            worker.on("exit", (code) => {
                process.stdout.write(`Worker ${i} exited with code ${code}\n`.yellow.dim);
            });
            worker.once("message", (data) => {
                process.stdout.write("once: " + data.msg);
            });
            worker.on("message", (data) => {
                process.stdout.write("on" + data.msg);
            });
            worker.send({ msg: 'Welcome!' });
            _a.workers.push(worker);
        }
    }
})();
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiRGlzdHJpYnV0ZWRBbmFseXNpcy5qcyIsInNvdXJjZVJvb3QiOiIvQzovVXNlcnMvU3R1ZGVudC9Eb2N1bWVudHMvTm9kZUpTV29ya3NwYWNlL1VyYmFuUmVjcmVhdGlvbi9zcmMvIiwic291cmNlcyI6WyJ0cy9zb2x2ZXIvRGlzdHJpYnV0ZWRBbmFseXNpcy50cyJdLCJuYW1lcyI6W10sIm1hcHBpbmdzIjoiO0FBQ0EsT0FBTyxFQUErQixNQUFNLEVBQUUsTUFBTSxnQkFBZ0IsQ0FBQTtBQUNwRSxPQUFPLE9BQU8sTUFBTSxTQUFTLENBQUE7QUFFN0IsT0FBTyxPQUFpQixNQUFNLFdBQVcsQ0FBQTtBQUl6QyxNQUFNLENBQUMsT0FBTyxPQUFPLG1CQUFtQjtJQUV0QyxNQUFNLENBQUMsZ0JBQWdCLENBQ3JCLElBQVUsRUFDVixPQUFlLEVBQ2YsR0FBUSxFQUNSLEtBQWEsRUFDYixLQUFhLEVBQ2IsSUFBYSxFQUNiLFVBQW1CO1FBRW5CLE9BQU8sSUFBSSxPQUFPLENBQUMsQ0FBQyxPQUFPLEVBQUUsTUFBTSxFQUFFLEVBQUU7WUFDckMsSUFBSSxNQUFNLEdBQUcsSUFBSSxNQUFNLENBQUMsNEJBQTRCLEVBQUU7Z0JBQ3BELFVBQVUsRUFBRTtvQkFDVixJQUFJLEVBQUUsT0FBTztvQkFDYixHQUFHLEVBQUUsSUFBSTtvQkFDVCxLQUFLLEVBQUUsS0FBSyxFQUFFLElBQUk7b0JBQ2xCLFVBQVU7b0JBQ1YsUUFBUSxFQUFFLEdBQUcsS0FBSyxJQUFJLEtBQUssSUFBSSxJQUFJLEVBQUU7aUJBQ3RDO2FBQ0YsQ0FBQyxDQUFDO1lBQ0gsTUFBTSxDQUFDLEVBQUUsQ0FBQyxTQUFTLEVBQUUsQ0FBQyxPQUFPLEVBQUUsRUFBRTtnQkFDL0IsT0FBTyxDQUFDLE9BQU8sQ0FBQyxJQUFJLENBQUMsT0FBTyxDQUFDLENBQUMsQ0FBQztZQUNqQyxDQUFDLENBQUMsQ0FBQztZQUNILE1BQU0sQ0FBQyxFQUFFLENBQUMsT0FBTyxFQUFFLE1BQU0sQ0FBQyxDQUFDO1lBQzNCLE1B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