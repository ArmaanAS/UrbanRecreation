import { Abilities, AbilityParser } from "./AbilityParser.ts";
import BattleData from "./battle/BattleData.ts";
import CachedBattleData from "./battle/CachedBattleData.ts";
import CachedEvents from "./battle/CachedEvents.ts";
import Card from "./Card.ts";
import Condition from "./Condition.ts";
import BasicModifier from "./modifiers/BasicModifier.ts";
import CancelModifier from "./modifiers/CancelModifier.ts";
import CopyModifier from "./modifiers/CopyModifier.ts";
import ExchangeModifier from "./modifiers/ExchangeModifier.ts";
import Modifier from "./modifiers/Modifier.ts";
import ProtectionModifier from "./modifiers/ProtectionModifier.ts";
import RecoverModifier from "./modifiers/RecoverModifier.ts";
import EventTime from "./types/EventTime.ts";
import { clone } from "../utils/Utils.ts";
import { DEBUG } from "../utils/Debug.ts";
export enum AbilityType {

  UNDEFINED = 0,
  GLOBAL = 1,
  ABILITY = 2,
  BONUS = 3,
  GLOBAL_ABILITY = 4,
  GLOBAL_BONUS = 5,
}

// const abilityCache: string[] = [];
// const abilityIndexCache: {[key: string]: number} = {};

export default class Ability {
  ability: string;
  // _ability: number;
  type: AbilityType;
  // defer = false;
  mods: Modifier[] = [];
  conditions: Condition[];
  delayed?: boolean = undefined;
  won?: boolean = undefined;
  /**
   * Whether the printed text names the opponent. Normalising drops the "Opp" after a
   * negative number, so it has to be read from the raw text: only compile() uses it.
   */
  private namesOpp: boolean;
  constructor(s: string, type = AbilityType.UNDEFINED) {
    this.namesOpp = /\bOpp/i.test(s);
    const conditions = Abilities.split(s);
    this.ability = conditions.pop()!;
    // const ability = conditions.pop()!;

    this.type = type;

    this.conditions = conditions
      .map(Condition.normalise)
      .map((s) => new Condition(s));
  }

  /**
   * A permanent latches by writing `won` on itself and a delayed one clears `delayed`, so
   * both carry state that belongs to one line of play and have to be copied. Everything
   * else is fixed once compiled and can be shared, which matters: the solver merges these
   * into millions of `Events`.
   *
   * Sharing a permanent used to leak its latch across the whole search tree. Only Poison,
   * Heal and Combust set `delayed`, so Toxin / Consume / Regen / Dope / Repair / Mindwipe
   * fell through to `return this`, and the first branch that won with one set `won = true`
   * on the single shared instance - after which every other branch, including the ones
   * where that card lost its round, applied the effect anyway.
   */
  clone() {
    if (this.delayed === undefined && !this.permanent) return this;

    return Object.setPrototypeOf({
      // conditions: this.conditions.map(c => c.clone()),
      conditions: this.conditions.map(clone),
      ability: this.ability,

      type: this.type,
      // defer: this.defer,
      // mods: this.mods.map(m => m.clone()),
      mods: this.mods.map(clone),
      delayed: this.delayed,
      won: this.won,
    }, Ability.prototype);
  }

  static from(o: Ability) {
    Object.setPrototypeOf(o, Ability.prototype);

    o.conditions = o.conditions.map(Condition.from);
    // o.mods = o.mods.map(Modifier.from);
    o.mods = o.mods.map((m) => BasicModifier.from(m as BasicModifier));

    return o;
  }

  /** Poison / Heal / Toxin / Repair and friends: latch once, then repeat every later round. */
  private get permanent() {
    return this.type === AbilityType.GLOBAL_ABILITY ||
      this.type === AbilityType.GLOBAL_BONUS;
  }

