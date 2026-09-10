# Replay triage — engine vs server mismatches

Status from `deno test -A --no-check tests/replay/` against 62 captured battles
(60 replayable, 2 in progress/Dojo ignored). Each entry is the first mismatching round of
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
(`BasicModifier.canApply`). Fixed 876712, 877023.

### Bonus before ability, reductions by descending Min
`Ability.card()` compiles the clan bonus before the ability, and `Events.execute()` sorts the
opponent-reduction phases (PRE1 power/damage, POST2 attack) by descending Min clamp:
Miss Stella (ability -8 Min 11, bonus -8 Min 3) on 18 → 11 → 3; Don Cr (bonus -12 Min 8,
ability -4 Min 2) on 18 → 8 → 4; Donna Black (bonus -12 Min 8, ability -10 Min 3) → 3.
Fixed 875272, 876752, 901613, 901292. Three data points; keep an eye on it.

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

## Open (5 battles)

### Inactive clan bonus ("None") and Damage Exchange — Free Fight / mixed decks
- 901004 r0 (Free Fight): Kubrat Cr's bonus is sent as "None" (only card of its clan in
  hand → no clan bonus). Check `Hand` applies the ≥2-same-clan rule and that a "None" bonus
  parses to nothing. Same round: Waldegrin Cr "Damage Exchange" lost with 8 damage vs 1;
  server dealt 1 (engine 6) — Exchange semantics / activation on loss to verify.

### Unimplemented / unparsed keywords
- **Cards Damage +2**: 874795 r0 El Resbaladizo damage 8 vs 6 (+4 → "+2 per something"; TBD).

### Revenge / Damage Impose
- 874712 r1 Tina "Revenge: Power And Damage +2" (lost previous round) vs Kochar "Damage
  Impose" + "Copy: Opp. Ability": engine damage 2, server 4.

### Recover N Pillz Out Of M
- 877983 r1 Eebiza "Defeat: Recover 1 Pillz Out Of 2" with 0 pillz bet: server +1, engine +0.
  But 867173 r1 AI-Lycs "Recover 2 Out Of 3" with 3 bet: server +2 (= ceil(3×2/3), not
  ceil(4×2/3) = 3). So the free pill is *not* counted, yet a 0-pill bet still recovers 1.
  Hypothesis: max(1, ceil(bet × N / M)). Two data points; needs more.

### Not reproducible from a testcase
- 874590 is a **Hazard** game and cannot be replayed as it stands. Administrator (Leader,
  ability 4144) "replaces the abilities of the three cards present in the draw with random
  abilities that are already being used in the game", so ReV_Next_'s Diabolus, Karkass Cr and
  Nero Cr each fought with an ability their card does not own - Karkass Cr with "Brawl:
  Damage + 1" instead of its real Sinister Symmetry, which is why this looked like a missing
  Brawl. A testcase carrying only card names and levels cannot express that. Either teach
  `ExtractBattle.ts` to record the server's per-card ability in the testcase and have the
  replay use it, or skip games whose draw contains a Hazard leader.

## Legacy tests
- `tests/Game_2.test.ts` "Protection" uses empty card names (never passed).
- `tests/ability/Oculus.test.ts` "Infiltrated" expects the opponent to lose 4 life; the
  engine's per-phase event ordering (validated by replays) no longer produces that. The
  expectation is probably stale; verify with a captured Oculus game before changing it.

## Data notes
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
