import Ability from "../Ability.ts";
import BattleData from "./BattleData.ts";
import EventTime from "../types/EventTime.ts";

export default class Events {
  events = new Array(10).fill(undefined).map<Ability[]>(() => []);
  repeat = new Array(10).fill(undefined).map<Ability[]>(() => []);

  clone(): Events {
    return Object.setPrototypeOf({
      events: this.events.map((arr) => arr.map((e) => e.clone())),
      repeat: this.repeat.map((arr) => arr.map((e) => e.clone())),
    }, Events.prototype);
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
    this.repeat[event].splice(this.repeat[event].indexOf(ability), 1);
  }

  execute(event: EventTime, data: BattleData) {
    // Opponent-targeting reductions (PRE1 = power/damage, POST2 = attack) are applied by
    // the server in descending order of their Min clamp: Miss Stella (ability -8 Min 11,
    // bonus -8 Min 3) on 18 → 11 → 3; Don Cr (bonus -12 Min 8, ability -4 Min 2) on 18
    // → 8 → 4. Captured battles 875272, 901613, 901292.
    if (event === EventTime.PRE1 || event === EventTime.POST2) {
      const min = (a: Ability) => {
        const m = a.mods[0] as { min?: number } | undefined;
        return m?.min === undefined || !Number.isFinite(m.min) ? -Infinity : m.min;
      };
      this.events[event].sort((a, b) => min(b) - min(a));
    }
    for (const ability of this.events[event]) {
      ability.apply(data);
    }
    this.events[event].length = 0;

    for (const ability of this.repeat[event]) {
      ability.apply(data);
    }
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
