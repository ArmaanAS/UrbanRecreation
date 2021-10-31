import { parentPort } from 'worker_threads';

import Game, { GameGenerator } from '../Game';
import { CardJSON, HandOf } from '../types/CardTypes';
import GameRenderer from '../utils/GameRenderer';
import Analysis from './Analysis';

let g: Game;
let buffer: DataType[] = [];
// let running = false;


interface DataType {
  type: string;
}

interface RecreateType extends DataType {
  type: 'recreate';
  moves: DataType[];
}

interface InitType extends DataType {
  type: 'recreate';
  h1: HandOf<CardJSON>;
  h2: HandOf<CardJSON>;
  life: number;
  pillz: number;
  name1: string;
  name2: string;
  first: boolean;
}

interface MoveType extends DataType {
  type: 'move';
  index: number;
  pillz: number;
  fury: boolean;
}

interface SimulateType extends DataType {
  type: 'simulate';
  index: number;
}

function handleMessage() {
  // running = true;
  while (buffer.length) {
    const data: DataType = buffer.splice(0, 1)[0];

    if (data.type == 'recreate') {
      const d = data as RecreateType;

      buffer = d.moves;
      handleMessage();

    } else if (data.type == 'init') {
      const d = data as InitType;
      g = GameGenerator.createUnique(
        d.h1, d.h2,
        d.life, d.pillz,
        d.name1, d.name2,
        d.first
      );
      GameRenderer.draw(g, true);
      parentPort?.postMessage('Update');

    } else if (data.type == 'move') {
      const d = data as MoveType;
      g.select(d.index, d.pillz, d.fury || false);

      GameRenderer.draw(g, true);
      parentPort?.postMessage('Update');

    } else if (data.type == 'simulate') {
      const d = data as SimulateType;
      (async function () {
        const gc = g.clone();

        if (d.index !== undefined)
          gc.select(d.index, 0, false);

        const log = console.log;
        console.log = () => 0;
        console.time('a');
        const m = await Analysis.iterTree(gc);
        console.log = log;
        console.timeEnd('a');

        if (!g.first)
          m.turn = !m.turn;

        const best = m.best();
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
  // running = false;
}

parentPort?.on('message', function incoming(data) {
  // console.log('worker received:', data);
  buffer.push(data);

  console.log(`buffer: (${buffer.length})`, buffer);

  // setTimeout(handleMessage, 5000);
  handleMessage();
});