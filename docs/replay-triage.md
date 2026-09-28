# Replay triage — engine vs server mismatches

Status from `deno test -A --no-check tests/replay/` against 774 captured battles
(761 replay-ready; 13 ignored because they stopped mid-match): 757 replay exactly and 4
mismatch. Each entry
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
| 2026-09-25 | 368 | 8 | `/ Life Lost` is Per; conditional Copy gated; Naja and Dope pay after KO; Recover win + floor; stale final rounds attributed |
| 2026-09-25 | 368 | 8 | A conditional `Protection: Power And Damage` refuses reductions while its condition holds (no replay moves) |
| 2026-09-26 | 369 | 8 | Card refresh (2498 cards): 1414087 replays exactly; rebalanced abilities replay with the battle's own text |
| 2026-09-26 | 376 | 8 | +7 Training captures (Leader test deck); Counter-attack only decides round one (1495980) |
| 2026-09-26 | 382 | 6 | +4 Training captures (GhosTown/Oculus test deck); the round-one second mover's END effects run first (1093173, 1496283); Perfect pays only on the exact bet (947670, 1496258) |
| 2026-09-27 | 387 | 7 | +6 Training captures (Lab 1 by day, Lab 3); Solomon's Tie-break wins tied rounds (1506259); 1506438 settles same-family Poison as replace, not stack (open) |
| 2026-09-27 | 402 | 12 | +20 Training captures (Labs 3-6): second rounds for Exchange (1507008), Cancel (1506931, 1507792) and Protection: Cards (1507713, 1507819), all open |
| 2026-09-27 | 408 | 6 | A Cancel Opp. Modif. spares the canceller's own reductions (1089974, 1506931, 1507792); Exchange and Impose run before every increase (1059149, 1507008, 874712) |
| 2026-09-27 | 412 | 2 | Protection: Cards guards both cards (1507819); Growth: Opp. Attack raises the opposing Attack (1414749, 1507713); same-family permanents replace (1506438) |
| 2026-09-27 | 427 | 3 | +18 Training captures (Labs 6-8, the Team Leaders); an opposing resource Cancel deactivates permanents for its round (1508676); 1508712 is a third Protection: Cards round; 1508932 is a new single point (a binding cap beside a same-stat bonus) |
| 2026-09-27 | 470 | 9 | +53 Training captures (autoplay run 1): six fresh mismatches (1514649, 1514836, 1515298, 1515451, 1515574, 1515692) |
| 2026-09-27 | 471 | 8 | A recap snapshot no longer overwrites a round's resolution (1514649) |
| 2026-09-27 | 471 | 8 | Two Oculus in one hand infiltrate nothing (1496283; no replay moves on its own) |
| 2026-09-27 | 474 | 5 | The end of the round settles every increase before every decrease, the decreases by descending Min (1514836, 1515298, 1515451); supersedes the round-one-seat order |
| 2026-09-27 | 475 | 4 | A single-stat Protection refuses an opposing reduction of its stat (1515574); 1515692 is a new single point (Per Opp. Damage against an opposing Damage bonus) |
| 2026-09-27 | 559 | 15 | +95 Training captures (autoplay runs 2-3): eleven fresh mismatches (1516740, 1516811, 1516832, 1516846, 1516906, 1517029, 1517121, 1517236, 1517271, 1517397, 1517419) |
| 2026-09-27 | 561 | 13 | A `Stop:` permanent latches only when its ability is stopped (1516740, 1517419; 1517397 moves on to round 3) |
| 2026-09-27 | 562 | 12 | A condition prefix no longer swallows an Impose (1517397) |
| 2026-09-27 | 568 | 6 | A capped increase is measured before its card's same-stat bonus (1516811, 1516832, 1516846, 1516906, 1517271, and the single point 1508932) |
| 2026-09-27 | 569 | 5 | Latched permanents pay after the round's own effects (1517029; Rust semantic revision 80) |
| 2026-09-27 | 570 | 4 | Versus and After read an Oculus by its printed clan (1517121); 1517236 is a new single point (an opposing `Players Life` gain revives a knocked-out player) |
| 2026-09-27 | 748 | 13 | +188 Training captures (autoplay runs 3-5): nine fresh mismatches (1518052, 1518765, 1519318, 1519333, 1519829, 1519871, 1520327, 1520579, 1521010) |
| 2026-09-27 | 752 | 9 | `+N Attack Per Opp. Damage` counts the printed opposing Damage (1515692, 1518052, 1518765, 1519829; Rust semantic revision 81) |
| 2026-09-27 | 755 | 6 | An opposing `Cancel Opp. <stat> Modif.` beats a stat Protection (1519871, 1520579, 1521010) |
| 2026-09-27 | 756 | 5 | `Disunion:` is Unison's complement (1519333) |
| 2026-09-27 | 757 | 4 | A latch from the other side replaces one of its family on the same player (1519318); 1520327 is a new single point (a two-way Copy loop whose displayed Attack contradicts the round's winner) |

## Fixed

### Robert Cobb's `Bypass` activates every clan bonus in its hand - 1529161 (fixed)
`Bypass` is Robert Cobb's Leader ability: "The bonus of your other cards is active even if you
don't have another card from the same clan. Their bonus can still be blocked by your opponent's
'Stop Opp. Bonus' cards" (`abilityData` 1546, specialAction `activate_all_bonuses`, first captured
in the autoplay runs 5-10). The engine switched off every lone card's bonus when the match was
built. In all seven captured hands with a lone Robert Cobb (1521878, 1528037, 1529161, 1529585,
1530253, 1532896, 1534028) the server sends every lone card's clan bonus - Mattachione's Montana
cut, Naliah Cr's and Stanly's Ulu Watu `Power +2`, Dorga's Tolvack `After`, Aisha's `+2 Life`,
Buford's `Tune Out`, Carlos Cr's `Attack +8` - and Robert Cobb's own `Cancel Leader` too. 1529161
r2 is the round it decides: Stanly, the hand's only Ulu Watu, fights at 6 + 2 = 8 Power, 8 x 7 =
56 (the engine had 6 and 42). The other six replayed either way.

The Game constructor now leaves every bonus of a hand whose lone Leader prints `Bypass` alone.
`getLeader()` is undefined beside a second Leader, whose `Cancel Leader` deactivates Bypass as it
does every Leader ability. Fixed 1529161. Tests in `tests/ability/Bypass.test.ts`. The Rust catalog
refuses a lone Robert Cobb as it does every unlisted Leader hand, and its corpus derivation tests
check the seven hands against the printed clan bonuses (commit "Add the autoplay runs 5-10
registry inventories").

### `-N Players Pillz` reduces both players' Pillz - 1527810, 1533508, 1533638 (fixed)
D4ggers' `-2 Players Pillz. Min 4` is "If D4ggers wins the round, the number of Pillz of the two
competing players will be reduced by 2 Pillz, or up to a minimum of 4" (`abilityData` 3202;
Karter's 3608 prints Min 5). In the negative branch of `compileAbility` "Players" was an unknown
stat, so the text compiled to nothing. Three autoplay rounds, all won:
- 1533508 r0: Karter wins on 3 pillz under the Cosmohnuts `Tune Out`; his owner goes 12 - 3 = 9
  -> 7 and the opponent 12 -> 10 (the engine had 9 and 12).
- 1527810 r1: D4ggers wins on no pillz and takes his own owner 8 -> 6; the opponent is on 0.
- 1533638 r1: D4ggers wins on 4 pillz: 10 - 4 = 6 -> 4, held at the Min; the opponent is on 0.

"Players" now duplicates the reduction onto both players as "Cards" does onto both cards, and
unlike Xantiax and Cards it keeps the win requirement its text prints (no losing round is
captured). Fixed 1527810, 1533508 and 1533638. Tests in `tests/ability/PlayersPillz.test.ts`. The
Rust registry does not compile the both-sides Pillz reduction and no draw holding D4ggers or
Karter is eligible, so it needs no change.

### `Players Combust` takes Life and Pillz from both players - 1521294, 1527285, 1528501, 1529213 (fixed)
Bobby Cornteeth's `Players Combust 1, Min 0` is "If Bobby Cornteeth wins the round, at the end of
each of the following turns, both players will lose 1 Life points and Pillz, minimum 0"
(`abilityData` 5580, `life&pillz`, sideAffected both, permanent). It normalises to `Players 1
Combust Min 0`, which no branch of `compileAbility` read, so it did nothing. Four autoplay
captures play it, and from the round after the win each player pays 1 Life and 1 Pillz, each with
a permanent entry posted (a player on 0 Pillz posts none; the latching round posts zero entries on
both, as a delayed Poison does):
- 1521294: Bobby wins r1; r2 takes Cley's 6 and 1 more from the recorder (16 -> 9), and 1 Life and
  1 Pillz from his own owner (12 -> 11, 11 - 2 bet -> 8); r3 does it again (9 - 7 - 1 = 1, 11 -> 10).
- 1527285: Bobby wins r0; in r1 Lucien's 1 Damage and `-6 Opp. Life Min 0` take the recorder to 3,
  the Combust to 2 and 12 - 2 -> 9 Pillz, and its own owner to 14 and 7 - 6 -> 0.
- 1528501: Bobby wins r2 on Fury; r3 takes the recorder from 8 - 7 = 1 to 0 and its owner to 5 and
  7 -> 6 Pillz.
- 1529213: Bobby wins r0; r1 takes both (4 -> 3 and 15 -> 14, 12 -> 11 and 5 -> 4), and in r2 the
  recorder, knocked out by Pulsar's 7, still loses 1 Pillz (6 -> 5).

The Combust branch now reads a leading `Players` and compiles the Life and Pillz decreases on both
sides, delayed like the plain Combust. Replays: 1521294, 1527285, 1528501 and 1529213 fixed, no
other moved. The owner's own half follows the ordinary knockout guard (a knocked-out owner pays
nothing), which no round shows. Tests in `tests/ability/PlayersCombust.test.ts`. The Rust registry
lists the both-sides Combust as unsupported (the Combust grammar reads `sideAffected: opponent`),
and no draw with a Players Combust card is eligible, so it needs no change.

