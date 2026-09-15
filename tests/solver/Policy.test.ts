import "colors";
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import EventTime from "@/game/types/EventTime.ts";
import { Turn } from "@/game/types/Types.ts";
import deepValue from "@/solver/Deep.ts";
import policyValue from "@/solver/Policy.ts";
import { GameResult } from "@/solver/Minimax.ts";
import Search, { shownPercent } from "@/solver/Search.ts";
import { assertEquals } from "@std/assert";

function capturedGame() {
  return new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(
      ["Anita", "Joana", "Scott Ld", "Shirley"],
      [3, 3, 4, 4],
    ),
    HandGenerator.handOf(
      ["Callie", "Lothar", "Spidee", "Sue"],
      [3, 2, 4, 2],
    ),
    Turn.PLAYER_1,
    false,
    true,
  );
}

/** The start of round two in captured battle 1065812. */
function reportedPosition() {
  const game = capturedGame();
  game.select(0, 2, false, false);
  game.select(2, 5, false, false);
  return game;
}

/** The start of round three in captured battle 1065812. */
function hiddenPillzPosition() {
  const game = reportedPosition();
  game.select(3, 1, false, false);
  game.select(1, 1, false, false);
  return game;
}

/** Round four of captured battle 1090338, after P2 reveals Burdock. */
function stoppedRescuePosition() {
  const game = new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(
      ["Spidee", "Sue", "Tina", "Wesley"],
      [4, 2, 3, 3],
    ),
    HandGenerator.handOf(
      ["Amadaus", "Avani", "Burdock", "Kalija"],
      [3, 1, 3, 4],
    ),
    Turn.PLAYER_1,
    false,
    true,
  );
  game.select(1, 5, false, false); // Sue
  game.select(0, 4, false, false); // Amadaus
  game.select(3, 3, false, false); // Kalija
  game.select(3, 2, false, false); // Wesley
  game.select(2, 0, false, false); // Tina
  game.select(1, 0, false, false); // Avani
  game.select(2, 0, false, false); // Burdock revealed; its pillz remain hidden
  return game;
}

/** Round two of captured battle 1130654, after P2 reveals Lobo. */
function reanimatePosition() {
  const game = new Game(
    new Player(14, 12, 0),
    new Player(14, 12, 1),
    HandGenerator.handOf(
      ["Lumia Cr", "Miyo", "Mou", "Nebula"],
      [4, 3, 3, 3],
    ),
    HandGenerator.handOf(
      ["Bernardite", "Dashiell", "Donald", "Lobo"],
      [3, 4, 3, 3],
    ),
    Turn.PLAYER_1,
    false,
    false,
  );
  game.select(2, 7, false, false); // Mou
  game.select(1, 6, false, false); // Dashiell
  game.select(3, 0, false, false); // Lobo revealed; its pillz remain hidden
  return game;
}

/** Round four of captured battle 1131463, after P2 reveals the live EFC semi-evo. */
function efcSemiEvoPosition() {
  const game = new Game(
    new Player(14, 12, 0),
    new Player(14, 12, 1),
    HandGenerator.handOf(
      ["Morgane", "Segar", "Smokey Cr", "Taljion"],
      [3, 3, 3, 3],
    ),
    HandGenerator.handOf(
      ["Quetzal Cr", "Kusm", "Fomalhaut Ld", "Toris"],
      [3, 2, 3, 3],
    ),
    Turn.PLAYER_1,
    false,
    false,
  );
  game.select(2, 2, false, false); // Smokey Cr
  game.select(1, 3, false, false); // Kusm
  game.select(3, 7, false, false); // Toris
  game.select(0, 6, false, false); // Morgane
  game.select(3, 1, true, false); // Taljion
  game.select(2, 0, false, false); // Fomalhaut Ld
  game.select(0, 0, false, false); // Quetzal Cr revealed; pillz hidden
  return game;
}

/** Round three of captured battle 1145959 after Pere Barali beat Segar. */
function adjacentPermanentPosition() {
  const game = new Game(
    new Player(14, 12, 0),
    new Player(14, 12, 1),
    HandGenerator.handOf(
      ["Goldie", "Scubb", "Segar", "Smokey Cr"],
      [3, 4, 3, 3],
    ),
    HandGenerator.handOf(
      ["Ashara", "Gibus", "Pere Barali", "Wez Cr"],
      [3, 3, 2, 2],
    ),
    Turn.PLAYER_1,
    false,
    true,
  );
  game.select(1, 9, false, false); // Scubb
  game.select(1, 0, false, false); // Gibus
  game.select(2, 12, false, false); // Pere Barali
  game.select(2, 7, false, false); // Segar
  return game;
}

