import Ability, { AbilityType, LatchFamily } from "../Ability.ts";
import BattleData from "./BattleData.ts";
import EventTime from "../types/EventTime.ts";
import type BasicModifier from "../modifiers/BasicModifier.ts";
import CancelModifier, { Cancel } from "../modifiers/CancelModifier.ts";

function sourceCancel(ability: Ability) {
  if (
    ability.type === AbilityType.ABILITY ||
    ability.type === AbilityType.GLOBAL_ABILITY
  ) return Cancel.ABILITY;
  if (
    ability.type === AbilityType.BONUS ||
    ability.type === AbilityType.GLOBAL_BONUS
  ) return Cancel.BONUS;
  return undefined;
}

function blocks(blocker: Ability, target: Ability) {
  const source = sourceCancel(target);
  if (source === undefined) return false;
  for (let i = 0; i < blocker.mods.length; i++) {
    const modifier = blocker.mods[i];
    if (modifier instanceof CancelModifier && modifier.cancel === source) {
      return true;
    }
  }
  return false;
}

function hasPendingBlocker(
  target: Ability,
  blockers: Ability[],
  done: number,
) {
  for (let i = 0; i < blockers.length; i++) {
    if ((done & (1 << i)) === 0 && blocks(blockers[i], target)) return true;
  }
  return false;
}

/**
 * Whether an entry after `repeat[i]`, of its family and aimed at the same player, pays this
 * round, which replaces `repeat[i]` (see `Events.executeRepeat`). Later entries are newer:
 * a battle appends what it merges, bonus before ability.
 */
function replacedLater(repeat: Ability[], i: number, data: BattleData) {
  const a = repeat[i];
  const opp = (a.mods[0] as BasicModifier).opp;
  for (let j = i + 1; j < repeat.length; j++) {
    const b = repeat[j];
    if (
      b.family === a.family && (b.mods[0] as BasicModifier).opp === opp &&
      b.paysNow(data)
    ) return true;
  }
  return false;
}

function minClamp(a: Ability) {
  const m = a.mods[0] as { min?: number } | undefined;
  return m?.min === undefined || !Number.isFinite(m.min) ? -Infinity : m.min;
}

function byMinDescending(a: Ability, b: Ability) {
  return minClamp(b) - minClamp(a);
}

/**
 * Which pass of the end of the round an END entry runs in: 0 for an increase (a Life or
 * Pillz gain, Recover, Heal, Regen, Dope, Repair), 1 for a decrease (every opposing or own
 * reduction, Poison, Toxin, Consume, Combust, Mindwipe). One text compiles to modifiers of
 * one sign, so the first modifier decides; a RecoverModifier has no `change`.
 */
function endPass(a: Ability) {
  const change = (a.mods[0] as { change?: number } | undefined)?.change;
  return change !== undefined && change < 0 ? 1 : 0;
}

/**
 * Empty a bucket without assigning `length`. `length` is an accessor with a C++ setter, so
 * `arr.length = 0` leaves optimised code through the StoreIC on every call - even on an
 * already empty array - and shrinks the backing store that the next push must regrow.
 * With twenty buckets cleared per battle, avoiding it halved `deno task time-search`
 * (1.27 s -> 0.64 s). `pop()` is inlined by TurboFan and keeps the capacity.
 */
function clear(events: Ability[]) {
  while (events.length !== 0) events.pop();
}

export default class Events {
  events = new Array(10).fill(undefined).map<Ability[]>(() => []);
  repeat = new Array(10).fill(undefined).map<Ability[]>(() => []);
  /**
   * Bit `t` is set whenever `events[t]` or `repeat[t]` might hold an ability, so CardBattle
   * can skip the times that have nothing to run. A side usually fills one or two of its ten
   * times, and an `execute` on an empty time is a pure no-op, so skipping it cannot change a
   * result. Every push sets the bit (`add`, `addGlobal`, `CachedEvents.merge`); only
   * `execute` and `executeCancels` clear it, once that time is empty in both arrays. An
   * array that shrinks elsewhere (`removeGlobal`) leaves a stale bit set, which costs one
   * no-op `execute` and is then cleared. `Game.unmake` restores the mask it saved, so a
   * make/unmake pair leaves it exactly as it was.
   */
  mask = 0;

