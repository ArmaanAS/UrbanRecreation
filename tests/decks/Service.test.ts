// The deck service answers only Deck Lab's own page, the site (the userscript panel) and
// local tools, and it never needs the owner's data to refuse a bad request.
import { assert, assertEquals, assertStringIncludes } from "@std/assert";
import { type MatchupRequest, type MatchupResponse, type MatchupRunner, MemoryMatchupCache, type SolveRequest } from "@/decks/Matchup.ts";
import { handle, useSolver } from "@/decks/Service.ts";

const req = (path: string, init: RequestInit & { origin?: string } = {}) => {
  const headers = new Headers(init.headers);
  if (init.origin) headers.set("origin", init.origin);
  return new Request(`http://127.0.0.1:8788${path}`, { ...init, headers });
};

Deno.test("Deck Lab's page and script are served", async () => {
  const page = await handle(req("/"));
  assertEquals(page.status, 200);
  assertStringIncludes(await page.text(), "<title>Deck Lab</title>");
  const script = await handle(req("/app.js"));
  assertEquals(script.headers.get("content-type"), "text/javascript; charset=utf-8");
  await script.body?.cancel();
});

Deno.test("only the site, Deck Lab itself and local tools may call it", async () => {
  assertEquals((await handle(req("/api/formats", { origin: "https://evil.example" }))).status, 403);
  for (const origin of ["https://www.urban-rivals.com", "http://127.0.0.1:8788", "http://localhost:8788"]) {
    const res = await handle(req("/api/formats", { origin }));
    assertEquals(res.status, 200, origin);
    await res.body?.cancel();
  }
  const preflight = await handle(req("/api/report", { method: "OPTIONS", origin: "https://www.urban-rivals.com" }));
  assertEquals(preflight.status, 204);
  assertEquals(preflight.headers.get("access-control-allow-origin"), "https://www.urban-rivals.com");
});

Deno.test("a malformed deck is refused before any data is read", async () => {
  const bad = await handle(req("/api/report", { method: "POST", body: JSON.stringify({ characters: [{ id: 1, level: 9 }] }) }));
  assertEquals(bad.status, 400);
  const notJson = await handle(req("/api/report", { method: "POST", body: "{" }));
  assertEquals(notJson.status, 400);
  await bad.body?.cancel();
  await notJson.body?.cancel();
});

/** Answers every solve at once with a value from the hands' card ids; waits for `gate` if given. */
class InstantRunner implements MatchupRunner {
  solves = 0;
  constructor(private readonly gate?: Promise<void>) {}
  async run(requests: readonly MatchupRequest[], onResponse?: (r: MatchupResponse, i: number) => void, signal?: AbortSignal) {
    if (this.gate) await this.gate;
    signal?.throwIfAborted();
    return requests.map((request, id) => {
      const r = request as SolveRequest;
      this.solves++;
      const leader = r.p1.findIndex(([card]) => card === 269);
      const response: MatchupResponse = leader >= 0
        ? { id, kind: "solve", refused: `P1 slot ${leader} contains unsupported Leader` }
        : {
          id,
          kind: "solve",
          value: Math.tanh((r.p1[0][0] - r.p2[0][0]) / 500),
          worst: -1,
          best: 1,
          best_move: { hand_index: 0, pillz: 1, fury: false },
          ko_share: 0,
          koed_share: 0,
          root_moves: 1,
          replies: 1,
          ms: 1,
        };
      onResponse?.(response, id);
      return response;
    });
  }
}

const post = (path: string, body: unknown) => req(path, { method: "POST", body: JSON.stringify(body) });
const draft = [{ id: 123, level: 1 }, { id: 124, level: 1 }, { id: 138, level: 1 }, { id: 139, level: 1 }, { id: 269, level: 5 }];

