import Ability from "../Ability"
import Events from "./Events";
import EventTime from "../types/EventTime";


export default class CachedEvents {
  events = new Array(10).fill(undefined).map<Ability[]>(() => []);
  repeat = new Array(10).fill(undefined).map<Ability[]>(() => []);

  add(event: EventTime, ability: Ability) {
    this.events[event].push(ability);
  }

  addGlobal(event: EventTime, ability: Ability) {
    this.repeat[event].push(ability);
  }

  merge(events: Events) {
    for (let i = 0; i < 10; i++) {
      for (const e of this.events[i]) {
        events.events[i].push(e.clone());
      }

      for (const e of this.repeat[i]) {
        events.repeat[i].push(e.clone());
      }
      // events.events[i].push(...this.events[i].map(e => e.clone()));
      // events.repeat[i].push(...this.repeat[i].map(e => e.clone()));
    }
  }
}