Deno.test("battle 1065812 no longer reports an impossible 100%", () => {
  const search = new Search(reportedPosition());
  while (search.step()) { /* finish every opponent reply */ }

  const sueOne = search.candidates.find((candidate) =>
    candidate.index === 3 && candidate.pillz === 1 && !candidate.fury
  );
  if (!sueOne) throw new Error("Sue with one pill was not searched");

  assertEquals(search.shownPercent(sueOne.average), 64);
  assertEquals(search.shownPercent(sueOne.minimax), 0);
});

Deno.test("battle 1090338 accounts for Burdock stopping the Rescue bonus", () => {
  const search = new Search(stoppedRescuePosition());
  while (search.step()) { /* finish every hidden Burdock bet */ }

  const spideeFive = search.candidates.find((candidate) =>
    candidate.index === 0 && candidate.pillz === 5 && !candidate.fury
  );
  if (!spideeFive) throw new Error("Spidee with five pillz was not searched");

  assertEquals(search.shownPercent(spideeFive.average), 89);
  assertEquals(search.shownPercent(spideeFive.minimax), 0);
});

Deno.test("battle 1130654 accounts for Lobo's Reanimate", () => {
  const search = new Search(reanimatePosition());
  while (search.step()) { /* finish every hidden Lobo bet */ }

  const miyoOne = search.candidates.find((candidate) =>
    candidate.index === 1 && candidate.pillz === 1 && !candidate.fury
  );
  if (!miyoOne) throw new Error("Miyo with one pill was not searched");

  assertEquals(search.shownPercent(miyoOne.average), 86);
});

Deno.test("battle 1131463 uses Quetzal Cr's live EFC semi-evo", () => {
  const search = new Search(efcSemiEvoPosition());
  while (search.step()) { /* finish every hidden Quetzal Cr bet */ }

  const segarAllIn = search.candidates.find((candidate) =>
    candidate.index === 1 && candidate.pillz === 3 && !candidate.fury
  );
  if (!segarAllIn) throw new Error("Segar with three pillz was not searched");

  assertEquals(search.shownPercent(segarAllIn.average) < 100, true);
  assertEquals(
    (search.outcome(segarAllIn, { pillz: 2, fury: false })?.value ?? 0) < 0,
    true,
  );
});

Deno.test("battle 1145959 processes a permanent after a failed adjacent latch", () => {
  const game = adjacentPermanentPosition();
  const heals = game.events2.repeat[EventTime.END];

  // Pere's stopped Copy: Opp. Ability adds a Dope before its own Heal. Removing that Dope
  // must not make iteration skip the Heal shifted into its array slot.
  assertEquals(heals.map((a) => a.ability), ["1 Heal Max 18"]);
  assertEquals(heals[0].won, true);
  assertEquals(heals[0].delayed, false);

  const before = JSON.stringify(game);
  policyValue(game, Turn.PLAYER_1);
  assertEquals(
    JSON.stringify(game),
    before,
    "policy search must fully unmake its moves",
  );
});

Deno.test("a continuation cannot choose its reply after seeing hidden pillz", () => {
  const game = hiddenPillzPosition();
  const before = JSON.stringify(game);

  // Perfect-information minimax says P2 can force a win by choosing a different response
  // for each P1 bet. For at least one revealed card, no single response covers every
  // hidden bet, so the executable pure policy correctly refuses to call that a forced win.
  assertEquals(deepValue(game), GameResult.PLAYER_2_WIN);
  assertEquals(policyValue(game, Turn.PLAYER_2), GameResult.PLAYER_1_WIN);
  assertEquals(
    JSON.stringify(game),
    before,
    "policy search must fully unmake its moves",
  );
});

Deno.test("rounding never turns a near certainty into an exact one", () => {
  assertEquals(shownPercent(99.6), 99);
  assertEquals(shownPercent(100), 100);
  assertEquals(shownPercent(0.4), 1);
  assertEquals(shownPercent(0), 0);
});
