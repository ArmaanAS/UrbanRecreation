// deno-lint-ignore-file no-control-regex
// The view redraws in place with cursor-home, so a single line wider than the terminal, or
// one line too many, shifts everything after it and the frame overprints itself - which is
// what a real game on an 80-column terminal looked like: duplicated letters, columns out of
// line, tables on top of each other. These are the invariants that stop that happening.
import "colors";
import { CardGenerator } from "@/game/Card.ts";
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import Game from "@/game/Game.ts";
import { assertEquals } from "@std/assert";
import { Turn } from "@/game/types/Types.ts";
import Search from "@/solver/Search.ts";
import GameRenderer, { compactClanTags } from "@/utils/GameRenderer.ts";
import {
  autoQueueClick,
  displayRanked,
  displaySafeRanked,
  idle,
  render,
  resultBanner,
  resultStylePreview,
  usableConsoleSize,
} from "@/solver/SolverView.ts";

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

/** Every CSI sequence, not just colours: the erase-to-EOL that ends each line counts too. */
const strip = (s: string) => s.replace(/\x1b\[[0-9;?]*[a-zA-Z]/g, "");

/** Round 2 with 12 pillz: enough candidates to exercise every part of the full view. */
function widest() {
  return quiet(() => {
    const g = new Game(
      new Player(12, 12, 0),
      new Player(12, 12, 1),
      HandGenerator.handOf(["Genmaicha", "Orka", "Sando", "Deborah"]),
      HandGenerator.handOf(["Nathan", "El Kuzco", "Noon Steevens", "Strygia"]),
      // P2 opens round one, so P1 is still the asking side after we advance to round two.
      // Several ranking fixtures deliberately write values in P1's frame.
      Turn.PLAYER_2,
      false,
    );
    // Round one intentionally exposes only the four zero-pill candidates. Advance once so
    // view tests that need a full pillz matrix keep exercising that richer layout.
    g.select(0, 0, false, false);
    g.select(0, 0, false, false);
    const s = new Search(g);
    for (let i = 0; i < 4; i++) s.step();
    return { g, s };
  });
}

const SIZES: [number, number][] = [[60, 20], [80, 24], [100, 30], [120, 40], [
  40,
  12,
]];

Deno.test("a frame never exceeds the terminal it is drawn on", () => {
  const { g, s } = widest();
  for (const [columns, rows] of SIZES) {
    const out = quiet(() =>
      render(g, s, {
        size: { columns, rows },
        status: "connected",
        played: {
          index: 2,
          pillz: 4,
          fury: false,
          percent: 61,
          best: 73,
          better: 3,
          scored: 40,
        },
        history: [
          {
            round: 1,
            card: "Orka",
            move: { index: 1, pillz: 2, fury: false, percent: 80, better: 0 },
          },
          {
            round: 2,
            card: "Sando",
            move: {
              index: 2,
              pillz: 0,
              fury: false,
              unevaluated: "not evaluated yet",
            },
          },
        ],
      })
    );
    const drawn = out.split("\n");
    // No trailing newline: writing one on the last available row scrolls the screen, and
    // the next cursor-home redraw then lands a line off and the whole frame walks.
    assertEquals(
      out.endsWith("\n"),
      false,
      `${columns}x${rows}: frame ends in a newline`,
    );
    // Every line must erase its own tail. Without that, a line that got shorter leaves the
    // previous frame's text behind - stale matrix columns, duplicated legends, "0ms=".
    for (const [i, line] of drawn.entries()) {
      assertEquals(
        line.endsWith("\x1b[K"),
        true,
        `${columns}x${rows}: line ${i} does not clear to end of line`,
      );
    }
    assertEquals(
      drawn.length <= rows,
      true,
      `${columns}x${rows}: ${drawn.length} lines drawn, only ${rows} rows available`,
    );
    for (const [i, line] of drawn.entries()) {
      assertEquals(
        strip(line).length <= columns,
        true,
        `${columns}x${rows}: line ${i} is ${
          strip(line).length
        } wide — it would wrap`,
      );
    }
  }
});

Deno.test("the idle screen fits too", () => {
  for (const [columns, rows] of SIZES) {
    const out = idle(
      "starting",
      "waiting for a turn of yours",
      {
        columns,
        rows,
      },
    );
    const drawn = out.split("\n");
    assertEquals(
      drawn.length <= rows,
      true,
      `${columns}x${rows}: too many lines`,
    );
    for (const line of drawn) {
      assertEquals(
        strip(line).length <= columns,
        true,
        `${columns}x${rows}: too wide`,
      );
      assertEquals(
        line.endsWith("\x1b[K"),
        true,
        `${columns}x${rows}: does not clear to EOL`,
      );
    }
    const plain = drawn.map(strip);
    const titleAt = plain.findIndex((line) => line.includes("UR ADVISOR"));
    assertEquals(
      plain[titleAt + 1],
      " " + "-".repeat(columns - 2),
      `${columns}x${rows}: idle title is missing its divider`,
    );
  }
});

Deno.test("an empty idle note leaves no floating body text", () => {
  const lines = strip(idle("", "", { columns: 80, rows: 24 })).split("\n");
  assertEquals(
    lines.filter((line) => line.trim().length > 0),
    [
      " UR ADVISOR │ idle",
      " " + "-".repeat(78),
    ],
  );
});

Deno.test("large terminal dimensions are preserved", () => {
  assertEquals(
    usableConsoleSize({ columns: 120, rows: 9001 }),
    { columns: 120, rows: 9001 },
  );
});

Deno.test("sections hold their rows as the search fills and the log grows", () => {
  const { g, s } = widest();
  const size = { columns: 80, rows: 24 };
  const hist = [
    {
      round: 1,
      card: "Genmaicha",
      move: { index: 0, pillz: 2, fury: false, percent: 80, better: 0 },
    },
    {
      round: 2,
      card: "Orka",
      move: {
        index: 1,
        pillz: 1,
        fury: false,
        percent: 55,
        better: 2,
        best: 70,
      },
    },
    {
      round: 3,
      card: "Sando",
      move: {
        index: 2,
        pillz: 0,
        fury: false,
        percent: 40,
        better: 5,
        best: 90,
      },
    },
  ];
  const rowOf = (out: string, needle: string) =>
    out.split("\n").findIndex((l) => strip(l).includes(needle));

  const rowsSeen: Record<string, Set<number>> = {
    "Best bets": new Set(),
    "first ranked row": new Set(),
    status: new Set(),
  };

  // The search filling in, a move being played, then the log growing round by round: the
  // view redraws in place, so anything that shifts a section makes the whole thing crawl.
  for (const [i, steps] of [0, 1, 5, 40].entries()) {
    quiet(() => {
      for (let k = 0; k < steps; k++) s.step();
    });
    const out = quiet(() =>
      render(g, s, {
        size,
        top: 5,
        status: "connected",
        played: i >= 2
          ? {
            index: 1,
            pillz: 3,
            fury: false,
            percent: 61,
            better: 1,
            best: 70,
          }
          : undefined,
        history: hist.slice(0, Math.max(0, i - 1)),
      })
    );
    rowsSeen["Best bets"].add(rowOf(out, "Best bets"));
    rowsSeen["first ranked row"].add(rowOf(out, " 1. "));
    rowsSeen["status"].add(rowOf(out, "connected"));
  }

  for (const [what, rows] of Object.entries(rowsSeen)) {
    // -1 means "absent", which the first frame legitimately is for the ranked row.
    const present = [...rows].filter((n) => n >= 0);
    assertEquals(
      present.length,
      1,
      `"${what}" appeared on rows ${
        JSON.stringify(present)
      } - it should never move`,
    );
  }
});

Deno.test("best-bets numbers line up with their headers", () => {
  const { g, s } = widest();
  for (
    const [columns, rows] of [[80, 40], [100, 40], [120, 40]] as [
      number,
      number,
    ][]
  ) {
    const drawn = quiet(() => render(g, s, { size: { columns, rows }, top: 3 }))
      .split("\n")
      .map(strip);

    const at = drawn.findIndex((l) => l.includes("Best bets"));
    assertEquals(at >= 0, true, `${columns}: no Best bets header`);
    const header = drawn[at];
    // The name field is the widest thing that is not a column heading, so measure the
    // numeric columns from where "win" starts.
    const from = header.indexOf("Win");
    const rightEdges = (line: string) =>
      [...line.matchAll(/\S+/g)].filter((m) => m.index! >= from - 2)
        .map((m) => m.index! + m[0].length);
    const headerCols = rightEdges(header);
    assertEquals(
      headerCols.length >= 3,
      true,
      `${columns}: too few columns in "${header}"`,
    );
    const worstEnd = header.indexOf("Worst") + "Worst".length;

    // Rows are the numbered entries that follow the header and its key line.
    const rowsOut = drawn.slice(at + 1).filter((l) => /^\s*\d+\.\s/.test(l));
    assertEquals(rowsOut.length > 0, true, `${columns}: no ranked rows`);
    for (const row of rowsOut) {
      const worst = row.slice(worstEnd - 8, worstEnd).trim();
      assertEquals(
        ["Win", "Draw", "Lose"].includes(worst),
        true,
        `Worst should be categorical, got ${JSON.stringify(worst)} in "${row}"`,
      );
      assertEquals(
        rightEdges(row),
        headerCols,
        `${columns} cols: ranked row does not sit under its headings\n  ${header}\n  ${row}`,
      );
    }
  }
});

Deno.test("round one is labelled as an opening estimate rather than a win chance", () => {
  const game = quiet(() =>
    new Game(
      new Player(12, 12, 0),
      new Player(12, 12, 1),
      HandGenerator.handOf(["Genmaicha", "Orka", "Sando", "Deborah"]),
      HandGenerator.handOf(["Nathan", "El Kuzco", "Noon Steevens", "Strygia"]),
      Turn.PLAYER_1,
      false,
    )
  );
  const search = new Search(game);
  const out = strip(
    quiet(() =>
      render(game, search, { size: { columns: 120, rows: 40 }, top: 3 })
    ),
  );

  assertEquals(out.includes("fast opening estimate"), true);
  assertEquals(out.includes("Opening estimates"), true);
  assertEquals(out.includes("Avg"), true);
  assertEquals(out.includes("Range"), true);
  assertEquals(out.includes("Opening score by pillz"), true);
  assertEquals(out.includes("Win % by pillz"), false);
});

Deno.test("round one reserves ten recommendation rows by default", () => {
  const game = quiet(() =>
    new Game(
      new Player(12, 12, 0),
      new Player(12, 12, 1),
      HandGenerator.handOf(["Genmaicha", "Orka", "Sando", "Deborah"]),
      HandGenerator.handOf(["Nathan", "El Kuzco", "Noon Steevens", "Strygia"]),
      Turn.PLAYER_1,
      false,
    )
  );
  const search = new Search(game);
  const gap = (top?: number) => {
    const lines = strip(
      quiet(() =>
        render(game, search, {
          size: { columns: 120, rows: 40 },
          top,
        })
      ),
    ).split("\n");
    const heading = lines.findIndex((line) =>
      line.includes("Opening estimates")
    );
    const matrix = lines.findIndex((line) =>
      line.includes("Opening score by pillz")
    );
    return matrix - heading;
  };

  assertEquals(gap(), gap(8) + 2);
});

Deno.test("three complete recommendations stay above optimistic partial rows", () => {
  const { s } = widest();
  const [a, b, c, partialBest, partialSecond] = s.candidates;
  for (
    const [candidate, average] of [
      [a, 0.6],
      [b, 0.4],
      [c, 0.2],
    ] as const
  ) {
    candidate.average = average;
    candidate.minimax = average;
    candidate.done = s.samples;
  }
  partialBest.average = 1;
  partialBest.minimax = 1;
  partialBest.done = 1;
  partialSecond.average = 0.9;
  partialSecond.minimax = 0.9;
  partialSecond.done = 1;

  assertEquals(displayRanked(s).slice(0, 3).map((move) => move.key), [
    a.key,
    b.key,
    c.key,
  ]);
  assertEquals(displayRanked(s)[3].key, partialBest.key);
});

Deno.test("safe recommendations are complete zero-risk moves ranked by win chance", () => {
  const { s } = widest();
  const [riskyBest, safeLow, safeBest, partial, exactZero] = s.candidates;
  for (const candidate of [riskyBest, safeLow, safeBest, exactZero]) {
    candidate.done = s.samples;
  }
  riskyBest.average = riskyBest.minimax = 1;
  riskyBest.koed = 1;
  safeLow.average = safeLow.minimax = 0.2;
  safeBest.average = safeBest.minimax = 0.6;
  partial.average = partial.minimax = 0.9;
  partial.done = 1;
  partial.koed = 0;
  exactZero.average = exactZero.minimax = -1;
  exactZero.koed = 0;

  assertEquals(displaySafeRanked(s).map((move) => move.key), [
    safeBest.key,
    safeLow.key,
  ]);
});

Deno.test("a zero-win safe move does not create a Safe Bets table", () => {
  const { g, s } = widest();
  const risky = s.candidates.slice(0, 8);
  const zeroWinSafe = s.candidates[8];
  for (const [index, candidate] of risky.entries()) {
    candidate.average = candidate.minimax = 1 - index * 0.05;
    candidate.done = s.samples;
    candidate.koed = 1;
  }
  zeroWinSafe.average = zeroWinSafe.minimax = -1;
  zeroWinSafe.done = s.samples;
  zeroWinSafe.koed = 0;

  const out = strip(
    quiet(() => render(g, s, { size: { columns: 134, rows: 70 }, top: 8 })),
  );
  assertEquals(out.includes("Safe bets (0% Risk)"), false);
});

Deno.test("a roomy frame shows a separate proven zero-risk shortlist", () => {
  const { g, s } = widest();
  const risky = s.candidates.slice(0, 8);
  const safe = s.candidates[8];
  for (const [index, candidate] of risky.entries()) {
    candidate.average = candidate.minimax = 1 - index * 0.05;
    candidate.done = s.samples;
    candidate.koed = 1;
  }
  safe.average = safe.minimax = 0.4;
  safe.done = s.samples;
  safe.koed = 0;
  Object.defineProperty(s, "done", { value: true });

  const rendered = quiet(() =>
    render(g, s, { size: { columns: 134, rows: 70 } })
  );
  const rawLines = rendered.split("\n");
  const lines = strip(rendered).split("\n");
  const at = lines.findIndex((line) => line.includes("Safe bets (0% Risk)"));
  assertEquals(at >= 0, true, "safe-bets table is missing");
  assertEquals(
    rawLines[at + 1].includes("\x1b[92m-\x1b[39m"),
    true,
    "safe move must show a green zero-risk dash",
  );
  assertEquals(
    lines[at + 1].includes("Done"),
    true,
    "safe move must be complete",
  );
  const bestAt = lines.findIndex((line) => line.includes("Best bets"));
  const handAt = lines.findIndex((line) =>
    line.includes("OPP") && line.includes("Life")
  );
  assertEquals(at, bestAt + 9, "main table must retain all eight rows");
  assertEquals(
    handAt < bestAt && bestAt < at,
    true,
    "hands, main recommendations and Safe Bets must appear in that order",
  );
});

Deno.test("the reserved safe block prevents hand-panel layout shift", () => {
  const { g, s } = widest();
  const handRow = () => {
    const lines = strip(
      quiet(() => render(g, s, { size: { columns: 134, rows: 70 } })),
    ).split("\n");
    return lines.findIndex((line) =>
      line.includes("OPP") && line.includes("Life")
    );
  };
  const before = handRow();
  const risky = s.candidates.slice(0, 8);
  const safe = s.candidates[8];
  for (const [index, candidate] of risky.entries()) {
    candidate.average = candidate.minimax = 1 - index * 0.05;
    candidate.done = s.samples;
    candidate.koed = 1;
  }
  safe.average = safe.minimax = 0.2;
  safe.done = s.samples;
  safe.koed = 0;
  assertEquals(handRow(), before);
});

Deno.test("safe bets wait until the full search is complete", () => {
  const { g, s } = widest();
  const [a, b, c, safe] = s.candidates;
  for (const [candidate, average] of [[a, 1], [b, 0.8], [c, 0.6]] as const) {
    candidate.average = candidate.minimax = average;
    candidate.done = s.samples;
    candidate.koed = 1;
  }
  safe.average = safe.minimax = 0.4;
  safe.done = s.samples;
  safe.koed = 0;

  const out = strip(
    quiet(() => render(g, s, { size: { columns: 134, rows: 70 }, top: 8 })),
  );
  assertEquals(out.includes("Safe bets (0% Risk)"), false);
});

Deno.test("a useful zero-risk move in the top five suppresses Safe Bets", () => {
  const { g, s } = widest();
  const main = s.candidates.slice(0, 8);
  const hiddenSafe = s.candidates[8];
  for (const [index, candidate] of main.entries()) {
    candidate.average = candidate.minimax = 1 - index * 0.05;
    candidate.done = s.samples;
    candidate.koed = index === 4 ? 0 : 1;
  }
  hiddenSafe.average = hiddenSafe.minimax = 0.4;
  hiddenSafe.done = s.samples;
  hiddenSafe.koed = 0;
  Object.defineProperty(s, "done", { value: true });

  const out = strip(
    quiet(() => render(g, s, { size: { columns: 134, rows: 70 }, top: 8 })),
  );
  assertEquals(out.includes("Safe bets (0% Risk)"), false);
});

Deno.test("an entirely safe search has no secondary table content", () => {
  const { g, s } = widest();
  for (const [index, candidate] of s.candidates.entries()) {
    candidate.average = candidate.minimax = 1 - index / s.candidates.length;
    candidate.done = s.samples;
    candidate.koed = 0;
  }
  Object.defineProperty(s, "done", { value: true });

  const rendered = quiet(() =>
    render(g, s, { size: { columns: 134, rows: 70 }, top: 8 })
  );
  const out = strip(rendered);
  assertEquals(out.includes("Safe bets (0% Risk)"), false);
  assertEquals(
    rendered.includes("\x1b[92m-\x1b[39m"),
    true,
    "the main table must colour zero-risk dashes green",
  );
});

Deno.test("auto-queue is a visible fixed last-row mouse target", () => {
  const size = { columns: 80, rows: 24 };
  const off = idle("connected", "waiting for your turn", size, false).split(
    "\n",
  );
  const on = idle("connected", "waiting for your turn", size, true).split("\n");
  const hovered = idle("connected", "waiting for your turn", size, false, true)
    .split("\n");
  const { g, s } = widest();
  const active = quiet(() =>
    render(g, s, { size, status: "connected", autoQueue: false })
  ).split("\n");

  assertEquals(off.length, size.rows);
  assertEquals(strip(off.at(-1)!).includes("[ AUTO-QUEUE OFF ]"), true);
  assertEquals(strip(on.at(-1)!).includes("[ AUTO-QUEUE ON  ]"), true);
  assertEquals(strip(active.at(-1)!).includes("[ AUTO-QUEUE OFF ]"), true);
  assertEquals(strip(off.at(-1)!).includes("click or A"), false);
  assertEquals(hovered.at(-1)!.includes("\x1b[7m"), true);
  assertEquals(autoQueueClick(2, 24, size), true);
  assertEquals(autoQueueClick(19, 24, size), true);
  assertEquals(autoQueueClick(20, 24, size), false);
  assertEquals(autoQueueClick(2, 23, size), false);
});

Deno.test("a holding frame advances to the live round without presenting stale advice as current", () => {
  const { g, s } = widest();
  const phase = {
    round: 2,
    headline: "opponent to move",
    you: { life: 8, pillz: 7 },
    them: { life: 11, pillz: 9 },
    progress: "round 1 resolved",
  };
  const active = strip(quiet(() =>
    render(g, s, {
      size: { columns: 80, rows: 24 },
      phase,
    })
  ));
  const waiting = strip(idle(
    "connected",
    "waiting for their card",
    {
      columns: 80,
      rows: 24,
    },
    false,
    false,
    phase,
  ));

  for (const out of [active, waiting]) {
    assertEquals(out.includes("ROUND 2/4"), true);
    assertEquals(out.includes("opponent to move"), true);
    assertEquals(out.includes("8 life 7 pillz"), false);
    assertEquals(out.includes("11 life 9 pillz"), false);
  }
  assertEquals(active.includes("Previous advice"), true);
  assertEquals(active.includes("Best bets"), false);
});

Deno.test("a large waiting frame keeps the advice placeholder and both complete hands", () => {
  const { g } = widest();
  const out = strip(idle(
    "connected",
    "waiting for a turn of yours",
    { columns: 134, rows: 70 },
    true,
    false,
    {
      round: 1,
      headline: "opponent to move",
      progress: "waiting",
      you: { life: 12, pillz: 12 },
      them: { life: 12, pillz: 12 },
      board: { you: Array.from(g.h1), them: Array.from(g.h2) },
    },
  ));

  assertEquals(out.includes("Best bets"), true);
  assertEquals(out.includes("waiting for the opponent's card"), true);
  for (const card of [...g.h1, ...g.h2]) {
    assertEquals(out.includes(card.name), true, `missing ${card.name}`);
  }
  const waitingLines = out.split("\n");
  const lastCardRow = waitingLines.findLastIndex((line) =>
    line.includes("Clan |")
  );
  const titleRow = waitingLines.findIndex((line) =>
    line.includes("UR ADVISOR")
  );
  const adviceRow = waitingLines.findIndex((line) =>
    line.includes("Best bets")
  );
  assertEquals(
    lastCardRow < titleRow && titleRow < adviceRow,
    true,
    "waiting hands must precede the title and advice placeholder",
  );
  // Unselected cards keep their single-line faces; the advisor omits the outer hand frame.
  assertEquals((out.match(/┌/g) ?? []).length, 8);
  assertEquals((out.match(/└/g) ?? []).length, 8);
  assertEquals((out.match(/╭/g) ?? []).length, 0);
  assertEquals((out.match(/╰/g) ?? []).length, 0);
  assertEquals((out.match(/╔/g) ?? []).length, 0);
  assertEquals((out.match(/Clan \|/g) ?? []).length, 8);
});

Deno.test("the waiting board shows remote hover and retains its starting resource scale", () => {
  const { g } = widest();
  const raw = quiet(() =>
    idle(
      "connected",
      "waiting for their card",
      { columns: 134, rows: 70 },
      false,
      false,
      {
        round: 2,
        headline: "opponent to move",
        progress: "round 1 resolved",
        you: { life: 8, pillz: 7 },
        them: { life: 9, pillz: 6 },
        barFloor: { life: 16, pillz: 15 },
        board: {
          you: Array.from(g.h1),
          them: Array.from(g.h2),
          hoveredThem: 1,
        },
      },
    )
  );
  assertEquals(raw.includes("\x1b[36m\u2554"), true);

  const lines = strip(raw).split("\n");
  for (const who of ["OPP", "YOU"]) {
    const status = lines.find((line) =>
      line.includes(who) && line.includes("Life")
    )!;
    const bars = status.match(/\[[^\]]+\]/g)!;
    assertEquals(bars[0].length - 2, 16, `${who} Life bar shrank`);
    assertEquals(bars[1].length - 2, 15, `${who} Pillz bar shrank`);
  }
});