async function settle() {
  for (let i = 0; i < 100; i++) {
    const view = await (await handle(req("/api/matchup"))).json();
    if (view.state !== "running") return view;
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
  throw new Error("the job never finished");
}

Deno.test("a draft is scored against the captured Tourney field in the background", async () => {
  const runner = new InstantRunner();
  useSolver(() => Promise.resolve({ runner, cache: new MemoryMatchupCache() }));
  const started = await handle(post("/api/matchup", { characters: draft, n: 12, opponent: { format: 54363 } }));
  assertEquals(started.status, 202);
  const view = await started.json();
  assertEquals(view.deck, draft.map((c) => [c.id, c.level]));
  assertStringIncludes(view.against, "12 of the");
  assertStringIncludes(view.against, "captured Tourney opponents");

  const done = await settle();
  assertEquals(done.state, "done", done.error);
  assertEquals(done.result.pairs, 12);
  assertEquals(done.result.scored + done.result.refused, 12);
  assert(runner.solves > 0);
  // Hands holding the Leader (269) are refused and say whose card it was.
  assert(done.result.refused > 0);
  assertEquals(done.result.refusals[0].side, "a");
  assertEquals(done.result.refusals[0].card, [269, 5]);
  // Every scored pair is counted under its opposing hand's clan ("mixed" without one).
  assertEquals(done.result.byClan.reduce((s: number, c: { scored: number }) => s + c.scored, 0), done.result.scored);
});

Deno.test("a new job stops the running one", async () => {
  let open!: () => void;
  const gate = new Promise<void>((resolve) => open = resolve);
  useSolver(() => Promise.resolve({ runner: new InstantRunner(gate), cache: new MemoryMatchupCache() }));
  const first = await (await handle(post("/api/matchup", { characters: draft, n: 4, opponent: { format: 54363 } }))).json();
  useSolver(() => Promise.resolve({ runner: new InstantRunner(), cache: new MemoryMatchupCache() }));
  await handle(post("/api/matchup", { characters: draft, n: 4, opponent: { format: 54363 } })).then((r) => r.body?.cancel());
  open();
  const done = await settle();
  assertEquals(done.state, "done");
  assert(done.id > first.id);
  const stopped = await (await handle(req("/api/matchup", { method: "DELETE" }))).json();
  assertEquals(stopped.id, done.id, "stopping a finished job leaves it as it was");
  assertEquals(stopped.state, "done");
});

Deno.test("bad scoring requests are refused before anything runs", async () => {
  useSolver(() => Promise.reject(new Error("the solver must not start")));
  const cases: [unknown, string][] = [
    [{ characters: draft.slice(0, 3), opponent: { format: 54363 } }, "4 to 30"],
    [{ characters: draft, n: 0, opponent: { format: 54363 } }, "hand pairs"],
    [{ characters: draft, n: 401, opponent: { format: 54363 } }, "hand pairs"],
    [{ characters: draft, opponent: {} }, "opponent must be"],
    [{ characters: draft, opponent: { format: 999999 } }, "no opposing hands"],
  ];
  for (const [body, message] of cases) {
    const res = await handle(post("/api/matchup", body));
    assertEquals(res.status, 400, JSON.stringify(body));
    assertStringIncludes((await res.json()).error, message);
  }
});

Deno.test("the clan matrix is read per format, and a bad format is refused", async () => {
  const none = await (await handle(req("/api/clans?format=999999"))).json();
  assertEquals(none, { day: null, night: null });
  const bad = await handle(req("/api/clans?format=x"));
  assertEquals(bad.status, 400);
  await bad.body?.cancel();
});

Deno.test("a swap search needs a slot inside the draft", async () => {
  useSolver(() => Promise.reject(new Error("the solver must not start")));
  for (const swap of [{ slot: 5 }, { slot: -1 }, { slot: "x" }, { slot: 0, scope: "world" }]) {
    const res = await handle(post("/api/matchup", { characters: draft, opponent: { format: 54363 }, swap }));
    assertEquals(res.status, 400, JSON.stringify(swap));
    assertStringIncludes((await res.json()).error, "swap.");
  }
});
