import Game from "../game/Game.ts";
import { Worker } from "node:worker_threads";
import Bar from "./Bar.ts";
import Minimax, { Node } from "./Minimax.ts";
import WorkerProcess from "./WorkerProcess.ts";
import { WorkerSolverData } from "../game/types/Types.ts";

export default class DistributedAnalysis {
  static threadedFillTree(
    game: Game,
    minimax: string,
    bar: Bar,
    index: number,
    pillz: number,
    fury: boolean,
    fullSearch: boolean,
  ) {
    return new Promise((resolve, reject) => {
      const worker = new Worker("./solver/ThreadFillTree.ts", {
        workerData: {
          game,
          minimax,
          bar: null,
          index,
          pillz,
          fury,
          fullSearch,
          threadID: `${index} ${pillz} ${fury}`,
        },
      });
      worker.on("message", (minimax) => {
        resolve(Minimax.from(minimax));
      });
      worker.on("error", reject);
      worker.on("exit", (code) => {
        if (code !== 0) {
          reject(new Error(`Worker stopped with exit code ${code}`));
        }
      });
    });
  }

  static threadedIterTree(game: Game) {
    return new Promise<Node>((resolve, reject) => {
      const worker = new Worker("./solver/ThreadIterTree.js", {
        workerData: game,
      });
      worker.on("message", (minimax) => {
        resolve(Minimax.from(minimax));
      });
      worker.on("error", reject);
      worker.on("exit", (code) => {
        if (code !== 0) {
          reject(new Error(`Worker stopped with exit code ${code}`));
        }
      });
    });
  }

  private static id = 0;

  static race = WorkerProcess.race.bind(WorkerProcess);
  static allFinished = WorkerProcess.allFinished.bind(WorkerProcess);

  static iterTree(
    game: Game,
    child?: boolean,
    rootName?: string,
    freeze?: boolean,
  ) {
    return WorkerProcess.processOnWorker<WorkerSolverData, Node>(
      { game, child, rootName, freeze, id: this.id++ },
    ).then(Minimax.from);
  }
}
