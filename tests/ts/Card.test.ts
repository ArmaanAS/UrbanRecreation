class CardStat {
  private a = 0;
  constructor(base: number) {
    this.base = base;
    this.final = base;
  }
  get cancel() {
    return !!(this.a & 1);
  }
  set cancel(n: boolean) {
    this.a = (this.a & ~1) | +n;
  }
  get prot() {
    return !!(this.a & 2);
  }
  set prot(n: boolean) {
    this.a = (this.a & ~2) | (+n << 1);
  }
  get base() {
    return this.a >> 2 & 0xff;  // 0b1111111100;
  }
  set base(n: number) {
    this.a = (this.a & ~0x3fc) | ((n & 0xff) << 2);
  }
  get final() {
    return this.a >> 10 & 0xff;
  }
  set final(n: number) {
    this.a = (this.a & ~0x3fc00) | ((n & 0xff) << 10);
  }
}
class CardString {
  string: string;
  private a = 0;
  constructor(string: string) {
    this.string = string
  }
  get cancel() {
    return !!(this.a & 1);
  }
  set cancel(n: boolean) {
    this.a = (this.a & ~1) | +n;
  }
  get prot() {
    return !!(this.a & 2);
  }
  set prot(n: boolean) {
    this.a = (this.a & ~2) | (+n << 1);
  }
}

test('CardStat', () => {
  const c = new CardStat(0);
  expect(c.base).toBe(0);
  expect(c.final).toBe(0);
  expect(c.prot).toBe(false);
  expect(c.cancel).toBe(false);

  c.base = 42;
  expect(c.base).toBe(42);
  expect(c.final).toBe(0);
  expect(c.prot).toBe(false);
  expect(c.cancel).toBe(false);

  c.final = 233;
  expect(c.base).toBe(42);
  expect(c.final).toBe(233);
  expect(c.prot).toBe(false);
  expect(c.cancel).toBe(false);

  c.prot = true;
  expect(c.base).toBe(42);
  expect(c.final).toBe(233);
  expect(c.prot).toBe(true);
  expect(c.cancel).toBe(false);

  c.cancel = true;
  expect(c.base).toBe(42);
  expect(c.final).toBe(233);
  expect(c.prot).toBe(true);
  expect(c.cancel).toBe(true);
})

test('CardString', () => {
  const c = new CardString('abcde');
  expect(c.string).toEqual('abcde');
  expect(c.prot).toBe(false);
  expect(c.cancel).toBe(false);

  c.string = 'xyz';
  expect(c.string).toEqual('xyz');
  expect(c.prot).toBe(false);
  expect(c.cancel).toBe(false);

  c.prot = true;
  expect(c.string).toEqual('xyz');
  expect(c.prot).toBe(true);
  expect(c.cancel).toBe(false);

  c.cancel = true;
  expect(c.string).toEqual('xyz');
  expect(c.prot).toBe(true);
  expect(c.cancel).toBe(true);
})


