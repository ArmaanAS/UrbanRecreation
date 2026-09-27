/**
 * Situation coverage: which in-game situations a round produces, as stable string keys, so
 * the capture corpus can be measured against them and a Training driver can steer towards
 * rounds no capture has shown yet.
 *
 * Every engine rule fixed so far was found by a captured round in which some ability did
 * something in a context no earlier capture had shown. A "situation" is that unit: a source
 * that visibly did something, the coarse context it did it in, and the pairs of sources that
 * changed each other. Nothing here changes the engine: a round is resolved with the engine's
 * own `CachedCardBattle`, once as it is and once more per source with that source's compiled
 * effects taken out (ablation), and the results are compared.
 *
 * ## Keys
 *
 * A *source key* names one thing that can act in a round:
 *
 * - `ability:<id>` / `bonus:<id>` - the ability or clan bonus of the card a player puts down,
 *   and a lone Leader's ability (`ability:<id>`, whether or not the Leader is the card
 *   played: it is compiled every round). Ids are the battle API's (`captures/abilities.json`)
 *   ability/bonus dictionary ids. `?` stands for an id that could not be resolved.
 * - `paid:ability:<id>` / `paid:bonus:<id>` - a permanent (Poison, Heal, Toxin, Regen, Dope,
 *   Consume, Repair, Combust, Mindwipe) latched in an earlier round, paying in this one. The
 *   slot is kept because one id can sit in either slot.
 *
 * The keys a round produces:
 *
 * - `fired:<slot>:<id>` - the source changed something observable (below), and
 *   `fired:<slot>:<id>:<win|lose>:<first|second>` - the same with whether its owner won the
 *   round and moved first in it.
 * - `fired:bonus:infiltrated` - an Oculus card's adopted bonus fired. "Infiltrated" has no
 *   dictionary id of its own (the server shows the adopted bonus), so it gets a name.
 * - `paid:<slot>:<id>` - a latched permanent paid (removing it changes the round).
 * - `pair:<own|opp>:<a>|<b>` - two source keys, sorted, of the same (`own`) or opposing
 *   (`opp`) players, whose effects interact: removing one changes what the other does
 *   (see "Interaction"). A Copy of the opposing ability or bonus that fired also pairs with
 *   the source it copied (`pair:opp:ability:<copier>|bonus:<copied>`).
 *
 * ## Observables
 *
 * A resolution is compared on: Power, Damage and Attack of both cards; who won the round;
 * both players' Life and Pillz after it; and the latched permanents (how many new entries
 * latched or were replaced this round, and the latched/replaced state of each entry already
 * there). A permanent that latches but pays nothing yet (Poison, Heal, Combust) therefore
 * still fires in the round it latches.
 *
 * ## Ablation
 *
 * The round is compiled once per card pair exactly as `Game` compiles it (`CachedCardBattle`
 * over this position's hands), and a source is taken out by dropping its compiled entries
 * before the battle runs:
 *
 * - the played card's ability: every entry of type ABILITY / GLOBAL_ABILITY of that side,
 *   which includes anything a `Copy: Opp. Ability/Bonus` in that slot copied - the copy is the
 *   copier's effect, and the copied card's own effect is untouched;
 * - the played card's bonus: BONUS / GLOBAL_BONUS entries;
 * - a Leader: its GLOBAL entries, both the one compiled for this round and the one already in
 *   `Events.repeat` from an earlier round; a Tie-break Leader also loses the tie rule (the
 *   turn-order table is rebuilt on the ablated copy with the Leader's ability flag cleared);
 * - a latched permanent: its entry is marked replaced (`won = false`), which makes it inert
 *   exactly as a newer latch of its family would, without moving any other entry.
 *
 * Edge cases, by construction rather than special-casing:
 *
 * - Stop / Cancel: a Stop that blocks an ability which would have done something fires; the
 *   stopped ability does not fire (removing it changes nothing), and the pair of the two is
 *   an interaction. Two redundant Stops (an ability and a bonus stopping the same thing) fire
 *   neither alone but pair with each other; their target, which only a three-way ablation
 *   would expose, is not named.
 * - Protection fires only when something tried to reduce or cancel what it protects, and
 *   that mattered.
 * - Copy: the copier fires when what it copied mattered, and pairs with the copied source.
 *   The copied card's ability fires only if it did something on its own card.
 * - Support, Brawl, Growth, Equalizer and the other multipliers are part of their source's
 *   entries; a multiplier of zero is simply a source that did not fire.
 * - Permanents: see `paid:` above and "Latch origins" below.
 * - Counter-attack only decides round one's order before any card is played, so it can never
 *   fire; neither can abilities the engine does not implement (they compile to nothing).
 * - Compile-time flags stay: a `Stop:`-conditioned ability marks its own card protected when
 *   it compiles, and dropping its entries leaves that flag, which only its own (dropped)
 *   entries read.
 *
 * ## Interaction
 *
 * Sources X and Y interact when the interaction term O(all) - O(-X) - O(-Y) + O(-X-Y) is not
 * zero on some channel: removing X changes what Y does. On Power, Damage and Attack (settled
 * before the round is decided) any such difference counts. On Life, Pillz and latches it
 * counts only when the round's winner is the same in all four resolutions, because otherwise
 * it is just "X decided the winner, so Y's victory effect or damage landed elsewhere", which
 * the `:win`/`:lose` variants of `fired:` already say. So a pair is a mechanical interaction:
 * blocking, copying, clamping against each other's Min/Max, multiplying, ordering.
 * Co-firing alone (both fired, independently) is deliberately not a pair: nearly every round
 * has two independent sources, and those keys would drown the ones that matter. The same
 * rule covers a latched permanent and a current source, and two latched permanents (a newer
 * latch replacing an older one of its family pairs them): a Poison paying beside an unrelated
 * `+2 Power` is not a pair, a Poison that an opposing Cancel switches off is. Interactions of
 * three sources are only seen through whichever of their pairs has a non-zero term.
 *
 * ## Latch origins
 *
 * A latched entry in `Events.repeat` does not say which card started it. `LatchLedger` records
 * that as a match is played (`ledger.play(game, p1, p2, ids)` in place of the two `select`
 * calls): each entry a round appends is attributed to that side's played card, GLOBAL_BONUS
 * entries to its bonus and GLOBAL_ABILITY entries to its ability. A Copy that latched a copied
 * Poison is therefore the copier's `paid:ability:<id>`. Without a ledger, an entry is matched
 * by its compiled text to the owner's already-played cards; one it cannot match unambiguously
 * (a copied permanent, say) becomes `paid:<slot>:?`.
 *
 * ## Ids
 *
 * `slotIds` prefers the battle's own ids (`captures/games/*.json` hands), but only where the
 * captured description is the text the engine fights with: a battle's final static block
 * shows what a Copy resolved to rather than the Copy, so a Copy card keeps its printed id.
 * Otherwise an ability id is the card data's `ability_id` when the text is the row's printed
 * (day) text, and the dictionary id printed with exactly that text for a night variant or a
 * Hazard-dealt or rebalanced text; a bonus id comes from `CLAN_BONUS_IDS`.
 *
 * ## What this is not
 *
 * The keys are the engine's prediction. A round the engine gets wrong produces the wrong
 * keys, which is why the corpus scan (`Corpus.ts`) counts a captured round only once the
 * engine has reproduced it.
 *
 * ## Cost
 *
 * `RoundProbe` keeps the compiled battles of its position, so a driver asking about many
 * moves and replies at one position pays for each card pair's compilation once; each round
 * resolution after that is a `Game.clone` plus an `Events` clone and the battle itself, and a
 * round takes 1 + s + s(s-1)/2 of them for s live sources (usually two to four). On 12,000
 * random legal move pairs at random captured positions (`tests/situations/Situations.bench.ts`,
 * 2026-09-27) a call costs about 30 us on one reused probe and about 45 us through
 * `situationsOfRound`, which builds a probe per call. The corpus (1,500 rounds) scans in 0.4 s.
 */
