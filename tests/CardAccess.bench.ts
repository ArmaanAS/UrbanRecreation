// Is the prototype-swap dispatch in CardTypes.ts costing anything?
//
//   deno bench -A --no-check tests/CardAccess.bench.ts
//
// The packing is not in question: one object per card holding two SMIs is what keeps a
// clone cheap and lets the search hold millions of nodes. The question is only how a stat
// view is selected. Three ways of reading the *same* packed layout are compared:
//
//   swap    what the engine does now - Object.setPrototypeOf(this, PowerStat.prototype)
//   wrap    a throwaway view object holding a reference to the data
//   flat    distinct accessor names on one prototype, no view at all
//
// Both alternatives keep `a`/`b` and the bit layout byte for byte; only dispatch differs.
import { AttackStat, BaseData, DamageStat, PowerStat } from "@/game/types/CardTypes.ts";

// --- wrap: one prototype per view, but the view points at the data ----------------------
class Packed {
  a = 0;
  b = 0;
  get power() {
    return new PowerView(this);
  }
  get damage() {
    return new DamageView(this);
  }
  get attack() {
    return new AttackView(this);
  }
}
class PowerView {
  constructor(private d: Packed) {}
  get final() {
    return this.d.a >> 5 & 0x1f;
  }
  set final(n: number) {
    this.d.a = (this.d.a & ~0x3e0) | ((n & 0x1f) << 5);
  }
  get blocked() {
    return (this.d.b >> 16 & 0b11) === 0b01;
  }
}
class DamageView {
  constructor(private d: Packed) {}
  get final() {
    return this.d.a >> 15 & 0x1f;
  }
  set final(n: number) {
    this.d.a = (this.d.a & ~0xf8000) | ((n & 0x1f) << 15);
  }
  get blocked() {
    return (this.d.b >> 18 & 0b11) === 0b01;
  }
}
class AttackView {
  constructor(private d: Packed) {}
  get final() {
    return this.d.b >> 8 & 0xff;
  }
  set final(n: number) {
    this.d.b = (this.d.b & ~0xff00) | ((n & 0xff) << 8);
  }
  get blocked() {
    return (this.d.b >> 20 & 0b11) === 0b01;
  }
}

// --- flat: the same bits, reached by name -----------------------------------------------
class Flat {
  a = 0;
  b = 0;
  get powerFinal() {
    return this.a >> 5 & 0x1f;
  }
  set powerFinal(n: number) {
    this.a = (this.a & ~0x3e0) | ((n & 0x1f) << 5);
  }
  get powerBlocked() {
    return (this.b >> 16 & 0b11) === 0b01;
  }
  get damageFinal() {
    return this.a >> 15 & 0x1f;
  }
  set damageFinal(n: number) {
    this.a = (this.a & ~0xf8000) | ((n & 0x1f) << 15);
  }
  get damageBlocked() {
    return (this.b >> 18 & 0b11) === 0b01;
  }
  get attackFinal() {
    return this.b >> 8 & 0xff;
  }
  set attackFinal(n: number) {
    this.b = (this.b & ~0xff00) | ((n & 0xff) << 8);
  }
}

// Many cards, so the access sites see the same spread of objects the solver gives them.
const N = 256;
const swaps = Array.from({ length: N }, () => new BaseData());
const wraps = Array.from({ length: N }, () => new Packed());
const flats = Array.from({ length: N }, () => new Flat());
for (let i = 0; i < N; i++) {
  swaps[i].a = flats[i].a = wraps[i].a = 0x2af5;
  swaps[i].b = flats[i].b = wraps[i].b = 0x51234;
}

// Pattern 1: alternating views, as CardBattle does - read power, read damage, write attack.
Deno.bench({ name: "alternating views · swap", group: "alternating", baseline: true }, () => {
  let t = 0;
  for (const d of swaps) {
    const p = (d as unknown as PowerStat).power.final;
    const dm = (d as unknown as DamageStat).damage.final;
    (d as unknown as AttackStat).attack.final = p * 3 + dm;
    t += (d as unknown as AttackStat).attack.final;
  }
  if (t < 0) throw new Error("no");
});

Deno.bench({ name: "alternating views · wrap", group: "alternating" }, () => {
  let t = 0;
  for (const d of wraps) {
    const p = d.power.final;
    const dm = d.damage.final;
    d.attack.final = p * 3 + dm;
    t += d.attack.final;
  }
  if (t < 0) throw new Error("no");
});

Deno.bench({ name: "alternating views · flat", group: "alternating" }, () => {
  let t = 0;
  for (const d of flats) {
    const p = d.powerFinal;
    const dm = d.damageFinal;
    d.attackFinal = p * 3 + dm;
    t += d.attackFinal;
  }
  if (t < 0) throw new Error("no");
});

// Pattern 2: repeated same view, as a modifier does - read power, clamp, write power back.
Deno.bench({ name: "same view repeated · swap", group: "same", baseline: true }, () => {
  let t = 0;
  for (const d of swaps) {
    const s = d as unknown as PowerStat;
    if (!s.power.blocked) s.power.final = Math.min(s.power.final + 2, 20);
    t += s.power.final;
  }
  if (t < 0) throw new Error("no");
});

Deno.bench({ name: "same view repeated · wrap", group: "same" }, () => {
  let t = 0;
  for (const d of wraps) {
    if (!d.power.blocked) d.power.final = Math.min(d.power.final + 2, 20);
    t += d.power.final;
  }
  if (t < 0) throw new Error("no");
});

Deno.bench({ name: "same view repeated · flat", group: "same" }, () => {
  let t = 0;
  for (const d of flats) {
    if (!d.powerBlocked) d.powerFinal = Math.min(d.powerFinal + 2, 20);
    t += d.powerFinal;
  }
  if (t < 0) throw new Error("no");
});

// Pattern 3: cloning, which is the other thing the packing buys - one object, two SMIs.
Deno.bench({ name: "clone · swap", group: "clone", baseline: true }, () => {
  for (const d of swaps) {
    const c = { ...d };
    if (c.a < 0) throw new Error("no");
  }
});
Deno.bench({ name: "clone · wrap", group: "clone" }, () => {
  for (const d of wraps) {
    const c = { ...d };
    if (c.a < 0) throw new Error("no");
  }
});
