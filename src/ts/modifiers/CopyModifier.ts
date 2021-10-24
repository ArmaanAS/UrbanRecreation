import BattleData from "../BattleData";
import { EventTime } from "../Events";
import Modifier from "./Modifier";

enum Copy {
  POWER = 1,
  DAMAGE = 2,
  ABILITY = 3,
  BONUS = 4,
}
export const CopyObject: { [key: string]: Copy } = {
  POWER: Copy.POWER,
  DAMAGE: Copy.DAMAGE,
  ABILITY: Copy.ABILITY,
  BONUS: Copy.BONUS,
}

export default class CopyModifier extends Modifier {
  copy = Copy.POWER;
  constructor(copy: Copy | string, et = EventTime.PRE3) {
    super();

    if (typeof copy == 'string')
      this.copy = CopyObject[copy.toUpperCase()]
    else
      this.copy = copy;
    this.eventTime = et;
  }

  apply(data: BattleData) {
    if (this.copy == Copy.POWER) {
      if (!data.card.power.blocked)
        // if (data.card.power.prot || !data.card.power.cancel)
        data.card.power_.final = data.oppCard.power.base;

    } else if (this.copy == Copy.DAMAGE)
      if (!data.card.damage.blocked)
        // if (data.card.damage.prot || !data.card.damage.cancel)
        data.card.damage_.final = data.oppCard.damage.base;
  }
}