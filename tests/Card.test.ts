import { assertEquals } from "@std/assert";

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
    return this.a >> 2 & 0xff; // 0b1111111100;
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
    this.string = string;
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
Deno.test("CardStat", () => {
  const c = new CardStat(0);
  assertEquals(c.base, 0);
  assertEquals(c.final, 0);
  assertEquals(c.prot, false);
  assertEquals(c.cancel, false);

  c.base = 42;
  assertEquals(c.base, 42);
  assertEquals(c.final, 0);
  assertEquals(c.prot, false);
  assertEquals(c.cancel, false);

  c.final = 233;
  assertEquals(c.base, 42);
  assertEquals(c.final, 233);
  assertEquals(c.prot, false);
  assertEquals(c.cancel, false);

  c.prot = true;
  assertEquals(c.base, 42);
  assertEquals(c.final, 233);
  assertEquals(c.prot, true);
  assertEquals(c.cancel, false);

  c.cancel = true;
  assertEquals(c.base, 42);
  assertEquals(c.final, 233);
  assertEquals(c.prot, true);
  assertEquals(c.cancel, true);
});

Deno.test("CardString", () => {
  const c = new CardString("abcde");
  assertEquals(c.string, "abcde");
  assertEquals(c.prot, false);
  assertEquals(c.cancel, false);

  c.string = "xyz";
  assertEquals(c.string, "xyz");
  assertEquals(c.prot, false);
  assertEquals(c.cancel, false);

  c.prot = true;
  assertEquals(c.string, "xyz");
  assertEquals(c.prot, true);
  assertEquals(c.cancel, false);

  c.cancel = true;
  assertEquals(c.string, "xyz");
  assertEquals(c.prot, true);
  assertEquals(c.cancel, true);
});

Deno.test("DamageStat", () => {
  const o = { a: 0, b: 0 };
  let d: DamageStat = Object.setPrototypeOf(o, DamageStat.prototype);

  assertEquals(d.base, 0);
  assertEquals(d.final, 0);
  assertEquals(d.cancel, false);
  assertEquals(d.prot, false);

  d.base = 128;
  assertEquals(d.base, 128);
  assertEquals(d.final, 0);
  assertEquals(d.cancel, false);
  assertEquals(d.prot, false);

  d.final = 42;
  assertEquals(d.base, 128);
  assertEquals(d.final, 42);
  assertEquals(d.cancel, false);
  assertEquals(d.prot, false);

  d.cancel = true;
  assertEquals(d.base, 128);
  assertEquals(d.final, 42);
  assertEquals(d.cancel, true);
  assertEquals(d.prot, false);

  d.prot = true;
  assertEquals(d.base, 128);
  assertEquals(d.final, 42);
  assertEquals(d.cancel, true);
  assertEquals(d.prot, true);

  let p: PowerStat = Object.setPrototypeOf(o, PowerStat.prototype);

  assertEquals(p.base, 0);
  assertEquals(p.final, 0);
  assertEquals(p.cancel, false);
  assertEquals(p.prot, false);

  p.base = 128;
  assertEquals(p.base, 128);
  assertEquals(p.final, 0);
  assertEquals(p.cancel, false);
  assertEquals(p.prot, false);

  p.final = 42;
  assertEquals(p.base, 128);
  assertEquals(p.final, 42);
  assertEquals(p.cancel, false);
  assertEquals(p.prot, false);

  p.cancel = true;
  assertEquals(p.base, 128);
  assertEquals(p.final, 42);
  assertEquals(p.cancel, true);
  assertEquals(p.prot, false);

  p.prot = true;
  assertEquals(p.base, 128);
  assertEquals(p.final, 42);
  assertEquals(p.cancel, true);
  assertEquals(p.prot, true);

  d = Object.setPrototypeOf(o, DamageStat.prototype);
  assertEquals(d.base, 128);
  assertEquals(d.final, 42);
  assertEquals(d.cancel, true);
  assertEquals(d.prot, true);

  const a: AttackStat = Object.setPrototypeOf(o, AttackStat.prototype);
  assertEquals(a.base, 0);
  assertEquals(a.final, 0);
  assertEquals(a.cancel, false);
  assertEquals(a.prot, false);

  a.base = 128;
  assertEquals(a.base, 128);
  assertEquals(a.final, 0);
  assertEquals(a.cancel, false);
  assertEquals(a.prot, false);

  a.final = 42;
  assertEquals(a.base, 128);
  assertEquals(a.final, 42);
  assertEquals(a.cancel, false);
  assertEquals(a.prot, false);

  a.cancel = true;
  assertEquals(a.base, 128);
  assertEquals(a.final, 42);
  assertEquals(a.cancel, true);
  assertEquals(a.prot, false);

  a.prot = true;
  assertEquals(a.base, 128);
  assertEquals(a.final, 42);
  assertEquals(a.cancel, true);
  assertEquals(a.prot, true);

  d = Object.setPrototypeOf(o, DamageStat.prototype);
  assertEquals(d.base, 128);
  assertEquals(d.final, 42);
  assertEquals(d.cancel, true);
  assertEquals(d.prot, true);

  p = Object.setPrototypeOf(o, PowerStat.prototype);
  assertEquals(p.base, 128);
  assertEquals(p.final, 42);
  assertEquals(p.cancel, true);
  assertEquals(p.prot, true);

  const z: PillzStat = Object.setPrototypeOf(o, PillzStat.prototype);
  assertEquals(z.cancel, false);
  assertEquals(z.prot, false);

  z.cancel = true;
  assertEquals(z.cancel, true);
  assertEquals(z.prot, false);

  z.prot = true;
  assertEquals(z.cancel, true);
  assertEquals(z.prot, true);

  const l: LifeStat = Object.setPrototypeOf(o, LifeStat.prototype);
  assertEquals(l.cancel, false);
  assertEquals(l.prot, false);

  l.cancel = true;
  assertEquals(l.cancel, true);
  assertEquals(l.prot, false);

  l.prot = true;
  assertEquals(l.cancel, true);
  assertEquals(l.prot, true);
});

abstract class BaseAttr {
  protected a!: number;
  protected b!: number;
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
    this.b = (this.b & ~0x10000) | (+n << 16);
  }
  get prot(): boolean {
    return !!(this.b >> 17 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x20000) | (+n << 17);
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
    this.b = (this.b & ~0x40000) | (+n << 18);
  }
  get prot(): boolean {
    return !!(this.b >> 19 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x80000) | (+n << 19);
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
    this.b = (this.b & ~0x100000) | (+n << 20);
  }
  get prot(): boolean {
    return !!(this.b >> 21 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x200000) | (+n << 21);
  }
}
class PillzStat extends BaseAttr {
  get cancel(): boolean {
    return !!(this.b >> 22 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x400000) | (+n << 22);
  }
  get prot(): boolean {
    return !!(this.b >> 23 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x800000) | (+n << 23);
  }
}
class LifeStat extends BaseAttr {
  get cancel(): boolean {
    return !!(this.b >> 24 & 1);
  }
  set cancel(n: boolean) {
    this.b = (this.b & ~0x1000000) | (+n << 24);
  }
  get prot(): boolean {
    return !!(this.b >> 25 & 1);
  }
  set prot(n: boolean) {
    this.b = (this.b & ~0x2000000) | (+n << 25);
  }
}
