import Hand, { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game, { Selection } from "@/game/Game.ts";
import { Turn } from "@/game/types/Types.ts";
import { CardGenerator } from "@/game/Card.ts";
import { Clan } from "@/game/types/CardTypes.ts";

interface Testcase {
  cards: string[];
  flip: boolean;
  life: number;
  pillz: number;
  moves: {
    s1: Selection;
    s2: Selection;
    p1life: number;
    p2life: number;
    p1pillz: number;
    p2pillz: number;
  }[];
}

function generateSelection(g: Game): Selection {
  const playerPillz = g.playingPlayer.pillz;
  const pillz = playerPillz * Math.random() | 0;
  const indexes = g.unplayedCardIndexes;
  const index = indexes[Math.floor(Math.random() * indexes.length)];
  const fury = pillz <= playerPillz - 3 && Math.random() < 0.25;
  return [index, pillz, fury];
}

Deno.test("GenerateTestCasesForRust", () => {
  // const h1 = HandGenerator.generate("Hugo");
  // const h1 = HandGenerator.generate("Eyrik");
  const h1 = HandGenerator.generate("Dark Scar");
  // const h1 = HandGenerator.generate();
  const h2 = HandGenerator.generate();
  const p1 = new Player(12, 12, 0);
  const p2 = new Player(12, 12, 1);

  const g = new Game(p1, p2, h1, h2, Turn.PLAYER_1);

  const data: Testcase = {
    cards: [...h1.map((c) => c.name), ...h2.map((c) => c.name)],
    flip: false,
    life: g.p1.life,
    pillz: g.p1.pillz,
    moves: [],
  };

  while (!g.hasWinner()) {
    const s1 = generateSelection(g);
    g.select(...s1);
    const s2 = generateSelection(g);
    g.select(...s2);

    data.moves.push({
      s1,
      s2,
      p1life: g.p1.life,
      p2life: g.p2.life,
      p1pillz: g.p1.pillz,
      p2pillz: g.p2.pillz,
    });
  }

  console.log(JSON.stringify(data));
});

Deno.test("Generate10,000TestCasesForRust", () => {
  console.log = () => 0;

  const testcases: Testcase[] = [];
  for (let i = 0; i < 10000; i++) {
    let h1 = HandGenerator.generate();
    let h2 = HandGenerator.generate();

    while (
      (<Clan[]> ["Cosmohnuts", "Zenith", "Oculus", "Leader"]).includes(
        h1[3].baseClan,
      )
    ) {
      h1 = HandGenerator.generate();
    }
    while (
      (<Clan[]> ["Cosmohnuts", "Zenith", "Oculus", "Leader"]).includes(
        h2[3].baseClan,
      )
    ) {
      h2 = HandGenerator.generate();
    }

    // Include a leader
    if (Math.random() < 0.05) {
      h1[0] = CardGenerator.getRandomCard("Leader");
      h1[0].index = 0;
    }
    if (Math.random() < 0.05) {
      h2[0] = CardGenerator.getRandomCard("Leader");
      h2[0].index = 0;
    }
    // Include an Oculus
    if (Math.random() < 0.05) {
      h1[1] = CardGenerator.getRandomCard("Oculus");
      h1[1].index = 1;
    }
    if (Math.random() < 0.05) {
      h2[1] = CardGenerator.getRandomCard("Oculus");
      h2[1].index = 1;
    }

    h1 = Hand.from(h1);
    h2 = Hand.from(h2);

    const p1 = new Player(12, 12, 0);
    const p2 = new Player(12, 12, 1);

    const g = new Game(p1, p2, h1, h2, Turn.PLAYER_1, false);

    const data: Testcase = {
      cards: [...h1.map((c) => c.name), ...h2.map((c) => c.name)],
      flip: false,
      life: g.p1.life,
      pillz: g.p1.pillz,
      moves: [],
    };

    while (!g.hasWinner()) {
      const s1 = generateSelection(g);
      g.select(...s1, false);
      const s2 = generateSelection(g);
      g.select(...s2, false);

      data.moves.push({
        s1,
        s2,
        p1life: g.p1.life,
        p2life: g.p2.life,
        p1pillz: g.p1.pillz,
        p2pillz: g.p2.pillz,
      });
    }

    testcases.push(data);
  }

  const url = new URL("./testcases10000.json", import.meta.url);
  Deno.writeTextFileSync(url, JSON.stringify(testcases));
});
