import Ability from "./ability.mjs";

export default class Events {
  constructor(e, r) {
    this.events = new Array(10).fill().map(() => []);
    this.repeat = new Array(10).fill().map(() => []);
  }

  clone() {
    return Object.setPrototypeOf({
      events: this.events.map(arr => arr.map(a => a.clone())),
      repeat: this.repeat.map(arr => arr.map(a => a.clone()))
    }, Events.prototype);
  }

  static from(o) {
    Object.setPrototypeOf(o, Events.prototype);

    o.events = o.events.map(arr => arr.map(Ability.from));
    o.repeat = o.repeat.map(arr => arr.map(Ability.from));

    return o;
  }

  add(event, ability) {
    this.events[event].push(ability);
  }

  addGlobal(event, ability) {
    this.repeat[event].push(ability);
  }

  execute(event, data) {
    let ability;
    while ((ability = this.events[event].pop()) != undefined) {
      ability.apply(data);
    }

    for (ability of this.repeat[event]) {
      ability.apply(data);
    }
  }


  executeStart(data) {
    this.execute(Events.START, data);
  }

  executePre(data) {
    for (let e of [1, 2, 3, 4]) {
      this.execute(e, data);
    }
  }

  executePost(data) {
    for (let e of [5, 6, 7, 8]) {
      this.execute(e, data);
    }
  }

  executeEnd(data) {
    this.execute(Events.END, data);
  }

  static START = 0;

  static PRE1 = 1;
  static PRE2 = 2;
  static PRE3 = 3;
  static PRE4 = 4;

  static POST1 = 5;
  static POST2 = 6;
  static POST3 = 7;
  static POST4 = 8;

  static END = 9;
}