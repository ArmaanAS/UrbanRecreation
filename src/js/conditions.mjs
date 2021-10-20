import Ability from "./ability.mjs";

export default class Condition {
  constructor(s) {
    this.s = s;
    this.type = Condition.Type[s.toUpperCase()] || 0;
  }

  clone() {
    return Object.setPrototypeOf({
      s: this.s,
      type: this.type,
      stop: this.stop
    }, Condition.prototype);
  }

  static from(o) {
    return Object.setPrototypeOf(o, Condition.prototype);
  }

  met(data) {
    if (this.type == Condition.Type.DEFEAT) {
      return data.player.won == false;

    } else if (this.type == Condition.Type.NIGHT) {
      return data.round.day == false;

    } else if (this.type == Condition.Type.DAY) {
      return data.round.day == true;

    } else if (this.type == Condition.Type.COURAGE) {
      return data.round.first == true;

    } else if (this.type == Condition.Type.REVENGE) {
      return data.player.wonPrevious == false;

    } else if (this.type == Condition.Type.CONFIDENCE) {
      return data.player.wonPrevious == true;

    } else if (this.type == Condition.Type.REPRISAL) {
      return data.round.first == false;

    } else if (this.type == Condition.Type.KILLSHOT) {
      return data.card.attack.final >= data.oppCard.attack.final * 2;

    } else if (this.type == Condition.Type.BACKLASH) {
      return data.player.won == true;

    } else if (this.type == Condition.Type.REANIMATE) {
      if (data.player.life < 0) data.player.life = 0;
      return data.player.won == false && data.player.life <= 0;

    } else if (this.type == Condition.Type.STOP) {
      if (this.stop == 'Ability') {
        return data.card.ability.cancel;
      } else if (this.stop == 'Bonus') {
        return data.card.bonus.cancel;
      }

    } else if (this.type == Condition.Type.SYMMETRY) {
      return data.card.index == data.oppCard.index;

    } else if (this.type == Condition.Type.ASYMMETRY) {
      return data.card.index != data.oppCard.index;

    }

    return true;
  }

  compile(data, ability) {
    if (this.type == Condition.Type.BACKLASH) {
      for (let mod of ability.mods) {
        mod.setOpp(false);
      }

    } else if (this.type == Condition.Type.BRAWL) {
      for (let mod of ability.mods) {
        mod.setPer('BRAWL');
      }

    } else if (this.type == Condition.Type.SUPPORT) {
      for (let mod of ability.mods) {
        mod.setPer('SUPPORT');
      }

    } else if (this.type == Condition.Type.GROWTH) {
      for (let mod of ability.mods) {
        mod.setPer('GROWTH');
      }

    } else if (this.type == Condition.Type.DEGROWTH) {
      for (let mod of ability.mods) {
        mod.setPer('DEGROWTH');
      }

    } else if (this.type == Condition.Type.EQUALIZER) {
      for (let mod of ability.mods) {
        mod.setPer('EQUALIZER');
      }

    } else if (this.type == Condition.Type.DEFEAT) {
      for (let mod of ability.mods) {
        mod.win = false;
      }

    } else if (this.type == Condition.Type["VICTORY OR DEFEAT"]) {
      for (let mod of ability.mods) {
        mod.win = false;
      }

    } else if (this.type == Condition.Type.REANIMATE) {
      for (let mod of ability.mods) {
        mod.win = false;
      }

    } else if (this.type == Condition.Type.STOP) {
      if (ability.type == Ability.Type.ABILITY) {
        this.stop = 'Ability';
        data.card.ability.prot = true;

      } else if (ability.type == Ability.Type.BONUS) {
        this.stop = 'Bonus'
        data.card.bonus.prot = true;
      }

    }
  }

  static Type = {
    UNDEFINED: 0, //
    COURAGE: 1, //
    DEFEAT: 2, //
    BRAWL: 3, //
    GROWTH: 4, //
    CONFIDENCE: 5, //
    DEGROWTH: 6, // 
    "VICTORY OR DEFEAT": 7, //
    EQUALIZER: 8, // 
    SUPPORT: 9, // 
    TEAM: 10,
    SYMMETRY: 11, // 
    REVENGE: 12,
    REPRISAL: 13, //
    DAY: 14, //
    NIGHT: 15, //
    KILLSHOT: 16, //
    BACKLASH: 17, //
    ASYMMETRY: 18, // 
    REANIMATE: 19, //
    STOP: 20
  }

  static normalise(c) {
    return c
      .replace(/vic.*/gi, 'Victory Or Defeat')
      .replace(/conf.*/gi, 'Confidence');
  }
}