import Game, { Winner } from "@/game/Game.ts";
import type Card from "@/game/Card.ts";
import Ability, { AbilityType } from "@/game/Ability.ts";
import type Events from "@/game/battle/Events.ts";
import CachedCardBattle from "@/game/battle/CachedCardBattle.ts";
import CachedEvents from "@/game/battle/CachedEvents.ts";
import { AbilityString, type Clan } from "@/game/types/CardTypes.ts";
import { Turn } from "@/game/types/Types.ts";
import cardRows from "@data/data.json" with { type: "json" };

/** One player's move in engine terms: hand index, pillz excluding the free pill, Fury. */
export type Move = readonly [index: number, pillz: number, fury: boolean];

export type Slot = "ability" | "bonus";

/**
 * The ability and bonus id of each hand slot, in engine order: 0-3 are player 1's hand
 * (`game.h1`), 4-7 player 2's. `undefined` where the slot has none or it is unknown.
 */
export interface SlotIds {
  ability: (number | undefined)[];
  bonus: (number | undefined)[];
}

/** Where a latched permanent came from. */
export interface SourceRef {
  slot: Slot;
  id: number | undefined;
}

/** The ability/bonus a battle snapshot attaches to a card (`captures/games/*.json` hands). */
export interface CapturedText {
  id: number;
  description: string;
}
export interface CapturedCard {
  ability?: CapturedText | null;
  bonus?: CapturedText | null;
}

