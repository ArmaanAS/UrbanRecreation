import {
  isMainThread,
  parentPort,
  workerData,
  threadId
} from 'worker_threads';
import Analysis from './Analysis';

if (!isMainThread) {
  let log = console.log;
  log(`[${threadId.toString().green}] ${'Thread Started'.grey}`);

  console.log = () => { };
  // let m = await Analysis.iterTree(Game.from(workerData), true);
  const m = await Analysis.iterTree(workerData, true);

  log(`[${threadId.toString().yellow}] ${'Thread Ended'.grey}`);
  parentPort?.postMessage(m);
}