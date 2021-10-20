import {
  isMainThread,
  parentPort,
  workerData,
  threadId
} from 'worker_threads';
import Game from '../Game';
import Analysis from './Analysis';
import Bar from './Bar';

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
  let minimax: string = workerData.minimax;
  let bar: Bar = workerData.bar;
  let index: number = workerData.index;
  let pillz: number = workerData.pillz;
  let fury: boolean = workerData.fury;
  let fullSearch: boolean = workerData.fullSearch;

  let log = console.log;
  console.log = () => { };
  let m = await Analysis.fillTree(
    game, minimax, bar, index, pillz, fury, fullSearch
  );

  log(`[${threadId.toString().red}] ${'Thread Ended'.grey}`);
  parentPort?.postMessage(m);
}