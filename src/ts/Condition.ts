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
    // this.type = Condition.Type[s.toUpperCase()] ?? 0;
    this.type = ConditionTypes[s.toUpperCase()] ?? ConditionType.UNDEFINED;
  }

  static from(o: { s: string, type: number, stop: string }) {
    return Object.setPrototypeOf(o, Condition.prototype);
  }

  met(data: BattleData) {
    if (this.type == ConditionType.DEFEAT) {
      return data.player.won == false;

    } else if (this.type == ConditionType.NIGHT) {
      return data.round.day == false;

    } else if (this.type == ConditionType.DAY) {
      return data.round.day == true;

    } else if (this.type == ConditionType.COURAGE) {
      return data.round.first == true;

    } else if (this.type == ConditionType.REVENGE) {
      return data.player.wonPrevious == false;

    } else if (this.type == ConditionType.CONFIDENCE) {
      return data.player.wonPrevious == true;

    } else if (this.type == ConditionType.REPRISAL) {
      return data.round.first == false;

    } else if (this.type == ConditionType.KILLSHOT) {
      return data.card.attack.final >= data.oppCard.attack.final * 2;

    } else if (this.type == ConditionType.BACKLASH) {
      return data.player.won == true;

    } else if (this.type == ConditionType.REANIMATE) {
      if (data.player.life < 0) data.player.life = 0;
      return data.player.won == false && data.player.life <= 0;

    } else if (this.type == ConditionType.STOP) {
      if (this.stop == 'Ability') {
        return data.card.ability.cancel;
      } else if (this.stop == 'Bonus') {
        return data.card.bonus.cancel;
      }

    } else if (this.type == ConditionType.SYMMETRY) {
      return data.card.index == data.oppCard.index;

    } else if (this.type == ConditionType.ASYMMETRY) {
      return data.card.index != data.oppCard.index;

    }

    return true;
  }

  compile(data: BattleData, ability: Ability) {
    if (this.type == ConditionType.BACKLASH) {
      for (let mod of ability.mods) {
        if (mod instanceof BasicModifier)
          mod.setOpp(false);
      }

    } else if (this.type == ConditionType.BRAWL) {
      for (let mod of ability.mods) {
        if (mod instanceof BasicModifier)
          mod.setPer('BRAWL');
      }

    } else if (this.type == ConditionType.SUPPORT) {
      for (let mod of ability.mods) {
        if (mod instanceof BasicModifier)
          mod.setPer('SUPPORT');
      }

    } else if (this.type == ConditionType.GROWTH) {
      for (let mod of ability.mods) {
        if (mod instanceof BasicModifier)
          mod.setPer('GROWTH');
      }

    } else if (this.type == ConditionType.DEGROWTH) {
      for (let mod of ability.mods) {
        if (mod instanceof BasicModifier)
          mod.setPer('DEGROWTH');
      }

    } else if (this.type == ConditionType.EQUALIZER) {
      for (let mod of ability.mods) {
        if (mod instanceof BasicModifier)
          mod.setPer('EQUALIZER');
      }

    } else if (this.type == ConditionType.DEFEAT) {
      for (let mod of ability.mods) {
        if (mod instanceof BasicModifier)
          mod.win = false;
      }

    } else if (this.type == ConditionType["VICTORY OR DEFEAT"]) {
      for (let mod of ability.mods) {
        if (mod instanceof BasicModifier)
          mod.win = false;
      }

    } else if (this.type == ConditionType.REANIMATE) {
      for (let mod of ability.mods) {
        if (mod instanceof BasicModifier)
          mod.win = false;
      }

    } else if (this.type == ConditionType.STOP) {
      if (ability.type == AbilityType.ABILITY) {
        this.stop = 'Ability';
        data.card.ability.prot = true;

      } else if (ability.type == AbilityType.BONUS) {
        this.stop = 'Bonus'
        data.card.bonus.prot = true;
      }
    }
  }

  // static Type = {
  //   UNDEFINED: 0, //
  //   COURAGE: 1, //
  //   DEFEAT: 2, //
  //   BRAWL: 3, //
  //   GROWTH: 4, //
  //   CONFIDENCE: 5, //
  //   DEGROWTH: 6, // 
  //   "VICTORY OR DEFEAT": 7, //
  //   EQUALIZER: 8, // 
  //   SUPPORT: 9, // 
  //   TEAM: 10,
  //   SYMMETRY: 11, // 
  //   REVENGE: 12,
  //   REPRISAL: 13, //
  //   DAY: 14, //
  //   NIGHT: 15, //
  //   KILLSHOT: 16, //
  //   BACKLASH: 17, //
  //   ASYMMETRY: 18, // 
  //   REANIMATE: 19, //
  //   STOP: 20
  // } as { [index: string]: number }

  static normalise(c: string) {
    return c
      .replace(/vic.*/gi, 'Victory Or Defeat')
      .replace(/conf.*/gi, 'Confidence');
  }
}