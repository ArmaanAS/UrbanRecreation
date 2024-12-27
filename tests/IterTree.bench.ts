import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import { Turn } from "@/game/types/Types.ts";
import Game from "@/game/Game.ts";
import Analysis from "@/solver/Analysis.ts";

const h1 = HandGenerator.generate(
  "Genmaicha",
  "Orka",
  "Sando",
  "Deborah",
);
const h2 = HandGenerator.generate(
  "Nathan",
  "El Kuzco",
  "Noon Steevens",
  "Strygia",
);
const p1 = new Player(12, 12, 0);
const p2 = new Player(12, 12, 1);

const g = new Game(p1, p2, h1, h2, Turn.PLAYER_1);

g.select(0, 0);
g.select(0, 0);

g.select(1, 0);

const log = console.log;
console.log = () => 0;
console.time("iterTree");

Analysis.iterTree(g);

console.log = log;
console.timeEnd("iterTree");
