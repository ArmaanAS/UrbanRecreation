// import { MessageChannel, receiveMessageOnPort } from 'worker_threads'
// import v8 from 'v8'

interface Tree {
  [index: string]: Tree | number;
}

export function wordTree(lines: string[]) {
  const tree: Tree = {};

  for (const line of lines) {
    let node = tree;
    for (const word of line.split(" ")) {
      if (node[word] === undefined)
        node[word] = {};

      node = node[word] as Tree;
    }
    node["<"] = 0;
  }

  function cull(tree: Tree) {
    for (const [k, node] of Object.entries(tree)) {
      if (node instanceof Object) {
        if ("<" in node && Object.keys(node).length === 1) {
          tree[k] = 0;
        } else {
          cull(node);
        }
      }
    }
  }

  cull(tree);

  function shorten(tree: Tree) {
    // for (const [k, node] of Object.entries(tree)) {
    //   if (node instanceof Object) {
    //     const entries = Object.entries(node);
    //     if (entries.length === 1 && entries[0][1] === 0) {
    //       delete tree[k];
    //       tree[`${k} ${entries[0][0]}`] = 0;
    //     } else {
    //       shorten(node);

    //     }
    //   }
    // }
    for (const [k, node] of Object.entries(tree)) {
      if (node instanceof Object) {
        shorten(node);
        const entries = Object.entries(node);
        if (entries.length === 1 && entries[0][1] === 0) {
          delete tree[k];
          tree[`${k} ${entries[0][0]}`] = 0;
        }
      }
    }
  }

  shorten(tree);

  return tree;
}

export function logTree(tree: Tree | number, depth = 0) {
  for (const [k, v] of Object.entries(tree)) {
    if (depth == 0) {
      console.log(k);
    } else {
      console.log(" ".repeat(depth * 4) + "+ " + k);
    }
    logTree(v, depth + 1);
  }
}


export function getN<T>(arr: T[], n = 1): T[] {
  if (n == 0) return [];
  if (n == 1) return [arr[(arr.length * Math.random()) | 0]];

  arr = Array.from(arr);

  if (arr.length <= n) return arr;

  const ret = [];
  while (n--)
    ret.push(arr.splice((arr.length * Math.random()) | 0, 1)[0]);


  return ret;
}

export function splitLines(s: string, len: number, min = 0) {
  let lines;
  if (min > 0) {
    lines = new Array(min).fill(" ".repeat(len));
  } else {
    lines = [];
  }
  const words = s.trim().split(/(?<= )/g);

  let line = "",
    lineN = 0;
  for (const word of words) {
    if (line.length == 0) {
      line = word;
    } else if (line.length + word.length <= len) {
      line += word;
    } else {
      lines[lineN] = line + " ".repeat(Math.max(len - line.length, 0));
      line = word;
      lineN++;
    }
  }
  lines[lineN] = line + " ".repeat(Math.max(len - line.length, 0));

  return lines;
}


export function* alternateRange(n: number) {
  let i = 0;
  for (; i < n; i++) {
    yield i;
    yield n--;
  }
  if (i == n) yield i;
}

// export function* shiftRangePre(n: number) {
export function* shiftRange(n: number) {
  yield n;

  if (n < 3) {
    for (let i = 0; i < n; i++)
      yield i;

  } else {
    yield n - 3;

    for (let i = 0; i < n - 3; i++)
      yield i;

    for (let i = n - 2; i < n; i++)
      yield i;
  }
}
// const shiftRangeCache = <number[][]>[];
// for (let i = 0; i < 32; i++)
//   shiftRangeCache[i] = [...shiftRangePre(i)];

// export function shiftRange(n: number) {
//   // const arr = shiftRangeCache[n];
//   // for (let i = 0; i <= n; i++)
//   //   yield arr[i];
//   return shiftRangeCache[n];
// }

// /**
//  * Clone any object and keeps the prototype. 
//  * Members should be primitive types
//  * @param o Any object
//  * @returns Shallow copy of base object
//  */
export function clone<T extends object>(o: T): T {
  return Object.setPrototypeOf({ ...o }, o.constructor.prototype);
}

// const { port1, port2 } = new MessageChannel();
// export function clone<T extends object>(o: T): T {
//   port2.postMessage(o);
//   return receiveMessageOnPort(port1)!.message;
// }

// export function clone<T extends object>(o: T): T {
//   // return Object.setPrototypeOf(
//   //   v8.deserialize(v8.serialize(o)), o.constructor.prototype);
//   return v8.deserialize(v8.serialize(o));
// }

export class FlowController {
  promise: Promise<void>;
  private res: () => void;
  constructor() {
    this.promise = new Promise(res => this.res = res);
  }

  resume() {
    this.res();
    this.promise = new Promise(res => this.res = res);
  }
}