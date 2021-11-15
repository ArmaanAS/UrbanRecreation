import { parentPort } from 'worker_threads';

import Game, { GameGenerator } from '../game/Game';
import { CardJSON, HandOf } from '../game/types/CardTypes';
import { Turn } from '../game/types/Types';
import GameRenderer from '../utils/GameRenderer';
import Analysis from './Analysis';

let g: Game;
let buffer: DataType[] = [];


type DataTypes = 'recreate' | 'init' | 'move' | 'simulate';

interface DataType {
  type: DataTypes;
}

interface RecreateType extends DataType {
  type: 'recreate';
  moves: DataType[];
}

interface InitType extends DataType {
  type: 'init';
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
  player1?: { life: number; pillz: number };
  player2?: { life: number; pillz: number };
}

interface SimulateType extends DataType {
  type: 'simulate';
  index: number;
}

async function handleMessage() {
  // running = true;
  while (buffer.length) {
    const data: DataType = buffer.splice(0, 1)[0];

    if (data.type == 'recreate') {
      const d = data as RecreateType;

      buffer = d.moves;
      handleMessage();

    } else if (data.type == 'init') {
      const d = data as InitType;
      console.log("FIRST:", d.first);
      g = GameGenerator.createUnique(
        d.h1, d.h2,
        d.life, d.pillz,
        // d.name1, d.name2,
        d.first ? Turn.PLAYER_1 : Turn.PLAYER_2
      );
      GameRenderer.draw(g, true);
      parentPort?.postMessage('Update');

    } else if (data.type == 'move') {
      const d = data as MoveType;
      g.select(d.index, d.pillz, d.fury || false);

      if (d.player1 !== undefined && d.player2 !== undefined) {
        g.p1.life = d.player1.life;
        g.p2.life = d.player2.life;

        g.p1.pillz = d.player1.pillz;
        g.p2.pillz = d.player2.pillz;
      }

      GameRenderer.draw(g, true);
      parentPort?.postMessage('Update');

    } else if (data.type == 'simulate') {
      const d = data as SimulateType;
      const gc = g.clone();

      // TODO: Fix { 'type': 'simulate, 'index': 1 }
      if (d.index !== undefined)
        gc.select(d.index, 0, false);


      if (gc.round > 1) {
        console.log('Simulating...')
        const log = console.log;
        console.log = () => 0;
        console.time('a');
        // const m = await Analysis.iterTree(gc, gc.firstHasSelected);
        const m = await Analysis.iterTree(gc);
        console.log = log;
        console.timeEnd('a');

        // if (!g.first)
        //   m.turn = !m.turn;

        const best = m.best();
        // console.log(best.debug());
        console.log(best.toString());

        parentPort?.postMessage({
          type: 'play',
          index: best.index,
          pillz: best.pillz,
          fury: best.fury
        });
      }

      // m.defer = !m.defer;
      // m.turn = !m.turn;
      // console.log(m.best().toString());

    } else console.log('Unknown data:', data);
  }
  // running = false;
}

parentPort?.on('message', function incoming(data) {
  // console.log('worker received:', data);
  buffer.push(data);

  console.log(`(${buffer.length}) buffer`);
  console.dir(buffer, { depth: 2 });

  // setTimeout(handleMessage, 5000);
  handleMessage();
});