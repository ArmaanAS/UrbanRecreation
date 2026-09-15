import Game from "../game/Game.ts";
import { Turn } from "../game/types/Types.ts";
import Search, { type SearchStats } from "./Search.ts";

interface ProgressMessage {
  type: "progress";
  done: boolean;
  values: number[][];
  sampleIndexes: number[][];
  sampleFlags: number[][];
  weights: number[][];
  kos: number[];
  koed: number[];
  unitsDone: number;
  terminal: number;
}

interface ErrorMessage {
  type: "error";
  message: string;
}

type WorkerMessage = ProgressMessage | ErrorMessage;

interface WorkerState {
  worker: Worker;
  unitsDone: number;
  terminal: number;
  done: boolean;
}

/**
 * The same Search split over several Web Workers (separate V8 isolates and CPU threads).
 * Search already assigns every depth-2 state a flat number; stride/offset partitions those
 * states exactly once, so merging the samples reproduces the single-thread result.
 */
export default class ParallelSearch extends Search {
  private readonly states: WorkerState[] = [];
  private readonly started = Date.now();
  private readonly waiters = new Set<() => void>();
  private pooling = true;
  private cancelled = false;
  private failure?: string;
  private finished?: number;

  constructor(
    game: Game,
    private readonly count = 3,
    reportMs = 100,
    blindSecond = false,
  ) {
    super(game, 1, 0, blindSecond);
    if (!Number.isInteger(count) || count < 2) {
      throw new RangeError("ParallelSearch needs at least two workers");
    }

    const url = new URL("./SearchWorker.ts", import.meta.url);
    for (let offset = 0; offset < count; offset++) {
      const worker = new Worker(url.href, { type: "module" });
      const state: WorkerState = {
        worker,
        unitsDone: 0,
        terminal: 0,
        done: false,
      };
      this.states.push(state);
      worker.onmessage = (event: MessageEvent<WorkerMessage>) =>
        this.receive(offset, event.data);
      worker.onerror = (event) => {
        event.preventDefault();
        this.fallBack(`worker ${offset + 1}: ${event.message}`);
      };
      worker.onmessageerror = () =>
        this.fallBack(`worker ${offset + 1}: unreadable result`);
      worker.postMessage({
        type: "start",
        game,
        stride: count,
        offset,
        reportMs,
        blindSecond,
      });
    }
  }

  override get workerCount() {
    return this.pooling ? this.count : 1;
  }

  override get done() {
    return this.pooling ? this.states.every((state) => state.done) : super.done;
  }

  override get stats(): SearchStats {
    if (!this.pooling) return super.stats;
    return {
      units: this.units,
      unitsDone: this.states.reduce((sum, state) => sum + state.unitsDone, 0),
      terminal: this.states.reduce((sum, state) => sum + state.terminal, 0),
      ms: (this.finished ?? Date.now()) - this.started,
    };
  }

  /** If the pool cannot start, the untouched superclass search safely takes over. */
  get workerFailure() {
    return this.failure;
  }

  private receive(offset: number, message: WorkerMessage) {
    if (!this.pooling || this.cancelled) return;
    if (message.type === "error") {
      this.fallBack(`worker ${offset + 1}: ${message.message}`);
      return;
    }

    const state = this.states[offset];
    state.unitsDone = message.unitsDone;
    state.terminal = message.terminal;
    state.done ||= message.done;
    for (const [i, values] of message.values.entries()) {
      if (
        values.length === 0 && message.kos[i] === 0 && message.koed[i] === 0
      ) continue;
      const candidate = this.candidates[i];
      candidate.values.push(...values);
      candidate.sampleIndexes.push(...message.sampleIndexes[i]);
      candidate.sampleFlags.push(...message.sampleFlags[i]);
      candidate.weights.push(...message.weights[i]);
      candidate.done += values.length;
      candidate.kos += message.kos[i];
      candidate.koed += message.koed[i];
      this.foldMerged(candidate);
    }

    this.wake();
    if (this.done) {
      this.finished = Date.now();
      this.terminateWorkers();
    }
  }

  private foldMerged(candidate: (typeof this.candidates)[number]) {
    let total = 0, weight = 0;
    let extreme = this.us === Turn.PLAYER_1 ? Infinity : -Infinity;
    for (let i = 0; i < candidate.values.length; i++) {
      const value = candidate.values[i], w = candidate.weights[i] ?? 1;
      total += value * w;
      weight += w;
      if (this.us === Turn.PLAYER_1 ? value < extreme : value > extreme) {
        extreme = value;
      }
    }
    candidate.average = total / weight;
    candidate.minimax = extreme;
  }

  /** Yield until a worker reports, or until it is time for the next frame. */
  override async workFor(ms: number): Promise<void> {
    if (!this.pooling) return await super.workFor(ms);
    if (this.done || this.cancelled) return;
    await new Promise<void>((resolve) => {
      const finish = () => {
        clearTimeout(timer);
        this.waiters.delete(finish);
        resolve();
      };
      const timer = setTimeout(finish, ms);
      this.waiters.add(finish);
    });
  }

  /** A pooled search is advanced by its workers, never by the coordinator thread. */
  override step(): boolean {
    if (!this.pooling) return super.step();
    throw new Error("ParallelSearch must be advanced with workFor()");
  }

  /** Stop obsolete work immediately when the live game advances to another position. */
  cancel() {
    if (this.cancelled) return;
    this.cancelled = true;
    this.finished ??= Date.now();
    this.terminateWorkers();
    this.wake();
  }

  private fallBack(message: string) {
    if (!this.pooling || this.cancelled) return;
    this.failure = message;
    this.pooling = false;
    this.terminateWorkers();
    // The superclass search has not stepped yet. Remove worker samples before it takes over
    // or those units would be counted twice.
    for (const candidate of this.candidates) {
      candidate.values.length = 0;
      candidate.sampleIndexes.length = 0;
      candidate.sampleFlags.length = 0;
      candidate.weights.length = 0;
      candidate.average = NaN;
      candidate.minimax = NaN;
      candidate.done = 0;
      candidate.kos = 0;
      candidate.koed = 0;
    }
    this.wake();
  }

  private terminateWorkers() {
    for (const state of this.states) state.worker.terminate();
  }

  private wake() {
    for (const resolve of this.waiters) resolve();
    this.waiters.clear();
  }
}
