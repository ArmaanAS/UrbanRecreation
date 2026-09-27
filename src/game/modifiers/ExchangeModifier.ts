import BattleData from "../battle/BattleData.ts";
import EventTime from "../types/EventTime.ts";
import Modifier from "./Modifier.ts";
import { DEBUG } from "../../utils/Debug.ts";
enum Exchange {

  POWER = 1,
  DAMAGE = 2,
  IMPOSE_POWER = 3,
  IMPOSE_DAMAGE = 4,
}
export const ExchangeObject: { [index: string]: Exchange } = {
  POWER: Exchange.POWER,
  DAMAGE: Exchange.DAMAGE,
  "IMPOSE POWER": Exchange.IMPOSE_POWER,
  "IMPOSE DAMAGE": Exchange.IMPOSE_DAMAGE,
};

/**
 * `Power Exchange`, `Damage Exchange`, `Power And Damage Exchange` and `Damage Impose` write
 * printed values ("the starting Power (written on the card)") onto the cards, so the server
 * resolves them before every increase and reduction: at PRE3 beside Copy, which does the
 * same for one card. They used to run in PRE2 among the own increases and overwrote whatever
 * PRE2 had already applied - the owner's own bonus always (bonus compiles before ability)
 * and the opponent's own increase whenever the opponent was internal P1. Three rounds pin
 * the order: Calamity's Power Exchange leaves Tina on 6 Power in 1059149 r1 (the swapped 5,
 * her own Revenge +2, then Calamity's Night -1) and Calamity herself on 7 in 1507008 r0
 * (Tatane's 6, then her own Day +1; 7 x 3 - 12 = 9 Attack), and Kochar's Damage Impose leaves
 * Tina on 4 Damage in 874712 r1 (Kochar's printed 2, then her Revenge +2). All 39 captured
 * Exchange and Impose rounds fit this order. Every write reads printed values, so two of
 * them, or one against a Copy, do not depend on which runs first. An opposing Cancel of the
 * stat (PRE4) still skips the swap.
 */
export default class ExchangeModifier extends Modifier {
  ex = Exchange.POWER;
  constructor(ex: Exchange | string, et = EventTime.PRE3) {
    super();

    if (typeof ex == "string") {
      this.ex = ExchangeObject[ex.toUpperCase()];
    } else {
      this.ex = ex;
    }
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
        } else {if (DEBUG) console.log(
            "data.card.power.blocked === true,",
            data.card.power.cancel,
            data.card.power.prot,
            data.card.power.blocked,
          );}
        break;
      }
      case Exchange.DAMAGE: {
        if (!data.card.damage.blocked) {
          // if (data.card.damage.prot || !data.card.damage.cancel) {
          // data.card.damage_.final = data.oppCard.damage.base;
          // data.oppCard.damage_.final = data.card.damage.base;
          data.card.damage.final = data.oppCard.damage.base;
          data.oppCard.damage.final = data.card.damage.base;
        } else if (DEBUG) console.log("data.card.damage.blocked === true");
        break;
      }
      case Exchange.IMPOSE_POWER: {
        if (!data.card.power.blocked) {
          data.oppCard.power.final = data.card.power.base;
        } else {if (DEBUG) console.log(
            "data.card.power.blocked === true,",
            data.card.power.cancel,
            data.card.power.prot,
            data.card.power.blocked,
          );}
        break;
      }
      case Exchange.IMPOSE_DAMAGE: {
        if (!data.card.damage.blocked) {
          data.oppCard.damage.final = data.card.damage.base;
        } else if (DEBUG) console.log("data.card.damage.blocked === true");
        break;
      }
    }
  }
}
