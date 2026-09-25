import BattleData from "../battle/BattleData.ts";
import EventTime from "../types/EventTime.ts";
import Modifier from "./Modifier.ts";
import { DEBUG } from "../../utils/Debug.ts";
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
  /**
   * Also refuse the opposing card's reductions of the protected Power or Damage, not only
   * a Cancel. Set for exactly `Protection: Power And Damage`, which the server shows
   * keeping both stats at their own values against an opposing ability, bonus or Support
   * reduction: Miss Pandora stays 7/4 against Sue's "-1 Opp Power And Damage, Min 3"
   * (1069506 r0, 7 x 5 = 35), Nebula keeps 7 Power against Olga Cr's "-2 Opp Power, Min 5"
   * (949439 r0, 7 x 5 = 35) and 4 Damage against Henry's "Support: -1 Opp Damage, Min 2"
   * (942983 r2). Only reductions: an opposing Attack reduction still lands (956805 r2), as
   * do an Exchange (948108 r3), an Impose (1091314 r3) and Tune Out (925868 r3), none of
   * which is a BasicModifier reduction. No capture shows the single-stat Protections or the
   * clan-gated `Protection : Damage` refusing a reduction of the stat they name, so they keep
   * the Cancel-only behaviour. A conditional Power And Damage form sets it whenever its
   * condition holds, by composition: `Reprisal: Protect. Power And Damage` (tested; the Rust
   * engine admits it since semantic revision 75) and, unobserved in any capture, the Revenge
   * and Courage forms, which the Rust engine still refuses.
   */
  guard = false;
  constructor(prot: Prot | string, both = false, et = EventTime.PRE3) {
    super();

    if (typeof prot == "string") {
      this.prot = Prot[prot.toUpperCase() as keyof typeof Prot];
    } else {
      this.prot = prot;
    }

    if (both) {
      if (DEBUG) console.log("Set both: " + both);
    }
    this.both = both;
    this.eventTime = et;
  }

  setBoth(both: boolean) {
    if (DEBUG) console.log("Set both: " + both);
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
          if (this.guard) data.card.power.guard = true;
          break;
        case Prot.DAMAGE:
          data.card.damage.prot = true;
          if (this.guard) data.card.damage.guard = true;
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
