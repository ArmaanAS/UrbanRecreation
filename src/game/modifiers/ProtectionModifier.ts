import BattleData from "../battle/BattleData.ts";
import EventTime from "../types/EventTime.ts";
import Modifier from "./Modifier.ts";

enum Prot {
  POWER = 1,
  DAMAGE = 2,
  ATTACK = 3,
  ABILITY = 4,
  BONUS = 5,
}

export default class ProtectionModifier extends Modifier {
  prot: Prot;
  both: boolean;
  constructor(prot: Prot | string, both = false, et = EventTime.PRE3) {
    super();

    if (typeof prot == "string") {
      this.prot = Prot[prot.toUpperCase() as keyof typeof Prot];
    } else {
      this.prot = prot;
    }

    if (both) {
      console.log("Set both: " + both);
    }
    this.both = both;
    this.eventTime = et;
  }

  setBoth(both: boolean) {
    console.log("Set both: " + both);
    this.both = both;

    return this;
  }

  apply(data: BattleData) {
    if (this.both) {
      switch (this.prot) {
        case Prot.POWER:
          data.oppCard.power.prot = true;
          data.card.power.prot = true;
          break;
        case Prot.DAMAGE:
          data.oppCard.damage.prot = true;
          data.card.damage.prot = true;
          break;
        case Prot.ATTACK:
          data.oppCard.attack.prot = true;
          data.card.attack.prot = true;
          break;
        case Prot.ABILITY:
          data.oppCard.ability.prot = true;
          data.card.ability.prot = true;
          break;
        case Prot.BONUS:
          data.oppCard.bonus.prot = true;
          data.card.bonus.prot = true;
          break;
      }
    } else {
      switch (this.prot) {
        case Prot.POWER:
          data.card.power.prot = true;
          break;
        case Prot.DAMAGE:
          data.card.damage.prot = true;
          break;
        case Prot.ATTACK:
          data.card.attack.prot = true;
          break;
        case Prot.ABILITY:
          data.card.ability.prot = true;
          break;
        case Prot.BONUS:
          data.card.bonus.prot = true;
          break;
      }
    }
  }
}
