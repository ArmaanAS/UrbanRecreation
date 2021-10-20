import BattleData from "../BattleData";
import { EventTime } from "../Events";


export default class Modifier {
  eventTime = EventTime.START;
  win?: boolean = undefined;
  static from(o: Modifier): Modifier {
    return Object.setPrototypeOf(o, Modifier.prototype)
  }
  apply(data: BattleData) { }
}