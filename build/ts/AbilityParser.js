const abilityCache = {};
export class Abilities {
    static normalise(ability) {
        return ability
            .replace(/(?<=[\+\-]) (?=[xy])/gi, "")
            .replace(/[,\.]/g, "")
            .replace(/ :/g, ":");
    }
    static splitConditions(ability) {
        return ability.map(s => s.split(/(?<=\w+) ?: /g));
    }
    static split(ability) {
        if (abilityCache.hasOwnProperty(ability)) {
            return Array.from(abilityCache[ability]);
        }
        const s = ability
            .replace(/(?<=[\+\-]) (?=[xy\d])/gi, "")
            .replace(/,(?! )/gi, " ")
            .replace(/[,\.]| (?=:)/gi, "")
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
        return Array.from(s);
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
            if (tokens[i + 1] == "Opp") {
                mod.setPer(tokens[i + 2], true);
            }
            else {
                mod.setPer(tokens[i + 1]);
            }
            return true;
        }
        return false;
    }
    static minmaxper(tokens, i, mod) {
        if (AbilityParser.minmax(tokens, i, mod)) {
            return true;
        }
        else if (AbilityParser.per(tokens, i, mod)) {
            AbilityParser.minmax(tokens, i + 2, mod);
            return true;
        }
        return false;
    }
}
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiQWJpbGl0eVBhcnNlci5qcyIsInNvdXJjZVJvb3QiOiIvQzovVXNlcnMvU3R1ZGVudC9Eb2N1bWVudHMvTm9kZUpTV29ya3NwYWNlL1VyYmFuUmVjcmVhdGlvbi9zcmMvIiwic291cmNlcyI6WyJ0cy9BYmlsaXR5UGFyc2VyLnRzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUVBLE1BQU0sWUFBWSxHQUVkLEVBQUUsQ0FBQztBQUVQLE1BQU0sT0FBTyxTQUFTO0lBQ3BCLE1BQU0sQ0FBQyxTQUFTLENBQUMsT0FBZTtRQUM5QixPQUFPLE9BQU87YUFDWCxPQUFPLENBQUMsd0JBQXdCLEVBQUUsRUFBRSxDQUFDO2FBQ3JDLE9BQU8sQ0FBQyxRQUFRLEVBQUUsRUFBRSxDQUFDO2FBQ3JCLE9BQU8sQ0FBQyxLQUFLLEVBQUUsR0FBRyxDQUFDLENBQUM7SUFDekIsQ0FBQztJQUVELE1BQU0sQ0FBQyxlQUFlLENBQUMsT0FBaUI7UUFDdEMsT0FBTyxPQUFPLENBQUMsR0FBRyxDQUFDLENBQUMsQ0FBQyxFQUFFLENBQUMsQ0FBQyxDQUFDLEtBQUssQ0FBQyxlQUFlLENBQUMsQ0FBQyxDQUFDO0lBQ3BELENBQUM7SUFFRCxNQUFNLENBQUMsS0FBSyxDQUFDLE9BQWU7UUFDMUIsSUFBSSxZQUFZLENBQUMsY0FBYyxDQUFDLE9BQU8sQ0FBQyxFQUFFO1lBQ3hDLE9BQU8sS0FBSyxDQUFDLElBQUksQ0FBQyxZQUFZLENBQUMsT0FBTyxDQUFDLENBQUMsQ0FBQztTQUMxQztRQUVELE1BQU0sQ0FBQyxHQUFHLE9BQU87YUFDZCxPQUFPLENBQUMsMEJBQTBCLEVBQUUsRUFBRSxDQUFDO2FBQ3ZDLE9BQU8sQ0FBQyxVQUFVLEVBQUUsR0FBRyxDQUFDO2FBQ3hCLE9BQU8sQ0FBQyxnQkFBZ0IsRUFBRSxFQUFFLENBQUM7YUFFN0IsT0FBTyxDQUFDLFFBQVEsRUFBRSxRQUFRLENBQUM7YUFDM0IsT0FBTyxDQUFDLFlBQVksRUFBRSxZQUFZLENBQUM7YUFDbkMsT0FBTyxDQUFDLFNBQVMsRUFBRSxNQUFNLENBQUM7YUFDMUIsT0FBTyxDQUFDLFVBQVUsRUFBRSxTQUFTLENBQUM7YUFDOUIsT0FBTyxDQUFDLGdCQUFnQixFQUFFLFFBQVEsQ0FBQzthQUNuQyxPQUFPLENBQUMsVUFBVSxFQUFFLE9BQU8sQ0FBQzthQUM1QixPQUFPLENBQUMsVUFBVSxFQUFFLFFBQVEsQ0FBQzthQUU3QixPQUFPLENBQUMsV0FBVyxFQUFFLFlBQVksQ0FBQzthQUNsQyxPQUFPLENBQUMsVUFBVSxFQUFFLFNBQVMsQ0FBQzthQUM5QixPQUFPLENBQUMsSUFBSSxFQUFFLEtBQUssQ0FBQzthQUVwQixPQUFPLENBQUMsK0NBQStDLEVBQUUsRUFBRSxDQUFDO2FBQzVELE9BQU8sQ0FBQyxrQkFBa0IsRUFBRSxrQkFBa0IsQ0FBQzthQUUvQyxPQUFPLENBQUMsNENBQTRDLEVBQUUsT0FBTyxDQUFDO2FBQzlELE9BQU8sQ0FBQyxpQ0FBaUMsRUFBRSxPQUFPLENBQUM7YUFDbkQsT0FBTyxDQUFDLG1CQUFtQixFQUFFLE9BQU8sQ0FBQzthQUNyQyxPQUFPLENBQUMsMkJBQTJCLEVBQUUsT0FBTyxDQUFDO2FBQzdDLE9BQU8sQ0FBQyxzQkFBc0IsRUFBRSxFQUFFLENBQUM7YUFDbkMsT0FBTyxDQUFDLGNBQWMsRUFBRSxDQUFDLENBQUMsRUFBRSxFQUFFLENBQUMsQ0FBQyxDQUFDLFdBQVcsRUFBRSxDQUFDO2FBQy9DLEtBQUssQ0FBQyxtQkFBbUIsQ0FBQyxDQUFDO1FBRTlCLFlBQVksQ0FBQyxPQUFPLENBQUMsR0FBRyxDQUFDLENBQUM7UUFDMUIsT0FBTyxLQUFLLENBQUMsSUFBSSxDQUFDLENBQUMsQ0FBQyxDQUFDO0lBQ3ZCLENBQUM7Q0FDRjtBQWlCRCxNQUFNLE9BQU8sYUFBYTtJQUN4QixNQUFNLENBQUMsTUFBTSxDQUFDLE1BQWdCLEVBQUUsQ0FBUyxFQUFFLEdBQWtCO1FBQzNELElBQUksTUFBTSxDQUFDLENBQUMsQ0FBQyxJQUFJLEtBQUssRUFBRTtZQUN0QixHQUFHLENBQUMsTUFBTSxDQUFDLENBQUMsTUFBTSxDQUFDLENBQUMsR0FBRyxDQUFDLENBQUMsQ0FBQyxDQUFDO1lBQzNCLE9BQU8sSUFBSSxDQUFDO1NBQ2I7YUFBTSxJQUFJLE1BQU0sQ0FBQyxDQUFDLENBQUMsSUFBSSxLQUFLLEVBQUU7WUFDN0IsR0FBRyxDQUFDLE1BQU0sQ0FBQyxDQUFDLE1BQU0sQ0FBQyxDQUFDLEdBQUcsQ0FBQyxDQUFDLENBQUMsQ0FBQztZQUMzQixPQUFPLElBQUksQ0FBQztTQUNiO1FBRUQsT0FBTyxLQUFLLENBQUM7SUFDZixDQUFDO0lBRUQsTUFBTSxDQUFDLEdBQUcsQ0FBQyxNQUFnQixFQUFFLENBQVMsRUFBRSxHQUFrQjtRQUN4RCxJQUFJLE1BQU0sQ0FBQyxDQUFDLENBQUMsSUFBSSxLQUFLLEVBQUU7WUFDdEIsSUFBSSxNQUFNLENBQUMsQ0FBQyxHQUFHLENBQUMsQ0FBQyxJQUFJLEtBQUssRUFBRTtnQkFDMUIsR0FBRyxDQUFDLE1BQU0sQ0FBQyxNQUFNLENBQUMsQ0FBQyxHQUFHLENBQUMsQ0FBQyxFQUFFLElBQUksQ0FBQyxDQUFDO2FBQ2pDO2lCQUFNO2dCQUNMLEdBQUcsQ0FBQyxNQUFNLENBQUMsTUFBTSxDQUFDLENBQUMsR0FBRyxDQUFDLENBQUMsQ0FBQyxDQUFDO2FBQzNCO1lBQ0QsT0FBTyxJQUFJLENBQUM7U0FDYjtRQUVELE9BQU8sS0FBSyxDQUFDO0lBQ2YsQ0FBQztJQUVELE1BQU0sQ0FBQyxTQUFTLENBQUMsTUFBZ0IsRUFBRSxDQUFTLEVBQUUsR0FBa0I7UUFDOUQsSUFBSSxhQUFhLENBQUMsTUFBTSxDQUFDLE1BQU0sRUFBRSxDQUFDLEVBQUUsR0FBRyxDQUFDLEVBQUU7WUFDeEMsT0FBTyxJQUFJLENBQUM7U0FDYjthQUFNLElBQUksYUFBYSxDQUFDLEdBQUcsQ0FBQyxNQUFNLEVBQUUsQ0FBQyxFQUFFLEdBQUcsQ0FBQyxFQUFFO1lBQzVDLGFBQWEsQ0FBQyxNQUFNLENBQUMsTUFBTSxFQUFFLENBQUMsR0FBRyxDQUFDLEVBQUUsR0FBRyxDQUFDLENBQUM7WUFFekMsT0FBTyxJQUFJLENBQUM7U0FDYjtRQUVELE9BQU8sS0FBSyxDQUFDO0lBQ2YsQ0FBQztDQUNGIn0=