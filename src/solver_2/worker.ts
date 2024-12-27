/// <reference lib="deno.worker" />

import Analysis from "@/solver/Analysis.ts";
import Game from "@/game/Game.ts";

const log = console.log;
console.log = () => 0;

const data = await new Promise((res) => onmessage = (e) => res(e.data));
const game = Game.from(data as Game);

log("\n\nCalculating best move...\n\n");

console.time("worker iterTree");
const best = Analysis.iterTree(game);
console.timeEnd("worker iterTree");
console.log = log;

log(best.best().toString());

postMessage("Close");
