# Replay triage — engine vs server mismatches

Status from `deno test -A --no-check tests/replay/` against 383 captured battles
(376 replay-ready; 7 ignored, 6 because they stopped mid-match and 1414087 because it deals
card 2714, which the 2026-09-10 card data predates): 347 replay exactly and 29 mismatch. Each entry
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
| 2026-09-15 | 269 | 53 | +1 capture; refreshed stable replay baseline |
| 2026-09-16 | 277 | 45 | Riots Victory-or-Defeat Pillz now applies after its owner's KO |
| 2026-09-16 | 278 | 44 | Pr Hide's exact printed Victory-or-Defeat Pillz ability now also applies after KO |
| 2026-09-17 | 304 | 48 | +31 captures extracted (29 committed but never extracted, plus 2 new); 4 fresh mismatches awaiting triage |
| 2026-09-17 | 310 | 42 | Bet > N Pillz gates its effect; Fury settles with the Damage dealt; a gift of Opp. Pillz needs no pool |
| 2026-09-19 | 312 | 42 | Dojo battles extracted and replayed like any other room (+1 capture) |
| 2026-09-20 | 313 | 42 | +1 Dojo Life-room capture, replays exactly |
| 2026-09-20 | 316 | 42 | +3 Dojo Killshot and Backlash captures, all replay exactly |
| 2026-09-24 | 331 | 45 | +19 live captures: 15 exact, 3 fresh mismatches, 1414087 unreplayable until a card refresh |
| 2026-09-24 | 333 | 43 | Growth permanents keep their latch-round amount; a reduction naming no opponent hits its own card |
| 2026-09-24 | 338 | 38 | Hazard games replay with the abilities the server dealt |
| 2026-09-24 | 345 | 31 | Protection: Power And Damage refuses an opposing reduction |
| 2026-09-24 | 347 | 29 | Corrupt implemented; abbreviated condition prefixes parse |

## Fixed

### 924615's last round is a stale capture, not an engine bug
The extractor could not attribute the closing `battles.result` to a side (`mySide` is null),
so the final round's life and pillz stayed at the pre-damage snapshot the server sends before
applying the last hit: Vektor wins round 3 for 2 Damage and the record still says the loser
is on 5. Nothing is wrong with the engine here and nothing can be fixed by changing it - the
round's ground truth was never captured. The Rust combat-stat gate carries 924615 for three
rounds for the same reason. Two other entries carry the same issue string (948108, 948390),
and since the Protection fix below so do 942983 and 943111: every card result in their last
round now matches, and only the final life/pillz, which the capture flags as stale, differ.


### Dojo (battle rule 6) is not a different rule set
`ExtractBattle.ts` used to refuse a testcase for every battle in the Dojo room, on the
stated grounds that "rules differ from PvP". No capture supports that. The two older Dojo
captures (`830285`, `869944`) send a null bonus on all eight cards, but so does every
lone-clan card in an ordinary battle, and that tutorial deck is eight singleton clans - the
ordinary "a bonus needs a clan-mate" rule accounts for it. `1294430`, a later Dojo battle in
the same room, deals two Jungo cards and the server duly sends the active Jungo bonus on
both. The arithmetic agrees either way: both completed Dojo captures now replay exactly on
the first attempt with no engine change at all.

The room is worth capturing deliberately, because each one drills a fixed set of abilities -
`1294430` is eight Life-gain cards - so playing a room is a cheap way to generate ground
truth for one ability family. The capture pipeline was always recording these battles; only
the extractor was discarding them.


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
Ordinary Defeat Life/Pillz gains still do not apply after their owner reaches 0 life:
Kubra receives neither half of `Defeat: +1 Pillz And Life` in lethal captures 876712 and
877023. Reanimate remains an explicit Life exception because it can prevent a KO.

Riots' `Victory Or Defeat : +1 Pillz` bonus is a second, narrower exception. Eight
independent captures show the bonus returning one Pillz after its owner is KO'd: 1058366,
1078669, 1078906, 1079482, 1081234, 1089933, 1092369, and 1093451. The modifier now carries
a post-KO-Pillz flag for the exact effective Riots clan bonus. Capture 1092909 round 3 also
directly shows Pr Hide's printed same-text ability returning a second Pillz after its KO;
that replay now matches. Because the TypeScript card model does not retain the structured
ability definition id at runtime, the ability exception is locked to Pr Hide id 1568 at
level 3, the exact printed text, an Ability source, and a compiled own `+1 Pillz` modifier.
Copied text, Atess's unobserved same-text variants, Life, and ordinary Defeat gains remain
excluded. Focused lethal-round and synthetic negative tests pin those boundaries.

