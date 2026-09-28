// Automated Training play. The owner asked for it on 2026-09-27: "I'm fully happy for you to
// set my decks automatically and PLAY THE GAME", in Training, where the opponent is the
// site's AI, forfeiting costs nothing and losing does not matter. The games are for evidence:
// every one is captured like the owner's own, and the driver picks decks and moves that
// should show the engines something the capture corpus has not (src/situations/).
//
//   deno task log                                # the capture server, with the autoplay broker
//   (the game open at https://www.urban-rivals.com/game/play/webgl/, userscript 0.10.0+, awake)
//   deno task autoplay --games 5                 # five games with the current deck
//   deno task autoplay --plan --batches 3        # three decks the planner picks, 4 games each
//   deno task autoplay --deck lab.json           # save this deck (DeckSpec) first
//   deno task autoplay --policy solver           # just the advisor's best move, no novelty
//
// The driver asks the log server's broker (scripts/AutoplayBroker.ts) for one call at a time,
// and the userscript in the game tab makes it with the game client's own session. The broker,
// not this file, decides what may reach the site: Training only, the deck actions Collection
// Pro uses, nothing else. Positions are rebuilt from the capture file the log server writes,
// with the same reconstruct/buildPosition the live advisor uses.
import { expandEntries, type CaptureEntry, loadAbilities } from "./BattleCapture.ts";
import { reconstruct } from "./ExtractBattle.ts";
import { buildPosition, isOurs } from "../src/solver/Advisor.ts";
import Search, { moveKey } from "../src/solver/Search.ts";
import type Game from "../src/game/Game.ts";
import type Hand from "../src/game/Hand.ts";
import { Turn } from "../src/game/types/Types.ts";
import { LatchLedger, type Move as ProbeMove, noveltyOf, RoundProbe, slotIds } from "../src/situations/Situations.ts";
import { type Collection, gameOf, type GameRecord, neverFired, scanRecord } from "../src/situations/Corpus.ts";

const ROOT = "http://127.0.0.1:8787";
const BATTLE_DIR = "captures/battles";
const GAME_DIR = "captures/games";
const SITUATIONS = "data/situations.json";
const COLLECTION = "data/my_collection.json";
const TRAINING_ROOM = 6;
const TRAINING_RULE = 2;
const POLL_MS = 1000;
/** Deck slots the planner may overwrite: the owner's two finished Lab decks. */
const PLAN_SLOTS = [37652428, 37652421];

// deno-lint-ignore no-explicit-any
type Json = any;
type Reconstructed = ReturnType<typeof reconstruct>;

export interface Move {
  index: number;
  pillz: number;
  fury: boolean;
}

/** A deck to save before playing: `slot` is an existing deck id to overwrite (0 = new). */
export interface DeckSpec {
  name: string;
  slot: number;
  cards: { id: number; level: number; state?: string }[];
}

export interface DecisionContext {
  rec: Reconstructed;
  /** The advisor's position: our turn, the opponent's first card applied if they moved. */
  game: Game;
  deadline: number;
}
export type Policy = (ctx: DecisionContext) => Promise<{ move: Move; how: string } | undefined>;

export class Bridge {
  constructor(readonly root = ROOT) {}

  async state(): Promise<{ bridge: boolean; lastPollAgoMs: number | null }> {
    const res = await fetch(`${this.root}/autoplay/state`);
    return await res.json();
  }

  async #post(body: unknown): Promise<Json> {
    const res = await fetch(`${this.root}/autoplay/call`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    const out = await res.json();
    if (!out.ok) throw new Error(out.error);
    return out.result;
  }

  /** A private-API call; its `data`, or an error carrying the site's own message. */
  async call(call: string, params: Record<string, unknown> = {}): Promise<Json> {
    const result = await this.#post({ call, params });
    if (result?.errors?.length) throw new Error(`${call}: ${result.errors.map((e: Json) => e.message).join("; ")}`);
    return result?.data;
  }