Deno.test("card faces replace clan image tags with compact readable codes", () => {
  assertEquals(
    compactClanTags("After [Clan:56][clan:60] : Power +3"),
    "After OC/TO : Power +3",
  );

  const card = HandGenerator.handOf([
    "Dark Nunavik",
    "Nathan",
    "Orka",
    "Sando",
  ])[0];
  const rawLines = GameRenderer.cardLines(card);
  const lines = rawLines.map(strip);
  const face = lines.join("\n");
  assertEquals(face.includes("AS/FP/FZ/KO/ZE"), true);
  assertEquals(face.includes("Courage: Power +4"), true);
  assertEquals(face.toLowerCase().includes("[clan:"), false);
  assertEquals(
    lines.every((line) => line.length === 26),
    true,
    "every bordered card row must remain exactly 26 columns wide",
  );
  const styledFace = rawLines.join("\n");
  assertEquals(styledFace.includes("\x1b[31mFP"), true);
  assertEquals(styledFace.includes("\x1b[36mFZ"), true);
  assertEquals(styledFace.includes("\x1b[90m/\x1b[39m"), true);

  // Hypnos has six clan tags and a custom-background clan in the middle. Its reset must
  // not make the remaining codes or the padding at the end of the ability row transparent.
  const hypnos = CardGenerator.get("Hypnos")!;
  const hypnosLines = GameRenderer.cardLines(hypnos);
  const clanRow = hypnosLines.find((line) =>
    strip(line).includes("BZ/CO/FR/PA/PI/RA")
  )!;
  const backgrounds: number[] = [];
  let background = 49;
  for (let i = 0; i < clanRow.length;) {
    if (clanRow[i] === "\x1b") {
      const end = clanRow.indexOf("m", i);
      const codes = clanRow.slice(i + 2, end).split(";").map(Number);
      for (const code of codes) {
        if (code === 0 || code === 49) background = 49;
        if ((code >= 40 && code <= 47) || (code >= 100 && code <= 107)) {
          background = code;
        }
      }
      i = end + 1;
      continue;
    }
    const character = String.fromCodePoint(clanRow.codePointAt(i)!);
    backgrounds.push(background);
    i += character.length;
  }
  assertEquals(backgrounds.length, 26);
  assertEquals(
    backgrounds.slice(2, 24).every((value) => value !== 49),
    true,
    "the complete ability block, including trailing padding, needs a background",
  );
});

