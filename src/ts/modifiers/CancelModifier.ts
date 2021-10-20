import BattleData from "../BattleData";
import { EventTime } from "../Events";
import Modifier from "./Modifier";

enum Cancel {
  POWER = 1,
  DAMAGE = 2,
  ATTACK = 3,
  ABILITY = 4,
  BONUS = 5,
  PILLZ = 6,
  LIFE = 7,
}
export const CancelObject: { [key: string]: Cancel } = {
  POWER: Cancel.POWER,
  DAMAGE: Cancel.DAMAGE,
  ATTACK: Cancel.ATTACK,
  ABILITY: Cancel.ABILITY,
  BONUS: Cancel.BONUS,
  PILLZ: Cancel.PILLZ,
  LIFE: Cancel.LIFE,
}

export default class CancelModifier extends Modifier {
  cancel = Cancel.POWER;
  constructor(cancel: Cancel | string, et = EventTime.PRE3) {
    super();

    if (typeof cancel == "string")
      this.cancel = CancelObject[cancel.toUpperCase()]
    else
      this.cancel = cancel;
    this.eventTime = et;
  }

  apply(data: BattleData) {
    // if (this.cancel == Cancel.POWER) {
    //   data.oppCard.power.cancel = true;
    // } else if (this.cancel == Cancel.DAMAGE) {
    //   data.oppCard.damage.cancel = true;
    // } else if (this.cancel == Cancel.ATTACK) {
    //   data.oppCard.attack.cancel = true;
    // } else if (this.cancel == Cancel.PILLZ) {
    //   data.oppCard.pillz.cancel = true;
    // } else if (this.cancel == Cancel.LIFE) {
    //   data.oppCard.life.cancel = true;
    // } else if (this.cancel == Cancel.ABILITY) {
    //   data.oppCard.ability.cancel = true;
    // } else if (this.cancel == Cancel.BONUS) {
    //   data.oppCard.bonus.cancel = true;
    // }
    switch (this.cancel) {
      case Cancel.POWER: data.oppCard.power.cancel = true; break;
      case Cancel.DAMAGE: data.oppCard.damage.cancel = true; break;
      case Cancel.ATTACK: data.oppCard.attack.cancel = true; break;
      case Cancel.PILLZ: data.oppCard.pillz.cancel = true; break;
      case Cancel.LIFE: data.oppCard.life.cancel = true; break;
      case Cancel.ABILITY: data.oppCard.ability.cancel = true; break;
      case Cancel.BONUS: data.oppCard.bonus.cancel = true; break;
    }
  }
}