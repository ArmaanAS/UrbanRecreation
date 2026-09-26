import { assertEquals } from "@std/assert";
import { blocker, compactCoverage, type CoverageFile, describeRefusal, whyNot } from "@/decks/Coverage.ts";

const uncapturedText =
  'P1 slot 0 Ability catalog source Some(47) "-1 Opp Damage, Min 3" lookup failed: effect description "-1 Opp Damage, Min 3" is missing';
const uncapturedId = 'P2 slot 3 Ability catalog source Some(901) "Stop Opp. Ability" lookup failed: effect id 901 is missing';
const notExecutable = 'P1 slot 1 Bonus catalog source Some(12) "Copy: Opp. Bonus" is not executable by this projection';

Deno.test("a refusal is read back into its source, its text and what it waits for", () => {
  assertEquals(blocker(uncapturedText), { source: "Ability", text: "-1 Opp Damage, Min 3", kind: "uncaptured_text" });
  assertEquals(blocker(uncapturedId).kind, "uncaptured_id");
  assertEquals(blocker(notExecutable), { source: "Bonus", text: "Copy: Opp. Bonus", kind: "not_executable" });
  assertEquals(blocker("P1 slot 0 contains unsupported Leader").kind, "other");

  assertEquals(describeRefusal(uncapturedText), 'ability "-1 Opp Damage, Min 3" has not been seen in a captured battle yet');
  assertEquals(describeRefusal(notExecutable), 'clan bonus "Copy: Opp. Bonus" is not modelled by the exact engine yet');
  assertEquals(
    describeRefusal("P2 slot 1 contains unsupported Leader Administrator (id 2095 level 2)"),
    "Leaders are not modelled by the exact engine",
  );
  assertEquals(describeRefusal("P1 slot 3 something new"), "something new", "the seat and slot are dropped");
  assertEquals(whyNot({ status: "exact" }), undefined);
  assertEquals(whyNot({ status: "leader" }), "Leaders are not modelled by the exact engine");
});

Deno.test("the compact form keeps one letter per status and each reason once", () => {
  const file = {
    generatedAt: "2026-09-26T00:00:00Z",
    provenance: {},
    cards: [
      { id: 1, levels: { "1": { day: { status: "exact" }, night: { status: "exact" } } } },
      {
        id: 2,
        levels: {
          "1": { day: { status: "refused", reason: uncapturedText }, night: { status: "exact" } },
          "2": { day: { status: "refused", reason: uncapturedText }, night: { status: "bonus_refused", reason: notExecutable } },
        },
      },
      { id: 3, levels: { "5": { day: { status: "leader" } } } },
    ],
  } as unknown as CoverageFile;
  const compact = compactCoverage(file);
  assertEquals(compact.cards["1"]["1"], ["e", "e", -1, -1]);
  assertEquals(compact.cards["2"]["1"], ["r", "e", 0, -1]);
  assertEquals(compact.cards["2"]["2"], ["r", "b", 0, 1]);
  assertEquals(compact.cards["3"]["5"], ["l", null, 2, -1]);
  assertEquals(compact.reasons.length, 3, "a reason shared by two levels is stored once");
});
