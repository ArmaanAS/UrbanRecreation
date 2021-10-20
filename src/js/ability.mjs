import Condition from "./conditions.mjs";
import Modifier from "./modifier.mjs";

let abilityCache = {};

class Abilities {
  static normalise(ability) {
    return ability
      .replace(/(?<=[\+\-]) (?=[xy])/gi, "")
      .replace(/[,\.]/g, "")
      .replace(/ :/g, ":");
  }

  static splitConditions(ability) {
    return ability.map((i) => i.split(/(?<=\w+) ?: /g));
  }

  static split(ability) {
    if (abilityCache.hasOwnProperty(ability)) {
      return Array.from(abilityCache[ability]);
    }

    let s = ability
      .replace(/(?<=[\+\-]) (?=[xy\d])/gi, "")
      .replace(/,(?! )/gi, " ")
      .replace(/[,\.]| (?=:)/gi, "")
      // .replace(/ :/g, ':')
      .replace(/At\w*/g, "Attack")
      .replace(/Prot\w*:?/g, "Protection")
      .replace(/Copy:/gi, "Copy")
      .replace("Xantiax:", "Xantiax")
      .replace(/(Dmg|Dam)\w*/gi, "Damage")
      .replace(/Pow\w*/gi, "Power")
      .replace(/Can\w*/gi, "Cancel")
      // .replace(/Mod\w*/ig, 'Modify')
      .replace(/Prot\w*/gi, "Protection")
      .replace(/Rec\w*/gi, "Recover")
      .replace(/&/g, "And")
      // .replace(/(?<=(Copy|Cancel|Stop|Per).*) (Opp|Mod|Left)\w*/gi, '')
      .replace(/(?<=(Copy|Cancel|Stop).*) (Opp|Mod|Left)\w*/gi, "")
      .replace("Bonus Protection", "Protection Bonus")
      // .replace(/([\w ]+) Exchange/ig, 'Exchange $1')
      .replace(/(\w+(?: \w+ \w+)?) ([+-][xy\d]+|Exchange)/i, "$2 $1")
      .replace(/([a-z]+)(?<!Min|Max) ([xy\d]+)/i, "$2 $1")
      .replace(/(\w+) And (\w+)/gi, "$1&$2")
      .replace(/(?<=[xy\d] )(\w+) (Opp)/gi, "$2 $1")
      .replace(/(?<=-[xy\d]+ )Opp /gi, "")
      .replace(/\b\w(?=\w+)/g, (s) => s.toUpperCase())
      .split(/(?<=\w+) ?[:;] /gi);

    abilityCache[ability] = s;
    return Array.from(s);
  }

  static getExecTime(s) {
    if (s.includes("mod")) {
      return execTime.pre;
    } else if (/.*(?<!per )(life|pill).*/gi.test(s)) {
      return execTime.post;
    } else if (/.*(?<!per )(d.?m|pow).*/gi.test(s)) {
      // +-
      return execTime.pre;
    } else if (s.includes("copy")) {
      return execTime.pre;
    }
  }
}

function splita(s) {
  conditions = Abilities.split(s);
  ability = conditions.pop();

  return conditions;
}

function splitb(s) {
  conditions = Abilities.split(s);
  ability = conditions.pop();

  return conditions;
}

// const abilityType = {
//   UNDEFINED: 0,
//   GLOBAL: 1,
//   PREROUND: 2,
//   MODIFIER: 3,
//   POSTROUND: 4
// };

class AbilityParser {
  static minmax(tokens, i, mod) {
    if (tokens[i] == "Min") {
      mod.setMin(+tokens[i + 1]);
      return true;
    } else if (tokens[i] == "Max") {
      mod.setMax(+tokens[i + 1]);
      return true;
    }

    return false;
  }

  static per(tokens, i, mod) {
    if (tokens[i] == "Per") {
      if (tokens[i + 1] == "Opp") {
        mod.setPer(tokens[i + 2], true);
      } else {
        mod.setPer(tokens[i + 1]);
      }
      return true;
    }

    return false;
  }

  static minmaxper(tokens, i, mod) {
    if (AbilityParser.minmax(tokens, i, mod)) {
      return true;
    } else if (AbilityParser.per(tokens, i, mod)) {
      AbilityParser.minmax(tokens, i + 2, mod);

      return true;
    }

    return false;
  }
}

export default class Ability {
  constructor(s, type) {
    this.conditions = Abilities.split(s);
    this.ability = this.conditions.pop();

    this.type = type || Ability.Type.UNDEFINED;
    this.defer = false;
    this.mods = [];

    this.conditions = this.conditions
      .map(Condition.normalise)
      .map((s) => new Condition(s));
  }

  clone() {
    return Object.setPrototypeOf({
        conditions: this.conditions.map((c) => c.clone()),
        ability: this.ability,

        type: this.type,
        defer: this.defer,
        mods: this.mods.map((m) => m.clone()),
      },
      Ability.prototype
    );
  }

