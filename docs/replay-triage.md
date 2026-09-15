# Replay triage — engine vs server mismatches

Status from `deno test -A --no-check tests/replay/` against 327 captured battles
(321 replayable, 6 incomplete/Dojo ignored): 268 replay exactly and 53 mismatch. Each entry
is the first mismatching round of
one battle; engine value first, server value second. Battle ids refer to
`captures/games/<id>.json`, which has the full context.

Before coding against an entry here, check it against `captures/games/<id>.json`: three of
the entries below turned out to be misattributed, and two of those pointed at abilities that
were already implemented. The per-card `abilityData` the server sends (collected in
`captures/abilities.json`) is the authority on what an ability actually does - `isSupport`,
`isAntiSupport`, `isPermanent`, `currentRoundRequirement`, `indexRequirement` and
`specialAction` between them describe most keywords exactly.

| Date | Exact | Mismatch | Change |
| --- | --- | --- | --- |
| 2026-09-10 (start) | 30 | 23 | baseline after level-aware card data |
| 2026-09-10 | 31 | 23 | Tolvack clan added (one more game replayable) |
| 2026-09-10 | 44 | 10 | modifier ordering, Day/Night, post-KO gains (below) |
| 2026-09-11 | 50 | 10 | +6 games; bonus before ability; same-phase reductions by descending Min |
| 2026-09-11 | 53 | 7 | permanents latch on their own trigger; Repair; Sinister Symmetry |
| 2026-09-11 | 55 | 5 | After [clan:...] is a previous-round look-back; stopped permanents never start |
| 2026-09-11 | 56 | 4 | Cards <stat> +N applies to both sides |
| 2026-09-11 | 57 | 3 | Recover never gives nothing |
| 2026-09-12 | 82 | 17 | +39 fresh captures; 14 new mismatches awaiting triage |
| 2026-09-13 | 85 | 14 | Tune Out ignores every Attack modifier |
| 2026-09-13 | 86 | 14 | +1 capture; Per Life/Pillz Lost fixed in live battle 1060510 |
| 2026-09-13 | 92 | 11 | +3 captures; Stop effects resolve by dependency; refreshed after pending fixes |
| 2026-09-14 | 92 | 12 | +1 capture; live advisor now resynchronises resources after engine drift |
| 2026-09-14 | 267 | 53 | +220 extracted captures; fresh replay baseline, new mismatches awaiting triage |
| 2026-09-15 | 268 | 53 | +1 capture; Reanimate applies after every defeat and can prevent KO |

## Fixed

### Modifier ordering (was "Support count" + "Montana clamp" + "caps vs mins")
The server applies a card's own Power / Damage / Attack modifiers (Support, per-X, Max caps,
clan bonus) before the opponent's reductions and their Min clamps. The engine ran them in
whatever order the events were queued, so e.g. Aurora 7×2 → Montana -12 Min 8 → 8, then
+12 Support = 20 instead of 7×2 + 12 = 26 → -12 = 14. Fix: opponent-targeting
Power/Damage modifiers run in PRE1 (own ones in PRE2), opponent Attack modifiers in POST2
(own in POST1). `BasicModifier.updateEventTime()`. Fixed 874520, 875032, 867173, 874962,
877950, 874642, 875098 and others.

### Day / Night
Clint City alternates every 4 hours; night is 06–10, 14–18, 22–02 Paris time (Urban Rivals
wiki/forum; confirmed by every captured GhosTown card, whose ability text the server sends
in its *active* Day:/Night: variant). `ExtractBattle.isNight(creationTime)` stamps each game
and testcase; `Game(..., night)` sets `PlayerRound.day` and switches cards to their
`night_ability` / `night_bonus` (from the site card DB via `deno task cards`).
Fixed 877575, 878120.

### Post-KO gains
A player at 0 life gains neither life nor pillz from Defeat / clan bonuses that round
(`BasicModifier.canApply`). Reanimate is the explicit exception: its rules say that it can
prevent a KO. Fixed 876712, 877023.

### Reanimate is an immediate Defeat life gain
Reanimate activates whenever its card loses, not only when the incoming damage would be
lethal. In 1130654 round 1, Lobo's owner started on 7 Life and Miyo dealt 5; the server left
them on 4, exactly `7 - 5 + 2`. The engine required Life to have reached zero, left them on
2, and consequently advertised Miyo with one pill as a 100% win. With the rule corrected,
the same fully searched recommendation is 86%. Reanimate is also allowed to lift its owner
from zero and prevent KO, while a stopped Reanimate does nothing; 1080877 supplies the
captured Stop Opp. Ability case. Tests in `tests/ability/Reanimate.test.ts` and
`tests/solver/Policy.test.ts` pin all three behaviours plus the displayed percentage.