### A latch from the other side replaces one of its family on the same player - 1519318 (fixed)
1519318 is the cross-owner case "Same-family permanents replace" left following the printed
text. The opposing Obyl Ld's `Poison 1, Min 0` latches on P1 in round zero; in round one P1's own
Gork wins with `Backlash: Poison 2, Min 4`, a Poison on its owner. The Cosmohnuts `Tune Out`
decides every round by the bet.
- r1: P1 goes 11 -> 10, Obyl's 1 alone: the Backlash is in its delayed round. The resolution
  snapshot posts the permanent entries [1, 0] on P1, as a delayed newcomer does beside the latch
  it will replace (1131208 r2, 1092369 r2, 926420 r3).
- r2: Maraval's 1 Damage, then 2: 10 - 1 - 2 = 7. Both paying gives 6, which the engine had. The
  snapshot posts one permanent entry, 2.
- r3: Colton's 1 Damage, then 2, held at the Backlash's Min 4: 7 - 1 - 2 = 4, and again one entry.

So "If two poisons or toxins are applied, the second will replace the first as soon as the
latter takes effect" holds whichever side latched them. The engine compared one side's latches
only, and a player's own `Backlash:` permanent sits in its owner's `Events` while an opposing one
sits in the other. Each `Ability` now records the round it latched in (`since`, set with `won`),
and `Events.executeRepeat`, given the other side at the end of the round, also treats a latch as
replaced when the other side holds a newer one of its family aimed at the same player that pays
this round (`replacedAcross`). Two such latches from one round replace neither: unobserved. The
continuation cache keys `since` with each entry's flags. Across the replays the cross-side
replacement fires in this one round only. Fixed 1519318. Test in
`tests/ability/LatchReplace.test.ts`. The Rust engine refuses every `Backlash:` permanent, and no
pairing it admits puts two owners' latches of one family on one player, so it needs no change.

