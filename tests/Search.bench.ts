// Time a full root-split Search over the same position as IterTree.bench.ts.
//
//   deno task time-search
//   deno task time-search --workers 3
//
// IterTree.bench.ts times the breadth-first `iterTree`, which is what `Main.ts` and the
// worker call. This times `Search`, which is what the live advisor drives and where any
// depth-first work lands.
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import { Turn } from "@/game/types/Types.ts";
import Game from "@/game/Game.ts";
import Search from "@/solver/Search.ts";
import ParallelSearch from "@/solver/ParallelSearch.ts";

const at = Deno.args.indexOf("--workers");
const workers = at < 0 ? 1 : Number(Deno.args[at + 1]);
if (!Number.isInteger(workers) || workers < 1) {
  throw new Error("--workers needs a positive integer");
}

const h1 = HandGenerator.generate("Genmaicha", "Orka", "Sando", "Deborah");
const h2 = HandGenerator.generate(
  "Nathan",
  "El Kuzco",
  "Noon Steevens",
  "Strygia",
);

const log = console.log, info = console.info;
console.log = () => 0;
console.info = () => 0;

const g = new Game(
  new Player(12, 12, 0),
  new Player(12, 12, 1),
  h1,
  h2,
  Turn.PLAYER_1,
  false,
);
g.select(0, 0, false, false);
g.select(0, 0, false, false);
g.select(1, 0, false, false);

const search: Search = workers === 1
  ? new Search(g)
  : new ParallelSearch(g, workers);
const started = Date.now();
while (!search.done) await search.workFor(100);
const ms = Date.now() - started;

console.log = log;
console.info = info;

// Print a checksum of the result as well as the time: a faster search that answers
// differently is not a faster search.
let sum = 0;
for (const c of search.candidates) {
  sum += c.average * (c.index + 1) * (c.pillz + 2);
}
console.log(
  `Search (${workers} worker${workers === 1 ? "" : "s"}): ${ms}ms  ` +
    `units ${search.units}  candidates ${search.candidates.length}  ` +
    `checksum ${sum.toFixed(6)}  best ${search.best()?.key}`,
);

if (search instanceof ParallelSearch) search.cancel();