### Bonus before ability, reductions by descending Min
`Ability.card()` compiles the clan bonus before the ability, and `Events.execute()` sorts the
opponent-reduction phases (PRE1 power/damage, POST2 attack) by descending Min clamp:
Miss Stella (ability -8 Min 11, bonus -8 Min 3) on 18 → 11 → 3; Don Cr (bonus -12 Min 8,
ability -4 Min 2) on 18 → 8 → 4; Donna Black (bonus -12 Min 8, ability -10 Min 3) → 3.
Fixed 875272, 876752, 901613, 901292. Three data points; keep an eye on it.

### Stop effects resolve by dependency
The PRE4 cancellation phase cannot use fixed P1/P2 order or first-mover order. It must first
run a Stop effect whose own ability/bonus cannot be stopped by another unresolved effect,
then discard newly blocked effects and continue. Battle 867116 pins one direction: Miyo's
ability stops Bonnie Ld's Piranas Stop Bonus, so Miyo's Hive bonus remains active even though
Bonnie moved first. Battle 1090338 pins the other: Burdock's Roots bonus stops Spidee's
Reprisal Stop Ability, leaving Burdock's `After Roots: Stop Opp. Bonus` active. The old P1
order gave Spidee an impossible +12 Rescue Attack and made its five-pill move display as a
100% win; it is actually 89% against Burdock's hidden bets with a losing worst case.
`Events.executeCancels()` now resolves the dependency graph, with the historical order only
as a deterministic fallback for a true mutual-stop cycle. Tests cover both captures plus the
advisor percentage.

### Permanents latch on their own trigger, not always on a win
`Ability.canApply` gated every permanent (the Poison / Heal / Toxin / Combust / Mindwipe /
Repair family, which compile to `GLOBAL_ABILITY` / `GLOBAL_BONUS` and repeat at each round
end) behind `data.player.won`, and deleted the effect outright when the card lost. Anything
triggered by losing therefore never started: Frogo's "Defeat : Heal 1 Max. 13" in 875230,
Dame Karkass Cr's "Victory Or Defeat: Toxin 1, Min 0" and Merweiss Cr's "Revenge: Mindwipe
2, Min 0" in 901092. The trigger is now `Ability.latches()`: every condition must be met,
and the card must have won *unless* a condition already pinned the outcome (Defeat, Victory
Or Defeat and Reanimate all clear `win` on the modifier when they compile). Once latched the
effect repeats unconditionally, so the conditions are no longer re-checked each round - a
"Defeat: Poison" would otherwise switch itself off in any round its owner won.
Fixed 875230, 901092. Tests in `tests/ability/Heal.test.ts`.

### Repair N, Max M
Unimplemented; now the own-side positive mirror of Mindwipe. On a win the owner gains N Life
*and* N Pillz at the end of that round and of every following one, each capped at M
(`abilityData` 3796: `life&pillz`, `increase`, `isPermanent` + `isImmediatePermanent`,
`currentRoundRequirement` "win" - so unlike Poison it is not delayed past its own round).
Tests in `tests/ability/Repair.test.ts`. No captured battle exercises it yet: Wilo Ld lost
the only round it was played in.

### Sinister Symmetry
Unimplemented; now a Life modifier on the opponent of -Infinity Min 0 (i.e. all of it) under
a Symmetry condition. `abilityData` 4303 pins the semantics: `currentRoundRequirement` win,
`indexRequirement` "symmetry", `specialAction` "ko". 874399 r3 is the confirmation - Karkass
Cr at index 3 beats Tina at index 3, deals 4 to an opponent on 9 Life, and the server then
reports a post-round life decrease of exactly the remaining 5, with `byKo: true`.
Fixed 874399. Tests in `tests/ability/SinisterSymmetry.test.ts`.

### After [clan:...] is a previous-round look-back
Not "once a listed clan's card was played earlier in the game": `abilityData` calls it
`previousClanRequirement`, and the Tolvack bonus (5585) reads "only activates if the player
of Tolvack played an Oculus or Tolvack character in the **previous round**". `Condition`
also mistyped it, because "After [Clan:56][Clan:60]" ends in "]" and so fell through to
`INFILTRATED`, which asks whether the *current* card is of a listed clan - always true for a
Tolvack card, hence the unconditional +3. There is now a `ConditionType.AFTER`, fed by
`PlayerRound.lastClan`, which `CardBattle` records for each side once a round resolves.
876464 confirms it round by round: Tor lv4 fights round 0 on its base power of 6, Drava lv3
(base 7) has 10 in round 1, Maelt Riv lv3 (base 8) has 11 in round 2.
Fixed 876464. Tests in `tests/ability/After.test.ts`.