  clone(): Events {
    const e: Events = Object.create(Events.prototype);
    e.events = this.events.map((arr) => arr.map((a) => a.clone()));
    e.repeat = this.repeat.map((arr) => arr.map((a) => a.clone()));
    e.mask = this.mask;
    return e;
  }

  static from(o: Events) {
    Object.setPrototypeOf(o, Events.prototype);

    o.events = o.events.map((arr) => arr.map(Ability.from));
    o.repeat = o.repeat.map((arr) => arr.map(Ability.from));
    // Rebuilt from the arrays rather than trusted, so an object that never had one works.
    let mask = 0;
    for (let t = 0; t < 10; t++) {
      if (o.events[t].length !== 0 || o.repeat[t].length !== 0) mask |= 1 << t;
    }
    o.mask = mask;

    return o;
  }

  add(event: EventTime, ability: Ability) {
    this.events[event].push(ability);
    this.mask |= 1 << event;
  }

  addGlobal(event: EventTime, ability: Ability) {
    this.repeat[event].push(ability);
    this.mask |= 1 << event;
  }

  removeGlobal(event: EventTime, ability: Ability) {
    const index = this.repeat[event].indexOf(ability);
    if (index >= 0) this.repeat[event].splice(index, 1);
  }

  /**
   * Apply one bucket of permanent effects without skipping the entry shifted into place
   * when an effect fails to latch and removes itself. A `for...of` iterator advances its
   * index after that splice, so two same-time effects could leave the second one unlatched
   * in `repeat` (battle 1145959: copied Dope followed by Pere Barali's Heal).
   *
   * A latch of a replace-not-stack family (`LatchFamily`) gives way to a newer one of its
   * family aimed at the same player as soon as the newer one pays. The older entry is
   * marked replaced (`won = false`) instead of being taken out, so `Undo` restores it with
   * the other flags and the saved prefix of `repeat` stays intact. A delayed newcomer
   * (Poison, Heal) does not pay in the round it latches, so the old latch pays once more
   * there: 1131208 r2, 1092369 r2 and 926420 r3 each take the old latch's 2. 1506438 then
   * shows the replacement: the opposing Freaks bonus `Poison 2, Min 3` latches in rounds
   * zero and one, and round two takes 2 (7 -> 5), not 4, and round three one entry of 2. An
   * immediate newcomer (Toxin, Regen, Dope, Consume) pays in its own round, so the old one
   * stops that very round, as printed; no capture shows that case yet.
   *
   * Only one side's latches are compared: a player's own `Backlash: Poison` and an opposing
   * Poison land on the same player from two `Events`, and still both pay (unobserved).
   */
  private executeRepeat(event: EventTime, data: BattleData, pass = -1) {
    const repeat = this.repeat[event];
    let i = 0;
    while (i < repeat.length) {
      const ability = repeat[i];
      if (pass >= 0 && endPass(ability) !== pass) {
        i++;
        continue;
      }
      if (
        ability.family !== LatchFamily.NONE && ability.won !== false &&
        replacedLater(repeat, i, data)
      ) {
        ability.won = false;
        i++;
        continue;
      }
      ability.apply(data);
      if (repeat[i] === ability) i++;
    }
  }

