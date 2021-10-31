import Ability, { AbilityType } from "./Ability";
import BattleData from "./BattleData";
import BasicModifier from "./modifiers/BasicModifier";

export enum ConditionType {
  UNDEFINED = 0,
  COURAGE = 1,
  DEFEAT = 2,
  BRAWL = 3,
  GROWTH = 4,
  CONFIDENCE = 5,
  DEGROWTH = 6,
  "VICTORY OR DEFEAT" = 7,
  EQUALIZER = 8,
  SUPPORT = 9,
  TEAM = 10,
  SYMMETRY = 11,
  REVENGE = 12,
  REPRISAL = 13,
  DAY = 14,
  NIGHT = 15,
  KILLSHOT = 16,
  BACKLASH = 17,
  ASYMMETRY = 18,
  REANIMATE = 19,
  STOP = 20,
}
const ConditionTypes: {
  [key: string]: ConditionType
} = {
  UNDEFINED: ConditionType.UNDEFINED,
  COURAGE: ConditionType.COURAGE,
  DEFEAT: ConditionType.DEFEAT,
  BRAWL: ConditionType.BRAWL,
  GROWTH: ConditionType.GROWTH,
  CONFIDENCE: ConditionType.CONFIDENCE,
  DEGROWTH: ConditionType.DEGROWTH,
  "VICTORY OR DEFEAT": ConditionType["VICTORY OR DEFEAT"],
  EQUALIZER: ConditionType.EQUALIZER,
  SUPPORT: ConditionType.SUPPORT,
  TEAM: ConditionType.TEAM,
  SYMMETRY: ConditionType.SYMMETRY,
  REVENGE: ConditionType.REVENGE,
  REPRISAL: ConditionType.REPRISAL,
  DAY: ConditionType.DAY,
  NIGHT: ConditionType.NIGHT,
  KILLSHOT: ConditionType.KILLSHOT,
  BACKLASH: ConditionType.BACKLASH,
  ASYMMETRY: ConditionType.ASYMMETRY,
  REANIMATE: ConditionType.REANIMATE,
  STOP: ConditionType.STOP,
}


export default class Condition {
  s: string;
  type: ConditionType;
  stop: string;
  constructor(s: string) {
    this.s = s;
    this.type = ConditionTypes[s.toUpperCase()] ?? ConditionType.UNDEFINED;
  }

  static from(o: Condition): Condition {
    return Object.setPrototypeOf(o, Condition.prototype);
  }

  met(data: BattleData) {
    switch (this.type) {
      case ConditionType.DEFEAT: return data.player.won == false;
      case ConditionType.NIGHT: return data.round.day == false;
      case ConditionType.DAY: return data.round.day == true;
      case ConditionType.COURAGE: return data.round.first == true;
      case ConditionType.REVENGE: return data.player.wonPrevious == false;
      case ConditionType.CONFIDENCE: return data.player.wonPrevious == true;
      case ConditionType.REPRISAL: return data.round.first == false;
      case ConditionType.KILLSHOT: return data.card.attack.final >=
        data.oppCard.attack.final * 2;
      case ConditionType.BACKLASH: return data.player.won == true;
      case ConditionType.REANIMATE:
        if (data.player.life < 0) data.player.life = 0;
        return data.player.won == false && data.player.life <= 0;

      case ConditionType.STOP:
        if (this.stop == 'Ability')
          return data.card.ability.cancel;
        else if (this.stop == 'Bonus')
          return data.card.bonus.cancel;
        else
          return true;
      case ConditionType.SYMMETRY: return data.card.index == data.oppCard.index;
      case ConditionType.ASYMMETRY: return data.card.index != data.oppCard.index;
    }

    return true;
  }

  compile(data: BattleData, ability: Ability) {
    // if (this.type == ConditionType.BACKLASH) {
    //   for (const mod of ability.mods) {
    //     if (mod instanceof BasicModifier)
    //       mod.setOpp(false);
    //   }

    // } else if (this.type == ConditionType.BRAWL) {
    //   for (const mod of ability.mods) {
    //     if (mod instanceof BasicModifier)
    //       mod.setPer('BRAWL');
    //   }

    // } else if (this.type == ConditionType.SUPPORT) {
    //   for (const mod of ability.mods) {
    //     if (mod instanceof BasicModifier)
    //       mod.setPer('SUPPORT');
    //   }

    // } else if (this.type == ConditionType.GROWTH) {
    //   for (const mod of ability.mods) {
    //     if (mod instanceof BasicModifier)
    //       mod.setPer('GROWTH');
    //   }

    // } else if (this.type == ConditionType.DEGROWTH) {
    //   for (const mod of ability.mods) {
    //     if (mod instanceof BasicModifier)
    //       mod.setPer('DEGROWTH');
    //   }

    // } else if (this.type == ConditionType.EQUALIZER) {
    //   for (const mod of ability.mods) {
    //     if (mod instanceof BasicModifier)
    //       mod.setPer('EQUALIZER');
    //   }

    // } else if (this.type == ConditionType.DEFEAT) {
    //   for (const mod of ability.mods) {
    //     if (mod instanceof BasicModifier)
    //       mod.win = false;
    //   }

    // } else if (this.type == ConditionType["VICTORY OR DEFEAT"]) {
    //   for (const mod of ability.mods) {
    //     if (mod instanceof BasicModifier)
    //       mod.win = false;
    //   }

    // } else if (this.type == ConditionType.REANIMATE) {
    //   for (const mod of ability.mods) {
    //     if (mod instanceof BasicModifier)
    //       mod.win = false;
    //   }

    // } else if (this.type == ConditionType.STOP) {
    //   if (ability.type == AbilityType.ABILITY) {
    //     this.stop = 'Ability';
    //     data.card.ability_.prot = true;

    //   } else if (ability.type == AbilityType.BONUS) {
    //     this.stop = 'Bonus'
    //     data.card.bonus_.prot = true;
    //   }
    // }
    switch (this.type) {
      case ConditionType.BACKLASH:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier)
            mod.setOpp(false);
        }
        break;

      case ConditionType.BRAWL:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier)
            mod.setPer('BRAWL');
        }
        break;

      case ConditionType.SUPPORT:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier)
            mod.setPer('SUPPORT');
        }
        break;

      case ConditionType.GROWTH:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier)
            mod.setPer('GROWTH');
        }
        break;

      case ConditionType.DEGROWTH:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier)
            mod.setPer('DEGROWTH');
        }
        break;

      case ConditionType.EQUALIZER:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier)
            mod.setPer('EQUALIZER');
        }
        break;

      case ConditionType.DEFEAT:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier)
            mod.win = false;
        }
        break;

      case ConditionType["VICTORY OR DEFEAT"]:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier)
            mod.win = false;
        }
        break;

      case ConditionType.REANIMATE:
        for (const mod of ability.mods) {
          if (mod instanceof BasicModifier)
            mod.win = false;
        }
        break;

      case ConditionType.STOP:
        if (ability.type === AbilityType.ABILITY) {
          this.stop = 'Ability';
          data.card.ability.prot = true;

        } else if (ability.type === AbilityType.BONUS) {
          this.stop = 'Bonus'
          data.card.bonus.prot = true;
        }
    }
  }


  static normalise(c: string) {
    return c
      .replace(/vic.*/gi, 'Victory Or Defeat')
      .replace(/conf.*/gi, 'Confidence');
  }
}