Deno.test("Unison cards use a green name badge without changing rarity", () => {
  const musardine = CardGenerator.get("Musardine")!;
  assertEquals(musardine.rarity, "r");
  assertEquals(musardine.hasUnisonAbility, true);

  const nameRow = GameRenderer.cardLines(musardine).find((line) =>
    strip(line).includes("Musardine")
  )!;
  assertEquals(nameRow.includes("\x1b[42m"), true);
  assertEquals(nameRow.includes("\x1b[30m"), true);

  const ordinaryRare = CardGenerator.get("Hypnos")!;
  assertEquals(ordinaryRare.rarity, "r");
  assertEquals(ordinaryRare.hasUnisonAbility, false);
  const ordinaryName = GameRenderer.cardLines(ordinaryRare).find((line) =>
    strip(line).includes("Hypnos")
  )!;
  assertEquals(ordinaryName.includes("\x1b[43m"), true);
  assertEquals(ordinaryName.includes("\x1b[42m"), false);
});

Deno.test("a battle-over frame contains no stale decision sections", () => {
  const { g } = widest();
  const out = strip(idle(
    "connected",
    "battle complete",
    { columns: 134, rows: 70 },
    true,
    false,
    {
      round: 4,
      headline: "battle over",
      progress: "final",
      outcome: "win",
      you: { life: 4, pillz: 0 },
      them: { life: 0, pillz: 2 },
      board: { you: Array.from(g.h1), them: Array.from(g.h2) },
    },
  ));

  assertEquals(out.includes("battle over"), true);
  assertEquals(out.includes("battle complete"), false);
  assertEquals(out.includes("__     __"), true);
  for (
    const stale of ["Best bets", "Previous advice", "Win % by pillz", "Played"]
  ) {
    assertEquals(out.includes(stale), false, `stale section: ${stale}`);
  }
  for (const card of [...g.h1, ...g.h2]) {
    assertEquals(out.includes(card.name), true, `missing ${card.name}`);
  }
});