  /**
   * Resolve the two cards' cancellation effects by dependency, rather than arbitrary
   * player order. A stop whose source is itself stopped must not fire first merely because
   * that card is internal P1 (1090338), while an unstopped ability must still be able to
   * suppress an opposing Stop Bonus even when it moved second (867116).
   *
   * The graph is normally acyclic: repeatedly discard effects whose source is now blocked,
   * then run an effect which no unresolved opposing effect can block. Retain the historical
   * P1/bonus-first order only as a stable fallback for a true mutual-stop cycle. The
   * bitmasks deliberately avoid allocating a temporary graph in this solver-hot path.
   */
  static executeCancels(
    first: Events,
    firstData: BattleData,
    second: Events,
    secondData: BattleData,
  ) {
    const event = EventTime.PRE4;
    const firstEvents = first.events[event];
    const secondEvents = second.events[event];
    let firstDone = 0;
    let secondDone = 0;
    let remaining = firstEvents.length + secondEvents.length;

    while (remaining > 0) {
      // Conditions and cancellations already resolved by an earlier dependency can make an
      // event inert. Removing it also releases anything it appeared able to block.
      let side = 0;
      let index = -1;
      for (let i = 0; i < firstEvents.length && index < 0; i++) {
        if (
          (firstDone & (1 << i)) === 0 &&
          !firstEvents[i].canApply(firstData)
        ) {
          side = 1;
          index = i;
        }
      }
      for (let i = 0; i < secondEvents.length && index < 0; i++) {
        if (
          (secondDone & (1 << i)) === 0 &&
          !secondEvents[i].canApply(secondData)
        ) {
          side = 2;
          index = i;
        }
      }

      if (index < 0) {
        for (let i = 0; i < firstEvents.length && index < 0; i++) {
          if (
            (firstDone & (1 << i)) === 0 &&
            !hasPendingBlocker(firstEvents[i], secondEvents, secondDone)
          ) {
            side = 1;
            index = i;
          }
        }
        for (let i = 0; i < secondEvents.length && index < 0; i++) {
          if (
            (secondDone & (1 << i)) === 0 &&
            !hasPendingBlocker(secondEvents[i], firstEvents, firstDone)
          ) {
            side = 2;
            index = i;
          }
        }
      }

      // Preserve the old P1/bonus-first order when every remaining event is in a cycle.
      if (index < 0) {
        for (let i = 0; i < firstEvents.length && index < 0; i++) {
          if ((firstDone & (1 << i)) === 0) {
            side = 1;
            index = i;
          }
        }
        for (let i = 0; i < secondEvents.length && index < 0; i++) {
          if ((secondDone & (1 << i)) === 0) {
            side = 2;
            index = i;
          }
        }
      }

      if (side === 1) {
        firstDone |= 1 << index;
        firstEvents[index].apply(firstData);
      } else {
        secondDone |= 1 << index;
        secondEvents[index].apply(secondData);
      }
      remaining--;
    }

    clear(firstEvents);
    clear(secondEvents);

    // No repeated PRE4 effects are currently known, but preserve execute()'s semantics.
    first.executeRepeat(event, firstData);
    second.executeRepeat(event, secondData);
    if (first.repeat[event].length === 0) first.mask &= ~(1 << event);
    if (second.repeat[event].length === 0) second.mask &= ~(1 << event);
  }

