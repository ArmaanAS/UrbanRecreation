// Shared logic for turning raw userscript records into secret-free, compact battle capture
// entries. Used live by log_server.ts and offline by scripts/ExtractBattle.ts.
//
// Storage format (captures/battles/<id>.jsonl), one JSON object per line:
//   { kind: "meta",   t, myId, room }
//   { kind: "static", t, s: BattleStatic }        — everything that never changes during a
//                                                   battle: players and their hands. Written
//                                                   once (again only if it ever differs).
//   { kind: "s",      t, d: BattleDynamic }       — one battles.status snapshot, dynamic
//                                                   fields only (life, pillz, per-card round
//                                                   state). ~300 bytes instead of ~15 KB.
//   { kind: "hover",  t, side, index, active }     — opponent mouse entered/left a card.
//   { kind: "selecting", t, side, index, active }  — opponent opened/closed its pillz UI.
//   { kind: "play",   t, request, response }
//   { kind: "result", t, result, ranking? }
// Ability / bonus definitions (id → description + abilityData) live in a single shared
// dictionary, captures/abilities.json, referenced by id from BattleStatic.
// `expandStatus()` rebuilds the original battles.status `battle` object losslessly.

export interface RawRecord {
  t: number;
  kind: "fetch" | "xhr" | "ws_open" | "ws_in" | "ws_out" | "page" | string;
  // deno-lint-ignore no-explicit-any
  payload: any;
}

export interface CaptureState {
  myId: number;
  room: unknown;
  lastBattleId: number;
  /** JSON of the last static block written per battle, to detect changes. */
  lastStatic?: string;
  /** Shared ability dictionary (mutated: new ids are added as they are seen). */
  abilities: AbilityDict;
  /** Set to true whenever `abilities` gained an entry (caller persists and resets). */
  abilitiesDirty?: boolean;
  /** Whether the most recent battle snapshot can still receive meaningful hover events. */
  battleActive: boolean;
  /** Active absolute card slots, used to collapse duplicate WebSocket frames. */
  hoveredSlots: Set<number>;
  /** Absolute card slot whose pillz chooser the remote player currently has open. */
  selectingSlot?: number;
}

export interface AbilityDef {
  id: number;
  unlockLevel: number;
  description: string;
  longDescription?: string;
  abilityData?: unknown;
}
export type AbilityDict = Record<string, AbilityDef>;

export interface StaticCharacter {
  id: number;
  level: number;
  index: number;
  inBattleId: number;
  ability: number | null;
  bonus: number | null;
  /** longDescription overrides when the card's text differs from the shared definition (it embeds the card name). */
  abilityLong?: string;
  bonusLong?: string;
}
export interface StaticPlayer {
  player: Record<string, unknown>;
  baseLife: number;
  basePillz: number;
  characters: StaticCharacter[];
}
export interface BattleStatic {
  id: number;
  creationTime: number;
  battleRuleId: number;
  teamBattleManagerId: number;
  players: [StaticPlayer, StaticPlayer];
}

/** Per-character dynamic fields, positional to keep snapshots tiny. */
export type DynCharacter = [
  roundPlayed: number,
  pillzUsed: number,
  isFury: number | boolean,
  roundWon: boolean | null,
  roundPower: number,
  roundDamage: number,
  roundAttack: number,
  state: string,
  position: string,
  /** Any character field not covered above (e.g. infiltratedClanId), present only when non-empty. */
  extra?: Record<string, unknown>,
];
export interface DynPlayer {
  life: number;
  pillz: number;
  c: DynCharacter[];
  pre?: unknown[];
  post?: unknown[];
  tbe?: unknown[];
  extra?: Record<string, unknown>;
}
export interface BattleDynamic {
  status: string;
  round: number;
  roundTotalTime: number;
  roundElapsedTime: number;
  turnPlayerId: number;
  p: [DynPlayer, DynPlayer];
  extra?: Record<string, unknown>;
}

