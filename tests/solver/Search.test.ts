// deno-lint-ignore-file no-control-regex
// Two engine faults that only the solver's branching exposed, plus the reporting scale the
// solver prints its answers on. All three predate this file; see the comments in
// Game.battle(), Ability.clone() and Node.toString().
import "colors";
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game, { Winner } from "@/game/Game.ts";
import { assertEquals, assertNotEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import { type HandOf } from "@/game/types/CardTypes.ts";
import Analysis from "@/solver/Analysis.ts";
import { GameResult, Node } from "@/solver/Minimax.ts";
import Search, { SearchMode } from "@/solver/Search.ts";

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

// Uchtul Cr lv4: 8/7, "Backlash: - 5 Life Min 0" — winning the round costs its owner 5
// life, so on 2 life apiece the round can put both players on 0 at once.
const doubleKo = () =>
  new Game(
    new Player(2, 12, 0),
    new Player(2, 12, 1),
    HandGenerator.handOf(
      ["Uchtul Cr", "Natrang", "Natrang", "Natrang"] as HandOf<string>,
      [4, 1, 1, 1] as HandOf<number | undefined>,
    ),
    HandGenerator.handOf(
      ["Natrang", "Natrang", "Natrang", "Natrang"] as HandOf<string>,
      [1, 1, 1, 1] as HandOf<number | undefined>,
    ),
    Turn.PLAYER_1,
    false,
  );

Deno.test("a double KO is a tie, not an unfinished game", () => {
  const g = quiet(() => {
    const g = doubleKo();
    g.select(0, 5, false, false); // p1 Uchtul, 5 pillz — wins, then Backlash takes it to 0
    g.select(0, 0, false, false); // p2 Natrang, no pillz — takes 7 and is also on 0
    return g;
  });

  assertEquals(g.p1.life, 0);
  assertEquals(g.p2.life, 0);
  assertEquals(g.hasWinner(), true);
  // Left on PLAYING, the solver read the position as live and kept expanding it.
  assertEquals(g.winner, Winner.TIE);
});

Deno.test("every node the solver builds has a finite rating", () => {
  const tree = quiet(() => {
    const g = new Game(
      new Player(2, 5, 0),
      new Player(2, 5, 1),
      HandGenerator.handOf(
        ["Uchtul Cr", "Natrang", "Natrang", "Natrang"] as HandOf<string>,
        [4, 1, 1, 1] as HandOf<number | undefined>,
      ),
      HandGenerator.handOf(
        ["Natrang", "Natrang", "Natrang", "Natrang"] as HandOf<string>,
        [1, 1, 1, 1] as HandOf<number | undefined>,
      ),
      Turn.PLAYER_1,
      false,
    );
    return Analysis.iterTree(g);
  });

  let nodes = 0, nonFinite = 0;
  const walk = (n: Node) => {
    nodes++;
    if (!Number.isFinite(n.rating())) nonFinite++;
    for (const c of n.nodes) walk(c);
  };
  quiet(() => walk(tree));

  assertNotEquals(nodes, 0);
  // Node.rating() returns Infinity for a node with neither a result nor children, and a
  // MAX ancestor propagates it, so one of these poisons whole branches of the search.
  assertEquals(nonFinite, 0);
});

Deno.test("a permanent does not latch across sibling branches", () => {
  // Galactea lv4: 8/4, ability "Toxin 1, Min 0" — poisons only if it wins its round.
  const root = quiet(() =>
    new Game(
      new Player(20, 12, 0),
      new Player(20, 12, 1),
      HandGenerator.handOf(
        ["Galactea", "Natrang", "Natrang", "Natrang"] as HandOf<string>,
        [4, 1, 1, 1] as HandOf<number | undefined>,
      ),
      HandGenerator.handOf(
        ["Sando", "Natrang", "Natrang", "Natrang"] as HandOf<string>,
        [3, 1, 1, 1] as HandOf<number | undefined>,
      ),
      Turn.PLAYER_1,
      false,
    )
  );

  // Expand the way Analysis.iterTree does: clone the parent, apply one select, and clone
  // again for the reply. Anything the engine writes to shared state shows up here.
  const branch = (p1pillz: number, p2pillz: number) =>
    quiet(() => {
      const ply1 = root.clone();
      ply1.select(0, p1pillz, false, false);
      const ply2 = ply1.clone();
      ply2.select(0, p2pillz, false, false);
      return ply2;
    });

  assertEquals(branch(0, 8).p1.won, false); // Galactea loses: no poison
  assertEquals(branch(0, 8).p2.life, 20);

  assertEquals(branch(8, 0).p1.won, true); // Galactea wins: Toxin latches here
  assertEquals(branch(8, 0).p2.life, 15); // 4 damage + 1 poison

  // The losing branch is the same state as before and must still be poison-free: the latch
  // used to be written onto an Ability instance shared by every branch of the tree.
  assertEquals(branch(0, 8).p2.life, 20);
  assertEquals(branch(0, 8).p2.life, 20);
});

Deno.test("a SECOND search does not deselect the source game's visible card", () => {
  const game = new Game(
    new Player(12, 3, 0),
    new Player(12, 3, 1),
    HandGenerator.handOf(["Natrang", "Natrang", "Natrang", "Natrang"]),
    HandGenerator.handOf(["Natrang", "Natrang", "Natrang", "Natrang"]),
    Turn.PLAYER_1,
    false,
  );
  game.select(2, 0, false, false);

  const search = new Search(game);

  assertEquals(search.mode, SearchMode.SECOND);
  assertEquals(search.oppIndex, 2);
  assertEquals(game.firstHasSelected, true);
  assertEquals(game.playedCardIndex, 2);
  assertEquals(game.h1[2].played, true);
});

Deno.test("Node.toString reports on the mover's side, in [-1, 1]", () => {
  const strip = (s: string) => s.replace(/\x1b\[[0-9;]*m/g, "");
  // A node's turn is who moves next, so its own move was made by the other player.
  const label = (turn: Turn, result: GameResult) => {
    const n = new Node("2 4 false", turn);
    n.add(
      new Node(
        "reply",
        turn === Turn.PLAYER_1 ? Turn.PLAYER_2 : Turn.PLAYER_1,
        result,
      ),
    );
    return strip(n.toString());
  };

  // Mover is P1, and +1 is a P1 win.
  assertEquals(
    label(Turn.PLAYER_2, GameResult.PLAYER_1_WIN),
    "[Win] 2 4 false",
  );
  assertEquals(label(Turn.PLAYER_2, GameResult.TIE), "[Draw] 2 4 false");
  assertEquals(
    label(Turn.PLAYER_2, GameResult.PLAYER_2_WIN),
    "[Loss] 2 4 false",
  );

  // Mover is P2, so the signs flip.
  assertEquals(
    label(Turn.PLAYER_1, GameResult.PLAYER_2_WIN),
    "[Win] 2 4 false",
  );
  assertEquals(label(Turn.PLAYER_1, GameResult.TIE), "[Draw] 2 4 false");
  assertEquals(
    label(Turn.PLAYER_1, GameResult.PLAYER_1_WIN),
    "[Loss] 2 4 false",
  );
});

// The hidden-wager order decides what a partial SECOND-mode average is standing on. The
// read panel's three defaults come first; everything after them is sampled middle-out so
// the early rows already span low, middle and high bets of both kinds.
const answeringTwelve = () => {
  const g = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(
      ["Natrang", "Natrang", "Natrang", "Natrang"] as HandOf<string>,
      [1, 1, 1, 1] as HandOf<number | undefined>,
    ),
    HandGenerator.handOf(
      ["Natrang", "Natrang", "Natrang", "Natrang"] as HandOf<string>,
      [1, 1, 1, 1] as HandOf<number | undefined>,
    ),
    Turn.PLAYER_1,
    false,
  );
  g.select(0, 4, false, false);
  return quiet(() => new Search(g));
};

Deno.test("the read panel's three hypotheses are still settled first", () => {
  const s = answeringTwelve();

  assertEquals(s.mode, SearchMode.SECOND);
  assertEquals(
    s.opponentMoves.slice(0, 3).map((m) => `${m.pillz} ${m.fury}`),
    ["12 false", "0 false", "9 true"],
  );
});

Deno.test("the hidden wagers after them are sampled middle-out", () => {
  const s = answeringTwelve();
  const after = s.opponentMoves.slice(3);

  // Plain and Fury alternate, and each family starts at its own midpoint.
  assertEquals(
    after.slice(0, 4).map((m) => `${m.pillz} ${m.fury}`),
    ["6 false", "4 true", "3 false", "1 true"],
  );
  // Nothing is lost or duplicated by the reordering.
  assertEquals(
    new Set(s.opponentMoves.map((m) => `${m.pillz} ${m.fury}`)).size,
    s.opponentMoves.length,
  );
  assertEquals(s.samples, s.opponentMoves.length);
});

Deno.test("an early average already spans the range of bets", () => {
  const s = answeringTwelve();
  const early = s.opponentMoves.slice(0, 8).filter((m) => !m.fury);
  const bets = early.map((m) => m.pillz).sort((a, b) => a - b);

  assertEquals(bets[0], 0);
  assertEquals(bets[bets.length - 1], 12);
  // At least one wager from the middle third, which carries most of the opening prior.
  assertEquals(bets.some((p) => p >= 4 && p <= 8), true);
});
