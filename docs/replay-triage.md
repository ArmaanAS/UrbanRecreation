# Replay triage — engine vs server mismatches

Status from `deno test -A --no-check tests/replay/` against 56 captured battles
(54 replayable, 2 in progress/Dojo ignored). Each entry is the first mismatching round of
one battle; engine value first, server value second. Battle ids refer to
`captures/games/<id>.json`, which has the full context.

| Date | Exact | Mismatch | Change |
| --- | --- | --- | --- |
| 2026-09-10 (start) | 30 | 23 | baseline after level-aware card data |
| 2026-09-10 | 31 | 23 | Tolvack clan added (one more game replayable) |
| 2026-09-10 | 44 | 10 | modifier ordering, Day/Night, post-KO gains (below) |
| 2026-09-11 | 50 | 10 | +6 games; bonus before ability; same-phase reductions by descending Min |

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

## Open (10 battles)

### Inactive clan bonus ("None") and Damage Exchange — Free Fight / mixed decks
- 901004 r0 (Free Fight): Kubrat Cr's bonus is sent as "None" (only card of its clan in
  hand → no clan bonus). Check `Hand` applies the ≥2-same-clan rule and that a "None" bonus
  parses to nothing. Same round: Waldegrin Cr "Damage Exchange" lost with 8 damage vs 1;
  server dealt 1 (engine 6) — Exchange semantics / activation on loss to verify.

### Poison-family keywords
- 901092 r1: Dame Karkass Cr "Victory Or Defeat: Toxin 1, Min 0" and Merweiss Cr
  "Revenge: Mindwipe 2, Min 0" — permanent post-round life decreases (server -1 / -2);
  engine left the player at 1 life instead of 0.

### Unimplemented / unparsed keywords
- **Brawl** (per opposing card of the same clan as the opposing card):
  874590 r0 Karkass Cr "Brawl: Damage + 1" vs 4 Rescue → damage 8 (engine 4).
- **Repair N, Max M**: permanent +N life post-round (`isPermanent: true`): 875230 r3 Wilo Ld.
- **Cards Damage +2**: 874795 r0 El Resbaladizo damage 8 vs 6 (+4 → "+2 per something"; TBD).
- **After [clan:…]**: Tolvack bonus "After [clan:56][clan:60] : Power +3" activates once a
  listed clan's card was played earlier in the game. Engine currently parses it as an
  unconditional +3: 876464 r0 Tør power 9 vs 6 (r1/r2 it *is* active).
- **Sinister Symmetry**: 874399 r3 Karkass Cr → post-round -5 life to opponent (KO).
- **Unison** (all four cards same clan): 877733 r1 Korakine "Unison : +2 Pillz And Life"
  lost the round; engine still gave +1 life (server +0). Check both the Unison condition
  and "Pillz And Life" parsing.

### Revenge / Damage Impose
- 874712 r1 Tina "Revenge: Power And Damage +2" (lost previous round) vs Kochar "Damage
  Impose" + "Copy: Opp. Ability": engine damage 2, server 4.

### Recover N Pillz Out Of M
- 877983 r1 Eebiza "Defeat: Recover 1 Pillz Out Of 2" with 0 pillz bet: server +1, engine +0.
  But 867173 r1 AI-Lycs "Recover 2 Out Of 3" with 3 bet: server +2 (= ceil(3×2/3), not
  ceil(4×2/3) = 3). So the free pill is *not* counted, yet a 0-pill bet still recovers 1.
  Hypothesis: max(1, ceil(bet × N / M)). Two data points; needs more.

## Legacy tests
- `tests/Game_2.test.ts` "Protection" uses empty card names (never passed).
- `tests/ability/Oculus.test.ts` "Infiltrated" expects the opponent to lose 4 life; the
  engine's per-phase event ordering (validated by replays) no longer produces that. The
  expectation is probably stale; verify with a captured Oculus game before changing it.

## Data notes
- The card DB dump does not include the structured `abilityData`; only battle snapshots do.
  `captures/abilities.json` accumulates it for every ability seen in a battle.
