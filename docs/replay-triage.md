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

## Open (10 battles)

### Unimplemented / unparsed keywords
- **Brawl** (per opposing card of the same clan as the opposing card):
  874590 r0 Karkass Cr "Brawl: Damage + 1" vs 4 Rescue → damage 8 (engine 4);
  876752 r1 Macey Rook "Brawl: - 1 Opp. Life Min 0" → post-round -3 (engine 1 life left, server 0).
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

### Order between bonus and ability on the same side
- 875272 r2 Wesley (6×1 + 12 Support = 18) vs Don Cr (bonus -12 Opp Attack Min 8, ability
  -4 Opp Attack Min 2): server 18 → 8 (bonus, clamped) → 4 (ability). Engine applied the
  ability first: 18 → 14 → 2 → Min 8 = 8. Hypothesis: within a phase the clan bonus
  resolves before the ability. One data point.

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