### A permanent stopped in its own round never starts
`Ability.latches()` now refuses to latch when the card's ability (or bonus) is blocked. The
check cannot be left to each application: from the next round on the ability object outlives
its card, and `data.card` is whichever card its owner plays then, so the blocked test would
be asking about the wrong card - which is why a latched permanent no longer runs it at all.
877733 r0 is the case: Pr Balthazar's "Stop Opp. Ability" meets Lianah Ld's "Heal 1 Max.
20", and the server never heals across the three rounds that follow.
Fixed 877733. Tests in `tests/ability/Heal.test.ts`.

### Cards <stat> +N
"Cards Damage +2" is "The Damage points of both characters are increased by 2 points"
(`abilityData` 3295, sideAffected "both"). Two things were wrong: `normalise` turned it into
"Cards +2 Damage", because the rule that moves the sign in front takes one or three words
and not two, so `tokens[0]` was "Cards" and no branch matched at all; and the positive
branch only treated "Players" as a both-sides marker, though the negative one already read
"Cards" that way for "-2 Cards Damage, Min 1" (4957). 874795 r0 confirms both halves in one
round: El Resbaladizo lv4 (base 6) fights at 8, Aurora lv5 (base 5) at 7.
Fixed 874795. Tests in `tests/ability/CardsDamage.test.ts`.

### Recover N Pillz Out Of M
A triggered Recover gives back `max(1, ceil(bet x N / M))`, where the bet excludes the free
pill. Rounding up is confirmed by the repo owner; the two captured rounds fix the rest:
877983 r1 Eebiza "Defeat: Recover 1 Pillz Out Of 2" loses on a bet of 0 and still gets 1,
where the proportion alone gives 0, and 901400 r1 D-aleq "Defeat: Recover 2 Pillz Out Of 3"
loses on a bet of 3 and gets 2, where counting the free pill would give ceil(4 x 2/3) = 3.
The engine already rounded up on the right quantity; only the minimum was missing.
Fixed 877983. Tests in `tests/ability/Recover.test.ts`.

Still worth a data point when one turns up: no captured round has a bet large enough to
separate the proportion from a flat N, since bet 3 with N=2 M=3 gives 2 either way. A loss
on 5 pillz with "Recover 2 Out Of 3" would settle it - the proportion says 4, a flat N says 2.
The `both` branch of `RecoverModifier` ("Recover Players Pillz") and the Life branch have no
captured data at all, and the Life branch has no minimum applied.

### Tune Out ignores every Attack modifier
The server's long description says the Attack calculation is ignored and the player who
bet the most Pillz wins; equal bets use the normal card tie-break. The engine already set
both cards' Power to 1, but then still applied Attack additions and reductions. In 964404
round 2 that let Hive's `-3 Opp Attack, Min 5` turn Pepe Andrei's pillz-only 7 Attack into
5, so Nebula's 6 Attack was incorrectly evaluated as a guaranteed immediate KO. Tune Out
now cancels Attack modifiers on both cards before the pillz-only totals are calculated.
Fixed 924146, 925868, 926071 and 964404. Tests in `tests/ability/TuneOut.test.ts`.

### Per Life/Pillz Lost
The parser treated `Per Pillz Lost` as ordinary `Per Pillz`, silently ignoring `Lost`, so
Nolegs received a multiplier of its zero remaining pillz rather than the twelve spent since
the match began. In 1060510 round 3 it therefore had 6 Attack in the engine instead of the
server's exact `6 x 1 + 2 x 12 = 30`; Wesley's 18 was incorrectly marked as a guaranteed
win and the advisor recommended the losing play. `Player` now retains its match-start Life
and Pillz in its existing packed integer, and `BasicModifier` has distinct lost-resource
multipliers. The same parser fix covers the corresponding Life Lost and opponent-targeting
cards, including Max/Min clauses after `Lost`. Fixed 1060510. Test in
`tests/ability/LostResources.test.ts` plus the captured replay.

## Previously triaged open rules

