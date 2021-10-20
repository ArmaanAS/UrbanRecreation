interface Tree {
  [index: string]: Tree | number;
}

export function wordTree(lines: string[]) {
  let tree = {} as Tree;

  for (let line of lines) {
    let node = tree;
    for (let word of line.split(" ")) {
      if (node[word] === undefined)
        node[word] = {};

      node = node[word] as Tree;
    }
    node["<"] = 0;
  }

  for (let [k, node] of Object.entries(tree)) {
    if (node instanceof Object && node["<"] !== undefined && Object.keys(node).length == 1) {
      tree[k] = 0;
    }
  }

  return tree;
}

export function logTree(tree: Tree | number, depth = 0) {
  Object.entries(tree).forEach(([k, v], i) => {
    if (depth == 0) {
      console.log(k);
    } else {
      console.log(" ".repeat(depth * 4) + "+ " + k);
    }
    logTree(v, depth + 1);
  });
}


export function getN<T>(arr: T[], n = 1): T[] {
  if (n == 0) return [];
  if (n == 1) return [arr[(arr.length * Math.random()) | 0]];

  arr = Array.from(arr);

  if (arr.length <= n) return arr;

  let ret = [];
  while (n--) {
    ret.push(arr.splice((arr.length * Math.random()) | 0, 1)[0]);
  }

  return ret;
}

export function splitLines(s: string, len: number, min = 0) {
  let lines;
  if (min > 0) {
    lines = new Array(min).fill(" ".repeat(len));
  } else {
    lines = [];
  }
  let words = s.trim().split(/(?<= )/g);

  let line = "",
    lineN = 0;
  for (let word of words) {
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

export function* shiftRange(n: number) {
  yield n;
  if (n >= 3) yield n - 3;
  for (let i = 0; i < n - 3; i++)
    yield i;

  for (let i = n - 2; i < n; i++)
    yield i;
}

/**
 * Clone any object and keeps the prototype. 
 * Members should be primitive types
 * @param o Any object
 * @returns Shallow copy of base object
 */
export function clone<T extends object>(o: T): T {
  return Object.setPrototypeOf({ ...o }, o.constructor.prototype);
}