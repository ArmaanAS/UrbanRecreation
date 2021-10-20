import Analysis from './analysis.mjs';
import {
  isMainThread,
  parentPort,
  workerData,
  threadId
} from 'worker_threads';
import Game from './game.mjs';

if (!isMainThread) {
  let log = console.log;
  log(`[${threadId.toString().green}] ${'Thread Started'.grey}`);

  console.log = () => { };
  let m = await Analysis.iterTree(Game.from(workerData), true);

  log(`[${threadId.toString().yellow}] ${'Thread Ended'.grey}`);
  parentPort.postMessage(m);
}