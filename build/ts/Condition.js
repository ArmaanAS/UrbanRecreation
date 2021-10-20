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
        if (this.type == ConditionType.DEFEAT) {
            return data.player.won == false;
        }
        else if (this.type == ConditionType.NIGHT) {
            return data.round.day == false;
        }
        else if (this.type == ConditionType.DAY) {
            return data.round.day == true;
        }
        else if (this.type == ConditionType.COURAGE) {
            return data.round.first == true;
        }
        else if (this.type == ConditionType.REVENGE) {
            return data.player.wonPrevious == false;
        }
        else if (this.type == ConditionType.CONFIDENCE) {
            return data.player.wonPrevious == true;
        }
        else if (this.type == ConditionType.REPRISAL) {
            return data.round.first == false;
        }
        else if (this.type == ConditionType.KILLSHOT) {
            return data.card.attack.final >= data.oppCard.attack.final * 2;
        }
        else if (this.type == ConditionType.BACKLASH) {
            return data.player.won == true;
        }
        else if (this.type == ConditionType.REANIMATE) {
            if (data.player.life < 0)
                data.player.life = 0;
            return data.player.won == false && data.player.life <= 0;
        }
        else if (this.type == ConditionType.STOP) {
            if (this.stop == 'Ability') {
                return data.card.ability.cancel;
            }
            else if (this.stop == 'Bonus') {
                return data.card.bonus.cancel;
            }
        }
        else if (this.type == ConditionType.SYMMETRY) {
            return data.card.index == data.oppCard.index;
        }
        else if (this.type == ConditionType.ASYMMETRY) {
            return data.card.index != data.oppCard.index;
        }
        return true;
    }
    compile(data, ability) {
        if (this.type == ConditionType.BACKLASH) {
            for (let mod of ability.mods) {
                if (mod instanceof BasicModifier)
                    mod.setOpp(false);
            }
        }
        else if (this.type == ConditionType.BRAWL) {
            for (let mod of ability.mods) {
                if (mod instanceof BasicModifier)
                    mod.setPer('BRAWL');
            }
        }
        else if (this.type == ConditionType.SUPPORT) {
            for (let mod of ability.mods) {
                if (mod instanceof BasicModifier)
                    mod.setPer('SUPPORT');
            }
        }
        else if (this.type == ConditionType.GROWTH) {
            for (let mod of ability.mods) {
                if (mod instanceof BasicModifier)
                    mod.setPer('GROWTH');
            }
        }
        else if (this.type == ConditionType.DEGROWTH) {
            for (let mod of ability.mods) {
                if (mod instanceof BasicModifier)
                    mod.setPer('DEGROWTH');
            }
        }
        else if (this.type == ConditionType.EQUALIZER) {
            for (let mod of ability.mods) {
                if (mod instanceof BasicModifier)
                    mod.setPer('EQUALIZER');
            }
        }
        else if (this.type == ConditionType.DEFEAT) {
            for (let mod of ability.mods) {
                if (mod instanceof BasicModifier)
                    mod.win = false;
            }
        }
        else if (this.type == ConditionType["VICTORY OR DEFEAT"]) {
            for (let mod of ability.mods) {
                if (mod instanceof BasicModifier)
                    mod.win = false;
            }
        }
        else if (this.type == ConditionType.REANIMATE) {
            for (let mod of ability.mods) {
                if (mod instanceof BasicModifier)
                    mod.win = false;
            }
        }
        else if (this.type == ConditionType.STOP) {
            if (ability.type == AbilityType.ABILITY) {
                this.stop = 'Ability';
                data.card.ability.prot = true;
            }
            else if (ability.type == AbilityType.BONUS) {
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