// ---------------------------------------------------------------------------------------
// Ids
// ---------------------------------------------------------------------------------------

interface CardRow {
  id: number;
  level: number;
  ability_id: number;
  ability: string;
}

const rows = new Map<number, CardRow>();
for (const r of cardRows as CardRow[]) rows.set(r.id * 8 + r.level, r);

/**
 * The battle API's id for each clan bonus. `data.json`'s `bonus_id` is a different number
 * (the site's clan table, e.g. 39 for Rescue where battles say 266), and one printed text can
 * belong to two clans ("Power +2" is Ulu Watu's 39 and Bangers' 43), so neither the card data
 * nor the text can stand in for this table. Read off every captured hand; `tests/situations`
 * holds it to the corpus. Oculus ("Infiltrated") shows the adopted clan's bonus and id, and
 * a Leader's "Cancel Leader" never resolves as a bonus. A new clan needs an entry here.
 */
export const CLAN_BONUS_IDS: Readonly<Partial<Record<Clan, { day: number; night?: number }>>> = {
  "All Stars": { day: 156 },
  Bangers: { day: 43 },
  Berzerk: { day: 680 },
  Cosmohnuts: { day: 3496 },
  Dominion: { day: 1578 },
  "Fang Pi Clang": { day: 36 },
  Freaks: { day: 206 },
  Frozn: { day: 801 },
  GHEIST: { day: 94 },
  GhosTown: { day: 1441, night: 1442 },
  Hive: { day: 1338 },
  Huracan: { day: 923 },
  Jungo: { day: 401 },
  Junkz: { day: 37 },
  Komboka: { day: 1714 },
  "La Junta": { day: 38 },
  Leader: { day: 117 },
  Montana: { day: 6 },
  Nightmare: { day: 130 },
  Oblivion: { day: 2918 },
  Paradox: { day: 1844 },
  Piranas: { day: 333 },
  Pussycats: { day: 7 },
  Raptors: { day: 1163 },
  Rescue: { day: 266 },
  Riots: { day: 1034 },
  Roots: { day: 41 },
  Sakrohm: { day: 42 },
  Sentinel: { day: 93 },
  Skeelz: { day: 461 },
  Tolvack: { day: 5585 },
  "Ulu Watu": { day: 39 },
  Uppers: { day: 40 },
  Vortex: { day: 577 },
  Zenith: { day: 4657 },
};

let textIds: Map<string, number> | null | undefined;
/**
 * The smallest dictionary id printed with exactly `text`, from `captures/abilities.json`, for
 * a text the card data has no id for (a night variant, a Hazard-dealt or rebalanced text).
 * Read lazily and only then; without read access it simply finds nothing.
 */
function idForText(text: string): number | undefined {
  if (textIds === undefined) {
    textIds = null;
    try {
      const json = JSON.parse(
        Deno.readTextFileSync(new URL("../../captures/abilities.json", import.meta.url)),
      ) as Record<string, { id: number; description: string }>;
      const map = new Map<string, number>();
      for (const a of Object.values(json)) {
        const known = map.get(a.description);
        if (known === undefined || a.id < known) map.set(a.description, a.id);
      }
      textIds = map;
    } catch { /* no dictionary: ids stay unknown */ }
  }
  return textIds?.get(text) ?? undefined;
}

function abilityIdOf(card: Card, captured?: CapturedText | null): number | undefined {
  const text = card.abilityString;
  if (text === "No Ability") return undefined;
  // A battle's static block shows what a Copy resolved to, not the Copy, so the captured id
  // is only trusted when it describes the text the engine fights with.
  if (captured != null && captured.description === text) return captured.id;
  // The card data carries one id per row, the day text's; a night variant, a Hazard-dealt or
  // a rebalanced text is looked up by its words instead.
  const row = rows.get(card.id * 8 + card.stars);
  if (row !== undefined && row.ability === text) return row.ability_id;
  return idForText(text);
}

