import "colors";
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
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