  deck(action: "loaddeck" | "savedeck", fields: [string, string][]): Promise<Json> {
    return this.#post({ deck: action, fields });
  }
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function waitForBridge(bridge: Bridge, timeoutMs = 60_000) {
  const until = Date.now() + timeoutMs;
  for (;;) {
    try {
      if ((await bridge.state()).bridge) return;
    } catch {
      throw new Error("the log server is not running: start it with `deno task log`");
    }
    if (Date.now() > until) {
      throw new Error(
        "no game tab is polling the autoplay bridge: open https://www.urban-rivals.com/game/play/webgl/ " +
          "with userscript 0.10.0 or later, and keep it awake (a hidden or sleeping tab is frozen)",
      );
    }
    await sleep(1000);
  }
}

async function readBattle(id: number): Promise<CaptureEntry[]> {
  const text = await Deno.readTextFile(`${BATTLE_DIR}/${id}.jsonl`);
  const entries: CaptureEntry[] = [];
  for (const line of text.split("\n")) {
    if (!line.trim()) continue;
    try {
      entries.push(JSON.parse(line) as CaptureEntry);
    } catch { /* torn final line */ }
  }
  return expandEntries(entries, await loadAbilities());
}

// ---------------------------------------------------------------------------------------
// Policies
// ---------------------------------------------------------------------------------------

async function searchFor(game: Game, deadline: number, budgetMs: number) {
  const search = new Search(game);
  const stop = Math.min(deadline, Date.now() + budgetMs);
  while (!search.done && Date.now() < stop) await search.workFor(120);
  return search;
}

/** The top-ranked move of the live advisor's own search, within the time left. */
export const solverPolicy = (budgetMs: number): Policy => async ({ game, deadline }) => {
  const search = await searchFor(game, deadline, budgetMs);
  const best = search.best();
  return best && {
    move: { index: best.index, pillz: best.pillz, fury: best.fury },
    how: `solver ${Math.round(search.percent(best.average))}%`,
  };
};

function legalMoves(hand: Hand, pillz: number, only?: number): ProbeMove[] {
  const out: ProbeMove[] = [];
  for (let i = 0; i < 4; i++) {
    if (hand[i].played || (only !== undefined && i !== only)) continue;
    for (let p = 0; p <= pillz; p++) {
      out.push([i, p, false]);
      if (p + 3 <= pillz) out.push([i, p, true]);
    }
  }
  return out;
}

/**
 * How new each of our legal moves would be (src/situations/): the novelty of the round's
 * predicted situations, averaged over every reply the opponent could make - every card and
 * bet when we move first, every bet on their revealed card when we move second. Keyed like
 * Search's moves; undefined when the round cannot be rebuilt.
 */
export function noveltyByMove(rec: Reconstructed, counts: Readonly<Record<string, number>>): Map<string, number> | undefined {
  const tc = rec.testcase;
  if (tc === null || rec.firstPlayer === null || rec.mySide === null) return undefined;
  const game = gameOf(tc);
  const captured = [...rec.players[rec.firstPlayer].hand, ...rec.players[(1 - rec.firstPlayer) as 0 | 1].hand];
  const ids = slotIds(game, captured);
  const ledger = new LatchLedger();
  for (const m of tc.moves) {
    const [m1, m2] = game.playingFirst === Turn.PLAYER_1 ? [m.s1, m.s2] : [m.s2, m.s1];
    ledger.play(game, m1 as unknown as ProbeMove, m2 as unknown as ProbeMove, ids);
    if (!game.isPlaying) return undefined;
  }
  const probe = new RoundProbe(game, { ids, ledger });
  const usP1 = rec.mySide === rec.firstPlayer;
  const theirSide = 1 - rec.mySide;
  const known = rec.rounds[tc.moves.length]?.moves.find((m) => m.side === theirSide)?.index;
  const mine = legalMoves(usP1 ? game.h1 : game.h2, (usP1 ? game.p1 : game.p2).pillz);
  const theirs = legalMoves(usP1 ? game.h2 : game.h1, (usP1 ? game.p2 : game.p1).pillz, known);
  const out = new Map<string, number>();
  for (const m of mine) {
    let total = 0, n = 0;
    for (const t of theirs) {
      try {
        total += noveltyOf(usP1 ? probe.situations(m, t) : probe.situations(t, m), counts);
        n++;
      } catch { /* a bet the rebuilt position cannot pay; the live one decides legality */ }
    }
    out.set(moveKey({ index: m[0], pillz: m[1], fury: m[2] }), n ? total / n : 0);
  }
  return out;
}

/**
 * Evidence, weighed against winning: the move's expected new situations plus `winWeight`
 * times its win chance from the advisor's search (a move the search has not reached yet
 * counts at the median of those it has). Winning matters because a game lost early shows
 * fewer rounds: with only novelty in charge, a round-one all-in at a 2% win chance won on
 * 1.7 new situations against 1.2. At 1.5 a large swing in win chance decides, and novelty
 * decides between moves that are close.
 */
export const noveltyPolicy = (counts: Record<string, number>, budgetMs: number, winWeight = 1.5): Policy =>
async ({ rec, game, deadline }) => {
  let scores: Map<string, number> | undefined;
  try {
    scores = noveltyByMove(rec, counts);
  } catch {
    scores = undefined;
  }
  const search = await searchFor(game, deadline, budgetMs);
  const known = search.candidates.filter((c) => !Number.isNaN(c.average)).map((c) => search.percent(c.average) / 100)
    .sort((a, b) => a - b);
  const median = known.length ? known[Math.floor(known.length / 2)] : 0.5;
  let best: { move: Move; score: number; novelty: number; win: number } | undefined;
  for (const c of search.candidates) {
    const novelty = scores?.get(moveKey(c)) ?? 0;
    const win = Number.isNaN(c.average) ? median : search.percent(c.average) / 100;
    const score = novelty + winWeight * win;
    if (!best || score > best.score) best = { move: { index: c.index, pillz: c.pillz, fury: c.fury }, score, novelty, win };
  }
  return best && {
    move: best.move,
    how: `novelty ${best.novelty.toFixed(2)}, win ${Math.round(best.win * 100)}%${scores ? "" : " (no novelty model)"}`,
  };
};

// ---------------------------------------------------------------------------------------
// Decks
// ---------------------------------------------------------------------------------------

/** Save `spec` into its slot and make it current, then read it back to confirm. */
export async function saveDeck(bridge: Bridge, spec: DeckSpec) {
  const before = spec.slot > 0 ? await bridge.deck("loaddeck", [["id", String(spec.slot)]]) : null;
  if (spec.slot > 0 && Number(before?.deck?.id) !== spec.slot) throw new Error(`deck ${spec.slot} did not load`);
  const fields: [string, string][] = [
    ["id", String(spec.slot)],
    ["name", spec.name],
    ["set_current", "true"],
    ...spec.cards.flatMap((c, i): [string, string][] => [
      [`characters[${i}][id]`, String(c.id)],
      [`characters[${i}][level]`, String(c.level)],
      [`characters[${i}][state]`, c.state ?? ""],
    ]),
  ];
  const saved = await bridge.deck("savedeck", fields);
  const id = Number(saved?.deck?.id);
  if (!Number.isInteger(id) || id <= 0) throw new Error(`the site did not save the deck: ${JSON.stringify(saved).slice(0, 300)}`);
  const loaded = await bridge.deck("loaddeck", [["id", String(id)]]);
  const got = (loaded?.deck?.Characters ?? loaded?.deck?.characters ?? []) as Json[];
  const key = (c: Json) => `${c.id}:${c.level}:${c.state ?? ""}`;
  const verified = JSON.stringify(got.map(key).sort()) === JSON.stringify(spec.cards.map(key).sort());
  // The same undo log Deck Lab keeps: re-saving `before` puts the old deck back.
  await Deno.writeTextFile(
    "data/deck_history.jsonl",
    JSON.stringify({
      t: new Date().toISOString(),
      requestId: `autoplay-${Date.now()}`,
      deckId: id,
      name: spec.name,
      before: before?.deck ?? null,
      after: loaded?.deck ?? null,
      verified,
      siteResponse: saved,
    }) + "\n",
    { append: true },
  );
  if (!verified) throw new Error("the saved deck does not hold the requested cards");
  return id;
}

/**
 * The next test deck: the clan whose owned cards hold the most abilities no capture shows
 * firing (a text never seen at all counts double), all eight from that clan so its bonus is
 * always live, topped up with its other owned cards. Leaders and Oculus are left out.
 */
export function planDeck(collection: Collection, counts: Readonly<Record<string, number>>, slot: number, avoid: ReadonlySet<string> = new Set()): DeckSpec | undefined {
  const { clans } = neverFired(collection, counts);
  const edition = (id: number, level: number) => {
    const editions = collection.cards[String(id)]?.[String(level)] ?? {};
    return (editions[""] ?? 0) > 0 ? "" : Object.keys(editions).find((e) => editions[e] > 0) ?? "";
  };
  const weight = (c: { textFired: boolean }) => (c.textFired ? 1 : 2);
  const ranked = clans
    .filter((c) => c.clan !== "Leader" && c.clan !== "Oculus" && c.owned >= 8 && !avoid.has(c.clan))
    .map((c) => ({ c, value: c.cards.slice().sort((a, b) => weight(b) - weight(a)).slice(0, 8).reduce((s, x) => s + weight(x), 0) }))
    .sort((a, b) => b.value - a.value);
  const pick = ranked[0]?.c;
  if (!pick) return undefined;
  const cards = pick.cards.slice().sort((a, b) => weight(b) - weight(a)).slice(0, 8)
    .map((c) => ({ id: c.id, level: c.level, state: edition(c.id, c.level) }));
  if (cards.length < 8) {
    // Top up with the clan's other owned cards, highest level first.
    for (const [idText, levels] of Object.entries(collection.cards)) {
      if (cards.length >= 8) break;
      const id = Number(idText);
      if (cards.some((c) => c.id === id)) continue;
      const owned = Object.entries(levels).filter(([, e]) => Object.values(e).some((n) => n > 0)).map(([l]) => Number(l));
      if (!owned.length) continue;
      const level = Math.max(...owned);
      if (clanOf(id, level) === pick.clan) cards.push({ id, level, state: edition(id, level) });
    }
  }
  if (cards.length < 8) return undefined;
  return { name: `Auto ${pick.clan}`.slice(0, 32), slot, cards };
}

let clanRows: Map<number, string> | undefined;
function clanOf(id: number, level: number): string | undefined {
  if (!clanRows) {
    clanRows = new Map();
    for (const r of JSON.parse(Deno.readTextFileSync("data/data.json")) as Json[]) clanRows.set(r.id * 8 + r.level, r.clan_name);
  }
  return clanRows.get(id * 8 + level);
}

// ---------------------------------------------------------------------------------------
// Playing
// ---------------------------------------------------------------------------------------

export interface GameSummary {
  battleId: number;
  result?: string;
  rounds: number;
  decisions: { round: number; move: Move; how: string }[];
}

/** Start one Training battle, or pick up the one in progress, and play it out. */
export async function playOne(bridge: Bridge, policy: Policy, myId: number, log = console.log): Promise<GameSummary> {
  let battleId = Number((await bridge.call("battles.ongoingBattleID", {}))?.battle?.id ?? 0);
  if (battleId) {
    log(`battle ${battleId} in progress: picking it up`);
  } else {
    const room = await bridge.call("rooms.join", { id: TRAINING_ROOM });
    if (room?.room?.id !== TRAINING_ROOM) throw new Error("could not join the Training room");
    await bridge.call("battles.quickBattle", {});
    for (let i = 0; i < 15 && !battleId; i++) {
      await sleep(700);
      battleId = Number((await bridge.call("battles.ongoingBattleID", {}))?.battle?.id ?? 0);
    }
    if (!battleId) throw new Error("no Training battle started");
    log(`battle ${battleId} started`);
  }
  const summary: GameSummary = { battleId, rounds: 0, decisions: [] };

  let round = -1; // the round we ask the server about; -1 until the first snapshot
  let played = -1; // the last round we have played in
  for (;;) {
    const params: Record<string, unknown> = { id: battleId };
    if (round >= 0) params.round = round;
    const battle = (await bridge.call("battles.status", params))?.battle;
    if (!battle) throw new Error(`battle ${battleId}: no status`);
    if (battle.battleRuleId !== TRAINING_RULE) throw new Error(`battle ${battleId} is not a Training battle`);
    if (battle.status !== "playing") break;
    const mine = battle.player0.player.id === myId ? battle.player0 : battle.player1;
    // The server holds the view on the round asked for until it has resolved; then move on.
    const inRound = (p: Json) => p.characters.find((c: Json) => c.roundPlayed === battle.round);
    const resolved = [battle.player0, battle.player1].every((p) => (inRound(p)?.roundAttack ?? -1) >= 0);
    if (round < 0 || battle.round > round || resolved) {
      round = resolved ? battle.round + 1 : battle.round;
      if (resolved) continue;
    }
    if (battle.turnPlayerId === myId && played < battle.round && !inRound(mine)) {
      const deadline = (battle.roundDeadline - battle.serverTime) * 1000 + Date.now() - 12_000;
      const { move, how } = await decide(battleId, policy, deadline, mine);
      const card = mine.characters.find((c: Json) => c.index === move.index);
      if (!card) throw new Error(`battle ${battleId}: no card at index ${move.index}`);
      // The server wants the round's animation to finish first: "You cannot play so fast."
      for (let attempt = 0; ; attempt++) {
        try {
          await bridge.call("battles.play", { id: battleId, characterInBattleID: card.inBattleId, pillz: move.pillz, fury: move.fury });
          break;
        } catch (e) {
          if (attempt >= 15 || !/so fast/i.test((e as Error).message)) throw e;
          await sleep(1500);
        }
      }
      played = battle.round;
      summary.decisions.push({ round: battle.round, move, how });
      log(`  round ${battle.round + 1}: card ${move.index} (#${card.id}) pillz ${move.pillz}${move.fury ? " fury" : ""} [${how}]`);
      continue;
    }
    await sleep(POLL_MS);
  }
  const result = await bridge.call("battles.result", { id: battleId });
  summary.result = result?.battle?.result;
  summary.rounds = played + 1;
  log(`battle ${battleId}: ${summary.result}${result?.battle?.byKo ? " by KO" : ""}`);
  return summary;
}

async function decide(battleId: number, policy: Policy, deadline: number, mine: Json): Promise<{ move: Move; how: string }> {
  try {
    const rec = reconstruct(battleId, await readBattle(battleId));
    const built = buildPosition(rec);
    if (isOurs(built) && !built.provisional) {
      const chosen = await policy({ rec, game: built.game, deadline });
      if (chosen) return chosen;
    } else if (!isOurs(built)) {
      return { move: randomMove(mine), how: `random (${built.why || "no position"})` };
    }
  } catch (e) {
    return { move: randomMove(mine), how: `random (${(e as Error).message})` };
  }
  return { move: randomMove(mine), how: "random" };
}

/** Any legal move from the server's own view: an unplayed card and a bet we can pay. */
export function randomMove(mine: Json): Move {
  const cards = mine.characters.filter((c: Json) => c.roundPlayed < 0);
  const card = cards[Math.floor(Math.random() * cards.length)];
  const fury = mine.pillz >= 3 && Math.random() < 0.15;
  const pillz = Math.floor(Math.random() * ((fury ? mine.pillz - 3 : mine.pillz) + 1));
  return { index: card.index, pillz, fury };
}

/** Write the battle's game record exactly as `deno task extract` does, and scan it. */
async function recordGame(battleId: number, counts: Record<string, number>) {
  const rec = reconstruct(battleId, await readBattle(battleId));
  await Deno.writeTextFile(`${GAME_DIR}/${battleId}.json`, JSON.stringify(rec, null, 2));
  const record = rec as unknown as GameRecord;
  if (record.testcase === null || record.firstPlayer === null) return { fresh: [] as string[], mismatch: undefined, issues: rec.issues };
  const scan = scanRecord(record);
  const fresh = new Set<string>();
  for (const r of scan.rounds) {
    for (const k of r.keys) {
      if (!counts[k]) fresh.add(k);
      counts[k] = (counts[k] ?? 0) + 1;
    }
  }
  return { fresh: [...fresh].sort(), mismatch: scan.mismatch, issues: rec.issues };
}

function parseArgs(argv: string[]) {
  const opts = {
    games: 4,
    budget: 6,
    policy: "novelty",
    deck: undefined as string | undefined,
    plan: false,
    batches: 1,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--games") opts.games = Number(argv[++i]);
    else if (a === "--budget") opts.budget = Number(argv[++i]);
    else if (a === "--policy") opts.policy = argv[++i];
    else if (a === "--deck") opts.deck = argv[++i];
    else if (a === "--plan") opts.plan = true;
    else if (a === "--batches") opts.batches = Number(argv[++i]);
    else throw new Error(`unknown argument ${a}`);
  }
  if (!Number.isInteger(opts.games) || opts.games < 1) throw new Error("--games must be a positive integer");
  if (!Number.isInteger(opts.batches) || opts.batches < 1) throw new Error("--batches must be a positive integer");
  if (!["novelty", "solver", "random"].includes(opts.policy)) throw new Error("--policy is novelty, solver or random");
  if (opts.plan && opts.deck) throw new Error("--plan and --deck cannot be used together");
  return opts;
}

if (import.meta.main) {
  const opts = parseArgs(Deno.args);
  const bridge = new Bridge();
  await waitForBridge(bridge);
  const myId = Number(await Deno.readTextFile("captures/.player-id"));
  if (!myId) throw new Error("captures/.player-id is empty: play one game with the log server running first");
  const counts: Record<string, number> = JSON.parse(await Deno.readTextFile(SITUATIONS)).counts;
  let collection: Collection = JSON.parse(await Deno.readTextFile(COLLECTION));
  const policy: Policy = opts.policy === "random"
    ? () => Promise.resolve(undefined)
    : opts.policy === "solver"
    ? solverPolicy(opts.budget * 1000)
    : noveltyPolicy(counts, opts.budget * 1000);
  const used = new Set<string>();
  for (let b = 0; b < opts.batches; b++) {
    if (opts.deck || opts.plan) {
      // The owner may have sold or evolved cards since the run began: the log server rewrites
      // the collection file whenever Collection Pro loads, so read it again for every deck.
      try {
        collection = JSON.parse(await Deno.readTextFile(COLLECTION));
      } catch { /* keep the last one */ }
      let saved = false;
      for (let attempt = 0; attempt < 5 && !saved; attempt++) {
        let spec = opts.deck
          ? JSON.parse(await Deno.readTextFile(opts.deck)) as DeckSpec
          : planDeck(collection, counts, PLAN_SLOTS[b % PLAN_SLOTS.length], used);
        if (!spec && !opts.deck && used.size > 0) {
          // Every clan has had its turn this run: start another round of them, ranked on what
          // is still unfired now.
          used.clear();
          spec = planDeck(collection, counts, PLAN_SLOTS[b % PLAN_SLOTS.length], used);
        }
        if (!spec) throw new Error("the planner found no clan left to test");
        used.add(spec.name.replace(/^Auto /, ""));
        try {
          const id = await saveDeck(bridge, spec);
          console.log(`deck "${spec.name}" saved as ${id} and made current`);
          saved = true;
        } catch (e) {
          // "Impossible to find all those characters in your collection": a card the capture
          // still lists has gone (sold, evolved). Try the next clan rather than end the run.
          if (opts.deck) throw e;
          console.log(`  deck "${spec.name}" not saved (${(e as Error).message}); trying another clan`);
        }
      }
      if (!saved) throw new Error("no planned deck could be saved");
    }
    const before = Object.keys(counts).length;
    for (let g = 0; g < opts.games; g++) {
      let summary: GameSummary | undefined;
      // A lost bridge (the tab reloaded or froze) or a transient site error should not end a
      // long unattended run: wait for the tab, then pick the battle up where it stands.
      for (let failures = 0; summary === undefined; failures++) {
        try {
          summary = await playOne(bridge, policy, myId);
        } catch (e) {
          console.log(`  error: ${(e as Error).message}`);
          // An expired game-tab login fails every call until the tab is reloaded, so keep
          // trying for about an hour rather than ending the run.
          if (failures >= 120) throw e;
          await sleep(30_000);
          await waitForBridge(bridge, 30 * 60_000).catch((w) => console.log(`  ${(w as Error).message}`));
        }
      }
      const { fresh, mismatch, issues } = await recordGame(summary.battleId, counts);
      console.log(`  ${fresh.length} new situation keys${fresh.length ? ": " + fresh.slice(0, 8).join(", ") + (fresh.length > 8 ? ", ..." : "") : ""}`);
      if (mismatch !== undefined) console.log(`  ENGINE DISAGREES with the server in round ${mismatch + 1} of ${summary.battleId}`);
      if (issues.length) console.log(`  capture issues: ${issues.join("; ")}`);
    }
    console.log(`batch ${b + 1}: ${Object.keys(counts).length - before} new keys, ${Object.keys(counts).length} distinct`);
  }
}
