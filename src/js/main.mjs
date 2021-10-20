import Analysis from './analysis.mjs';
import Game from './game.mjs';

let g = Game.create(false, !false, false);
// g.select(2, 3);
// g.select(0, 0);
// g.select(1, 3);
// g.select(0, 0);
// g.select(1, 0);
// g.select(2, 0);

g.select(1, 0);
g.select(0, 1);

// g.select(1, 0);
// g.select(1, 4);
// g.select(0, 0);
// g.select(2, 4);
g.log(true);

let log = console.log;

while (!g.checkWinner(true)) {
  if (g.getTurn() == 'Player' && !false) {
    await g.input(false);
  } else {

    console.log = () => { };
    console.time('a');
    let m = await Analysis.iterTree(g);
    console.log = log;
    console.timeEnd('a');

    log();

    try {
      // console.log('tree', m.tree());
      // console.log('debug', JSON.stringify(m.debug(3)));

      // console.log('debug', m.debug(2));
      let best = m.best();
      console.log(best.toString());


      // let s = best.name.split(' ');
      // if (!g.select(+s[0], +s[1], s[2] == 'true')) {
      let res = g.select(best.index, best.pillz, best.fury);
      if (!res) {
        console.log(`Failed ${[typeof best.index, typeof best.pillz, typeof best.fury]} ${g.getTurn()} ${res}`.red);
        g.log(true);
        // console.log(g);
        // console.log(g.h1);
        // console.log(g.h2);
        break;
      }

    } catch (e) {
      console.error(e);
      console.log(m);
    }
  }
  g.log(true);
}

console.log('Ended.')
process.exit();



// Analysis.threadedMove({
//   threaded: 2
// }, g, 0, 0);
// Analysis.threadedMove({
//   threaded: 2
// }, g, 1, 0);
// Analysis.threadedMove({
//   threaded: 2
// }, g, 2, 0);
// Analysis.threadedMove({
//   threaded: 2
// }, g, 3, 0);

// async function a() {
//   console.log(await Analysis.best(g, true));
// }

// a();