const CHAR_KEYS = new Set([
  "id",
  "level",
  "index",
  "inBattleId",
  "ability",
  "bonus",
  "roundPlayed",
  "pillzUsed",
  "isFury",
  "roundWon",
  "roundPower",
  "roundDamage",
  "roundAttack",
  "state",
  "position",
]);
const PLAYER_KEYS = new Set([
  "player",
  "baseLife",
  "basePillz",
  "characters",
  "life",
  "pillz",
  "preRoundAbilities",
  "postRoundAbilities",
  "teamBattleEffects",
]);
const BATTLE_KEYS = new Set([
  "id",
  "creationTime",
  "battleRuleId",
  "teamBattleManagerId",
  "status",
  "round",
  "roundTotalTime",
  "roundElapsedTime",
  "turnPlayerId",
  "player0",
  "player1",
]);
function extras(
  obj: Record<string, unknown>,
  known: Set<string>,
): Record<string, unknown> | undefined {
  let out: Record<string, unknown> | undefined;
  for (const k of Object.keys(obj)) {
    if (!known.has(k)) (out ??= {})[k] = obj[k];
  }
  return out;
}

export type CaptureEntry =
  | { kind: "meta"; t: number; myId: number; room: unknown }
  | { kind: "static"; t: number; s: BattleStatic }
  | { kind: "s"; t: number; d: BattleDynamic }
  // Legacy (pre-compaction) full snapshot; still accepted by expandEntries().
  // deno-lint-ignore no-explicit-any
  | { kind: "status"; t: number; battle: any }
  /** A remote hover over an absolute server-side hand slot (not a committed selection). */
  | { kind: "hover"; t: number; side: 0 | 1; index: number; active: boolean }
  /** A remote card whose pillz chooser is open; code 7 enters and code 8 leaves. */
  | {
    kind: "selecting";
    t: number;
    side: 0 | 1;
    index: number;
    active: boolean;
  }
  | { kind: "play"; t: number; request: unknown; response: unknown }
  // deno-lint-ignore no-explicit-any
  | { kind: "result"; t: number; result: any; ranking?: unknown };

export interface CaptureEvent {
  battleId: number;
  entry: CaptureEntry;
}

export function newCaptureState(abilities: AbilityDict = {}): CaptureState {
  return {
    myId: 0,
    room: undefined,
    lastBattleId: 0,
    abilities,
    battleActive: false,
    hoveredSlots: new Set(),
  };
}

// ---------------------------------------------------------------------------------------
// Redaction / parsing helpers
// ---------------------------------------------------------------------------------------
const SECRET_KEY = /token|password|secret|email|session|auth/i;

/** Recursively drop any object key that looks like a credential. */
export function redact<T>(value: T): T {
  if (Array.isArray(value)) return value.map(redact) as T;
  if (value && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
      if (SECRET_KEY.test(k)) continue;
      out[k] = redact(v);
    }
    return out as T;
  }
  return value;
}

