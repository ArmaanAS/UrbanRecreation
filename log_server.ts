// Local sink for the UR logger userscript (ur-logger.user.js).
//
//   deno task log
//
// Every record is appended verbatim to ur_log.jsonl (gitignored: it contains access
// tokens and account details). Battle traffic is additionally split into one file per
// battle under captures/battles/<battleId>.jsonl with all secrets removed, which is the
// input for scripts/ExtractBattle.ts.
import { type BattleStatic, expandStatus, extractFromRecord, loadAbilities, newCaptureState, type RawRecord, saveAbilities } from "./scripts/BattleCapture.ts";

const RAW_LOG = "ur_log.jsonl";
const CAPTURE_DIR = "captures/battles";
const SITE_CHARACTERS = "data/site_characters.jsonl";
const SITE_CLANS = "data/site_clans.json";
const MAX = 160;
const cors = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "*",
};
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

await Deno.mkdir(CAPTURE_DIR, { recursive: true });

let n = 0;
const state = newCaptureState(await loadAbilities());
const statics = new Map<number, BattleStatic>();

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

Deno.serve({ port: 8787, onListen: ({ port }) => console.log(`UR log server on :${port} → ${RAW_LOG}, ${CAPTURE_DIR}/`) }, async (r) => {
  if (r.method !== "POST") return new Response(null, { status: 204, headers: cors });

  const body = await r.text();
  await Deno.writeTextFile(RAW_LOG, body + "\n", { append: true });

  let rec: RawRecord;
  try {
    rec = JSON.parse(body);
  } catch {
    print("raw", Date.now(), clip(body));
    return new Response(null, { status: 204, headers: cors });
  }

  try {
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
    const events = extractFromRecord(rec, state);
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
        if (s) print("battle", rec.t, battleLine(expandStatus(s, e.d, state.abilities)));
      } else if (e.kind === "result") print("battle", rec.t, `battle ${ev.battleId} RESULT ${e.result.result} byKo=${e.result.byKo} score=${e.result.score}`);
      else print("battle", rec.t, `battle ${ev.battleId} ${e.kind} ${DIM}${clip(JSON.stringify(e))}${RESET}`);
    }
    if (events.length) return new Response(null, { status: 204, headers: cors });

    // Everything else: compact console line
    const { t, kind, payload } = rec;
    if (typeof payload === "string") {
      if (!QUIET_WS.test(payload)) print(kind, t, clip(payload));
    } else if (kind === "page") {
      print(kind, t, payload.href);
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
});