Deno.test("the final result banner distinguishes wins, losses and draws", () => {
  for (
    const [style, rows] of [
      ["classic", 6],
      ["slant", 6],
      ["solid", 5],
      ["shadow", 6],
      ["framed", 7],
    ] as const
  ) {
    const lines = resultBanner("win", 80, style).map(strip);
    assertEquals(lines.length, rows);
    assertEquals(lines.every((line) => line.length <= 80), true);
    assertEquals(
      lines.some((line) =>
        line.includes(style === "classic" || style === "slant" ? "__" : "███")
      ),
      true,
    );
  }
  for (const outcome of ["win", "lose", "draw"] as const) {
    const lines = resultBanner(outcome, 80).map(strip);
    assertEquals(lines.length, 6);
    assertEquals(lines.every((line) => line.length <= 80), true);
  }
  const compact = strip(resultStylePreview({ columns: 80, rows: 24 }));
  for (const style of ["CLASSIC", "SLANT"]) {
    assertEquals(compact.includes(style), true);
  }
  assertEquals(compact.includes("Also available: solid, shadow, framed"), true);
  const tall = strip(resultStylePreview({ columns: 80, rows: 50 }));
  for (const style of ["CLASSIC", "SLANT", "SOLID", "SHADOW", "FRAMED"]) {
    assertEquals(tall.includes(style), true);
  }
});

