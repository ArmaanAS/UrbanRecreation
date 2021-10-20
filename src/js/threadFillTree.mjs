import Analysis from './analysis.mjs';
import {
  isMainThread,
  parentPort,
  workerData,
  threadId
} from 'worker_threads';
import Game from './game.mjs';

//   game: game,
//   minimax: minimax,
//   bar: null,
//   index: index,
//   pillz: pillz,
//   fury: fury,
//   fullSearch: fullSearch,

if (!isMainThread) {
  console.log(`[${threadId.toString().green}] ${'Thread Started'.grey}`);

  let game = Game.from(workerData.game);
  let minimax = workerData.minimax;
  let bar = workerData.bar;
  let index = workerData.index;
  let pillz = workerData.pillz;
  let fury = workerData.fury;
  let fullSearch = workerData.fullSearch;

  let log = console.log;
  console.log = () => {};
  let m = await Analysis.fillTree(game, minimax, bar, index, pillz, fury, fullSearch);

  log(`[${threadId.toString().brightRed}] ${'Thread Ended'.grey}`);
  parentPort.postMessage(m);
}