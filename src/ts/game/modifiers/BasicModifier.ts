import BattleData from "../battle/BattleData"
import EventTime from "../types/EventTime"
import Modifier from "./Modifier"

type PerType = {
  opp: boolean;
  type: number;
  name: string;
} | undefined
const Per: { [index: string]: PerType } = {
  POWER: { opp: false, type: 1, name: "POWER" },
  DAMAGE: { opp: false, type: 2, name: "DAMAGE" },
  LIFE: { opp: false, type: 3, name: "LIFE" },
  PILLZ: { opp: false, type: 4, name: "PILLZ" },
  SUPPORT: { opp: false, type: 5, name: "SUPPORT" },
  BRAWL: { opp: false, type: 6, name: "BRAWL" },
  GROWTH: { opp: false, type: 7, name: "GROWTH" },
  DEGROWTH: { opp: false, type: 8, name: "DEGROWTH" },
  EQUALIZER: { opp: false, type: 9, name: "EQUALIZER" },
  SYMMETRY: { opp: false, type: 10, name: "SYMMETRY" },
  ASYMMETRY: { opp: false, type: 11, name: "ASYMMETRY" },
  OPP_POWER: { opp: true, type: 1, name: "OPP_POWER" },
  OPP_DAMAGE: { opp: true, type: 2, name: "OPP_DAMAGE" },
  OPP_LIFE: { opp: true, type: 3, name: "OPP_LIFE" },
  OPP_PILLZ: { opp: true, type: 4, name: "OPP_PILLZ" },
} as const
function perFromObject(o: PerType): PerType {
  return o && Per[o.name]
}


type TimeType = {
  eventTime: number;
  name: string;
  win: boolean;
} | undefined
const Time: { [index: string]: TimeType } = {
  // const Time = {
  POWER: { eventTime: EventTime.PRE2, name: "POWER", win: false },
  DAMAGE: { eventTime: EventTime.PRE2, name: "DAMAGE", win: false },
  ATTACK: { eventTime: EventTime.POST1, name: "ATTACK", win: false },
  LIFE: { eventTime: EventTime.END, name: "LIFE", win: true },
  PILLZ: { eventTime: EventTime.END, name: "PILLZ", win: true }
} as const
function timeFromObject(o: TimeType): TimeType {
  return o && Time[o.name]
}
// function timeFromString(s: string): TimeType {
//   return Time[s.toUpperCase()]
// }
const Type = Time;


export default class BasicModifier extends Modifier {
  change = 0;
  per: PerType = undefined;
  type: TimeType = undefined;
  opp = false;
  min = -Infinity;
  max = Infinity;
  always = false;
  // constructor(change: number, minmax?: number) {
  // constructor(change: number) {
  //   super();

  //   this.change = change;

  //   // if (minmax !== undefined)
  //   //   if (change > 0)
  //   //     this.setMax(minmax);
  //   //   else
  //   //     this.setMin(minmax);
  // }

  static from(o: BasicModifier) {
    Object.setPrototypeOf(o, BasicModifier.prototype);

    // o.per = Modifier.Per.from(o.per);
    // o.per = Per[o.per.name]
    o.per = perFromObject(o.per);
    o.type = timeFromObject(o.type);
    process.stderr.write(o.per + "  " + o.per!.constructor.name)
    process.stderr.write(o.type + "  " + o.type!.constructor.name)
    // o.type = timeFromString(o.type);
    // o.type = Time[o.type.name]

    return o;
  }

  setMin(min: number) {
    this.min = min;
    console.log("Set min: " + min);

    return this;
  }

  setMax(max: number) {
    this.max = max;
    console.log("Set max: " + max);

    return this;
  }

  setPer(per: string | PerType, opp = false) {
    if (typeof per == "string") {
      if (opp) {
        this.per = Per[`OPP_${per.toUpperCase()}`];
      } else {
        this.per = Per[per.toUpperCase()];
      }
    } else this.per = per;
    console.log(`Set per: ${this.per?.name}`);

    return this;
  }

  setType(type: TimeType | string) {
    const _temp = type
    if (typeof type == "string")
      // type = timeFromString(type)
      type = Time[type.toUpperCase()]

    if (type === undefined)
      throw new Error('type is undefined for some odd reason??: ' + _temp)

    this.type = type;
    this.win = type.win;

    // if (type !== undefined)
    this.eventTime = type.eventTime;

    console.log(`Set modifier type: ${type.name}`);

    return this;
  }

