import { Abilities, AbilityParser } from "./AbilityParser";
import Condition from "./Condition";
import BasicModifier from "./modifiers/BasicModifier";
import CancelModifier from "./modifiers/CancelModifier";
import CopyModifier from "./modifiers/CopyModifier";
import Modifier from "./modifiers/Modifier";
import ProtectionModifier from "./modifiers/ProtectionModifier";
import { clone } from "./utils/Utils";
export var AbilityType;
(function (AbilityType) {
    AbilityType[AbilityType["UNDEFINED"] = 0] = "UNDEFINED";
    AbilityType[AbilityType["GLOBAL"] = 1] = "GLOBAL";
    AbilityType[AbilityType["ABILITY"] = 2] = "ABILITY";
    AbilityType[AbilityType["BONUS"] = 3] = "BONUS";
})(AbilityType || (AbilityType = {}));
export default class Ability {
    constructor(s, type = AbilityType.UNDEFINED) {
        this.defer = false;
        this.mods = [];
        const conditions = Abilities.split(s);
        this.ability = conditions.pop();
        this.type = type;
        this.conditions = conditions
            .map(Condition.normalise)
            .map((s) => new Condition(s));
    }
    clone() {
        return Object.setPrototypeOf({
            conditions: this.conditions.map(clone),
            ability: this.ability,
            type: this.type,
            defer: this.defer,
            mods: this.mods.map(clone),
        }, Ability.prototype);
    }
    static from(o) {
        Object.setPrototypeOf(o, Ability.prototype);
        o.conditions = o.conditions.map(Condition.from);
        o.mods = o.mods.map(Modifier.from);
        return o;
    }
    canApply(data) {
        for (let cond of this.conditions) {
            if (!cond.met(data)) {
                console.log(`[Condition] ${cond.s} met: false`.yellow.dim);
                return false;
            }
            console.log(`[Condition] ${cond.s} met: true`.green);
        }
        if (this.type == AbilityType.ABILITY)
            return data.card.ability.prot || !data.card.ability.cancel;
        else if (this.type == AbilityType.BONUS)
            return data.card.bonus.prot || !data.card.bonus.cancel;
        return true;
    }
    compileConditions(data) {
        for (let cond of this.conditions) {
            cond.compile(data, this);
        }
    }
    compileAbility(data) {
        let failed = true;
        let tokens = this.ability.split(" ");
        compile: if (/\+\d+/.test(tokens[0])) {
            let t, i, opp = false;
            if (tokens[1] == "Opp") {
                t = [tokens[2]];
                i = 2;
                opp = true;
            }
            else if (tokens[1].includes("&")) {
                t = tokens[1].split("&");
                i = 1;
            }
            else {
                t = [tokens[1]];
                i = 1;
            }
            for (let a of t) {
                let mod = new BasicModifier();
                mod.change = +tokens[0];
                mod.setOpp(opp);
                if (["Power", "Damage", "Life", "Pillz", "Attack"].includes(a)) {
                    failed = false;
                    mod.setType(a);
                }
                else {
                    failed = true;
                    console.log("Unknown token[1]:".red + '"' + tokens[1] + '"', this.ability);
                    break compile;
                }
                AbilityParser.minmaxper(tokens, i + 1, mod);
                this.mods.push(mod);
            }
        }
        else if (/-\d+/.test(tokens[0])) {
            let dupe = ["Xantiax", "Cards"].includes(tokens[1]);
            let t, i = 1;
            if (dupe) {
                i = 2;
            }
            if (tokens[i].includes("&")) {
                t = tokens[i].split("&");
            }
            else {
                t = [tokens[i]];
            }
            for (let a of t) {
                let mod = new BasicModifier();
                mod.change = +tokens[0];
                mod.setOpp(true);
                failed = false;
                if (["Power", "Damage", "Life", "Pillz", "Attack"].includes(a)) {
                    mod.setType(a);
                }
                else {
                    console.log(`Unknown token[${i}]: `.red + `"${tokens[i]}"`, this.ability);
                    break compile;
                }
                AbilityParser.minmaxper(tokens, i + 1, mod);
                this.mods.push(mod);
            }
            if (dupe) {
                let newMods = [];
                for (let mod of this.mods) {
                    mod.win = false;
                    const cloned = clone(mod);
                    if (cloned instanceof BasicModifier)
                        cloned.setOpp(false);
                    newMods.push(cloned);
                }
                this.mods = [...this.mods, ...newMods];
            }
        }
        else if (tokens[0] == "Stop") {
            failed = false;
            this.mods.push(new CancelModifier(tokens[1]));
        }
        else if (tokens[0] == "Cancel") {
            failed = false;
            if (tokens[1].includes("&")) {
                for (let t of tokens[1].split("&")) {
                    this.mods.push(new CancelModifier(t));
                }
            }
            else if (tokens[1] == "Leader") {
                return;
            }
            else {
                this.mods.push(new CancelModifier(tokens[1]));
            }
        }
        else if (tokens[0] == "Protection") {
            failed = false;
            if (tokens[1].includes("&")) {
                for (let prot of tokens[1].split("&")) {
                    this.mods.push(new ProtectionModifier(prot));
                }
            }
            else {
                this.mods.push(new ProtectionModifier(tokens[1]));
            }
        }
        else if (tokens[0] == "Copy") {
            failed = false;
            if (tokens[1] == "Bonus") {
                new Ability(data.oppCard.bonus.string, this.type).compile(data);
                return;
            }
            else {
                if (tokens[1].includes("&")) {
                    for (let c of tokens[1].split("&")) {
                        this.mods.push(new CopyModifier(c));
                    }
                }
                else {
                    this.mods.push(new CopyModifier(tokens[1]));
                }
            }
        }
        else if (tokens[0] == "No") {
            return;
        }
        else if (tokens[0] == "Counter-Attack") {
            return;
        }
        if (!failed) {
            console.log(`[Added] ${this.ability}`.green);
            if (this.mods.length) {
                data.events.add(this.mods[0].eventTime, this);
            }
        }
        else {
            console.log(`[Failed] ${this.ability}`.red);
        }
    }
    compile(data) {
        this.compileAbility(data);
        this.compileConditions(data);
    }
    apply(data) {
        if (this.canApply(data)) {
            for (let mod of this.mods) {
                console.log(`Applying modifier... (${this.ability})`);
                mod.apply(data);
            }
        }
    }
    static card(card, data) {
        new Ability(card.ability.string, AbilityType.ABILITY).compile(data);
        new Ability(card.bonus.string, AbilityType.BONUS).compile(data);
    }
    static leader(card, data) {
        new Ability(card.ability.string, AbilityType.GLOBAL).compile(data);
    }
}
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiQWJpbGl0eS5qcyIsInNvdXJjZVJvb3QiOiIvQzovVXNlcnMvU3R1ZGVudC9Eb2N1bWVudHMvTm9kZUpTV29ya3NwYWNlL1VyYmFuUmVjcmVhdGlvbi9zcmMvIiwic291cmNlcyI6WyJ0cy9BYmlsaXR5LnRzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUFBLE9BQU8sRUFBRSxTQUFTLEVBQUUsYUFBYSxFQUFFLE1BQU0saUJBQWlCLENBQUM7QUFHM0QsT0FBTyxTQUFTLE1BQU0sYUFBYSxDQUFDO0FBQ3BDLE9BQU8sYUFBYSxNQUFNLDJCQUEyQixDQUFDO0FBQ3RELE9BQU8sY0FBYyxNQUFNLDRCQUE0QixDQUFDO0FBQ3hELE9BQU8sWUFBWSxNQUFNLDBCQUEwQixDQUFDO0FBQ3BELE9BQU8sUUFBUSxNQUFNLHNCQUFzQixDQUFDO0FBQzVDLE9BQU8sa0JBQWtCLE1BQU0sZ0NBQWdDLENBQUM7QUFDaEUsT0FBTyxFQUFFLEtBQUssRUFBRSxNQUFNLGVBQWUsQ0FBQztBQVl0QyxNQUFNLENBQU4sSUFBWSxXQUtYO0FBTEQsV0FBWSxXQUFXO0lBQ3JCLHVEQUFhLENBQUE7SUFDYixpREFBVSxDQUFBO0lBQ1YsbURBQVcsQ0FBQTtJQUNYLCtDQUFTLENBQUE7QUFDWCxDQUFDLEVBTFcsV0FBVyxLQUFYLFdBQVcsUUFLdEI7QUFFRCxNQUFNLENBQUMsT0FBTyxPQUFPLE9BQU87SUFPMUIsWUFBWSxDQUFTLEVBQUUsSUFBSSxHQUFHLFdBQVcsQ0FBQyxTQUFTO1FBSG5ELFVBQUssR0FBWSxLQUFLLENBQUM7UUFDdkIsU0FBSSxHQUFlLEVBQUUsQ0FBQztRQUdwQixNQUFNLFVBQVUsR0FBRyxTQUFTLENBQUMsS0FBSyxDQUFDLENBQUMsQ0FBQyxDQUFDO1FBQ3RDLElBQUksQ0FBQyxPQUFPLEdBQUcsVUFBVSxDQUFDLEdBQUcsRUFBRyxDQUFDO1FBRWpDLElBQUksQ0FBQyxJQUFJLEdBQUcsSUFBSSxDQUFDO1FBRWpCLElBQUksQ0FBQyxVQUFVLEdBQUcsVUFBVTth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