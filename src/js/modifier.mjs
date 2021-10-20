import Events from "./events.mjs";

let getKey = (o, val) => Object.entries(o).filter(([k, v]) => v == val)[0][0];

class Per {
  // Enum
  constructor(opp, type, name) {
    this.opp = opp;
    this.type = type;
    this.name = name;
  }

  static enum(opp, type, name) {
    if (opp == undefined) return undefined;
    return new Per(opp, type, name);
  }
}

class Time {
  constructor(eventTime, win, name) {
    this.eventTime = eventTime;
    this.win = win;
    this.name = name;
  }

  static enum(eventTime, name, win = false) {
    if (eventTime == undefined) return undefined;
    return new Time(eventTime, win, name);
  }
}

export default class Modifier {
  static basic(change, minmax) {
    return new BasicModifier(change, minmax);
  }

  static copy(copy) {
    return new CopyModifier(Modifier.Copy[copy.toUpperCase()]);
  }

  static exchange(ex) {
    return new ExchangeModifier(Modifier.Exchange[ex.toUpperCase()]);
  }

  static cancel(cancel) {
    return new CancelModifier(Modifier.Cancel[cancel.toUpperCase()]);
  }

  static prot(prot) {
    return new ProtectionModifier(Modifier.Prot[prot.toUpperCase()]);
  }

  static Per = {
    UNDEFINED: Per.enum(),
    POWER: Per.enum(false, 1, "POWER"),
    DAMAGE: Per.enum(false, 2, "DAMAGE"),
    LIFE: Per.enum(false, 3, "LIFE"),
    PILLZ: Per.enum(false, 4, "PILLZ"),
    SUPPORT: Per.enum(false, 5, "SUPPORT"),
    BRAWL: Per.enum(false, 6, "BRAWL"),
    GROWTH: Per.enum(false, 7, "GROWTH"),
    DEGROWTH: Per.enum(false, 8, "DEGROWTH"),
    EQUALIZER: Per.enum(false, 9, "EQUALIZER"),
    SYMMETRY: Per.enum(false, 10, "SYMMETRY"),
    ASYMMETRY: Per.enum(false, 11, "ASYMMETRY"),
    OPP_POWER: Per.enum(true, 1, "OPP_POWER"),
    OPP_DAMAGE: Per.enum(true, 2, "OPP_DAMAGE"),
    OPP_LIFE: Per.enum(true, 3, "OPP_LIFE"),
    OPP_PILLZ: Per.enum(true, 4, "OPP_PILLZ"),
    from: o => Object.values(Modifier.Per).filter(e => e.name == o.name)[0]
  };

  static Type = {
    UNDEFINED: Time.enum(),
    POWER: Time.enum(Events.PRE4, "POWER"),
    DAMAGE: Time.enum(Events.PRE4, "DAMAGE"),
    ATTACK: Time.enum(Events.POST1, "ATTACK"),
    LIFE: Time.enum(Events.END, "LIFE", true),
    PILLZ: Time.enum(Events.END, "PILLZ", true),
    fromString: function (s) {
      return this[s.toUpperCase()] || 0;
    },
    from: o => Object.values(Modifier.Type).filter(e => e.name == o.name)[0]
  };

  static Copy = {
    POWER: 1,
    DAMAGE: 2,
    ABILITY: 3,
    BONUS: 4,
  };

  static Exchange = {
    POWER: 1,
    DAMAGE: 2,
  };

  static Cancel = {
    POWER: 1,
    DAMAGE: 2,
    ATTACK: 3,
    ABILITY: 4,
    BONUS: 5,
    PILLZ: 6,
    LIFE: 7,
  };

  static Prot = {
    POWER: 1,
    DAMAGE: 2,
    ATTACK: 3,
    ABILITY: 4,
    BONUS: 5,
  };
}

class BasicModifier extends Modifier {
  constructor(change, minmax) {
    super();

    this.change = change;
    this.per = Modifier.Per.UNDEFINED;
    this.type = Modifier.Type.UNDEFINED;

    this.eventTime = undefined;
    this.opp = false;

    this.min = -Infinity;
    this.max = Infinity;

    if (minmax != undefined) {
      if (change > 0) {
        this.setMax(minmax);
      } else {
        this.setMin(minmax);
      }
    }
  }

