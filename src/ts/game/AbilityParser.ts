import BasicModifier from "./modifiers/BasicModifier";

const abilityCache: {
  [index: string]: string[]
} = {};

export class Abilities {
  static normalise(ability: string) {
    return ability
      .replace(/(?<=[+-]) (?=[xy])/gi, "")
      .replace(/[,.]/g, "")
      .replace(/ :/g, ":");
  }

  static splitConditions(ability: string[]) {
    return ability.map(s => s.split(/(?<=\w+) ?: /g));
  }

  static split(ability: string) {
    if (abilityCache[ability] !== undefined)
      return [...abilityCache[ability]];

    const s = ability
      .replace(/(?<=[+-]) (?=[xy\d])/gi, "")
      .replace(/,(?! )/gi, " ")
      .replace(/[,.]| (?=:)/gi, "")
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
      .replace(/(?<=Per.*) Left\w*/gi, "")
      .replace("Bonus Protection", "Protection Bonus")
      // .replace(/([\w ]+) Exchange/ig, 'Exchange $1')
      .replace(/(\w+(?: \w+ \w+)?) ([+-][xy\d]+|Exchange)/i, "$2 $1")
      .replace(/([a-z]+)(?<!Min|Max) ([xy\d]+)/i, "$2 $1")
      .replace(/(\w+) And (\w+)/gi, "$1&$2")
      .replace(/(?<=[xy\d] )(\w+) (Opp)/gi, "$2 $1")
      .replace(/(?<=-[xy\d]+ )Opp /gi, "")
      .replace(/\b\w(?=\w+)/g, (s) => s.toUpperCase())
      .replace(/\[star]/gi, "★")
      .split(/(?<=\w+) ?[:;] /gi);

    abilityCache[ability] = s;
    return [...s];
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

  static per(tokens: string[], i: number, mod: BasicModifier) {
    if (tokens[i] == "Per") {
      if (tokens[i + 1] == "Opp")
        mod.setPer(tokens[i + 2], true);
      else
        mod.setPer(tokens[i + 1]);

      return true;
    }

    return false;
  }

  static minmaxper(tokens: string[], i: number, mod: BasicModifier) {
    if (AbilityParser.minmax(tokens, i, mod))
      return true;
    else if (AbilityParser.per(tokens, i, mod)) {
      AbilityParser.minmax(tokens, i + 2, mod);

      return true;
    }

    return false;
  }
}