Deno.test("an undersized terminal omits the card panel instead of squeezing it", () => {
  const { g, s } = widest();
  for (const size of [{ columns: 130, rows: 70 }, { columns: 134, rows: 51 }]) {
    const out = strip(quiet(() => render(g, s, { size, top: 8 })));
    assertEquals(out.includes("Best bets"), true);
    assertEquals(
      out.includes("Nathan"),
      false,
      `${JSON.stringify(size)} leaked cards`,
    );
    assertEquals(
      out.includes("Clan |"),
      false,
      `${JSON.stringify(size)} leaked cards`,
    );
    assertEquals(
      out.includes("╔") || out.includes("┌") || out.includes("╭"),
      false,
      `${JSON.stringify(size)} drew a partial hand`,
    );
  }
});

Deno.test("a large terminal uses the complete original hand UI and attack centre", () => {
  const { g, s } = widest();
  const raw = quiet(() =>
    render(g, s, {
      size: { columns: 134, rows: 70 },
      top: 3,
      board: {
        you: Array.from(g.h1),
        them: Array.from(g.h2),
        turn: "you",
        selectedYou: 0,
        selectedThem: 0,
        hoveredThem: 1,
        battle: {
          round: 1,
          them: {
            card: "Nathan",
            pillz: 3,
            availablePillz: 12,
            attack: 16,
          },
          you: {
            card: "Genmaicha",
            pillz: 4,
            fury: true,
            availablePillz: 7,
            attack: 49,
          },
        },
      },
    })
  );
  const out = strip(raw);

  assertEquals(
    (out.match(/╔/g) ?? []).length,
    3,
    "selected cards and the remote-hovered card have double borders",
  );
  assertEquals(
    (out.match(/╚/g) ?? []).length,
    3,
    "selected cards and the remote-hovered card have double borders",
  );
  assertEquals((out.match(/┌/g) ?? []).length, 5);
  assertEquals((out.match(/└/g) ?? []).length, 5);
  assertEquals(raw.includes("\x1b[36m╔"), true, "remote hover uses cyan");
  assertEquals((out.match(/╭/g) ?? []).length, 0);
  assertEquals((out.match(/╰/g) ?? []).length, 0);
  assertEquals((out.match(/Ability/g) ?? []).length >= 8, true);
  assertEquals((out.match(/Bonus/g) ?? []).length >= 8, true);
  assertEquals((out.match(/Clan \|/g) ?? []).length, 8);
  assertEquals((out.match(/\[OOOOOOOOOOOO\]/g) ?? []).length, 2);
  for (const life of [g.p1.life, g.p2.life]) {
    assertEquals(
      out.includes(
        `[${"♥".repeat(life)}${"-".repeat(12 - life)}] Life ${
          String(life).padStart(2)
        }`,
      ),
      true,
    );
  }
  assertEquals(out.includes("Round |"), false);
  assertEquals(out.includes("ROUND 1 BATTLE"), false);
  assertEquals(out.includes("UR ADVISOR"), true);
  assertEquals(out.includes("UR SOLVER"), false);
  const lines = out.split("\n");
  const lastCardRow = lines.findLastIndex((line) => line.includes("Clan |"));
  const titleRow = lines.findIndex((line) => line.includes("UR ADVISOR"));
  const bestBetsRow = lines.findIndex((line) => line.includes("Best bets"));
  const matrixRow = lines.findIndex((line) => line.includes("Win % by pillz"));
  assertEquals(
    lastCardRow < titleRow && titleRow < bestBetsRow &&
      bestBetsRow < matrixRow,
    true,
    "hands must precede the title, recommendations and grid",
  );
  assertEquals(out.includes("╭") || out.includes("╰"), false);
  const opponentStatus = lines.findIndex((line) =>
    line.includes("OPP") && line.includes("Life")
  );
  const ourStatus = lines.findIndex((line) =>
    line.includes("YOU") && line.includes("Life")
  );
  assertEquals(opponentStatus > 0, true);
  assertEquals(lines[opponentStatus].includes("▶"), false);
  assertEquals(lines[ourStatus].includes("▶ YOU ◀"), true);
  assertEquals(raw.includes("\x1b[33m▶ YOU ◀"), true);
  assertEquals(lines[opponentStatus + 1], "");
  assertEquals(lines[ourStatus - 1], "");
  for (
    const [who, at] of [["OPP", opponentStatus], ["YOU", ourStatus]] as const
  ) {
    const line = lines[at];
    assertEquals(
      line.indexOf("Life") < line.indexOf(who) &&
        line.indexOf(who) < line.indexOf("Pillz"),
      true,
      `${who} must sit between Life and Pillz`,
    );
    assertEquals(
      line.includes("|"),
      false,
      "resource dividers are no longer needed",
    );
    assertEquals(
      /\] Life\s+\d+\s+(?:▶ )?(?:OPP|YOU)(?: ◀)?\s+\d+\s+Pillz \[/.test(
        line,
      ),
      true,
      `${who} resource values must be closest to the player label`,
    );
    const labelCentre = line.indexOf(who) + 1;
    assertEquals(
      Math.abs(labelCentre - (134 - 1) / 2) <= 0.5,
      true,
      `${who} label is not anchored to the terminal centre`,
    );
  }
  assertEquals(
    lines[opponentStatus].indexOf("OPP"),
    lines[ourStatus].indexOf("YOU"),
    "OPP and YOU must share a fixed column",
  );
  assertEquals(
    (lines.slice(0, opponentStatus).join("\n").match(/Clan \|/g) ?? []).length,
    4,
    "the opponent status belongs below all four opponent cards",
  );

  const theirBattle = lines.find((line) => /16\s+Attack/.test(line))!;
  const ourBattle = lines.find((line) => /49\s+Attack/.test(line))!;
  assertEquals(theirBattle.includes("Nathan 3 pillz"), true);
  assertEquals(ourBattle.includes("Genmaicha 4 pillz + Fury"), true);
  assertEquals(raw.includes("\x1b[34m3 pillz\x1b[39m"), true);
  assertEquals(raw.includes("\x1b[35m4 pillz\x1b[39m"), true);
  assertEquals(
    theirBattle.indexOf("Attack"),
    ourBattle.indexOf("Attack"),
    "both battle rows must read in the same direction",
  );
  assertEquals(theirBattle.indexOf("16") < theirBattle.indexOf("Attack"), true);
  assertEquals(ourBattle.indexOf("49") < ourBattle.indexOf("Attack"), true);
  assertEquals(theirBattle.indexOf("16"), ourBattle.indexOf("49"));
  assertEquals(
    Math.abs(
      theirBattle.indexOf("16") + 0.5 -
        (lines[opponentStatus].indexOf("OPP") + 1),
    ) <= 0.5,
    true,
    "attack values must be centred under the player labels",
  );
  assertEquals(theirBattle.includes("THEM"), false);
  assertEquals(ourBattle.includes("YOU"), false);
  assertEquals(raw.includes("\x1b[31m16"), true);
  assertEquals(raw.includes("\x1b[32m49"), true);

  const opponentTurnLines = strip(quiet(() =>
    render(g, s, {
      size: { columns: 134, rows: 70 },
      top: 3,
      board: {
        you: Array.from(g.h1),
        them: Array.from(g.h2),
        turn: "them",
      },
    })
  )).split("\n");
  const activeOpponent = opponentTurnLines.find((line) =>
    line.includes("OPP") && line.includes("Life")
  )!;
  const inactiveUs = opponentTurnLines.find((line) =>
    line.includes("YOU") && line.includes("Life")
  )!;
  assertEquals(activeOpponent.includes("▶ OPP ◀"), true);
  assertEquals(inactiveUs.includes("▶"), false);
  assertEquals(
    opponentTurnLines.length,
    lines.length,
    "the turn indicator must not add any rows",
  );

  const tied = quiet(() =>
    render(g, s, {
      size: { columns: 134, rows: 70 },
      top: 3,
      board: {
        you: Array.from(g.h1),
        them: Array.from(g.h2),
        battle: {
          round: 1,
          them: { card: "Nathan", pillz: 3, attack: 49 },
          you: { card: "Genmaicha", pillz: 6, attack: 49 },
        },
      },
    })
  );
  assertEquals((tied.match(/\x1b\[33m49/g) ?? []).length, 2);

  const hidden = strip(quiet(() =>
    render(g, s, {
      size: { columns: 134, rows: 70 },
      top: 3,
      board: {
        you: Array.from(g.h1),
        them: Array.from(g.h2),
        battle: { round: 2, them: { card: "Nathan" } },
      },
    })
  )).split("\n");
  const hiddenAttack = hidden.find((line) =>
    line.includes("Nathan") && line.includes("Attack")
  )!;
  assertEquals(hiddenAttack.includes("pillz"), false);
  assertEquals(hiddenAttack.includes("?"), false);

  const empty = strip(quiet(() =>
    render(g, s, {
      size: { columns: 134, rows: 70 },
      top: 3,
      board: { you: Array.from(g.h1), them: Array.from(g.h2) },
    })
  )).split("\n").filter((line) =>
    line.includes("no card selected") && line.includes("Attack")
  );
  assertEquals(empty.length, 2);
  assertEquals(empty.every((line) => !line.includes("?")), true);
});

Deno.test("resource bars distinguish recent spending, damage and gains", () => {
  const { g, s } = widest();
  const raw = quiet(() =>
    render(g, s, {
      size: { columns: 134, rows: 70 },
      top: 1,
      phase: {
        round: 2,
        headline: "you move first",
        progress: "round 1 resolved",
        you: { life: 8, pillz: 7 },
        them: { life: 13, pillz: 10 },
        board: {
          you: Array.from(g.h1),
          them: Array.from(g.h2),
          beforeLastRound: {
            you: { life: 12, pillz: 12 },
            them: { life: 12, pillz: 8 },
          },
        },
      },
    })
  );

  // Lost life is bright red, spent pillz yellow, and newly gained resources green.
  assertEquals(raw.includes("\x1b[91m----\x1b[39m"), true);
  assertEquals(raw.includes("\x1b[33m-----\x1b[39m"), true);
  assertEquals(raw.includes("\x1b[92m♥\x1b[39m"), true);
  assertEquals(raw.includes("\x1b[92mOO\x1b[39m"), true);
  const lines = strip(raw).split("\n");
  const opponentStatus = lines.find((line) =>
    line.includes("OPP") && line.includes("Life")
  )!;
  const ourStatus = lines.find((line) =>
    line.includes("YOU") && line.includes("Life")
  )!;
  assertEquals(
    opponentStatus.indexOf("OPP"),
    ourStatus.indexOf("YOU"),
    "one- and two-digit resources must not move the player labels",
  );
  assertEquals(/\] Life\s+13\s+OPP\s+10\s+Pillz \[/.test(opponentStatus), true);
  assertEquals(/\] Life\s+8\s+YOU\s+7\s+Pillz \[/.test(ourStatus), true);
});

Deno.test("resource bars retain the game-start scale after both sides fall below it", () => {
  const game = quiet(() => {
    const g = new Game(
      new Player(16, 15, 0),
      new Player(14, 13, 1),
      HandGenerator.handOf(["Genmaicha", "Orka", "Sando", "Deborah"]),
      HandGenerator.handOf([
        "Nathan",
        "El Kuzco",
        "Noon Steevens",
        "Strygia",
      ]),
      Turn.PLAYER_1,
      false,
    );
    g.p1.life = 8;
    g.p1.pillz = 7;
    g.p2.life = 9;
    g.p2.pillz = 6;
    return g;
  });
  const search = new Search(game);
  const lines = strip(quiet(() =>
    render(game, search, {
      size: { columns: 134, rows: 90 },
      top: 1,
    })
  )).split("\n");

  for (const who of ["OPP", "YOU"]) {
    const status = lines.find((line) =>
      line.includes(who) && line.includes("Life")
    )!;
    const bars = status.match(/\[[♥O-]+\]/g)!;
    assertEquals(bars.length, 2);
    assertEquals(bars[0].length - 2, 16, `${who} Life bar shrank`);
    assertEquals(bars[1].length - 2, 15, `${who} Pillz bar shrank`);
  }
});

Deno.test("ranked bets colour zero neutrally, all-in distinctly, and Fury red", () => {
  const { g, s } = widest();
  const score = (pillz: number, fury: boolean, average: number) => {
    const c = s.candidates.find((c) => c.pillz === pillz && c.fury === fury)!;
    c.average = average;
    c.minimax = average;
    c.done = s.samples;
  };

  // Twelve plain pillz and nine plus Fury both consume a twelve-pill stack. Eight does not.
  score(12, false, 1);
  score(9, true, 0.8);
  score(8, false, 0.6);
  score(0, false, 0.4);
  const out = quiet(() =>
    render(g, s, {
      size: { columns: 120, rows: 40 },
      top: 4,
      played: { index: 0, pillz: 9, fury: true },
      history: [{
        round: 1,
        card: "Genmaicha",
        move: { index: 0, pillz: 0, fury: false },
      }],
    })
  );

  assertEquals(out.includes("\x1b[35m12 pillz\x1b[39m"), true);
  assertEquals(out.includes("\x1b[35m9 pillz\x1b[39m"), true);
  assertEquals(out.includes("\x1b[34m8 pillz\x1b[39m"), true);
  assertEquals(
    (out.match(/\x1b\[90m0 pillz\x1b\[39m/g) ?? []).length >= 2,
    true,
  );
  assertEquals(out.includes("\x1b[34m0 pillz\x1b[39m"), false);
  // Ranked and played labels include the leading space; matrix row labels do not.
  assertEquals(
    (out.match(/\x1b\[31m \+ Fury\x1b\[39m/g) ?? []).length >= 2,
    true,
  );
  assertEquals(out.includes("\x1b[31m+ Fury\x1b[39m"), true);
  assertEquals(strip(out).includes("+ fury"), false);
});

Deno.test("matrix cells line up with their pillz headers", () => {
  const { g, s } = widest();
  for (const [columns, rows] of [[80, 40], [120, 40]] as [number, number][]) {
    const drawn = quiet(() => render(g, s, { size: { columns, rows } })).split(
      "\n",
    ).map(strip);

    // The matrix is the block from its header to the next blank line.
    const at = drawn.findIndex((l) => l.includes("Win % by pillz"));
    assertEquals(at >= 0, true, "no matrix header");
    const header = drawn[at];
    const body: string[] = [];
    for (let i = at + 1; i < drawn.length && drawn[i].trim() !== ""; i++) {
      // Card and fury rows end in a cell; the legend that follows them ends in a word.
      if (/(\d+|-|\.)$/.test(drawn[i].trimEnd())) body.push(drawn[i]);
    }
    assertEquals(body.length > 0, true, "no card rows found");

    // Cells are right-aligned in their column, so compare right edges.
    const rightEdges = (line: string, from: number) =>
      [...line.matchAll(/\S+/g)]
        .filter((m) => m.index! >= from)
        .map((m) => m.index! + m[0].length);

    // Bet labels start after the "win % by pillz" caption.
    const headerCols = rightEdges(header, header.indexOf("pillz") + 5);
    assertEquals(
      headerCols.length > 1,
      true,
      `no bet columns in header: "${header}"`,
    );

    for (const row of body) {
      // Card rows carry a name and stats first; cells begin at the first header column.
      const cellCols = rightEdges(row, headerCols[0] - 3);
      assertEquals(
        cellCols,
        headerCols,
        `${columns} cols: cells do not sit under their headers\n  ${header}\n  ${row}`,
      );
    }
  }
});

Deno.test("the Played history is demoted below the complete grid", () => {
  const { g, s } = widest();
  const lines = strip(
    quiet(() =>
      render(g, s, {
        size: { columns: 134, rows: 100 },
        top: 3,
        played: {
          index: 0,
          pillz: 2,
          fury: false,
          percent: 60,
          better: 1,
          best: 70,
        },
      })
    ),
  ).split("\n");
  const matrixAt = lines.findIndex((line) => line.includes("Win % by pillz"));
  const playedAt = lines.findIndex((line) =>
    line.trimStart().startsWith("Played")
  );
  assertEquals(matrixAt >= 0, true, "grid is missing");
  assertEquals(playedAt > matrixAt, true, "Played must follow the grid");
});

Deno.test("the footer uses a compact live indicator and right-aligned battle id", () => {
  const { g, s } = widest();
  const raw = quiet(() =>
    render(g, s, {
      size: { columns: 80, rows: 24 },
      status: "solved, waiting for you to play",
      connection: "connected",
      battleId: 901613,
      autoQueue: false,
    })
  ).split("\n").at(-1)!;
  const footer = strip(raw);
  assertEquals(footer.includes("LIVE"), true);
  assertEquals(footer.includes("solved, waiting for you to play"), true);
  assertEquals(footer.includes("localhost"), false);
  assertEquals(footer.trimEnd().endsWith("Battle 901613"), true);
  assertEquals(raw.includes("\x1b[92m●\x1b[39m"), true);
});
