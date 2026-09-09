import { HandGenerator } from "@/game/Hand.ts";
// import tests from "./testcases.json" with { type: "json" };
import tests from "./testcases10000.json" with { type: "json" };
import Game from "@/game/Game.ts";
import Player from "@/game/Player.ts";
import { Turn } from "@/game/types/Types.ts";
import { HandOf } from "@/game/types/CardTypes.ts";
import { assertEquals } from "@std/assert";

function runTestcase(test: typeof tests[number]) {
  const h1 = HandGenerator.generate(
    ...test.cards.slice(0, 4) as HandOf<string>,
  );
  const h2 = HandGenerator.generate(
    ...test.cards.slice(4, 8) as HandOf<string>,
  );

  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);

  const g = new Game(p1, p2, h1, h2, Turn.PLAYER_1, false);

  for (const move of test.moves) {
    g.select(
      move.s1[0] as number,
      move.s1[1] as number,
      move.s1[2] as boolean,
      false,
    );
    g.select(
      move.s2[0] as number,
      move.s2[1] as number,
      move.s2[2] as boolean,
      false,
    );

    assertEquals(g.p1.life, move.p1life, `p1life should be ${move.p1life}`);
    assertEquals(g.p2.life, move.p2life, `p2life should be ${move.p2life}`);
    assertEquals(
      g.p1.pillz,
      move.p1pillz,
      `p1pillz should be ${move.p1pillz}`,
    );
    assertEquals(
      g.p2.pillz,
      move.p2pillz,
      `p2pillz should be ${move.p2pillz}`,
    );
  }
}

if (Deno.args[0] !== undefined) {
  const i = +Deno.args[0];
  Deno.test(`Testcase ${i}`, () => runTestcase(tests[i]));
} else {
  Deno.test("All Testcases", () => {
    console.log = () => 0;
    let failed = 0;
    for (let i = 0; i < tests.length; i++) {
      try {
        runTestcase(tests[i]);
      } catch (_) {
        console.error(`Testcase ${i} failed`);
        failed += 1;
        continue;
      }
    }
    console.info(`\n\n${failed} / ${tests.length} Failed`);
  });
}