  /**
   * Whether a permanent effect starts at all. It latches in the round its card is played,
   * so the conditions describe that one round; unless one of them already pins the outcome
   * (Defeat, Victory Or Defeat and Reanimate all clear `win` on the modifier) the card has
   * to have won. Frogo's "Defeat : Heal 1 Max. 13" heals from the round after it loses -
   * requiring a win here discarded the effect instead (captured battle 875230).
   */
  private latches(data: BattleData) {
    // A stopped ability never starts at all: Pr Balthazar's "Stop Opp. Ability" in captured
    // battle 877733 r0 keeps Lianah Ld's "Heal 1 Max. 20" from ever healing. It has to be
    // checked here and not on each application, because from the next round on `data.card`
    // is whichever card the owner plays then, not the one that started the effect.
    if (this.type === AbilityType.GLOBAL_ABILITY && data.card.ability.blocked) {
      return false;
    }
    if (this.type === AbilityType.GLOBAL_BONUS && data.card.bonus.blocked) {
      return false;
    }

    for (const cond of this.conditions) {
      if (!cond.met(data)) {
        if (DEBUG) console.log(`[Condition] ${cond.s} met: false`.yellow.dim);
        return false;
      }
      if (DEBUG) console.log(`[Condition] ${cond.s} met: true`.green);
    }

    return this.mods[0].win === false || data.player.won === true;
  }

  canApply(data: BattleData) {
    // let apply = true;

    // Permanents latch once and then repeat at the end of every later round, so the
    // conditions gate the latch only.
    if (this.permanent) {
      if (this.won === undefined) {
        if (!this.latches(data)) {
          data.events.removeGlobal(this.mods[0].eventTime, this);
          return false;
        }

        this.won = true;

        // A Growth permanent's amount is fixed by the round it latches in, not rescaled by
        // every later round: Abby Salia's "Growth: Heal 1 Max. 12" wins round 2 of captured
        // battle 1414168 and heals 2 in rounds 3 and 4 alike, as the server's own text says
        // ("multiplied by the number of the round in which Abby Salia has won"). These mods
        // are this entry's own copies (see `clone`), and unmake drops the entry that latched.
        for (const mod of this.mods) {
          if (mod instanceof BasicModifier && (mod.per?.type === 7 || mod.per?.type === 8)) {
            mod.change *= mod.getMultiplier(data);
            mod.per = undefined;
          }
        }
      }

      if (this.delayed) {
        this.delayed = false;
        return false;
      }

      // Latched, so it repeats whatever happens from here; the blocked checks below would
      // be asking about the wrong card.
      return true;
    }

    if (this.delayed) {
      this.delayed = false;
      return false;
    }

    for (const cond of this.conditions) {
      if (!cond.met(data)) {
        if (DEBUG) console.log(`[Condition] ${cond.s} met: false`.yellow.dim);
        return false;
      }
      if (DEBUG) console.log(`[Condition] ${cond.s} met: true`.green);
    }

    if (
      this.type === AbilityType.ABILITY ||
      this.type === AbilityType.GLOBAL_ABILITY
    ) {
      // return !data.card.ability.blocked();
      // return data.card.ability.prot || !data.card.ability.cancel
      return !data.card.ability.blocked;
    } else if (
      this.type === AbilityType.BONUS ||
      this.type === AbilityType.GLOBAL_BONUS
    ) {
      // return !data.card.bonus.blocked();
      // return data.card.bonus.prot || !data.card.bonus.cancel;
      return !data.card.bonus.blocked;
    }

    return true;

    // return apply;
  }

  compileConditions(data: BattleData | CachedBattleData) {
    for (const cond of this.conditions) {
      try {
        cond.compile(data, this);
      } catch { /* */ }
    }
  }

