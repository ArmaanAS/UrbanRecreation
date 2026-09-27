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
   * Refuse the opposing card's reductions of the protected Power or Damage. (It used to be
   * "also", beside resisting a Cancel; the Cancel wins, see `apply`.) Set for exactly
   * `Protection: Power And Damage`, which the server shows
   * keeping both stats at their own values against an opposing ability, bonus or Support
   * reduction: Miss Pandora stays 7/4 against Sue's "-1 Opp Power And Damage, Min 3"
   * (1069506 r0, 7 x 5 = 35), Nebula keeps 7 Power against Olga Cr's "-2 Opp Power, Min 5"
   * (949439 r0, 7 x 5 = 35) and 4 Damage against Henry's "Support: -1 Opp Damage, Min 2"
   * (942983 r2). Only reductions: an opposing Attack reduction still lands (956805 r2), as
   * do an Exchange (948108 r3), an Impose (1091314 r3) and Tune Out (925868 r3), none of
   * which is a BasicModifier reduction. The single-stat Protections print the same refusal
   * ("The Damage of El Kaktus cannot be reduced by an opposing character", 930) and set it
   * for the stat they name: 1515574 r3 has El Kaktus's `Protection : Damage` keep 5 Damage
   * against a Pussycats `-2 Opp Damage, Min 1` while the Power reduction beside it lands.
   * The Power and Attack forms and the clan-gated `After [clan:25]: Protection : Damage`
   * follow the same text; no capture has shown them meeting a reduction of their stat yet.
   * A conditional Power And Damage form sets it whenever its
   * condition holds, by composition: `Reprisal: Protect. Power And Damage` (tested; the Rust
   * engine admits it since semantic revision 75) and, unobserved in any capture, the Revenge
   * and Courage forms, which the Rust engine still refuses.
   *
   * Khrull Cr's `Protection: Cards Power And Damage` sets it on both cards: "The Power and
   * Damage of both characters cannot be reduced by the opposing character" (abilityData
   * 2255, sideAffected "both"). Its owner's own Dominion `Growth: -1 Opp Power, Min 4` is
   * refused on the opposing card - Twyh stays 5 x 4 = 20 in 1507713 r1, Nancy 7 x 1 + 12 =
   * 19 in 1507819 r2 - so a reduction is refused whichever side the Khrull Cr is on. Only
   * cross-card reductions: neither round shows an increase to a guarded stat, and the text
   * names reductions only, so increases still land.
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

  /**
   * A stat Protection sets only the guard: it does not shield its card's own modifiers of the
   * stat from an opposing `Cancel Opp. <stat> Modif.` ("Any modifier of the opposing character
   * affecting attack will be deactivated", captures/abilities.json 1163). Three rounds show the
   * Cancel winning. Gemini's `Protection: Attack` keeps nothing of its Hive bonus `Equalizer:
   * -3 Opp Attack, Min 5` against the Raptors bonus `Cancel Opp. Attack Modif.`: Sauropsite
   * fights at 8 x 7 = 56 in 1519871 r0 and St4rve Ld at 6 x 3 + 5 x 4 (Brawl) = 38 in 1521010
   * r0, where the Equalizer would take 9 and 6. Davis's `Protection : Damage` keeps nothing of
   * his La Junta `Damage +2` against Lenora's `Cancel Opp. Damage Modif.`: 3 Damage, not 5
   * (1520579 r1). A stat's `prot` used to be set here, which made its `blocked` false under the
   * Cancel. None of the three rounds shows whether the Cancel also switches off the guard (an
   * opposing reduction of the protected stat beside the Cancel); the guard stays.
   */
  apply(data: BattleData) {
    if (this.both) {
      switch (this.prot) {
        case Prot.POWER:
          if (this.guard) {
            data.oppCard.power.guard = true;
            data.card.power.guard = true;
          }
          break;
        case Prot.DAMAGE:
          if (this.guard) {
            data.oppCard.damage.guard = true;
            data.card.damage.guard = true;
          }
          break;
        case Prot.ATTACK:
          if (this.guard) {
            data.oppCard.attack.guard = true;
            data.card.attack.guard = true;
          }
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
          if (this.guard) data.card.power.guard = true;
          break;
        case Prot.DAMAGE:
          if (this.guard) data.card.damage.guard = true;
          break;
        case Prot.ATTACK:
          if (this.guard) data.card.attack.guard = true;
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
