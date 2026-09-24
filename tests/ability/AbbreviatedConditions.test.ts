// Abbreviated condition names. Normalising the ability text drops every ".", so "Asymm.:",
// "Asy. :" and "Repris.:" used to reach Condition as "Asymm", "Asy" and "Repris" - names it
// does not know, and an unknown condition is met unconditionally. Each abbreviation now
// expands to the condition its abilityData names (captures/abilities.json 4999, 5072 and
// 5073: indexRequirement "asymmetry"; 5275: positionRequirement "defender"). "Asym.",
// "Rev." and "Brwl." are data.json-only, and each can only abbreviate one condition.
import Ability from "@/game/Ability.ts";
import { ConditionType } from "@/game/Condition.ts";
import { assertEquals } from "@std/assert";

const conditionsOf = (s: string) =>
  new Ability(s).conditions.map((c) => ConditionType[c.type]);

Deno.test("abbreviated condition prefixes expand to their full conditions", () => {
  const cases: [string, string[], string][] = [
    ["[clan:31][clan:46][clan:54][clan:49] Asymm.: Stop Opp. Ability", ["INFILTRATED", "ASYMMETRY"], "Stop Ability"],
    ["[clan:46][clan:58][clan:40][clan:55][clan:42][clan:50] Asy. : -3 Opp Dam., Min 1", ["INFILTRATED", "ASYMMETRY"], "-3 Damage Min 1"],
    ["[clan:46][clan:58][clan:40][clan:55][clan:42][clan:50] Asy. : Copy: Opp. Ability", ["INFILTRATED", "ASYMMETRY"], "Copy Ability"],
    ["[clan:53][clan:52][clan:37][clan:57][clan:44] Repris.: Consume 1, Min 4", ["INFILTRATED", "REPRISAL"], "1 Consume Min 4"],
    ["[clan:53][clan:52][clan:37][clan:57][clan:44] Repris. : Mindwipe 1, Min 0", ["INFILTRATED", "REPRISAL"], "1 Mindwipe Min 0"],
    ["[clan:46][clan:53][clan:47][clan:37] Asym.: -3 Opp Power, Min 2", ["INFILTRATED", "ASYMMETRY"], "-3 Power Min 2"],
    ["[clan:25][clan:48][clan:4][clan:60][clan:45] Rev.: -5 Opp. Pow., Min 1", ["INFILTRATED", "REVENGE"], "-5 Power Min 1"],
    ["[clan:38][clan:25][clan:48][clan:27][clan:41][clan:33] Brwl. : -2 Opp Dam., Min 1", ["INFILTRATED", "BRAWL"], "-2 Damage Min 1"],
  ];
  for (const [text, conditions, body] of cases) {
    const ability = new Ability(text);
    assertEquals(conditionsOf(text), conditions, text);
    assertEquals(ability.ability, body, text);
  }
});

Deno.test("abbreviations that already worked keep working", () => {
  assertEquals(conditionsOf("Night: Confid.: -2 Opp Pow. & Damage, Min 3"), ["NIGHT", "CONFIDENCE"]);
  assertEquals(conditionsOf("Conf.: Vict. Or Def.: -2 Opp. Life, Min 0"), ["CONFIDENCE", "VICTORY OR DEFEAT"]);
  // "Prot." is the effect, not a condition: Courage stays the only condition.
  assertEquals(conditionsOf("Courage: Prot.: Power & Damage"), ["COURAGE"]);
  assertEquals(new Ability("Courage: Prot.: Power & Damage").ability, "Protection Power&Damage");
  for (const [text, type] of [
    ["Asymmetry: Damage +3", "ASYMMETRY"],
    ["Reprisal: Damage +4", "REPRISAL"],
    ["Revenge: Damage +4", "REVENGE"],
    ["Brawl: Power And Damage + 1", "BRAWL"],
  ]) {
    assertEquals(conditionsOf(text), [type], text);
  }
});
