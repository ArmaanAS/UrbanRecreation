// Local sink for the UR logger userscript (ur-logger.user.js).
//
//   deno task log
//
// Every record is appended to ur_log.jsonl (gitignored: it contains access tokens and
// account details), minus response bodies that are binary or absurdly large - see
// elideBinary. Battle traffic is additionally split into one file per battle under
// captures/battles/<battleId>.jsonl with all secrets removed, which is the input for
// scripts/ExtractBattle.ts.
import { type BattleStatic, type CaptureEntry, expandStatus, extractFromRecord, loadAbilities, newCaptureState, type RawRecord, saveAbilities } from "./scripts/BattleCapture.ts";
import { absorbRecord, collectionAction, loadDeckStore, rawLogStandIn, saveDeckStore } from "./scripts/DeckCapture.ts";

const RAW_LOG = "ur_log.jsonl";
const CAPTURE_DIR = "captures/battles";
const PLAYER_ID = "captures/.player-id";
const SITE_CHARACTERS = "data/site_characters.jsonl";
const SITE_CLANS = "data/site_clans.json";
const USERSCRIPT = "ur-logger.user.js";
const MAX = 160;
// Only the userscript on the site itself, and local tools (which send no Origin), may talk
// to this server. It used to answer every origin with `*`, so any page open in the browser
// could post records here - overwrite data/site_clans.json, truncate
// data/site_characters.jsonl - or read the live battle feed.
const SITE_ORIGIN = "https://www.urban-rivals.com";
const cors = {
  "Access-Control-Allow-Origin": SITE_ORIGIN,
  "Access-Control-Allow-Headers": "content-type",
  "Vary": "Origin",
};

/** The `@version` of the userscript in the repository, to tell the owner when to update. */
async function repoUserscriptVersion(): Promise<string | undefined> {
  try {
    return /\/\/ @version\s+(\S+)/.exec(await Deno.readTextFile(USERSCRIPT))?.[1];
  } catch {
    return undefined;
  }
}
const clip = (s: string) =>
  s.length > MAX ? s.slice(0, MAX) + `… (+${s.length - MAX})` : s;
const colour: Record<string, string> = {
  ws_in: "\x1b[36m",
  ws_out: "\x1b[33m",
  xhr: "\x1b[35m",
  fetch: "\x1b[32m",
  api: "\x1b[1;32m",
  battle: "\x1b[1;35m",
  page: "\x1b[1;34m",
  decks: "\x1b[1;33m",
};
const DIM = "\x1b[2m", RESET = "\x1b[0m";