function bonusIdOf(card: Card, captured?: CapturedText | null): number | undefined {
  const text = card.bonusString;
  if (text === "No Bonus") return undefined;
  if (captured != null && captured.description === text) return captured.id;
  // `clan` is the effective clan, so an infiltrated Oculus looks up the clan it joined.
  const ids = CLAN_BONUS_IDS[card.clan];
  if (ids !== undefined) {
    return ids.night !== undefined && /^\s*Night\s*:/i.test(text) ? ids.night : ids.day;
  }
  return idForText(text);
}

/**
 * Every slot's ability and bonus id. `captured`, in engine slot order, supplies the battle's
 * own ids (prefer them when replaying a capture); anything it lacks comes from the card data.
 */
export function slotIds(game: Game, captured?: readonly (CapturedCard | undefined)[]): SlotIds {
  const ids: SlotIds = { ability: [], bonus: [] };
  for (let slot = 0; slot < 8; slot++) {
    const card = slot < 4 ? game.h1[slot] : game.h2[slot - 4];
    ids.ability.push(abilityIdOf(card, captured?.[slot]?.ability));
    ids.bonus.push(bonusIdOf(card, captured?.[slot]?.bonus));
  }
  return ids;
}

// ---------------------------------------------------------------------------------------
// Latch origins
// ---------------------------------------------------------------------------------------

const isGlobal = (a: Ability) => a.type === AbilityType.GLOBAL;

function permanentCounts(events: Events): number[] {
  const counts: number[] = [];
  for (let t = 0; t < 10; t++) {
    let n = 0;
    for (const a of events.repeat[t]) if (!isGlobal(a)) n++;
    counts.push(n);
  }
  return counts;
}

/**
 * Where each latched permanent of a match came from, recorded while the match is played.
 * `origins[side][t][k]` is the source of the k-th non-Leader entry of that side's
 * `events.repeat[t]` (side 0 = player 1).
 */
export class LatchLedger {
  readonly origins: SourceRef[][][] = [0, 1].map(() =>
    Array.from({ length: 10 }, () => [] as SourceRef[])
  );

  /**
   * Resolve one round on `game` exactly as the two `select` calls would (the first mover's
   * move first), then attribute every permanent it appended to the side's played card.
   */
  play(game: Game, p1: Move, p2: Move, ids: SlotIds) {
    if (game.firstHasSelected) {
      throw new Error("LatchLedger: the position must be at the start of a round");
    }
    const before = [permanentCounts(game.events1), permanentCounts(game.events2)];
    const [first, second] = game.playingFirst === Turn.PLAYER_1 ? [p1, p2] : [p2, p1];
    if (
      !game.select(first[0], first[1], first[2], false) ||
      !game.select(second[0], second[1], second[2], false)
    ) {
      throw new Error("LatchLedger: a card that was already played");
    }
    this.record(game, p1[0], p2[0], before, ids);
  }

  private record(game: Game, i1: number, i2: number, before: number[][], ids: SlotIds) {
    for (let side = 0; side < 2; side++) {
      const events = side === 0 ? game.events1 : game.events2;
      const slot = side === 0 ? i1 : 4 + i2;
      for (let t = 0; t < 10; t++) {
        let k = 0;
        for (const a of events.repeat[t]) {
          if (isGlobal(a)) continue;
          if (k++ < before[side][t]) continue;
          this.origins[side][t].push(
            a.type === AbilityType.GLOBAL_BONUS
              ? { slot: "bonus", id: ids.bonus[slot] }
              : { slot: "ability", id: ids.ability[slot] },
          );
        }
      }
    }
  }
}

/** Match a latched entry to the owner's played card whose own text compiles to it. */
function originByText(game: Game, side: number, entry: Ability, ids: SlotIds): SourceRef {
  const slot: Slot = entry.type === AbilityType.GLOBAL_BONUS ? "bonus" : "ability";
  const hand = side === 0 ? game.h1 : game.h2;
  const found = new Set<number | undefined>();
  for (let i = 0; i < 4; i++) {
    const card = hand[i];
    if (card.won === undefined) continue;
    const text = slot === "bonus" ? card.bonusString : card.abilityString;
    const compiled = new Ability(text);
    if (
      compiled.ability === entry.ability &&
      compiled.conditions.map((c) => c.s).join("|") === entry.conditions.map((c) => c.s).join("|")
    ) {
      found.add(slot === "bonus" ? ids.bonus[side * 4 + i] : ids.ability[side * 4 + i]);
    }
  }
  return { slot, id: found.size === 1 ? [...found][0] : undefined };
}