  /**
   * Settle the end of the round: every increase of both players first, then every decrease,
   * the decreases in descending order of their Min clamp - the order the server already uses
   * for the opposing combat-stat reductions (`execute`). Within a pass the order is the old
   * one: internal P2 before P1, bonus before ability, a side's current-round effects before
   * the permanents it has latched, and all current-round decreases before the latched ones.
   *
   * Four captured rounds put an increase before a decrease that would have clamped it, on
   * either seat and whoever won: 1093173 r1 (P2's Riots `+1 Pillz` before P1's Goose `-2 Opp.
   * Pillz And Life, Min 5`: 5 + 1 - 2 = 4, clamped to 5), 1514836 r0 (P1's DJ LBerto `Defeat:
   * +2 Pillz Max. 10` before P2's Yomi Ld `-2 Opp. Pillz And Life, Min 1`: 9 + 2 = 11, capped
   * at 10, - 2 = 8), 1515298 r1 (P1's Tortuga `Defeat: Recover 2 Pillz Out Of 3` before P2's
   * Brampah Noel `-2 Opp Pillz. Min 3`: 4 + 2 - 2 = 4, not 4 -> 3 + 2 = 5) and 1515451 r2
   * (P1's Ennio `+4 Life` and its latched Regen before P2's latched Poison: 13 + 4 = 17, Regen
   * capped at 17, Poison 1 -> 16). Two same-target reductions resolve by descending Min, as
   * the bonus happens to be the higher one in 876752 r1 (Berzerk `-2 Opp. Life Min 2` before
   * Macey Rook's Brawl `-1 Opp. Life Min 0`: 5 -> 3 -> 0) and 1514883 r2, and the ability in
   * 1515853 r0 and 1515873 r0 (Hilly Billy's `-5 Opp. Life Min 4` before the Berzerk bonus:
   * 10 -> 5 -> 3). The rule it replaces, internal P2's END effects before P1's, fitted
   * 1093173 r1 and 1496283 r2 only; 1496283 r2 does not constrain the order at all once its
   * two Oculus are left uninfiltrated (`Hand.from`).
   *
   * Allocation-free, like `executeCancels`: bitmasks mark the decreases already applied.
   * Applying an entry never adds to or removes from `events`; a permanent that fails to latch
   * removes itself from `repeat`, which `executeRepeat` already walks safely.
   */
  static executeEnd(
    first: Events,
    firstData: BattleData,
    second: Events,
    secondData: BattleData,
  ) {
    const event = EventTime.END;
    const secondEvents = second.events[event];
    const firstEvents = first.events[event];

    for (let i = 0; i < secondEvents.length; i++) {
      if (endPass(secondEvents[i]) === 0) secondEvents[i].apply(secondData);
    }
    second.executeRepeat(event, secondData, 0);
    for (let i = 0; i < firstEvents.length; i++) {
      if (endPass(firstEvents[i]) === 0) firstEvents[i].apply(firstData);
    }
    first.executeRepeat(event, firstData, 0);

    let secondDone = 0;
    let firstDone = 0;
    for (;;) {
      let side = 0;
      let index = -1;
      let best = -Infinity;
      for (let i = 0; i < secondEvents.length; i++) {
        if ((secondDone & (1 << i)) !== 0 || endPass(secondEvents[i]) !== 1) continue;
        const min = minClamp(secondEvents[i]);
        if (index < 0 || min > best) {
          side = 2;
          index = i;
          best = min;
        }
      }
      for (let i = 0; i < firstEvents.length; i++) {
        if ((firstDone & (1 << i)) !== 0 || endPass(firstEvents[i]) !== 1) continue;
        const min = minClamp(firstEvents[i]);
        if (index < 0 || min > best) {
          side = 1;
          index = i;
          best = min;
        }
      }
      if (index < 0) break;
      if (side === 2) {
        secondDone |= 1 << index;
        secondEvents[index].apply(secondData);
      } else {
        firstDone |= 1 << index;
        firstEvents[index].apply(firstData);
      }
    }
    clear(secondEvents);
    clear(firstEvents);

    second.executeRepeat(event, secondData, 1);
    first.executeRepeat(event, firstData, 1);
    if (second.repeat[event].length === 0) second.mask &= ~(1 << event);
    if (first.repeat[event].length === 0) first.mask &= ~(1 << event);
  }

  execute(event: EventTime, data: BattleData) {
    // Opponent-targeting reductions (PRE1 = power/damage, POST2 = attack) are applied by
    // the server in descending order of their Min clamp: Miss Stella (ability -8 Min 11,
    // bonus -8 Min 3) on 18 → 11 → 3; Don Cr (bonus -12 Min 8, ability -4 Min 2) on 18
    // → 8 → 4. Captured battles 875272, 901613, 901292.
    const events = this.events[event];
    if (
      events.length > 1 &&
      (event === EventTime.PRE1 || event === EventTime.POST2)
    ) {
      events.sort(byMinDescending);
    }
    for (const ability of events) {
      ability.apply(data);
    }
    clear(events);

    this.executeRepeat(event, data);
    if (this.repeat[event].length === 0) this.mask &= ~(1 << event);
  }

  // executeStart(data: BattleData) {
  //   this.execute(EventTime.START, data);
  // }

  // executePre(data: BattleData) {
  //   for (const e of [1, 2, 3, 4]) {
  //     console.log("Executing pre", e);
  //     this.execute(e, data);
  //   }
  // }

  // executePost(data: BattleData) {
  //   for (const e of [5, 6, 7, 8]) {
  //     this.execute(e, data);
  //   }
  // }

  // executeEnd(data: BattleData) {
  //   this.execute(EventTime.END, data);
  // }
}
