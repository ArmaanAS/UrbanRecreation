export default class CallbackPromiseMap {
  constructor(items, cb, max = 3) {
    // if (cb.constructor.name != 'AsyncFunction') {
    //   throw Error(`Callback is not async`);
    // }

    this.items = items;
    this.cb = cb;
    this.max = max;
    this.id = 0;
    this.pending = new Map();
  }

  promise(item) {
    let id = this.id;
    let promise = this.cb(item)
      .then(_ => this.pending.delete(id))

    this.pending.set(this.id++, promise);
  }

  async race() {
    for (let item of this.items) {
      this.promise(item);

      if (this.pending.size >= this.max) {
        await Promise.race(this.promises);
      }
    }
  }

  get promises() {
    return this.pending.values();
  }

  static async race(items, cb, max) {
    await new CallbackPromiseMap(items, cb, max).race();
  }
}


class PromiseMap {
  constructor(items, max = 4) {
    this.items = items;
    this.max = max;
    this.id = 0;
    this.pending = new Map();
  }

  promise(item) {
    let id = this.id;
    let promise = item
      .then(_ => this.pending.delete(id))

    this.pending.set(this.id++, promise);
  }

  async race() {
    for (let promise of this.items) {
      this.promise(promise);

      if (this.pending.size >= this.max) {
        await Promise.race(this.promises);
      }
    }
  }

  get promises() {
    return this.pending.values();
  }

  static async race(promises, max) {
    await new PromiseMap(promises, max).race();
  }
}

async function cb(val) {
  return new Promise(r => setTimeout(_ => {
    console.log(`Finished: ${val}`);
    r(val);
  }, 5000));
}

// await CallbackPromiseMap.race([0, 1, 2, 3, 4, 5, 6, 7], cb, 3);

// await PromiseMap.race([0, 1, 2, 3, 4, 5, 6, 7].map(cb), 3);