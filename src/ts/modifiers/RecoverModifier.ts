import BattleData from "../BattleData";
import EventTime from "../types/EventTime";
import Modifier from "./Modifier";

enum Recover {
  PILLZ = 1,
  LIFE = 2,
}
export const RecoverObject: { [index: string]: Recover } = {
  PILLZ: Recover.PILLZ,
  LIFE: Recover.LIFE,
}

export default class ExchangeModifier extends Modifier {
  rec = Recover.PILLZ;
  n: number;
  outOf: number;
  constructor(rec: Recover | string, n: number, outOf: number, et = EventTime.POST2) {
    super();

    if (typeof rec == 'string')
      this.rec = RecoverObject[rec.toUpperCase()];
    else
      this.rec = rec;

    this.n = n;
    this.outOf = outOf;
    this.eventTime = et;
  }

  apply(data: BattleData) {
    if (this.rec === Recover.PILLZ) {
      if (!data.card.pillz.blocked) {
        const gain = Math.ceil(data.playerPillzUsed * (this.n / this.outOf));
        console.log(`Player recovered ${gain} pillz / ${data.playerPillzUsed}`);
        data.player.pillz += gain;
      }
    } else if (this.rec === Recover.LIFE) {
      if (!data.card.life.blocked) {
        const gain = Math.ceil(data.oppCard.damage.final * (this.n / this.outOf));
        console.log(`Player recovered ${gain} life / ${data.oppCard.damage.final} damage`);
        data.player.life += gain;
      }
    } else console.error('Unknown recover type', this.rec);
  }
}