### Reanimate is an immediate Defeat life gain
Reanimate activates whenever its card loses, not only when the incoming damage would be
lethal. In 1130654 round 1, Lobo's owner started on 7 Life and Miyo dealt 5; the server left
them on 4, exactly `7 - 5 + 2`. The engine required Life to have reached zero, left them on
2, and consequently advertised Miyo with one pill as a 100% win. With the rule corrected,
the same fully searched recommendation is 86%. Reanimate is also allowed to lift its owner
from zero and prevent KO, while a stopped Reanimate does nothing; 1080877 supplies the
captured Stop Opp. Ability case. Tests in `tests/ability/Reanimate.test.ts` and
`tests/solver/Policy.test.ts` pin all three behaviours plus the displayed percentage.

### Bet > N Pillz is a condition, not decoration
The prefix was split off as a condition and then matched nothing in `ConditionType`, so it
fell through to UNDEFINED and every one of these abilities fired unconditionally. The server
states the rule itself, identically on all 14 captured `betPillzLink: "more"` entries: the
effect applies "only if the player has bet a number of Pillz strictly greater than N,
including free Pillz and excluding Fury". So the test is `bet + 1 > N`, and Fury's three
Pillz - paid, but not bet - do not count. 1207064 r0 has Tyd win on five Pillz (6 > 6 is
false) and gain no Life; 1131144 r1 has Ilarius bet nothing, leaving Rescue's Support bonus
to carry Callie to 48 Attack; 901004 r0 has Kubrat Cr bet one and take no Life. Fixed
901004, 1131144, 1207064. Tests in `tests/ability/BetPillz.test.ts`.

