import Ability, { AbilityType } from "../Ability.ts";
import BattleData from "./BattleData.ts";
import EventTime from "../types/EventTime.ts";
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

function minClamp(a: Ability) {
  const m = a.mods[0] as { min?: number } | undefined;
  return m?.min === undefined || !Number.isFinite(m.min) ? -Infinity : m.min;
}

function byMinDescending(a: Ability, b: Ability) {
  return minClamp(b) - minClamp(a);
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

  clone(): Events {
    const e: Events = Object.create(Events.prototype);
    e.events = this.events.map((arr) => arr.map((a) => a.clone()));
    e.repeat = this.repeat.map((arr) => arr.map((a) => a.clone()));
    return e;
  }

  static from(o: Events) {
    Object.setPrototypeOf(o, Events.prototype);

    o.events = o.events.map((arr) => arr.map(Ability.from));
    o.repeat = o.repeat.map((arr) => arr.map(Ability.from));

    return o;
  }

  add(event: EventTime, ability: Ability) {
    this.events[event].push(ability);
  }

  addGlobal(event: EventTime, ability: Ability) {
    this.repeat[event].push(ability);
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
   */
  private executeRepeat(event: EventTime, data: BattleData) {
    const repeat = this.repeat[event];
    let i = 0;
    while (i < repeat.length) {
      const ability = repeat[i];
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