  compileAbility(data: BattleData | CachedBattleData) {
    let failed = true;
    const tokens = this.ability.split(" ");

    if (
      tokens[0] == "No" ||
      tokens[0] == "Counter-Attack" ||
      tokens[1] == "Xp%"
    ) {
      return;
    }

    compile:
    if (/\+\d+/.test(tokens[0])) {
      // "Players" and "Cards" both mean the modifier lands once on each side. "Cards Damage
      // +2" normalises to "+2 Cards Damage" and is "The Damage points of both characters are
      // increased by 2" (captures/abilities.json 3295, sideAffected "both"); 874795 r0 has
      // El Resbaladizo lv4 at 6+2 and Aurora lv5 at 5+2 in the same round. The negative
      // branch below already reads "Cards" this way for "-2 Cards Damage, Min 1".
      const dupe = tokens[1] === "Players" || tokens[1] === "Cards";
      let t: string[],
        i: number,
        opp1 = false;
      if (dupe) {
        t = [tokens[2]];
        i = 2;
      } else if (tokens[1] === "Opp") {
        t = [tokens[2]];
        i = 2;
        opp1 = true;
      } else if (tokens[1].includes("&")) {
        t = tokens[1].split("&");
        i = 1;
      } else {
        t = [tokens[1]];
        i = 1;
      }

      for (const opp of dupe ? [true, false] : [opp1]) {
        for (const a of t) {
          // let mod = Modifier.basic(+tokens[0]);
          // let mod = new BasicModifier(+tokens[0]);
          const mod = new BasicModifier();
          mod.change = +tokens[0];
          mod.setOpp(opp);

          if (["Power", "Damage", "Life", "Pillz", "Attack"].includes(a)) {
            failed = false;

            mod.setType(a);
          } else {
            failed = true;
            if (DEBUG) console.log(
              "Unknown token[1]:".red + '"' + tokens[1] + '"',
              this.ability,
            );
            break compile;
          }

          AbilityParser.minmaxper(tokens, i + 1, mod);

          this.mods.push(mod);
        }
      }
    } else if (/-\d+/.test(tokens[0])) {
      const dupe = ["Xantiax", "Cards"].includes(tokens[1]);
      let t: string[];
      const i = dupe ? 2 : 1;

      if (tokens[i].includes("&")) {
        t = tokens[i].split("&");
      } else {
        t = [tokens[i]];
      }

      for (const a of t) {
        // let mod = Modifier.basic(+tokens[0]);
        // let mod = new BasicModifier(+tokens[0]);
        const mod = new BasicModifier();
        mod.change = +tokens[0];
        // A reduction hits the opposing card unless its text says otherwise. Bugamon's
        // "Growth: -1 Power And Damage, Min 4" names no opponent and lowers Bugamon's own
        // Power and Damage (captured 1414749 r1 and 1088641 r0, sideAffected "player"). It
        // is the only such combat-stat text in the card data; Life and Pillz keep aiming at
        // the opponent, and Backlash turns itself back to the owner in Condition.compile.
        mod.setOpp(
          dupe || this.namesOpp || !["Power", "Damage", "Attack"].includes(a),
        );
        failed = false;

        if (["Power", "Damage", "Life", "Pillz", "Attack"].includes(a)) {
          mod.setType(a);
        } else {
          if (DEBUG) console.log(
            `Unknown token[${i}]: `.red + `"${tokens[i]}"`,
            this.ability,
          );
          break compile;
        }

        AbilityParser.minmaxper(tokens, i + 1, mod);

        this.mods.push(mod);
      }

      if (dupe) {
        const newMods = [];
        for (const mod of this.mods) {
          mod.win = false;
          // const clone = mod.clone();
          const cloned = clone(mod);
          if (cloned instanceof BasicModifier) {
            cloned.setOpp(false);
          }

          newMods.push(cloned);
        }

        this.mods = [...this.mods, ...newMods];
      }
    } else if (tokens[0] == "Stop") {
      failed = false;
      // this.mods.push(Modifier.cancel(tokens[1]));
      this.mods.push(new CancelModifier(tokens[1]));
    } else if (tokens[0] == "Cancel") {
      failed = false;
      if (tokens[1].includes("&")) {
        for (const t of tokens[1].split("&")) {
          this.mods.push(new CancelModifier(t));
        }
      } else if (tokens[1] == "Leader") {
        return;
      } else {
        this.mods.push(new CancelModifier(tokens[1]));
      }
    } else if (tokens[0] == "Protection") {
      failed = false;
      let i = 1;
      let both = false;

      if (tokens[1] == "Cards") {
        i = 2;
        both = true;
      }

      if (tokens[i].includes("&")) {
        // Only the plain, unconditional "Protection: Power And Damage" refuses opposing
        // reductions (see ProtectionModifier.guard); the Reprisal, Revenge, Courage and
        // Cards forms have no captured round showing it and stay Cancel-only.
        const guard = !both && tokens[i] === "Power&Damage" &&
          this.conditions.length === 0;
        for (const prot of tokens[i].split("&")) {
          const mod = new ProtectionModifier(prot, both);
          mod.guard = guard;
          this.mods.push(mod);
        }
      } else {
        this.mods.push(new ProtectionModifier(tokens[i], both));
      }
    } else if (tokens[0] == "Copy") {
      failed = false;

      // console.log(
      //   `${data.card.name} vs ${data.oppCard.name}`,
      //   this.ability,
      //   this.type,
      // );

      if (tokens[1] == "Bonus") {
        if (/Copy.+Bonus/i.test(data.oppCard.bonusString)) {
          if (DEBUG) console.log("Copy Bonus loop detected. Skipping...");
          return;
        } else if (
          /Copy.+Ability/i.test(data.oppCard.bonusString) &&
          /Copy.+Bonus/i.test(data.oppCard.abilityString)
        ) {
          if (DEBUG) console.log("Copy Ability and Bonus loop detected. Skipping...");
          return;
        }
        if (DEBUG) console.log("Copying", data.oppCard.bonusString);
        // new Ability(data.oppCard.bonus.string, this.type).compile(data);
        new Ability(data.oppCard.bonusString, this.type).compile(data);

        return;
      } else if (tokens[1] == "Ability") {
        if (/Copy.+Ability/i.test(data.oppCard.abilityString)) {
          if (DEBUG) console.log("Copy Ability loop detected. Skipping...");
          return;
        } else if (
          /Copy.+Bonus/i.test(data.oppCard.abilityString) &&
          /Copy.+Ability/i.test(data.oppCard.bonusString)
        ) {
          if (DEBUG) console.log("Copy Ability and Bonus loop detected. Skipping...");
          return;
        }
        if (DEBUG) console.log("Copying", data.oppCard.abilityString);
        new Ability(data.oppCard.abilityString, this.type).compile(data);

        return;
      } else {
        if (tokens[1].includes("&")) {
          for (const c of tokens[1].split("&")) {
            this.mods.push(new CopyModifier(c));
          }
        } else {
          this.mods.push(new CopyModifier(tokens[1]));
        }
      }
    } else if (tokens[0] == "Exchange") {
      failed = false;
      if (tokens[1].includes("&")) {
        for (const prot of tokens[1].split("&")) {
          this.mods.push(new ExchangeModifier(prot));
        }
      } else {
        this.mods.push(new ExchangeModifier(tokens[1]));
      }
    } else if (tokens[0] == "Impose") {
      failed = false;
      if (tokens[1].includes("&")) {
        for (const prot of tokens[1].split("&")) {
          this.mods.push(new ExchangeModifier("Impose " + prot));
        }
      } else {
        this.mods.push(new ExchangeModifier("Impose " + tokens[1]));
      }
    } else if (tokens[1] == "Recover") {
      failed = false;
      // "Pillz", 1, 3
      if (tokens[2] == "Players") { // 1 Recover Players Pillz Out Of 3
        this.mods.push(
          new RecoverModifier(tokens[3], +tokens[0], +tokens[6], true),
        );
      } else { // 1 Recover Pillz Out Of 3
        this.mods.push(
          new RecoverModifier(tokens[2], +tokens[0], +tokens[5], false),
        );
      }
    } else if (tokens[0] === "Tune") {
      failed = false;
      const mod = new BasicModifier();
      mod.setType("TUNEOUT");

      this.mods.push(mod);
    } else if (tokens[0] == "Sinister" && tokens[1] == "Symmetry") {
      // captures/abilities.json 4303 (Karkass Cr): win the round against the card opposite
      // this one (indexRequirement "symmetry") and the opponent loses the match outright.
      // The server sends specialAction "ko" and takes their Life to 0, whatever is left of
      // it - captured battle 874399 r3 has them on 9 Life, takes 4 as damage, then reports
      // a post-round life decrease of exactly the remaining 5.
      failed = false;

      const mod = new BasicModifier();
      mod.setType("Life");
      mod.setOpp(true);
      mod.change = -Infinity; // "all of it", clamped by the Min below
      mod.setMin(0);

      this.mods.push(mod);
      this.conditions.push(new Condition("Symmetry"));
    } else if (
      ["Poison", "Toxin", "Consume", "Regen", "Heal", "Dope"].includes(
        tokens[1],
      )
    ) {
      // 2 Poison Min 2
      failed = false;

      if (this.type === AbilityType.ABILITY) {
        this.type = AbilityType.GLOBAL_ABILITY;
      } else if (this.type === AbilityType.BONUS) {
        this.type = AbilityType.GLOBAL_BONUS;
      }

      const mod = new BasicModifier();
      mod.eventTime = EventTime.END;
      mod.always = true;

      const type = tokens[1];
      if (type == "Dope" || type == "Consume") {
        mod.setType("Pillz");
      } else {
        mod.setType("Life");
      }

      if (type == "Regen" || type == "Heal" || type == "Dope") {
        mod.setOpp(false);
        mod.change = +tokens[0];
      } else {
        mod.setOpp(true);
        mod.change = -tokens[0];
      }

      if (type == "Poison" || type == "Heal") {
        this.delayed = true;
      }

      if (tokens[2] == "Min") {
        mod.setMin(+tokens[3]);
      } else if (tokens[2] == "Max") {
        mod.setMax(+tokens[3]);
      }

      this.mods.push(mod);
    } else if (tokens[1] == "Repair") {
      // "Repair 1, Max. 14" normalises to "1 Repair Max 14": if the card wins its round its
      // owner gains N Life *and* N Pillz at the end of that round and of every following
      // one, each capped at M. The server sends it as isPermanent + isImmediatePermanent
      // with currentRoundRequirement "win" (captures/abilities.json 3796, Wilo Ld), so
      // unlike Poison it is not delayed past the round that started it.
      failed = false;

      if (this.type === AbilityType.ABILITY) {
        this.type = AbilityType.GLOBAL_ABILITY;
      } else if (this.type === AbilityType.BONUS) {
        this.type = AbilityType.GLOBAL_BONUS;
      }

      for (const type of ["Life", "Pillz"]) {
        const mod = new BasicModifier();
        mod.eventTime = EventTime.END;
        mod.setType(type);
        mod.change = +tokens[0];
        mod.always = true;
        if (tokens[2] == "Max") {
          mod.setMax(+tokens[3]);
        } else if (tokens[2] == "Min") {
          mod.setMin(+tokens[3]);
        }

        this.mods.push(mod);
      }
    } else if (tokens[1] == "Combust" || tokens[1] == "Mindwipe") {
      failed = false;

      if (this.type === AbilityType.ABILITY) {
        this.type = AbilityType.GLOBAL_ABILITY;
      } else if (this.type === AbilityType.BONUS) {
        this.type = AbilityType.GLOBAL_BONUS;
      }

      if (tokens[1] == "Combust") {
        this.delayed = true;
      }

      for (const type of ["Life", "Pillz"]) {
        const mod = new BasicModifier();
        mod.eventTime = EventTime.END;
        mod.setType(type);
        mod.setOpp(true);
        mod.change = -tokens[0];
        mod.always = true;
        if (tokens[2] == "Min") {
          mod.setMin(+tokens[3]);
        }

        this.mods.push(mod);
      }
    }

    if (!failed) {
      if (DEBUG) console.log(`[Added] ${this.ability}\n`.green);
      // for (let mod of this.mods) {
      // for (let i in this.mods) {
      //   // if (this.type == Ability.Type.GLOBAL) {
      //   //   data.events.addGlobal(mod.eventTime, this.apply.bind(this, mod));
      //   // }
      //   data.events.add(mod.eventTime, this.);
      // }
      if (this.mods.length) {
        switch (this.type) {
          case AbilityType.ABILITY:
          case AbilityType.BONUS:
            data.events.add(this.mods[0].eventTime, this);
            break;
          case AbilityType.GLOBAL:
          case AbilityType.GLOBAL_ABILITY:
          case AbilityType.GLOBAL_BONUS:
            data.events.addGlobal(this.mods[0].eventTime, this);
            break;
        }
      }
    } else {
      if (DEBUG) console.log(`[Failed] ${this.ability}`.red);
    }
  }

  compile(data: BattleData | CachedBattleData) {
    this.compileAbility(data);
    this.compileConditions(data);
  }

  // apply(mod, data) {
  apply(data: BattleData) {
    // let mod = this.mods[i];
    if (this.canApply(data)) {
      for (const mod of this.mods) {
        if (DEBUG) console.log(`Applying modifier... (${this.ability})`);
        mod.apply(data);
      }
    }
  }

  static card(card: Card, data: BattleData | CachedBattleData) {
    // Within a phase the server resolves the clan bonus before the ability, e.g. Don Cr
    // (bonus -12 Opp Attack Min 8, ability -4 Opp Attack Min 2) vs 18 attack → 8 → 4, not
    // 14 → 2 → Min 8. Seen in captured battles 875272 and 901613.
    new Ability(card.bonusString, AbilityType.BONUS).compile(data);
    new Ability(card.abilityString, AbilityType.ABILITY).compile(data);
  }

  static leader(card: Card, data: BattleData | CachedBattleData) {
    new Ability(card.abilityString, AbilityType.GLOBAL).compile(data);
  }
}