/** Decode a userscript body string ('b64:' prefix = raw bytes) into JSON if possible. */
export function parseBody(body: unknown): unknown {
  if (typeof body !== "string") return undefined;
  let text = body;
  if (text.startsWith("b64:")) {
    try {
      text = new TextDecoder().decode(
        Uint8Array.from(atob(text.slice(4)), (c) => c.charCodeAt(0)),
      );
    } catch {
      return { raw: body };
    }
  }
  // The site posts form-encoded `requests=<urlencoded JSON array of {call, params}>`.
  if (/^requests=/.test(text)) {
    try {
      return {
        requests: JSON.parse(
          decodeURIComponent(
            text.slice("requests=".length).replace(/\+/g, " "),
          ),
        ),
      };
    } catch { /* fall through */ }
  }
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

/** Parse a private-API response into [method, payload] or undefined. */
export function parseApi(
  rec: RawRecord,
): { method: string; data: unknown; body: unknown } | undefined {
  const p = rec.payload;
  if ((rec.kind !== "fetch" && rec.kind !== "xhr") || typeof p !== "object") {
    return;
  }
  if (!String(p.u ?? "").includes("/api/private/v2/")) return;
  try {
    const j = JSON.parse(p.resp);
    const method = Object.keys(j)[0];
    return { method, data: j[method], body: parseBody(p.body) };
  } catch {
    return;
  }
}

// deno-lint-ignore no-explicit-any
function trimPlayer(player: any) {
  if (!player) return player;
  const { pictureUrl: _p, stuckStickersData: _s, ...rest } = player;
  return rest;
}

// ---------------------------------------------------------------------------------------
// Compaction: full battles.status → static + dynamic
// ---------------------------------------------------------------------------------------
// deno-lint-ignore no-explicit-any
function registerAbility(
  dict: AbilityDict,
  a: any,
  state: CaptureState,
): { id: number | null; long?: string } {
  if (!a || typeof a.id !== "number") return { id: null };
  const key = String(a.id);
  if (!dict[key]) {
    dict[key] = {
      id: a.id,
      unlockLevel: a.unlockLevel,
      description: a.description,
      longDescription: a.longDescription,
      abilityData: a.abilityData,
    };
    state.abilitiesDirty = true;
  }
  const long = a.longDescription !== dict[key].longDescription
    ? a.longDescription
    : undefined;
  return { id: a.id, long };
}

// deno-lint-ignore no-explicit-any
export function splitStatus(
  battle: any,
  state: CaptureState,
): { s: BattleStatic; d: BattleDynamic } {
  const sides = [battle.player0, battle.player1];
  const s: BattleStatic = {
    id: battle.id,
    creationTime: battle.creationTime,
    battleRuleId: battle.battleRuleId,
    teamBattleManagerId: battle.teamBattleManagerId,
    players: sides.map((p) => ({
      player: trimPlayer(p.player),
      baseLife: p.baseLife,
      basePillz: p.basePillz,
      // deno-lint-ignore no-explicit-any
      characters: [...p.characters].sort((a: any, b: any) => a.index - b.index)
        .map((c: any) => {
          const ab = registerAbility(state.abilities, c.ability, state);
          const bo = registerAbility(state.abilities, c.bonus, state);
          const sc: StaticCharacter = {
            id: c.id,
            level: c.level,
            index: c.index,
            inBattleId: c.inBattleId,
            ability: ab.id,
            bonus: bo.id,
          };
          if (ab.long !== undefined) sc.abilityLong = ab.long;
          if (bo.long !== undefined) sc.bonusLong = bo.long;
          return sc;
        }),
    })) as [StaticPlayer, StaticPlayer],
  };
  const d: BattleDynamic = {
    status: battle.status,
    round: battle.round,
    roundTotalTime: battle.roundTotalTime,
    roundElapsedTime: battle.roundElapsedTime,
    turnPlayerId: battle.turnPlayerId,
    p: sides.map((p) => {
      const dp: DynPlayer = {
        life: p.life,
        pillz: p.pillz,
        // deno-lint-ignore no-explicit-any
        c: [...p.characters].sort((a: any, b: any) => a.index - b.index).map(
          (c: any) => {
            const dc = [
              c.roundPlayed,
              c.pillzUsed,
              c.isFury,
              c.roundWon,
              c.roundPower,
              c.roundDamage,
              c.roundAttack,
              c.state,
              c.position,
            ] as DynCharacter;
            const x = extras(c, CHAR_KEYS);
            if (x) dc.push(x);
            return dc;
          },
        ),
      };
      if (p.preRoundAbilities?.length) dp.pre = p.preRoundAbilities;
      if (p.postRoundAbilities?.length) dp.post = p.postRoundAbilities;
      if (p.teamBattleEffects?.length) dp.tbe = p.teamBattleEffects;
      const px = extras(p, PLAYER_KEYS);
      if (px) dp.extra = px;
      return dp;
    }) as [DynPlayer, DynPlayer],
  };
  const bx = extras(battle, BATTLE_KEYS);
  if (bx) d.extra = bx;
  return { s, d };
}

/** Inverse of splitStatus: rebuild the original battles.status `battle` object. */
// deno-lint-ignore no-explicit-any
export function expandStatus(
  s: BattleStatic,
  d: BattleDynamic,
  abilities: AbilityDict,
): any {
  const side = (i: 0 | 1) => {
    const sp = s.players[i], dp = d.p[i];
    return {
      player: sp.player,
      life: dp.life,
      pillz: dp.pillz,
      baseLife: sp.baseLife,
      basePillz: sp.basePillz,
      characters: sp.characters.map((c, j) => {
        const [
          roundPlayed,
          pillzUsed,
          isFury,
          roundWon,
          roundPower,
          roundDamage,
          roundAttack,
          state,
          position,
          extra,
        ] = dp.c[j];
        // deno-lint-ignore no-explicit-any
        const out: any = {
          id: c.id,
          level: c.level,
          state,
          roundPlayed,
          inBattleId: c.inBattleId,
          roundWon,
          pillzUsed,
          roundPower,
          roundDamage,
          roundAttack,
          isFury,
          position,
          index: c.index,
          ...(extra ?? {}),
        };
        if (c.ability !== null) {
          out.ability = c.abilityLong !== undefined
            ? {
              ...abilities[String(c.ability)],
              longDescription: c.abilityLong,
            }
            : abilities[String(c.ability)];
        }
        if (c.bonus !== null) {
          out.bonus = c.bonusLong !== undefined
            ? { ...abilities[String(c.bonus)], longDescription: c.bonusLong }
            : abilities[String(c.bonus)];
        }
        return out;
      }),
      postRoundAbilities: dp.post ?? [],
      preRoundAbilities: dp.pre ?? [],
      teamBattleEffects: dp.tbe ?? [],
      ...(dp.extra ?? {}),
    };
  };
  return {
    id: s.id,
    status: d.status,
    creationTime: s.creationTime,
    battleRuleId: s.battleRuleId,
    round: d.round,
    roundTotalTime: d.roundTotalTime,
    roundElapsedTime: d.roundElapsedTime,
    turnPlayerId: d.turnPlayerId,
    teamBattleManagerId: s.teamBattleManagerId,
    player0: side(0),
    player1: side(1),
    ...(d.extra ?? {}),
  };
}

/**
 * Normalise a battle file's entries to full `status` snapshots (expanding compact ones),
 * so consumers can treat old and new files alike.
 */
export function expandEntries(
  entries: CaptureEntry[],
  abilities: AbilityDict,
): CaptureEntry[] {
  // Lines may be slightly out of order (concurrent polls appended by the log server), so a
  // snapshot can precede its static block: fall back to the first static block in the file.
  const firstStatic = entries.find((
    e,
  ): e is Extract<CaptureEntry, { kind: "static" }> => e.kind === "static")?.s;
  let s: BattleStatic | undefined = firstStatic;
  const out: CaptureEntry[] = [];
  for (const e of entries) {
    if (e.kind === "static") s = e.s;
    else if (e.kind === "s") {
      if (!s) throw new Error("compact snapshot without any static block");
      out.push({
        kind: "status",
        t: e.t,
        battle: expandStatus(s, e.d, abilities),
      });
    } else out.push(e);
  }
  return out;
}

/** Convert a legacy (full-snapshot) battle file into the compact format. */
export function compactEntries(
  entries: CaptureEntry[],
  state: CaptureState,
): CaptureEntry[] {
  const out: CaptureEntry[] = [];
  let lastStatic: string | undefined;
  for (const e of entries) {
    if (e.kind !== "status") {
      out.push(e);
      continue;
    }
    const { s, d } = splitStatus(e.battle, state);
    const sj = JSON.stringify(s);
    if (sj !== lastStatic) {
      out.push({ kind: "static", t: e.t, s });
      lastStatic = sj;
    }
    out.push({ kind: "s", t: e.t, d });
  }
  return out;
}

// ---------------------------------------------------------------------------------------
// Live: raw userscript record → capture events
// ---------------------------------------------------------------------------------------
/**
 * Feed one raw record; returns zero or more capture events. `state` persists across
 * calls so battle-less responses (battles.play, battles.result) can be attributed.
 */
export function extractFromRecord(
  rec: RawRecord,
  state: CaptureState,
): CaptureEvent[] {
  // The battle socket sends 5/6 for hover enter/leave and 7/8 for opening/closing the
  // card's pillz chooser. Values 1..4 are player0's hand and 5..8 are player1's. Hover
  // frames are commonly delivered twice, so retain active state and emit transitions only.
  if (rec.kind === "ws_in" && state.lastBattleId && state.battleActive) {
    try {
      const message = typeof rec.payload === "string"
        ? JSON.parse(rec.payload)
        : rec.payload;
      const code = message?.code;
      const kind = code === 5 || code === 6
        ? "hover"
        : code === 7 || code === 8
        ? "selecting"
        : undefined;
      const active = code === 5 || code === 7
        ? true
        : code === 6 || code === 8
        ? false
        : undefined;
      const value = Number(message?.values?.[0]);
      if (
        kind === undefined || active === undefined ||
        !Number.isInteger(value) || value < 1 ||
        value > 8
      ) {
        return [];
      }
      const slot = value - 1;
      if (kind === "selecting") {
        if (active && state.selectingSlot === slot) return [];
        if (!active && state.selectingSlot !== slot) return [];

        const events: CaptureEvent[] = [];
        if (active && state.selectingSlot !== undefined) {
          const previous = state.selectingSlot;
          events.push({
            battleId: state.lastBattleId,
            entry: {
              kind: "selecting",
              t: rec.t,
              side: previous < 4 ? 0 : 1,
              index: previous % 4,
              active: false,
            },
          });
        }
        state.selectingSlot = active ? slot : undefined;
        events.push({
          battleId: state.lastBattleId,
          entry: {
            kind: "selecting",
            t: rec.t,
            side: slot < 4 ? 0 : 1,
            index: slot % 4,
            active,
          },
        });
        return events;
      }

      if (
        active && state.hoveredSlots.size === 1 && state.hoveredSlots.has(slot)
      ) {
        return [];
      }
      if (!active && !state.hoveredSlots.has(slot)) return [];

      const events: CaptureEvent[] = [];
      if (active) {
        // ws_in is one remote mouse, so it cannot genuinely hover two cards. If a leave
        // frame was dropped or reordered, close the stale slot before entering the new one.
        for (const previous of state.hoveredSlots) {
          if (previous === slot) continue;
          events.push({
            battleId: state.lastBattleId,
            entry: {
              kind: "hover",
              t: rec.t,
              side: previous < 4 ? 0 : 1,
              index: previous % 4,
              active: false,
            },
          });
        }
        state.hoveredSlots.clear();
        state.hoveredSlots.add(slot);
      } else {
        state.hoveredSlots.delete(slot);
      }
      events.push({
        battleId: state.lastBattleId,
        entry: {
          kind: "hover",
          t: rec.t,
          side: slot < 4 ? 0 : 1,
          index: slot % 4,
          active,
        },
      });
      return events;
    } catch {
      return [];
    }
  }

  const api = parseApi(rec);
  if (!api) return [];
  // deno-lint-ignore no-explicit-any
  const data = (api.data as any)?.data;
  const t = rec.t;

  switch (api.method) {
    case "general.initPlayer":
      if (data?.player?.id) state.myId = data.player.id;
      return [];
    case "rooms.join":
      if (data?.room) {
        const { id, name, idBattleRule, idDeckFormat } = data.room;
        state.room = { id, name, idBattleRule, idDeckFormat };
      }
      return [];
    case "battles.status": {
      const battle = data?.battle;
      if (!battle?.id) return [];
      const events: CaptureEvent[] = [];
      if (battle.id !== state.lastBattleId) {
        state.lastBattleId = battle.id;
        state.lastStatic = undefined;
        state.hoveredSlots.clear();
        state.selectingSlot = undefined;
        events.push({
          battleId: battle.id,
          entry: { kind: "meta", t, myId: state.myId, room: state.room },
        });
      }
      state.battleActive = battle.status === "playing";
      if (!state.battleActive) {
        state.hoveredSlots.clear();
        state.selectingSlot = undefined;
      }
      const { s, d } = splitStatus(battle, state);
      const sj = JSON.stringify(s);
      if (sj !== state.lastStatic) {
        state.lastStatic = sj;
        events.push({ battleId: battle.id, entry: { kind: "static", t, s } });
      }
      events.push({ battleId: battle.id, entry: { kind: "s", t, d } });
      return events;
    }
    case "battles.play":
      if (!state.lastBattleId) return [];
      return [{
        battleId: state.lastBattleId,
        entry: {
          kind: "play",
          t,
          request: redact(api.body),
          response: redact(data),
        },
      }];
    case "battles.result": {
      if (!state.lastBattleId || !data?.battle) return [];
      state.battleActive = false;
      state.hoveredSlots.clear();
      state.selectingSlot = undefined;
      const ranking = Array.isArray(data.ranking)
        ? data.ranking.map((r: { player?: Record<string, unknown> }) => ({
          ...r,
          player: trimPlayer(r.player),
        }))
        : undefined;
      return [{
        battleId: state.lastBattleId,
        entry: { kind: "result", t, result: data.battle, ranking },
      }];
    }
  }
  return [];
}

// ---------------------------------------------------------------------------------------
// Ability dictionary persistence
// ---------------------------------------------------------------------------------------
export const ABILITIES_PATH = "captures/abilities.json";

export async function loadAbilities(
  path = ABILITIES_PATH,
): Promise<AbilityDict> {
  try {
    return JSON.parse(await Deno.readTextFile(path));
  } catch {
    return {};
  }
}

export async function saveAbilities(dict: AbilityDict, path = ABILITIES_PATH) {
  const sorted = Object.fromEntries(
    Object.keys(dict).map(Number).sort((a, b) => a - b).map((
      k,
    ) => [String(k), dict[String(k)]]),
  );
  await Deno.writeTextFile(path, JSON.stringify(sorted, null, 1));
}
