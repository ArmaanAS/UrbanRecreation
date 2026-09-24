import BasicModifier from "./modifiers/BasicModifier.ts";

const abilityCache = new Map<string, string[]>();

export class Abilities {
  // static normalise(ability: string) {
  //   return ability
  //     .replace(/(?<=[+-]) (?=[xy])/gi, "")
  //     .replace(/[,.]/g, "")
  //     .replace(/ :/g, ":");
  // }
  static abilityStringCache = new Map<string, string>();
  static normalise(ability: string) {
    if (this.abilityStringCache.has(ability)) {
      return this.abilityStringCache.get(ability)!;
    }

    const norm = ability
      .replace(/(?<=[+-]) (?=[xy\d])/gi, "")
      .replace(/,(?! )/gi, " ")
      .replace(/[,.]| (?=:)/gi, "")
      .replace(/At\w*/g, "Attack")
      .replace(/Prot\w*:?/g, "Protection")
      .replace(/Copy:/gi, "Copy")
      .replace("Xantiax:", "Xantiax")
      .replace(/(Dmg|Dam)\w*/gi, "Damage")
      .replace(/Pow\w*/gi, "Power")
      .replace(/Can\w*/gi, "Cancel")
      .replace(/Prot\w*/gi, "Protection")
      .replace(/Rec\w*/gi, "Recover")
      // Abbreviated condition names ("Asymm.", "Asy.", "Repris.") lose their "." above and
      // are expanded by Condition.normalise once the conditions are split off.
      .replace(/ ?[&/] ?/g, " And ")
      .replace(/(?<=(Copy|Cancel|Stop).*) (Opp|Mod|Left)\w*/gi, "")
      .replace(/(?<=Per.*) Left\w*/gi, "")
      .replace("Bonus Protection", "Protection Bonus")
      .replace(/^(.+) Impose/, "Impose $1")
      .replace(/(\w+(?: \w+ \w+)?) ([+-][xy\d]+|Exchange)/i, "$2 $1")
      // The swap above only takes one or three words, not two, so "Cards Damage +2"
      // comes out as "Cards +2 Damage". Put the sign back in front so it parses like any
      // other modifier, with "Cards" left in front of the stat to mark both sides - the
      // shape "-2 Cards Damage Min 1" already has.
      .replace(/^(Cards|Players) ([+-][xy\d]+)/, "$2 $1")
      .replace(/([a-z]+)(?<!Min|Max) ([xy\d]+)/i, "$2 $1")
      .replace(/(\w+) And (\w+)/gi, "$1&$2")
      .replace(/(?<=[xy\d] )(\w+) (Opp)/gi, "$2 $1")
      .replace(/(?<=-[xy\d]+ )Opp /gi, "")
      .replace(/\b\w(?=\w+)/g, (s) => s.toUpperCase())
      .replace(/\[star]/gi, "★");

    this.abilityStringCache.set(ability, norm);

    return norm;
  }

  private static splitConditions(normalisedAbility: string) {
    return normalisedAbility
      .split(/(?<=\w+) ?[:;] |(?<=\[Clan:\d+\]):? (?!:)/gi);
  }

  static split(ability: string) {
    if (abilityCache.has(ability)) {
      return [...abilityCache.get(ability)!];
    }

    const normalised = this.normalise(ability);
    const splitConditions = this.splitConditions(normalised);

    abilityCache.set(ability, splitConditions);
    return [...splitConditions];
  }
}

// function splita(s: string) {
//   const conditions = Abilities.split(s);
//   const ability = conditions.pop();

//   return conditions;
// }

// function splitb(s: string) {
//   const conditions = Abilities.split(s);
//   const ability = conditions.pop();

//   return conditions;
// }

export class AbilityParser {
  static minmax(tokens: string[], i: number, mod: BasicModifier) {
    if (tokens[i] == "Min") {
      mod.setMin(+tokens[i + 1]);
      return true;
    } else if (tokens[i] == "Max") {
      mod.setMax(+tokens[i + 1]);
      return true;
    }

    return false;
  }

  /** Parse a Per clause and return how many tokens it consumed. */
  static per(tokens: string[], i: number, mod: BasicModifier) {
    if (tokens[i] == "Per") {
      let stat = i + 1;
      let opp = false;
      if (tokens[i + 1] == "Opp") {
        opp = true;
        stat++;
      }
      const lost = tokens[stat + 1] === "Lost";
      mod.setPer(tokens[stat] + (lost ? "_LOST" : ""), opp);

      return stat - i + 1 + +lost;
    }

    return 0;
  }

  static minmaxper(tokens: string[], i: number, mod: BasicModifier) {
    if (AbilityParser.minmax(tokens, i, mod)) {
      return true;
    } else {
      const consumed = AbilityParser.per(tokens, i, mod);
      if (!consumed) return false;
      AbilityParser.minmax(tokens, i + consumed, mod);

      return true;
    }
  }
}
