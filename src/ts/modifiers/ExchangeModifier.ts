import BattleData from "../BattleData";
import { EventTime } from "../Events"
import Modifier from "./Modifier";

enum Exchange {
  POWER = 1,
  DAMAGE = 2,
}
export const ExchangeObject: { [index: string]: Exchange } = {
  POWER: Exchange.POWER,
  DAMAGE: Exchange.DAMAGE,
}

export default class ExchangeModifier extends Modifier {
  ex = Exchange.POWER;
  constructor(ex: Exchange | string, et = EventTime.PRE3) {
    super();

    if (typeof ex == 'string')
      this.ex = ExchangeObject[ex.toUpperCase()];
    else
      this.ex = ex;
    this.eventTime = et;
  }

  apply(data: BattleData) {
    if (this.ex == Exchange.POWER) {
      if (!data.card.power.blocked) {
        // if (data.card.power.prot || !data.card.power.cancel) {
        data.card.power_.final = data.oppCard.power.base;
        data.oppCard.power_.final = data.card.power.base;
      }
    } else if (this.ex == Exchange.DAMAGE) {
      if (!data.card.damage.blocked) {
        // if (data.card.damage.prot || !data.card.damage.cancel) {
        data.card.damage_.final = data.oppCard.damage.base;
        data.oppCard.damage_.final = data.card.damage.base;
      }
    }
  }
}