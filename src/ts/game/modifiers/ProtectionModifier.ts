import BattleData from "../battle/BattleData";
import EventTime from "../types/EventTime";
import Modifier from "./Modifier";

enum Prot {
  POWER = 1,
  DAMAGE = 2,
  ATTACK = 3,
  ABILITY = 4,
  BONUS = 5,
}
export const ProtObject: { [key: string]: Prot } = {
  POWER: Prot.POWER,
  DAMAGE: Prot.DAMAGE,
  ATTACK: Prot.ATTACK,
  ABILITY: Prot.ABILITY,
  BONUS: Prot.BONUS,
}

export default class ProtectionModifier extends Modifier {
  prot = Prot.POWER;
  constructor(prot: Prot | string, et = EventTime.PRE3) {
    super();

    if (typeof prot == 'string')
      this.prot = ProtObject[prot.toUpperCase()]
    else
      this.prot = prot;
    this.eventTime = et;
  }

  apply(data: BattleData) {
    switch (this.prot) {
      // case Prot.POWER: data.card.power_.prot = true; break;
      // case Prot.DAMAGE: data.card.damage_.prot = true; break;
      // case Prot.ATTACK: data.card.attack_.prot = true; break;
      // case Prot.ABILITY: data.card.ability_.prot = true; break;
      // case Prot.BONUS: data.card.bonus_.prot = true; break;
      case Prot.POWER: data.card.power.prot = true; break;
      case Prot.DAMAGE: data.card.damage.prot = true; break;
      case Prot.ATTACK: data.card.attack.prot = true; break;
      case Prot.ABILITY: data.card.ability.prot = true; break;
      case Prot.BONUS: data.card.bonus.prot = true; break;
    }
  }
}