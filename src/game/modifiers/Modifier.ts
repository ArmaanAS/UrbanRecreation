import BattleData from "../battle/BattleData.ts";
import EventTime from "../types/EventTime.ts";

export default abstract class Modifier {
  eventTime = EventTime.START;
  win?: boolean = undefined;
  static from(o: Modifier): Modifier {
    return Object.setPrototypeOf(o, Modifier.prototype);
  }
  abstract apply(data: BattleData): void;
}