abstract class BaseAttr {
  protected a: number;
  protected b: number;
  abstract get cancel(): boolean;
  abstract set cancel(n: boolean);
  abstract get prot(): boolean;
  abstract set prot(n: boolean);
}
abstract class BaseStat extends BaseAttr {
  abstract get base(): number;
  abstract set base(n: number);
  abstract get final(): number;
  abstract set final(n: number);
}
class PowerStat extends BaseStat {
  get base(): number {
    return this.a & 0xff;
  }
  set base(n: number) {
    this.a = (this.a & ~0xff) | (n & 0xff);
  }
  get final(): number {
    return this.a >> 8 & 0xff;
  }
  set final(n: number) {
    this.a = (this.a & ~0xff00) | ((n & 0xff) << 8);
  }
  get cancel(): boolean {
    return !!(this.b >> 16 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x10000) | (+n << 16)
  }
  get prot(): boolean {
    return !!(this.b >> 17 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x20000) | (+n << 17)
  }
}
class DamageStat extends BaseStat {
  get base(): number {
    return this.a >> 16 & 0xff;
  }
  set base(n: number) {
    this.a = (this.a & ~0xff0000) | ((n & 0xff) << 16);
  }
  get final(): number {
    return this.a >> 24 & 0xff;
  }
  set final(n: number) {
    this.a = (this.a & ~0xff000000) | ((n & 0xff) << 24);
  }
  get cancel(): boolean {
    return !!(this.b >> 18 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x40000) | (+n << 18)
  }
  get prot(): boolean {
    return !!(this.b >> 19 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x80000) | (+n << 19)
  }
}
class AttackStat extends BaseStat {
  get base(): number {
    return this.b & 0xff;
  }
  set base(n: number) {
    this.b = (this.b & ~0xff) | (n & 0xff);
  }
  get final(): number {
    return this.b >> 8 & 0xff;
  }
  set final(n: number) {
    this.b = (this.b & ~0xff00) | ((n & 0xff) << 8);
  }
  get cancel(): boolean {
    return !!(this.b >> 20 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x100000) | (+n << 20)
  }
  get prot(): boolean {
    return !!(this.b >> 21 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x200000) | (+n << 21)
  }
}
class PillzStat extends BaseAttr {
  get cancel(): boolean {
    return !!(this.b >> 22 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x400000) | (+n << 22)
  }
  get prot(): boolean {
    return !!(this.b >> 23 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x800000) | (+n << 23)
  }
}
class LifeStat extends BaseAttr {
  get cancel(): boolean {
    return !!(this.b >> 24 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x1000000) | (+n << 24)
  }
  get prot(): boolean {
    return !!(this.b >> 25 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x2000000) | (+n << 25)
  }
}
test('DamageStat', () => {
  const o = { a: 0, b: 0 }
  let d: DamageStat = Object.setPrototypeOf(o, DamageStat.prototype);

  expect(d).toMatchObject({ base: 0, final: 0, cancel: false, prot: false })

  d.base = 128
  expect(d).toMatchObject({ base: 128, final: 0, cancel: false, prot: false })

  d.final = 42
  expect(d).toMatchObject({ base: 128, final: 42, cancel: false, prot: false })

  d.cancel = true;
  expect(d).toMatchObject({ base: 128, final: 42, cancel: true, prot: false })

  d.prot = true;
  expect(d).toMatchObject({ base: 128, final: 42, cancel: true, prot: true })


  let p: PowerStat = Object.setPrototypeOf(o, PowerStat.prototype);

  expect(p).toMatchObject({ base: 0, final: 0, cancel: false, prot: false })

  p.base = 128
  expect(p).toMatchObject({ base: 128, final: 0, cancel: false, prot: false })

  p.final = 42
  expect(p).toMatchObject({ base: 128, final: 42, cancel: false, prot: false })

  p.cancel = true;
  expect(p).toMatchObject({ base: 128, final: 42, cancel: true, prot: false })

  p.prot = true;
  expect(p).toMatchObject({ base: 128, final: 42, cancel: true, prot: true })


  d = Object.setPrototypeOf(o, DamageStat.prototype)
  expect(d).toMatchObject({ base: 128, final: 42, cancel: true, prot: true })

  const a: AttackStat = Object.setPrototypeOf(o, AttackStat.prototype);
  expect(a).toMatchObject({ base: 0, final: 0, cancel: false, prot: false })

  a.base = 128
  expect(a).toMatchObject({ base: 128, final: 0, cancel: false, prot: false })

  a.final = 42
  expect(a).toMatchObject({ base: 128, final: 42, cancel: false, prot: false })

  a.cancel = true;
  expect(a).toMatchObject({ base: 128, final: 42, cancel: true, prot: false })

  a.prot = true;
  expect(a).toMatchObject({ base: 128, final: 42, cancel: true, prot: true })


  d = Object.setPrototypeOf(o, DamageStat.prototype)
  expect(d).toMatchObject({ base: 128, final: 42, cancel: true, prot: true })

  p = Object.setPrototypeOf(o, PowerStat.prototype)
  expect(p).toMatchObject({ base: 128, final: 42, cancel: true, prot: true })

  const z: PillzStat = Object.setPrototypeOf(o, PillzStat.prototype);
  expect(z).toMatchObject({ cancel: false, prot: false })

  z.cancel = true
  expect(z).toMatchObject({ cancel: true, prot: false })

  z.prot = true
  expect(z).toMatchObject({ cancel: true, prot: true })

  const l: LifeStat = Object.setPrototypeOf(o, LifeStat.prototype);
  expect(l).toMatchObject({ cancel: false, prot: false })

  l.cancel = true
  expect(l).toMatchObject({ cancel: true, prot: false })

  l.prot = true
  expect(l).toMatchObject({ cancel: true, prot: true })
})