// ---------------------------------------------------------------------------------------
// Probe
// ---------------------------------------------------------------------------------------

/** Bits of an ablation mask for the current sources; player 2's are shifted by 3. */
const ABILITY = 1, BONUS = 2, LEADER = 4;

function dropClass(a: Ability): number {
  switch (a.type) {
    case AbilityType.ABILITY:
    case AbilityType.GLOBAL_ABILITY:
      return ABILITY;
    case AbilityType.BONUS:
    case AbilityType.GLOBAL_BONUS:
      return BONUS;
    case AbilityType.GLOBAL:
      return LEADER;
  }
  return 0;
}

/** A copy of compiled events without the entries of the classes in `drop`. */
function without(events: CachedEvents | undefined, drop: number): CachedEvents | undefined {
  if (events === undefined || drop === 0) return events;
  const out = new CachedEvents();
  for (let t = 0; t < 10; t++) {
    for (const a of events._events?.[t] ?? []) if ((dropClass(a) & drop) === 0) out.add(t, a);
    for (const a of events._repeat?.[t] ?? []) {
      if ((dropClass(a) & drop) === 0) out.addGlobal(t, a);
    }
  }
  return out;
}

function classesIn(events: CachedEvents | undefined): number {
  let classes = 0;
  for (let t = 0; t < 10; t++) {
    for (const a of events?._events?.[t] ?? []) classes |= dropClass(a);
    for (const a of events?._repeat?.[t] ?? []) classes |= dropClass(a);
  }
  return classes;
}

/** "Copy: Opp. Ability" / "Reprisal: Copy Opp. Bonus" and friends: which slot they copy. */
const COPY_SOURCE = /\bCopy\b\s*:?\s*Opp\.?\s*(Ability|Bonus)\b/i;

/** Number of channels before the latch channels; see `RoundProbe.outcome`. */
const BASE_CHANNELS = 11;
/** Power, Damage and Attack of both cards: settled before the round is decided. */
const STAT_CHANNELS = 6;
const WON_CHANNEL = 6;

interface Source {
  /** 0 = player 1, 1 = player 2. */
  side: number;
  /** Source key, e.g. `ability:73` or `paid:bonus:206`. */
  key: string;
  slot: Slot;
  id: number | undefined;
  /** Ablation bits for the current sources, or 0 for a latched one. */
  drop: number;
  /** Index into `RoundProbe.latched` for a latched one, else -1. */
  latched: number;
  /** An infiltrated Oculus bonus. */
  oculus: boolean;
}

interface Latched {
  side: number;
  t: number;
  /** Position among the non-Leader entries of `repeat[t]`. */
  k: number;
  /** Outcome channel holding this entry's latch state. */
  channel: number;
  ref: SourceRef;
}

export interface CardResult {
  power: number;
  damage: number;
  attack: number;
  won: boolean;
}

/** One round's situations, and the round as the engine resolves it with every source live. */
export interface RoundSituations {
  keys: Set<string>;
  p1: CardResult;
  p2: CardResult;
  life: [number, number];
  pillz: [number, number];
  /** Source keys that fired or paid, per player. */
  fired: [string[], string[]];
  /** The `pair:` keys alone. */
  pairs: string[];
}

export interface ProbeOptions {
  /** Ability/bonus id per hand slot; by default read from the cards (`slotIds(game)`). */
  ids?: SlotIds;
  /** Where the latched permanents came from; by default matched by text. */
  ledger?: LatchLedger;
}

/**
 * The situations of any round played from one position. It snapshots `game` (which must be
 * at the start of a round) and never touches it again, and it keeps each card pair's
 * compiled battle, so asking about many moves and replies at one position is cheap.
 */
export class RoundProbe {
  private readonly base: Game;
  private readonly ids: SlotIds;
  private readonly latched: Latched[] = [];
  /** Per side, per time: the non-Leader entries of `repeat[t]` already there. */
  private readonly before: number[][];
  private readonly compiled = new Map<number, CachedCardBattle>();
  /** Per side: a lone Leader's hand index, whether it has entries in `repeat`, Tie-break. */
  private readonly leader: { index: number; inRepeat: boolean; tieBreak: boolean }[] = [];

