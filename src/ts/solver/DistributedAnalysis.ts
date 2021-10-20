import Game from "../Game"
import { MessageChannel, MessagePort, Worker } from "worker_threads"
import cluster from 'cluster'
import Bar from "./Bar"
import Minimax, { Node } from "./Minimax"
import { WorkerSolverData } from "../types/Types"


export default class DistributedAnalysis {

  static threadedFillTree(
    game: Game,
    minimax: string,
    bar: Bar,
    index: number,
    pillz: number,
    fury: boolean,
    fullSearch: boolean
  ) {
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

  static threadedIterTree(game: Game) {
    return new Promise<Node>((resolve, reject) => {
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


  // static threads = 5;
  // static workers: Worker[] = [];
  // static channels: MessagePort[] = [];
  // static promises = new Set<Promise<Node>>();
  // private static id = 0;
  // static {
  //   for (let i = 0; i < this.threads; i++) {

  //     const channel = new MessageChannel();
  //     const worker = new Worker("./solver/ThreadSolver.js", {});
  //     worker.on("online", () => {
  //       process.stdout.write(`Worker ${worker.threadId} is online\n`.green);
  //     })
  //     worker.on("error", (err) => {
  //       throw err;
  //     });
  //     worker.on("exit", (code) => {
  //       // if (code !== 0)
  //       //   throw new Error(`Worker stopped with exit code ${code}`);

  //       process.stdout.write(`Worker ${i} exited with code ${code}\n`.yellow.dim)
  //     });

  //     worker.postMessage({ port: channel.port1 }, [channel.port1]);

  //     this.workers.push(worker);
  //     this.channels.push(channel.port2);

  //     channel.port2.once("message", (msg) => {
  //       console.log('Received init message: ' + msg);
  //     })
  //   }
  // }

  // static async race() {
  //   if (this.promises.size >= this.threads)
  //     await Promise.race(this.promises);
  // }

  // static async iterTree(game: Game) {
  //   await this.race();

  //   const id = this.id++;
  //   const channel = this.channels.pop()!;
  //   const promise = new Promise<Node>((resolve) => {
  //     channel.once("message", (data) => {
  //       this.channels.push(channel);

  //       resolve(Minimax.from(data));
  //     })
  //   })
  //   promise.then(() => this.promises.delete(promise));

  //   this.promises.add(promise);


  //   channel.postMessage(<WorkerSolverData>{ game, id });

  //   return promise;
  // }


  static threads = 5;
  static workers: cluster.Worker[] = []
  static promises = new Set<Promise<Node>>();
  private static id = 0;
  static {
    if (cluster.isMaster) {
      cluster.setupMaster({
        exec: './solver/ProcSolver.js',
        execArgv: [
          "--experimental-specifier-resolution=node",
          "--enable-source-maps",
          "--max-old-space-size=6350",
          "--inspect", // "--inspect-brk"
        ]
      })
      for (let i = 0; i < this.threads; i++) {
        const worker = cluster.fork();

        worker.on("online", () => {
          process.stdout.write(`Worker ${worker.id} is online\n`.green);
        })
        worker.on("error", (err) => {
          // throw err;
          process.stdout.write(`Cluster process error: ${err}\n`.red)
        });
        worker.on("exit", (code) => {
          // if (code !== 0)
          //   throw new Error(`Worker stopped with exit code ${code}`);

          process.stdout.write(`Worker ${i} exited with code ${code}\n`.yellow.dim)
        });

        worker.once("message", (data) => {
          process.stdout.write("once: " + data.msg);
        })
        worker.on("message", (data) => {
          process.stdout.write("on" + data.msg);
        })
        worker.send({ msg: 'Welcome!' })

        this.workers.push(worker);
      }
    }
  }

  static async race() {
    if (this.promises.size >= this.threads)
      await Promise.race(this.promises);
  }

  static async iterTree(game: Game) {
    await this.race();

    const id = this.id++;
    const worker = this.workers.pop()!;
    const promise = new Promise<Node>((resolve) => {
      // channel.once("message", (data) => {
      //   this.channels.push(channel);

      //   resolve(Minimax.from(data));
      // })
    })
    promise.then(() => this.promises.delete(promise));

    this.promises.add(promise);


    worker.send(<WorkerSolverData>{ game, id });

    return promise;
  }
}