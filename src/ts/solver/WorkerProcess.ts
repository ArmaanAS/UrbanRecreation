import cluster from 'cluster'
import { cpus } from 'os'
import { baseCards } from '../CardLoader';
import { WorkerSolverData } from '../types/Types';
import { FlowController } from '../utils/Utils';
import { Node } from './Minimax';

export default class WorkerProcess<InputType extends object, OutputType> {
  worker: cluster.Worker;
  id: number;

  // isReady = false;
  // readyPromise: Promise<void>;

  private readyCallbacks = new Set<() => void>();
  private exitCallbacks = new Set<() => void>();

  // busy = false;

  constructor(id: number) {
    this.id = id;
    WorkerProcess.numWorkers++;
    this.startProcess();
  }

  startProcess() {
    WorkerProcess.loading++;

    cluster.setupMaster({
      exec: './solver/ProcSolver.js',
      inspectPort: 9230 + this.id,
      execArgv: [
        "--experimental-specifier-resolution=node",
        "--enable-source-maps",
        "--max-old-space-size=6350",
        "--trace-warnings",
        // "--inspect", // "--inspect-brk"
      ]
    })
    this.worker = cluster.fork();

    this.worker.on("online", () => {
      // process.stdout.write(`Worker ${this.id} is online\n`.green);
    })
    this.worker.on("error", (err) => {
      // throw err;
      process.stdout.write(`Cluster process error: ${err}\n`.red)
    });

    this.worker.on("exit", (code, signal) => {
      if (code !== 0) {
        // throw new Error(`Worker stopped with exit code ${code} with signal ${signal}`);
        process.stderr.write(
          `\n\n\nWorker stopped with exit code ${code} with signal ${signal}\n\n\n`.red
        );
      } else {
        process.stderr.write(
          `Worker ${this.id} exited with code ${code}\n`.yellow.dim
        );
      }

      // this.isReady = false;

      this.exitCallbacks.forEach(cb => cb());
      this.exitCallbacks.clear();

      this.startProcess();
    });

    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    this.worker.once("message", (data) => {
      this.worker.send(baseCards);
      // process.stdout.write(`Init message: ${data.msg.grey}\n`);

      // this.isReady = true;
      WorkerProcess.loading--;
      if (WorkerProcess.loading === 0)
        WorkerProcess.loadingFlow.resume();

      this.readyCallbacks.forEach(cb => cb());
      this.readyCallbacks.clear();
    })
  }

  onReady(cb: (...any: unknown[]) => void) {
    this.readyCallbacks.add(cb);
  }

  onExit(cb: () => void) {
    this.exitCallbacks.add(cb);
  }

  process(data: InputType) {
    const promise = new Promise<OutputType>((resolve) => {
      const onExit = () => {
        process.stderr.write(`onExit recursion called\n`.yellow);
        this.onReady(() => {
          resolve(this.process(data));
        })
      }
      this.onExit(onExit);

      this.worker.once("message", (data) => {
        // process.stdout.write(`once message recieved on process ${this.id}\n`.white);
        this.exitCallbacks.delete(onExit);


        // resolve(Minimax.from(data));
        resolve(data);

        WorkerProcess.processFlow.resume();
      })
    });

    // process.stdout.write(`sending data to process\n`);
    this.worker.send(data);

    return promise;
  }

  private static _init = false;
  private static threads = cpus().length;
  private static loading = 0;
  private static numWorkers = 0;
  private static workers: WorkerProcess<object, object>[] = [];
  private static processFlow = new FlowController();
  private static loadingFlow = new FlowController();

  static init() {
    if (cluster.isMaster && !this._init) {
      // process.stdout.write('WorkerProcess.init()\n'.blue)
      for (let i = 0; i < this.threads; i++) {
        this.create<WorkerSolverData, Node>(i);
      }

      this._init = true;
    }
  }

  static create<I extends object, O extends object>(id: number) {
    this.workers.push(new WorkerProcess<I, O>(id));
  }

  static async race() {
    // process.stdout.write(`Racing processes...\n`.yellow);
    if (this.loading)
      await this.loadingFlow.promise;

    if (this.workers.length === 0)
      await this.processFlow.promise;
    // process.stdout.write(`Process is free.\n`.green);
  }

  static async allFinished() {
    if (this.workers.length >= this.numWorkers)
      return;

    await this.processFlow.promise;
    await this.allFinished();
  }

  static async processOnWorker<I extends object, O extends object>(data: I): Promise<O> {
    if (!this.init)
      this.init();

    if (this.workers.length === 0)
      await this.race();

    const worker = this.workers.pop()! as WorkerProcess<I, O>;
    const promise = worker.process(data);
    promise.then(() => {
      // this.workers.push(worker);
      // this.workers.splice(0, 0, worker);
      this.workers = [worker, ...this.workers];
    });

    return promise;
  }
}

WorkerProcess.init();