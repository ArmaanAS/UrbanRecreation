import BattleData from "../battle/BattleData.ts";
import EventTime from "../types/EventTime.ts";
import Modifier from "./Modifier.ts";
import { DEBUG } from "../../utils/Debug.ts";
enum Recover {

  PILLZ = 1,
  LIFE = 2,
}
export const RecoverObject: { [index: string]: Recover } = {
  PILLZ: Recover.PILLZ,
  LIFE: Recover.LIFE,
};

export default class RecoverModifier extends Modifier {
  rec = Recover.PILLZ;
  n: number;
  outOf: number;
  both: boolean;
  constructor(
    rec: Recover | string,
    n: number,
    outOf: number,
    both = false,
    et = EventTime.END,
  ) {
    super();

    if (typeof rec == "string") {
      this.rec = RecoverObject[rec.toUpperCase()];
    } else {
      this.rec = rec;
    }

    this.n = n;
    this.outOf = outOf;
    this.both = both;
    this.eventTime = et;
    // The plain and Unison forms pay only when their card wins (abilityData
    // currentRoundRequirement "win" for 3459/3752/4050/4610/5651, "If Costello wins the
    // fight..."): Costello's "Recover 1 Pillz Out Of 3" loses in 946810 r0 and the server
    // posts no pillz increase. Condition.compile clears this for Defeat and Victory Or
    // Defeat, whose own conditions decide the outcome instead.
    this.win = true;
  }

  apply(data: BattleData) {
    if (this.win && data.card.won !== true) return;
    if (this.rec === Recover.PILLZ) {
      if (!data.card.pillz.blocked) {
        // The server returns the stated share of the Pillz placed on the card - the bet,
        // the free pill and Fury's three - rounded down, with a minimum of 1 (the long
        // description of every Recover in captures/abilities.json). Kyrioz Ld bets 7 on
        // "Recover 1 Pillz Out Of 3" in 947010 r0 and recovers floor(8 / 3) = 2, not
        // ceil(7 / 3) = 3; 877983 r1 has Eebiza "Defeat: Recover 1 Pillz Out Of 2" lose on
        // a bet of 0 and still recover 1; 1024592 r1 pins the Fury term, floor(8 x 2/3) = 5
        // on a bet of 4 with Fury. For 1/2 and 2/3 this equals the old ceil(bet x N / M).
        const gain = Math.max(
          1,
          Math.floor((data.playerPillzUsed + 1) * this.n / this.outOf),
        );
        if (DEBUG) console.log(`Player recovered ${gain} pillz / ${data.playerPillzUsed + 1}`);
        data.player.pillz += gain;

        if (this.both) {
          const oppGain = Math.ceil(
            data.oppCard.damage.final * (this.n / this.outOf),
          );
          if (DEBUG) console.log(
            `Opponent recovered ${oppGain} life / ${data.oppCard.damage.final} damage`,
          );
          data.opp.life += oppGain;
        }
      }
    } else if (this.rec === Recover.LIFE) {
      if (!data.card.life.blocked) {
        const gain = Math.ceil(
          data.oppCard.damage.final * (this.n / this.outOf),
        );
        if (DEBUG) console.log(
          `Player recovered ${gain} life / ${data.oppCard.damage.final} damage`,
        );
        data.player.life += gain;

        if (this.both) {
          throw new Error("Both is not supported for life recovery");
        }
      }
    } else console.error("Unknown recover type", this.rec);
  }
}