  clone() {
    return Object.setPrototypeOf({
        change: this.change,
        per: this.per,
        type: this.type,

        eventTime: this.eventTime,
        win: this.win,
        opp: this.opp,

        min: this.min,
        max: this.max,
      },
      BasicModifier.prototype
    );
  }

  static from(o) {
    Object.setPrototypeOf(o, Modifier.prototype);

    o.per = Modifier.Per.from(o.per);
    o.type = Modifier.Type.from(o.type);

    return o;
  }

  setMin(min) {
    this.min = min;
    console.log("Set min: " + min);

    return this;
  }

  setMax(max) {
    this.max = max;
    console.log("Set max: " + max);

    return this;
  }

  setPer(per, opp = false) {
    if (typeof per == "string") {
      if (opp) {
        per = Modifier.Per[`OPP_${per.toUpperCase()}`] || 0;
      } else {
        per = Modifier.Per[per.toUpperCase()] || 0;
      }
    }
    this.per = per;
    console.log(`Set per: ${per.name}`);

    return this;
  }

  setType(type) {
    if (typeof type == "string") {
      type = Modifier.Type.fromString(type);
    }
    this.type = type;
    this.win = type.win;

    if (type != Modifier.Type.UNDEFINED) {
      this.eventTime = type.eventTime;
    }

    console.log(`Set modifier type: ${type.name}`);

    return this;
  }

  setOpp(opp = true) {
    if (opp != this.opp) {
      console.log("Set opp: " + opp);
    }
    this.opp = opp;

    return this;
  }

  canApply(data) {
    if (this.win && !data.player.won) return false;

    if (this.opp) {
      if (this.type == Modifier.Type.POWER) {
        return !data.oppCard.power.prot;
      } else if (this.type == Modifier.Type.DAMAGE) {
        return !data.oppCard.damage.prot;
      } else if (this.type == Modifier.Type.ATTACK) {
        return !data.oppCard.attack.prot;
      } else if (this.type == Modifier.Type.LIFE) {
        return !data.oppCard.life.prot;
      } else if (this.type == Modifier.Type.PILLZ) {
        return !data.oppCard.pillz.prot;
      }
    } else {
      if (this.type == Modifier.Type.POWER) {
        return !data.card.power.blocked();
      } else if (this.type == Modifier.Type.DAMAGE) {
        return !data.card.damage.blocked();
      } else if (this.type == Modifier.Type.ATTACK) {
        return !data.card.attack.blocked();
      } else if (this.type == Modifier.Type.LIFE) {
        return !data.card.life.blocked();
      } else if (this.type == Modifier.Type.PILLZ) {
        return !data.card.pillz.blocked();
      }
    }

    return true;
  }

  getMultiplier(data) {
    per: if (this.per) {
      let player, card;
      if (this.per.opp) {
        player = data.opp;
        card = data.oppCard;
      } else {
        player = data.player;
        card = data.card;
      }

      switch (this.per.type) {
        case 1:
          return card.power.final;
        case 2:
          return card.damage.final;
        case 3:
          return player.life;
        case 4:
          return player.pillz;
        case 5:
          return data.round.getClanCards(data.card);
        case 6:
          return data.round.getClanCards(data.oppCard, true);
        case 7:
          return data.round.round;
        case 8:
          return 5 - data.round.round;
        case 9:
          return data.oppCard.stars;
        case 10:
          return data.card.index == data.oppCard.index;
        case 11:
          return data.card.index != data.oppCard.index;
      }
    }

    return 1;
  }

  mod(base, data) {
    if (base <= this.min || base >= this.max) return base;
    let change = this.change * this.getMultiplier(data);
    let final = base + change;
    let squash = Math.min(Math.max(final, this.min), this.max);

    console.log(`${base} => ${final} >=< ${squash}`);
    if (isNaN(squash)) {
      console.log(data.player.life);
      console.log(`${this.getMultiplier(data)} => ${this.change} => ${base}`);
      console.log(
        `${typeof this.getMultiplier(data)} => ${typeof this
          .change} => ${typeof base}`
      );
    }
    return squash;
  }

