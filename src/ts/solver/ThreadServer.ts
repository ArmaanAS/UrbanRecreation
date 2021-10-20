import {
  isMainThread,
  parentPort,
  workerData,
  threadId
} from 'worker_threads';

import Game from '../Game';
import Analysis from './Analysis';

let g: Game;
let buffer: any[] = [];
let running = false;

function handleMessage() {
  running = true;
  while (buffer.length) {
    let data = buffer.splice(0, 1)[0];

    if (data.type == 'recreate') {
      buffer = data.moves;
      handleMessage();

    } else if (data.type == 'init') {
      g = Game.createUnique(
        data.h1, data.h2,
        data.life, data.pillz,
        data.name1, data.name2,
        data.first
      );
      g.log(true);
      parentPort?.postMessage('Update');

    } else if (data.type == 'move') {
      g.select(data.index, data.pillz, data.fury || false);
      g.log(true);
      parentPort?.postMessage('Update');

    } else if (data.type == 'simulate') {
      (async function () {
        let gc = g.clone();

        if (data.index != undefined) {
          gc.select(data.index, 0, false);
        }

        let log = console.log;
        console.log = () => { };
        console.time('a');
        let m = await Analysis.iterTree(gc);
        console.log = log;
        console.timeEnd('a');

        if (!g.round.first) {
          m.turn = !m.turn;
        }
        let best = m.best();
        console.log(best.toString());

        parentPort?.postMessage({
          type: 'play',
          index: best.index,
          pillz: best.pillz,
          fury: best.fury
        });

        // m.defer = !m.defer;
        // m.turn = !m.turn;
        // console.log(m.best().toString());
      })();

    } else console.log('Unknown data:', data);
  }
  running = false;
}

parentPort?.on('message', function incoming(data) {
  // console.log('worker received:', data);
  buffer.push(data);

  console.log(`buffer: (${buffer.length})`, buffer);

  // setTimeout(handleMessage, 5000);
  handleMessage();
});