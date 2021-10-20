type PromiseCallback = (item: any) => Promise<any>

export default class CallbackPromiseMap {
  items: any[];
  cb: PromiseCallback;
  max: number;
  id: number;
  pending: Map<number, Promise<any>>;
  constructor(items: any[], cb: PromiseCallback, max = 3) {
    // if (cb.constructor.name != 'AsyncFunction') {
    //   throw Error(`Callback is not async`);
    // }

    this.items = items;
    this.cb = cb;
    this.max = max;
    this.id = 0;
    this.pending = new Map();
  }

  promise(item: any) {
    let id = this.id;
    let promise = this.cb(item)
      .then(() => this.pending.delete(id))

    this.pending.set(this.id++, promise);
  }

  get promises() {
    return this.pending.values();
  }

  async race() {
    for (let item of this.items) {
      this.promise(item);

      if (this.pending.size >= this.max) {
        await Promise.race(this.promises);
      }
    }
  }

  static async race(items: any[], cb: PromiseCallback, max: number) {
    await new CallbackPromiseMap(items, cb, max).race();
  }
}


class PromiseMap {
  items: Promise<any>[];
  max: number;
  id: number;
  pending: Map<number, Promise<any>>;
  constructor(items: Promise<any>[], max = 4) {
    this.items = items;
    this.max = max;
    this.id = 0;
    this.pending = new Map();
  }

  promise(item: Promise<any>) {
    let id = this.id;
    let promise = item
      .then(() => this.pending.delete(id))

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

  static async race(promises: Promise<any>[], max: number) {
    await new PromiseMap(promises, max).race();
  }
}

async function cb(val: any) {
  return new Promise(resolve => setTimeout(_ => {
    console.log(`Finished: ${val}`);
    resolve(val);
  }, 5000));
}

// await CallbackPromiseMap.race([0, 1, 2, 3, 4, 5, 6, 7], cb, 3);

// await PromiseMap.race([0, 1, 2, 3, 4, 5, 6, 7].map(cb), 3);