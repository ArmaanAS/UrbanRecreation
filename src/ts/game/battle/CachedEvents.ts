import Ability from "../Ability"
import Events from "./Events";
import EventTime from "../types/EventTime";


export default class CachedEvents {
  _events?: Ability[][];
  _repeat?: Ability[][];

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
  }

  addGlobal(event: EventTime, ability: Ability) {
    this.repeat[event].push(ability);
  }

  merge(events: Events) {
    if (this._events !== undefined)
      for (let i = 0; i < 10; i++)
        for (const e of this._events[i])
          events.events[i].push(e.clone());

    if (this._repeat !== undefined)
      for (let i = 0; i < 10; i++)
        for (const e of this._repeat[i])
          events.repeat[i].push(e.clone());
  }
}