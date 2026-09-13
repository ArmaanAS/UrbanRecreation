import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { Turn } from "@/game/types/Types.ts";
import Search from "@/solver/Search.ts";
import ParallelSearch from "@/solver/ParallelSearch.ts";
import { assertAlmostEquals, assertEquals } from "@std/assert";

const quiet = <T>(f: () => T): T => {
  const log = console.log, info = console.info;
  console.log = () => 0;
  console.info = () => 0;
  try {
    return f();
  } finally {
    console.log = log;
    console.info = info;
  }
};

function finalRound() {
  return quiet(() => {
    // High life keeps the fixture alive through three deliberately cheap rounds. The last
    // round still exercises hundreds of independent units but every subtree is terminal,
    // keeping this worker integration test quick.
    const game = new Game(
      new Player(50, 12, 0),
      new Player(50, 12, 1),
      HandGenerator.handOf(["Genmaicha", "Orka", "Sando", "Deborah"]),
      HandGenerator.handOf(["Nathan", "El Kuzco", "Noon Steevens", "Strygia"]),
      Turn.PLAYER_1,
      false,
    );
    for (let index = 0; index < 3; index++) {
      game.select(index, 0, false, false);
      game.select(index, 0, false, false);
    }
    return game;
  });
}

Deno.test("ParallelSearch merges worker slices into the single-thread answer", async () => {
  const game = finalRound();
  const serial = new Search(game);
  quiet(() => {
    while (serial.step());
  });

  const parallel = new ParallelSearch(game, 3, 10);
  while (!parallel.done) await parallel.workFor(50);

  assertEquals(parallel.stats.unitsDone, serial.stats.unitsDone);
  assertEquals(
    parallel.candidates.map((c) => c.key),
    serial.candidates.map((c) => c.key),
  );
  for (const [i, candidate] of parallel.candidates.entries()) {
    assertEquals(candidate.done, serial.candidates[i].done, candidate.key);
    assertEquals(candidate.kos, serial.candidates[i].kos, candidate.key);
    assertEquals(candidate.koed, serial.candidates[i].koed, candidate.key);
    assertAlmostEquals(
      candidate.average,
      serial.candidates[i].average,
      1e-12,
      candidate.key,
    );
    assertAlmostEquals(
      candidate.minimax,
      serial.candidates[i].minimax,
      1e-12,
      candidate.key,
    );
  }
  parallel.cancel();
});

Deno.test("ParallelSearch merges blind-second estimates before a card is revealed", async () => {
  const game = finalRound();
  const serial = new Search(game, 1, 0, true);
  quiet(() => {
    while (serial.step());
  });

  const parallel = new ParallelSearch(game, 3, 10, true);
  while (!parallel.done) await parallel.workFor(50);

  assertEquals(parallel.stats.unitsDone, serial.stats.unitsDone);
  assertEquals(
    parallel.candidates.map((candidate) => candidate.key),
    serial.candidates.map((candidate) => candidate.key),
  );
  for (const [index, candidate] of parallel.candidates.entries()) {
    assertEquals(candidate.done, serial.candidates[index].done, candidate.key);
    assertAlmostEquals(
      candidate.average,
      serial.candidates[index].average,
      1e-12,
      candidate.key,
    );
    assertAlmostEquals(
      candidate.minimax,
      serial.candidates[index].minimax,
      1e-12,
      candidate.key,
    );
  }
  parallel.cancel();
});
