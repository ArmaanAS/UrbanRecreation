import Ability from "../Ability.ts";
import Events from "./Events.ts";
import EventTime from "../types/EventTime.ts";
import BattleData from "@/game/battle/BattleData.ts";
import { AbilityType } from "@/game/Ability.ts";

export default class CachedEvents {
  _events?: Ability[][];
  _repeat?: Ability[][];
  /**
   * The times `add` and `addGlobal` filled. `merge` walks only those and hands them on as
   * `Events.mask`; a battle here fills one or two of the twenty.
   */
  eventsMask = 0;
  repeatMask = 0;

  get events() {
    return this._events ??= new Array(10)
      .fill(undefined).map<Ability[]>(() => []);
  }
  get repeat() {
    return this._repeat ??= new Array(10)
      .fill(undefined).map<Ability[]>(() => []);
  }

  add(event: EventTime, ability: Ability) {
    this.events[event].push(ability);
    this.eventsMask |= 1 << event;
  }

  addGlobal(event: EventTime, ability: Ability) {
    this._repeat ??= new Array(10).fill(undefined).map<Ability[]>(() => []);
    this._repeat[event].push(ability);
    this.repeatMask |= 1 << event;
  }

  /**
   * Each time is its own array, so visiting only the filled ones, lowest bit first, pushes
   * exactly what the plain 0..9 loop did in the same order within every array.
   */
  merge(events: Events) {
    // A GLOBAL entry below may be skipped as a duplicate; its bit set anyway only costs an
    // empty execute, which clears it again.
    events.mask |= this.eventsMask | this.repeatMask;
    for (let m = this.eventsMask; m !== 0; m &= m - 1) {
      const i = 31 - Math.clz32(m & -m);
      const from = this._events![i];
      const to = events.events[i];
      for (let k = 0; k < from.length; k++) to.push(from[k].clone());
    }

    for (let m = this.repeatMask; m !== 0; m &= m - 1) {
      const i = 31 - Math.clz32(m & -m);
      const from = this._repeat![i];
      const to = events.repeat[i];
      for (let k = 0; k < from.length; k++) {
        const e = from[k];
        if (e.type !== AbilityType.GLOBAL || to.length === 0) to.push(e.clone());
      }
    }
  }

  execute(event: EventTime, data: BattleData) {
    let ability: Ability | undefined;
    if (this._events !== undefined) {
      while ((ability = this._events[event].pop()) !== undefined) {
        ability.apply(data);
      }
    }

    if (this._repeat !== undefined) {
      for (const ability of this._repeat[event]) {
        ability.apply(data);
      }
    }
  }
}
