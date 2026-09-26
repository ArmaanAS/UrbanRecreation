// The continuation cache may only make policyValue faster. Every root unit Search can
// evaluate - both players' moves in round one of an exact opening, and every unit of a
// round-two and a round-three root - is solved once through one shared cache, as a Search
// does, and once with no cache at all; the values must be identical and the game must be
// left exactly as it was found. The hands are DeepEquivalence's, chosen because a latched
// permanent (Galactea's "Toxin 1, Min 0") and a Backlash card (Uchtul Cr) are the state
// that outlives a round, which is what the cache key has to capture. A third pair of hands
// latches a delayed Poison and Heal, one on each side. Two more carry the other state a
// later round reads: a Growth permanent that freezes its amount when it latches (Mildred),
// Revenge and Confidence (Boomer, Selene), "After [clan]" against the previous round's clan
// (Rauta, Frau Vanda), an Oculus infiltration (Dark Smokey), and a match played at night.
import "colors";
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game, { Winner } from "@/game/Game.ts";
import { assert, assertEquals, assertNotEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";
import { shiftRange } from "@/utils/Utils.ts";
import policyValue, { newPolicyStack, type PolicyStack } from "@/solver/Policy.ts";

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

/** Everything a search could disturb, including each latched permanent's flags. */
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
  const repeat = (e: Game["events1"]) =>
    e.repeat.map((bucket) =>
      bucket.map((a) => `${a.ability}:${a.won}:${a.delayed}`).join(",")
    ).join(";");
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
    repeat(g.events1),
    repeat(g.events2),
    g.events1.events.map((a) => a.length).join(""),
    g.events2.events.map((a) => a.length).join(""),
    g.events1.mask,
    g.events2.mask,
  ].join("|");
}

interface Case {
  name: string;
  h1: string[];
  h2: string[];
  levels1?: (number | undefined)[];
  life: number;
  pillz: number;
  /**
   * Two rounds, as [i, pillz, fury] in playing order (P1 moves first in round one, P2 in
   * round two): the first pair reaches round two, both reach round three.
   */
  rounds: [number, number, boolean][];
  /** Clint City at night. */
  night?: boolean;
  /** A permanent latched before the round-two root is still in every unit under it. */
  latchedThroughout?: boolean;
}

const CASES: Case[] = [
  {
    name: "latching permanent",
    h1: ["Galactea", "Genmaicha", "Orka", "Sando"],
    h2: ["Nathan", "El Kuzco", "Noon Steevens", "Strygia"],
    levels1: [4, undefined, undefined, undefined],
    life: 14,
    pillz: 7,
    // Galactea wins round one here, so her Toxin is latched from round two on.
    rounds: [[0, 3, false], [1, 1, false], [2, 1, false], [1, 0, false]],
    latchedThroughout: true,
  },
  {
    name: "backlash and KOs",
    h1: ["Uchtul Cr", "Genmaicha", "Orka", "Sando"],
    h2: ["Nathan", "El Kuzco", "Noon Steevens", "Strygia"],
    levels1: [4, undefined, undefined, undefined],
    life: 8,
    pillz: 6,
    // Uchtul Cr stays in hand through both rounds, so every root still holds Backlash.
    rounds: [[1, 0, false], [1, 0, false], [0, 0, false], [2, 0, false]],
  },
  {
    // Two delayed permanents, one a side: Rosa's "Poison 1, Min 1" latches in round one and
    // Cherry's "Heal 2 Max. 10" in round two, so the round-three root carries both.
    name: "delayed permanents on both sides",
    h1: ["Rosa", "Genmaicha", "Orka", "Sando"],
    h2: ["Cherry", "El Kuzco", "Noon Steevens", "Strygia"],
    life: 12,
    pillz: 5,
    rounds: [[0, 1, false], [1, 0, false], [0, 0, false], [2, 0, false]],
    latchedThroughout: true,
  },
  {
    // Mildred wins round two on four pillz, so the round-three root carries her Growth Heal
    // frozen at twice its printed amount; the previous rounds' clans feed Rauta's "After".
    name: "growth freeze, revenge, after clan and oculus",
    h1: ["Mildred", "Boomer", "Selene", "Abby Salia"],
    levels1: [2, 3, undefined, 4],
    h2: ["Scavros", "Rauta", "Maelt Riv", "Dark Smokey"],
    life: 14,
    pillz: 5,
    rounds: [[1, 0, false], [0, 0, false], [1, 0, false], [0, 4, false]],
  },
  {
    name: "night, after clan, toxin and consume",
    h1: ["Frau Vanda", "Huldra", "Maelt Riv", "Vande"],
    h2: ["Mildred", "Boomer", "Selene", "Abby Salia"],
    life: 13,
    pillz: 5,
    night: true,
    rounds: [[1, 1, false], [0, 0, false], [1, 2, false], [0, 1, false]],
    latchedThroughout: true,
  },
];

