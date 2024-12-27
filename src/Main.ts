import Game from "./game/Game.ts";
import { HandGenerator } from "./game/Hand.ts";
import Player from "./game/Player.ts";
import Analysis from "./solver/Analysis.ts";
import { Turn } from "./game/types/Types.ts";
import { HandOf } from "@/game/types/CardTypes.ts";
import { Hono } from "hono";
import { cors } from "hono/cors";

const log = console.log;

interface Body {
  h1: HandOf<string | number>;
  h2: HandOf<string | number>;
  life: number;
  pillz: number;
  first: boolean;
}

async function playGame(body: Body) {
  let g: Game;
  block: {
    const p1 = new Player(body.life, body.pillz, 0);
    const p2 = new Player(body.life, body.pillz, 1);
    const h1 = HandGenerator.generate(...body.h1);
    const h2 = HandGenerator.generate(...body.h2);
    g = new Game(p1, p2, h1, h2, body.first ? Turn.PLAYER_1 : Turn.PLAYER_2);

    const answer = prompt("Is this correct? (y/n)", "")?.trim().toLowerCase();

    if (answer?.startsWith("y")) {
      break block;
    }

    g = new Game(p1, p2, h1, h2, body.first ? Turn.PLAYER_2 : Turn.PLAYER_1);

    console.log("Okay so this must be it then...");
  }

  while (!g.hasWinner(true) && true) {
    if (g.turn === Turn.PLAYER_1 && !false) {
      if (g.round > 1) {
        Analysis.iterTreeWorker(g);
      }

      if (g.firstHasSelected) {
        g.deselect();

        log("Select 2nd player's card and pillz...\n\n");
        g.input(false);

        if (g.hasWinner()) break;
      }

      g.input(false);
    } else {
      if (!g.firstHasSelected) {
        log("Just select 2nd player's card...\n\n");
      }
      g.input(false);
    }
  }
}

const app = new Hono();

app.use(cors());
app.post("/", async (c) => {
  const body = await c.req.json();
  playGame(body);
  return c.text("Started");
});

Deno.serve(app.fetch);

// const h1 = HandGenerator.generate("Roderick", "Frank", "Katsuhkay", "Oyoh"); // Roderick
// const h2 = HandGenerator.generate("Behemoth Cr", "Vholt", "Eyrik", "Kate");
// const h1 = HandGenerator.generate(
//   "Genmaicha",
//   "Orka",
//   "Sando",
//   "Deborah",
// );
// const h2 = HandGenerator.generate(
//   "Nathan",
//   "El Kuzco",
//   "Noon Steevens",
//   "Strygia",
// );
// const h1 = HandGenerator.generate('Kapel', 'Malicia', 'Brittany', 'Veronica'); // Roderick
// const h2 = HandGenerator.generate('Rik-L', 'Behemoth Cr', 'X-Hares', 'Dagg Cr');
// const h1 = HandGenerator.generate('Globumm Cr', 'Globumm Cr', 'Globumm Cr', 'Globumm Cr'); // Roderick
// const h2 = HandGenerator.generate('Behemoth Cr', 'Behemoth Cr', 'Behemoth Cr', 'Behemoth Cr');

// const h1 = HandGenerator.generate(
//   0,
//   "",
//   "",
//   "",
// );
// const h2 = HandGenerator.generate(
//   "",
//   "",
//   "",
//   "",
// );

// const p1 = new Player(12, 12, 0);
// const p2 = new Player(12, 12, 1);
// const g = new Game(p1, p2, h1, h2, false, !false, false);

// g.input(false);
// // g.input(false);

// const g = new Game(p1, p2, h1, h2, false, !false, false, Turn.PLAYER_2);
// const g = GameGenerator.create(false, !false, false);

// g.select(1, 0);
// g.select(3, 0);
// g.select(0, 4);
// g.select(0, 0);
// g.select(2, 0);
// g.select(2, 3);

// g.select(1, 5);

// g.select(1, 0);
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

// console.dir(g, { depth: 6 })
// console.log(g.h1.get(0).clan)

// while (!g.hasWinner(true) && true) {
//   if (g.turn === Turn.PLAYER_1 && !false) {
//     log("\n\nCalculating best move...\n\n");

//     console.log = () => 0;
//     console.time("a");
//     const m = await Analysis.iterTree(g);
//     console.log = log;
//     console.timeEnd("a");

//     log();

//     const best = m.best();
//     console.log(best.toString());

//     if (g.firstHasSelected) {
//       g.deselect();

//       log("Select 2nd player's card and pillz...\n\n");
//       g.input(false);
//     }

//     g.input(false);
//   } else {
//     if (!g.firstHasSelected) {
//       log("Just select 2nd player's card...\n\n");
//     }
//     g.input(false);
//     // log("\n\nCalculating best move...\n\n");

//     // console.log = () => 0;
//     // console.time("a");
//     // const m = await Analysis.iterTree(g, false);
//     // console.log = log;
//     // console.timeEnd("a");

//     // log();

//     // try {
//     //   // console.log('tree', m.tree());
//     //   // console.log('debug', JSON.stringify(m.debug(3)));

//     //   // console.log('debug', m.debug(2));
//     //   const best = m.best();
//     //   console.log(best.toString());

//     //   // let s = best.name.split(' ');
//     //   // if (!g.select(+s[0], +s[1], s[2] == 'true')) {
//     //   const res = g.select(best.index, best.pillz, best.fury);
//     //   if (!res) {
//     //     console.log(
//     //       `Failed ${[
//     //         typeof best.index,
//     //         typeof best.pillz,
//     //         typeof best.fury,
//     //       ]} ${g.turn} ${res}`.red,
//     //     );
//     //     // GameRenderer.draw(g, true);
//     //     break;
//     //   }
//     // } catch (e) {
//     //   console.error(e);
//     //   console.log(m);
//     // }
//   }
//   // GameRenderer.draw(g, true);
// }

// console.log("Ended.");
// process.exit(0)

// console.dir(g, { depth: 5 });
