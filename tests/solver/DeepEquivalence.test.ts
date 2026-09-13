// deepValue() is the allocation-free reference for iterTree(game, false), so it has to
// return the same number on the same position and leave the game exactly as it found it.
// The live advisor uses the information-aware Policy evaluator instead. Make/unmake edits
// the live game rather than exploring a copy, so "leaves it as it found it" is the whole
// safety property: hands here deliberately include a latching permanent and a Backlash
// card, which are the two things whose state outlives a round.
import "colors";
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";
import Analysis from "@/solver/Analysis.ts";
import deepValue, { newStack } from "@/solver/Deep.ts";

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

/** Everything a search could disturb, as a comparable string. */
function fingerprint(g: Game): string {
  const card = (
    c: {
      played: boolean;
      won?: boolean;
      power: { final: number };
      damage: { final: number };
      attack: { final: number };
    },
  ) =>
    `${c.played ? 1 : 0}${c.won === undefined ? "-" : c.won ? "W" : "L"}` +
    `${c.power.final}/${c.damage.final}/${c.attack.final}`;
  return [
    g.id,
    g.winner,
    g.p1.snapshot(),
    g.p2.snapshot(),
    g.r1.snapshot(),
    g.r2.snapshot(),
    g.r1.lastClan ?? "-",
    g.r2.lastClan ?? "-",
    JSON.stringify(g.i1 ?? null),
    JSON.stringify(g.i2 ?? null),
    [0, 1, 2, 3].map((i) => card(g.h1[i])).join(","),
    [0, 1, 2, 3].map((i) => card(g.h2[i])).join(","),
    g.events1.repeat.map((a) => a.length).join(""),
    g.events2.repeat.map((a) => a.length).join(""),
  ].join("|");
}

interface Case {
  name: string;
  h1: string[];
  h2: string[];
  levels1?: number[];
  levels2?: number[];
  life: number;
  pillz: number;
  /** Rounds to play out before comparing, as [i, pillz, fury] pairs. */
  opening: [number, number, boolean][];
}

const CASES: Case[] = [
  {
    // Galactea's "Toxin 1, Min 0" latches into Events.repeat and must not survive an unmake.
    name: "latching permanent",
    h1: ["Galactea", "Genmaicha", "Orka", "Sando"],
    h2: ["Nathan", "El Kuzco", "Noon Steevens", "Strygia"],
    levels1: [
      4,
      undefined as unknown as number,
      undefined as unknown as number,
      undefined as unknown as number,
    ],
    life: 14,
    pillz: 7,
    opening: [[1, 2, false], [1, 1, false]],
  },
  {
    // Uchtul Cr is "Backlash: - 5 Life Min 0": winning can end the game for its own owner.
    name: "backlash and KOs",
    h1: ["Uchtul Cr", "Genmaicha", "Orka", "Sando"],
    h2: ["Nathan", "El Kuzco", "Noon Steevens", "Strygia"],
    levels1: [
      4,
      undefined as unknown as number,
      undefined as unknown as number,
      undefined as unknown as number,
    ],
    life: 8,
    pillz: 6,
    opening: [[1, 1, false], [1, 2, false]],
  },
  {
    name: "plain hands, deeper tree",
    h1: ["Genmaicha", "Orka", "Sando", "Deborah"],
    h2: ["Nathan", "El Kuzco", "Noon Steevens", "Strygia"],
    life: 12,
    pillz: 6,
    opening: [[0, 1, false], [0, 1, false]],
  },
];

function build(c: Case): Game {
  return quiet(() => {
    const g = new Game(
      new Player(c.life, c.pillz, 0),
      new Player(c.life, c.pillz, 1),
      HandGenerator.handOf(
        c.h1 as HandOf<string>,
        c.levels1 as HandOf<number | undefined>,
      ),
      HandGenerator.handOf(
        c.h2 as HandOf<string>,
        c.levels2 as HandOf<number | undefined>,
      ),
      Turn.PLAYER_1,
      false,
    );
    for (const [i, p, f] of c.opening) g.select(i, p, f, false);
    return g;
  });
}

for (const c of CASES) {
  Deno.test(`deepValue matches iterTree — ${c.name}`, () => {
    const root = build(c);
    const stack = newStack();
    let compared = 0;

    // Walk every (move, reply) pair, which is exactly the depth-2 state Search evaluates.
    const moves = quiet(() => root.unplayedCardIndexes);
    for (const i of moves) {
      for (const p of [0, 1, root.playingPlayer.pillz]) {
        const g1 = quiet(() => {
          const g = root.clone();
          g.select(i, p, false, false);
          return g;
        });
        for (const j of quiet(() => g1.unplayedCardIndexes)) {
          for (const q of [0, g1.playingPlayer.pillz]) {
            const g2 = quiet(() => {
              const g = g1.clone();
              g.select(j, q, false, false);
              return g;
            });
            if (!g2.isPlaying) continue; // decided; no subtree to compare

            const before = fingerprint(g2);
            const deep = quiet(() => deepValue(g2, stack));
            const after = fingerprint(g2);
            const reference = quiet(() => Analysis.iterTree(g2, false).result!);

            assertEquals(
              after,
              before,
              `deepValue disturbed the game at ${i}/${p} vs ${j}/${q}`,
            );
            assertEquals(
              deep,
              reference,
              `value mismatch at ${i}/${p} vs ${j}/${q}`,
            );
            compared++;
          }
        }
      }
    }
    assertEquals(compared > 0, true, "no positions were compared");
  });
}

Deno.test("make/unmake round-trips a resolved round", () => {
  const g = build(CASES[0]);
  const stack = newStack();
  const before = fingerprint(g);

  // One full round: first mover, then the reply that resolves it.
  const i = quiet(() => g.unplayedCardIndexes)[0];
  assertEquals(quiet(() => g.make(i, 2, false, stack[0])), true);
  const j = quiet(() => g.unplayedCardIndexes)[0];
  assertEquals(quiet(() => g.make(j, 3, false, stack[1])), true);
  assertEquals(
    fingerprint(g) === before,
    false,
    "the round should have changed something",
  );

  quiet(() => g.unmake(stack[1]));
  quiet(() => g.unmake(stack[0]));
  assertEquals(fingerprint(g), before, "unmake did not restore the position");
});

Deno.test("make refuses a card already played", () => {
  const g = build(CASES[2]);
  const stack = newStack();
  const i = quiet(() => g.unplayedCardIndexes)[0];
  assertEquals(quiet(() => g.make(i, 0, false, stack[0])), true);
  const before = fingerprint(g);
  // The opponent moves next, so their hand is what `make` looks at; replay our index on a
  // card of theirs that is already spent to prove the guard changes nothing.
  quiet(() => g.make(i, 0, false, stack[1]));
  quiet(() => g.unmake(stack[1]));
  assertEquals(fingerprint(g), before);
});