### Fury is settled with the Damage dealt, not with the Damage modifiers
`CardBattle` added Fury's +2 before the Attack phase, so every POST modifier saw it. Goran's
"+2 Attack Per Opp. Damage" then read a Fury Uuber as 4 Damage instead of 2 and gave Goran
28 Attack where the server reported 24 (8x4 + 2x2 - Hive's Equalizer 3x4). The +2 now lands
after the POST buckets and before the life loss, so the Attack phase and the Attack
modifiers see the printed Damage while the damage dealt and every END effect still count it.
Own and opposing Damage modifiers were already earlier than this (PRE2 / PRE1) and are
unaffected. Fixed 1093129, 1130726. Tests in `tests/ability/FuryDamage.test.ts`.

Whether the multiplier is the opponent's printed Damage or their modified Damage minus Fury
is still open: the only two captured rounds with a Per Opp. Damage card (925899 r0, 1130726
r3) agree on both readings. This implements the second.

### A gift of Opp. Pillz does not need a non-empty pool
`BasicModifier.canApply` required `data.opp.pillz > 0` for every opposing Pillz effect. That
guard exists so a removal cannot drive the counter negative, but it also refused to *pay*
a player who had none: in 1130425 r2 Pr SenQ's "Defeat: +1 Opp. Pillz" was dropped because
its Rescue opponent had just spent their last three. It now applies to removals only.
Fixed 1130425. Tests in `tests/ability/OppPillz.test.ts`.

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

### A Growth permanent keeps the amount it latched with
Abby Salia's `Growth: Heal 1 Max. 12` wins round 1 (zero-based) of 1414168, and the server
heals 2 in round 2 and 2 again in round 3 - the raw battle file carries the post-round
entry with `quantity` 0 in the latching round (Heal is delayed), then 2 and 2. The engine
re-read the Growth multiplier every time the permanent paid, so it healed the current
round's 3 and then 4. The 2 is value x the 1-based round the card won in, which is what
the server's text for every Growth permanent says ("multiplied by the number of the round
in which <card> has won"). Wooly's ordinary `Growth: Power +1` in round 3 of the same battle
still scales by the current round (6 + 4 + 2 = 12), so only a latched permanent freezes.

`Ability.canApply` now multiplies a Growth/Degrowth permanent's change by the latch round
once, when it latches, and drops the multiplier. Fixed 1414168. Tests in
`tests/ability/GrowthPermanent.test.ts`. The evidence is one latch paid twice: it rules
out every reading that rescales by the current round, the rounds since the latch or the
round before, but only the printed text rules out a flat x2 or a count of the owner's lost
rounds. A Growth permanent that latches in round 0 or 2 would settle it; the card data has
20 of them (Growth Heal, Poison, Regen, Toxin and the clan-gated Dope) and no Degrowth one.
The Rust projection refuses every Growth permanent, so nothing changes there; if it ever
admits one, the latch-round factor has to be bound into the latched amount.

The extracted record for 1414168 has `postRoundAbilities` empty in rounds 1 and 2 although
the raw battle file carries the permanent's entries there, so read the raw file when a
permanent's payments matter.

### A reduction that names no opponent hits its own card
Bugamon lv2 prints `Growth: -1 Power And Damage, Min 4`, a drawback on its own card
(`abilityData` 1676, `sideAffected: "player"`). Normalising drops the `Opp` after a negative
number, and the compiler then aimed every `-N <stat>` at the opposing card, so the engine
lowered the opponent instead. Both selected rounds show Bugamon falling by exactly the round
number while the opposing card is untouched: 1088641 r0, Bugamon 8/7 -> 7/6 (35 Attack)
against Aurora at her printed 7/5 plus Support (61), and 1414749 r1, Bugamon 6/5 (18) against
Sandro Cr at 6 + 4 Support - 2 from the opposing Dominion bonus = 8 (48). The Dominion bonus
visibly applies in rounds 0 and 2 of the same match, so it cannot be the missing reduction.

The compiler now reads from the raw text whether it names the opponent, and a Power,
Damage or Attack reduction that does not - and is not a `Cards`/`Xantiax` both-sides form -
lands on its owner's card in the own-stat phase. Life and Pillz reductions are unchanged,
and Backlash still turns itself back in `Condition.compile`. Every ability and bonus text in
the card data and in `captures/abilities.json` was scanned: this is the only combat-stat
reduction that names no opponent, so the fix moves exactly one card-level. Fixed 1088641;
1414749 now fails later (below). Test in `tests/ability/OwnReduction.test.ts`. The Min 4
floor on the owner's card, and how the reduction orders against an own increase, are
unobserved.

### Hazard deals random abilities, and the testcase now records them
Administrator (a Leader; ability `Hazard`, `abilityData.specialAction: "random_abilities"`)
"replaces the abilities of the three cards present in the draw with random abilities that are
already being used in the game". The server swaps the ability of each of its owner's three
other cards before round 0 - even at a level that prints none - and leaves bonuses and the
opponent's cards alone, then every dealt ability resolves by the ordinary rules. The battle's
static block and every hand carry the dealt text. Across the six Hazard captures (874590,
1023946, 1024821, 1078820, 1089830, 1414699) 18 of 18 owner cards differ from
`data/data.json` and 0 of 24 opposing cards do. 874590 was filed here as unreproducible:
Karkass Cr fought with `Brawl: Damage + 1` instead of its Sinister Symmetry, which had looked
like a missing Brawl. 1414699 r0 is XU-Dr0ne's dealt `-4 Opp Attack, Min 1` taking Kephren's
8 Attack to the server's 4.

`ExtractBattle.ts` now writes an optional `abilities` array into the testcase, in card order,
only when a hand holds a random-abilities card: the dealt text for that owner's non-Leader
cards and null elsewhere, so every other record stays byte-identical. It reads the last
snapshot, because a dealt `Copy: Opp. Bonus` shows as the Copy in the first static block
and as the resolved ability once its card is played (874590 and 1078820 both end on
`Support: Attack +3`). The replay gives each such card a private base row
(`Card.withAbility`), so the opponent's copy of the same card and later tests keep the
printed row. The live advisor refuses a Hazard battle outright, since its search and the Rust
worker read printed abilities; the Rust projection already refuses every Leader hand
structurally. Fixed 874590, 1023946, 1024821, 1089830 and 1414699; 1078820 now fails at
round 2 on Protection (below). Test in `tests/solver/Advisor.test.ts` plus the replays.

### Protection: Power And Damage refuses an opposing reduction
The TypeScript `ProtectionModifier` only resisted a Cancel, so an opposing reduction of a
protected card's Power or Damage still landed. The server keeps both stats at the card's own
values. Power refusal is pinned by seven rounds - 1069506 r0 (Miss Pandora stays 7/4 against
Sue's `-1 Opp Power And Damage, Min 3`, 7 x 5 = 35), 949439 r0 (Nebula keeps 7 Power against
Olga Cr's `-2 Opp Power, Min 5`), 1078820 r2 (Agent Brundel's Hazard-dealt Protection, 8 x 3 =
24), 1078555 r2, 1091235 r3, 943111 r3 and 1093569 r1 (a live `Confidence :` reduction) -
and Damage refusal by three: 1069506 r0, 924320 r1 (Donald's `-3 Opp Damage, Min 2` leaves
Nebula on 4) and 942983 r2 (Henry's Support reduction). An opposing Attack reduction still
lands (956805 r2), as do an Exchange, an Impose and Tune Out, none of which is a reduction
modifier, and the Reprisal form refuses nothing when its condition is off (1089830 r1).

Only the plain, unconditional `Protection: Power And Damage` gets the new behaviour: a
per-card guard bit on its Power and Damage, set at PRE3, which an opposing reduction checks
at PRE1. `Protection: Power`, `Protection : Damage`, `Protection: Attack`, the Reprisal,
Revenge and Courage forms and the clan-gated ones have no round showing them meet a
reduction of the stat they name, so they stay Cancel-only; the first two print "cannot be
reduced by an opposing character" and are the likely next candidates once a capture shows
one. Fixed 924320, 949439, 1069506, 1078555, 1078820, 1091235 and 1093569; 942983 and 943111
now fail only on their stale last round (above). Tests in `tests/ability/Protection.test.ts`.
The Rust engine has refused these reductions since semantic revision 23.

### Corrupt lowers its owner's own Life
The TypeScript engine did not implement `Corrupt N Min. M` at all, so Nega D Ld's `Corrupt 2
Min. 5` (`abilityData` 5286: own Life, decrease, `currentRoundRequirement: any`, not
permanent) did nothing. Both selected rounds have Nega winning a knockout with its owner on
6, and the raw battle file reporting a Life decrease of exactly 1 for that owner (6 -> 5, the
Min binding): 1065308 r2 (48 against 44) and 1066210 r2 (64 against 37). It now compiles as
Xantiax's own half on its own - an own Life reduction at the end of the round, floored at
Min, win or lose, not latched - and Stop Opp. Ability still cancels it. Fixed 1065308 and
1066210. Test in `tests/ability/Corrupt.test.ts`.

Both rounds come from the same player and deck, so what is pinned is the owner's side, the
win, the Min floor and paying while the opponent is knocked out. The losing side, the
unclamped amount of 2 and the order against other end-of-round effects follow the printed
text, as Xantiax's own half does; a Nega round that loses, or wins with its owner above 7,
would pin them.

### Abbreviated condition prefixes parse as their conditions
`Abilities.normalise` deletes every `.` before its old `/Asymm\.:?/` replacement ran, so that
replacement never matched, and `Repris.` and `Asy. :` had no mapping at all. The prefixes
reached `Condition` as `Asymm`, `Repris` and `Asy`, which it did not know, and an unknown
condition is met unconditionally - so `[clan] Asymm.: Stop Opp. Ability` (4999), `[clan] Asy.
: -3 Opp Dam., Min 1` (5072), `[clan] Asy. : Copy: Opp. Ability` (5073) and `[clan] Repris.:
Consume 1, Min 4` (5275) all ran without their condition. `Condition.normalise` now expands
`Asymm`/`Asym`/`Asy` to Asymmetry and `Repris` to Reprisal, as `abilityData` names them
(`indexRequirement: asymmetry`, `positionRequirement: defender`), and `Rev`/`Brwl` - printed in
the card data without captured `abilityData` - to Revenge and Brawl, the only conditions their
letters can abbreviate. No replay changes: the four captured games with these texts pass or
fail exactly as before, their selected rounds having satisfied the condition anyway. Test in
`tests/ability/AbbreviatedConditions.test.ts`. A copied ability still compiles without the
copier's conditions, so the conditional Copies (`Asy. :`, Reprisal, Revenge) stay
unconditional in TypeScript; that predates this fix. The remaining unknown conditions are not
abbreviations: Perfect, Disunion, `Bet < N Pillz`, and the Day/Reprisal/Unison Impose forms.

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

### Inactive clan bonuses — nothing to do
Across the captures there are 23 played rounds where the server sends no clan bonus, and in
every one the card is the only member of its clan in the hand. Most of those games already
replay exactly, so the ≥2-same-clan rule is already right.

Damage Exchange was filed here with it and was never the bug. 901004 r0 pins the exchange
itself: Waldegrin Cr lv5 writes 1 Damage and Kubrat Cr lv5 writes 8, and the server reports
them fighting at 8 and 1 - the two *base* values swapped, as `abilityData` 1588 says, and
`ExchangeModifier` already did exactly that. The missing 5 Life was Kubrat's *other* text,
"Bet > 11 Pillz: -5 Opp. Life Min 0", firing on a one-Pillz bet; see the fixed entry above.
Only 3 captured rounds play a Damage Exchange card at all, one of them on a loss.

### Revenge / Damage Impose
- 874712 r1 Tina "Revenge: Power And Damage +2" (lost previous round) vs Kochar "Damage
  Impose" + "Copy: Opp. Ability": engine damage 2, server 4.

### Exchange overwrites the increases before it — 1059149 (TypeScript only)
- 1059149 r1: Calamity's `Power Exchange` against Tina, both printing 5 Power. The server
  gives Tina 6 Power and 36 Attack: the swapped 5, her own +2, then Calamity's -1. The
  TypeScript engine gives 4 and 28, because `ExchangeModifier` runs in PRE2 and *sets*
  `final` to the printed values, wiping any PRE2 increase registered before it - the
  owner's own bonus always (bonus compiles before ability), and the opponent's own increase
  whenever the opponent is internal P1. The Rust engine swaps printed values in the Copy
  phase, before every increase, and reproduces every one of the 12 selected Exchange rounds
  (semantic revision 43); the likely TypeScript fix is to run the swap before PRE2, beside
  `Copy` at PRE3. One round, so recorded rather than coded. A second round where an
  Exchange meets an own increase registered before it would settle it.

### An increase to the opposing card - 1414749 (single point)
- 1414749 r2: Pepo Brahms' `Growth: Opp. Attack +1` (`abilityData` 5210, `sideAffected:
  "opponent"`, increase) raises the opposing Schredder's Attack: the server gives 6 x 3 + 3 =
  21 in round 3, the engine 18. Normalised it reads `Opp +1 Attack`, which neither numeric
  branch of `compileAbility` accepts, so it compiles to nothing. 5210 is the only
  opponent-increase combat-stat definition in `captures/abilities.json` and this is its only
  selected round, so it is recorded rather than coded.

## Fresh capture backlog

The expanded corpus now has 39 additional mismatches that have not yet been
grouped or attributed to rules. They are recorded as regression targets only; inspect the
first failing round and group them by ability keyword before changing the engine:

924573, 924615, 924669, 924740, 924853, 924890, 925818, 942983, 943111,
943231, 946810, 947010, 947670, 948108, 948390, 956902,
1024592, 1024732, 1025413, 1059149, 1060341,
1079078, 1089974, 1090269, 1091381, 1092066.

The 2026-09-23 live session added three more: 1414168, 1414699 and 1414749. The first two
are fixed and the last has had its first failure fixed (all above, as are 1088641, the
Hazard games 1023946, 1024821 and 1089830, and the Protection games 924320, 949439, 1069506,
1078555, 1078820, 1091235 and 1093569, and the Corrupt games 1065308 and 1066210). A fourth,
1414087, has no testcase at all: its opponent deals card 2714 (level 2, `Brawl: Damage + 1`),
which is newer than the 2026-09-10 character dump, so the extractor has no name, clan or
stats for it. Run `__ur.dumpCharacters()` and `deno task cards`, then `deno task extract`.

The five that arrived with the 2026-09-17 captures are no longer here: 1130425, 1130726,
1131144, 1207064 and 1093129 are all fixed above, and so is 901004, which had been filed
under Damage Exchange since the first triage pass.

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
  that clan; a Day/Night card, where the DB row is the day variant; or a Hazard game, where
  the abilities are random and the testcase now carries them. None of these are card-data bugs, but all of them will look
  like engine bugs in a replay.
- Brawl, Support, Growth, Degrowth and Equalizer are per-X multipliers (`Per` in
  `BasicModifier.ts`). Symmetry and Asymmetry are instead conditions that gate the complete
  effect by equality or inequality of the two cards' immutable original hand slots. Do not
  diagnose either family using the other's execution model.
- **Illusion rewrites a slot's whole card identity in the capture, not just its ability.**
  In battle 1130527 the server showed side 0's slot 2 as Bonnie Ld (808) in every status
  snapshot and only revealed Kate (1966) in a static block rewritten at the end. Kate is what
  actually fought: the round resolved at 9 power / 5 damage, her level-3 stats, not Bonnie Ld
  level 1's 8/1. `ExtractBattle.ts` builds each player's hand from the last snapshot, so a
  move first seen under the disguise used to contradict its own hand; it now follows the hand
  and records the disguise as `displayedCardId` beside it. That capture is the only one of
  359 that carries the field. Expect more as Kate and her siblings turn up, and do not treat
  a `moves[].cardId` as evidence about a card's real identity.
