import EventTime from "../types/EventTime";
import Modifier from "./Modifier";
var Cancel;
(function (Cancel) {
    Cancel[Cancel["POWER"] = 1] = "POWER";
    Cancel[Cancel["DAMAGE"] = 2] = "DAMAGE";
    Cancel[Cancel["ATTACK"] = 3] = "ATTACK";
    Cancel[Cancel["ABILITY"] = 4] = "ABILITY";
    Cancel[Cancel["BONUS"] = 5] = "BONUS";
    Cancel[Cancel["PILLZ"] = 6] = "PILLZ";
    Cancel[Cancel["LIFE"] = 7] = "LIFE";
})(Cancel || (Cancel = {}));
export const CancelObject = {
    POWER: Cancel.POWER,
    DAMAGE: Cancel.DAMAGE,
    ATTACK: Cancel.ATTACK,
    ABILITY: Cancel.ABILITY,
    BONUS: Cancel.BONUS,
    PILLZ: Cancel.PILLZ,
    LIFE: Cancel.LIFE,
};
export default class CancelModifier extends Modifier {
    constructor(cancel, et = EventTime.PRE3) {
        super();
        this.cancel = Cancel.POWER;
        if (typeof cancel == "string")
            this.cancel = CancelObject[cancel.toUpperCase()];
        else
            this.cancel = cancel;
        this.eventTime = et;
    }
    apply(data) {
        switch (this.cancel) {
            case Cancel.POWER:
                data.oppCard.power.cancel = true;
                break;
            case Cancel.DAMAGE:
                data.oppCard.damage.cancel = true;
                break;
            case Cancel.ATTACK:
                data.oppCard.attack.cancel = true;
                break;
            case Cancel.PILLZ:
                data.oppCard.pillz.cancel = true;
                break;
            case Cancel.LIFE:
                data.oppCard.life.cancel = true;
                break;
            case Cancel.ABILITY:
                data.oppCard.ability.cancel = true;
                break;
            case Cancel.BONUS:
                data.oppCard.bonus.cancel = true;
                break;
        }
    }
}
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiQ2FuY2VsTW9kaWZpZXIuanMiLCJzb3VyY2VSb290IjoiL0M6L1VzZXJzL1N0dWRlbnQvRG9jdW1lbnRzL05vZGVKU1dvcmtzcGFjZS9VcmJhblJlY3JlYXRpb24vc3JjLyIsInNvdXJjZXMiOlsidHMvbW9kaWZpZXJzL0NhbmNlbE1vZGlmaWVyLnRzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUNBLE9BQU8sU0FBUyxNQUFNLG9CQUFvQixDQUFDO0FBQzNDLE9BQU8sUUFBUSxNQUFNLFlBQVksQ0FBQztBQUVsQyxJQUFLLE1BUUo7QUFSRCxXQUFLLE1BQU07SUFDVCxxQ0FBUyxDQUFBO0lBQ1QsdUNBQVUsQ0FBQTtJQUNWLHVDQUFVLENBQUE7SUFDVix5Q0FBVyxDQUFBO0lBQ1gscUNBQVMsQ0FBQTtJQUNULHFDQUFTLENBQUE7SUFDVCxtQ0FBUSxDQUFBO0FBQ1YsQ0FBQyxFQVJJLE1BQU0sS0FBTixNQUFNLFFBUVY7QUFDRCxNQUFNLENBQUMsTUFBTSxZQUFZLEdBQThCO0lBQ3JELEtBQUssRUFBRSxNQUFNLENBQUMsS0FBSztJQUNuQixNQUFNLEVBQUUsTUFBTSxDQUFDLE1BQU07SUFDckIsTUFBTSxFQUFFLE1BQU0sQ0FBQyxNQUFNO0lBQ3JCLE9BQU8sRUFBRSxNQUFNLENBQUMsT0FBTztJQUN2QixLQUFLLEVBQUUsTUFBTSxDQUFDLEtBQUs7SUFDbkIsS0FBSyxFQUFFLE1BQU0sQ0FBQyxLQUFLO0lBQ25CLElBQUksRUFBRSxNQUFNLENBQUMsSUFBSTtDQUNsQixDQUFBO0FBRUQsTUFBTSxDQUFDLE9BQU8sT0FBTyxjQUFlLFNBQVEsUUFBUTtJQUVsRCxZQUFZLE1BQXVCLEVBQUUsRUFBRSxHQUFHLFNBQVMsQ0FBQyxJQUFJO1FBQ3RELEtBQUssRUFBRSxDQUFDO1FBRlYsV0FBTSxHQUFHLE1BQU0sQ0FBQyxLQUFLLENBQUM7UUFJcEIsSUFBSSxPQUFPLE1BQU0sSUFBSSxRQUFRO1lBQzNCLElBQUksQ0FBQyxNQUFNLEdBQUcsWUFBWSxDQUFDLE1BQU0sQ0FBQyxXQUFXLEVBQUUsQ0FBQyxDQUFBOztZQUVoRCxJQUFJLENBQUMsTUFBTSxHQUFHLE1BQU0sQ0FBQztRQUN2QixJQUFJLENBQUMsU0FBUyxHQUFHLEVBQUUsQ0FBQztJQUN0QixDQUFDO0lBRUQsS0FBSyxDQUFDLElBQWdCO1FBZ0JwQixRQUFRLElBQUksQ0FBQyxNQUFNLEVBQUU7WUFRbkIsS0FBSyxNQUFNLENBQUMsS0FBSztnQkFBRSxJQUFJLENBQUMsT0FBTyxDQUFDLEtBQUssQ0FBQyxNQUFNLEdBQUcsSUFBSSxDQUFDO2dCQUFDLE1BQU07WUFDM0QsS0FBSyxNQUFNLENBQUMsTUFBTTtnQkFBRSxJQUFJLENBQUMsT0FBTyxDQUFDLE1BQU0sQ0FBQyxNQUFNLEdBQUcsSUFBSSxDQUFDO2dCQUFDLE1BQU07WUFDN0QsS0FBSyxNQUFNLENBQUMsTUFBTTtnQkFBRSxJQUFJLENBQUMsT0FBTyxDQUFDLE1BQU0sQ0FBQyxNQUFNLEdBQUcsSUFBSSxDQUFDO2dCQUFDLE1BQU07WUFDN0QsS0FBSyxNQUFNLENBQUMsS0FBSztnQkFBRSxJQUFJLENBQUMsT0FBTyxDQUFDLEtBQUssQ0FBQyxNQUFNLEdBQUcsSUFBSSxDQUFDO2dCQUFDLE1BQU07WUFDM0QsS0FBSyxNQUFNLENBQUMsSUFBSTtnQkFBRSxJQUFJLENBQUMsT0FBTyxDQUFDLElBQUksQ0FBQyxNQUFNLEdBQUcsSUFBSSxDQUFDO2dCQUFDLE1BQU07WUFDekQsS0FBSyxNQUFNLENBQUMsT0FBTztnQkFBRSxJQUFJLENBQUMsT0FBTyxDQUFDLE9BQU8sQ0FBQyxNQUFNLEdBQUcsSUFBSSxDQUFDO2dCQUFDLE1BQU07WUFDL0QsS0FBSyxNQUFNLENBQUMsS0FBSztnQkFBRSxJQUFJLENBQUMsT0FBTyxDQUFDLEtBQUssQ0FBQyxNQUFNLEdBQUcsSUFBSSxDQUFDO2dCQUFDLE1BQU07U0FDNUQ7SUFDSCxDQUFDO0NBQ0YifQ==