  static from(o) {
    Object.setPrototypeOf(o, Ability.prototype);

    o.conditions = o.conditions.map(Condition.from);
    o.mods = o.mods.map(Modifier.from);

    return o;
  }

  canApply(data) {
    // let apply = true;

    for (let cond of this.conditions) {
      if (!cond.met(data)) {
        console.log(`[Condition] ${cond.s} met: false`.yellow);
        return false;
      }
      console.log(`[Condition] ${cond.s} met: true`.brightGreen);
    }

    if (this.type == Ability.Type.ABILITY) {
      return !data.card.ability.blocked();
    } else if (this.type == Ability.Type.BONUS) {
      return !data.card.bonus.blocked();
    }

    return true;

    // return apply;
  }

  compileConditions(data) {
    for (let cond of this.conditions) {
      cond.compile(data, this);
    }
  }

  compileAbility(data) {
    let failed = true;
    let tokens = this.ability.split(" ");

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

      for (let a of t) {
        let mod = Modifier.basic(+tokens[0]);
        mod.setOpp(opp);

        if (["Power", "Damage", "Life", "Pillz", "Attack"].includes(a)) {
          failed = false;

          mod.setType(a);
        } else {
          failed = true;
          console.log(
            "Unknown token[1]:".brightRed + '"' + tokens[1] + '"',
            this.ability
          );
          break compile;
        }

        AbilityParser.minmaxper(tokens, i + 1, mod);

        this.mods.push(mod);
      }
    } else if (/-\d+/.test(tokens[0])) {
      let dupe = ["Xantiax", "Cards"].includes(tokens[1]);
      let t,
        i = 1;
      if (dupe) {
        i = 2;
      }

      if (tokens[i].includes("&")) {
        t = tokens[i].split("&");
      } else {
        t = [tokens[i]];
      }

      for (let a of t) {
        let mod = Modifier.basic(+tokens[0]);
        mod.setOpp(true);

        if (["Power", "Damage", "Life", "Pillz", "Attack"].includes(a)) {
          failed = false;

          mod.setType(a);
        } else {
          failed = true;
          console.log(
            `Unknown token[${i}]: `.brightRed + '"' + tokens[i] + '"',
            this.ability
          );
          break compile;
        }

        AbilityParser.minmaxper(tokens, i + 1, mod);

        this.mods.push(mod);
      }

      if (dupe) {
        let newMods = [];
        for (let mod of this.mods) {
          mod.win = false;
          newMods.push(mod.clone().setOpp(false));
        }

        this.mods = [...this.mods, ...newMods];
      }
    } else if (tokens[0] == "Stop") {
      failed = false;
      this.mods.push(Modifier.cancel(tokens[1]));
    } else if (tokens[0] == "Cancel") {
      failed = false;
      if (tokens[1].includes("&")) {
        for (let t of tokens[1].split("&")) {
          this.mods.push(Modifier.cancel(t));
        }
      } else if (tokens[1] == "Leader") {
        return;
      } else {
        this.mods.push(Modifier.cancel(tokens[1]));
      }
    } else if (tokens[0] == "Protection") {
      failed = false;
      if (tokens[1].includes("&")) {
        for (let prot of tokens[1].split("&")) {
          this.mods.push(Modifier.prot(prot));
        }
      } else {
        this.mods.push(Modifier.prot(tokens[1]));
      }
    } else if (tokens[0] == "Copy") {
      failed = false;

      if (tokens[1] == "Bonus") {
        new Ability(data.oppCard.bonus.string, this.type).compile(data);

        return;
      } else {
        if (tokens[1].includes("&")) {
          for (let c of tokens[1].split("&")) {
            this.mods.push(Modifier.copy(c));
          }
        } else {
          this.mods.push(Modifier.copy(tokens[1]));
        }
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
      if (this.mods.length) {
        data.events.add(this.mods[0].eventTime, this);
      }
    } else {
      console.log(`[Failed] ${this.ability}`.red);
    }
  }

  compile(data) {
    this.compileAbility(data);
    this.compileConditions(data);
  }

  // apply(mod, data) {
  apply(data) {
    // let mod = this.mods[i];
    if (this.canApply(data)) {
      for (let mod of this.mods) {
        console.log(`Applying modifier... (${this.ability})`);
        mod.apply(data);
      }
    }
  }

  static Type = {
    UNDEFINED: 0,
    GLOBAL: 1,
    ABILITY: 2,
    BONUS: 3,
  };

  static card(card, data) {
    new Ability(card.ability.string, Ability.Type.ABILITY).compile(data);
    new Ability(card.bonus.string, Ability.Type.BONUS).compile(data);
  }

  static leader(card, data) {
    new Ability(card.ability.string, Ability.Type.GLOBAL).compile(data);
  }
}

// module.exports = Ability;