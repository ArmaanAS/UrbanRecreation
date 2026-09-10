import BattleData from "../battle/BattleData.ts";
import EventTime from "../types/EventTime.ts";
import Modifier from "./Modifier.ts";

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
  }

  apply(data: BattleData) {
    if (this.rec === Recover.PILLZ) {
      if (!data.card.pillz.blocked) {
        // Rounding is always up, and a triggered Recover never gives nothing: captured
        // battle 877983 r1 has Eebiza "Defeat: Recover 1 Pillz Out Of 2" lose on a bet of 0
        // and still recover 1, where the proportion alone would be 0. 901400 r1 pins the
        // proportional half - D-aleq "Recover 2 Pillz Out Of 3" loses on a bet of 3 and
        // recovers 2, so the free pill is not part of the count.
        const gain = Math.max(
          1,
          Math.ceil(data.playerPillzUsed * (this.n / this.outOf)),
        );
        console.log(`Player recovered ${gain} pillz / ${data.playerPillzUsed}`);
        data.player.pillz += gain;

        if (this.both) {
          const oppGain = Math.ceil(
            data.oppCard.damage.final * (this.n / this.outOf),
          );
          console.log(
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
        console.log(
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
