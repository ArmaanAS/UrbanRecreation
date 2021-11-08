import { Abilities, AbilityParser } from "./AbilityParser";
import BattleData from "./BattleData";
import Card from "./Card";
import Condition from "./Condition";
import BasicModifier from "./modifiers/BasicModifier";
import CancelModifier from "./modifiers/CancelModifier";
import CopyModifier from "./modifiers/CopyModifier";
import ExchangeModifier from "./modifiers/ExchangeModifier";
import Modifier from "./modifiers/Modifier";
import ProtectionModifier from "./modifiers/ProtectionModifier";
import { clone } from "./utils/Utils";



// const abilityType = {
//   UNDEFINED: 0,
//   GLOBAL: 1,
//   PREROUND: 2,
//   MODIFIER: 3,
//   POSTROUND: 4
// };

export enum AbilityType {
  UNDEFINED = 0,
  GLOBAL = 1,
  ABILITY = 2,
  BONUS = 3,
}

export default class Ability {
  ability: string;
  // type: number;
  type: AbilityType;
  defer = false;
  mods: Modifier[] = [];
  conditions: Condition[];
  constructor(s: string, type = AbilityType.UNDEFINED) {
    const conditions = Abilities.split(s);
    this.ability = conditions.pop()!;

    this.type = type;

    this.conditions = conditions
      .map(Condition.normalise)
      .map((s) => new Condition(s));
  }

  clone() {
    return Object.setPrototypeOf({
      // conditions: this.conditions.map(c => c.clone()),
      conditions: this.conditions.map(clone),
      ability: this.ability,

      type: this.type,
      defer: this.defer,
      // mods: this.mods.map(m => m.clone()),
      mods: this.mods.map(clone),
    }, Ability.prototype);
  }

  static from(o: Ability) {
    Object.setPrototypeOf(o, Ability.prototype);

    o.conditions = o.conditions.map(Condition.from);
    o.mods = o.mods.map(Modifier.from);

    return o;
  }

  canApply(data: BattleData) {
    // let apply = true;

    for (const cond of this.conditions) {
      if (!cond.met(data)) {
        console.log(`[Condition] ${cond.s} met: false`.yellow.dim);
        return false;
      }
      console.log(`[Condition] ${cond.s} met: true`.green);
    }

    if (this.type == AbilityType.ABILITY)
      // return !data.card.ability.blocked();
      // return data.card.ability.prot || !data.card.ability.cancel
      return !data.card.ability.blocked;
    else if (this.type == AbilityType.BONUS)
      // return !data.card.bonus.blocked();
      // return data.card.bonus.prot || !data.card.bonus.cancel;
      return !data.card.bonus.blocked;

    return true;

    // return apply;
  }

  compileConditions(data: BattleData) {
    for (const cond of this.conditions) {
      cond.compile(data, this);
    }
  }

  compileAbility(data: BattleData) {
    let failed = true;
    const tokens = this.ability.split(" ");

    compile: if (/\+\d+/.test(tokens[0])) {
      let t,
        i,
        opp = false;
      if (tokens[1] == "Opp") {
        t = [tokens[2]];
        i = 2;
        opp = true;
      } else if (tokens[1].includes("&")) {
        t = tokens[1].split("&");
        i = 1;
      } else {
        t = [tokens[1]];
        i = 1;
      }

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
          console.log(
            "Unknown token[1]:".red + '"' + tokens[1] + '"',
            this.ability
          );
          break compile;
        }

        AbilityParser.minmaxper(tokens, i + 1, mod);

        this.mods.push(mod);
      }
    } else if (/-\d+/.test(tokens[0])) {
      const dupe = ["Xantiax", "Cards"].includes(tokens[1]);
      let t: string[];
      const i = dupe ? 2 : 1;

      if (tokens[i].includes("&"))
        t = tokens[i].split("&");
      else
        t = [tokens[i]];

      for (const a of t) {
        // let mod = Modifier.basic(+tokens[0]);
        // let mod = new BasicModifier(+tokens[0]);
        const mod = new BasicModifier();
        mod.change = +tokens[0];
        mod.setOpp(true);
        failed = false;

        if (["Power", "Damage", "Life", "Pillz", "Attack"].includes(a)) {
          mod.setType(a);

        } else {
          console.log(
            `Unknown token[${i}]: `.red + `"${tokens[i]}"`,
            this.ability
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
          if (cloned instanceof BasicModifier)
            cloned.setOpp(false)

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
      if (tokens[1].includes("&")) {
        for (const prot of tokens[1].split("&")) {
          this.mods.push(new ProtectionModifier(prot));
        }
      } else {
        this.mods.push(new ProtectionModifier(tokens[1]));
      }
    } else if (tokens[0] == "Copy") {
      failed = false;

      if (tokens[1] == "Bonus") {
        // new Ability(data.oppCard.bonus.string, this.type).compile(data);
        new Ability(data.oppCard.bonusString, this.type).compile(data);

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
    } else if (tokens[0] == "No") {
      return;
    } else if (tokens[0] == "Counter-Attack") {
      return;
    }

    if (!failed) {
      console.log(`[Added] ${this.ability}`.green);
      // for (let mod of this.mods) {
      // for (let i in this.mods) {
      //   // if (this.type == Ability.Type.GLOBAL) {
      //   //   data.events.addGlobal(mod.eventTime, this.apply.bind(this, mod));
      //   // }
      //   data.events.add(mod.eventTime, this.);
      // }
      if (this.mods.length)
        data.events.add(this.mods[0].eventTime, this);
    } else {
      console.log(`[Failed] ${this.ability}`.red);
    }
  }

  compile(data: BattleData) {
    this.compileAbility(data);
    this.compileConditions(data);
  }

  // apply(mod, data) {
  apply(data: BattleData) {
    // let mod = this.mods[i];
    if (this.canApply(data)) {
      for (const mod of this.mods) {
        console.log(`Applying modifier... (${this.ability})`);
        mod.apply(data);
      }
    }
  }


  static card(card: Card, data: BattleData) {
    // new Ability(card.ability.string, AbilityType.ABILITY).compile(data);
    // new Ability(card.bonus.string, AbilityType.BONUS).compile(data);
    new Ability(card.abilityString, AbilityType.ABILITY).compile(data);
    new Ability(card.bonusString, AbilityType.BONUS).compile(data);
  }

  static leader(card: Card, data: BattleData) {
    // new Ability(card.ability.string, AbilityType.GLOBAL).compile(data);
    new Ability(card.abilityString, AbilityType.GLOBAL).compile(data);
  }
}