// Noise we still write to the raw file but don't print.
const QUIET_API = new Set([
  "teams.get",
  "general.refreshPlayer",
  "slotmachine.uniqueReward",
  "collections.decks",
  "collections.certificates",
  "missions.progressed",
]);
const QUIET_WS = /^(\[binary\]|b64:|\{"(type":"pong"|code":(0|14|18|39|42))\})/;
const QUIET_URL = /\/ajax\/news\/|\.(jpg|png|gif|webp|data|wasm|js|css)(\?|$)/;

// Response bodies that can never be read back. ur-logger.user.js >= 0.5 already leaves these
// out, but the raw log is the one thing here that grows without bound, so do not depend on
// the version of the script the browser happens to have installed: an earlier one mirrored
// WebGL asset bundles decoded as lossy UTF-8 (about a third U+FFFD, so the bytes are gone),
// and they were 65% of the first real log - 3.5 GB of 5.4 GB from 289 of 26227 lines.
// scripts/PruneLog.ts applies the same rule to logs captured before this existed.
const MAX_RESP = 4 * 1024 * 1024;

function elideBinary(body: string): string {
  // Cheap outs first: this runs on every record, and battle traffic must pass through intact.
  if (body.length < 8192 || body.includes("/api/private/v2/")) return body;
  try {
    const rec = JSON.parse(body);
    const p = rec.payload;
    if (!p || typeof p !== "object") return body;
    let hit = false;
    for (const k of ["resp", "body"]) {
      const v = p[k];
      if (typeof v !== "string" || (v.length <= MAX_RESP && !v.includes("�"))) continue;
      p[k] = `[dropped by log_server: ${v.length} chars${v.includes("�") ? " of binary" : ""}]`;
      hit = true;
    }
    return hit ? JSON.stringify(rec) : body;
  } catch {
    return body; // not JSON; the console path logs it verbatim too
  }
}

await Deno.mkdir(CAPTURE_DIR, { recursive: true });

async function loadPlayerId(): Promise<number> {
  try {
    const saved = Number(await Deno.readTextFile(PLAYER_ID));
    if (Number.isInteger(saved) && saved > 0) return saved;
  } catch { /* first run with the identity cache */ }

  // Seed an older checkout from the newest capture that still knows which player was us.
  // Battle ids rise over time, so this normally reads only the one or two newest files.
  const files: number[] = [];
  for await (const f of Deno.readDir(CAPTURE_DIR)) {
    if (f.isFile && /^\d+\.jsonl$/.test(f.name)) files.push(Number(f.name.slice(0, -6)));
  }
  files.sort((a, b) => b - a);
  for (const id of files) {
    try {
      const first = (await Deno.readTextFile(`${CAPTURE_DIR}/${id}.jsonl`)).split("\n", 1)[0];
      const meta = JSON.parse(first);
      if (meta.kind === "meta" && Number.isInteger(meta.myId) && meta.myId > 0) {
        await Deno.writeTextFile(PLAYER_ID, String(meta.myId));
        return meta.myId;
      }
    } catch { /* incomplete or legacy capture; try the previous one */ }
  }
  return 0;
}

let n = 0;
// Collection Pro's collection, formats and decks, kept in the deck builder's data files as
// the page loads them (scripts/DeckCapture.ts).
const deckStore = await loadDeckStore();
const state = newCaptureState(await loadAbilities());
state.myId = await loadPlayerId();
const statics = new Map<number, BattleStatic>();

// Live feed for src/solver/Advisor.ts: the same capture entries that go to disk, with
// snapshots already expanded so a subscriber needs no ability dictionary to read them.
// Kept deliberately thin - this process must keep up with the game client's polling, so
// the solver runs in its own process and merely listens.
const feeds = new Set<(chunk: string) => void>();
// Opt-in only and intentionally process-local: it survives consecutive games, but a logger
// restart returns automation to the safe OFF state. The browser userscript polls this.
let autoQueue = false;

function broadcast(battleId: number, entry: CaptureEntry) {
  if (feeds.size === 0) return;
  const chunk = `data: ${JSON.stringify({ battleId, entry })}\n\n`;
  for (const send of feeds) {
    try {
      send(chunk);
    } catch {
      // Subscriber went away mid-write; its cancel handler removes it.
    }
  }
}

function feed(): Response {
  let send: (chunk: string) => void;
  const body = new ReadableStream<Uint8Array>({
    start(controller) {
      const enc = new TextEncoder();
      send = (chunk) => controller.enqueue(enc.encode(chunk));
      feeds.add(send);
      send(": connected\n\n");
      console.log(`solver feed attached (${feeds.size} listening)`);
    },
    cancel() {
      feeds.delete(send);
      console.log(`solver feed detached (${feeds.size} listening)`);
    },
  });
  return new Response(body, {
    headers: { ...cors, "content-type": "text/event-stream", "cache-control": "no-cache" },
  });
}

const battleLine = (b: any) => {
  const side = (p: any) => {
    const played = p.characters
      .filter((c: any) => c.roundPlayed >= 0)
      .sort((a: any, b: any) => a.roundPlayed - b.roundPlayed)
      .map((c: any) =>
        `#${c.id}${c.isFury ? "F" : ""}x${c.pillzUsed}` +
        (c.roundAttack >= 0 ? `=${c.roundAttack}${c.roundWon ? "W" : "L"}` : "")
      )
      .join(" ");
    return `${p.player.name} ${p.life}hp ${p.pillz}pz [${played}]`;
  };
  return `battle ${b.id} r${b.round} ${b.status} turn=${b.turnPlayerId}  ${
    side(b.player0)
  }  vs  ${side(b.player1)}`;
};

function print(label: string, t: number, summary: string) {
  const time = new Date(t).toLocaleTimeString("en-GB", { hour12: false }) +
    "." + String(t % 1000).padStart(3, "0");
  console.log(
    `${String(++n).padStart(5)}  ${time}  ${colour[label] ?? ""}${
      label.padEnd(6)
    }${RESET}  ${summary}`,
  );
}

const DECK_SERVICE = "http://127.0.0.1:8788";
async function proxyDecks(r: Request, path: string): Promise<Response> {
  // The panel's POST is preflighted; answer that here rather than forwarding it.
  if (r.method === "OPTIONS") return new Response(null, { status: 204, headers: cors });
  try {
    const res = await fetch(DECK_SERVICE + path, {
      method: r.method,
      headers: { "content-type": "application/json" },
      body: r.method === "POST" ? await r.text() : undefined,
    });
    return new Response(await res.text(), {
      status: res.status,
      headers: { ...cors, "content-type": "application/json" },
    });
  } catch {
    return Response.json({ error: "deck service not running: `deno task decks`" }, { status: 503, headers: cors });
  }
}

// Requests arrive concurrently (the game client fires several polls at once); process them
// strictly in arrival order so appended lines and capture state stay consistent.
let queue: Promise<unknown> = Promise.resolve();

Deno.serve({ port: 8787, onListen: ({ port }) => console.log(`UR log server on :${port} → ${RAW_LOG}, ${CAPTURE_DIR}/`) }, async (r) => {
  const path = new URL(r.url).pathname;
  // The userscript is repository content, so anything may fetch it: opening this URL in the
  // browser offers Tampermonkey's install/update page, and its @updateURL points here.
  if (r.method === "GET" && path === "/" + USERSCRIPT) {
    return new Response(await Deno.readTextFile(USERSCRIPT), {
      headers: { "content-type": "text/javascript; charset=utf-8", "cache-control": "no-store" },
    });
  }
  const origin = r.headers.get("origin");
  if (origin !== null && origin !== SITE_ORIGIN) return new Response(null, { status: 403 });
  if (r.method === "GET" && path === "/events") return feed();
  // The Collection Pro panel reaches the deck service through here, so the browser only
  // ever needs to reach this one local port: Edge asks the owner separately for each
  // local address a site may call, and a pending prompt stalls the page.
  if (path.startsWith("/decks/")) return proxyDecks(r, path.slice("/decks".length));
  if (path === "/control") {
    if (r.method === "GET") {
      return Response.json({ autoQueue }, { headers: { ...cors, "cache-control": "no-store" } });
    }
    if (r.method === "POST") {
      // Only the local Deno advisor may mutate control state. A web page supplies Origin,
      // even for a no-cors form POST, so it cannot silently switch automation on.
      if (r.headers.has("origin")) return new Response(null, { status: 403, headers: cors });
      try {
        const requested = (await r.json()).autoQueue;
        if (typeof requested !== "boolean") throw new Error("autoQueue must be boolean");
        autoQueue = requested;
        console.log(`auto-queue ${autoQueue ? "enabled" : "disabled"} by advisor`);
        return Response.json({ autoQueue }, { headers: cors });
      } catch (e) {
        return Response.json({ error: (e as Error).message }, { status: 400, headers: cors });
      }
    }
    return new Response(null, { status: 405, headers: cors });
  }
  if (r.method !== "POST") return new Response(null, { status: 204, headers: cors });
  const body = await r.text();
  const run = queue.then(() => handle(body));
  queue = run.catch(() => {});
  return run;
});

async function handle(body: string): Promise<Response> {
  let rec: RawRecord;
  try {
    rec = JSON.parse(body);
  } catch {
    await Deno.writeTextFile(RAW_LOG, elideBinary(body) + "\n", { append: true });
    print("raw", Date.now(), clip(body));
    return new Response(null, { status: 204, headers: cors });
  }
  // A Collection Pro visit loads the whole catalog, 2.7 MB a page; the deck files keep what
  // matters, so the raw log gets a one-line stand-in instead of 13 MB per visit.
  const standIn = rawLogStandIn(rec);
  const logged = standIn ? JSON.stringify({ ...rec, payload: { ...rec.payload, resp: standIn } }) : elideBinary(body);
  await Deno.writeTextFile(RAW_LOG, logged + "\n", { append: true });

  try {
    if (absorbRecord(rec, deckStore)) {
      const written = await saveDeckStore(deckStore);
      const what = collectionAction(rec)?.action ?? "collections.decks";
      print("decks", rec.t, `${what} → ${written.join(", ") || "no change"}`);
      return new Response(null, { status: 204, headers: cors });
    }
    // Card database dump triggered from the browser via __ur.dumpCharacters()
    if (rec.kind === "characters") {
      const { page, since, count, hasNextPage, characters, raw } = rec.payload;
      if (count > 0) {
        await Deno.writeTextFile(SITE_CHARACTERS, characters.map((c: unknown) => JSON.stringify(c)).join("\n") + "\n", { append: page > 0 || since > 0 });
      }
      print("cards", rec.t, `characters.get page ${page} since=${since}: ${count} rows${hasNextPage ? ", more…" : ", done"} → ${SITE_CHARACTERS}` + (raw ? `  ${DIM}${clip(JSON.stringify(raw))}${RESET}` : ""));
      return new Response(null, { status: 204, headers: cors });
    }

    if (rec.kind === "clans") {
      const { count, clans, raw } = rec.payload;
      if (count > 0) await Deno.writeTextFile(SITE_CLANS, JSON.stringify(clans, null, 1));
      print("cards", rec.t, `clans.get: ${count} clans → ${SITE_CLANS}` + (raw ? `  ${DIM}${clip(JSON.stringify(raw))}${RESET}` : ""));
      return new Response(null, { status: 204, headers: cors });
    }

    // Battle capture (secret-free, compact, one file per battle)
    const previousMyId = state.myId;
    const events = extractFromRecord(rec, state);
    if (state.myId > 0 && state.myId !== previousMyId) {
      await Deno.writeTextFile(PLAYER_ID, String(state.myId));
    }
    if (state.abilitiesDirty) {
      state.abilitiesDirty = false;
      await saveAbilities(state.abilities);
    }
    for (const ev of events) {
      await Deno.writeTextFile(
        `${CAPTURE_DIR}/${ev.battleId}.jsonl`,
        JSON.stringify(ev.entry) + "\n",
        { append: true },
      );
      const e = ev.entry;
      if (e.kind === "static") statics.set(ev.battleId, e.s);
      else if (e.kind === "s") {
        const s = statics.get(ev.battleId);
        if (s) {
          const battle = expandStatus(s, e.d, state.abilities);
          print("battle", rec.t, battleLine(battle));
          broadcast(ev.battleId, { kind: "status", t: e.t, battle });
        }
      } else if (e.kind === "result") {
        print("battle", rec.t, `battle ${ev.battleId} RESULT ${e.result.result} byKo=${e.result.byKo} score=${e.result.score}`);
        broadcast(ev.battleId, e);
      } else {
        print("battle", rec.t, `battle ${ev.battleId} ${e.kind} ${DIM}${clip(JSON.stringify(e))}${RESET}`);
        broadcast(ev.battleId, e);
      }
    }
    if (events.length) return new Response(null, { status: 204, headers: cors });

    // Everything else: compact console line
    const { t, kind, payload } = rec;
    if (typeof payload === "string") {
      if (!QUIET_WS.test(payload)) print(kind, t, clip(payload));
    } else if (kind === "page") {
      print(kind, t, payload.href);
      const current = await repoUserscriptVersion();
      if (current !== undefined && payload.version !== current) {
        print("page", t, `\x1b[33muserscript ${payload.version ?? "0.7.x or older"} is not the repository's ${current}: open http://localhost:8787/${USERSCRIPT} in the browser to update it${RESET}`);
      }
    } else if ((payload.u ?? "").includes("/api/private/v2/")) {
      let method = "?", data: unknown = null;
      try {
        const j = JSON.parse(payload.resp);
        method = Object.keys(j)[0];
        data = j[method]?.data ?? j[method];
      } catch { /* non-JSON */ }
      if (!QUIET_API.has(method)) {
        const req = payload.body ? "req=" + clip(String(payload.body)) + "  " : "";
        print("api", t, method.padEnd(28) + `${DIM}${req}resp=${clip(JSON.stringify(data))}${RESET}`);
      }
    } else if (!QUIET_URL.test(payload.u ?? "")) {
      print(kind, t, `${payload.m ?? "GET"} ${payload.u}  → ${payload.status ?? ""}  ${DIM}${clip(String(payload.resp ?? "")).replace(/\s+/g, " ")}${RESET}`);
    }
  } catch (e) {
    print("err", rec.t, `${(e as Error).message}  ${DIM}${clip(body)}${RESET}`);
  }
  return new Response(null, { status: 204, headers: cors });
}