  constructor(game: Game, options: ProbeOptions = {}) {
    if (game.winner !== Winner.PLAYING) throw new Error("RoundProbe: the match is over");
    if (game.firstHasSelected) {
      throw new Error("RoundProbe: the position must be at the start of a round");
    }
    this.base = game.clone();
    this.base.events1 = game.events1.clone();
    this.base.events2 = game.events2.clone();
    this.ids = options.ids ?? slotIds(game);
    this.before = [permanentCounts(game.events1), permanentCounts(game.events2)];

    let channel = BASE_CHANNELS;
    for (let side = 0; side < 2; side++) {
      const events = side === 0 ? this.base.events1 : this.base.events2;
      channel += 2;
      for (let t = 0; t < 10; t++) {
        let k = 0;
        for (const a of events.repeat[t]) {
          if (isGlobal(a)) continue;
          if (a.won === true) {
            const ref = options.ledger?.origins[side][t][k] ?? originByText(game, side, a, this.ids);
            this.latched.push({ side, t, k, channel, ref });
          }
          k++;
          channel++;
        }
      }

      const hand = side === 0 ? this.base.h1 : this.base.h2;
      const leader = hand.getLeader();
      const index = leader === undefined ? -1 : hand.indexOf(leader);
      this.leader.push({
        index,
        inRepeat: events.repeat.some((arr) => arr.some(isGlobal)),
        tieBreak: leader !== undefined && leader.abilityString === "Tie-break",
      });
    }
  }

  /** The keys of the round in which player 1 plays `p1` and player 2 plays `p2`. */
  situations(p1: Move, p2: Move): Set<string> {
    return this.analyse(p1, p2).keys;
  }

  analyse(p1: Move, p2: Move): RoundSituations {
    this.check(0, p1);
    this.check(1, p2);

    const full = this.battle(p1[0], p2[0], 0);
    const sources: Source[] = [];
    for (let side = 0; side < 2; side++) {
      const move = side === 0 ? p1 : p2;
      const hand = side === 0 ? this.base.h1 : this.base.h2;
      const card = hand[move[0]];
      const slot = side * 4 + move[0];
      const shift = side * 3;
      const classes = classesIn(side === 0 ? full.events1 : full.events2);
      const leader = this.leader[side];
      if (classes & ABILITY) {
        const id = this.ids.ability[slot];
        sources.push(current(side, "ability", id, ABILITY << shift));
      }
      if (classes & BONUS) {
        const id = this.ids.bonus[slot];
        const src = current(side, "bonus", id, BONUS << shift);
        src.oculus = card.baseClan === "Oculus" && card.clan !== "Oculus";
        sources.push(src);
      }
      if (leader.index >= 0 && ((classes & LEADER) || leader.inRepeat || leader.tieBreak)) {
        const id = this.ids.ability[side * 4 + leader.index];
        sources.push(current(side, "ability", id, LEADER << shift));
      }
    }
    this.latched.forEach((l, i) => {
      sources.push({
        side: l.side,
        key: `paid:${l.ref.slot}:${l.ref.id ?? "?"}`,
        slot: l.ref.slot,
        id: l.ref.id,
        drop: 0,
        latched: i,
        oculus: false,
      });
    });

    const all = this.resolve(p1, p2, 0, 0);
    const alone = sources.map((s) =>
      this.resolve(p1, p2, s.drop, s.latched < 0 ? 0 : 1 << s.latched)
    );

    const keys = new Set<string>();
    const fired: [string[], string[]] = [[], []];
    const firedFlags = sources.map(() => false);
    const p1Won = all[WON_CHANNEL] === 1;
    sources.forEach((s, i) => {
      if (!this.differs(all, alone[i], [s])) return;
      firedFlags[i] = true;
      fired[s.side].push(s.key);
      if (s.latched >= 0) {
        keys.add(s.key);
        return;
      }
      const won = (s.side === 0) === p1Won;
      const first = (this.base.playingFirst === Turn.PLAYER_1) === (s.side === 0);
      keys.add(`fired:${s.key}`);
      keys.add(`fired:${s.key}:${won ? "win" : "lose"}:${first ? "first" : "second"}`);
      if (s.oculus) keys.add("fired:bonus:infiltrated");
    });

    const pairs: string[] = [];
    const addPair = (a: Source, b: Source) => {
      const [x, y] = [a.key, b.key].sort();
      const key = `pair:${a.side === b.side ? "own" : "opp"}:${x}|${y}`;
      if (!keys.has(key)) {
        keys.add(key);
        pairs.push(key);
      }
    };
    for (let i = 0; i < sources.length; i++) {
      for (let j = i + 1; j < sources.length; j++) {
        const a = sources[i], b = sources[j];
        const latchedOff = (a.latched < 0 ? 0 : 1 << a.latched) |
          (b.latched < 0 ? 0 : 1 << b.latched);
        const both = this.resolve(p1, p2, a.drop | b.drop, latchedOff);
        if (this.interacts(all, alone[i], alone[j], both, [a, b])) addPair(a, b);
      }
    }

    // A Copy of the opposing ability or bonus that fired pairs with what it copied: taking
    // the copied card's own entries out leaves the copy in place, so no interaction term can
    // see it.
    sources.forEach((s, i) => {
      if (!firedFlags[i] || s.latched >= 0 || (s.drop & (LEADER | (LEADER << 3))) !== 0) return;
      const move = s.side === 0 ? p1 : p2;
      const card = (s.side === 0 ? this.base.h1 : this.base.h2)[move[0]];
      const copy = COPY_SOURCE.exec(s.slot === "ability" ? card.abilityString : card.bonusString);
      if (copy === null) return;
      const oppSide = 1 - s.side;
      const oppMove = oppSide === 0 ? p1 : p2;
      const oppCard = (oppSide === 0 ? this.base.h1 : this.base.h2)[oppMove[0]];
      const slot: Slot = copy[1].toLowerCase() === "bonus" ? "bonus" : "ability";
      const text = slot === "bonus" ? oppCard.bonusString : oppCard.abilityString;
      if (text === "No Ability" || text === "No Bonus") return;
      const id = this.ids[slot][oppSide * 4 + oppMove[0]];
      addPair(s, current(oppSide, slot, id, 0));
    });

    const c1 = { power: all[0], damage: all[1], attack: all[2], won: p1Won };
    const c2 = { power: all[3], damage: all[4], attack: all[5], won: !p1Won };
    return {
      keys,
      p1: c1,
      p2: c2,
      life: [all[7], all[8]],
      pillz: [all[9], all[10]],
      fired,
      pairs,
    };
  }

