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
    let ability: Ability | undefined;
    while ((ability = this.events[event].pop()) !== undefined) {
      ability.apply(data);
    }

    for (const ability of this.repeat[event]) {
      ability.apply(data);
    }
  }

  executeStart(data: BattleData) {
    this.execute(EventTime.START, data);
  }

  executePre(data: BattleData) {
    for (const e of [1, 2, 3, 4]) {
      this.execute(e, data);
    }
  }

  executePost(data: BattleData) {
    for (const e of [5, 6, 7, 8]) {
      this.execute(e, data);
    }
  }

  executeEnd(data: BattleData) {
    this.execute(EventTime.END, data);
  }
}
