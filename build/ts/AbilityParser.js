const abilityCache = {};
export class Abilities {
    static normalise(ability) {
        return ability
            .replace(/(?<=[+-]) (?=[xy])/gi, "")
            .replace(/[,.]/g, "")
            .replace(/ :/g, ":");
    }
    static splitConditions(ability) {
        return ability.map(s => s.split(/(?<=\w+) ?: /g));
    }
    static split(ability) {
        if (abilityCache[ability] !== undefined)
            return [...abilityCache[ability]];
        const s = ability
            .replace(/(?<=[+-]) (?=[xy\d])/gi, "")
            .replace(/,(?! )/gi, " ")
            .replace(/[,.]| (?=:)/gi, "")
            .replace(/At\w*/g, "Attack")
            .replace(/Prot\w*:?/g, "Protection")
            .replace(/Copy:/gi, "Copy")
            .replace("Xantiax:", "Xantiax")
            .replace(/(Dmg|Dam)\w*/gi, "Damage")
            .replace(/Pow\w*/gi, "Power")
            .replace(/Can\w*/gi, "Cancel")
            .replace(/Prot\w*/gi, "Protection")
            .replace(/Rec\w*/gi, "Recover")
            .replace(/&/g, "And")
            .replace(/(?<=(Copy|Cancel|Stop).*) (Opp|Mod|Left)\w*/gi, "")
            .replace("Bonus Protection", "Protection Bonus")
            .replace(/(\w+(?: \w+ \w+)?) ([+-][xy\d]+|Exchange)/i, "$2 $1")
            .replace(/([a-z]+)(?<!Min|Max) ([xy\d]+)/i, "$2 $1")
            .replace(/(\w+) And (\w+)/gi, "$1&$2")
            .replace(/(?<=[xy\d] )(\w+) (Opp)/gi, "$2 $1")
            .replace(/(?<=-[xy\d]+ )Opp /gi, "")
            .replace(/\b\w(?=\w+)/g, (s) => s.toUpperCase())
            .split(/(?<=\w+) ?[:;] /gi);
        abilityCache[ability] = s;
        return [...s];
    }
}
export class AbilityParser {
    static minmax(tokens, i, mod) {
        if (tokens[i] == "Min") {
            mod.setMin(+tokens[i + 1]);
            return true;
        }
        else if (tokens[i] == "Max") {
            mod.setMax(+tokens[i + 1]);
            return true;
        }
        return false;
    }
    static per(tokens, i, mod) {
        if (tokens[i] == "Per") {
            if (tokens[i + 1] == "Opp")
                mod.setPer(tokens[i + 2], true);
            else
                mod.setPer(tokens[i + 1]);
            return true;
        }
        return false;
    }
    static minmaxper(tokens, i, mod) {
        if (AbilityParser.minmax(tokens, i, mod))
            return true;
        else if (AbilityParser.per(tokens, i, mod)) {
            AbilityParser.minmax(tokens, i + 2, mod);
            return true;
        }
        return false;
    }
}
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiQWJpbGl0eVBhcnNlci5qcyIsInNvdXJjZVJvb3QiOiIvQzovVXNlcnMvU3R1ZGVudC9Eb2N1bWVudHMvTm9kZUpTV29ya3NwYWNlL1VyYmFuUmVjcmVhdGlvbi9zcmMvIiwic291cmNlcyI6WyJ0cy9BYmlsaXR5UGFyc2VyLnRzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUVBLE1BQU0sWUFBWSxHQUVkLEVBQUUsQ0FBQztBQUVQLE1BQU0sT0FBTyxTQUFTO0lBQ3BCLE1BQU0sQ0FBQyxTQUFTLENBQUMsT0FBZTtRQUM5QixPQUFPLE9BQU87YUFDWCxPQUFPLENBQUMsc0JBQXNCLEVBQUUsRUFBRSxDQUFDO2FBQ25DLE9BQU8sQ0FBQyxPQUFPLEVBQUUsRUFBRSxDQUFDO2FBQ3BCLE9BQU8sQ0FBQyxLQUFLLEVBQUUsR0FBRyxDQUFDLENBQUM7SUFDekIsQ0FBQztJQUVELE1BQU0sQ0FBQyxlQUFlLENBQUMsT0FBaUI7UUFDdEMsT0FBTyxPQUFPLENBQUMsR0FBRyxDQUFDLENBQUMsQ0FBQyxFQUFFLENBQUMsQ0FBQyxDQUFDLEtBQUssQ0FBQyxlQUFlLENBQUMsQ0FBQyxDQUFDO0lBQ3BELENBQUM7SUFFRCxNQUFNLENBQUMsS0FBSyxDQUFDLE9BQWU7UUFDMUIsSUFBSSxZQUFZLENBQUMsT0FBTyxDQUFDLEtBQUssU0FBUztZQUNyQyxPQUFPLENBQUMsR0FBRyxZQUFZLENBQUMsT0FBTyxDQUFDLENBQUMsQ0FBQztRQUVwQyxNQUFNLENBQUMsR0FBRyxPQUFPO2FBQ2QsT0FBTyxDQUFDLHdCQUF3QixFQUFFLEVBQUUsQ0FBQzthQUNyQyxPQUFPLENBQUMsVUFBVSxFQUFFLEdBQUcsQ0FBQzthQUN4QixPQUFPLENBQUMsZUFBZSxFQUFFLEVBQUUsQ0FBQzthQUU1QixPQUFPLENBQUMsUUFBUSxFQUFFLFFBQVEsQ0FBQzthQUMzQixPQUFPLENBQUMsWUFBWSxFQUFFLFlBQVksQ0FBQzthQUNuQyxPQUFPLENBQUMsU0FBUyxFQUFFLE1BQU0sQ0FBQzthQUMx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