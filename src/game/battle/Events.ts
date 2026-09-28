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

/**
 * Whether a latch in the other side's `Events`, of `a`'s family, aimed at the same player and
 * latched in a later round, pays this round, which replaces `a` as a newer entry on its own
 * side would (`replacedLater`). The two sides aim at one player when exactly one of them names
 * the opponent. An entry merged this round counts as latching in it. Two latches from one
 * round on different sides replace neither (unobserved).
 */
function replacedAcross(
  a: Ability,
  data: BattleData,
  other: Events | undefined,
  otherData: BattleData | undefined,
  event: EventTime,
) {
  if (other === undefined || otherData === undefined) return false;
  const opp = (a.mods[0] as BasicModifier).opp;
  const since = a.won === true ? a.since : data.round.round;
  const repeat = other.repeat[event];
  for (let j = 0; j < repeat.length; j++) {
    const b = repeat[j];
    if (b.family !== a.family || (b.mods[0] as BasicModifier).opp === opp) continue;
    const bSince = b.won === true ? b.since : otherData.round.round;
    if (bSince > since && b.paysNow(otherData)) return true;
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

  /** `add`, ahead of what the time already holds (`Ability.capsOwnStat`). */
  addFirst(event: EventTime, ability: Ability) {
    this.events[event].unshift(ability);
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
   * A latch on the same player from the other side replaces it too, ordered by the round each
   * latched (`replacedAcross`, given `other` at the end of the round). 1519318 shows it: the
   * owner's own Gork `Backlash: Poison 2, Min 4` latches in round one beside the opposing
   * Obyl Ld's `Poison 1, Min 0` from round zero, and from round two the player loses 2 a round,
   * not 3 (10 - 1 - 2 = 7, then 7 - 1 - 2 = 4). The resolution snapshot posts one permanent
   * entry of 2 on the player in rounds two and three, after the [1, 0] of the latching round.
   */
  private executeRepeat(
    event: EventTime,
    data: BattleData,
    pass = -1,
    other?: Events,
    otherData?: BattleData,
  ) {
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
        (replacedLater(repeat, i, data) ||
          replacedAcross(ability, data, other, otherData, event))
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
   * Settle the end of the round: the round's own effects first - every increase of both
   * players, then every decrease, the decreases in descending order of their Min clamp, the
   * order the server already uses for the opposing combat-stat reductions (`execute`) - and
   * only then the permanents latched on either side, their increases before their decreases.
   * Within a pass the order is the old one: internal P2 before P1, bonus before ability.
   *
   * The latched effects come after every current-round one, not with the increases: in
   * 1517029 r2 Owen's `-4 Opp. Life Min 2` takes Lakit Cr's owner from 10 to 6 and only then
   * does the latched `Heal 4 Max. 7` pay, to 7; the Heal first would find 10 above its cap,
   * pay nothing and leave 6. The server posts the round's effects on each player in the order
   * it applies them, and every post list in the corpus has the current-round entries ahead of
   * the latched ones: a latched gain after a current decrease in 1516124 r0, 1516372 r1 and
   * r2, 924669 r0 and 1514883 r2 as well, where no cap binds, and never the other way round.
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
    for (let i = 0; i < firstEvents.length; i++) {
      if (endPass(firstEvents[i]) === 0) firstEvents[i].apply(firstData);
    }

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

    second.executeRepeat(event, secondData, 0, first, firstData);
    first.executeRepeat(event, firstData, 0, second, secondData);
    second.executeRepeat(event, secondData, 1, first, firstData);
    first.executeRepeat(event, firstData, 1, second, secondData);
    if (second.repeat[event].length === 0) second.mask &= ~(1 << event);
    if (first.repeat[event].length === 0) first.mask &= ~(1 << event);
  }

  /**
   * The reductions phase of a stat (PRE1 for Power and Damage, POST2 for Attack), both sides at
   * once, in descending order of the Min clamp. Within one side that is the order the server
   * has always shown for two reductions on one card - Miss Stella (ability -8 Min 11, bonus -8
   * Min 3) on 18 -> 11 -> 3, Don Cr (bonus -12 Min 8, ability -4 Min 2) on 18 -> 8 -> 4
   * (875272, 901613, 901292). Across the sides it decides the one case where both sides reduce
   * one card, the owner's own half of a `Cards` reduction against the opposing card's cut, and
   * four captured rounds fit it on either seat where running one side and then the other
   * fits two:
   * - 1526067 r1: Niva's `Courage: -4 Opp. Dmg, Min 2` (P2) before Pere Fourrure's own
   *   `Support: -1 Cards Damage, Min 0` x 2 (P1): 6 -> 2 -> 0. P1 first gave 6 -> 4 -> 2.
   * - 1079078 r3: Rajesh's own `-2 Cards Damage, Min 4` (P2) before Sue's `-1 Opp Power And
   *   Damage, Min 3` (P1): 6 -> 4 -> 3. P1 first gave 6 -> 5 -> 4.
   * - 1525903 r1: Magenta's `-6 Opp Attack, Min 6` (P1) before Miss Denna's own `-7 Cards
   *   Attack, Min 0` (P2): 10 -> 6 -> 0, where P2 first would leave 3.
   * - 1525934 r2: C0re Cr's Hive `Equalizer: -3 Opp Attack, Min 5` (P1) before Miss Denna's
   *   own `-7 Cards Attack, Min 0` (P2): 10 -> 7 -> 0.
   * Equal Mins keep the old order, internal P1 before P2 and bonus before ability; so do the
   * unbounded entries (Tune Out, an opposing increase), after every bounded one.
   * Allocation-free: each side is sorted in place and the two are merged by index.
   */
  static executeCuts(
    event: EventTime,
    first: Events,
    firstData: BattleData,
    second: Events,
    secondData: BattleData,
  ) {
    const a = first.events[event];
    const b = second.events[event];
    if (a.length > 1) a.sort(byMinDescending);
    if (b.length > 1) b.sort(byMinDescending);
    let i = 0;
    let j = 0;
    while (i < a.length || j < b.length) {
      if (j >= b.length || (i < a.length && minClamp(a[i]) >= minClamp(b[j]))) {
        a[i++].apply(firstData);
      } else {
        b[j++].apply(secondData);
      }
    }
    clear(a);
    clear(b);

    first.executeRepeat(event, firstData);
    second.executeRepeat(event, secondData);
    if (first.repeat[event].length === 0) first.mask &= ~(1 << event);
    if (second.repeat[event].length === 0) second.mask &= ~(1 << event);
  }

  execute(event: EventTime, data: BattleData) {
    const events = this.events[event];
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