  apply(data) {
    if (this.canApply(data)) {
      console.log(`canApply modifier`);
      let card, player;
      if (this.opp) {
        card = data.oppCard;
        player = data.opp;
      } else {
        card = data.card;
        player = data.player;
      }

      if (this.type == Modifier.Type.POWER) {
        let p = card.power;
        p.final = this.mod(p.final, data);
      } else if (this.type == Modifier.Type.DAMAGE) {
        let d = card.damage;
        d.final = this.mod(d.final, data);
      } else if (this.type == Modifier.Type.ATTACK) {
        let a = card.attack;
        a.final = this.mod(a.final, data);
      } else if (this.type == Modifier.Type.LIFE) {
        player.life = this.mod(player.life, data);
      } else if (this.type == Modifier.Type.PILLZ) {
        player.pillz = this.mod(player.pillz, data);
      }
    } else console.log(`Failed to apply modifier`.brightYellow);
  }
}

class CopyModifier extends Modifier {
  constructor(copy, et = Events.PRE3) {
    super();

    this.copy = copy;
    this.eventTime = et;
  }

  clone() {
    return Object.setPrototypeOf({
        copy: this.copy,
        eventTime: this.eventTime,
      },
      CopyModifier.prototype
    );
  }

  apply(data) {
    if (this.copy == Modifier.Copy.POWER) {
      if (!data.card.power.blocked()) {
        data.card.power.final = data.oppCard.power.base;
      }
    } else if (this.copy == Modifier.Copy.DAMAGE) {
      if (!data.card.damage.blocked()) {
        data.card.damage.final = data.oppCard.damage.base;
      }
    }
  }
}

class ExchangeModifier {
  constructor(ex, et = Events.PRE3) {
    this.ex = ex;
    this.eventTime = et;
  }

  clone() {
    return Object.setPrototypeOf({
        ex: this.ex,
        eventTime: this.eventTime,
      },
      ExchangeModifier.prototype
    );
  }

  apply(data) {
    if (this.ex == Modifier.Exchange.POWER) {
      if (!data.card.power.blocked()) {
        data.card.power.final = data.oppCard.power.base;
        data.oppCard.power.final = data.card.power.base;
      }
    } else if (this.ex == Modifier.Exchange.DAMAGE) {
      if (!data.card.damage.blocked()) {
        data.card.damage.final = data.oppCard.damage.base;
        data.oppCard.damage.final = data.card.damage.base;
      }
    }
  }
}

class CancelModifier {
  constructor(cancel, et = Events.PRE3) {
    this.cancel = cancel;
    this.eventTime = et;
  }

  clone() {
    return Object.setPrototypeOf({
        cancel: this.cancel,
        eventTime: this.eventTime,
      },
      CancelModifier.prototype
    );
  }

  apply(data) {
    if (this.cancel == Modifier.Cancel.POWER) {
      data.oppCard.power.cancel = true;
    } else if (this.cancel == Modifier.Cancel.DAMAGE) {
      data.oppCard.damage.cancel = true;
    } else if (this.cancel == Modifier.Cancel.ATTACK) {
      data.oppCard.attack.cancel = true;
    } else if (this.cancel == Modifier.Cancel.PILLZ) {
      data.oppCard.pillz.cancel = true;
    } else if (this.cancel == Modifier.Cancel.LIFE) {
      data.oppCard.life.cancel = true;
    } else if (this.cancel == Modifier.Cancel.ABILITY) {
      data.oppCard.ability.cancel = true;
    } else if (this.cancel == Modifier.Cancel.BONUS) {
      data.oppCard.bonus.cancel = true;
    }
  }
}

class ProtectionModifier {
  constructor(prot, et = Events.PRE3) {
    this.prot = prot;
    this.eventTime = et;
  }

  clone() {
    return Object.setPrototypeOf({
        prot: this.prot,
        eventTime: this.eventTime,
      },
      ProtectionModifier.prototype
    );
  }

  apply(data) {
    if (this.prot == Modifier.Prot.POWER) {
      data.card.power.prot = true;
    } else if (this.prot == Modifier.Prot.DAMAGE) {
      data.card.damage.prot = true;
    } else if (this.prot == Modifier.Prot.ATTACK) {
      data.card.attack.prot = true;
    } else if (this.prot == Modifier.Prot.ABILITY) {
      data.card.ability.prot = true;
    } else if (this.prot == Modifier.Prot.BONUS) {
      data.card.bonus.prot = true;
    }
  }
}

// module.exports = Modifier;