/// <reference lib="deno.worker" />

// One isolate in the advisor's worker pool. A report contains only samples added since the
// previous report; repeatedly cloning every value found so far became surprisingly visible
// once three workers were posting progress several times a second.
import type GameType from "../game/Game.ts";

interface StartMessage {
  type: "start";
  game: GameType;
  stride: number;
  offset: number;
  reportMs: number;
  blindSecond: boolean;
}

const send = (message: unknown) => postMessage(message);

onmessage = (event: MessageEvent<StartMessage>) => {
  if (event.data.type !== "start") return;
  void run(event.data);
};

async function run(message: StartMessage) {
  try {
    // Imports load the card database, which normally narrates its startup. Worker console
    // output shares the advisor's terminal and would punch holes through the alternate
    // screen, so silence it before dynamically loading any engine module.
    console.log = () => {};
    console.info = () => {};
    console.debug = () => {};
    console.warn = () => {};
    const [{ default: Game }, { default: Search }] = await Promise.all([
      import("../game/Game.ts"),
      import("./Search.ts"),
    ]);
    // A structured clone keeps the graph but strips prototypes. Every worker has its own
    // module globals, which is important because the battle cache is process-global.
    const search = new Search(
      Game.from(message.game),
      message.stride,
      message.offset,
      message.blindSecond,
    );
    const sentValues = search.candidates.map(() => 0);
    const sentWeights = search.candidates.map(() => 0);
    const sentKos = search.candidates.map(() => 0);
    const sentKoed = search.candidates.map(() => 0);

    const report = (done: boolean) => {
      send({
        type: "progress",
        done,
        values: search.candidates.map((candidate, i) => {
          const values = candidate.values.slice(sentValues[i]);
          sentValues[i] = candidate.values.length;
          return values;
        }),
        weights: search.candidates.map((candidate, i) => {
          const weights = candidate.weights.slice(sentWeights[i]);
          sentWeights[i] = candidate.weights.length;
          return weights;
        }),
        kos: search.candidates.map((candidate, i) => {
          const delta = candidate.kos - sentKos[i];
          sentKos[i] = candidate.kos;
          return delta;
        }),
        koed: search.candidates.map((candidate, i) => {
          const delta = candidate.koed - sentKoed[i];
          sentKoed[i] = candidate.koed;
          return delta;
        }),
        unitsDone: search.stats.unitsDone,
        terminal: search.stats.terminal,
      });
    };

    while (!search.done) {
      const until = Date.now() + message.reportMs;
      while (Date.now() < until && search.step());
      report(search.done);
      // Let this isolate deliver control messages and let the main isolate draw the frame.
      if (!search.done) await new Promise((resolve) => setTimeout(resolve, 0));
    }
  } catch (error) {
    send({
      type: "error",
      message: error instanceof Error
        ? `${error.name}: ${error.message}`
        : String(error),
    });
  }
}
