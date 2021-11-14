import {
  isMainThread,
  parentPort,
  workerData,
  threadId
} from 'worker_threads';
import Game from '../game/Game';
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

  const game = Game.from(workerData.game);
  const minimax: string = workerData.minimax;
  const bar: Bar = workerData.bar;
  const index: number = workerData.index;
  const pillz: number = workerData.pillz;
  const fury: boolean = workerData.fury;
  const fullSearch: boolean = workerData.fullSearch;

  const log = console.log;
  console.log = () => 0;
  const m = await Analysis.fillTree(
    game, minimax, bar, index, pillz, fury, fullSearch
  );

  log(`[${threadId.toString().red}] ${'Thread Ended'.grey}`);
  parentPort?.postMessage(m);
}