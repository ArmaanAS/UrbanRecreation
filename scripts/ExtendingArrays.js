/* eslint-disable no-undef */
// class Hand extends Array {
//   get(i) {
//     return this[i];
//   }
// }

// const h = new Hand()
// h[0] = 1

// console.log(h)
// console.log(h.get(0))


class A extends Uint32Array {
  constructor() {
    super(2);
  }
  get a() {
    return this[0];
  }
}

class B {
  a = 0;
  b = 0;
  get f() {
    return this.a;
  }
}

class C extends Uint32Array {
  constructor(b) {
    super(2);
    this[0] = b[0];
    this[1] = b[1];
  }
  get a() {
    return this[0];
  }
}


class D extends Array { // Fastest!
  '0' = 0;
  '1' = 0;
  // constructor() {
  //   super(2);
  // }
  get a() {
    return this[0];
  }
}

class E extends Array {
  constructor(b) {
    super(2);
    this[0] = b[0];
    this[1] = b[1];
  }
  get a() {
    return this[0];
  }
}

console.log(new D)
console.log(new E([12314, 23425]))

const N = 2e6;
const a = new Array(N)
const b = new Array(N)
const c = new Array(N)
const d = new Array(N)
const e = new Array(N)
const f = new Array(N)
const g = new Array(N)
const h = new Array(N)
const i = new Array(N)
const j = new Array(N)

let mem = process.memoryUsage().heapUsed;
console.time('a')
for (let i = 0; i < N; i++)
  a[i] = new A
console.timeEnd('a')
console.log((process.memoryUsage().heapUsed - mem) / 1e6)
gc()
mem = process.memoryUsage().heapUsed

console.time('b')
for (let i = 0; i < N; i++) {
  const x = new Uint32Array(2);
  b[i] = Object.setPrototypeOf(x, A.prototype);
}
console.timeEnd('b')
console.log((process.memoryUsage().heapUsed - mem) / 1e6)
gc()
mem = process.memoryUsage().heapUsed

console.time('c')
for (let i = 0; i < N; i++)
  c[i] = new B
console.timeEnd('c')
console.log((process.memoryUsage().heapUsed - mem) / 1e6)
gc()
mem = process.memoryUsage().heapUsed

console.time('d')
for (let i = 0; i < N; i++) {
  const x = new Uint32Array(2);
  d[i] = new C(x);
}
console.timeEnd('d')
console.log((process.memoryUsage().heapUsed - mem) / 1e6)
gc()
mem = process.memoryUsage().heapUsed

console.time('e')
for (let i = 0; i < N; i++)
  e[i] = new D
console.timeEnd('e')
console.log((process.memoryUsage().heapUsed - mem) / 1e6)
gc()
mem = process.memoryUsage().heapUsed

console.time('f')
for (let i = 0; i < N; i++) {
  const x = [2, 4]
  f[i] = new E(x)
}
console.timeEnd('f')
console.log((process.memoryUsage().heapUsed - mem) / 1e6)
gc()
mem = process.memoryUsage().heapUsed

console.time('g')
for (let i = 0; i < N; i++) {
  const x = [23442, 64345];
  g[i] = Object.setPrototypeOf(x, D.prototype);
}
console.timeEnd('g')
console.log((process.memoryUsage().heapUsed - mem) / 1e6)
gc()
mem = process.memoryUsage().heapUsed

console.time('h')
for (let i = 0; i < N; i++) {
  const x = [23442, 64345];
  h[i] = Object.setPrototypeOf(x, E.prototype);
}
console.timeEnd('h')
console.log((process.memoryUsage().heapUsed - mem) / 1e6)
gc()
mem = process.memoryUsage().heapUsed

console.time('i')
for (let j = 0; j < N; j++) {
  const x = new D
  x[0] = 7645
  x[1] = 2342
  i[j] = new D(x)
}
console.timeEnd('i')
console.log((process.memoryUsage().heapUsed - mem) / 1e6)
gc()
mem = process.memoryUsage().heapUsed

console.time('j')
const xx = [12432, 63454]
for (let i = 0; i < N; i++) {
  j[i] = Object.setPrototypeOf([xx[0], xx[1]], D.prototype);
}
console.timeEnd('j')
console.log((process.memoryUsage().heapUsed - mem) / 1e6)
gc()
mem = process.memoryUsage().heapUsed



class S extends Array {
  // '0' = 0
  constructor() {
    super(1)
    this[0] = 0;
  }
  get f() {
    return this[0]
  }
}

class T {
  a = 0;
  get f() {
    return this.a
  }
}


const k = new Array(N)
const l = new Array(N)
const m = new Array(N)
const n = new Array(N)

console.time('k')
for (let i = 0; i < N; i++)
  k[i] = new S
console.timeEnd('k')
console.log((process.memoryUsage().heapUsed - mem) / 1e6)
gc()
mem = process.memoryUsage().heapUsed

console.time('l')
for (let i = 0; i < N; i++)
  l[i] = new T
console.timeEnd('l')
console.log((process.memoryUsage().heapUsed - mem) / 1e6)
gc()
mem = process.memoryUsage().heapUsed

console.time('m')
for (let i = 0; i < N; i++)
  m[i] = Object.setPrototypeOf([2], S.prototype)
let sum = 0;
for (let i = 0; i < N; i++)
  sum += m[i][0]
console.timeEnd('m')
console.log((process.memoryUsage().heapUsed - mem) / 1e6)
gc()
mem = process.memoryUsage().heapUsed

console.time('n')
for (let i = 0; i < N; i++)
  n[i] = Object.setPrototypeOf({ a: 2 }, T.prototype)
sum = 0;
for (let i = 0; i < N; i++)
  sum += n[i].a
console.timeEnd('n')
console.log((process.memoryUsage().heapUsed - mem) / 1e6)
gc()
mem = process.memoryUsage().heapUsed