  private check(side: number, move: Move) {
    const hand = side === 0 ? this.base.h1 : this.base.h2;
    const player = side === 0 ? this.base.p1 : this.base.p2;
    const [index, pillz, fury] = move;
    if (!Number.isInteger(index) || index < 0 || index > 3 || hand[index].played) {
      throw new Error(`RoundProbe: player ${side + 1} cannot play card ${index}`);
    }
    if (!Number.isInteger(pillz) || pillz < 0 || pillz + (fury ? 3 : 0) > player.pillz) {
      throw new Error(
        `RoundProbe: player ${side + 1} cannot bet ${pillz}${fury ? " + Fury" : ""} ` +
          `with ${player.pillz} pillz`,
      );
    }
  }

  /** The battle of card pair (i1, i2), compiled once, with the classes in `drop` taken out. */
  private battle(i1: number, i2: number, drop: number): CachedCardBattle {
    const key = ((i1 * 4 + i2) << 6) | drop;
    let ccb = this.compiled.get(key);
    if (ccb === undefined) {
      if (drop === 0) {
        ccb = new CachedCardBattle(this.base.h1, this.base.h1[i1], this.base.h2, this.base.h2[i2]);
      } else {
        const full = this.battle(i1, i2, 0);
        const filtered: CachedCardBattle = Object.create(CachedCardBattle.prototype);
        filtered.card1 = full.card1;
        filtered.card2 = full.card2;
        filtered.events1 = without(full.events1, drop & 7);
        filtered.events2 = without(full.events2, drop >> 3);
        ccb = filtered;
      }
      this.compiled.set(key, ccb);
    }
    return ccb;
  }

  /** Resolve the round on a private copy of the position with the given sources taken out. */
  private resolve(p1: Move, p2: Move, drop: number, latchedOff: number): number[] {
    const g = this.base.clone();
    // Between rounds a clone shares its parent's events, and the battle writes into them.
    g.events1 = this.base.events1.clone();
    g.events2 = this.base.events2.clone();

    for (let i = 0; latchedOff !== 0; i++, latchedOff >>>= 1) {
      if ((latchedOff & 1) === 0) continue;
      const l = this.latched[i];
      let k = 0;
      for (const a of (l.side === 0 ? g.events1 : g.events2).repeat[l.t]) {
        if (isGlobal(a)) continue;
        // Marked replaced: inert, and it no longer replaces anything older either.
        if (k++ === l.k) a.won = false;
      }
    }

    let rebuildTurns = false;
    for (let side = 0; side < 2; side++) {
      if ((drop & (LEADER << (side * 3))) === 0) continue;
      const events = side === 0 ? g.events1 : g.events2;
      for (let t = 0; t < 10; t++) events.repeat[t] = events.repeat[t].filter((a) => !isGlobal(a));
      const leader = this.leader[side];
      if (leader.tieBreak) {
        const hand = side === 0 ? g.h1 : g.h2;
        const copy = hand[leader.index].clone();
        copy.ability.string = AbilityString.NO_ABILITY;
        hand[leader.index] = copy;
        rebuildTurns = true;
      }
    }
    // Tie-break lives in the turn-order table; rebuild it for this copy alone.
    if (rebuildTurns) g.createBaseGameCache();

    this.battle(p1[0], p2[0], drop).play(g, p1[1], p1[2], p2[1], p2[2]);
    return this.outcome(g, p1[0], p2[0]);
  }

