import {
  isMainThread,
  parentPort,
  threadId,
  workerData,
} from "node:worker_threads";
import Analysis from "./Analysis.ts";

if (!isMainThread) {
  const log = console.log;
  log(`[${threadId.toString().green}] ${"Thread Started".grey}`);

  console.log = () => 0;
  // let m = await Analysis.iterTree(Game.from(workerData), true);
  const m = await Analysis.iterTree(workerData, true);

  log(`[${threadId.toString().yellow}] ${"Thread Ended".grey}`);
  parentPort?.postMessage(m);
}
