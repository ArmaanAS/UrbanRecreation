/// <reference lib="deno.worker" />

import Analysis from "@/solver/Analysis.ts";
import Game from "@/game/Game.ts";

const log = console.log;
console.log = () => 0;

const {
  game,
} = await new Promise((res) => onmessage = (e) => res(e.data)) as {
  game: Game;
};

log("\n\nCalculating best move...\n\n");

console.time("worker iterTree");
const best = Analysis.iterTree(Game.from(game as Game), true)!;
console.timeEnd("worker iterTree");
console.log = log;

log(best.best().toString());

// postMessage("Close");
postMessage(best);
