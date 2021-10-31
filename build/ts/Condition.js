import { AbilityType } from "./Ability";
import BasicModifier from "./modifiers/BasicModifier";
export var ConditionType;
(function (ConditionType) {
    ConditionType[ConditionType["UNDEFINED"] = 0] = "UNDEFINED";
    ConditionType[ConditionType["COURAGE"] = 1] = "COURAGE";
    ConditionType[ConditionType["DEFEAT"] = 2] = "DEFEAT";
    ConditionType[ConditionType["BRAWL"] = 3] = "BRAWL";
    ConditionType[ConditionType["GROWTH"] = 4] = "GROWTH";
    ConditionType[ConditionType["CONFIDENCE"] = 5] = "CONFIDENCE";
    ConditionType[ConditionType["DEGROWTH"] = 6] = "DEGROWTH";
    ConditionType[ConditionType["VICTORY OR DEFEAT"] = 7] = "VICTORY OR DEFEAT";
    ConditionType[ConditionType["EQUALIZER"] = 8] = "EQUALIZER";
    ConditionType[ConditionType["SUPPORT"] = 9] = "SUPPORT";
    ConditionType[ConditionType["TEAM"] = 10] = "TEAM";
    ConditionType[ConditionType["SYMMETRY"] = 11] = "SYMMETRY";
    ConditionType[ConditionType["REVENGE"] = 12] = "REVENGE";
    ConditionType[ConditionType["REPRISAL"] = 13] = "REPRISAL";
    ConditionType[ConditionType["DAY"] = 14] = "DAY";
    ConditionType[ConditionType["NIGHT"] = 15] = "NIGHT";
    ConditionType[ConditionType["KILLSHOT"] = 16] = "KILLSHOT";
    ConditionType[ConditionType["BACKLASH"] = 17] = "BACKLASH";
    ConditionType[ConditionType["ASYMMETRY"] = 18] = "ASYMMETRY";
    ConditionType[ConditionType["REANIMATE"] = 19] = "REANIMATE";
    ConditionType[ConditionType["STOP"] = 20] = "STOP";
})(ConditionType || (ConditionType = {}));
const ConditionTypes = {
    UNDEFINED: ConditionType.UNDEFINED,
    COURAGE: ConditionType.COURAGE,
    DEFEAT: ConditionType.DEFEAT,
    BRAWL: ConditionType.BRAWL,
    GROWTH: ConditionType.GROWTH,
    CONFIDENCE: ConditionType.CONFIDENCE,
    DEGROWTH: ConditionType.DEGROWTH,
    "VICTORY OR DEFEAT": ConditionType["VICTORY OR DEFEAT"],
    EQUALIZER: ConditionType.EQUALIZER,
    SUPPORT: ConditionType.SUPPORT,
    TEAM: ConditionType.TEAM,
    SYMMETRY: ConditionType.SYMMETRY,
    REVENGE: ConditionType.REVENGE,
    REPRISAL: ConditionType.REPRISAL,
    DAY: ConditionType.DAY,
    NIGHT: ConditionType.NIGHT,
    KILLSHOT: ConditionType.KILLSHOT,
    BACKLASH: ConditionType.BACKLASH,
    ASYMMETRY: ConditionType.ASYMMETRY,
    REANIMATE: ConditionType.REANIMATE,
    STOP: ConditionType.STOP,
};
export default class Condition {
    constructor(s) {
        var _a;
        this.s = s;
        this.type = (_a = ConditionTypes[s.toUpperCase()]) !== null && _a !== void 0 ? _a : ConditionType.UNDEFINED;
    }
    static from(o) {
        return Object.setPrototypeOf(o, Condition.prototype);
    }
    met(data) {
        switch (this.type) {
            case ConditionType.DEFEAT: return data.player.won == false;
            case ConditionType.NIGHT: return data.round.day == false;
            case ConditionType.DAY: return data.round.day == true;
            case ConditionType.COURAGE: return data.round.first == true;
            case ConditionType.REVENGE: return data.player.wonPrevious == false;
            case ConditionType.CONFIDENCE: return data.player.wonPrevious == true;
            case ConditionType.REPRISAL: return data.round.first == false;
            case ConditionType.KILLSHOT: return data.card.attack.final >=
                data.oppCard.attack.final * 2;
            case ConditionType.BACKLASH: return data.player.won == true;
            case ConditionType.REANIMATE:
                if (data.player.life < 0)
                    data.player.life = 0;
                return data.player.won == false && data.player.life <= 0;
            case ConditionType.STOP:
                if (this.stop == 'Ability')
                    return data.card.ability.cancel;
                else if (this.stop == 'Bonus')
                    return data.card.bonus.cancel;
                else
                    return true;
            case ConditionType.SYMMETRY: return data.card.index == data.oppCard.index;
            case ConditionType.ASYMMETRY: return data.card.index != data.oppCard.index;
        }
        return true;
    }
    compile(data, ability) {
        switch (this.type) {
            case ConditionType.BACKLASH:
                for (const mod of ability.mods) {
                    if (mod instanceof BasicModifier)
                        mod.setOpp(false);
                }
                break;
            case ConditionType.BRAWL:
                for (const mod of ability.mods) {
                    if (mod instanceof BasicModifier)
                        mod.setPer('BRAWL');
                }
                break;
            case ConditionType.SUPPORT:
                for (const mod of ability.mods) {
                    if (mod instanceof BasicModifier)
                        mod.setPer('SUPPORT');
                }
                break;
            case ConditionType.GROWTH:
                for (const mod of ability.mods) {
                    if (mod instanceof BasicModifier)
                        mod.setPer('GROWTH');
                }
                break;
            case ConditionType.DEGROWTH:
                for (const mod of ability.mods) {
                    if (mod instanceof BasicModifier)
                        mod.setPer('DEGROWTH');
                }
                break;
            case ConditionType.EQUALIZER:
                for (const mod of ability.mods) {
                    if (mod instanceof BasicModifier)
                        mod.setPer('EQUALIZER');
                }
                break;
            case ConditionType.DEFEAT:
                for (const mod of ability.mods) {
                    if (mod instanceof BasicModifier)
                        mod.win = false;
                }
                break;
            case ConditionType["VICTORY OR DEFEAT"]:
                for (const mod of ability.mods) {
                    if (mod instanceof BasicModifier)
                        mod.win = false;
                }
                break;
            case ConditionType.REANIMATE:
                for (const mod of ability.mods) {
                    if (mod instanceof BasicModifier)
                        mod.win = false;
                }
                break;
            case ConditionType.STOP:
                if (ability.type === AbilityType.ABILITY) {
                    this.stop = 'Ability';
                    data.card.ability.prot = true;
                }
                else if (ability.type === AbilityType.BONUS) {
                    this.stop = 'Bonus';
                    data.card.bonus.prot = true;
                }
        }
    }
    static normalise(c) {
        return c
            .replace(/vic.*/gi, 'Victory Or Defeat')
            .replace(/conf.*/gi, 'Confidence');
    }
}
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiQ29uZGl0aW9uLmpzIiwic291cmNlUm9vdCI6Ii9DOi9Vc2Vycy9TdHVkZW50L0RvY3VtZW50cy9Ob2RlSlNXb3Jrc3BhY2UvVXJiYW5SZWNyZWF0aW9uL3NyYy8iLCJzb3VyY2VzIjpbInRzL0NvbmRpdGlvbi50cyJdLCJuYW1lcyI6W10sIm1hcHBpbmdzIjoiQUFBQSxPQUFnQixFQUFFLFdBQVcsRUFBRSxNQUFNLFdBQVcsQ0FBQztBQUVqRCxPQUFPLGFBQWEsTUFBTSwyQkFBMkIsQ0FBQztBQUV0RCxNQUFNLENBQU4sSUFBWSxhQXNCWDtBQXRCRCxXQUFZLGFBQWE7SUFDdkIsMkRBQWEsQ0FBQTtJQUNiLHVEQUFXLENBQUE7SUFDWCxxREFBVSxDQUFBO0lBQ1YsbURBQVMsQ0FBQTtJQUNULHFEQUFVLENBQUE7SUFDViw2REFBYyxDQUFBO0lBQ2QseURBQVksQ0FBQTtJQUNaLDJFQUF1QixDQUFBO0lBQ3ZCLDJEQUFhLENBQUE7SUFDYix1REFBVyxDQUFBO0lBQ1gsa0RBQVMsQ0FBQTtJQUNULDBEQUFhLENBQUE7SUFDYix3REFBWSxDQUFBO0lBQ1osMERBQWEsQ0FBQTtJQUNiLGdEQUFRLENBQUE7SUFDUixvREFBVSxDQUFBO0lBQ1YsMERBQWEsQ0FBQTtJQUNiLDBEQUFhLENBQUE7SUFDYiw0REFBYyxDQUFBO0lBQ2QsNERBQWMsQ0FBQTtJQUNkLGtEQUFTLENBQUE7QUFDWCxDQUFDLEVBdEJXLGFBQWEsS0FBYixhQUFhLFFBc0J4QjtBQUNELE1BQU0sY0FBYyxHQUVoQjtJQUNGLFNBQVMsRUFBRSxhQUFhLENBQUMsU0FBUztJQUNsQyxPQUFPLEVBQUUsYUFBYSxDQUFDLE9BQU87SUFDOUIsTUFBTSxFQUFFLGFBQWEsQ0FBQyxNQUFNO0lBQzVCLEtBQUssRUFBRSxhQUFhLENBQUMsS0FBSztJQUMxQixNQUFNLEVBQUUsYUFBYSxDQUFDLE1BQU07SUFDNUIsVUFBVSxFQUFFLGFBQWEsQ0FBQyxVQUFVO0lBQ3BDLFFBQVEsRUFBRSxhQUFhLENBQUMsUUFBUTtJ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