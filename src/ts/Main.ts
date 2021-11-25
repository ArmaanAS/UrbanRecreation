/* eslint-disable @typescript-eslint/no-unused-vars */
import Game from './game/Game';
import { HandGenerator } from './game/Hand';
import Player from './game/Player';
import Analysis from './solver/Analysis';
import { Turn } from './game/types/Types';
import GameRenderer from './utils/GameRenderer';


const p1 = new Player(12, 12, 0);
const p2 = new Player(12, 12, 1);

const h1 = HandGenerator.generate('Roderick', 'Frank', 'Katsuhkay', 'Oyoh'); // Roderick
const h2 = HandGenerator.generate('Behemoth Cr', 'Vholt', 'Eyrik', 'Kate');
// const h1 = HandGenerator.generate('Sando', 'Deborah', 'Orka', 'Genmaicha'); // Roderick
// const h2 = HandGenerator.generate('Strygia', 'El Kuzco', 'Noon Steevens', 'Nathan');
// const h1 = HandGenerator.generate('Kapel', 'Malicia', 'Brittany', 'Veronica'); // Roderick
// const h2 = HandGenerator.generate('Rik-L', 'Behemoth Cr', 'X-Hares', 'Dagg Cr');
// const h1 = HandGenerator.generate('Globumm Cr', 'Globumm Cr', 'Globumm Cr', 'Globumm Cr'); // Roderick
// const h2 = HandGenerator.generate('Behemoth Cr', 'Behemoth Cr', 'Behemoth Cr', 'Behemoth Cr');

const g = new Game(p1, p2, h1, h2, false, !false, false);
// const g = new Game(p1, p2, h1, h2, false, !false, false, Turn.PLAYER_2);
// const g = GameGenerator.create(false, !false, false);

// g.select(1, 0);
// g.select(3, 0);
// g.select(0, 4);
// g.select(0, 0);
// g.select(2, 0);
// g.select(2, 3);

// g.select(1, 5);

g.select(1, 0);
// g.select(0, 1);

// g.select(1, 0);
// g.select(0, 0);
// g.select(2, 0);
// g.select(2, 4);

// g.select(3, 5);
// g.select(0, 0);
// g.select(2, 0);
// g.select(3, 8);
// g.select(2, 7);

// GameRenderer.draw(g, true);

const log = console.log;

// console.dir(g, { depth: 6 })
// console.log(g.h1.get(0).clan)

while (!g.hasWinner(true) && true) {
  if (g.turn === Turn.PLAYER_1 && !false) {
    console.log = () => 0;
    console.time('a');
    const m = await Analysis.iterTree(g);
    console.log = log;
    console.timeEnd('a');

    log();

    const best = m.best();
    console.log(best.toString());

    await g.input(false);

  } else {
    console.log = () => 0;
    console.time('a');
    const m = await Analysis.iterTree(g, false);
    console.log = log;
    console.timeEnd('a');

    log();

    try {
      // console.log('tree', m.tree());
      // console.log('debug', JSON.stringify(m.debug(3)));

      // console.log('debug', m.debug(2));
      const best = m.best();
      console.log(best.toString())


      // let s = best.name.split(' ');
      // if (!g.select(+s[0], +s[1], s[2] == 'true')) {
      const res = g.select(best.index, best.pillz, best.fury);
      if (!res) {
        console.log(`Failed ${[typeof best.index, typeof best.pillz, typeof best.fury]} ${g.turn} ${res}`.red);
        // GameRenderer.draw(g, true);
        break;
      }

    } catch (e) {
      console.error(e);
      console.log(m);
    }
  }
  // GameRenderer.draw(g, true);
}

console.log('Ended.')
// process.exit(0)

// console.dir(g, { depth: 5 });