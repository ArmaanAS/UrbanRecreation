import BattleData from "../battle/BattleData";
import EventTime from "../types/EventTime";
import Modifier from "./Modifier";

enum Exchange {
  POWER = 1,
  DAMAGE = 2,
  IMPOSE_POWER = 3,
  IMPOSE_DAMAGE = 4,
}
export const ExchangeObject: { [index: string]: Exchange; } = {
  POWER: Exchange.POWER,
  DAMAGE: Exchange.DAMAGE,
  "IMPOSE POWER": Exchange.IMPOSE_POWER,
  "IMPOSE DAMAGE": Exchange.IMPOSE_DAMAGE,
};

export default class ExchangeModifier extends Modifier {
  ex = Exchange.POWER;
  constructor(ex: Exchange | string, et = EventTime.PRE2) {
    super();

    if (typeof ex == 'string')
      this.ex = ExchangeObject[ex.toUpperCase()];
    else
      this.ex = ex;
    this.eventTime = et;
  }

  apply(data: BattleData) {
    switch (this.ex) {
      case Exchange.POWER: {
        if (!data.card.power.blocked) {
          // if (data.card.power.prot || !data.card.power.cancel) {
          // data.card.power_.final = data.oppCard.power.base;
          // data.oppCard.power_.final = data.card.power.base;
          data.card.power.final = data.oppCard.power.base;
          data.oppCard.power.final = data.card.power.base;
        } else console.log('data.card.power.blocked === true,', data.card.power.cancel, data.card.power.prot, data.card.power.blocked);
        break;
      }
      case Exchange.DAMAGE: {
        if (!data.card.damage.blocked) {
          // if (data.card.damage.prot || !data.card.damage.cancel) {
          // data.card.damage_.final = data.oppCard.damage.base;
          // data.oppCard.damage_.final = data.card.damage.base;
          data.card.damage.final = data.oppCard.damage.base;
          data.oppCard.damage.final = data.card.damage.base;
        } else console.log('data.card.damage.blocked === true');
        break;
      }
      case Exchange.IMPOSE_POWER: {
        if (!data.card.power.blocked) {
          data.oppCard.power.final = data.card.power.base;
        } else console.log('data.card.power.blocked === true,', data.card.power.cancel, data.card.power.prot, data.card.power.blocked);
        break;
      }
      case Exchange.IMPOSE_DAMAGE: {
        if (!data.card.damage.blocked) {
          data.oppCard.damage.final = data.card.damage.base;
        } else console.log('data.card.damage.blocked === true');
        break;
      }
    }
  }
}