import BattleData from "../battle/BattleData";
import EventTime from "../types/EventTime";


export default abstract class Modifier {
  eventTime = EventTime.START;
  win?: boolean = undefined;
  static from(o: Modifier): Modifier {
    return Object.setPrototypeOf(o, Modifier.prototype)
  }
  abstract apply(data: BattleData): void;
}