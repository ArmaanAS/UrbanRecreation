import Ability from "./Ability"
import BattleData from "./BattleData";
import EventTime from "./types/EventTime";


export default class Events {
  events: Ability[][];
  repeat: Ability[][];
  constructor() {
    this.events = new Array(10).fill(null).map<Ability[]>(() => []);
    this.repeat = new Array(10).fill(null).map<Ability[]>(() => []);
  }

  clone(): Events {
    return Object.setPrototypeOf({
      events: this.events.map(arr => arr.map(a => a.clone())),
      repeat: this.repeat.map(arr => arr.map(a => a.clone()))
    }, Events.prototype);
  }

  static from(o: Events) {
    Object.setPrototypeOf(o, Events.prototype);

    o.events = o.events.map(arr => arr.map(Ability.from));
    o.repeat = o.repeat.map(arr => arr.map(Ability.from));

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
    while ((ability = this.events[event].pop()) !== undefined)
      ability.apply(data);

    for (const ability of this.repeat[event])
      ability.apply(data);
  }


  executeStart(data: BattleData) {
    this.execute(EventTime.START, data);
  }

  executePre(data: BattleData) {
    for (const e of [1, 2, 3, 4])
      this.execute(e, data);
  }

  executePost(data: BattleData) {
    for (const e of [5, 6, 7, 8])
      this.execute(e, data);
  }

  executeEnd(data: BattleData) {
    this.execute(EventTime.END, data);
  }
}