// Two matches alive in one process must not answer for each other.
//
// The engine used to keep its compiled CardBattles and its turn-order table in module
// globals, filled by whichever Game was built last, so building a second Game silently
// replaced the first one's cards and Counter-attack order mid-match (AGENTS.md, "Still
// open"). Both tables are now per match and shared by reference with every clone, so each
// Game here is run alone first, then both are built up front and interleaved move by move
// and search step by step, and the interleaved runs must reproduce the solo ones exactly.
import "colors";
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game, { Undo } from "@/game/Game.ts";
import { Turn } from "@/game/types/Types.ts";
import Search from "@/solver/Search.ts";
import policyValue from "@/solver/Policy.ts";
import { assert, assertEquals, assertNotEquals } from "@std/assert";

const quiet = <T>(f: () => T): T => {
  const log = console.log, info = console.info, error = console.error;
  console.log = () => 0;
  console.info = () => 0;
  console.error = () => 0;
  try {
    return f();
  } finally {
    console.log = log;
    console.info = info;
    console.error = error;
  }
};

/** Match A: the Search bench hands, alternating first mover. */
const buildA = () =>
  quiet(() =>
    new Game(
      new Player(12, 12, 0),
      new Player(12, 12, 1),
      HandGenerator.handOf(["Genmaicha", "Orka", "Sando", "Deborah"]),
      HandGenerator.handOf(["Nathan", "El Kuzco", "Noon Steevens", "Strygia"]),
      Turn.PLAYER_1,
      false,
    )
  );

/**
 * Match B: different cards in every slot, and a Counter-attack Leader (Ashigaru), so its
 * turn order differs from A's from round three on as well as its battles.
 */
const buildB = () =>
  quiet(() =>
    new Game(
      new Player(12, 12, 0),
      new Player(12, 12, 1),
      HandGenerator.handOf(["Ashigaru", "Scott Ld", "Shirley", "Joana"]),
      HandGenerator.handOf(["Callie", "Lothar", "Spidee", "Sue"]),
      Turn.PLAYER_1,
      false,
    )
  );

/** Everything a move can change, plus the turn order, as a comparable string. */
function fingerprint(g: Game): string {
  const card = (
    c: {
      name: string;
      played: boolean;
      won?: boolean;
      power: { final: number };
      damage: { final: number };
      attack: { final: number };
    },
  ) =>
    `${c.name}:${c.played ? 1 : 0}${c.won === undefined ? "-" : c.won ? "W" : "L"}` +
    `${c.power.final}/${c.damage.final}/${c.attack.final}`;
  return [
    g.id,
    g.winner,
    g.turn,
    g.round,
    g.p1.snapshot(),
    g.p2.snapshot(),
    g.r1.snapshot(),
    g.r2.snapshot(),
    [0, 1, 2, 3].map((i) => card(g.h1[i])).join(","),
    [0, 1, 2, 3].map((i) => card(g.h2[i])).join(","),
  ].join("|");
}

/** Four rounds, each side's cards in a fixed order; the mover is whoever `turn` says. */
const PLAN: Record<Turn, [number, number, boolean][]> = {
  [Turn.PLAYER_1]: [[0, 2, false], [1, 1, false], [2, 3, false], [3, 0, false]],
  [Turn.PLAYER_2]: [[2, 1, false], [0, 3, false], [3, 0, false], [1, 2, false]],
};

function step(g: Game, used: Record<Turn, number>) {
  const turn = g.turn;
  const [index, pillz, fury] = PLAN[turn][used[turn]++];
  quiet(() => g.select(index, pillz, fury, false));
  return fingerprint(g);
}

function playAlone(build: () => Game) {
  const g = build();
  const used = { [Turn.PLAYER_1]: 0, [Turn.PLAYER_2]: 0 };
  const trace = [fingerprint(g)];
  while (g.isPlaying && g.round <= 4) trace.push(step(g, used));
  return trace;
}

Deno.test("two live Games interleaved move by move each play exactly as they do alone", () => {
  const soloA = playAlone(buildA);
  const soloB = playAlone(buildB);
  assertNotEquals(soloA, soloB);

  const a = buildA();
  const b = buildB();
  const usedA = { [Turn.PLAYER_1]: 0, [Turn.PLAYER_2]: 0 };
  const usedB = { [Turn.PLAYER_1]: 0, [Turn.PLAYER_2]: 0 };
  const traceA = [fingerprint(a)];
  const traceB = [fingerprint(b)];
  while (
    (a.isPlaying && a.round <= 4) || (b.isPlaying && b.round <= 4)
  ) {
    if (a.isPlaying && a.round <= 4) traceA.push(step(a, usedA));
    if (b.isPlaying && b.round <= 4) traceB.push(step(b, usedB));
  }
  assertEquals(traceA, soloA);
  assertEquals(traceB, soloB);
});

/** Round three, first mover to play: small enough to search completely in a test. */
function roundThree(build: () => Game) {
  const g = build();
  const used = { [Turn.PLAYER_1]: 0, [Turn.PLAYER_2]: 0 };
  for (let k = 0; k < 4; k++) step(g, used);
  return g;
}

const ranked = (s: Search) =>
  s.candidates.map((c) => `${c.index}/${c.pillz}/${c.fury}:${c.values.join(",")}`);

Deno.test("two live Searches stepped alternately each rank exactly as they do alone", () => {
  const solo = (build: () => Game) =>
    quiet(() => {
      const s = new Search(roundThree(build));
      while (s.step()) { /* complete */ }
      return ranked(s);
    });
  const soloA = solo(buildA);
  const soloB = solo(buildB);
  assertNotEquals(soloA, soloB);

  const [sa, sb] = quiet(() => {
    const ga = roundThree(buildA);
    const gb = roundThree(buildB);
    return [new Search(ga), new Search(gb)];
  });
  quiet(() => {
    let more = true;
    while (more) {
      const ma = sa.step();
      const mb = sb.step();
      more = ma || mb;
    }
  });
  assertEquals(ranked(sa), soloA);
  assertEquals(ranked(sb), soloB);
});

Deno.test("clones and make/unmake share their match's tables, and a new Game does not disturb them", () => {
  const a = roundThree(buildA);
  const clone = a.clone();
  const before = fingerprint(a);
  const valueAlone = quiet(() => policyValue(a.clone(), Turn.PLAYER_1));
  // Building another match must leave an existing one and its clones answering as before.
  buildB();
  assertEquals(quiet(() => policyValue(clone, Turn.PLAYER_1)), valueAlone);
  const u = new Undo();
  const [index] = a.unplayedCardIndexes;
  assert(quiet(() => a.make(index, 1, false, u)));
  a.unmake(u);
  assertEquals(fingerprint(a), before);
});