### Disunion is Unison's complement - 1519333 (fixed)
1519333 r0: Bernardite level 3's `Disunion: +1 Life` wins on 5 pillz under the opposing
Cosmohnuts `Tune Out`, in a hand of four Rescue, and her owner stays on 15; the engine gave 16.
`Disunion` was not a known condition, so it was met unconditionally. The server's text is Unison's
complement: "The ability is only activated if your hand contains at least one character from a
clan other than the Disunion character. Oculus characters who have infiltrated the clan do not
count towards the activation of the ability" (4388; Karsen's `Disunion: -10 Opp Attack, Min 0`,
5751, says the same, and both records carry `isAntiClanmatesCountLinked`). 1509142 r0 is the other
direction: Bernardite wins beside a Frozn card and her owner goes 15 -> 16. The other three
captured Disunion rounds decide nothing: 1130654 r2 loses, and in 1081037 r0 and 1089262 r2
Spidee's `Reprisal: Stop Opp. Ability` stops Karsen. `ConditionType.DISUNION` holds when fewer than
four of the hand's distinct characters share the card's clan, which is `Unison`'s count
(`Hand.getClanCards`, where an infiltrated Oculus is a clan-mate). Fixed 1519333. Tests in
`tests/ability/Disunion.test.ts`, and `tests/solver/Policy.test.ts` moves with it (see "Reanimate
is an immediate Defeat life gain"). The Rust registry lists the anti-clan-mates link as an
unsupported linked magnitude and no compiler grammar reads it, so neither text is admitted there.

### An opposing Cancel beats a stat Protection - 1519871, 1520579, 1521010 (fixed)
Three autoplay rounds, two cards and two stats, have a stat Protection's own modifier of that stat
meet an opposing `Cancel Opp. <stat> Modif.` ("Any modifier of the opposing character affecting
attack will be deactivated. This applies to attack reductions AND increases", 1163), and each time
the Cancel wins:
- 1519871 r0: Gemini level 3's `Protection: Attack` beside its Hive bonus `Equalizer: -3 Opp
  Attack, Min 5`, against the Raptors bonus `Cancel Opp. Attack Modif.`: Sauropsite fights at
  8 x 7 = 56, where the Equalizer would take 3 x 3 (the engine had 47).
- 1521010 r0: the same Gemini against St4rve Ld level 2, 6 x 3 + 5 x 4 (`Brawl: Attack +5` over
  four Hive) = 38, where the Equalizer would take 3 x 2 (32).
- 1520579 r1: Davis level 3's `Protection : Damage` beside his La Junta `Damage +2`, against
  Lenora's `Cancel Opp. Damage Modif.`: Davis wins on his printed 3 Damage (the engine had 5).

`ProtectionModifier` set the protected stat's `prot` bit, which makes the stat's `blocked`
(cancelled and not protected) false, so an opposing Cancel left the card's own modifiers of the
stat alone. That was the engine's original model of every Protection ("resisted a Cancel only",
see "Protection: Power And Damage refuses an opposing reduction"), and no round had pinned it for
a stat: over the corpus these three are the only rounds where a stat Protection's card has a
modifier of the protected stat facing a Cancel of it. 1515809 r3 (Clifford's `Protection: Attack`,
whose only other text is `Power +2`), 1519762 r2 (Gregor Ld's `Asymmetry: Protection: Power`,
with nothing of its own on Power) and 962404 r1 (Nebula's `Protection: Power And Damage` against
a Cancel of Attack) have nothing for the Cancel to decide. A stat Protection now sets only its
guard, which still refuses an opposing reduction of the stat. Whether the Cancel also switches
the guard off - an opposing reduction of the protected stat beside the Cancel - is unobserved,
so the guard stays. `Protection: Ability` and `Protection: Bonus` keep resisting a Stop. Fixed
1519871, 1520579 and 1521010. Tests in `tests/ability/ProtectionCancel.test.ts`. The Rust engine
already let the Cancel win (its cancellation and protection masks are independent in
`apply_power_damage_effect`), and it refuses a single-stat Protection facing an opposing Cancel
of its stat (`SingleStatProtectionAgainstUnpinnedEffect`); these rounds pin that meeting for the
Attack and Damage forms, for a slice that wants to lift it.

### `+N Attack Per Opp. Damage` counts the printed opposing Damage - 1515692, 1518052, 1518765, 1519829 (fixed)
1515692 r3 was a single point: Shawnia's `+3 Attack Per Opp. Damage` against Helsa level 2
(8/2), whose La Junta `Damage +2` takes her to 4. The server has 7 x 1 + 3 x 2 = 13, the printed
Damage; the engine read the modified 4 and had 19. Autoplay runs 3-5 brought three more rounds,
each against a different kind of modifier on the opposing card's own Damage, and every one counts
the printed value:
- 1518052 r0: Shawnia again, against Coby Cr level 4 (8/3) on 7 under his Sentinel `Support:
  Damage +1` x 4: 7 x 4 + 3 x 3 = 37 (the engine had 49).
- 1518765 r1: Adytia Ld level 2's `+2 Attack Per Opp. Damage` against Kinjo Cr level 5 (6/5) on 10
  under his `Damage +3` and the Fang Pi Clang `Damage +2`: 8 x 1 + 2 x 5 = 18 (28).
- 1519829 r1: Cobretti level 5's `+2` against Baxter level 3 (6/3), whose `Copy: Power And Damage
  Opp.` gives him Cobretti's printed 7/7: 7 x 1 + 2 x 3 + 8 (Sentinel `Attack +8`) - 8 (Sakrohm
  `-8 Opp Attack, Min 3`) = 13 (21).

Goran's `+2` against a Fury Uuber (1130726 r3) reads the printed 2 too, which settling Fury late
had already modelled. Of the 78 captured card-rounds that play a `Per Opp.` card, eight face an
opposing stat that ends away from its printed value: these four, 1130726 r3, and three that pay
nothing either way - 1515465 r2 and 1518648 r2 (Kenny West's and AbsorptionBoy's `+1 Life Per Opp.
Damage` lose) and 1520737 r1 (Genash's `Cancel Opp. Attack Modif.` switches Adytia Ld's off). No
round has the converting card's own side reducing the opposing Damage, so whether the reading is
before that reduction as well is unobserved; the printed value reads before it.

`BasicModifier` has a new multiplier, `OPP_PRINTED_DAMAGE` (the opposing card's `damage.base`),
which `AbilityParser.per` picks at compile time for an Attack modifier per opposing Damage. The
other two `Per Opp.` grammars keep what they read: `+N Life Per Opp. Damage` the final Damage
(Fury included), and `+N Attack Per Opp. Power` the resolved Power, since no round has shown
either against a modified stat. These four rounds suggest the Power form reads the printed Power
too; a `Per Opp. Power` card against a card with its own Power modifier would say. Fixed 1515692,
1518052, 1518765 and 1519829. Tests in `tests/ability/PerOppDamage.test.ts`. The Rust engine read
the resolved Damage before Fury (semantic revision 25); semantic revision 81 reads the printed one,
so the two engines agree again (`docs/rust-migration.md`).

### Versus and After read an Oculus by its printed clan - 1517121 (fixed)
1517121 r1: Predtr Ld lv3 (Vortex, 9/4) meets Nobutomo lv2, whose `Equalizer: -1 Opp Damage,
Min 3` would take Predtr Ld to 3 Damage (the engine's number). The server leaves it on 4: Predtr
Ld's `Versus [clan:51][clan:56] : Stop Opp. Ability` ("The opponent's Ability is cancelled if
they have one. This ability only activates if there's a Hive or Oculus in your opponent's hand",
`abilityData` 3684) stopped the Equalizer. The opposing hand holds Dark Sentogan, an Oculus that
fights as a Montana there (the server sends it Montana's `-12 Opp Attack, Min 8`), and the engine
read the infiltrated clan and found no Oculus. The Versus condition now reads each opposing card's
printed clan.

The same question for `After [clan:...]`, which reads the owner's previous card, is settled by the
printed text alone: every After text that names a clan other than the Oculus says "The Oculus,
even when infiltrated into the Frozn clan, do not activate this condition" (5602, 5670, 5681,
5700-5702, 5708, 5723, 5732, 5738, 5757, 5779, 5780, 5820, 5847, 5853-5855), and the two that name
the Oculus (`[clan:56][clan:60]`, 5585 and 5750) say "played an Oculus or Tolvack character". The
engine recorded the previous card's infiltrated clan, so an Oculus that joined Frozn would have
fired Noma's `After [clan:47]` and missed the Tolvack bonus. It now records the printed clan. No
captured round tells the two apart for After: in the four rounds where an After source follows an
Oculus (1022847 r3, 1066481 r3, 1073107 r1, 1090096 r1), the Oculus had joined Tolvack and every
text there either names both or neither. A Versus that names the clan an Oculus joined (a Versus
[Montana] against Dark Sentogan here) is unobserved; the rule reads it as printed, so it does not
count. Fixed 1517121. Tests in
`tests/ability/VersusClan.test.ts` and `tests/ability/After.test.ts`. The Rust engine already reads
canonical clans for both and refuses a draw where an infiltrating Oculus would make the canonical
and effective readings disagree; these rounds pin the canonical reading for a slice that wants to
lift that.

### Latched permanents pay after the round's own effects - 1517029 (fixed)
1517029 r2: Lakit Cr's owner (internal P2) has a `Heal 4 Max. 7` latched from round one and is
on 15. Owen wins for 5 Damage, 10, and his `-4 Opp. Life Min 2` takes that to 6. The server ends
on 7: the Heal paid after the cut, 6 + 4 capped at 7. The engine ended on 6, because its end of
the round (the order "End-of-round order" below settled) paid every increase first, latched
ones included, and the Heal found 10 above its cap and paid nothing.

The raw battle files say which order the server uses. The resolution snapshot lists the round's
end-of-round effects on each player as `post` entries, and the list is in application order: in
1517029 r2 Lakit Cr's owner gets `life -4` and then `life +1 (permanent)`. (The extractor keeps
the entries of the snapshot after the round, which in these Training captures usually has none,
so `postRoundAbilities` in `captures/games/` is often empty; the resolution snapshot has them.)
Over every capture, a player's list puts the round's own entries ahead of the latched ones,
never the other way round. A latched gain follows a current-round cut in 1516124 r0, 1516372 r1
and r2, 924669 r0 and 1514883 r2 as well, where no cap binds; within the round's own entries the
increases come first (15 lists, 1093173 r1 and 1514836 r0 among them), and within the latched
ones the gains (1515451 r1 and r3, 1516711 r3, 1517078 r2 and r3). The only list with a latched
cut ahead of a latched gain, 1515238 r3, has the gain as the zero entry of a Heal latching in
that round.

`Events.executeEnd` now pays the round's own increases, then its decreases by descending Min, and
only then every latched increase and every latched decrease, internal P2 before P1 within a pass
as before. Replaying every round on its own with the server's totals before it, the new order fits
every round the previous one fitted, plus 1517029 r2; the rounds either order fails are the other
fresh mismatches. Fixed 1517029. Test in `tests/ability/EndOrder.test.ts`.

The Rust engine paid each owner's latches right after its own current-round effects, P1 first:
right for this round by seat (the Heal was P2's), wrong with the seats swapped - and the previous
TypeScript order was wrong on the other seat, so the two engines disagreed on an admitted pairing
(a catalog probe prepares Lakit Cr L4 beside Owen L5 either way round). Semantic revision 80 moves
its latched pay after both owners' current-round effects, gains before reductions, so the two
agree again (`docs/rust-migration.md`). A latched gain of P2 beside a latched cut of P1's on the
same player follows the rule unobserved: every round above with both has the gain on P1.

### A capped increase is measured before its card's bonus - Razor and 1508932 (fixed)
Five autoplay rounds failed on one card: Razor level 4 (Ulu Watu, 5/6), `+1 Power Per Life Lost
Max. 9` (`abilityData` 5556: "increased by 1 point for every Life point lost ... up to a maximum
of 9"), beside its clan bonus `Power +2`. The server has 11 Power every time:
- 1516811 r1: 5 lost, 5 + 5 = 10 capped at 9, + 2 = 11 (the engine had 9);
- 1516832 r1: 7 lost, 12 capped at 9, + 2 = 11;
- 1516846 r1: 5 lost, 11, then Kruger's `-2 Opp Power And Damage, Min 4` = 9 (the engine had 7);
- 1516906 r2: 6 lost, 11 capped at 9, + 2 = 11;
- 1517271 r1: 4 lost, 9, + 2 = 11, and 11 x 8 = 88 (the engine had 72).

The engine ran the bonus first, as it does for every same-phase pair, and capped the total:
5 + 2 + lost, capped at 9. That is exactly the single point 1508932 r2 recorded (P. Steevens Cr
lv4's `+1 Damage Per Life Lost Max. 7` beside La Junta's `Damage +2`, 9 lost: the server's 9 is
2 + 9 capped at 7, + 2), so six rounds on two cards and two stats now say the cap is measured
before the bonus lands. The two readings the single point left open - the cap measured on the
printed stat, or the capped ability before the bonus - agree on every captured shape: only card
abilities print a capped combat-stat increase (no clan bonus does), and a card has one bonus.
They would part only where something writes the stat before the card's own modifiers, such as
an Exchange or an Impose, which no capture shows beside a cap.

`Ability.capsOwnStat` now queues a card ability that caps an own Power, Damage or Attack increase
at the front of its phase (`Events.addFirst`), ahead of the bonus. It is a compile-time choice,
so the solver's hot path is unchanged. The six other binding caps the corpus has (Wachtmann,
Veles Cr, Sir Taco, Noeptus, KinGreow twice, Jochar) are on untouched printed values and move
nothing. Fixed 1508932, 1516811, 1516832, 1516846, 1516906 and 1517271. Tests in
`tests/ability/CapBeforeBonus.test.ts`. The Rust engine refuses Razor's and P. Steevens Cr's
texts, and refuses its one admitted capped increase, Jochar's `Power +N, Max. M`, wherever
another own Power increase could meet it (`CappedPowerIncreaseAgainstOwnPowerIncrease`); these
rounds pin that order for a slice that wants to lift it.

### A condition prefix no longer swallows an Impose - 1517397 (fixed)
With its Stop fixed (next entry), 1517397 failed in r3 on Noma's Power: the engine had 9, the
server 5. Zodiack lv3 fights it by day with `Day: Power Impose` ("The opposing character has
equal Power to Zodiack. This number only takes into account the figure shown on Zodiack",
`abilityData` 5560): 5 x 1 = 5, which Montana's `-12 Opp Attack, Min 8` leaves alone below its
Min. `Abilities.normalise` moved "Impose" to the front of the whole text with `^(.+) Impose`,
prefix and all, so the text came out as `Impose Day: Power` - an unknown condition `Impose Day`
over an ability `Power` that compiles to nothing. Every prefixed Impose the card list prints
did nothing: five `Reprisal: Damage Impose`, `Unison : Damage Impose`, `Day: Power Impose` and
Oryon's `Versus [clan:56][clan:30] : Power Impose`. The swap now starts after the last
condition prefix. It is a parse fix, not a new rule: Impose itself is pinned (see "Exchange and
Impose write printed values before every increase"), and so are the prefixes. Oryon is played
in six autoplay rounds, all against hands with neither an Oculus nor a Sakrohm, so its Versus
is off and nothing else moves. Fixed 1517397. Tests in `tests/ability/ImposePrefix.test.ts`. The
Rust engine refuses `Day:` Impose (`DescriptionContext Day`).

### A `Stop:` permanent latches only when its ability is stopped - 1516740, 1517397, 1517419 (fixed)
`Stop: X` fires only when the opposing card cancels this card's ability: "If Ardwizz's ability
is cancelled out by the opposing character, If Ardwizz wins the round, at the end of each of the
following rounds the player controlling Ardwizz will earn 2 Life point(s) if he/she has fewer
than 20 Life points" (`abilityData` 1629, `isInverted`). `Condition.compile` sets up the Stop
condition by the ability's type, and a permanent is `GLOBAL_ABILITY` by then (`compileAbility`
runs first), which neither branch named: the condition was left without its target and held
unconditionally, so every won round latched it. Three autoplay rounds have one win unstopped,
facing no Stop Opp. Ability, and the server latches nothing:
- 1516740 r2: Ardwizz's `Stop: Heal 2 Max. 20` wins; in r3 its owner goes 8 -> 6 on Dr Swamp's
  Poison alone (the engine healed 2 first and had 8).
- 1517397 r1: Giacomo's `Stop: Poison 3, Min 2` wins; r2 leaves the target on 13 (the engine had
  10).
- 1517419 r2: Giacomo again; r3 ends on 8 + 3 (Talhia's `Bet > 3 Pillz: +3 Life`) = 11, where the
  engine took 3 more.

The condition now treats `GLOBAL_ABILITY`/`GLOBAL_BONUS` like the one-round types. The other
half, a stopped `Stop:` permanent latching, follows the printed text: no capture has one stopped
yet (the fourth captured round, Ylati's `Stop: Heal 3 Max. 14` in 1516862 r3, is the last round
and wins unstopped). The three texts the registry prints are Giacomo's `1147`, Ylati's `1312`
and Ardwizz's `1629`. Fixed 1516740 and 1517419; 1517397 then failed in round 3 on the prefixed
Impose (the entry above). Tests in `tests/ability/StopPermanent.test.ts`. The Rust engine admits
none of the three.

### A recap snapshot is not the round's resolution - 1514649 (fixed)
1514649 r0 failed on El Toucan's Attack: the engine had 20, the record 14. El Toucan level 3
(7/4) is cut to 5 Power by Kaskar's `Equalizer: -1 Opp Pow. & Dam., Min 5` (x3 stars, held at
Min 5) and adds its Huracan `+1 Attack Per Life Left` on 15 Life: 5 + 15 = 20, which is what
the snapshot taken as the round resolved says. The 14 came from a later snapshot that
reported round 0 again. After a game the site fetches recaps of the earlier rounds, and the
server rebuilds each one with the Life of the moment - 9 after round one, so 5 + 9 = 14. The
extractor kept the last snapshot that showed a round's Attack, so the recap overwrote the live
value. `ExtractBattle.ts` now ignores a snapshot whose round is below one it has already seen,
for anything a live snapshot has already recorded. 27 captures carry such recaps; re-extracting
changed 1514649's Attack and only the transport field `damageAfter` of seven others (925169,
1080464, 1090887, 1130454, 1414581, 1514836, 1514916), which no replay reads. Fixed 1514649.
The first reading of this round, that El Toucan's bonus counts something other than the Life
left, was the artifact: every other captured `Per Life Left` round reads the full Life.

### A stale final round is attributed from the result block
When the extractor cannot tell which side is the recorder's (`mySide` is null), it could not
attribute the closing `battles.result`, so the final round kept the pre-damage snapshot the
server sends before applying the last hit: in 924615 Vektor wins round 3 for 2 Damage and the
record still said the loser was on 5. Nothing was wrong with the engine. `ExtractBattle.ts`
now tries both assignments of `result.player`/`result.opponent`: it takes the previous round's
life and pillz, charges each side its bet (plus Fury), gives the loser the winner's damage and
credits the round's `postRoundAbilities` to their players, and fills the final round only
when exactly one assignment matches the result on both sides; otherwise the stale issue stays.
All eight flagged games fit exactly one way - 924573, 924615, 924669, 924740, 942983, 943111,
948108 and 948390 - and now replay exactly; 943231 and 945585, whose values already matched,
only lose the issue line. Test in `tests/BattleCapture.test.ts`. The Rust combat-stat gate
still carries 924615 for three rounds, as it was fixed before this.


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
the same fully searched recommendation is 86%. Since `Disunion:` stopped paying Bernardite's
+1 Life in that all-Rescue hand (see "Disunion is Unison's complement"), one pill wins every
line again, for the right reason, and the policy test pins Reanimate on Miyo's two-pill line
instead: 91%, where a lethal-only Reanimate gives 100%. Reanimate is also allowed to lift its owner
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
was open until the autoplay runs 3-5: it is the printed Damage (see "`+N Attack Per Opp.
Damage` counts the printed opposing Damage" above).

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
Rust semantic revision 74 admits the two Growth permanents with a registry record
(`Growth: Heal` `4959`, `Growth: Poison` `1266`/`1282`) and binds the latch-round factor into
the latched amount when it latches, so the repeat never reads the round again; 1414168 is in
its gate.

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
unobserved. Rust semantic revision 74 admits `1676` in the owner's own phase, before the
opposing reductions (the order 1079078/3 pins for the own half of `Cards`), and refuses it
beside another own change to the same card, a Protection there, an opposing cancel of Power
or Damage modifiers, or an opposing Copy; 1088641 is in its gate.

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

At first only the plain, unconditional `Protection: Power And Damage` got the new behaviour:
a per-card guard bit on its Power and Damage, set at PRE3, which an opposing reduction checks
at PRE1. Since 2026-09-25 the guard is set whenever a Power And Damage Protection applies at
all, because the ability's conditions (Reprisal, Revenge, Courage) already gate the modifier:
the conditional forms refuse a reduction while their condition holds, by composition with the
plain form's refusal. No captured round shows one live against a reduction - Forjoten Ld's
`Reprisal: Protect. Power And Damage` is live with nothing to refuse in 1131114 r2 - and the
Reprisal form lets Callie's cut land when its condition is off (1089830 r1). The replays are
unchanged, 368 exact and 8 mismatches before and after; the Rust engine admits the Reprisal
form under the same rule since semantic revision 75. The Cards (both sides) form was
Cancel-only until 2026-09-27 (next entry), and so were the single-stat forms until 1515574
(the entry after it). Fixed 924320, 949439, 1069506, 1078555, 1078820, 1091235 and 1093569; 942983 and 943111
now fail only on their stale last round (above). Tests in `tests/ability/Protection.test.ts`,
including a constructed Forjoten Ld that keeps 8/5 against Sue's cut when it moves second and
falls to 7/4 when it moves first. The Rust engine has refused these reductions since semantic
revision 23.

### Protection: Cards Power And Damage guards both cards
Khrull Cr's `Protection: Cards Power And Damage` (`abilityData` 2255, sideAffected "both":
"The Power and Damage of both characters cannot be reduced by the opposing character") only
resisted a Cancel, so its owner's own Dominion bonus `Growth: -1 Opp Power, Min 4` still cut
the opposing card. Two rounds pin it, with the Khrull Cr on each side of the table:
- 1507713 r1: Twyh keeps 5 Power, 5 x 4 = 20, where round two's -2 (clamped at 4) gave 16.
- 1507819 r2: Nancy keeps 7 Power, 7 x 1 + 12 (her Rescue `Support: Attack +3` x 4) = 19,
  where round three's -3 gave 4 + 12 = 16.

The guard that `Protection: Power And Damage` sets on its own card is now set on both cards,
so every reduction either card aims at the other's Power or Damage is refused. Neither round
shows an opposing reduction of the Khrull Cr itself, which "both characters" covers, or an
increase to a guarded stat, which the text does not name, so increases still land. These are
the only captured rounds with the ability, and Khrull Cr (levels 3 and 4) is the only card
that prints it. Fixed 1507819; 1507713 also needed the Pepo Brahms fix below. Tests in
`tests/ability/Protection.test.ts`. The Rust engine does not admit 2255.

### A single-stat Protection refuses an opposing reduction of its stat - 1515574 (fixed)
Every stat Protection prints the same refusal: "The Damage of El Kaktus cannot be reduced by
an opposing character while the Protection ability is activated" (930; Power 940/4660, Attack
1136/1142/1340, the clan-gated `After [clan:25]: Protection : Damage` 5708/5757). Only the
Power And Damage forms had the guard; the rest resisted a Cancel only. 1515574 r3 is the first
capture of one meeting a reduction of its stat: El Kaktus (6/5) against Noemi, whose `-3 Opp
Power, Min 3` takes the Power to 3 while the Pussycats bonus `-2 Opp Damage, Min 1` leaves the
Damage on 5 (the engine had 3). 3 x 1 + 9 Life Left = 12, and Noemi's owner goes 15 -> 10. The
single-stat forms now set the guard on the stat they name, so an opposing reduction of it is
refused and an increase still lands; the Attack guard is a new bit in the card's packed state
and is checked beside the other two. The Power and Attack forms follow the printed text alone:
no captured round shows one against a reduction of its stat, and none shows the clan-gated
Damage one live against a reduction. Fixed 1515574. Test in `tests/ability/Protection.test.ts`.
The Rust engine has admitted the single-stat forms since semantic revision 62, holding the
stat as its Power And Damage mask does, but refuses construction wherever the opposing hand
could change the protected stat, because no round had shown that half; 1515574 r3 is the
first that does, for `Protection : Damage`, and would lift that refusal for the Damage form.

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

### `/ Life Lost` is Per Life Lost
Wachtmann level 3 prints `[clan...] +1 Dam./ Life Lost Max. 6` (5113). `Abilities.normalise`
read every `/` as `And`, so it compiled as a flat +1 Damage and a flat +1 Life, where the
server adds 1 per point of Life its owner has lost since the match began, capping the final
stat at 6. All four selected Wachtmann rounds agree: nothing at full Life (1090269 r0, 1
Damage), 1 + 4 at 8 Life (925818 r2), 1 + 5 at 7 (1092066 r3), and 1 + 7 clamped to 6 at 5
(924890 r3 - Max caps the final stat, not the addition). A `/` before `Life Lost` now reads
as `Per`, which the existing lost-Life multiplier handles; `Cancel Opp. Pow/dam Mod.`, the
only other slash in the card data, is unchanged. Fixed 1090269, 1092066, 924890 and 925818.
Test in `tests/ability/PerLifeLostSlash.test.ts`. The Rust engine has pinned the same rule
since semantic revision 55.

### A conditional Copy copies only when its condition holds
The Copy branch compiled the opposing text as a fresh ability without the copier's own
condition, so every Reprisal, Revenge, Asymmetry, Unison, `Bet > N` or clan-gated Copy adopted
unconditionally. Dr Van Wesel Ld's `Reprisal: Copy Opp. Bonus` copies nothing when its owner
moves first (1060341 r3: 8 x 9 = 72, not 84 with Sue's copied Support; 1091381 r3: Mou keeps
18 without the copied Equalizer), and Madlocks' `Bet > 3 Pillz: Copy: Opp. Ability` copies
nothing at `pillzUsed` 3 (943231 r1), while the moved-second Reprisal adoptions and the
`Bet > 3` adoption at 4 (878093 r0) still happen. The copied ability now carries the copier's
conditions ahead of its own. Fixed 1060341, 1091381 and 943231. Test in
`tests/ability/ConditionalCopy.test.ts`. The Rust engine has gated conditional Copy this way
since semantic revision 50.

### Naja Ld's Players Pillz and a latched Dope pay a knocked-out owner
The KO guard in `BasicModifier.canApply` stops an owner the round knocks out from gaining,
with identity-locked exceptions for the Riots bonus and Pr Hide. The server also pays Naja
Ld's own half of `Victory Or Defeat : +3 Players Pillz` (5511) through the knockout: 1024592
r2 (5 - 0 + 3 = 8 with two +3 post entries) and 1024732 r3 (0 - 0 + 3 = 3). It is now a third
exception, locked to the card, the Ability slot and the exact text; only level 1 is captured,
and levels 2 and 3 print identical text, so they share it by that text. A latched Dope pays a
knocked-out owner too: Talhia's `Dope 3, Max. 4` (924853 r3, 0 + 3) and Shao Xue's `Defeat:
Dope 1, Max. 13` (956902 r2, 6 + 1), so the Dope modifier now carries the same exemption.
Heal, Regen, Consume and Repair keep the guard. Fixed 1024592, 1024732, 924853 and 956902.
Tests in `tests/ability/PlayersPillzAfterKo.test.ts` and `tests/ability/DopeAfterKo.test.ts`.

### Recover pays on a win and rounds the placed pillz down
`RecoverModifier` never checked the outcome and paid ceil(bet x N / M). Plain `Recover N Pillz
Out Of M` pays only when its card wins: Costello's `Recover 1 Pillz Out Of 3` loses in 946810
r0 and its owner stays on 12 - 5 = 7, and `abilityData` gives `currentRoundRequirement: win`
for every plain form (3459, 3752, 4050, 4610, 5651); six winning rounds pay. `Defeat: Recover`
still pays on a loss. And the amount is max(1, floor((bet + 1 + 3 x Fury) x N / M)) - the
placed pillz including the free one, rounded down, as every Recover long description says:
Kyrioz Ld bets 7 and wins in 947010 r0, and recovers 2 (12 - 7 + 2 = 7), where the old rule
gave 3. The two formulas agree for every 1/2 and 2/3 ratio, so 947010 r0 is the only round
that separates them; a second winning 1/3 Recover would pin it. The uncaptured `Recover 1
Players Pillz Out Of 2` (Radden Cr) gets the same win gate by analogy, not evidence. Fixed
946810 and 947010. Tests in `tests/ability/Recover.test.ts`. The Rust engine has used both
rules since semantic revision 63.

### Cancel Opp. Modif. cancels only the cancelled card's own modifiers
`BasicModifier.canApply` refused an opposing Power, Damage, Attack or Life modifier whenever
the *target's* stat was cancelled, so a canceller's own reduction of the card it cancels never
landed. The server's text is "Any modifier of the opposing character affecting power will be
deactivated" (`captures/abilities.json` 1164; 1350, 4414, 1163 and 1172 say the same of
their stats, 1497 "any of your opponent's abilities or bonuses"), and three rounds agree:
- 1089974 r2: Dookor's `Cancel Opp. Power And Damage Modif.` with his own Dominion `Growth: -1
  Opp Power, Min 4` live in round three. Sue goes 6 - 3 to the Min, 4, and 4 x 1 + Support 12 =
  16; Sue's own `-1 Opp Power And Damage, Min 3` stays refused and Dookor fights 6/4, 6 x 3.
- 1506931 r3: Eyrton Cr's cancel and his own All Stars `-2 Opp Power, Min 1`. Hammer Cr goes
  6 - 2 = 4, 4 x 7 = 28, where the engine gave 6 and 42; Hammer Cr's identical bonus stays
  refused and Eyrton Cr keeps 8, 8 x 6 = 48.
- 1507792 r3: Dookor again, in round four. Angie copies his 6 and his Growth -4 stops at the
  Min: 4 x 4 = 16, where the engine gave 6.

Only the source's own flag decides now (`data.card`, whose stat an opposing Cancel switched
off), for every stat. The Power half is what the three rounds pin; Damage, Attack and Life
follow the same printed rule, and no captured round shows a canceller's own reduction of
those (the one Attack canceller with its own `-12 Opp Attack, Min 8`, Mimmo in 1506971 r0, has
its cancel stopped and faces an Attack already under the Min). Pillz read only the target's
flag, so it let a cancelled card's own opposing Pillz modifier through as well; it follows
the rule too, and no round shows either direction. Tune Out cancels both cards' Attack, so it
still turns every Attack modifier off. The Rust engine has cancelled by source since the
clan-bonus diagnostic (1089974 r2 is its own gate round for it). Fixed 1089974, 1506931 and
1507792. Tests in `tests/ability/CancelModif.test.ts`.

### Exchange and Impose write printed values before every increase
`ExchangeModifier` ran in PRE2 among the own increases and *set* `final` to the opposing
printed value, so it wiped every PRE2 increase registered before it: the owner's own bonus
always (bonus compiles before ability) and the opponent's own increase whenever the opponent
was internal P1. `Damage Impose` shares the modifier. Three rounds:
- 1059149 r1 (night): Calamity's `Power Exchange` against Tina, both printing 5. The server
  gives Tina 6 Power and 36 Attack - the swapped 5, her own Revenge +2, then Calamity's
  `Night: -1 Opp Pow. And Damage, Min 1`: 6 x 4 + 12. The engine gave 4 and 28.
- 1507008 r0 (day): Calamity's `Power Exchange` with her own GhosTown `Day: Power And Damage +
  1` live, against Tatane. The server gives Calamity 7 Power and 9 Attack - Tatane's 6 plus 1,
  then 7 x 3 - 12 under Tatane's `-12 Opp Attack, Min 5`. The engine gave 6 and 6.
- 874712 r1, filed until now as "Revenge / Damage Impose": Kochar imposes his printed 2 Damage
  on Tina and her own Revenge +2 lands on it, 4; the engine imposed after the +2 and gave 2.

The modifier now runs at PRE3 beside Copy, which writes printed values too, so before every
increase and reduction. All 39 captured Exchange and Impose rounds fit that order - the swap,
then own increases, then opposing reductions by descending Min - among them 1087884 r1 (Sue's
cut on Lagertha Cr's swapped 6), 1080007 r2 and 1091770 r0 (an opposing increase on a swapped
value), 1089262 r3 (an own increase and an opposing reduction on one swapped card) and 901292
r0 (an own reduction on an imposed 2). The Rust engine swaps in its Copy phase since semantic
revision 43 and imposes there since 66, so the two engines agree again. An opposing Cancel of
the stat (PRE4) still skips the swap, and every such write reads printed values, so two
Exchanges, or an Exchange against a Copy, cannot depend on their order. Fixed 1059149, 1507008
and 874712. Tests in `tests/ability/Exchange.test.ts`.

### An opposing resource Cancel deactivates permanents for its round - 1508676 (fixed)
A permanent's modifiers carry `always`, which returned before every other check, so no Cancel
could touch one. The server prints the opposite on Babe's `Cancel Opp. Pillz & Life Modif.`
(1655): "The effects of your opponent's poison, heal, regen, toxin, consume and dope
abilities will be deactivated for the round". 1508676 shows it on Combust, which that list
leaves out: Kontrø Ld's `Combust 1, Min 0` latches in r0 and takes 1 Life and 1 Pillz at the
end of r1 (10 - 2 - 1 = 7) and r3 (4 - 1 = 3), but not of r2, where Babe cancels (7 - 3 = 4,
and the Pillz stay on 2; the engine gave 3 and 1). Maelt Riv's latched `Consume 1, Min 2`
sits on its Min in r2 and shows nothing either way.

A permanent's Life or Pillz modifier now refuses to pay while its owner's current card has
that resource cancelled - the flag a `Cancel Opp. <resource> Modif.` sets, and `data.card`
from the latching round on. It is the only round in the corpus where a latched permanent meets
an opposing resource Cancel (an instrumented run found no other), so the printed text carries
the other families. A permanent still latches under a Cancel; no round shows whether the
server lets it. The Rust engine keeps refusing a resource canceller against a permanent.
Fixed 1508676. Test in `tests/ability/CancelPermanent.test.ts`.

## Previously triaged open rules

### End-of-round order: increases, then decreases by descending Min — 1514836, 1515298, 1515451 (fixed)
Until 2026-09-27 the engine ran the round-one second mover's (internal P2's) END effects before
P1's. That rule rested on two rounds, 1093173 r1 and 1496283 r2, and the second of them turned
out not to constrain the order at all: its Dark Kaizerin reduction never fires, because a hand
with two Oculus infiltrates nothing ("Two Oculus in one hand" below). Three autoplay rounds then contradicted the
rule outright, and every round in the corpus where the order changes a number fits one rule:
**every increase of both players first, then every decrease, the decreases by descending Min**
- the order the server already uses for the opposing combat-stat reductions (PRE1/POST2).

Replaying every round of the corpus on its own, with the server's totals restored before it,
under 24 candidate orders (by seat, by winner, by round mover, bonus or ability first, by Min,
increases or decreases first, latched effects first or last), plus two rounds from the running
autoplay batch, finds nine rounds where the order changes a number. This rule fits all nine,
under any of its tie-breaks, and no other candidate does:
- 1093173 r1: P2's Riots `+1 Pillz` before P1's Goose `-2 Opp. Pillz And Life, Min 5`:
  7 - 2 = 5, + 1 = 6, - 2 = 4, held at 5. The reduction first ends on 6.
- 1514836 r0: P1's DJ LBerto `Defeat: +2 Pillz Max. 10` before P2's Yomi Ld `-2 Opp. Pillz And
  Life, Min 1`: 12 - 3 (Fury) = 9, + 2 = 11, capped at 10, - 2 = 8. The old order gave 9.
- 1515298 r1: P1's Tortuga `Defeat: Recover 2 Pillz Out Of 3` before P2's Brampah Noel `-2 Opp
  Pillz. Min 3`: 7 - 3 = 4, + floor(4 x 2 / 3) = 6, - 2 = 4. The old order gave 4 -> 3, + 2 = 5.
  The Recover text says the Pillz come back "at the start of the next round", yet it goes before
  the reduction; it is an increase like any other here.
- 1515451 r2: P1's Ennio `Victory Or Defeat : +4 Life` and P1's latched `Growth: Regen 1, Max.
  17` (2 a round) before P2's latched Esther `Poison 1, Min 2`: 13 + 4 = 17, the Regen capped
  there, - 1 = 16. The old order (P2's Poison, then P1's) gave 12 + 4 + 1 = 17. A side's
  current-round effects still come before the permanents it has latched: the Regen first would
  give 15 + 4 - 1 = 18.
- 876752 r1 and 1514883 r2: two reductions of one Life from one side, the Berzerk bonus `-2 Opp.
  Life Min 2` before a `Min 0` ability (876752 r1: 5 -> 3, then Macey Rook's Brawl -4 -> 0; the
  ability first would give 1).
- 1515853 r0 and 1515873 r0 (autoplay run, in the main checkout at the time of writing): Hilly
  Billy's ability `-5 Opp. Life Min 4` before the same Berzerk bonus: 15 - 5 = 10, -> 5, -> 3.
  The bonus first gives 10 -> 8 -> 4, which the engine had. These two are what separates
  descending Min from the old bonus-before-ability order, which fits the other two.
- 1496283 r2 is the ninth only under the old infiltration, and fits once it is gone.

`Events.executeEnd` ran P2's increases, P2's latched increases, P1's increases and P1's latched
increases; then every current-round decrease of both sides by descending Min (ties: P2 first,
bonus before ability), then P2's and P1's latched decreases. The replay-per-round check did not
separate the remaining ties (which seat first within a pass, a latched decrease against a
current-round one), so they kept the old order. It is allocation-free, like `executeCancels`.
Fixed 1514836, 1515298 and 1515451; no other replay moved. Tests in
`tests/ability/EndOrder.test.ts`. Since the autoplay runs 2-3 the latched increases no longer
pay with the round's own: see "Latched permanents pay after the round's own effects" above.

In 1496283 the old infiltration also crashed the live advisor once: the engine left the
opponent 2 pillz instead of 3, and replaying their 3-pill bet in round four threw.
`buildPosition` now checks each resolved round against the server and carries on from the
server's totals, keeps the latest disagreement on screen, and returns "cannot replay" instead
of throwing (`tests/solver/Advisor.test.ts`).

The Rust engine was left unchanged. It refuses many matches where a cross-owner order could meet
a binding floor or cap on one resource (the "1093173/1 order rule" in `docs/rust-migration.md`),
because the server's order was unknown; the rounds above now pin it, which a Rust slice could
use to lift those refusals. Not every such match is refused, though: a latched Heal beside an
opposing current-round Life cut is admitted, and there the two engines disagreed until semantic
revision 80 (see "Latched permanents pay after the round's own effects" above).

### Same-family permanents replace, as the server text says — 1506438 (fixed)
The server prints "If two poisons or toxins are applied, the second will replace the first as
soon as the latter takes effect" on every Toxin, and the same note for two Heal or Regen, two
Dope and two Consume (Combust, Mindwipe and Repair print none; Consume 5275 carries the poison
note, apparently a copy error). Both engines used to stack instead: TypeScript paid every entry
in `Events.repeat`, and Rust every entry in `LatchedEffectsV1`.

1506438 (2026-09-27, Lab 3 test deck) settles it for two identical Poisons. The opposing Freaks
hand (three Freaks cards, bonus `Poison 2, Min 3`) won rounds 0 and 1, so two Poison 2 latches
sat on the owner, who went into round 2's end on 7 Life. Stacking takes 4 (7 to 3, still above
Min 3); the server took 2 (7 to 5), and round 3 posts a single permanent entry of 2 (5 to 3).
What was already pinned still holds: a delayed newcomer (Poison, Heal) lets the old latch pay
once more in its own latching round. 1131208 r2, 1092369 r2 and 926420 r3 each post the
permanent entries [2, 0] and the Life drops by the old latch's 2, and 1506438 r1 does the same
(12 - 3 - 2 = 7).

Both engines now replace. A newer latch of a family, aimed at the same player, replaces an
older one as soon as it pays: a delayed one from the round after it latches, an immediate one
(Toxin, Regen, Dope, Consume) in its own round.
- TypeScript: the text sets `Ability.family` (`LatchFamily`). Before paying an entry,
  `Events.executeRepeat` asks whether a newer entry of its family on the same player pays this
  round (`Ability.paysNow`); if so it marks the entry replaced (`won = false`) and skips it. The
  entry stays in `repeat`, so `Undo` restores its flag with the others and truncation still
  restores the rest, and the continuation cache key already carries the flag.
- Rust, semantic revision 78: `LatchedEffectV1::family`, and the pay loop skips an entry that
  `LatchedEffectsV1::replaced_this_round` reports. The list itself is unchanged, so the position
  needs no new state.

Across the replays a replacement fires in three rounds - 1506438 r2, 1092369 r3 (target knocked
out) and 1131208 r3 (target on its Min 3) - and changes a number only in 1506438. Fixed 1506438.
Tests in `tests/ability/LatchReplace.test.ts`, a replacing hand pair in
`tests/solver/PolicyCache.test.ts`, and in Rust
`a_second_poison_replaces_the_first_once_it_pays_as_in_capture_1506438` plus 1506438 as a
four-round gate fixture.

Still unobserved, and following the printed text:
- different magnitudes (the newer one's value pays);
- cross-kind replacement (Toxin over Poison, Regen over Heal);
- the immediate-newcomer case, where the old latch stops in the newcomer's own round;
- a weaker newcomer replacing a stronger latch;
- two latches from different owners on one player from the same round (1519318 settled the
  newer-round case: see "A latch from the other side replaces one of its family on the same
  player").

To see the first three, win with a Toxin card over a latched Freaks Poison while the target
stays above both Mins.

Before 1506438, a corpus-wide scan of every permanent post entry, adversarially re-checked,
found no round that told the two apart. Every round where one target held two latches of a
family had the second still in its delayed latching round, or had the target at or below Min or
knocked out, and a floored Poison posts no entry at all (1060510 r2, 963847 r2, 1059648 r3,
1087712 r3, 1089742 r3), so a single entry proved nothing.

For the solver, the Rust-eligible draws that can reach two live latches of one family in search
now follow replacement in both engines. There are ten Freaks decks carrying bonus 206 on two to
four cards (963847, 1023274, 1025181, 1025525, 1060510, 1073010, 1087712, 1089742, 1092369,
1131208), 1506438 itself, and two with Poisons of different sizes (1092294, 1092454). Rust's
construction refusals for a second latch beside a Unison, Growth, Killshot or clan-gated latch
of its family (`UnisonLatchAgainstSameFamilyLatch` and its siblings, revisions 73-76) are
unchanged. They cost no draw, and lifting them is a slice of its own. Unison Poison (`4033`)
stays closed; its only draw, 926226, is the replacement case.

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

### Single points waiting for a second capture
- 1079078 r3: Rajesh's `-2 Cards Damage, Min 4` puts its own half in the opposing-reduction
  phase, because an ability is queued as a whole at its first modifier's phase; the server
  resolves the owner's half with the owner's own modifiers, before the opposing reductions.
- 1089974 r2, fixed with 1506931 r3 and 1507792 r3: a canceller's own reduction of the card it
  cancels lands; see "Cancel Opp. Modif. cancels only the cancelled card's own modifiers" above.
- 947670 r3, fixed with 1496258 r0: `Perfect` was an unknown condition, met unconditionally,
  but the server's text is "If Akirale wins his round with the exact number of pillz needed
  (Perfect Pillz)" (3674, 4030, 4382, 5594; `currentRoundRequirement: "perfect"`). Akirale won
  with more than he needed in 947670 r3 and Tatiana won 5 to 2 on 5 pillz in 1496258 r0, and
  neither `Perfect: +2 Pillz` paid. `ConditionType.PERFECT` now holds when the card won and one
  pill fewer would not have: one Power less Attack, with the engine's own tie rule. Both
  observations are negative; no winning exact bet has been captured yet.
- 1025413 r1: Dark Kaizerin (Oculus) infiltrates GhosTown at night and fights with the night
  bonus `Night: -1 Opp Pow. And Damage, Min 1` (1442); the hand stores the host's day bonus
  before the game switches to night. It is the only such round.
- 1508932 r2, fixed with five Razor rounds: see "A capped increase is measured before its
  card's bonus" above.
- 1517236 r3: Slobodan Cr's `+1 Players Life` ("If Slobodan Cr wins the fight, the two competing
  players will receive 1 Life points at the end of the round", `abilityData` 2433) wins for 3
  Damage against a player on 2. The server ends that player on 1, not 0: the result block says
  `byKo: false` and `life: 1`, and both players get a `life +1` post entry. So the damage is
  floored at 0 and the opposing half of the gain then revives the knocked-out player; the engine
  guards an opposing Life modifier with `data.opp.life > 0` and ends the match on 0. It is the
  only captured round with a `Players Life` card in a round that knocks someone out (El Toucan,
  Ambazaak and El Cascabel's `Victory Or Defeat : +N Players Life` have eleven rounds between
  them, none a knockout). It sits beside Naja Ld's `Victory Or Defeat : +3 Players Pillz`, whose
  own half pays its knocked-out owner (1024592 r2, 1024732 r3, identity-locked in
  `Condition.compile`), so the likely rule is "a Players gain pays both players through the
  knockout" - but Kubra, Argos and El Gascaro show ordinary gains stopping at a knockout, and the
  Naja exception was only coded after its second round. A second needs a Players Life card
  (Slobodan Cr, El Toucan, Ambazaak, El Cascabel) in a round where either player ends on 0, or
  where its owner is knocked out while losing. The Rust engine refuses `2433`. 1519178 r3
  (autoplay runs 3-5) looked like it: Slobodan Cr wins for 5 Damage with Fury against a player
  on 3. But Beltran Cr's Roots bonus `Stop Opp. Ability` stops the gain, so neither player
  moves and the round says nothing.
- 1520327 r0: El Mariachi level 1's `Copy: Opp. Bonus` meets Anoda level 3, whose Oblivion bonus
  is `Copy: Opp. Ability`, so each card copies the other's copy. The resolution snapshot's static
  block rewrites both to `+1 Attack Per Life Left` (923, El Mariachi's own Huracan bonus) and
  reports El Mariachi at 42 (6 x 2 + 15 + 15) and Anoda at 33 (9 x 2 + 15). Yet Anoda wins the
  round: its 1 Damage and its `Toxin 1, Min 0` take El Mariachi's owner from 15 to 13, and the
  snapshot marks Anoda the winner. 42 cannot lose to 33, so the Attack the server shows for El
  Mariachi is not the one it fought with. The engine has El Mariachi at 27 (12 + its own +15;
  its copy finds Anoda's copy and adds nothing to its Attack) and Anoda at 33, and with the
  record's 42 read as 27 the whole game replays exactly. It is not an extractor artifact: 42 is
  what the live snapshot sends. The only other round where two copiers meet, 1519271 r2 (Jiya's
  Oblivion Copy against Cravy's `Revenge: Copy Opp. Bonus`), replays either way. A second needs
  a `Copy: Opp. Bonus` card (El Mariachi or any of the other cards printing it) played into an
  Oblivion card, to see whether the displayed Attack and the winner disagree again.

### An increase to the opposing card - 1414749, 1507713 (fixed)
Pepo Brahms' `Growth: Opp. Attack +1` (level 4, `abilityData` 5210) and `Growth: Opp. Attack
+2` (level 5, 5214) raise the *opposing* card's Attack by N x the round number (sideAffected
"opponent", increase). Normalised, both read `Opp +1 Attack`, which neither numeric branch of
`compileAbility` accepts, so they compiled to nothing. 1414749 r2 was a single point until
1507713 r3 brought the second:
- 1414749 r2: Schredder (Fury, two pillz) fights at 6 x 3 + 1 x 3 = 21 in round three, where
  the engine had 18.
- 1507713 r3: Filomena on nine pillz fights at 4 x 10 + 2 x 4 = 48 in round four, where it had
  40. Pepo is 7 x 2 = 14, cut to 8 by Filomena's Montana `-12 Opp Attack, Min 8`.

`Abilities.normalise` now puts the sign in front, `+1 Opp Attack`, which is the shape `+1 Opp
Life` already has, so it compiles as an opposing Attack modifier scaled by Growth. It runs with
the opposing Attack reductions (POST2, after the target's own Attack modifiers). Neither round
has another Attack modifier on the target, so the order against one is unobserved. Pepo Brahms
is the only card that prints this shape. Fixed 1414749 and 1507713. Tests in
`tests/ability/OppIncrease.test.ts`. The Rust engine still refuses 5210 and 5214.

### Two Oculus in one hand infiltrate nothing - 1496283 (fixed)
The rules text quoted in `Hand.from` says a hand holding more than one Oculus gets no
Infiltrated effect, and the Rust catalog (`derive_effective_catalog_hand`) infiltrates only
when exactly one Oculus is in the hand. The TypeScript scan `break`s at the first Oculus, so
its `if (oculus !== undefined) return hand` guard could never fire: with two Oculus the first
one still infiltrated, counting only the non-Oculus cards. This entry used to say no captured
hand held two Oculus; 1496283 (2026-09-26, Training) does, Dark Kaizerin and Dark Morphun
beside two GhosTown, and it settles the text:
- the server sends no bonus on either Oculus card, where a lone Oculus carries the clan bonus
  it infiltrated (1025413 sends Dark Kaizerin the GhosTown `Night: -1 Opp Pow. And Damage,
  Min 1`);
- in round two Dark Kaizerin wins with `[clan:52]... -2 Opp Pillz. Min 2`, which "only
  activates if Dark Kaizerin infiltrates GhosTown, Komboka, La Junta, Oblivion or Pussycats",
  and Naele's owner goes from 2 to 3 on the Vortex `Defeat: Recover 2 Pillz Out Of 3` alone.

The engine had been infiltrating Dark Kaizerin into GhosTown and reaching the 3 only through
the end-of-round order it then adopted (reduction first, held at Min 2, then the recovery); see
"End-of-round order" above. The scan now runs to the end, so a second Oculus leaves both
uninfiltrated. The replays are unchanged, 471 exact before and after: 1496283 replays either
way. Test in `tests/ability/Oculus.test.ts`, with a one-Oculus control that does infiltrate.

### Counter-attack only decides round one - settled 2026-09-26
The engine kept the owner of a Counter-attack Leader (Ashigaru) second in every round. The
server's own text is narrower - "The player who has Ashigaru on his team always plays the
first round in second. If both players have Ashigaru in their team, the order of play is
decided in the usual way" (`captures/abilities.json` 124, `specialAction: "strike_back"`) - and
Training capture 1495980 agrees: Ashigaru was the owner's only Leader, the opponent moved
first in round one and the order then alternated (opponent, owner, opponent, owner). The old
table replayed round two with the wrong mover and failed, and the live advisor, which checks
the engine's mover against the server's, lost track of that game (the owner saw it miss the
end of round three). `createBaseGameCache` now always
alternates: who moves first in round one is settled by the server before any move, and the
testcase's first mover already carries it. That also retires the two-Leader worry recorded
here (the turn order no longer depends on Leaders at all, so the constructor and `Game.from`
cannot disagree about it). 1495879 held Ashigaru with Administrator, whose shared `Cancel
Leader` bonus reads "Your Leader Abilities are deactivated if you have more than one Leader in
your team" (117); the owner moved first there, which fits either reading of one round. That
bonus is live whenever two Leaders share a hand (1495879, 1496119, 1496142 show it on both), and
the Rust catalog, which never derives a Leader bonus, now counts those slots in its corpus test
instead of expecting none.

### Solomon's Tie-break wins a tied round - 1506259 (fixed)
1506259 r3 (Training, Lab 3 test deck): the owner's Hammer Cr level 5, Fury on no pillz, meets
the opposing Solomon level 5 on no pillz; both Attacks are 6. The engine gave the tie to Hammer
Cr (equal stars, and Hammer Cr moved first), the server to Solomon, whose "Tie-break" reads
"During the entire fight, if your attack is equal to your opponent's, you will always win the
round. If your and your opponent's Life points are equal at the end of the fight, you will win
the fight. (These effects are cancelled out if your opponent also has Solomon)"
(`captures/abilities.json` 1135). The engine never read the ability; the live advisor showed
100% for that round. `createBaseGameCache` now records a lone Solomon's side as the match's
`tieBreaker` (none when both sides have one, or when Cancel Leader deactivates it), and
`CardBattle` gives that side every tied round. The second clause, a level match at the end
going to Solomon's owner, is coded from the text alone: no capture has ended level with a
Solomon in play. The Rust catalog still refuses every Leader.

### Second rounds from the 2026-09-27 test decks (fixed)
All five rounds are fixed above. The Exchange round (1507008 r0) and the two Cancel rounds
(1506931 r3, 1507792 r3) settled their rules; 1507713 r1 and 1507819 r2, Khrull Cr's
`Protection: Cards Power And Damage` against its own Dominion Growth, are **Protection: Cards
Power And Damage guards both cards**, and 1507713 r3 turned out to be the second round for
**An increase to the opposing card** (1414749).

## Fresh capture backlog

Nothing is untriaged. The four remaining mismatches are single points waiting on a second
capture: 1079078, 1025413, 1517236 and 1520327. Autoplay runs 3-5 (188 Training battles,
2026-09-27; 1518086 stopped in its fourth round when the game tab reloaded) brought nine
mismatches, and they came down to four rules and a single point: `+N Attack Per Opp. Damage`
reading the modified opposing Damage (1518052, 1518765, 1519829, which also settled the single
point 1515692), a stat Protection shielding its own modifiers from an opposing Cancel (1519871,
1520579, 1521010), `Disunion:` met unconditionally (1519333) and a latch from the other side not
replacing one of its family on the same player (1519318), all fixed above, and El Mariachi's
two-way Copy loop (1520327), filed with the single points. None was a capture artifact: each
diff is in the live resolution snapshot, and 1520327's is the server's displayed Attack
disagreeing with its own winner. None of the new captures is a second point for the other
three: no other Oculus joins GhosTown at night (1025413), the new `Cards` reductions never meet
an opposing reduction of the same stat under a binding Min (1079078), and 1519178 r3's
`Players Life` is stopped (1517236). Autoplay runs 2-3 (95 Training battles,
2026-09-27, none stopped before its first round resolved) brought eleven mismatches, and they
came down to six rules: `Stop:` permanents latching unstopped (1516740, 1517397, 1517419), a
prefixed Impose that compiled to nothing (1517397 again), a capped increase measured after its
card's bonus (Razor in 1516811, 1516832, 1516846, 1516906 and 1517271, which also settled the
single point 1508932), latched permanents paying before the round's own effects (1517029),
Versus reading an infiltrated Oculus by its host clan (1517121), all fixed above, and
Slobodan Cr's `+1 Players Life` reviving a knocked-out player (1517236), filed with the single
points. None was a capture artifact: each diff is in the live resolution snapshot, and 1517236's
Life is the result block's. The first autoplay run (53 Training
battles, four of them stopped before their first round resolved) brought six mismatches: a
recap snapshot (1514649), three rounds of the end-of-round order (1514836, 1515298, 1515451),
a single-stat Protection (1515574), all fixed above, and 1515692, filed with the single points.
The 2026-09-27 test-deck captures settled 1059149 (Exchange, with 874712's Damage Impose
beside it), 1089974 (Cancel), 1506438 (same-family Poison replaces), 1414749 (an increase to
the opposing card) and the Protection: Cards pair 1507713 and 1507819, all fixed above; the
2026-09-26 Training captures settled 947670 (Perfect). The backlog of 39 that the expanded
corpus brought on 2026-09-14 and 2026-09-17, and the three from the 2026-09-23 session, are
fixed above or among those four. A fourth 2026-09-23 capture, 1414087, deals card 2714
(Gloria, level 2, `Brawl: Damage + 1`), which the 2026-09-10 character dump predated; since
the 2026-09-26 card refresh it has a testcase and replays exactly.

That refresh also rebalanced cards the corpus had already captured, which is a new kind of
replay failure: 1080464 broke because Trasher-X level 4 fought it with `Revenge: -16 Opp
Attack, Min 3` and the refreshed catalog prints -13. `deno task extract` now writes a card's
battle-start ability text into the testcase's `abilities` slot whenever it differs from the
catalog (the slot Hazard already used), which put 1080464 back and gave 1090887 its
Chelonite `Unison: Killshot: +4 Life`. Only those two testcases changed. A power or damage
rebalance of a captured card would need a stats slot too; none has happened yet.

The five that arrived with the 2026-09-17 captures are no longer here: 1130425, 1130726,
1131144, 1207064 and 1093129 are all fixed above, and so is 901004, which had been filed
under Damage Exchange since the first triage pass.

## Legacy tests
Both repaired on 2026-09-25 with no engine change; the full suite now fails only on the
open replay mismatches above.
- `tests/ability/Oculus.test.ts` "Infiltrated": the expectation was stale, not the engine.
  Alekperov (lv3, 6/5) joins three Ulu Watu, which his clan list names, so his "-2 Opp
  Power, Min 5" takes Tamakuchi (lv4) from 7 to 5; the infiltrated "Power +2" falls to
  Tamakuchi's Nightmare "Stop Opp. Bonus". 6 x 1 against 5 x 1 + 3 Growth = 8, so p1 takes
  5 (life 7) rather than p2 taking 4. The old number needed the infiltrated bonus to
  survive Stop Opp. Bonus (an 8-8 tie that Alekperov wins on fewer stars) and used the
  Dec 2024 row (6/4, "Min 1"). The captures show the engine's rules: 1131463 r0 and
  1131373 r2 (Kusm joins a listed clan, its -12 Opp Attack lands, and the Piranas Stop
  Opp. Bonus cancels the bonus it infiltrated), 1090269 r0 (an Oculus joining three Ulu
  Watu gets Power +2), and 1022847 r2 (Alekperov joins Tolvack, which is not in his list,
  so his ability does nothing). The test now also asserts both cards' Power/Damage/Attack,
  so the ability is visible even though it does not change the winner.
- `tests/Game_2.test.ts` "Protection" was a copy of Game_1 with its card names blanked
  and only the first two rounds' move comments rewritten (Madabook against Lianah Ld,
  Dave against Cybil). The rest was left over: round 3 reused a slot already played, and
  the pillz assertions went back up. The name never meant anything either, since Game_1's
  "Protection" has no Protection card in it. It is rebuilt as a whole Ulu Watu against
  Nightmare game on those cards with pinned levels, renamed for what it checks, and
  every rule it relies on is in the captures: Stop Opp. Bonus against Power +2
  (878056, 878120), Stop Opp. Ability stopping Heal from latching (877733 against 878093),
  Copy: Power And Damage Opp. (1069345 r0), Damage Exchange (1065557 r1, 901004 r0) and
  Defeat: +2 Life (878120 r1, 1091770 r1).

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
