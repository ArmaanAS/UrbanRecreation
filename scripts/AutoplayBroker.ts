// The broker between a local autoplay driver (scripts/AutoPlay.ts) and the userscript in the
// game tab. The owner asked for automated Training play on 2026-09-27 ("I'm fully happy for
// you to set my decks automatically and PLAY THE GAME"), against the Training AI only.
//
// The driver asks log_server.ts for one call at a time (`POST /autoplay/call`); the userscript
// long-polls `GET /autoplay/next`, runs the call with the game client's own session and
// answers `POST /autoplay/result`. Nothing reaches the site unless a local tool queued it, and
// the rules below, not anything the driver sends, decide what may go:
// - private-API calls on AUTOPLAY_CALLS only;
// - `rooms.join` to the Training room only, and `battles.quickBattle` only straight after a
//   successful join there, so matchmaking can never start in a PvP room;
// - `battles.play` and `battles.quit` only on a battle whose status this broker has seen
//   with the Training battle rule;
// - the two deck actions Collection Pro itself uses, loaddeck and savedeck.

/** Room 6, battle rule 2: "Training / Solo PVE / Free deck, 8+ cards" (docs/site-api.md 3b). */
export const TRAINING_ROOM = 6;
export const TRAINING_RULE = 2;

export const AUTOPLAY_CALLS = [
  "rooms.join",
  "battles.quickBattle",
  "battles.stopQuickBattle",
  "battles.ongoingBattleID",
  "battles.status",
  "battles.play",
  "battles.quit",
  "battles.result",
  "collections.decks",
  "general.refreshPlayer",
] as const;
export const AUTOPLAY_DECK_ACTIONS = ["loaddeck", "savedeck"] as const;

export type AutoplayCommand =
  | { id: number; call: string; params: Record<string, unknown> }
  | { id: number; deck: string; fields: [string, string][] };
export type AutoplayOutcome = { ok: true; result: unknown } | { ok: false; error: string };
type Request = Omit<AutoplayCommand, "id">;

// deno-lint-ignore no-explicit-any
type Json = any;

const isObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);

export class AutoplayBroker {
  #queue: AutoplayCommand[] = [];
  #pending = new Map<number, { command: AutoplayCommand; resolve: (o: AutoplayOutcome) => void; timer: ReturnType<typeof setTimeout> }>();
  #poller?: { resolve: (c: AutoplayCommand | null) => void; timer: ReturnType<typeof setTimeout> };
  #seq = 0;
  #trainingBattles = new Set<number>();
  /** Whether the last private call that ran was a successful `rooms.join` to Training. */
  #inTraining = false;
  lastPoll = 0;

  /** Why `body` may not be sent, or the request it describes. */
  check(body: unknown): { request: Request } | { error: string } {
    if (!isObject(body)) return { error: "expected a JSON object" };
    if (typeof body.call === "string") {
      const call = body.call;
      if (!(AUTOPLAY_CALLS as readonly string[]).includes(call)) return { error: `autoplay never sends ${call}` };
      const params = body.params === undefined ? {} : body.params;
      if (!isObject(params)) return { error: "params must be an object" };
      if (call === "rooms.join" && params.id !== TRAINING_ROOM) {
        return { error: `autoplay only joins the Training room (${TRAINING_ROOM})` };
      }
      if (call === "battles.quickBattle" && !this.#inTraining) {
        return { error: "battles.quickBattle needs a successful rooms.join to Training right before it" };
      }
      if ((call === "battles.play" || call === "battles.quit") && !this.#trainingBattles.has(Number(params.id))) {
        return { error: `battle ${params.id} is not a Training battle this broker has seen` };
      }
      return { request: { call, params } };
    }
    if (typeof body.deck === "string") {
      if (!(AUTOPLAY_DECK_ACTIONS as readonly string[]).includes(body.deck)) {
        return { error: `autoplay never sends ${body.deck}` };
      }
      const fields = body.fields;
      if (!Array.isArray(fields) || !fields.every((f) => Array.isArray(f) && f.length === 2 && f.every((s) => typeof s === "string"))) {
        return { error: "fields must be [name, value] string pairs" };
      }
      return { request: { deck: body.deck, fields: fields as [string, string][] } };
    }
    return { error: "expected a call or a deck action" };
  }

  /** Queue a checked request and wait for the userscript's answer. */
  request(body: unknown, timeoutMs = 30_000): Promise<AutoplayOutcome> {
    const checked = this.check(body);
    if ("error" in checked) return Promise.resolve({ ok: false, error: checked.error });
    const command = { id: ++this.#seq, ...checked.request } as AutoplayCommand;
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        this.#pending.delete(command.id);
        this.#queue = this.#queue.filter((c) => c.id !== command.id);
        resolve({ ok: false, error: `no answer from the game tab within ${timeoutMs / 1000} s (is /game/play/webgl/ open?)` });
      }, timeoutMs);
      this.#pending.set(command.id, { command, resolve, timer });
      if (this.#poller) {
        const poller = this.#poller;
        this.#poller = undefined;
        clearTimeout(poller.timer);
        poller.resolve(command);
      } else {
        this.#queue.push(command);
      }
    });
  }

  /** The userscript's long poll: the next command, or null after `waitMs`. */
  next(waitMs = 20_000): Promise<AutoplayCommand | null> {
    this.lastPoll = Date.now();
    const queued = this.#queue.shift();
    if (queued) return Promise.resolve(queued);
    // One tab drives; a second poll (a reload, a second tab) takes over from the first.
    if (this.#poller) {
      clearTimeout(this.#poller.timer);
      this.#poller.resolve(null);
    }
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        if (this.#poller?.resolve === resolve) this.#poller = undefined;
        resolve(null);
      }, waitMs);
      this.#poller = { resolve, timer };
    });
  }

  /** The userscript's answer to command `body.id`. False when nothing was waiting for it. */
  result(body: unknown): boolean {
    if (!isObject(body) || typeof body.id !== "number") return false;
    const pending = this.#pending.get(body.id);
    if (!pending) return false;
    this.#pending.delete(body.id);
    clearTimeout(pending.timer);
    const outcome: AutoplayOutcome = body.ok === true
      ? { ok: true, result: body.result }
      : { ok: false, error: typeof body.error === "string" ? body.error : "the game tab reported a failure" };
    this.#learn(pending.command, outcome);
    pending.resolve(outcome);
    return true;
  }

  state(now = Date.now()) {
    return {
      bridge: now - this.lastPoll < 30_000,
      lastPollAgoMs: this.lastPoll ? now - this.lastPoll : null,
      queued: this.#queue.length,
      waiting: this.#pending.size,
    };
  }

  #learn(command: AutoplayCommand, outcome: AutoplayOutcome) {
    if (!("call" in command)) return;
    const result: Json = outcome.ok ? outcome.result : undefined;
    const failed = !outcome.ok || (isObject(result) && Array.isArray(result.errors));
    if (command.call === "rooms.join") {
      this.#inTraining = !failed && result?.data?.room?.id === TRAINING_ROOM;
      return;
    }
    // Only an immediately preceding join counts: any other call, the quickBattle itself
    // included, closes the window.
    this.#inTraining = false;
    const battle = result?.data?.battle;
    if (command.call === "battles.status" && !failed && battle && battle.battleRuleId === TRAINING_RULE) {
      this.#trainingBattles.add(Number(battle.id));
    }
  }
}