### End-of-round gain/reduction order — 1093173
Round 1 (zero-based) has Goose's `-2 Opp. Pillz And Life, Min 5` beat Dr Web Ld,
whose Riots bonus is `Victory Or Defeat: +1 Pillz`. DashSmashing starts the round on 7,
bets 2, and the server finishes on 5. That arithmetic requires the Riots gain to apply
before Goose's reduction: `7 - 2 + 1 - 2`, clamped to 5. The engine executes internal P1's
END events first, so it clamps Goose's reduction at 5 and then adds Riots: `7 - 2 - 0 + 1
= 6`. This is the only captured round found with an opposing Pillz reduction and a
simultaneous own Pillz gain, so retain it as an ordering hypothesis until a second data
point confirms the general rule.

The live advisor now reports the disagreement and then replaces replayed life/pillz with
the server's completed-round totals before solving the next decision. Therefore round 4
correctly starts with 3 pillz and cannot offer the impossible fourth pill, while the replay
continues to fail and keeps the engine bug visible.

### Damage Exchange — 901004
The inactive-bonus half of this entry is a false lead: across the captures there are 23
played rounds where the server sends no clan bonus, and in every one the card is the only
member of its clan in the hand. Most of those games already replay exactly, so the
≥2-same-clan rule is already right and nothing needs doing there.

What is left is Damage Exchange, and 901004 r0 pins its semantics completely: Waldegrin Cr
lv5 has a written Damage of 1 and Kubrat Cr lv5 has 8, and the server reports Waldegrin
fighting at 8 and Kubrat at 1 - the two *base* values swapped, exactly as `abilityData` 1588
says ("The starting Damage (written on the card) of your card is exchanged with that of your
opponent's", sideAffected "both", attributeAction "copy"). Kubrat then deals 1.
`ExchangeModifier` already does precisely that swap, so the bug is elsewhere in the round:
the engine has Kubrat dealing 6, which is neither base (8) nor the swapped value (1). Find
where the 2 goes missing before touching the modifier. Only 3 captured rounds play a Damage
Exchange card at all, one of them on a loss.

### Revenge / Damage Impose
- 874712 r1 Tina "Revenge: Power And Damage +2" (lost previous round) vs Kochar "Damage
  Impose" + "Copy: Opp. Ability": engine damage 2, server 4.

### Not reproducible from a testcase
- 874590 is a **Hazard** game and cannot be replayed as it stands. Administrator (Leader,
  ability 4144) "replaces the abilities of the three cards present in the draw with random
  abilities that are already being used in the game", so ReV_Next_'s Diabolus, Karkass Cr and
  Nero Cr each fought with an ability their card does not own - Karkass Cr with "Brawl:
  Damage + 1" instead of its real Sinister Symmetry, which is why this looked like a missing
  Brawl. A testcase carrying only card names and levels cannot express that. Either teach
  `ExtractBattle.ts` to record the server's per-card ability in the testcase and have the
  replay use it, or skip games whose draw contains a Hazard leader.

## Fresh capture backlog

The expanded 326-game corpus exposed 49 additional mismatches that have not yet been
grouped or attributed to rules. They are recorded as regression targets only; inspect the
first failing round and group them by ability keyword before changing the engine:

924320, 924573, 924615, 924669, 924740, 924853, 924890, 925818, 942983, 943111,
943231, 946810, 947010, 947670, 948108, 948390, 949439, 956902, 1023946,
1024592, 1024732, 1024821, 1025413, 1058366, 1059149, 1060341, 1065308,
1066210, 1069506, 1078555, 1078669, 1078820, 1078906, 1079078, 1079482,
1081234, 1088641, 1089830, 1089933, 1089974, 1090269, 1091235, 1091381,
1092066, 1092369, 1092909, 1093129, 1093451, 1093569.

## Legacy tests
- `tests/Game_2.test.ts` "Protection" uses empty card names (never passed).
- `tests/ability/Oculus.test.ts` "Infiltrated" expects the opponent to lose 4 life; the
  engine's per-phase event ordering (validated by replays) no longer produces that. The
  expectation is probably stale; verify with a captured Oculus game before changing it.

## Data notes
- Battle 1131463 exposed a fifth source of card-definition differences: EFC had rebalanced
  the level-3 semi-evo Quetzal Cr to 7/4 with Stop Opp. Bonus, while the 2026-09-10
  character dump still said 2/6 with no ability. The exact server definition is held in
  `data/battle_card_overrides.json` until the next dump catches up. The advisor also refuses
  to solve any future card for which the battle supplies an ability while the loaded level
  says No Ability; changed base stats are not included in `battles.status`, so guessing
  would make a displayed 100% unsafe.
- The card DB dump does not include the structured `abilityData`; only battle snapshots do.
  `captures/abilities.json` accumulates it for every ability seen in a battle.
- A card's ability in a battle is **not** always the one the card DB lists for it. Comparing
  every captured hand against `data/data.json` gives 66 differences, and they are all one of
  four things: a Copy card, where the server reports the ability it resolved to rather than
  "Copy: Opp. Ability"; a bonus sent as "None" because the hand holds fewer than two cards of
  that clan; a Day/Night card, where the DB row is the day variant; or a Hazard game (874590),
  where the abilities are random. None of these are card-data bugs, but all of them will look
  like engine bugs in a replay.
- Brawl, Support, Growth, Degrowth, Equalizer, Symmetry and Asymmetry are per-X multipliers
  (`Per` in `BasicModifier.ts`, wired up by `Condition.compile`), not conditions that gate an
  effect. A replay mismatch on a card carrying one of these is much more likely to be the
  card data or another ability in the round than the multiplier itself.