  setOpp(opp = true) {
    if (opp !== this.opp)
      console.log("Set opp: " + opp);

    this.opp = opp;

    return this;
  }

  canApply(data: BattleData) {
    if (this.always) return true;
    if (this.win && !data.card.won) return false;

    if (this.opp) {
      // console.log('this.opp === true')
      // console.log(data.opp.won, data.oppCard.won)
      // if (this.win && !data.oppCard.won) return false;
      // console.log(data.oppCard.life.prot, data.oppCard.life.cancel);
      switch (this.type) {
        // case Type.POWER: return !data.oppCard.power.prot;
        // case Type.DAMAGE: return !data.oppCard.damage.prot;
        // case Type.ATTACK: return !data.oppCard.attack.prot;
        // case Type.LIFE: return !data.oppCard.life.prot;
        // case Type.PILLZ: return !data.oppCard.pillz.prot;
        case Type.POWER: return !data.oppCard.power.blocked;
        case Type.DAMAGE: return !data.oppCard.damage.blocked;
        case Type.ATTACK: return !data.oppCard.attack.blocked;
        case Type.LIFE: return !data.oppCard.life.blocked;
        case Type.PILLZ: return !data.oppCard.pillz.blocked;
      }
    } else {
      // console.log('this.opp === false')
      // if (this.win && !data.card.won) return false;
      // console.log(data.card.life.prot, data.card.life.cancel);

      switch (this.type) {
        // Don't change to .blocked!!
        case Type.POWER:
          return !data.card.power.prot || !data.card.power.cancel;
        case Type.DAMAGE:
          return !data.card.damage.prot || !data.card.damage.cancel;
        case Type.ATTACK:
          return !data.card.attack.prot || !data.card.attack.cancel;
        case Type.LIFE:
          return (!data.card.life.prot || !data.card.life.cancel) &&
            data.player.life > 0;
        case Type.PILLZ:
          return (!data.card.pillz.prot || !data.card.pillz.cancel) &&
            data.player.pillz > 0;
        // case Type.POWER:
        //   return !data.card.power.blocked;
        // case Type.DAMAGE:
        //   return !data.card.damage.blocked;
        // case Type.ATTACK:
        //   return !data.card.attack.blocked;
        // case Type.LIFE:
        //   return !data.card.life.blocked;
        // case Type.PILLZ:
        //   return !data.card.pillz.blocked;
      }
    }

    return true;
  }

  getMultiplier(data: BattleData) {
    if (this.per === undefined) return 1;

    let player, card;
    if (this.per.opp) {
      player = data.opp;
      card = data.oppCard;
    } else {
      player = data.player;
      card = data.card;
    }

    switch (this.per.type) {
      case 1: return card.power.final;
      case 2: return card.damage.final;
      case 3: return player.life;
      case 4: return player.pillz;
      case 5: return data.round.getClanCards(data.card);
      case 6: return data.round.getClanCards(data.oppCard, true);
      case 7: return data.round.round;
      case 8: return 5 - data.round.round;
      case 9: return data.oppCard.stars;
      case 10: return +(data.card.index == data.oppCard.index);
      case 11: return +(data.card.index != data.oppCard.index);
      default: return 1;
    }
  }

  mod(base: number, data: BattleData) {
    if (base <= this.min || base >= this.max) return base;

    const multiplier = this.getMultiplier(data);
    const change = this.change * multiplier;
    const final = base + change;
    const squash = Math.min(Math.max(final, this.min), this.max);

    console.log(`${base} => ${final} >=< ${squash}`);
    if (isNaN(squash)) {
      console.log(data.player.life);
      console.log(`${multiplier} => ${this.change} => ${base}`);
      console.log(
        `${typeof multiplier} => ${typeof this
          .change} => ${typeof base}`
      );
    }
    return squash;
  }

  apply(data: BattleData) {
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

      switch (this.type) {
        case Type.POWER:
          // card.power_.final = this.mod(card.power.final, data); break;
          card.power.final = this.mod(card.power.final, data); break;
        case Type.DAMAGE:
          // card.damage_.final = this.mod(card.damage.final, data); break;
          card.damage.final = this.mod(card.damage.final, data); break;
        case Type.ATTACK:
          // card.attack_.final = this.mod(card.attack.final, data); break;
          card.attack.final = this.mod(card.attack.final, data); break;
        case Type.LIFE:
          player.life = this.mod(player.life, data); break;
        case Type.PILLZ:
          player.pillz = this.mod(player.pillz, data); break;
      }
    } else console.log(`Failed to apply modifier`.yellow);
  }
}