var _a;
import { Worker } from "worker_threads";
import cluster from 'cluster';
import Minimax from "./Minimax";
import { cpus } from 'os';
import WorkerProcess from "./WorkerProcess";
export default class DistributedAnalysis {
    static threadedFillTree(game, minimax, bar, index, pillz, fury, fullSearch) {
        return new Promise((resolve, reject) => {
            const worker = new Worker("./solver/ThreadFillTree.js", {
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
            const worker = new Worker("./solver/ThreadIterTree.js", {
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
    static async iterTree(game) {
        return WorkerProcess.processOnWorker({ game, id: this.id++ }).then(Minimax.from);
    }
}
_a = DistributedAnalysis;
DistributedAnalysis.threads = cpus().length;
DistributedAnalysis.id = 0;
(() => {
    if (cluster.isMaster) {
        for (let i = 0; i < _a.threads; i++) {
            WorkerProcess.create(i);
        }
    }
})();
DistributedAnalysis.race = WorkerProcess.race.bind(WorkerProcess);
DistributedAnalysis.allFinished = WorkerProcess.allFinished.bind(WorkerProcess);
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiRGlzdHJpYnV0ZWRBbmFseXNpcy5qcyIsInNvdXJjZVJvb3QiOiIvQzovVXNlcnMvU3R1ZGVudC9Eb2N1bWVudHMvTm9kZUpTV29ya3NwYWNlL1VyYmFuUmVjcmVhdGlvbi9zcmMvIiwic291cmNlcyI6WyJ0cy9zb2x2ZXIvRGlzdHJpYnV0ZWRBbmFseXNpcy50cyJdLCJuYW1lcyI6W10sIm1hcHBpbmdzIjoiO0FBQ0EsT0FBTyxFQUFFLE1BQU0sRUFBRSxNQUFNLGdCQUFnQixDQUFBO0FBQ3ZDLE9BQU8sT0FBTyxNQUFNLFNBQVMsQ0FBQTtBQUU3QixPQUFPLE9BQWlCLE1BQU0sV0FBVyxDQUFBO0FBQ3pDLE9BQU8sRUFBRSxJQUFJLEVBQUUsTUFBTSxJQUFJLENBQUE7QUFDekIsT0FBTyxhQUFhLE1BQU0saUJBQWlCLENBQUE7QUFLM0MsTUFBTSxDQUFDLE9BQU8sT0FBTyxtQkFBbUI7SUFFdEMsTUFBTSxDQUFDLGdCQUFnQixDQUNyQixJQUFVLEVBQ1YsT0FBZSxFQUNmLEdBQVEsRUFDUixLQUFhLEVBQ2IsS0FBYSxFQUNiLElBQWEsRUFDYixVQUFtQjtRQUVuQixPQUFPLElBQUksT0FBTyxDQUFDLENBQUMsT0FBTyxFQUFFLE1BQU0sRUFBRSxFQUFFO1lBQ3JDLE1BQU0sTUFBTSxHQUFHLElBQUksTUFBTSxDQUFDLDRCQUE0QixFQUFFO2dCQUN0RCxVQUFVLEVBQUU7b0JBQ1YsSUFBSSxFQUFFLE9BQU87b0JBQ2IsR0FBRyxFQUFFLElBQUk7b0JBQ1QsS0FBSyxFQUFFLEtBQUssRUFBRSxJQUFJO29CQUNsQixVQUFVO29CQUNWLFFBQVEsRUFBRSxHQUFHLEtBQUssSUFBSSxLQUFLLElBQUksSUFBSSxFQUFFO2lCQUN0QzthQUNGLENBQUMsQ0FBQztZQUNILE1BQU0sQ0FBQyxFQUFFLENBQUMsU0FBUyxFQUFFLENBQUMsT0FBTyxFQUFFLEVBQUU7Z0JBQy9CLE9BQU8sQ0FBQyxPQUFPLENBQUMsSUFBSSxDQUFDLE9BQU8sQ0FBQyxDQUFDLENBQUM7WUFDakMsQ0FBQyxDQUFDLENBQUM7WUFDSCxNQUFNLENBQUMsRUFBRSxDQUFDLE9BQU8sRUFBRSxNQUFNLENBQUMsQ0FBQztZQUMzQixNQUFNLENBQUMsRUFBRSxDQUFDLE1BQU0sRUFBRSxDQUFDLElBQUksRUFBRSxFQUFFO2dCQUN6QixJQUFJLElBQUksS0FBSyxDQUFDO29CQUNaLE1BQU0sQ0FBQyxJQUFJLEtBQUssQ0FBQyxpQ0FBaUMsSUFBSSxFQUFFLENBQUMsQ0FBQyxDQUFDO1lBQy9ELENBQUMsQ0FBQyxDQUFDO1FBQ0wsQ0FBQyxDQUFDLENBQUM7SUFDTCxDQUFDO0lBRUQsTUFBTSxDQUFDLGdCQUFnQixDQUFDLElBQVU7UUFDaEMsT0FBTyxJQUFJLE9BQU8sQ0FBTyxDQUFDLE9BQU8sRUFBRSxNQUFNLEVBQUUsRUFBRTtZQUMzQyxNQUFNLE1BQU0sR0FBRyxJQUFJLE1BQU0sQ0FBQyw0QkFBNEIsRUFBRTtnQkFDdEQsVUFBVSxFQUFFLElBQUk7YUFDakIsQ0FBQyxDQUFDO1lBQ0gsTUFBTSxDQUFDLEVBQUUsQ0FBQyxTQUFTLEVBQUUsQ0FBQyxPQUFPLEVBQUUsRUFBRTtnQkFDL0IsT0FBTyxDQUFDLE9BQU8sQ0FBQyxJQUFJLENBQUMsT0FBTyxDQUFDLENBQUMsQ0FBQztZQUNqQyxDQUFDLENBQUMsQ0FBQztZQUNILE1BQU0sQ0FBQyxFQUFFLENBQUMsT0FBTyxFQUFFLE1BQU0sQ0FBQyxDQUFDO1lBQzNCLE1BQU0sQ0FBQyxFQUFFLENBQUMsTUFBTSxFQUFFLENBQUMsSUFBSSxFQUFFLEVBQUU7Z0JBQ3pCLElBQUksSUFBSSxLQUFLLENBQUM7b0JBQ1osTUFBTSxDQUFDLElBQUksS0FBSyxDQUFDLGlDQUFpQyxJQUFJLEVBQUUsQ0FBQyxDQUFDLENBQUM7WUFDL0QsQ0FBQyxDQUFDLENBQUM7UUFDTCxDQUFDLENBQUMsQ0FBQztJQUNMLENBQUM7SUE4QkQsTUFBTSxDQUFDLEtBQUssQ0FBQyxRQUFRLENBQUMsSUFBVTtRQUM5QixPQUFPLGFBQWEsQ0FBQyxlQUFlLENBQ2xDLEVBQUUsSUFBSSxFQUFFLEVBQUUsRUFBRSxJQUFJLENBQUMsRUFBRSxFQUFFLEVBQUUsQ0FBQyxDQUFDLElBQUksQ0FBQyxPQUFPLENBQUMsSUFBSSxDQUFDLENBQUM7SUFDaEQsQ0FBQzs7O0FBOUJNLDJCQUFPLEdBQUcsSUFBSSxFQUFFLENBQUMsTUFBTSxDQUFDO0FBQ2hCLHNCQUFFLEdBQUcsQ0FBRSxDQUFBO0FBRXRCO0lBQ0UsSUFBSSxPQUFPLENBQUMsUUFBUSxFQUFFO1FBQ3BCLEtBQUssSUFBSSxDQUFDLEdBQUcsQ0FBQyxFQUFFLENBQUMsR0FBRyxFQUFJLENBQUMsT0FBTyxFQUFFLENBQUMsRUFBRSxFQUFFO1lBYXJDLGFBQWEsQ0FBQyxNQUFNLENBQXlCLENBQUMsQ0FBQyxDQUFDO1NBQ2pEO0tBQ0Y7QUFDSCxDQUFDLEdBQUEsQ0FBQTtBQUdNLHdCQUFJLEdBQUcsYUFBYSxDQUFDLElBQUksQ0FBQyxJQUFJLENBQUMsYUFBYSxDQUFDLENBQUM7QUFDOUMsK0JBQVcsR0FBRyxhQUFhLENBQUMsV0FBVyxDQUFDLElBQUksQ0FBQyxhQUFhLENBQUMsQ0FBQyJ9