  /**
   * The channels a round is compared on: 0-5 Power, Damage and Attack of player 1's and
   * player 2's card, 6 whether player 1 won, 7-8 Life, 9-10 Pillz, then per player the count
   * of entries latched and replaced this round, and the state of every entry already latched.
   */
  private outcome(g: Game, i1: number, i2: number): number[] {
    const c1 = g.h1[i1], c2 = g.h2[i2];
    const o = [
      c1.power.final,
      c1.damage.final,
      c1.attack.final,
      c2.power.final,
      c2.damage.final,
      c2.attack.final,
      c1.won ? 1 : 0,
      g.p1.life,
      g.p2.life,
      g.p1.pillz,
      g.p2.pillz,
    ];
    for (let side = 0; side < 2; side++) {
      const events = side === 0 ? g.events1 : g.events2;
      const at = o.length;
      o.push(0, 0);
      for (let t = 0; t < 10; t++) {
        let k = 0;
        for (const a of events.repeat[t]) {
          if (isGlobal(a)) continue;
          if (k++ < this.before[side][t]) o.push(a.won === true ? 1 : a.won === false ? 2 : 0);
          else if (a.won === true) o[at]++;
          else if (a.won === false) o[at + 1]++;
        }
      }
    }
    return o;
  }

  /** Channels of latched entries that some compared resolution switched off itself. */
  private ignored(sources: Source[]): Set<number> {
    const set = new Set<number>();
    for (const s of sources) if (s.latched >= 0) set.add(this.latched[s.latched].channel);
    return set;
  }

  private differs(a: number[], b: number[], sources: Source[]): boolean {
    const ignore = this.ignored(sources);
    for (let c = 0; c < a.length; c++) if (a[c] !== b[c] && !ignore.has(c)) return true;
    return false;
  }

  private interacts(
    all: number[],
    noX: number[],
    noY: number[],
    neither: number[],
    sources: Source[],
  ): boolean {
    const term = (c: number) => all[c] - noX[c] - noY[c] + neither[c];
    for (let c = 0; c < STAT_CHANNELS; c++) if (term(c) !== 0) return true;
    const w = all[WON_CHANNEL];
    if (noX[WON_CHANNEL] !== w || noY[WON_CHANNEL] !== w || neither[WON_CHANNEL] !== w) {
      return false;
    }
    // A latch state (1 latched, 2 replaced) is a two-valued label, so the additive term
    // reads it like an indicator, as it does the counts.
    const ignore = this.ignored(sources);
    for (let c = WON_CHANNEL + 1; c < all.length; c++) {
      if (!ignore.has(c) && term(c) !== 0) return true;
    }
    return false;
  }
}

function current(side: number, slot: Slot, id: number | undefined, drop: number): Source {
  return { side, key: `${slot}:${id ?? "?"}`, slot, id, drop, latched: -1, oculus: false };
}

/** The situation keys the engine predicts for one round; see the module notes. */
export function situationsOfRound(
  game: Game,
  p1: Move,
  p2: Move,
  options?: ProbeOptions,
): Set<string> {
  return new RoundProbe(game, options).situations(p1, p2);
}

/**
 * How much new evidence a set of keys would add: the sum over keys of 1 / (1 + n), n being
 * how often the corpus has seen the key. A key never seen is worth 1 and each sighting
 * lowers the next one's worth harmonically - the rule-of-succession shape of "the chance this
 * sighting is the first to disagree with the engine, after n that agreed". Keys are weighted
 * equally; the hierarchy comes from the scheme itself: a first-ever source brings both its
 * plain and its context key, a known source in a new context brings one.
 */
export function noveltyOf(
  keys: Iterable<string>,
  counts: Readonly<Record<string, number>>,
): number {
  let novelty = 0;
  for (const key of keys) novelty += 1 / (1 + (counts[key] ?? 0));
  return novelty;
}