function build(c: Case, rounds: number): Game {
  return quiet(() => {
    const g = new Game(
      new Player(c.life, c.pillz, 0),
      new Player(c.life, c.pillz, 1),
      HandGenerator.handOf(
        c.h1 as HandOf<string>,
        c.levels1 as HandOf<number | undefined>,
      ),
      HandGenerator.handOf(c.h2 as HandOf<string>),
      Turn.PLAYER_1,
      false,
      c.night ?? false,
    );
    for (const [i, p, f] of c.rounds.slice(0, rounds * 2)) {
      g.select(i, p, f, false);
    }
    return g;
  });
}

/** Search's own enumeration: every unplayed card, every bet, Fury where affordable. */
function moves(game: Game): [number, number, boolean][] {
  const out: [number, number, boolean][] = [];
  const pillz = game.playingPlayer.pillz;
  for (const index of game.unplayedCardIndexes) {
    for (const p of shiftRange(pillz)) {
      for (const fury of p <= pillz - 3 ? [true, false] : [false]) {
        out.push([index, p, fury]);
      }
    }
  }
  return out;
}

const latched = (g: Game) =>
  [g.events1, g.events2].some((e) =>
    e.repeat.some((bucket) => bucket.some((a) => a.won === true))
  );

interface Tally {
  units: number;
  latched: number;
}

/**
 * Solve every depth-2 unit under `root` for both askers - FIRST asks as the mover, SECOND
 * and blind-second as the reply - through `cached`, and compare each against a cache-free
 * solve of the same position.
 */
function compareRoot(root: Game, cached: PolicyStack, tally: Tally) {
  const uncached = newPolicyStack(false);
  for (const [i, p, f] of moves(root)) {
    const outer = quiet(() => {
      const g = root.clone();
      g.select(i, p, f, false);
      return g;
    });
    for (const [j, q, h] of moves(outer)) {
      const game = quiet(() => {
        const g = outer.clone();
        g.select(j, q, h, false);
        return g;
      });
      if (game.winner !== Winner.PLAYING) continue;
      if (latched(game)) tally.latched++;
      for (const us of [Turn.PLAYER_1, Turn.PLAYER_2]) {
        const before = fingerprint(game);
        const fast = quiet(() => policyValue(game, us, cached));
        assertEquals(
          fingerprint(game),
          before,
          `the cached solve disturbed ${i}/${p}/${f} vs ${j}/${q}/${h}`,
        );
        const slow = quiet(() => policyValue(game, us, uncached));
        assertEquals(
          fast,
          slow,
          `cached value differs at ${i}/${p}/${f} vs ${j}/${q}/${h} for P${us + 1}`,
        );
      }
      tally.units++;
    }
  }
}

// Every unit of a round-one root is the thorough case, and it is most of this file's time
// (about 45 s for all five cases), so it runs with UR_SLOW_PARITY=1 like the other slow
// gates. The round-two and round-three roots, which already carry every latched
// permanent, always run.
const ROOT_ROUNDS = Deno.env.get("UR_SLOW_PARITY") === "1" ? [0, 1, 2] : [1, 2];

for (const c of CASES) {
  Deno.test(`the continuation cache changes no value — ${c.name}`, () => {
    // One cache for the whole case, as one Search keeps one across its units: positions
    // from all three roots and both askers share it, so the key has to tell them apart.
    const stack = newPolicyStack();
    for (const rounds of ROOT_ROUNDS) {
      const root = build(c, rounds);
      assertEquals(root.round, rounds + 1);
      assertEquals(root.winner, Winner.PLAYING);
      const tally = { units: 0, latched: 0 };
      compareRoot(root, stack, tally);
      assert(tally.units > 0, `round ${rounds + 1} compared nothing`);
      if (c.latchedThroughout && rounds > 0) {
        assertEquals(
          tally.latched,
          tally.units,
          "a permanent latched before this root should be in every unit",
        );
      }
    }
    assert(stack.cache!.hits > 0, "the cache was never used");
  });
}

Deno.test("the continuation cache forgets a match it is not shown", () => {
  const stack = newPolicyStack();
  const cache = stack.cache!;
  const first = build(CASES[0], 1);
  const value = quiet(() => policyValue(first, Turn.PLAYER_1, stack));
  const size = cache.size;
  assertNotEquals(size, 0);

  // A clone is a position in the same match, so its root is already known.
  let hits = cache.hits;
  quiet(() => policyValue(first.clone(), Turn.PLAYER_1, stack));
  assertEquals(cache.hits, hits + 1);
  assertEquals(cache.size, size);

  // The same hands built again are another match: every key is the same string, so only
  // forgetting keeps a stale value from answering. It must count exactly what a fresh
  // cache counts.
  const fresh = newPolicyStack();
  quiet(() => policyValue(build(CASES[0], 1), Turn.PLAYER_1, fresh));
  hits = cache.hits;
  const misses = cache.misses;
  assertEquals(
    quiet(() => policyValue(build(CASES[0], 1), Turn.PLAYER_1, stack)),
    value,
  );
  assertEquals(cache.hits - hits, fresh.cache!.hits);
  assertEquals(cache.misses - misses, fresh.cache!.misses);
  assertEquals(cache.size, fresh.cache!.size);
});
