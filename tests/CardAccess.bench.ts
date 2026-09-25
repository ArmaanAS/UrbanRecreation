// Is the stat-view dispatch in CardTypes.ts costing anything?
//
//   deno bench -A --no-check tests/CardAccess.bench.ts
//
// The packing is not in question: one object per card holding two SMIs is what keeps a
// clone cheap and lets the search hold millions of nodes. The question is only how a stat
// view is selected. Four ways of reading the *same* packed layout are compared:
//
//   engine  what the engine does now - `BaseData.power` returns a throwaway PowerStat view
//           holding a reference to the data (src/game/types/CardTypes.ts)
//   swap    what the engine used to do - Object.setPrototypeOf(this, PowerStat.prototype),
//           reconstructed below because the engine no longer has it
//   wrap    the same throwaway-view design, as minimal standalone view classes declared
//           in this module
//   flat    distinct accessor names on one prototype, no view at all
//
// All four keep `a`/`b` and the bit layout byte for byte; only dispatch differs. The
// `engine` rows go through the real classes, so they measure whatever CardTypes.ts ships.
// (Until September 2026 the baseline was labelled `swap` but cast BaseData to PowerStat
// and called the engine's getters, so after the dispatch changed it silently measured the
// engine's views and the swap it named was not run at all.)
//
// `wrap` is the reference for `engine`. When the bench was fixed on 2026-09-25 the engine's
// views still inherited from an abstract BaseAttr -> BaseStat chain and ran ~35x slower
// than `wrap` (alternating 22.8 us vs 0.62 us per pass): every `super()` call went through
// V8's FindNonDefaultConstructorOrConstruct builtin, and the shared constructor's `d`
// store saw all seven view maps, so the view was never scalar-replaced. The real search
// paid for it too - making the views standalone took `deno task time-search` from 1795 to
// 1410 ms and `deno task time` from 18.9 to 16.4 s (medians, same results). After that
// change `engine` measured 1.2 us alternating and 0.76 us same-view, against 0.62 us and
// 0.50 us for `wrap` and `flat`. What remains is the cost of reaching an *exported* class
// binding, which is read through a module cell, rather than a module-local one: the same
// classes left unexported ran at 0.78 us. If `engine` drifts back towards `swap`,
// something defeats escape analysis again.
import { BaseData } from "@/game/types/CardTypes.ts";

// --- swap: the previous engine dispatch, one prototype per view, re-pointed per access ---
class SwapData {
  a = 0;
  b = 0;
  get power(): SwapPower {
    return Object.setPrototypeOf(this, SwapPower.prototype);
  }
  get damage(): SwapDamage {
    return Object.setPrototypeOf(this, SwapDamage.prototype);
  }
  get attack(): SwapAttack {
    return Object.setPrototypeOf(this, SwapAttack.prototype);
  }
}
class SwapPower extends SwapData {
  get final() {
    return this.a >> 5 & 0x1f;
  }
  set final(n: number) {
    this.a = (this.a & ~0x3e0) | ((n & 0x1f) << 5);
  }
  get blocked() {
    return (this.b >> 16 & 0b11) === 0b01;
  }
}
class SwapDamage extends SwapData {
  get final() {
    return this.a >> 15 & 0x1f;
  }
  set final(n: number) {
    this.a = (this.a & ~0xf8000) | ((n & 0x1f) << 15);
  }
  get blocked() {
    return (this.b >> 18 & 0b11) === 0b01;
  }
}
class SwapAttack extends SwapData {
  get final() {
    return this.b >> 8 & 0xff;
  }
  set final(n: number) {
    this.b = (this.b & ~0xff00) | ((n & 0xff) << 8);
  }
  get blocked() {
    return (this.b >> 20 & 0b11) === 0b01;
  }
}

// --- wrap: the engine's design, minus its class hierarchy ------------------------------
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
const engines = Array.from({ length: N }, () => new BaseData());
const swaps = Array.from({ length: N }, () => new SwapData());
const wraps = Array.from({ length: N }, () => new Packed());
const flats = Array.from({ length: N }, () => new Flat());
for (let i = 0; i < N; i++) {
  engines[i].a = swaps[i].a = wraps[i].a = flats[i].a = 0x2af5;
  engines[i].b = swaps[i].b = wraps[i].b = flats[i].b = 0x51234;
}

// Pattern 1: alternating views, as CardBattle does - read power, read damage, write attack.
Deno.bench({ name: "alternating views · engine", group: "alternating", baseline: true }, () => {
  let t = 0;
  for (const d of engines) {
    const p = d.power.final;
    const dm = d.damage.final;
    d.attack.final = p * 3 + dm;
    t += d.attack.final;
  }
  if (t < 0) throw new Error("no");
});

Deno.bench({ name: "alternating views · swap", group: "alternating" }, () => {
  let t = 0;
  for (const d of swaps) {
    const p = d.power.final;
    const dm = d.damage.final;
    d.attack.final = p * 3 + dm;
    t += d.attack.final;
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
Deno.bench({ name: "same view repeated · engine", group: "same", baseline: true }, () => {
  let t = 0;
  for (const d of engines) {
    if (!d.power.blocked) d.power.final = Math.min(d.power.final + 2, 20);
    t += d.power.final;
  }
  if (t < 0) throw new Error("no");
});

Deno.bench({ name: "same view repeated · swap", group: "same" }, () => {
  let t = 0;
  for (const d of swaps) {
    if (!d.power.blocked) d.power.final = Math.min(d.power.final + 2, 20);
    t += d.power.final;
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
Deno.bench({ name: "clone · engine", group: "clone", baseline: true }, () => {
  for (const d of engines) {
    const c = { ...d };
    if (c.a < 0) throw new Error("no");
  }
});
Deno.bench({ name: "clone · swap", group: "clone" }, () => {
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
