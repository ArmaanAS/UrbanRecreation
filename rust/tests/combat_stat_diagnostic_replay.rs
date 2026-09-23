use std::collections::BTreeSet;
use std::fs::File;
use std::path::PathBuf;

use urban_recreation_rust::catalog::{CardCatalog, CardKey};
use urban_recreation_rust::effect_registry::{
    AttributeAffectedV1, EffectRegistryV1, MagnitudeMultiplierV1, SupportedEffectV1,
};
use urban_recreation_rust::engine::{
    BaseRulesRoundInput, BaseRulesSelection, ByPlayer, CombatStatDiagnosticErrorV1,
    CombatStatEffectSourceV1, CombatStatPredicateV1, CombatStatSourcePlanV1, PlayerId,
};
use urban_recreation_rust::replay::{
    adapt_capture, CapturedGame, CombatStatDiagnosticPreparationErrorV1,
    CombatStatDiagnosticProjectionV1, CombatStatDiagnosticReplayErrorV1,
    CombatStatDiagnosticReplayV1, CombatStatDisabledReasonV1, CombatStatProjectionDispositionV1,
    CombatStatWholeHandHazardSourceV1, EnginePlayer, ReplayCaseV1, ReplayClassification,
    SourceModifier, COMBAT_STAT_DIAGNOSTIC_COMPILER_POLICY_SEMANTIC_REVISION_V1,
};

mod support;

const COMBAT_STAT_PREFIX_FIXTURES: &[(u64, usize)] = &[
    (875032, 2),
    (875155, 1),
    // Four rounds since revision 46, whose Unison reduction pays in round 3.
    (1088323, 4),
    (1089001, 1),
    (1024673, 2),
    (1060199, 3),
    (1081463, 4),
    (1069193, 1),
    (1089513, 2),
    // Three rounds since revision 49, whose round 2 selects Wilkinson's Pillz & Life cancel
    // against a hand with nothing for it to cancel.
    (901400, 3),
    (874837, 2),
    (1011643, 2),
    (1011768, 1),
    (1011483, 2),
    (877812, 3),
    (874642, 1),
    (1059269, 1),
    (1091585, 1),
    // Four rounds since revision 44: Figaro's `Day:` ability takes Aurora from 7/5 to 6/4 in
    // round 2.
    (868094, 4),
    (875230, 1),
    (877950, 1),
    (945585, 2),
    (1023396, 2),
    // Four rounds since revision 45: Sir Taco's `Per Life Left` Power clamps at its Max.
    (874962, 4),
    // Four rounds since revision 45, whose Huracan `+1 Attack Per Life Left` opens round 2.
    (946400, 4),
    (1058366, 3),
    (1061897, 4),
    (946288, 3),
    (1092660, 1),
    (1093500, 2),
    (1092909, 2),
    (925719, 3),
    (877636, 4),
    (943111, 1),
    (924740, 2),
    (963694, 1),
    (970972, 1),
    (1059454, 4),
    (1069813, 4),
    (1078906, 3),
    (1089346, 4),
    (1090607, 4),
    // Revision 20 unconditional Victory opponent-Life. Mou wins with the complete -5 in
    // 1091473/0 and with the Min-5 clamp in 926367/1; Berzerk wins in 877093/0 and
    // 1080662/1, while 1025102 keeps three consecutive Berzerk defeats that must pay
    // nothing at all.
    (926367, 3),
    (1091473, 2),
    // Four rounds since revision 45, whose Ametia `Per Life Left` Power opens round 1.
    (877093, 4),
    // Four rounds since revision 47, whose Asymmetry Stop is predicate-false in round 2.
    (1080662, 4),
    (1025102, 3),
    // Revision 22 conditional Victory opponent-Life. Both players picked hand slot 1, so
    // Doela Noel's Symmetry reduction is live: Aneta's Courage leaves her at 6 power for
    // attack 42, her two damage takes P2's 12 Life to 10, and the complete -4 makes 6.
    // Only this one round is a valid prefix - round 1 then selects Pride's deferred
    // `Protection: Attack`.
    (1011297, 1),
    // Revision 23 Protection. Stat protection refuses an opposing reduction: Nebula keeps
    // 7 Power against Olga Cr's "-2 Opp Power, Min 5" in 949439/0, keeps 4 Damage against
    // Donald's "-3 Opp Damage, Min 2" in 924320/1 and against Henry's Support reduction in
    // 942983/2, and Miss Pandora keeps 7/4 against Sue's "-1 Opp Power And Damage, Min 3"
    // in 1069506/0. Source protection survives a Stop: in 926525/0 Lumia Cr's Stop Opp.
    // Ability does not stop Andy Ld, whose "-20 Opp Attack, Min 5" takes Lumia Cr's own 36
    // to the reported 16, while the Skeelz bonus carrying the Protection is untouched.
    // 1091235/0 is the Bikini Joe Ld mirror for Protection: Bonus against Stop Opp. Bonus.
    (949439, 1),
    (924320, 3),
    (942983, 3),
    (1069506, 3),
    (926525, 3),
    (1091235, 3),
    // The three remaining draws the same revision makes catalog-eligible. 1070207/0 is the
    // only captured Buckler round and nothing there reduces it, so it pins that admitting
    // Protection changes nothing when no opposing reduction exists.
    (877687, 4),
    (924257, 4),
    (1070207, 1),
    // Revision 24 stat Copy. 1025031/0 pins both halves of the rule at once: Natasha copies
    // Nantosuelte's printed 4 Damage rather than the 7 its Asymmetry bonus had made of it,
    // and her own Damage +2 then produces the reported 6. 1065812/1 copies into an opposing
    // reduction - Joana takes Sue's printed 6 Power and Sue's own -1 leaves the reported 5 -
    // and 1069345/0 does the same for the Power And Damage pair. 876635/1 is the plain case,
    // Javert at Keya's printed 8 Power.
    (876635, 2),
    (1025031, 3),
    (1065812, 4),
    (1069345, 1),
    // The two remaining draws the same revision makes catalog-eligible. Neither plays a Copy
    // grammar in these rounds: every selected Asymmetry Copy in the corpus reaches the
    // capture already rewritten to the source it adopted, so admitting it moves catalog
    // eligibility without moving any replayed round.
    (946112, 2),
    (947228, 3),
    // Revision 25 Attack per opposing Damage. Kalija's +2 is worth 6 against a 3-Damage
    // card in 1090691/0 and 12 against a 6-Damage one in 1092066/2; Taka's +3 reaches 23
    // from 14 in 1090418/1, Sunscale's 16 from 7 in 1079482/1, and HU0-M31's 58 from 49 in
    // 1130833/2. 1066077/1 is the negative case: Spidee's Reprisal Stop Opp. Ability
    // leaves Adytia Ld at a plain 8 x 4. 925899/0 has the conversion and an opposing
    // Equalizer in the same round, 40 + 4 - 12 = 32.
    (925899, 2),
    (1066077, 2),
    (1079482, 3),
    (1090418, 2),
    (1090691, 2),
    (1092066, 3),
    (1130833, 4),
    (949750, 4),
    // Revision 25 Defeat opponent-Life. Waller pays 2 after losing in 925796/0, D. Carver
    // in 925999/0 and C Wing in 945989/1; 874520/0 is the same card winning instead, which
    // pins the trigger rather than the magnitude.
    (874520, 4),
    (925796, 2),
    (925999, 1),
    (945989, 2),
    (1060052, 4),
    (1072715, 3),
    // Revision 26 permanent Life: Lianah Ld's `Heal 1 Max. 20` is admitted through the
    // new latch channel. In every one of these complete draws she loses the round she is
    // played (877773/0, 877860/1, 878056/2; 1079482/0 above), so the gate pins that a
    // losing Heal never latches and that the rest of each draw plays out unchanged. The
    // positive branch is pinned by the engine tests and by the capture evidence below.
    (877773, 4),
    (877860, 4),
    (878056, 4),
    // Revision 27 widens the latch to the plain `Heal N Max. M` grammar. 1080877 is the
    // paying draw: Campbell's `Heal 1 Max. 15` latches in round 0 and pays one Life after
    // each of rounds 1, 2 and 3 - beside Scott Ld's VOD in round 1 and in the round 2 where
    // Spidee's Reprisal SOA stops Lobo's Reanimate but not the already-latched Heal.
    // 1059895 does the same for `4625` after Campbell wins a 61-61 tie on level: round 1
    // pays beside a VOD, round 2 beside Cleo's Defeat Life, round 3 after a loss (Lennard's
    // Killshot stays visible-but-disabled there and did not trigger: 24 against 19). 924669/0
    // is a latch round paying nothing (Pere Barali wins into Uuber's VOD reduction), and
    // 875375/0 and 1025525/0 are losing Heals that never latch.
    (1080877, 4),
    (1059895, 4),
    (924669, 1),
    (875375, 1),
    (1025525, 2),
    // Revision 28 Toxin and Poison on the same latch. Toxin pays in its latching round:
    // Zis takes AI-Lycs' owner from 12 to 8 with 3 Damage and 1 Toxin in 963039/0, Galactea
    // 12 to 10 with 1 and 1 in 1091904/1, Dr Elisa 12 to 9 in 1090531/0. It keeps paying after its card is gone (963039/1, 926226/1, 1090531/1) and
    // in the round its owner is knocked out (1091585/2). Poison waits a round: the Freaks
    // bonus latched by Sofilia in 1092369/1 pays 2 in round 2 and reports 0 in round 3 with
    // its target already at zero, and Araaknat's ability latched in 1092294/2 pays 2 in
    // round 3 while Agnes knocks its owner out.
    (963039, 3),
    (926226, 2),
    // Three rounds since revision 46: Tina 5 + 2 - 3 = 3 under the Unison reduction.
    (1090531, 3),
    (1091585, 3),
    (1091904, 2),
    (1092369, 4),
    (1092294, 4),
    // Revision 29 plain `+N Pillz`. The winner's own Pillz rise after the bet is paid:
    // Archimedes' `1150` takes 12 - 7 + 2 to 7 in 1092141/0 (Petra's Stop Opp. Bonus
    // silences the Riots VOD but not the ability), 12 - 5 + 2 to 9 in 1092201/0, 12 - 9 + 2
    // + 1 to 6 in 1060341/0, 12 - 1 + 2 to 13 in 1093275/0, and pays beside the VOD in
    // 1092578/0, 1092773/0 and 1092840/0; Corvus Cr's `503` +3 reaches 14 in 1092201/1 while
    // Argos' capped Defeat gain lands on the other side; Mercury's `1054` pays in 1090887/0
    // and Grudj Cr's `455` in 1060510/0 while latching the Freaks Poison. Nothing is paid on
    // a loss: Archimedes in 1092992/0 and 1092992/1 (a rule-3 draw, once per side), Zaveli's
    // `5258` in 1060510/1, Joy's `1229` in 1089626/0, and Grudj Cr in 1025525/1 where the
    // owner is knocked out at 0 Pillz. 1092066/0 above is the stopped case: Markus' Roots
    // bonus stops the ability and only the VOD pays. 1092454 stops at two rounds because
    // round 2 selects a dynamic `Copy: Opp. Ability`, which replay keeps fail-closed, so
    // the round where Archimedes pays into a KO (1092454/3, 7 - 5 + 2 + 1 = 5) is pinned by
    // the engine test instead.
    (1092454, 2),
    (1092992, 4),
    (1092141, 2),
    (1092201, 3),
    (1060341, 1),
    (1060510, 3),
    (1089626, 1),
    (1090887, 3),
    (1092578, 1),
    // Four rounds since revision 46, whose Unison Damage +3 pays in round 1.
    (1092773, 4),
    (1092840, 3),
    (1093275, 1),
    // Revision 30 plain `-N Opp Pillz. Min M`. Dalhia Cr's `339` takes Callie from 12 - 5
    // to exactly the Min of 4 in 1131294/0 (a rule-3 draw), and in 1091644/1 finds AI-Lycs
    // already on the Min after his Defeat recovery (6 - 6, recover 4) and changes nothing.
    // A loss takes nothing: Yomi Ld in 924413/0, Gil Cr in 956608/0 (rule 2), Baldovino in
    // 1087712/0, Hawkins Cr in 1131294/1 and Andsom in 946288/2 while his target is
    // knocked out at 0 Pillz. Thorpah Cr's losing `854` in 1023396/0 is already above.
    // Three rounds since revision 47: the Confidence Stop in round 2 stops a payload-free
    // opposing Stop, so it pins liveness only.
    (1091644, 3),
    (1131294, 3),
    (924413, 4),
    (956608, 4),
    (1087712, 2),
    // Revision 31 `+1 Pillz Per Damage`. The unconditional form pays the winner its final
    // Damage, Fury included: Spade's `1090` takes 12 - 10 + 5 to 7 in 1024592/0 and 12 - 11
    // + 5 to 6 in 1024732/0. Ramak's `Symmetry:` form pays only when both selected cards sit
    // in the same hand slot: 12 - 5 + 4 to 11 in 1023274/1 and 12 - 3 + 4 to 13 in 1058151/0
    // (both slot 3 against slot 3), and nothing in 1011183/3 where the slots differ (and
    // Impudicus' Stop Opp. Ability bonus would have stopped it anyway), as in 1023396/1 and
    // 1025102/1 above. A loss pays nothing: Grace's `1051` in 1089933/0, Sah Brinak Cr's
    // `809` in 1023396/1, Ramak in 1011643/1 and 1025031/2.
    (1011183, 4),
    (1023274, 3),
    (1024592, 2),
    (1024732, 1),
    (1058151, 1),
    (1089933, 2),
    // Revision 32 `+N Life Per Damage`. Nyema's `492` pays her final 3 Damage beside the
    // Jungo Victory Life in 1089121/2 (9 + 3 + 2 = 14), and Jautya's `4500` takes 7 to 10 in
    // 1092141/1, the round that had kept that draw to a one-round prefix. Kenny Cr's `+2`
    // form pays nothing on a loss in 1093399/2 (a rule-3 draw). The Revenge and Confidence
    // forms and the predicate-carrying permanents have no reachable server round - 877476,
    // 1025279, 924853 and 1130454 all open on an unadmitted source - so their conditions
    // are pinned by the engine tests.
    (1089121, 4),
    (1093399, 4),
    // Revision 33 generalises the opponent-Life reduction on the Victory and Victory-or-
    // Defeat channels. Glenn's `512` takes Uuber's owner from 12 - 6 to exactly its Min of 3
    // in 962243/0, where Uuber's own Victory-or-Defeat `1628` pays on the losing side, and
    // Kazayan's `769` pays into a knockout in 962243/2. Regan's Victory-or-Defeat `1726`
    // pays after losing 963039/2, before the Toxin its owner latched in 963039/0, which is
    // what takes that target to 0 rather than to the Min of 1. A loss pays nothing: Dao
    // Wang's `935` and Zinfrid's `594` in 926367, Surstorming's `4948` in 1092840/0 and
    // Dregn Cr's `602` in 1131010/0. A stopped source pays nothing either: Mavi's Stop Opp.
    // Ability silences Glenn's `512` in 876712/0 and only his 6 Damage lands.
    (962243, 3),
    (1131010, 4),
    (876712, 1),
    // Revision 34 puts a cap on the Life conversion and a predicate on fixed Victory Life.
    // C Dusk's `1146` never carries its owner past 8: a Fury-inclusive 6 Damage takes 5 to
    // exactly 8 in 1130609/3, and a plain 4 takes 7 to 8 in 1131010/2, which is why that
    // fixture now runs in full. Impudicus' `2638` pays its 3 in 1010898/0, where his Roots
    // Stop Opp. Ability leaves Aneta's Courage inert and the two selected slots differ, and
    // pays nothing in 1011183/3 above, where he loses with the slots differing. La
    // Salerosa's `1161` and the `Confidence :` form have no reachable server round - 877167
    // opens on `Victory Or Defeat : +3 Players Life` and every Barcius draw carries the
    // Cosmohnuts `Tune Out` bonus - so their arithmetic is pinned by the engine tests.
    (1010898, 4),
    (1130609, 4),
    // Revision 35 admits Xantiax, the one grammar that charges both players and reads no
    // outcome. Xantiax Robb Cr's `-3 Life, Min. 0` takes 6 to 3 on its winning owner and,
    // with the Berzerk `680` behind it, 17 - 1 - 3 - 2 to 11 on the loser in 1080464/2; it
    // takes its losing owner's side from 12 - 3 to 6 and the winner's from 11 - 2 (Cyb
    // Lhia's latched Poison) + 3 (Anita's Courage conversion) to 9 in 1059648/1; and in
    // 1058151/3 it still charges the opponent 5 to 2 from an owner the round's 7 Damage has
    // already floored at zero, which is the Min 0 clamp on both sides at once.
    (1058151, 4),
    (1059648, 4),
    (1080464, 3),
    // Revision 36 puts the `Confidence:` previous-round predicate on the plain `+N Pillz`
    // grammar. Balixto's `1702` pays 7 - 5 + 4 = 6 in 924615/2, behind a round his side won,
    // and that is the corpus's only paying round for this form: the three others it appears
    // in are all unreachable here. 1092515/2 (a loss, where the Vortex `577` recovery pays
    // instead) already mismatches in its round 0, 1073010/1 (his side did win the round
    // before, but Spidee's Reprisal `Stop Opp. Ability` silences him) selected a deferred
    // `Brawl:` source in its round 0 until semantic revision 40 admitted the combat-stat
    // Brawl grammars - that draw is eligible now and is a candidate fixture, though it is
    // not one yet - and 925781/1 (Monkovski's `4449`, a loss with no
    // prior win) sits behind the Cosmohnuts `Tune Out` bonus. The engine tests pin the
    // negative arms instead. 924615 itself stops at three rounds because its capture could
    // not attribute the closing `battles.result` to a side, so the last round's life is the
    // stale pre-damage snapshot rather than a server fact.
    (924615, 3),
    // Revision 37 admits Hattori's `Courage: -4 Opp. Dmg, Min 2` - the same opponent-Damage
    // grammar under the position predicate the compiler already had, blocked only by the
    // abbreviated printed spelling - and the losing-side `Defeat: -N Opp. Pillz, Min M`.
    // 1078999/2 pins both halves of the Courage floor in one round: Hattori moves first and
    // loses, so Lothar's printed 5 Damage resolves as max(5 - 4, 2) = 2 and the life ledger
    // agrees at 7 to 5, while Lothar's own `-3 Opp Power, Min 4` independently takes
    // Hattori's 8 Power to 5. 1092515/0 pins the Pillz reduction away from the floor -
    // Boomstock Cr's 12 - 5 + 1 (its own Victory Or Defeat bonus) - 2 = 6 - and 1092201/2
    // repeats it at 11 - 0 + 1 - 2 = 10. 1088480/2 is the floor case the corpus also holds,
    // where the target's own bet has already taken it to 0, so it distinguishes nothing.
    (1078999, 3),
    (1092515, 1),
    // Revision 42 admits the post-round `Brawl:` grammars: a won round pays the printed
    // amount once per distinct character in the opposing hand sharing the opposing selected
    // card's clan, onto the opposing Life, the opposing Pillz or the owner's own Pillz. Every
    // count in the corpus is 4. Fomalhaut Ld's `2893` takes 12 - 4 Damage - 4 to 4 in
    // 1093451/1; Eeok Ld's `5650` takes 12 - 1 - 4 to 7 in 1058545/0 with its Min 3 not
    // binding; Macey Rook's `4380` meets the Min 0 floor in 964213/3 (2 - 1 = 1, the Berzerk
    // bonus leaves it alone at its Min 2, Brawl takes it to 0). Sirrena's capped `5822` and
    // `5844` take 12 - 7 + 4 to exactly the Max of 9 in 1066739/0, 1091848/0 and 1092020/0,
    // so the cap is reached but never binds below the uncapped sum. A loss pays nothing:
    // Macey Rook in 1024524/2 and 1088716/2, Fomalhaut Ld in 1079263/0, and Newell's `5172`
    // in 1078669/1, its only selection, so the opposing Pillz form has no paying round and
    // rests on the revision-30 reduction arm it binds to.
    (1093451, 3),
    (1058545, 2),
    (964213, 4),
    (1066739, 1),
    // Four rounds since revision 43, whose Power Exchange opens its last round.
    (1091848, 4),
    (1092020, 4),
    (1024524, 4),
    (1088716, 4),
    (1079263, 4),
    (1078669, 2),
    // Revision 43 admits the unconditional Exchanges: the two selected cards swap their
    // printed values of the stat before any own increase or opposing reduction. Clean swaps:
    // Lagertha Cr 5 against Uuber's 7 in 867116/0, Djet 4 against Aurora's 7 in 1088919/0
    // and against Mou's 6 in 1091703/3 (and in 1091848/3 behind Sirrena's Brawl round).
    // 1087884/1 pins the order against a reduction - Sue's `-1 Opp Power And Damage, Min 3`
    // takes her swapped 6 to 5 - and 1080007/2 an opposing increase landing on the swapped
    // value (Tina's Revenge +2 on Marlowe's 5). 1066210/0 is the stopped case: Spidee's
    // Reprisal `Stop Opp. Ability` leaves Joan Cena's 5 and Spidee's 6 where they were.
    // 901004/0 is Waldegrin Cr's `Damage Exchange` taking Kubrat Cr's 1 and giving up his 8,
    // which the life ledger shows although the capture's `damageAfter` reports the swap.
    (867116, 2),
    (1088919, 4),
    (1091703, 4),
    (1087884, 3),
    (1080007, 4),
    (1066210, 2),
    (901004, 2),
    // Revision 44 admits the `Night:` and `Day:` forms of the plain numeric grammar under a
    // match-constant predicate. Figaro's night ability takes itself from 7/4 to 8/5 in
    // 877575/0 and his day ability takes Nantosuelte's Asymmetry-raised 8/7 to 7/6 in
    // 1009264/0. The GhosTown night bonus `-1 Opp Pow. And Damage, Min 1` pays in every
    // round of 1059149 - Callie 6/5 to 5/4, Sue 6/3 to 5/2 under her own floor - and in
    // round 1 lands on the Power Calamity's Exchange gave Tina: 5 + 2 - 1 = 6, the round
    // docs/replay-triage.md records the TypeScript engine getting wrong.
    (877575, 4),
    (1009264, 2),
    (1059149, 4),
    // Revision 45 admits the `Per Life Left` magnitudes, scaled by the owner's Life at the
    // start of the round. Ametia's `+1 Power Per Life Left Max. 13` reaches its Max at 12
    // Life in 1065427/0 and falls short of it at 11 in 877533/1; Sir Taco's `Max. 8` clamps
    // 1 + 12 to 8 in 1130454/0; KinGreow's clamps at 6 Life in 1025349/1. The Huracan bonus
    // `+1 Attack Per Life Left` pays across 1145812 and in 1079173/2, a losing round whose
    // 15 + 12 = 27 reads the round-start Life rather than what the round left,
    // and Jagan's `-1 Opp Att. Per Life Left, Min 2` takes Sue's 12 + 12 down by its own
    // owner's 9 Life, not the opponent's 7, in 1079650/3.
    (1065427, 4),
    (877533, 4),
    (1130454, 2),
    (1025349, 3),
    (1145812, 4),
    (1079173, 4),
    (1079650, 4),
    (876752, 2),
    // Revision 46 admits `Unison :` over the plain fixed numeric body: the printed amount
    // applies once when every card in the owner's hand shares the selected card's effective
    // clan. Sauropsite's `5318` takes 5/3 to 8/6 in 1069608/2 and so wins at 72 against 49,
    // where 45 would lose; Aurora falls from 7 to 4 under `4553` in 1131144/0; Wesley 6 - 2
    // - 2 = 2 in 1088323/3 shows the whole-hand condition holds after cards are spent.
    // 867173 stops at two rounds, where Prince Candle's unadmitted Combust latches.
    (1069608, 3),
    (947488, 2),
    (1131144, 1),
    (867173, 2),
    // Revision 47 admits `Stop Opp. Ability`/`Stop Opp. Bonus` under the Courage,
    // Confidence, Revenge, Asymmetry, Symmetry and Night predicates. Kerry Cr moves first in
    // 1069721/0 and stops Callie's `Support: Attack +3`, so her attack is 6 x 8 = 48, not
    // 60; Edd Cr stops an Equalizer attack reduction the same way in 926584/2.
    // 876882/0 stops the Komboka gain, so Keya ends on 12 and 12. In 1089742/1
    // Barbacoatl's Courage Stop Bonus is itself stopped by Spidee's Reprisal, so Spidee's
    // Support stays live (24 + 12 = 36), and in 1089742/3 Fraggle's Revenge stops Callie's
    // reduction. TTQ's Asymmetry Stop Bonus pays in 1091314/2 (48, not 42), and
    // Skeletrezar's Night Stop takes Buck's Power +2 away in 878120/0.
    (1069721, 3),
    (926584, 3),
    (876882, 2),
    (1089742, 4),
    (1091314, 3),
    (877308, 4),
    (878120, 4),
    // Revision 48 admits `Stop:` over the plain numeric body, modelled as the half the
    // server has shown: it never fires unless its owner's ability is stopped, and nothing
    // opposite these cards can stop one. Curie's `Stop: Damage +4` stays at 4 Damage in
    // 1009300/3, Nantosuelte's 4 Damage survives the opposing `Stop: -3 Opp. Dmg, Min 3` in
    // 1025645/0, and Belladone's attack is 27, not 34, in 925051/3.
    (1009300, 4),
    (1025645, 3),
    (925051, 4),
    // Revision 49 admits `Cancel Opp. Life Modif.` and `Cancel Opp. Pillz & Life Modif.` -
    // the opposing selected card's end-of-round effects on those resources are dropped for
    // the round - and `Killshot: +N Pillz And Life`. Ryujin Cr's Life cancel keeps Glenn's
    // `-3 Opp. Life Min 3` from taking 6 to 3 in 1337321/0, Sylvia Ld's Berzerk bonus in
    // 1337265/0, Hal Gladius' Equalizer reduction in 943231/0 and Aurora's `+3 Life` in
    // 1131208/0. Torkan's Killshot compound pays 2 and 2 at 38 against 14 in 1337321/2 and
    // not at 66 against 36 in 956805/0.
    (1337321, 3),
    (1337230, 3),
    (1337265, 2),
    (943231, 1),
    (1131208, 1),
    (956805, 1),
    (877357, 3),
];

const PROJECTION: CombatStatDiagnosticProjectionV1 =
    CombatStatDiagnosticProjectionV1::DisableDeferredAndOutOfSliceCardLocalEffects;

fn root_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(path)
}

fn catalog() -> CardCatalog {
    CardCatalog::load(root_path("data/data.json")).unwrap()
}

fn registry() -> EffectRegistryV1 {
    EffectRegistryV1::load(root_path("captures/abilities.json")).unwrap()
}

fn numeric_entry(
    id: u32,
    description: &str,
    position: &str,
    value: u16,
    minimum: u16,
) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "unlockLevel": 0,
        "description": description,
        "longDescription": description,
        "abilityData": {
            "value": value, "valueMin": minimum, "valueMax": 0, "valueCondition": 0,
            "positionRequirement": position, "previousRoundRequirement": "any",
            "currentRoundRequirement": "any", "indexRequirement": "any",
            "clanRequirement": "", "oppClanRequirement": "",
            "previousClanRequirement": "", "betPillzLink": "no",
            "sideAffected": "opponent", "attributeAffected": "pwr",
            "attributeAction": "decrease", "specialAction": "none",
            "isInverted": false, "isSupport": false, "isAntiSupport": false,
            "isOverdrive": false, "isDivide": false, "isLifeLinked": false,
            "isPillzLinked": false, "isLostLifeLinked": false,
            "isLostPillzLinked": false, "isOppStarsLinked": false,
            "isClanmatesCountLinked": false, "isAntiClanmatesCountLinked": false,
            "isPermanent": false, "isImmediatePermanent": false
        }
    })
}

fn index_numeric_entry(
    id: u32,
    description: &str,
    index: &str,
    position: &str,
    value: u16,
    minimum: u16,
) -> serde_json::Value {
    let mut entry = numeric_entry(id, description, position, value, minimum);
    entry["abilityData"]["indexRequirement"] = serde_json::json!(index);
    entry
}

fn previous_round_numeric_entry(
    id: u32,
    description: &str,
    previous_round: &str,
    value: u16,
    minimum: u16,
) -> serde_json::Value {
    let mut entry = numeric_entry(id, description, "both", value, minimum);
    entry["abilityData"]["previousRoundRequirement"] = serde_json::json!(previous_round);
    entry
}

fn defeat_recover_entry(id: u32, description: &str) -> serde_json::Value {
    let mut entry = numeric_entry(id, description, "both", 2, 3);
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("lose");
    entry["abilityData"]["sideAffected"] = serde_json::json!("player");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("pillz");
    entry["abilityData"]["attributeAction"] = serde_json::json!("increase");
    entry["abilityData"]["specialAction"] = serde_json::json!("recover_pillz");
    entry
}

fn victory_or_defeat_entry(id: u32) -> serde_json::Value {
    let mut entry = numeric_entry(id, "Victory Or Defeat : +1 Pillz", "both", 1, 0);
    entry["abilityData"]["sideAffected"] = serde_json::json!("player");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("pillz");
    entry["abilityData"]["attributeAction"] = serde_json::json!("increase");
    entry
}

fn victory_or_defeat_life_entry(
    id: u32,
    description: &str,
    life: u16,
    minimum: u16,
    owner_life: bool,
) -> serde_json::Value {
    let mut entry = numeric_entry(id, description, "both", life, minimum);
    entry["abilityData"]["sideAffected"] =
        serde_json::json!(if owner_life { "player" } else { "opponent" });
    entry["abilityData"]["attributeAffected"] = serde_json::json!("life");
    entry["abilityData"]["attributeAction"] =
        serde_json::json!(if owner_life { "increase" } else { "decrease" });
    entry
}

fn komboka_victory_pillz_and_life_entry(id: u32) -> serde_json::Value {
    let mut entry = numeric_entry(id, "+1 Pillz And Life", "both", 1, 0);
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("win");
    entry["abilityData"]["sideAffected"] = serde_json::json!("player");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("life&pillz");
    entry["abilityData"]["attributeAction"] = serde_json::json!("increase");
    entry
}

fn victory_life_entry(id: u32, life: u16) -> serde_json::Value {
    let mut entry = numeric_entry(id, &format!("+{life} Life"), "both", life, 0);
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("win");
    entry["abilityData"]["sideAffected"] = serde_json::json!("player");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("life");
    entry["abilityData"]["attributeAction"] = serde_json::json!("increase");
    entry
}

fn victory_pillz_entry(id: u32, pillz: u16) -> serde_json::Value {
    let mut entry = numeric_entry(id, &format!("+{pillz} Pillz"), "both", pillz, 0);
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("win");
    entry["abilityData"]["sideAffected"] = serde_json::json!("player");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("pillz");
    entry["abilityData"]["attributeAction"] = serde_json::json!("increase");
    entry
}

fn victory_opponent_pillz_entry(id: u32, pillz: u16, minimum: u16) -> serde_json::Value {
    let mut entry = numeric_entry(
        id,
        &format!("-{pillz} Opp Pillz. Min {minimum}"),
        "both",
        pillz,
        minimum,
    );
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("win");
    entry["abilityData"]["sideAffected"] = serde_json::json!("opponent");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("pillz");
    entry["abilityData"]["attributeAction"] = serde_json::json!("decrease");
    entry
}

fn defeat_opponent_pillz_entry(id: u32, pillz: u16, minimum: u16) -> serde_json::Value {
    let mut entry = numeric_entry(
        id,
        &format!("Defeat: -{pillz} Opp. Pillz, Min {minimum}"),
        "both",
        pillz,
        minimum,
    );
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("lose");
    entry["abilityData"]["sideAffected"] = serde_json::json!("opponent");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("pillz");
    entry["abilityData"]["attributeAction"] = serde_json::json!("decrease");
    entry
}

fn victory_pillz_per_damage_entry(id: u32, index: &str) -> serde_json::Value {
    let description = match index {
        "symmetry" => "Symmetry: +1 Pillz Per Damage",
        "asymmetry" => "Asymmetry: +1 Pillz Per Damage",
        _ => "+1 Pillz Per Damage",
    };
    let mut entry = numeric_entry(id, description, "both", 1, 0);
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("win");
    entry["abilityData"]["indexRequirement"] = serde_json::json!(index);
    entry["abilityData"]["sideAffected"] = serde_json::json!("player");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("pillz");
    entry["abilityData"]["attributeAction"] = serde_json::json!("increase");
    entry["abilityData"]["specialAction"] = serde_json::json!("convert_dmg_to_pillz");
    entry
}

fn victory_life_per_damage_entry(id: u32, life: u16, previous_round: &str) -> serde_json::Value {
    let description = match previous_round {
        "lose" => format!("Revenge: +{life} Life Per Damage"),
        "win" => format!("Confidence: +{life} Life Per Dmg."),
        _ => format!("+{life} Life Per Damage"),
    };
    let mut entry = numeric_entry(id, &description, "both", life, 0);
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("win");
    entry["abilityData"]["previousRoundRequirement"] = serde_json::json!(previous_round);
    entry["abilityData"]["sideAffected"] = serde_json::json!("player");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("life");
    entry["abilityData"]["attributeAction"] = serde_json::json!("increase");
    entry["abilityData"]["specialAction"] = serde_json::json!("convert_dmg_to_life");
    entry
}

fn prefixed_toxin_entry(
    id: u32,
    description: &str,
    index: &str,
    previous: &str,
) -> serde_json::Value {
    let mut entry = numeric_entry(id, description, "both", 3, 0);
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("win");
    entry["abilityData"]["indexRequirement"] = serde_json::json!(index);
    entry["abilityData"]["previousRoundRequirement"] = serde_json::json!(previous);
    entry["abilityData"]["sideAffected"] = serde_json::json!("opponent");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("life");
    entry["abilityData"]["attributeAction"] = serde_json::json!("decrease");
    entry["abilityData"]["isPermanent"] = serde_json::json!(true);
    entry["abilityData"]["isImmediatePermanent"] = serde_json::json!(true);
    entry
}

fn defeat_life_entry(id: u32, life: u16) -> serde_json::Value {
    let mut entry = numeric_entry(id, &format!("Defeat: +{life} Life"), "both", life, 1);
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("lose");
    entry["abilityData"]["sideAffected"] = serde_json::json!("player");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("life");
    entry["abilityData"]["attributeAction"] = serde_json::json!("increase");
    entry
}

fn reanimate_life_entry(id: u32, life: u16) -> serde_json::Value {
    let mut entry = numeric_entry(id, &format!("Reanimate: +{life} Life"), "both", life, 0);
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("lose");
    entry["abilityData"]["sideAffected"] = serde_json::json!("player");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("life");
    entry["abilityData"]["attributeAction"] = serde_json::json!("increase");
    entry
}

fn reprisal_stop_ability_entry(id: u32) -> serde_json::Value {
    let mut entry = numeric_entry(id, "Reprisal: Stop Opp. Ability", "defender", 0, 0);
    entry["abilityData"]["sideAffected"] = serde_json::json!("player");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("none");
    entry["abilityData"]["attributeAction"] = serde_json::json!("none");
    entry["abilityData"]["specialAction"] = serde_json::json!("stop_ability");
    entry
}

fn argos_defeat_capped_pillz_entry(id: u32, description: &str) -> serde_json::Value {
    let mut entry = numeric_entry(id, description, "both", 2, 0);
    entry["abilityData"]["valueMax"] = serde_json::json!(11);
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("lose");
    entry["abilityData"]["sideAffected"] = serde_json::json!("player");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("pillz");
    entry["abilityData"]["attributeAction"] = serde_json::json!("increase");
    entry
}

fn anita_courage_damage_to_life_entry(id: u32, description: &str) -> serde_json::Value {
    let mut entry = numeric_entry(id, description, "attacker", 1, 0);
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("win");
    entry["abilityData"]["sideAffected"] = serde_json::json!("player");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("life");
    entry["abilityData"]["attributeAction"] = serde_json::json!("increase");
    entry["abilityData"]["specialAction"] = serde_json::json!("convert_dmg_to_life");
    entry
}

fn round_scaled_numeric_entry(
    id: u32,
    description: &str,
    growth: bool,
    degrowth: bool,
    value: u16,
    minimum: u16,
) -> serde_json::Value {
    let mut entry = numeric_entry(id, description, "both", value, minimum);
    entry["abilityData"]["isOverdrive"] = serde_json::json!(growth);
    entry["abilityData"]["isDivide"] = serde_json::json!(degrowth);
    entry
}

fn equalizer_numeric_entry(
    id: u32,
    description: &str,
    value: u16,
    minimum: u16,
) -> serde_json::Value {
    let mut entry = numeric_entry(id, description, "both", value, minimum);
    entry["abilityData"]["isOppStarsLinked"] = serde_json::json!(true);
    entry
}

fn equalizer_opponent_life_entry(id: u32, description: &str) -> serde_json::Value {
    let mut entry = equalizer_numeric_entry(id, description, 1, 2);
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("win");
    entry["abilityData"]["sideAffected"] = serde_json::json!("opponent");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("life");
    entry["abilityData"]["attributeAction"] = serde_json::json!("decrease");
    entry
}

fn support_numeric_entry(
    id: u32,
    description: &str,
    value: u16,
    minimum: u16,
) -> serde_json::Value {
    let mut entry = numeric_entry(id, description, "both", value, minimum);
    entry["abilityData"]["isSupport"] = serde_json::json!(true);
    entry
}

fn one_entry_registry(entry: serde_json::Value) -> EffectRegistryV1 {
    let id = entry["id"].as_u64().unwrap().to_string();
    let mut entries = serde_json::Map::new();
    entries.insert(id, entry);
    let bytes = serde_json::to_vec(&serde_json::Value::Object(entries)).unwrap();
    EffectRegistryV1::from_reader(bytes.as_slice()).unwrap()
}

fn clear_sources(replay: &mut ReplayCaseV1) {
    for player in &mut replay.players {
        for card in &mut player.hand {
            card.source_ability = None;
            card.source_bonus = None;
        }
    }
}

fn replay(id: u64, catalog: &CardCatalog) -> ReplayCaseV1 {
    let capture = CapturedGame::from_reader(
        File::open(root_path(&format!("captures/games/{id}.json"))).unwrap(),
    )
    .unwrap();
    let ReplayClassification::Ready(replay) = adapt_capture(capture, catalog).unwrap() else {
        panic!("battle {id} was not replay-ready");
    };
    *replay
}

fn diagnostic(
    id: u64,
    catalog: &CardCatalog,
    registry: &EffectRegistryV1,
) -> CombatStatDiagnosticReplayV1 {
    CombatStatDiagnosticReplayV1::new(replay(id, catalog), catalog, registry, PROJECTION)
        .unwrap_or_else(|error| panic!("prepare combat-stat-diagnostic-v1 battle {id}: {error}"))
}

#[test]
fn fixed_server_backed_gate_replays_every_unique_sequential_prefix_round() {
    let catalog = catalog();
    let registry = registry();
    let mut rounds = 0;
    let mut execute_ids = BTreeSet::new();
    let mut disabled_ids = BTreeSet::new();
    let mut absent = 0;
    for &(battle_id, prefix) in COMBAT_STAT_PREFIX_FIXTURES {
        let report = diagnostic(battle_id, &catalog, &registry)
            .execute_combat_stat_diagnostic_v1_prefix(prefix)
            .unwrap_or_else(|error| panic!("combat-stat fixture {battle_id}/{prefix}: {error}"));
        rounds += report.rounds.len();
        for round in &report.rounds {
            for player in PlayerId::ALL {
                for disposition in [
                    &round.selected[player].ability,
                    &round.selected[player].bonus,
                ] {
                    match disposition {
                        CombatStatProjectionDispositionV1::Absent => absent += 1,
                        CombatStatProjectionDispositionV1::Execute { identity, .. } => {
                            execute_ids.insert(identity.id);
                        }
                        CombatStatProjectionDispositionV1::ExecutePostRound {
                            identity, ..
                        } => {
                            execute_ids.insert(identity.id);
                        }
                        CombatStatProjectionDispositionV1::Disabled { identity, .. } => {
                            disabled_ids.insert(identity.id);
                        }
                    }
                }
            }
        }
    }
    support::expect_count(
        "combat_stat_gate_rounds",
        "Sequential prefix rounds the combat-stat gate replays.",
        rounds,
    );
    support::expect_ids(
        "combat_stat_gate_execute_ids",
        "Registry definition ids the gate executed as a selected ability or bonus.",
        &execute_ids,
    );
    support::expect_ids(
        "combat_stat_gate_disabled_ids",
        "Registry definition ids the gate selected but left visible-but-disabled.",
        &disabled_ids,
    );
    support::expect_count(
        "combat_stat_gate_absent_dispositions",
        "Selected ability/bonus slots the gate found empty across those rounds.",
        absent,
    );
}

#[test]
fn gate_pins_stop_bonus_cancellation_and_sequential_fury() {
    let catalog = catalog();
    let registry = registry();

    let stopped = diagnostic(1088323, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(1)
        .unwrap();
    let attacks: Vec<_> = stopped.rounds[0]
        .round
        .cards
        .0
        .iter()
        .map(|card| card.attack)
        .collect();
    assert!(attacks.contains(&18));
    assert!(attacks.contains(&4));

    let cancelled = diagnostic(1089513, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(1)
        .unwrap();
    let attacks: Vec<_> = cancelled.rounds[0]
        .round
        .cards
        .0
        .iter()
        .map(|card| card.attack)
        .collect();
    assert!(attacks.contains(&26));
    assert!(attacks.contains(&6));

    let sequential = diagnostic(874837, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(2)
        .unwrap();
    assert_eq!(sequential.rounds.len(), 2);
    assert!(sequential.rounds[1]
        .round
        .cards
        .0
        .iter()
        .any(|card| card.damage == 5));
}

#[test]
fn server_replays_pin_unconditional_soa_from_ability_and_gheist_bonus() {
    let catalog = catalog();
    let registry = registry();
    let cases = [
        // Alexei's ability leaves Lothar's -3 Power inert.
        (1088323, 2, 1, true, 2965, (6, 4, 24), (1, 5, 14)),
        // An active GHEIST bonus suppresses -1 Opp Power And Damage.
        (1089001, 1, 0, false, 94, (8, 3, 24), (6, 5, 42)),
    ];
    for (battle_id, prefix, round_index, ability_source, source_id, owner_stats, opponent_stats) in
        cases
    {
        let report = diagnostic(battle_id, &catalog, &registry)
            .execute_combat_stat_diagnostic_v1_prefix(prefix)
            .unwrap_or_else(|error| panic!("SOA fixture {battle_id}/{prefix}: {error}"));
        let round = &report.rounds[round_index];
        let owner = PlayerId::ALL
            .into_iter()
            .find(|player| {
                let source = if ability_source {
                    &round.selected[*player].ability
                } else {
                    &round.selected[*player].bonus
                };
                matches!(
                    source,
                    CombatStatProjectionDispositionV1::Execute {
                        identity,
                        effect: SupportedEffectV1::StopOpponentAbility,
                        predicate: CombatStatPredicateV1::Always,
                    } if identity.id == source_id
                )
            })
            .unwrap_or_else(|| panic!("battle {battle_id} did not select SOA source {source_id}"));
        let opponent = owner.other();
        let stats = |player| {
            let card = &round.round.cards[player];
            (card.power, card.damage, card.attack)
        };
        assert_eq!(stats(owner), owner_stats, "battle {battle_id} SOA owner");
        assert_eq!(
            stats(opponent),
            opponent_stats,
            "battle {battle_id} SOA opponent"
        );
    }
}

#[test]
fn lyse_teria_soa_unlocks_the_complete_two_round_server_replay() {
    let catalog = catalog();
    let registry = registry();
    let report = diagnostic(1024673, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1()
        .unwrap();
    assert_eq!(report.rounds.len(), 2);
    let round = &report.rounds[0];
    let owner = PlayerId::ALL
        .into_iter()
        .find(|player| {
            matches!(
                round.selected[*player].ability,
                CombatStatProjectionDispositionV1::Execute {
                    identity: ref id,
                    effect: SupportedEffectV1::StopOpponentAbility,
                    ..
                } if id.id == 73
            )
        })
        .expect("Lyse Teria Cr's exact SOA must execute");
    assert_eq!(
        (
            round.round.cards[owner].power,
            round.round.cards[owner].damage,
            round.round.cards[owner].attack,
        ),
        (7, 2, 28)
    );
    assert_eq!(round.round.cards[owner.other()].attack, 8);
}

#[test]
fn dave_victory_life_full_four_round_replay_is_exact() {
    let catalog = catalog();
    let registry = registry();
    let report = diagnostic(877636, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1()
        .unwrap();
    assert_eq!(report.rounds.len(), 4);

    // The second normalized round selects Dave.  His +2 Life resolves after Zatman's
    // one damage: the owner therefore reaches 14 rather than the pre-effect 12.
    let round = &report.rounds[1];
    let owner = PlayerId::ALL
        .into_iter()
        .find(|player| matches!(
            round.selected[*player].ability,
            CombatStatProjectionDispositionV1::ExecutePostRound {
                effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainLifeOnVictory { life: 2 },
                ..
            }
        ))
        .expect("Dave's exact +2 Life plan is selected in round 2");
    assert!(round.round.cards[owner].won);
    assert_eq!(round.round.players[owner].life, 14);
    assert_eq!(report.final_position.players[owner].life, 4);
}

#[test]
fn server_replays_pin_jungo_victory_life_bonus_on_independent_wins() {
    let catalog = catalog();
    let registry = registry();
    for (battle_id, prefix, selected_round, expected_life) in
        [(877860, 1, 0, 14), (878011, 1, 0, 14)]
    {
        let report = diagnostic(battle_id, &catalog, &registry)
            .execute_combat_stat_diagnostic_v1_prefix(prefix)
            .unwrap();
        let round = &report.rounds[selected_round];
        let owner = PlayerId::ALL
            .into_iter()
            .find(|player| matches!(
                round.selected[*player].bonus,
                CombatStatProjectionDispositionV1::ExecutePostRound {
                    effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainLifeOnVictory { life: 2 },
                    ..
                }
            ))
            .expect("the selected Jungo card must carry the captured +2 Life bonus");
        assert!(round.round.cards[owner].won, "battle {battle_id}");
        assert_eq!(
            round.round.players[owner].life, expected_life,
            "battle {battle_id}"
        );
    }
}

/// Which engine player owns `card_id` in `round_index`, that player's captured Life after
/// the round, and the card's hand slot.
fn owner_life_and_slot(
    prepared: &CombatStatDiagnosticReplayV1,
    round_index: usize,
    card_id: u32,
) -> (PlayerId, u16, usize) {
    let round = &prepared.replay().rounds[round_index];
    let play = round
        .plays
        .iter()
        .find(|play| play.card.id == card_id)
        .unwrap_or_else(|| {
            panic!(
                "battle {} round {round_index} is missing card {card_id}",
                prepared.battle_id()
            )
        });
    let owner = match play.engine_player {
        EnginePlayer::P1 => PlayerId::P1,
        EnginePlayer::P2 => PlayerId::P2,
    };
    (
        owner,
        round.expected_player_states[owner.index()].life,
        usize::from(play.hand_index),
    )
}

/// The server evidence behind the Heal latch. None of these three draws is strict-eligible
/// (878093 and 1091985 select unadmitted grammars in their first round, 877733 a Unison
/// Pillz-and-Life in its second), so they cannot join the gate; their captured Life is what
/// the engine tests reproduce.
#[test]
fn lianah_heal_capture_evidence_pins_the_latch_round_the_repeat_and_the_stop() {
    let catalog = catalog();
    let registry = registry();
    let is_lianah_heal = |disposition: &CombatStatProjectionDispositionV1| {
        matches!(
            disposition,
            CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::HealLifeOnVictory {
                    life: 1,
                    maximum: 20,
                },
                predicate: CombatStatPredicateV1::Always,
            } if identity.id == 3526 && identity.description == "Heal 1 Max. 20"
        )
    };

    // 878093: Lianah wins round 0 on 12 Life and stays on 12 - the latch round pays nothing.
    // Round 1 is won by Buck with a KO, and the owner still ends it on 13: the repeat pays
    // whatever card is played, and a KO of the opponent does not skip it.
    let latched = diagnostic(878093, &catalog, &registry);
    let (owner, life_after_latch, slot) = owner_life_and_slot(&latched, 0, 978);
    assert!(is_lianah_heal(&latched.preparation()[owner][slot].ability));
    assert_eq!(life_after_latch, 12);
    assert_eq!(
        latched.replay().rounds[1].expected_player_states[owner.index()].life,
        13
    );
    assert_eq!(
        latched.replay().rounds[1].expected_player_states[owner.other().index()].life,
        0
    );

    // 1091985: Lianah wins round 1 with Fury and KOs the opponent. The server reports the
    // permanent Life increase for that round as quantity 0 and the owner stays on 12.
    let terminal = diagnostic(1091985, &catalog, &registry);
    let (owner, life, slot) = owner_life_and_slot(&terminal, 1, 978);
    assert!(is_lianah_heal(&terminal.preparation()[owner][slot].ability));
    assert_eq!(life, 12);
    assert_eq!(
        terminal.replay().rounds[1].expected_player_states[owner.other().index()].life,
        0
    );

    // 877733: Lianah wins round 0 against Pr Balthazar's Stop Opp. Ability. A stopped Heal
    // never latches, so her owner is still on 12 after round 1 and takes Agnes' 4 Damage
    // to 8 in round 2 with only Eugene's Defeat: +2 Life bringing it back to 10.
    let stopped = diagnostic(877733, &catalog, &registry);
    let (owner, life, slot) = owner_life_and_slot(&stopped, 0, 978);
    assert!(is_lianah_heal(&stopped.preparation()[owner][slot].ability));
    assert_eq!(life, 12);
    let (stopper, _, stopper_slot) = owner_life_and_slot(&stopped, 0, 1457);
    assert_eq!(stopper, owner.other());
    assert!(matches!(
        stopped.preparation()[stopper][stopper_slot].ability,
        CombatStatProjectionDispositionV1::Execute {
            effect: SupportedEffectV1::StopOpponentAbility,
            predicate: CombatStatPredicateV1::Always,
            ..
        }
    ));
    let owner_life =
        |round: usize| stopped.replay().rounds[round].expected_player_states[owner.index()].life;
    assert_eq!([owner_life(1), owner_life(2), owner_life(3)], [12, 10, 5]);
    // Round 0 itself replays: the stopped Heal is live-checked and refused there.
    let report = stopped.execute_combat_stat_diagnostic_v1_prefix(1).unwrap();
    assert!(report.final_position.latched[owner].is_empty());
    assert_eq!(report.final_position.players[owner].life, 12);
}

#[test]
fn dispositions_and_provenance_expose_predicates_and_compiler_revision() {
    let catalog = catalog();
    let registry = registry();
    let prepared = diagnostic(901400, &catalog, &registry);
    let provenance = prepared.preparation_provenance();
    assert_eq!(provenance.projection, PROJECTION);
    assert_eq!(
        provenance.compiler_policy_semantic_revision,
        COMBAT_STAT_DIAGNOSTIC_COMPILER_POLICY_SEMANTIC_REVISION_V1
    );
    support::expect_count(
        "combat_stat_compiler_policy_semantic_revision",
        "Compiler policy revision the replay provenance reports; bumped by every slice.",
        usize::from(provenance.compiler_policy_semantic_revision),
    );
    assert_eq!(
        provenance.effect_registry_source_fingerprint_fnv1a64,
        registry.source_fingerprint_fnv1a64()
    );
    assert!(prepared
        .preparation()
        .0
        .iter()
        .flatten()
        .any(|card| matches!(
            card.ability,
            CombatStatProjectionDispositionV1::Execute {
                predicate: CombatStatPredicateV1::OwnerMovesSecond,
                ..
            }
        )));
    let report = prepared
        .execute_combat_stat_diagnostic_v1_prefix(1)
        .unwrap();
    assert_eq!(report.provenance, provenance);
    assert_eq!(report.rounds[0].provenance, provenance);
}

#[test]
fn defeat_life_and_reanimate_capture_evidence_is_visible_without_widening_the_gate() {
    let catalog = catalog();
    let registry = registry();
    let source_in_round = |prepared: &CombatStatDiagnosticReplayV1,
                           round_index: usize,
                           card_id: u32| {
        let round = &prepared.replay().rounds[round_index];
        let play = round
            .plays
            .iter()
            .find(|play| play.card.id == card_id)
            .unwrap_or_else(|| panic!("battle {} is missing card {card_id}", prepared.battle_id()));
        (
            round.expected_player_states[match play.engine_player {
                EnginePlayer::P1 => PlayerId::P1,
                EnginePlayer::P2 => PlayerId::P2,
            }
            .index()]
            .life,
            match play.engine_player {
                EnginePlayer::P1 => PlayerId::P1,
                EnginePlayer::P2 => PlayerId::P2,
            },
            usize::from(play.hand_index),
        )
    };

    let lobo = diagnostic(1130654, &catalog, &registry);
    assert_eq!(
        lobo.preparation_provenance()
            .compiler_policy_semantic_revision,
        COMBAT_STAT_DIAGNOSTIC_COMPILER_POLICY_SEMANTIC_REVISION_V1
    );
    let (life, owner, slot) = source_in_round(&lobo, 1, 453);
    assert_eq!(life, 4); // 7 - Miyo 5 + 2
    assert!(matches!(
        lobo.preparation()[owner][slot].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            ref identity,
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::ReanimateLife {
                life: 2
            },
            ..
        } if identity.id == 4951
    ));

    let stopped_lobo = diagnostic(1080877, &catalog, &registry);
    let (life, owner, slot) = source_in_round(&stopped_lobo, 2, 453);
    // Lobo starts the round on 13 and Spidee deals 6. The capture ends at 8: Campbell's
    // already-latched Heal supplies the one Life, so stopped Reanimate did not supply +2.
    assert_eq!(life, 8);
    assert!(matches!(
        stopped_lobo.preparation()[owner][slot].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::ReanimateLife {
                life: 2
            },
            ..
        }
    ));

    let chadwik = diagnostic(1069193, &catalog, &registry);
    let (life, owner, slot) = source_in_round(&chadwik, 3, 2539);
    // Reprisal SOA is in the opposing selected source; Chadwik is therefore left at
    // 18 - Spidee 6 = 12 rather than receiving Defeat: +2 Life.
    assert_eq!(life, 12);
    assert!(matches!(
        chadwik.preparation()[owner][slot].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            ref identity,
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainLifeOnDefeat {
                life: 2
            },
            ..
        } if identity.id == 4635
    ));

    let reprisal_slot = chadwik.replay().rounds[3]
        .plays
        .iter()
        .find(|play| play.card.id == 1498)
        .expect("1069193 round 3 must select Spidee");
    let reprisal_owner = match reprisal_slot.engine_player {
        EnginePlayer::P1 => PlayerId::P1,
        EnginePlayer::P2 => PlayerId::P2,
    };
    assert!(matches!(
        chadwik.preparation()[reprisal_owner][usize::from(reprisal_slot.hand_index)].ability,
        CombatStatProjectionDispositionV1::Execute {
            ref identity,
            effect: SupportedEffectV1::StopOpponentAbility,
            predicate: CombatStatPredicateV1::OwnerMovesSecond,
        } if identity.id == 1310
    ));

    // In 1060199/r2 Spidee is the first mover, so the same captured source is present in
    // the compact plan but Reprisal is false. Donna Black's active Revenge reduction is
    // consequently still visible in the server's 44 Attack result.
    let inactive = diagnostic(1060199, &catalog, &registry);
    let round = &inactive.replay().rounds[2];
    let play = round
        .plays
        .iter()
        .find(|play| play.card.id == 1498)
        .expect("1060199 round 2 must select Spidee");
    let owner = match play.engine_player {
        EnginePlayer::P1 => PlayerId::P1,
        EnginePlayer::P2 => PlayerId::P2,
    };
    assert_eq!(round.first_mover, play.engine_player);
    assert!(matches!(
        inactive.preparation()[owner][usize::from(play.hand_index)].ability,
        CombatStatProjectionDispositionV1::Execute {
            ref identity,
            effect: SupportedEffectV1::StopOpponentAbility,
            predicate: CombatStatPredicateV1::OwnerMovesSecond,
        } if identity.id == 1310
    ));
    assert!(
        round
            .expected_card_results
            .iter()
            .flatten()
            .any(|result| result.attack == 44),
        "Donna Black's captured Attack 44 must survive inactive Reprisal"
    );
}

#[test]
fn kombokas_exact_pillz_and_life_bonus_unlocks_1069193_round_zero() {
    let catalog = catalog();
    let registry = registry();
    let report = diagnostic(1069193, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(1)
        .expect("1069193/r0 must execute Komboka's exact Victory bonus");
    let round = &report.rounds[0];
    assert!(matches!(
        round.selected[PlayerId::P2].bonus,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            ref identity,
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainOnePillzAndLifeOnVictory,
            ..
        } if identity.id == 1714
    ));
    // Pantherine pays five, then atomically gains Pillz before Life: 12 - 5 + 1 = 8.
    // Her winning +3 Life reaches 15 first; the Komboka bonus supplies the final 16.
    assert!(round.round.cards[PlayerId::P2].won);
    assert_eq!(round.round.players[PlayerId::P2].pillz, 8);
    assert_eq!(round.round.players[PlayerId::P2].life, 16);
}

#[test]
fn kombokas_pillz_and_life_admission_is_exact_to_bonus_1714_and_its_full_shape() {
    let catalog = catalog();
    let cases = [
        (1714, false, "+1 Pillz And Life", false, true),
        (3356, true, "+1 Pillz And Life", false, false),
        (1716, true, "Defeat: +1 Pillz And Life", false, false),
        (1714, false, "+1 Pillz And Life", true, false),
        (900_1714, false, "+1 Pillz And Life", true, false),
    ];
    for (id, ability, description, malformed, admitted) in cases {
        let mut source = replay(1069193, &catalog);
        clear_sources(&mut source);
        source.rounds.clear();
        let player = PlayerId::P2;
        let selected_slot = 3_usize;
        if ability {
            source.players[player.index()].hand[selected_slot].source_ability =
                Some(SourceModifier {
                    id,
                    description: description.to_owned(),
                });
        } else {
            source.players[player.index()].hand[selected_slot].source_bonus =
                Some(SourceModifier {
                    id,
                    description: description.to_owned(),
                });
        }
        let mut entry = komboka_victory_pillz_and_life_entry(id);
        entry["description"] = serde_json::json!(description);
        entry["longDescription"] = serde_json::json!(description);
        if id == 1716 {
            entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("lose");
        }
        if malformed {
            entry["abilityData"]["valueMax"] = serde_json::json!(12);
        }
        let prepared = CombatStatDiagnosticReplayV1::new(
            source,
            &catalog,
            &one_entry_registry(entry),
            PROJECTION,
        )
        .unwrap();
        let disposition = if ability {
            &prepared.preparation()[player][selected_slot].ability
        } else {
            &prepared.preparation()[player][selected_slot].bonus
        };
        assert_eq!(
            matches!(
                disposition,
                CombatStatProjectionDispositionV1::ExecutePostRound {
                    identity,
                    effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainOnePillzAndLifeOnVictory,
                    ..
                } if identity.id == 1714
            ),
            admitted,
            "id={id} ability={ability} malformed={malformed}"
        );
        let game = prepared.new_game();
        let plan = if ability {
            &game.card_plans()[player][selected_slot].ability
        } else {
            &game.card_plans()[player][selected_slot].bonus
        };
        if admitted {
            assert!(matches!(
                plan,
                CombatStatSourcePlanV1::Execute {
                    source_id: 1714,
                    predicate: CombatStatPredicateV1::Always,
                    effect: urban_recreation_rust::engine::CombatStatEffectV1::GainOnePillzAndLifeOnVictory,
                }
            ));
        } else {
            assert!(matches!(
                plan,
                CombatStatSourcePlanV1::RejectIfSelected { source_id } if *source_id == id
            ));
        }
    }
}

#[test]
fn komboka_bonus_server_evidence_pins_win_loss_stop_bonus_and_soa_liveness() {
    let catalog = catalog();
    let registry = registry();

    let win = diagnostic(866431, &catalog, &registry);
    // Hewa Cr's server win is evidence for the coupled amount, but the same round selects
    // deferred Support: -1 Opp. Life and so is not an executable prefix.
    assert!(matches!(
        win.preparation()[PlayerId::P2][1].bonus,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            ref identity,
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainOnePillzAndLifeOnVictory,
            ..
        } if identity.id == 1714
    ));
    assert_eq!(
        win.replay().rounds[0].expected_player_states[PlayerId::P2.index()].life,
        13
    );
    assert_eq!(
        win.replay().rounds[0].expected_player_states[PlayerId::P2.index()].pillz,
        6
    );

    let stop_bonus = diagnostic(876882, &catalog, &registry);
    // The exact bonus is visible on Keya, and since revision 47 the opponent's selected
    // Courage Stop Bonus executes: its owner moves first, so the Komboka gain is stopped
    // and Keya ends on 12 and 12 rather than 13 and 13.
    assert!(matches!(
        stop_bonus.preparation()[PlayerId::P2][1].bonus,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            ref identity,
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainOnePillzAndLifeOnVictory,
            ..
        } if identity.id == 1714
    ));
    assert!(matches!(
        stop_bonus.preparation()[PlayerId::P1][0].ability,
        CombatStatProjectionDispositionV1::Execute {
            ref identity,
            predicate: CombatStatPredicateV1::OwnerMovesFirst,
            ..
        } if identity.id == 287
    ));
    assert!(
        stop_bonus.replay().rounds[0].expected_card_results[PlayerId::P2.index()]
            .expect("Keya has a server result")
            .won
    );
    assert_eq!(
        stop_bonus.replay().rounds[0].expected_player_states[PlayerId::P2.index()].life,
        12
    );
    assert_eq!(
        stop_bonus.replay().rounds[0].expected_player_states[PlayerId::P2.index()].pillz,
        12
    );

    let soa_not_sob = diagnostic(1066077, &catalog, &registry);
    // Spidee's active Reprisal is SOA rather than SOB. Adytia's selected bonus remains
    // live in the compact plan and the server records its 8 Life / 5 Pillz win; Adytia's
    // deferred own ability prevents this round from becoming an executable prefix.
    let round = &soa_not_sob.replay().rounds[1];
    let adytia = round
        .plays
        .iter()
        .find(|play| play.card.id == 1867)
        .expect("1066077/r1 selects Adytia");
    let owner = match adytia.engine_player {
        EnginePlayer::P1 => PlayerId::P1,
        EnginePlayer::P2 => PlayerId::P2,
    };
    assert!(matches!(
        soa_not_sob.preparation()[owner][usize::from(adytia.hand_index)].bonus,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            ref identity,
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainOnePillzAndLifeOnVictory,
            ..
        } if identity.id == 1714
    ));
    assert!(
        round.expected_card_results[owner.index()]
            .expect("Adytia has a server result")
            .won
    );
    assert_eq!(round.expected_player_states[owner.index()].life, 8);
    assert_eq!(round.expected_player_states[owner.index()].pillz, 5);

    // Kunglaba's KO win is not an executable prefix because Hilal's deferred compound
    // opponent reduction was selected earlier. The source plan and server endpoint still
    // pin that the coupled Victory gain is paid after a KO.
    let ko_win = diagnostic(1023608, &catalog, &registry);
    let round = &ko_win.replay().rounds[3];
    let play = round
        .plays
        .iter()
        .find(|play| play.card.id == 2302)
        .expect("1023608/r3 selects Kunglaba");
    let owner = match play.engine_player {
        EnginePlayer::P1 => PlayerId::P1,
        EnginePlayer::P2 => PlayerId::P2,
    };
    assert!(matches!(
        ko_win.preparation()[owner][usize::from(play.hand_index)].bonus,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            ref identity,
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainOnePillzAndLifeOnVictory,
            ..
        } if identity.id == 1714
    ));
    assert!(
        round.expected_card_results[owner.index()]
            .expect("Kunglaba has a server result")
            .won
    );
    assert_eq!(round.expected_player_states[owner.index()].life, 12);
    assert_eq!(round.expected_player_states[owner.index()].pillz, 1);

    // A losing Kubra does not receive either part of the Victory-only pair, including at
    // zero Life. `876712/r1` remains outside the executable prefix because it contains
    // other deferred effects, so this is deliberately disposition-and-server evidence.
    let lethal_loss = diagnostic(876712, &catalog, &registry);
    let round = &lethal_loss.replay().rounds[1];
    let play = round
        .plays
        .iter()
        .find(|play| play.card.id == 1868)
        .expect("876712/r1 selects Kubra");
    let owner = match play.engine_player {
        EnginePlayer::P1 => PlayerId::P1,
        EnginePlayer::P2 => PlayerId::P2,
    };
    assert!(matches!(
        lethal_loss.preparation()[owner][usize::from(play.hand_index)].bonus,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            ref identity,
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainOnePillzAndLifeOnVictory,
            ..
        } if identity.id == 1714
    ));
    assert!(
        !round.expected_card_results[owner.index()]
            .expect("Kubra has a server result")
            .won
    );
    assert_eq!(round.expected_player_states[owner.index()].life, 0);
    assert_eq!(round.expected_player_states[owner.index()].pillz, 9);
}

#[test]
fn canonical_leader_and_team_modifier_are_fatal_even_when_unplayed() {
    let catalog = catalog();
    let registry = registry();
    let mut leader = replay(1069938, &catalog);
    leader.players[0].hand[0].source_ability = None;
    let error =
        CombatStatDiagnosticReplayV1::new(leader, &catalog, &registry, PROJECTION).unwrap_err();
    assert!(matches!(
        error,
        CombatStatDiagnosticPreparationErrorV1::WholeHandExecutionHazard {
            source: CombatStatWholeHandHazardSourceV1::CanonicalLeaderCard { .. },
            ..
        }
    ));

    for player in 0..2 {
        for bonus in [false, true] {
            let mut team = replay(875032, &catalog);
            let modifier = Some(SourceModifier {
                id: 4237,
                description: "Team: +7 Attack".to_owned(),
            });
            if bonus {
                team.players[player].hand[3].source_bonus = modifier;
            } else {
                team.players[player].hand[3].source_ability = modifier;
            }
            let error = CombatStatDiagnosticReplayV1::new(team, &catalog, &registry, PROJECTION)
                .unwrap_err();
            assert!(matches!(
                error,
                CombatStatDiagnosticPreparationErrorV1::WholeHandExecutionHazard {
                    source: CombatStatWholeHandHazardSourceV1::Modifier { .. },
                    ..
                }
            ));
            assert!(error.to_string().contains("battle 875032"));
            assert!(error.to_string().contains("slot 3"));
        }
    }

    let mut illusion = replay(875032, &catalog);
    illusion.players[0].hand[3].source_ability = Some(SourceModifier {
        id: 3128,
        description: "Illusion".to_owned(),
    });
    assert!(matches!(
        CombatStatDiagnosticReplayV1::new(illusion, &catalog, &registry, PROJECTION),
        Err(
            CombatStatDiagnosticPreparationErrorV1::WholeHandExecutionHazard {
                source: CombatStatWholeHandHazardSourceV1::Modifier { .. },
                ..
            }
        )
    ));
}

#[test]
fn support_abilities_execute_while_capped_increases_remain_disabled() {
    let catalog = catalog();
    let registry = registry();
    let mut source = replay(875032, &catalog);
    source.rounds.clear();
    source.players[0].hand[0].source_ability = Some(SourceModifier {
        id: 266,
        description: "Support: Attack +3".to_owned(),
    });
    source.players[0].hand[1].source_ability = Some(SourceModifier {
        id: 2969,
        description: "Power +6, Max. 8".to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][0].ability,
        CombatStatProjectionDispositionV1::Execute { .. }
    ));
    assert_eq!(
        prepared.preparation()[PlayerId::P1][0].source_ability_support_count,
        prepared.preparation()[PlayerId::P1][0].effective_clan_character_count
    );
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][1].ability,
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::CappedIncrease { .. },
            ..
        }
    ));

    for (id, description) in [(2969, "Power +6, Max. 8")] {
        let mut source = replay(875032, &catalog);
        clear_sources(&mut source);
        let selected_slot = usize::from(
            source.rounds[0]
                .plays
                .iter()
                .find(|play| play.engine_player == EnginePlayer::P1)
                .unwrap()
                .hand_index,
        );
        source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
            id,
            description: description.to_owned(),
        });
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        assert!(matches!(
            prepared.execute_combat_stat_diagnostic_v1_prefix(1),
            Err(CombatStatDiagnosticReplayErrorV1::Engine { .. })
        ));
    }
}

#[test]
fn every_observed_basic_combat_stat_support_definition_executes_as_an_ability() {
    let catalog = catalog();
    let registry = registry();
    let expected = BTreeSet::from([
        266, 272, 295, 367, 391, 412, 469, 472, 514, 532, 546, 567, 574, 739, 899, 1269, 1297,
        1325, 1330, 1735, 1805, 2535, 2556, 3197, 3475, 3719, 4068, 4297, 4593, 4824, 4839, 4857,
        5483, 5841,
    ]);
    let observed: BTreeSet<_> = registry
        .iter()
        .filter_map(|(id, definition)| {
            let input = definition.structured_input();
            (input.is_support
                && matches!(
                    input.attribute_affected,
                    AttributeAffectedV1::Attack
                        | AttributeAffectedV1::Damage
                        | AttributeAffectedV1::Power
                        | AttributeAffectedV1::PowerAndDamage
                ))
            .then_some(id)
        })
        .collect();
    assert_eq!(observed, expected);

    let mut template = replay(875032, &catalog);
    template.rounds.clear();
    clear_sources(&mut template);
    for id in expected {
        let definition = registry.get(id).unwrap();
        let mut source = template.clone();
        source.players[0].hand[0].source_ability = Some(SourceModifier {
            id,
            description: definition.description().to_owned(),
        });
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        assert!(
            matches!(
                prepared.preparation()[PlayerId::P1][0].ability,
                CombatStatProjectionDispositionV1::Execute {
                    effect: SupportedEffectV1::ModifyCombatStat {
                        multiplier: MagnitudeMultiplierV1::Support,
                        ..
                    },
                    predicate: CombatStatPredicateV1::Always,
                    ..
                }
            ),
            "effect {id}"
        );
        assert_eq!(
            prepared.preparation()[PlayerId::P1][0].source_ability_support_count,
            prepared.preparation()[PlayerId::P1][0].effective_clan_character_count,
            "effect {id}"
        );
    }
}

#[test]
fn support_ability_grammar_is_exact_and_nested_or_unobserved_shapes_fail_closed() {
    const EFFECT_ID: u32 = 900_105;
    let mut cases = Vec::new();
    cases.push((
        "exact",
        support_numeric_entry(EFFECT_ID, "Support: -2 Opp Power, Min 3", 2, 3),
        true,
    ));
    cases.push((
        "malformed prefix",
        support_numeric_entry(EFFECT_ID, "Support:-2 Opp Power, Min 3", 2, 3),
        false,
    ));

    let mut position = support_numeric_entry(EFFECT_ID, "Support: -2 Opp Power, Min 3", 2, 3);
    position["abilityData"]["positionRequirement"] = serde_json::json!("attacker");
    cases.push(("position", position, false));

    let mut current = support_numeric_entry(EFFECT_ID, "Support: -2 Opp Power, Min 3", 2, 3);
    current["abilityData"]["currentRoundRequirement"] = serde_json::json!("win");
    cases.push(("current round", current, false));

    let mut index = support_numeric_entry(EFFECT_ID, "Support: -2 Opp Power, Min 3", 2, 3);
    index["abilityData"]["indexRequirement"] = serde_json::json!("symmetry");
    cases.push(("index", index, false));

    let mut growth = support_numeric_entry(EFFECT_ID, "Support: -2 Opp Power, Min 3", 2, 3);
    growth["abilityData"]["isOverdrive"] = serde_json::json!(true);
    cases.push(("growth", growth, false));

    let mut power_and_damage =
        support_numeric_entry(EFFECT_ID, "Support: Power And Damage +1", 1, 0);
    power_and_damage["abilityData"]["sideAffected"] = serde_json::json!("player");
    power_and_damage["abilityData"]["attributeAffected"] = serde_json::json!("pwr&dmg");
    power_and_damage["abilityData"]["attributeAction"] = serde_json::json!("increase");
    cases.push(("unobserved Power And Damage", power_and_damage, false));

    let catalog = catalog();
    let mut template = replay(875032, &catalog);
    template.rounds.clear();
    clear_sources(&mut template);
    for (label, entry, admitted) in cases {
        let description = entry["description"].as_str().unwrap().to_owned();
        let registry = one_entry_registry(entry);
        let mut source = template.clone();
        source.players[0].hand[0].source_ability = Some(SourceModifier {
            id: EFFECT_ID,
            description,
        });
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        assert_eq!(
            matches!(
                prepared.preparation()[PlayerId::P1][0].ability,
                CombatStatProjectionDispositionV1::Execute {
                    effect: SupportedEffectV1::ModifyCombatStat {
                        multiplier: MagnitudeMultiplierV1::Support,
                        ..
                    },
                    predicate: CombatStatPredicateV1::Always,
                    ..
                }
            ),
            admitted,
            "{label}"
        );
        if !admitted {
            assert!(matches!(
                prepared.preparation()[PlayerId::P1][0].ability,
                CombatStatProjectionDispositionV1::Disabled {
                    reason: CombatStatDisabledReasonV1::SupportAbility { .. },
                    ..
                }
            ));
        }
    }
}

#[test]
fn selected_stop_ability_is_visible_and_rejected_fail_closed() {
    let catalog = catalog();
    let registry = registry();
    let mut source = replay(875032, &catalog);
    let selected_slot = usize::from(
        source.rounds[0]
            .plays
            .iter()
            .find(|play| play.engine_player == EnginePlayer::P1)
            .unwrap()
            .hand_index,
    );
    // Courage Stop Opp. Ability is admitted by grammar since revision 47; the Unison form
    // is not, so it is the selected control that must still refuse the round.
    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: 3839,
        description: "Unison : Stop Opp. Ability".to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][selected_slot].ability,
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::UnsupportedSelectedHazard { .. },
            ..
        }
    ));
    let error = prepared
        .execute_combat_stat_diagnostic_v1_prefix(1)
        .unwrap_err();
    assert!(matches!(
        error,
        CombatStatDiagnosticReplayErrorV1::Engine { .. }
    ));
    assert!(error.to_string().contains("selected="));
}

#[test]
fn reprisal_soa_is_exactly_the_two_captured_ability_aliases() {
    let catalog = catalog();
    let selected_slot = 0_usize;
    for (id, admitted) in [(1310, true), (2073, true), (900_131, false)] {
        let mut source = replay(875032, &catalog);
        clear_sources(&mut source);
        source.rounds.clear();
        if admitted {
            source.players[0].hand[selected_slot].key = match id {
                1310 => CardKey { id: 1498, level: 4 },
                2073 => CardKey { id: 2042, level: 3 },
                _ => unreachable!("only the two admitted aliases reach this branch"),
            };
        }
        source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
            id,
            description: "Reprisal: Stop Opp. Ability".to_owned(),
        });
        let registry = one_entry_registry(reprisal_stop_ability_entry(id));
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        let disposition = &prepared.preparation()[PlayerId::P1][selected_slot].ability;
        assert_eq!(
            matches!(
                disposition,
                CombatStatProjectionDispositionV1::Execute {
                    identity,
                    effect: SupportedEffectV1::StopOpponentAbility,
                    predicate: CombatStatPredicateV1::OwnerMovesSecond,
                } if identity.id == id
            ),
            admitted,
            "registry id {id}"
        );
        if !admitted {
            assert!(matches!(
                disposition,
                CombatStatProjectionDispositionV1::Disabled {
                    reason: CombatStatDisabledReasonV1::UnsupportedPromisedControl { .. }
                        | CombatStatDisabledReasonV1::UnsupportedSelectedHazard { .. },
                    ..
                }
            ));
            assert!(matches!(
                prepared.new_game().card_plans()[PlayerId::P1][selected_slot].ability,
                CombatStatSourcePlanV1::RejectIfSelected { source_id } if source_id == id
            ));
        }
    }

    // The admitted identity still has to retain its complete defender shape.  A malformed
    // same-id record is not quietly disabled: selecting it remains an atomic hazard.
    let mut malformed = reprisal_stop_ability_entry(1310);
    malformed["abilityData"]["positionRequirement"] = serde_json::json!("attacker");
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    source.rounds.clear();
    source.players[0].hand[selected_slot].key = CardKey { id: 1498, level: 4 };
    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: 1310,
        description: "Reprisal: Stop Opp. Ability".to_owned(),
    });
    let prepared = CombatStatDiagnosticReplayV1::new(
        source,
        &catalog,
        &one_entry_registry(malformed),
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][selected_slot].ability,
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::UnsupportedPromisedControl { .. }
                | CombatStatDisabledReasonV1::UnsupportedSelectedHazard { .. },
            ..
        }
    ));
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][selected_slot].ability,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: 1310 }
    ));
}

#[test]
fn selected_pillz_and_life_cancellation_rejects_before_admitted_recovery_can_run() {
    let catalog = catalog();
    let registry = registry();
    const CONTROL_ID: u32 = 1497;
    const CONTROL_DESCRIPTION: &str = "Cancel Opp. Pillz & Life Modif.";
    const RECOVERY_DESCRIPTION: &str = "Defeat: Recover 2 Pillz Out Of 3";
    let cases = [
        (PlayerId::P1, PlayerId::P2, 1418, true),
        (PlayerId::P2, PlayerId::P1, 577, false),
    ];

    for (control_player, recovery_player, recovery_id, recovery_is_ability) in cases {
        let mut source = replay(875032, &catalog);
        clear_sources(&mut source);
        let (first_mover, selections) = {
            let round = &source.rounds[0];
            let selection = |player| {
                round
                    .plays
                    .iter()
                    .find(|play| {
                        matches!(
                            (player, play.engine_player),
                            (PlayerId::P1, EnginePlayer::P1) | (PlayerId::P2, EnginePlayer::P2)
                        )
                    })
                    .unwrap()
            };
            let selection = |player| {
                let play = selection(player);
                BaseRulesSelection::new(play.hand_index, play.pillz, play.fury)
            };
            (
                match round.first_mover {
                    EnginePlayer::P1 => PlayerId::P1,
                    EnginePlayer::P2 => PlayerId::P2,
                },
                ByPlayer::new(selection(PlayerId::P1), selection(PlayerId::P2)),
            )
        };
        let control_slot = usize::from(selections[control_player].hand_index);
        let recovery_slot = usize::from(selections[recovery_player].hand_index);
        source.players[control_player.index()].hand[control_slot].source_ability =
            Some(SourceModifier {
                id: CONTROL_ID,
                description: CONTROL_DESCRIPTION.to_owned(),
            });
        let recovery = Some(SourceModifier {
            id: recovery_id,
            description: RECOVERY_DESCRIPTION.to_owned(),
        });
        if recovery_is_ability {
            source.players[recovery_player.index()].hand[recovery_slot].source_ability = recovery;
        } else {
            source.players[recovery_player.index()].hand[recovery_slot].source_bonus = recovery;
        }

        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        // Since revision 49 the canceller is admitted, but only where nothing opposite has an
        // effect whose cancellation is unpinned. The Pillz half of `Cancel Opp. Pillz & Life`
        // has no paying round, and the opposing recovery is Pillz, so the canceller is a
        // selected hazard here rather than an executed control.
        assert!(matches!(
            prepared.preparation()[control_player][control_slot].ability,
            CombatStatProjectionDispositionV1::Disabled {
                reason: CombatStatDisabledReasonV1::UnsupportedSelectedHazard { .. },
                ..
            }
        ));
        let recovery_disposition = if recovery_is_ability {
            &prepared.preparation()[recovery_player][recovery_slot].ability
        } else {
            &prepared.preparation()[recovery_player][recovery_slot].bonus
        };
        assert!(matches!(
            recovery_disposition,
            CombatStatProjectionDispositionV1::ExecutePostRound { identity, .. }
                if identity.id == recovery_id
        ));

        let mut game = prepared.new_game();
        assert!(matches!(
            game.card_plans()[control_player][control_slot].ability,
            CombatStatSourcePlanV1::RejectIfSelected {
                source_id: CONTROL_ID
            }
        ));
        let before = game.position().clone();
        let input = BaseRulesRoundInput {
            first_mover,
            selections,
        };
        assert!(matches!(
            game.make(input),
            Err(CombatStatDiagnosticErrorV1::UnsupportedSelectedHazard {
                player,
                source: CombatStatEffectSourceV1::Ability,
                source_id: CONTROL_ID,
                ..
            }) if player == control_player
        ));
        assert_eq!(game.position(), &before);
    }
}

#[test]
fn positional_grammar_is_exact_and_nested_contexts_fail_closed() {
    let catalog = catalog();
    const EFFECT_ID: u32 = 900_101;
    let cases = [
        ("Courage: Night: -2 Opp Power, Min 3", false),
        ("Courage: Experimental: -2 Opp Power, Min 3", false),
        ("Courage: -3 Opp Power, Min 3", false),
        ("Courage: -2 Opp Power, Min 3", true),
    ];
    for (description, admitted) in cases {
        let registry = one_entry_registry(numeric_entry(EFFECT_ID, description, "attacker", 2, 3));
        let mut source = replay(875032, &catalog);
        clear_sources(&mut source);
        let selected_slot = usize::from(
            source.rounds[0]
                .plays
                .iter()
                .find(|play| play.engine_player == EnginePlayer::P1)
                .unwrap()
                .hand_index,
        );
        source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
            id: EFFECT_ID,
            description: description.to_owned(),
        });
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        assert_eq!(
            matches!(
                prepared.preparation()[PlayerId::P1][selected_slot].ability,
                CombatStatProjectionDispositionV1::Execute { .. }
            ),
            admitted,
            "{description}"
        );
        if !admitted {
            assert!(matches!(
                prepared.execute_combat_stat_diagnostic_v1_prefix(1),
                Err(CombatStatDiagnosticReplayErrorV1::Engine { .. })
            ));
        }
    }

    let description = "Courage: Team: -2 Opp Power, Min 3";
    let registry = one_entry_registry(numeric_entry(EFFECT_ID, description, "attacker", 2, 3));
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    source.players[1].hand[3].source_bonus = Some(SourceModifier {
        id: EFFECT_ID,
        description: description.to_owned(),
    });
    assert!(matches!(
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION),
        Err(CombatStatDiagnosticPreparationErrorV1::WholeHandExecutionHazard { .. })
    ));
}

#[test]
fn previous_round_grammar_is_exact_for_fixed_numeric_abilities_and_bonuses() {
    let catalog = catalog();
    const EFFECT_ID: u32 = 900_106;
    let cases = [
        (
            "Confidence: -2 Opp Power, Min 3",
            "win",
            Some(CombatStatPredicateV1::OwnerWonPreviousRound),
        ),
        (
            "Confidence : -2 Opp Power, Min 3",
            "win",
            Some(CombatStatPredicateV1::OwnerWonPreviousRound),
        ),
        (
            "Revenge: -2 Opp Power, Min 3",
            "lose",
            Some(CombatStatPredicateV1::OwnerLostPreviousRound),
        ),
        ("Confidence: -2 Opp Power, Min 3", "lose", None),
        ("Confidence: Night: -2 Opp Power, Min 3", "win", None),
        ("Revenge: -3 Opp Power, Min 3", "lose", None),
    ];
    for (description, previous_round, predicate) in cases {
        let registry = one_entry_registry(previous_round_numeric_entry(
            EFFECT_ID,
            description,
            previous_round,
            2,
            3,
        ));
        let mut source = replay(875032, &catalog);
        clear_sources(&mut source);
        let selected_slot = usize::from(
            source.rounds[0]
                .plays
                .iter()
                .find(|play| play.engine_player == EnginePlayer::P1)
                .unwrap()
                .hand_index,
        );
        source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
            id: EFFECT_ID,
            description: description.to_owned(),
        });
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        assert_eq!(
            match &prepared.preparation()[PlayerId::P1][selected_slot].ability {
                CombatStatProjectionDispositionV1::Execute {
                    predicate: actual, ..
                } => Some(*actual),
                CombatStatProjectionDispositionV1::Absent
                | CombatStatProjectionDispositionV1::ExecutePostRound { .. }
                | CombatStatProjectionDispositionV1::Disabled { .. } => None,
            },
            predicate,
            "{description} / {previous_round}"
        );
        if predicate.is_none() {
            assert!(matches!(
                prepared.execute_combat_stat_diagnostic_v1_prefix(1),
                Err(CombatStatDiagnosticReplayErrorV1::Engine { .. })
            ));
        }
    }

    let description = "Confidence: -2 Opp Power, Min 3";
    let registry = one_entry_registry(previous_round_numeric_entry(
        EFFECT_ID,
        description,
        "win",
        2,
        3,
    ));
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    let selected_slot = usize::from(
        source.rounds[0]
            .plays
            .iter()
            .find(|play| play.engine_player == EnginePlayer::P1)
            .unwrap()
            .hand_index,
    );
    source.players[0].hand[selected_slot].source_bonus = Some(SourceModifier {
        id: EFFECT_ID,
        description: description.to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][selected_slot].bonus,
        CombatStatProjectionDispositionV1::Execute {
            predicate: CombatStatPredicateV1::OwnerWonPreviousRound,
            ..
        }
    ));

    let mut nested = previous_round_numeric_entry(EFFECT_ID, description, "win", 2, 3);
    nested["abilityData"]["currentRoundRequirement"] = serde_json::json!("win");
    let nested_registry = one_entry_registry(nested);
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: EFFECT_ID,
        description: description.to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &nested_registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][selected_slot].ability,
        CombatStatProjectionDispositionV1::Disabled { .. }
    ));
}

#[test]
fn defeat_recover_grammar_is_exact_for_the_three_audited_source_id_pairs() {
    let catalog = catalog();
    const DESCRIPTION: &str = "Defeat: Recover 2 Pillz Out Of 3";
    let cases = [
        (577, false, true),
        (577, true, false),
        (729, false, false),
        (729, true, true),
        (1418, false, false),
        (1418, true, true),
        (2475, false, false),
        (2475, true, false),
    ];

    for (id, ability, admitted) in cases {
        let registry = one_entry_registry(defeat_recover_entry(id, DESCRIPTION));
        let mut source = replay(875032, &catalog);
        clear_sources(&mut source);
        let selected_slot = usize::from(
            source.rounds[0]
                .plays
                .iter()
                .find(|play| play.engine_player == EnginePlayer::P1)
                .unwrap()
                .hand_index,
        );
        let modifier = Some(SourceModifier {
            id,
            description: DESCRIPTION.to_owned(),
        });
        if ability {
            source.players[0].hand[selected_slot].source_ability = modifier;
        } else {
            source.players[0].hand[selected_slot].source_bonus = modifier;
        }
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        let disposition = if ability {
            &prepared.preparation()[PlayerId::P1][selected_slot].ability
        } else {
            &prepared.preparation()[PlayerId::P1][selected_slot].bonus
        };
        assert_eq!(
            matches!(
                disposition,
                CombatStatProjectionDispositionV1::ExecutePostRound { .. }
            ),
            admitted,
            "id={id}, ability={ability}"
        );
        if id == 2475 {
            assert!(matches!(
                disposition,
                CombatStatProjectionDispositionV1::Disabled { identity, .. }
                    if identity.id == 2475
            ));
        }
        if !admitted {
            assert!(matches!(
                prepared.execute_combat_stat_diagnostic_v1_prefix(1),
                Err(CombatStatDiagnosticReplayErrorV1::Engine { .. })
            ));
        }
    }

    let mut malformed = defeat_recover_entry(1418, DESCRIPTION);
    malformed["abilityData"]["valueMin"] = serde_json::json!(2);
    let registry = one_entry_registry(malformed);
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    let selected_slot = usize::from(
        source.rounds[0]
            .plays
            .iter()
            .find(|play| play.engine_player == EnginePlayer::P1)
            .unwrap()
            .hand_index,
    );
    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: 1418,
        description: DESCRIPTION.to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][selected_slot].ability,
        CombatStatProjectionDispositionV1::Disabled { .. }
    ));
    assert!(matches!(
        prepared.execute_combat_stat_diagnostic_v1_prefix(1),
        Err(CombatStatDiagnosticReplayErrorV1::Engine { .. })
    ));
}

#[test]
fn victory_life_compiler_requires_the_complete_structured_shape_for_abilities_and_bonuses() {
    let catalog = catalog();
    const ID: u32 = 888;
    const DESCRIPTION: &str = "+2 Life";
    for ability in [false, true] {
        let registry = one_entry_registry(victory_life_entry(ID, 2));
        let mut source = replay(875032, &catalog);
        clear_sources(&mut source);
        let selected_slot = usize::from(
            source.rounds[0]
                .plays
                .iter()
                .find(|play| play.engine_player == EnginePlayer::P1)
                .unwrap()
                .hand_index,
        );
        let modifier = Some(SourceModifier {
            id: ID,
            description: DESCRIPTION.to_owned(),
        });
        if ability {
            source.players[0].hand[selected_slot].source_ability = modifier;
        } else {
            source.players[0].hand[selected_slot].source_bonus = modifier;
        }
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        let disposition = if ability {
            &prepared.preparation()[PlayerId::P1][selected_slot].ability
        } else {
            &prepared.preparation()[PlayerId::P1][selected_slot].bonus
        };
        assert!(matches!(
            disposition,
            CombatStatProjectionDispositionV1::ExecutePostRound {
                effect:
                    urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainLifeOnVictory {
                        life: 2
                    },
                ..
            }
        ));
    }

    // A condition mutation remains a selected hazard.  It cannot silently become generic
    // life support merely because the printed description happens to look familiar.
    let mut malformed = victory_life_entry(ID, 2);
    malformed["abilityData"]["currentRoundRequirement"] = serde_json::json!("any");
    let registry = one_entry_registry(malformed);
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    let selected_slot = usize::from(
        source.rounds[0]
            .plays
            .iter()
            .find(|play| play.engine_player == EnginePlayer::P1)
            .unwrap()
            .hand_index,
    );
    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: ID,
        description: DESCRIPTION.to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][selected_slot].ability,
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::UnsupportedPostRoundResourceEffect { .. },
            ..
        }
    ));
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][selected_slot].ability,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: ID }
    ));

    // Description grammar is not the fail-closed boundary. A structurally Life-affecting
    // source with near-miss text must still reject when selected.
    const MALFORMED_DESCRIPTION: &str = "+2 life";
    let mut malformed = victory_life_entry(ID, 2);
    malformed["description"] = serde_json::json!(MALFORMED_DESCRIPTION);
    let registry = one_entry_registry(malformed);
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: ID,
        description: MALFORMED_DESCRIPTION.to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][selected_slot].ability,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: ID }
    ));
}

#[test]
fn victory_pillz_compiler_admits_the_complete_ability_shape_and_near_misses_reject() {
    let catalog = catalog();
    const ID: u32 = 1150;
    const DESCRIPTION: &str = "+2 Pillz";
    let selected_slot = |source: &ReplayCaseV1| {
        usize::from(
            source.rounds[0]
                .plays
                .iter()
                .find(|play| play.engine_player == EnginePlayer::P1)
                .unwrap()
                .hand_index,
        )
    };

    // The Ability slot executes with the printed magnitude.
    let registry = one_entry_registry(victory_pillz_entry(ID, 2));
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    let slot = selected_slot(&source);
    source.players[0].hand[slot].source_ability = Some(SourceModifier {
        id: ID,
        description: DESCRIPTION.to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][slot].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            effect:
                urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainPillzOnVictory {
                    pillz: 2
                },
            predicate: CombatStatPredicateV1::Always,
            ..
        }
    ));
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].ability,
        CombatStatSourcePlanV1::Execute {
            source_id: ID,
            predicate: CombatStatPredicateV1::Always,
            effect: urban_recreation_rust::engine::CombatStatEffectV1::GainPillzOnVictory {
                pillz: 2
            },
        }
    ));

    // No clan bonus prints `+N Pillz`: the same record in the Bonus slot is a hazard.
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    source.players[0].hand[slot].source_bonus = Some(SourceModifier {
        id: ID,
        description: DESCRIPTION.to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][slot].bonus,
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::UnsupportedPostRoundResourceEffect { .. },
            ..
        }
    ));
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].bonus,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: ID }
    ));

    // A condition mutation under the familiar text rejects when selected.
    let mut malformed = victory_pillz_entry(ID, 2);
    malformed["abilityData"]["currentRoundRequirement"] = serde_json::json!("any");
    let registry = one_entry_registry(malformed);
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    source.players[0].hand[slot].source_ability = Some(SourceModifier {
        id: ID,
        description: DESCRIPTION.to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][slot].ability,
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::UnsupportedPostRoundResourceEffect { .. },
            ..
        }
    ));
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].ability,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: ID }
    ));

    // The complete structure under near-miss text is the other half of the boundary.
    const MALFORMED_DESCRIPTION: &str = "+2 pillz";
    let mut malformed = victory_pillz_entry(ID, 2);
    malformed["description"] = serde_json::json!(MALFORMED_DESCRIPTION);
    let registry = one_entry_registry(malformed);
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    source.players[0].hand[slot].source_ability = Some(SourceModifier {
        id: ID,
        description: MALFORMED_DESCRIPTION.to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].ability,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: ID }
    ));

    // A capped sibling differs structurally and keeps the ordinary disabled record.
    let mut capped = victory_pillz_entry(1139, 3);
    capped["description"] = serde_json::json!("+3 Pillz Max. 9");
    capped["abilityData"]["valueMax"] = serde_json::json!(9);
    let registry = one_entry_registry(capped);
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    source.players[0].hand[slot].source_ability = Some(SourceModifier {
        id: 1139,
        description: "+3 Pillz Max. 9".to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][slot].ability,
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::CappedIncrease { .. },
            ..
        }
    ));
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].ability,
        CombatStatSourcePlanV1::Disabled { source_id: 1139 }
    ));
}

#[test]
fn victory_opponent_pillz_compiler_admits_the_complete_ability_shape_and_near_misses_reject() {
    let catalog = catalog();
    const ID: u32 = 339;
    const DESCRIPTION: &str = "-3 Opp Pillz. Min 4";
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    let slot = usize::from(
        source.rounds[0]
            .plays
            .iter()
            .find(|play| play.engine_player == EnginePlayer::P1)
            .unwrap()
            .hand_index,
    );
    let with_ability = |id: u32, description: &str| {
        let mut source = source.clone();
        source.players[0].hand[slot].source_ability = Some(SourceModifier {
            id,
            description: description.to_owned(),
        });
        source
    };

    // The Ability slot executes with the printed magnitude and floor.
    let registry = one_entry_registry(victory_opponent_pillz_entry(ID, 3, 4));
    let prepared = CombatStatDiagnosticReplayV1::new(
        with_ability(ID, DESCRIPTION),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][slot].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            effect:
                urban_recreation_rust::engine::CombatStatPostRoundEffectV1::ReduceOpponentPillzOnVictory {
                    pillz: 3,
                    minimum: 4
                },
            predicate: CombatStatPredicateV1::Always,
            ..
        }
    ));

    // The same record in the Bonus slot is a hazard: no clan prints it.
    let mut bonus = source.clone();
    bonus.players[0].hand[slot].source_bonus = Some(SourceModifier {
        id: ID,
        description: DESCRIPTION.to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(bonus, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][slot].bonus,
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::UnsupportedPostRoundResourceEffect { .. },
            ..
        }
    ));
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].bonus,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: ID }
    ));

    // A condition mutation under the familiar text rejects when selected.
    let mut malformed = victory_opponent_pillz_entry(ID, 3, 4);
    malformed["abilityData"]["currentRoundRequirement"] = serde_json::json!("any");
    let prepared = CombatStatDiagnosticReplayV1::new(
        with_ability(ID, DESCRIPTION),
        &catalog,
        &one_entry_registry(malformed),
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].ability,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: ID }
    ));

    // So does the complete structure under near-miss text.
    const MALFORMED_DESCRIPTION: &str = "-3 opp pillz. min 4";
    let mut malformed = victory_opponent_pillz_entry(ID, 3, 4);
    malformed["description"] = serde_json::json!(MALFORMED_DESCRIPTION);
    let prepared = CombatStatDiagnosticReplayV1::new(
        with_ability(ID, MALFORMED_DESCRIPTION),
        &catalog,
        &one_entry_registry(malformed),
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].ability,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: ID }
    ));

    // The round-scaled `Growth:` sibling differs structurally and keeps its ordinary
    // disabled record.
    const GROWTH: &str = "Growth: -1 Opp Pillz. Min 0";
    let mut growth = victory_opponent_pillz_entry(2590, 1, 0);
    growth["description"] = serde_json::json!(GROWTH);
    growth["abilityData"]["isOverdrive"] = serde_json::json!(true);
    let prepared = CombatStatDiagnosticReplayV1::new(
        with_ability(2590, GROWTH),
        &catalog,
        &one_entry_registry(growth),
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][slot].ability,
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::OrdinaryAbility { .. },
            ..
        }
    ));
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].ability,
        CombatStatSourcePlanV1::Disabled { source_id: 2590 }
    ));
}

/// Hattori's `304`/`961` print the opponent-Damage reduction with the stat abbreviated.
/// The grammar, the shape and the Courage predicate were all already admitted - only the
/// spelling was not - so this pins the spelling as an alternative of the same text check
/// and nothing wider: a different magnitude, a different floor or a lowercased stat under
/// the same complete structure still rejects.
#[test]
fn courage_opponent_damage_admits_the_abbreviated_printed_spelling_only() {
    let catalog = catalog();
    const EFFECT_ID: u32 = 900_117;
    let cases = [
        ("Courage: -4 Opp. Dmg, Min 2", true),
        ("Courage: -4 Opp. Damage, Min 2", true),
        ("Courage: -4 Opp Damage, Min 2", true),
        ("Courage: -3 Opp. Dmg, Min 2", false),
        ("Courage: -4 Opp. Dmg, Min 3", false),
        ("Courage: -4 opp. dmg, min 2", false),
        ("Courage: -4 Opp. Dmg Min 2", false),
        ("-4 Opp. Dmg, Min 2", false),
    ];
    for (description, admitted) in cases {
        let mut entry = numeric_entry(EFFECT_ID, description, "attacker", 4, 2);
        entry["abilityData"]["attributeAffected"] = serde_json::json!("dmg");
        let registry = one_entry_registry(entry);
        let mut source = replay(875032, &catalog);
        clear_sources(&mut source);
        let slot = usize::from(
            source.rounds[0]
                .plays
                .iter()
                .find(|play| play.engine_player == EnginePlayer::P1)
                .unwrap()
                .hand_index,
        );
        source.players[0].hand[slot].source_ability = Some(SourceModifier {
            id: EFFECT_ID,
            description: description.to_owned(),
        });
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        assert_eq!(
            matches!(
                prepared.preparation()[PlayerId::P1][slot].ability,
                CombatStatProjectionDispositionV1::Execute {
                    predicate: CombatStatPredicateV1::OwnerMovesFirst,
                    ..
                }
            ),
            admitted,
            "{description}"
        );
    }
}

#[test]
fn defeat_opponent_pillz_compiler_admits_the_complete_ability_shape_and_near_misses_reject() {
    let catalog = catalog();
    const ID: u32 = 912;
    const DESCRIPTION: &str = "Defeat: -2 Opp. Pillz, Min 4";
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    let slot = usize::from(
        source.rounds[0]
            .plays
            .iter()
            .find(|play| play.engine_player == EnginePlayer::P1)
            .unwrap()
            .hand_index,
    );
    let with_ability = |id: u32, description: &str| {
        let mut source = source.clone();
        source.players[0].hand[slot].source_ability = Some(SourceModifier {
            id,
            description: description.to_owned(),
        });
        source
    };

    // The Ability slot executes with the printed magnitude and floor.
    let registry = one_entry_registry(defeat_opponent_pillz_entry(ID, 2, 4));
    let prepared = CombatStatDiagnosticReplayV1::new(
        with_ability(ID, DESCRIPTION),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][slot].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            effect:
                urban_recreation_rust::engine::CombatStatPostRoundEffectV1::ReduceOpponentPillzOnDefeat {
                    pillz: 2,
                    minimum: 4
                },
            predicate: CombatStatPredicateV1::Always,
            ..
        }
    ));

    // The same record in the Bonus slot is a hazard: no clan prints it.
    let mut bonus = source.clone();
    bonus.players[0].hand[slot].source_bonus = Some(SourceModifier {
        id: ID,
        description: DESCRIPTION.to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(bonus, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].bonus,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: ID }
    ));

    // The Victory sibling's outcome under this text is a different record and rejects.
    let mut malformed = defeat_opponent_pillz_entry(ID, 2, 4);
    malformed["abilityData"]["currentRoundRequirement"] = serde_json::json!("win");
    let prepared = CombatStatDiagnosticReplayV1::new(
        with_ability(ID, DESCRIPTION),
        &catalog,
        &one_entry_registry(malformed),
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].ability,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: ID }
    ));

    // So does the complete structure under the Victory grammar's own spelling, which is a
    // different printed text and must not be read as this one.
    const VICTORY_SPELLING: &str = "-2 Opp Pillz. Min 4";
    let mut malformed = defeat_opponent_pillz_entry(ID, 2, 4);
    malformed["description"] = serde_json::json!(VICTORY_SPELLING);
    let prepared = CombatStatDiagnosticReplayV1::new(
        with_ability(ID, VICTORY_SPELLING),
        &catalog,
        &one_entry_registry(malformed),
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].ability,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: ID }
    ));

    // The clan-gated sibling `4673` carries a clan requirement, which every admitted
    // post-round shape requires empty. It stays an ordinary disabled ability.
    const CLAN_GATED: &str =
        "[clan:38][clan:54][clan:42][clan:28][clan:59] Defeat: -1 Opp. Pillz, Min 4";
    let mut gated = defeat_opponent_pillz_entry(4673, 1, 4);
    gated["description"] = serde_json::json!(CLAN_GATED);
    gated["abilityData"]["clanRequirement"] = serde_json::json!("38,54,42,28,59");
    let prepared = CombatStatDiagnosticReplayV1::new(
        with_ability(4673, CLAN_GATED),
        &catalog,
        &one_entry_registry(gated),
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].ability,
        CombatStatSourcePlanV1::Disabled { source_id: 4673 }
    ));
}

#[test]
fn victory_pillz_per_damage_compiler_admits_the_plain_and_symmetry_abilities_only() {
    let catalog = catalog();
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    let slot = usize::from(
        source.rounds[0]
            .plays
            .iter()
            .find(|play| play.engine_player == EnginePlayer::P1)
            .unwrap()
            .hand_index,
    );
    let with_ability = |id: u32, description: &str| {
        let mut source = source.clone();
        source.players[0].hand[slot].source_ability = Some(SourceModifier {
            id,
            description: description.to_owned(),
        });
        source
    };

    // The plain and Symmetry forms execute with the predicate their prefix names.
    for (id, index, description, predicate) in [
        (
            1090,
            "any",
            "+1 Pillz Per Damage",
            CombatStatPredicateV1::Always,
        ),
        (
            1852,
            "symmetry",
            "Symmetry: +1 Pillz Per Damage",
            CombatStatPredicateV1::SelectedHandSlotsMatch,
        ),
    ] {
        let prepared = CombatStatDiagnosticReplayV1::new(
            with_ability(id, description),
            &catalog,
            &one_entry_registry(victory_pillz_per_damage_entry(id, index)),
            PROJECTION,
        )
        .unwrap();
        match &prepared.preparation()[PlayerId::P1][slot].ability {
            CombatStatProjectionDispositionV1::ExecutePostRound {
                effect,
                predicate: actual,
                ..
            } => {
                assert_eq!(
                    *effect,
                    urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainPillzEqualToFinalDamageOnVictory
                );
                assert_eq!(*actual, predicate);
            }
            other => panic!("{description} was not prepared as Pillz per Damage: {other:?}"),
        }
    }

    // The Bonus slot is a hazard: no clan prints the conversion.
    let mut bonus = source.clone();
    bonus.players[0].hand[slot].source_bonus = Some(SourceModifier {
        id: 1090,
        description: "+1 Pillz Per Damage".to_owned(),
    });
    let prepared = CombatStatDiagnosticReplayV1::new(
        bonus,
        &catalog,
        &one_entry_registry(victory_pillz_per_damage_entry(1090, "any")),
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][slot].bonus,
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::UnsupportedPostRoundResourceEffect { .. },
            ..
        }
    ));
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].bonus,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: 1090 }
    ));

    // An unreviewed hand-slot prefix over the complete shape rejects when selected, and so
    // does a magnitude other than one under the plain text.
    let prepared = CombatStatDiagnosticReplayV1::new(
        with_ability(900_201, "Asymmetry: +1 Pillz Per Damage"),
        &catalog,
        &one_entry_registry(victory_pillz_per_damage_entry(900_201, "asymmetry")),
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].ability,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: 900_201 }
    ));
    let mut doubled = victory_pillz_per_damage_entry(1090, "any");
    doubled["abilityData"]["value"] = serde_json::json!(2);
    let prepared = CombatStatDiagnosticReplayV1::new(
        with_ability(1090, "+1 Pillz Per Damage"),
        &catalog,
        &one_entry_registry(doubled),
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].ability,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: 1090 }
    ));
}

#[test]
fn life_per_damage_and_prefixed_permanents_carry_the_predicate_their_prefix_names() {
    let catalog = catalog();
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    let slot = usize::from(
        source.rounds[0]
            .plays
            .iter()
            .find(|play| play.engine_player == EnginePlayer::P1)
            .unwrap()
            .hand_index,
    );
    let with_ability = |id: u32, description: &str| {
        let mut source = source.clone();
        source.players[0].hand[slot].source_ability = Some(SourceModifier {
            id,
            description: description.to_owned(),
        });
        source
    };
    let plan_of = |source: ReplayCaseV1, registry: &EffectRegistryV1| {
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, registry, PROJECTION).unwrap();
        (
            prepared.preparation()[PlayerId::P1][slot].ability.clone(),
            prepared.new_game().card_plans()[PlayerId::P1][slot].ability,
        )
    };

    // The three Life-per-Damage grammars.
    for (id, life, previous, description, predicate) in [
        (
            492,
            1,
            "any",
            "+1 Life Per Damage",
            CombatStatPredicateV1::Always,
        ),
        (
            189,
            2,
            "any",
            "+2 Life Per Damage",
            CombatStatPredicateV1::Always,
        ),
        (
            1661,
            1,
            "lose",
            "Revenge: +1 Life Per Damage",
            CombatStatPredicateV1::OwnerLostPreviousRound,
        ),
        (
            1810,
            1,
            "win",
            "Confidence: +1 Life Per Dmg.",
            CombatStatPredicateV1::OwnerWonPreviousRound,
        ),
    ] {
        let registry = one_entry_registry(victory_life_per_damage_entry(id, life, previous));
        let (disposition, plan) = plan_of(with_ability(id, description), &registry);
        match disposition {
            CombatStatProjectionDispositionV1::ExecutePostRound {
                effect,
                predicate: actual,
                ..
            } => {
                assert_eq!(
                    effect,
                    urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainLifePerFinalDamageOnVictory {
                        life_per_damage: life,
                        maximum: 0
                    }
                );
                assert_eq!(actual, predicate);
            }
            other => panic!("{description} was not prepared as Life per Damage: {other:?}"),
        }
        assert!(matches!(
            plan,
            CombatStatSourcePlanV1::Execute { predicate: p, .. } if p == predicate
        ));
    }

    // The capped `Max. M` form executes with the same magnitude and an upper bound.
    for (id, life, maximum) in [(1146, 1, 8), (1161, 2, 20)] {
        let mut capped = victory_life_per_damage_entry(id, life, "any");
        capped["description"] =
            serde_json::json!(format!("+{life} Life Per Damage Max. {maximum}"));
        capped["abilityData"]["valueMax"] = serde_json::json!(maximum);
        let description = format!("+{life} Life Per Damage Max. {maximum}");
        let (disposition, plan) =
            plan_of(with_ability(id, &description), &one_entry_registry(capped));
        match disposition {
            CombatStatProjectionDispositionV1::ExecutePostRound {
                effect,
                predicate: actual,
                ..
            } => {
                assert_eq!(
                    effect,
                    urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainLifePerFinalDamageOnVictory {
                        life_per_damage: life,
                        maximum
                    }
                );
                assert_eq!(actual, CombatStatPredicateV1::Always);
            }
            other => panic!("{description} was not prepared as capped Life per Damage: {other:?}"),
        }
        assert!(matches!(
            plan,
            CombatStatSourcePlanV1::Execute {
                predicate: CombatStatPredicateV1::Always,
                ..
            }
        ));
    }

    // A cap under a previous-round prefix has never been printed, so its text is a guess
    // and the combination rejects when selected.
    let mut capped_prefix = victory_life_per_damage_entry(1146, 1, "win");
    capped_prefix["description"] = serde_json::json!("Confidence: +1 Life Per Dmg. Max. 8");
    capped_prefix["abilityData"]["valueMax"] = serde_json::json!(8);
    let (_, plan) = plan_of(
        with_ability(1146, "Confidence: +1 Life Per Dmg. Max. 8"),
        &one_entry_registry(capped_prefix),
    );
    assert!(matches!(
        plan,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: 1146 }
    ));

    // Anita's Courage record keeps its identity lock: the same text under another id, and
    // a Bonus-slot Life conversion, reject when selected.
    let mut anita_alias = victory_life_per_damage_entry(843, 1, "any");
    anita_alias["description"] = serde_json::json!("Courage: +1 Life Per Dmg");
    anita_alias["abilityData"]["positionRequirement"] = serde_json::json!("attacker");
    let (_, plan) = plan_of(
        with_ability(843, "Courage: +1 Life Per Dmg"),
        &one_entry_registry(anita_alias),
    );
    assert!(matches!(
        plan,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: 843 }
    ));
    let mut bonus = source.clone();
    bonus.players[0].hand[slot].source_bonus = Some(SourceModifier {
        id: 492,
        description: "+1 Life Per Damage".to_owned(),
    });
    let prepared = CombatStatDiagnosticReplayV1::new(
        bonus,
        &catalog,
        &one_entry_registry(victory_life_per_damage_entry(492, 1, "any")),
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][slot].bonus,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: 492 }
    ));

    // A permanent under a hand-slot or previous-round prefix latches under that predicate;
    // the prefix has to agree with the structured field, and any other condition is still a
    // hazard when the complete permanent shape sits beneath it.
    let registry = one_entry_registry(prefixed_toxin_entry(
        5092,
        "Symmetry: Toxin 3, Min 0",
        "symmetry",
        "any",
    ));
    let (disposition, plan) = plan_of(with_ability(5092, "Symmetry: Toxin 3, Min 0"), &registry);
    assert!(matches!(
        disposition,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::ToxinOpponentLifeOnVictory {
                life: 3,
                minimum: 0
            },
            predicate: CombatStatPredicateV1::SelectedHandSlotsMatch,
            ..
        }
    ));
    assert!(matches!(
        plan,
        CombatStatSourcePlanV1::Execute {
            predicate: CombatStatPredicateV1::SelectedHandSlotsMatch,
            ..
        }
    ));
    let registry = one_entry_registry(prefixed_toxin_entry(
        5092,
        "Symmetry: Toxin 3, Min 0",
        "any",
        "lose",
    ));
    let (_, plan) = plan_of(with_ability(5092, "Symmetry: Toxin 3, Min 0"), &registry);
    assert!(matches!(
        plan,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: 5092 }
    ));
    // Two conditions at once is no admitted grammar; under plain Toxin text it is the
    // same-text hazard every malformed plain permanent is.
    let mut both = prefixed_toxin_entry(900_301, "Symmetry: Toxin 3, Min 0", "symmetry", "lose");
    both["description"] = serde_json::json!("Toxin 3, Min 0");
    let (disposition, plan) = plan_of(
        with_ability(900_301, "Toxin 3, Min 0"),
        &one_entry_registry(both),
    );
    assert!(matches!(
        disposition,
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::UnsupportedPostRoundResourceEffect { .. },
            ..
        }
    ));
    assert!(matches!(
        plan,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: 900_301 }
    ));
}

#[test]
fn defeat_life_and_reanimate_are_post_round_abilities_and_near_misses_reject() {
    let catalog = catalog();
    const DEFEAT_ID: u32 = 862;
    const REANIMATE_ID: u32 = 4951;
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    let selected_slot = usize::from(
        source.rounds[0]
            .plays
            .iter()
            .find(|play| play.engine_player == EnginePlayer::P1)
            .unwrap()
            .hand_index,
    );
    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: DEFEAT_ID,
        description: "Defeat: +2 Life".to_owned(),
    });
    let prepared = CombatStatDiagnosticReplayV1::new(
        source.clone(),
        &catalog,
        &one_entry_registry(defeat_life_entry(DEFEAT_ID, 2)),
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][selected_slot].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainLifeOnDefeat {
                life: 2
            },
            ..
        }
    ));

    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: REANIMATE_ID,
        description: "Reanimate: +2 Life".to_owned(),
    });
    let prepared = CombatStatDiagnosticReplayV1::new(
        source.clone(),
        &catalog,
        &one_entry_registry(reanimate_life_entry(REANIMATE_ID, 2)),
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][selected_slot].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::ReanimateLife {
                life: 2
            },
            ..
        }
    ));

    // Other neutral Reanimate records are a visible selected hazard until server evidence
    // authorizes an identity beyond Lobo's captured Ability:4951.
    const OTHER_REANIMATE_ID: u32 = 900_114;
    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: OTHER_REANIMATE_ID,
        description: "Reanimate: +2 Life".to_owned(),
    });
    let prepared = CombatStatDiagnosticReplayV1::new(
        source.clone(),
        &catalog,
        &one_entry_registry(reanimate_life_entry(OTHER_REANIMATE_ID, 2)),
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][selected_slot].ability,
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::UnsupportedPostRoundResourceEffect { .. },
            ..
        }
    ));
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][selected_slot].ability,
        CombatStatSourcePlanV1::RejectIfSelected {
            source_id: OTHER_REANIMATE_ID
        }
    ));

    let mut malformed = defeat_life_entry(DEFEAT_ID, 2);
    malformed["abilityData"]["valueMin"] = serde_json::json!(0);
    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: DEFEAT_ID,
        description: "Defeat: +2 Life".to_owned(),
    });
    let prepared = CombatStatDiagnosticReplayV1::new(
        source,
        &catalog,
        &one_entry_registry(malformed),
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][selected_slot].ability,
        CombatStatSourcePlanV1::RejectIfSelected {
            source_id: DEFEAT_ID
        }
    ));

    // Deferred variants remain hazards rather than becoming disabled no-ops: a Life cap,
    // compound Life/Pillz gain, or nested Reanimate context is not admitted by this slice.
    for (id, description, mut entry) in [
        (1217, "Defeat: +2 Life Max. 12", defeat_life_entry(1217, 2)),
        (
            1716,
            "Defeat: +1 Pillz And Life",
            defeat_life_entry(1716, 1),
        ),
        (
            900_115,
            "Support: Reanimate: +2 Life",
            reanimate_life_entry(900_115, 2),
        ),
    ] {
        entry["description"] = serde_json::json!(description);
        entry["longDescription"] = serde_json::json!(description);
        match id {
            1217 => entry["abilityData"]["valueMax"] = serde_json::json!(12),
            1716 => entry["abilityData"]["attributeAffected"] = serde_json::json!("life&pillz"),
            900_115 => entry["abilityData"]["isSupport"] = serde_json::json!(true),
            _ => unreachable!(),
        }
        let mut deferred = replay(875032, &catalog);
        clear_sources(&mut deferred);
        deferred.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
            id,
            description: description.to_owned(),
        });
        let prepared = CombatStatDiagnosticReplayV1::new(
            deferred,
            &catalog,
            &one_entry_registry(entry),
            PROJECTION,
        )
        .unwrap();
        assert!(matches!(
            prepared.new_game().card_plans()[PlayerId::P1][selected_slot].ability,
            CombatStatSourcePlanV1::RejectIfSelected { source_id } if source_id == id
        ));
    }
}

#[test]
fn victory_or_defeat_is_exactly_the_audited_post_round_resource_effect() {
    let catalog = catalog();
    const DESCRIPTION: &str = "Victory Or Defeat : +1 Pillz";
    let cases = [
        (1034, false, true),
        (1034, true, true),
        (1375, true, true),
        (4111, true, true),
        (5085, true, true),
        (5520, true, true),
        (1375, false, false),
        (900_107, true, false),
    ];

    for (id, ability, admitted) in cases {
        let registry = one_entry_registry(victory_or_defeat_entry(id));
        let mut source = replay(875032, &catalog);
        clear_sources(&mut source);
        let selected_slot = usize::from(
            source.rounds[0]
                .plays
                .iter()
                .find(|play| play.engine_player == EnginePlayer::P1)
                .unwrap()
                .hand_index,
        );
        let modifier = Some(SourceModifier {
            id,
            description: DESCRIPTION.to_owned(),
        });
        if ability {
            source.players[0].hand[selected_slot].source_ability = modifier;
        } else {
            source.players[0].hand[selected_slot].source_bonus = modifier;
        }
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        let disposition = if ability {
            &prepared.preparation()[PlayerId::P1][selected_slot].ability
        } else {
            &prepared.preparation()[PlayerId::P1][selected_slot].bonus
        };
        assert_eq!(
            matches!(
                disposition,
                CombatStatProjectionDispositionV1::ExecutePostRound {
                    effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainOnePillzOnVictoryOrDefeat,
                    ..
                }
            ),
            admitted,
            "id={id}, ability={ability}"
        );
        if !admitted {
            assert!(matches!(
                disposition,
                CombatStatProjectionDispositionV1::Disabled {
                    reason: CombatStatDisabledReasonV1::UnsupportedPostRoundResourceEffect { .. },
                    ..
                }
            ));
            let plan = if ability {
                prepared.new_game().card_plans()[PlayerId::P1][selected_slot].ability
            } else {
                prepared.new_game().card_plans()[PlayerId::P1][selected_slot].bonus
            };
            assert!(matches!(
                plan,
                CombatStatSourcePlanV1::RejectIfSelected { source_id } if source_id == id
            ));
            let mut game = prepared.new_game();
            let before = game.position().clone();
            assert!(matches!(
                game.make(BaseRulesRoundInput {
                    first_mover: PlayerId::P1,
                    selections: ByPlayer::new(
                        BaseRulesSelection::new(selected_slot as u8, 0, false),
                        BaseRulesSelection::new(0, 0, false),
                    ),
                }),
                Err(CombatStatDiagnosticErrorV1::UnsupportedSelectedHazard {
                    player: PlayerId::P1,
                    source_id,
                    ..
                }) if source_id == id
            ));
            assert_eq!(game.position(), &before);
        }
    }

    let mut malformed = victory_or_defeat_entry(1034);
    malformed["abilityData"]["valueMin"] = serde_json::json!(1);
    let registry = one_entry_registry(malformed);
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    let selected_slot = usize::from(
        source.rounds[0]
            .plays
            .iter()
            .find(|play| play.engine_player == EnginePlayer::P1)
            .unwrap()
            .hand_index,
    );
    source.players[0].hand[selected_slot].source_bonus = Some(SourceModifier {
        id: 1034,
        description: DESCRIPTION.to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][selected_slot].bonus,
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::UnsupportedPostRoundResourceEffect { .. },
            ..
        }
    ));
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][selected_slot].bonus,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: 1034 }
    ));

    let registry = one_entry_registry({
        let mut entry = victory_or_defeat_entry(900_107);
        entry["description"] = serde_json::json!("+1 Pillz");
        entry["longDescription"] = serde_json::json!("+1 Pillz");
        entry
    });
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    source.players[0].hand[0].source_bonus = Some(SourceModifier {
        id: 900_107,
        description: "+1 Pillz".to_owned(),
    });
    // Since revision 29 the plain `+N Pillz` text is a reviewed grammar of its own, so this
    // same-text record over the Victory Or Defeat shape is a selected hazard rather than an
    // inert no-op, exactly as `+2 life` is for Victory Life.
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][0].bonus,
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::UnsupportedPostRoundResourceEffect { .. },
            ..
        }
    ));
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][0].bonus,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: 900_107 }
    ));
}

#[test]
fn victory_or_defeat_life_is_exactly_the_audited_post_round_family() {
    let catalog = catalog();
    let cases = [
        (1396, "Victory Or Defeat : +1 Life", 1, 1, true),
        (2992, "Victory Or Defeat : +1 Life", 1, 1, true),
        (5835, "Victory Or Defeat : +1 Life", 1, 1, true),
        (5799, "Victory Or Defeat : +1 Life", 1, 1, true),
        (5802, "Victory Or Defeat : +2 Life", 2, 1, true),
        (2944, "Victory Or Defeat : +2 Life", 2, 1, true),
        (1628, "Victory Or Defeat: - 1 Opp. Life Min 1", 1, 1, false),
        // The opposing reduction is a grammar since revision 33, so every same-shape record
        // whose printed text agrees with both of its numbers executes, including an id no
        // reviewed list names.
        (1386, "Victory Or Defeat: - 1 Opp. Life Min 0", 1, 0, false),
        (1726, "Victory Or Defeat: - 2 Opp. Life Min 1", 2, 1, false),
        (3367, "Victory Or Defeat: - 1 Opp. Life Min 0", 1, 0, false),
        (4331, "Victory Or Defeat: - 2 Opp. Life Min 4", 2, 4, false),
        (
            900_1386,
            "Victory Or Defeat: - 3 Opp. Life Min 2",
            3,
            2,
            false,
        ),
    ];

    for (id, description, life, minimum, owner_life) in cases {
        let registry = one_entry_registry(victory_or_defeat_life_entry(
            id,
            description,
            life,
            minimum,
            owner_life,
        ));
        let mut source = replay(875032, &catalog);
        clear_sources(&mut source);
        let selected_slot = usize::from(
            source.rounds[0]
                .plays
                .iter()
                .find(|play| play.engine_player == EnginePlayer::P1)
                .unwrap()
                .hand_index,
        );
        source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
            id,
            description: description.to_owned(),
        });
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        assert!(
            matches!(
                prepared.preparation()[PlayerId::P1][selected_slot].ability,
                CombatStatProjectionDispositionV1::ExecutePostRound {
                    effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainLifeOnVictoryOrDefeat { life: actual },
                    ..
                } if owner_life && actual == life
            ) || matches!(
                prepared.preparation()[PlayerId::P1][selected_slot].ability,
                CombatStatProjectionDispositionV1::ExecutePostRound {
                    effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryOrDefeat { life: actual_life, minimum: actual_minimum },
                    ..
                } if !owner_life && actual_life == life && actual_minimum == minimum
            ),
            "id={id}"
        );
    }

    // A reviewed own-Life gain stays identity-and-shape locked, and the opposing reduction
    // keeps the two-sided boundary every post-round grammar has: a complete shape whose
    // printed numbers disagree with it is a selected hazard, not an inert no-op.
    let mut malformed =
        victory_or_defeat_life_entry(1396, "Victory Or Defeat : +1 Life", 1, 1, true);
    malformed["abilityData"]["valueMin"] = serde_json::json!(0);
    let disagreeing = victory_or_defeat_life_entry(
        900_1396,
        "Victory Or Defeat: - 2 Opp. Life Min 1",
        3,
        1,
        false,
    );
    // Uuber's id is pinned to its own magnitude however the record is printed.
    let reserved =
        victory_or_defeat_life_entry(1628, "Victory Or Defeat: - 2 Opp. Life Min 1", 2, 1, false);
    for (id, description, entry) in [
        (1396, "Victory Or Defeat : +1 Life", malformed),
        (
            900_1396,
            "Victory Or Defeat: - 2 Opp. Life Min 1",
            disagreeing,
        ),
        (1628, "Victory Or Defeat: - 2 Opp. Life Min 1", reserved),
    ] {
        let registry = one_entry_registry(entry);
        let mut source = replay(875032, &catalog);
        clear_sources(&mut source);
        let selected_slot = usize::from(
            source.rounds[0]
                .plays
                .iter()
                .find(|play| play.engine_player == EnginePlayer::P1)
                .unwrap()
                .hand_index,
        );
        source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
            id,
            description: description.to_owned(),
        });
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        assert!(
            matches!(
                prepared.preparation()[PlayerId::P1][selected_slot].ability,
                CombatStatProjectionDispositionV1::Disabled {
                    reason: CombatStatDisabledReasonV1::UnsupportedPostRoundResourceEffect { .. },
                    ..
                }
            ),
            "id={id}"
        );
        assert!(
            matches!(
                prepared.new_game().card_plans()[PlayerId::P1][selected_slot].ability,
                CombatStatSourcePlanV1::RejectIfSelected { source_id } if source_id == id
            ),
            "id={id}"
        );
    }
}

#[test]
fn victory_or_defeat_life_server_evidence_preserves_liveness_and_stop_semantics() {
    let catalog = catalog();
    let registry = registry();
    let source_in_round = |prepared: &CombatStatDiagnosticReplayV1,
                           round_index: usize,
                           card_id: u32| {
        let round = &prepared.replay().rounds[round_index];
        let play = round
            .plays
            .iter()
            .find(|play| play.card.id == card_id)
            .unwrap_or_else(|| panic!("battle {} is missing card {card_id}", prepared.battle_id()));
        let owner = match play.engine_player {
            EnginePlayer::P1 => PlayerId::P1,
            EnginePlayer::P2 => PlayerId::P2,
        };
        (owner, usize::from(play.hand_index))
    };

    // Independent owner-Life evidence: Scott's +1 is paid after a win, and Kora Mail Ld's
    // distinct +2 is paid after a clean loss.
    for (battle_id, round_index, card_id, life, won, gain) in [
        (947370, 0, 820, 13, true, 1),
        (1091235, 2, 1676, 7, false, 2),
    ] {
        let prepared = diagnostic(battle_id, &catalog, &registry);
        let (owner, slot) = source_in_round(&prepared, round_index, card_id);
        let round = &prepared.replay().rounds[round_index];
        assert_eq!(
            round.expected_player_states[owner.index()].life,
            life,
            "battle {battle_id}"
        );
        assert_eq!(
            round.expected_card_results[owner.index()].unwrap().won,
            won,
            "battle {battle_id}"
        );
        assert!(matches!(
            prepared.preparation()[owner][slot].ability,
            CombatStatProjectionDispositionV1::ExecutePostRound {
                effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainLifeOnVictoryOrDefeat { life: actual },
                ..
            } if actual == gain
        ));
    }

    // Reprisal is SOA, so it suppresses Scott's selected ability: the owner reaches 1,
    // not 2, after Spidee's six damage.
    let stopped = diagnostic(1069721, &catalog, &registry);
    let (owner, slot) = source_in_round(&stopped, 2, 820);
    let round = &stopped.replay().rounds[2];
    assert!(!round.expected_card_results[owner.index()].unwrap().won);
    assert_eq!(round.expected_player_states[owner.index()].life, 1);
    assert!(matches!(
        stopped.preparation()[owner][slot].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainLifeOnVictoryOrDefeat { life: 1 },
            ..
        }
    ));

    // Uuber directs the effect at the live opponent after either outcome.  The last case
    // proves that an owner KO does not cancel the opponent reduction (11 -> 10).
    for (battle_id, round_index, owner_life, opponent_life, won) in [
        (924485, 0, 12, 9, true),
        (867116, 0, 6, 11, false),
        (956608, 3, 0, 10, false),
    ] {
        let prepared = diagnostic(battle_id, &catalog, &registry);
        let (owner, slot) = source_in_round(&prepared, round_index, 1788);
        let round = &prepared.replay().rounds[round_index];
        assert_eq!(
            round.expected_card_results[owner.index()].unwrap().won,
            won,
            "battle {battle_id}"
        );
        assert_eq!(
            round.expected_player_states[owner.index()].life,
            owner_life,
            "battle {battle_id}"
        );
        assert_eq!(
            round.expected_player_states[owner.other().index()].life,
            opponent_life,
            "battle {battle_id}"
        );
        assert!(matches!(
            prepared.preparation()[owner][slot].ability,
            CombatStatProjectionDispositionV1::ExecutePostRound {
                effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryOrDefeat { life: 1, minimum: 1 },
                ..
            }
        ));
    }

    // Capture packets expose Copy's resulting source on the receiving card.  Both an
    // ability and a bonus source must remain live under the exact VOD-Life classifier.
    for (battle_id, card_id, bonus) in [(924573, 2364, false), (924669, 2295, true)] {
        let prepared = diagnostic(battle_id, &catalog, &registry);
        let (owner, slot) = source_in_round(&prepared, 0, card_id);
        let disposition = if bonus {
            &prepared.preparation()[owner][slot].bonus
        } else {
            &prepared.preparation()[owner][slot].ability
        };
        assert!(
            matches!(
                disposition,
                CombatStatProjectionDispositionV1::ExecutePostRound {
                    identity,
                    effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryOrDefeat { life: 1, minimum: 1 },
                    ..
                } if identity.id == 1628
            ),
            "battle {battle_id}"
        );
    }
}

#[test]
fn equalizer_opponent_life_is_exactly_the_audited_post_round_family() {
    let catalog = catalog();
    const DESCRIPTION: &str = "Equalizer: - 1 Opp. Life Min 2";

    // Copy can materialize either reviewed registry identity in either source slot.
    for (id, source_kind) in [
        (1415, CombatStatEffectSourceV1::Ability),
        (1415, CombatStatEffectSourceV1::Bonus),
        (4458, CombatStatEffectSourceV1::Ability),
        (4458, CombatStatEffectSourceV1::Bonus),
    ] {
        let registry = one_entry_registry(equalizer_opponent_life_entry(id, DESCRIPTION));
        let mut source = replay(875032, &catalog);
        clear_sources(&mut source);
        let selected_slot = usize::from(
            source.rounds[0]
                .plays
                .iter()
                .find(|play| play.engine_player == EnginePlayer::P1)
                .unwrap()
                .hand_index,
        );
        let modifier = Some(SourceModifier {
            id,
            description: DESCRIPTION.to_owned(),
        });
        if source_kind == CombatStatEffectSourceV1::Ability {
            source.players[0].hand[selected_slot].source_ability = modifier;
        } else {
            source.players[0].hand[selected_slot].source_bonus = modifier;
        }
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        let disposition = if source_kind == CombatStatEffectSourceV1::Ability {
            &prepared.preparation()[PlayerId::P1][selected_slot].ability
        } else {
            &prepared.preparation()[PlayerId::P1][selected_slot].bonus
        };
        assert!(matches!(
            disposition,
            CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars { per_star: 1, minimum: 2 },
                ..
            } if identity.id == id
        ));
        let plan = if source_kind == CombatStatEffectSourceV1::Ability {
            prepared.new_game().card_plans()[PlayerId::P1][selected_slot].ability
        } else {
            prepared.new_game().card_plans()[PlayerId::P1][selected_slot].bonus
        };
        assert!(matches!(
            plan,
            CombatStatSourcePlanV1::Execute {
                source_id,
                predicate: CombatStatPredicateV1::Always,
                effect: urban_recreation_rust::engine::CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars { per_star: 1, minimum: 2 },
            } if source_id == id
        ));
    }

    // Identity, player-gain, and nested/adjacent Equalizer Life records are selected
    // hazards, never disabled no-ops.
    let mut player_gain = equalizer_opponent_life_entry(900_1415, "Equalizer: +1 Life");
    player_gain["abilityData"]["sideAffected"] = serde_json::json!("player");
    player_gain["abilityData"]["attributeAction"] = serde_json::json!("increase");
    let mut nested =
        equalizer_opponent_life_entry(900_4458, "Night: Equalizer: - 1 Opp. Life Min 2");
    nested["abilityData"]["positionRequirement"] = serde_json::json!("attacker");
    let mut malformed = equalizer_opponent_life_entry(1415, DESCRIPTION);
    malformed["abilityData"]["valueMin"] = serde_json::json!(1);
    for (id, description, entry) in [
        (1415, DESCRIPTION, malformed),
        (
            5793,
            "Equalizer: - 1 Opp. Life Min 0",
            equalizer_opponent_life_entry(5793, "Equalizer: - 1 Opp. Life Min 0"),
        ),
        (900_1415, "Equalizer: +1 Life", player_gain),
        (900_4458, "Night: Equalizer: - 1 Opp. Life Min 2", nested),
    ] {
        let registry = one_entry_registry(entry);
        let mut source = replay(875032, &catalog);
        clear_sources(&mut source);
        source.players[0].hand[0].source_ability = Some(SourceModifier {
            id,
            description: description.to_owned(),
        });
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        assert!(matches!(
            prepared.new_game().card_plans()[PlayerId::P1][0].ability,
            CombatStatSourcePlanV1::RejectIfSelected { source_id } if source_id == id
        ));
    }
}

#[test]
fn equalizer_opponent_life_copy_bonus_preparation_preserves_924669_provenance() {
    // 924669 has earlier out-of-slice post-round work, so it remains preparation evidence
    // rather than an immutable executable prefix. Its Copy result is nevertheless exact.
    let prepared = diagnostic(924669, &catalog(), &registry());
    let round = &prepared.replay().rounds[2];
    assert!(matches!(
        prepared.preparation()[PlayerId::P2][usize::from(
            round
                .plays
                .iter()
                .find(|play| play.engine_player == EnginePlayer::P2)
                .expect("924669/r2 must select Copy carrying 1415 as Bonus")
                .hand_index
        )]
        .bonus,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            ref identity,
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars { per_star: 1, minimum: 2 },
            ..
        } if identity.id == 1415
    ));
    assert_eq!(round.expected_player_states[PlayerId::P1.index()].life, 2);
}

#[test]
fn uuber_victory_or_defeat_life_unlocks_925719_through_round_two() {
    let catalog = catalog();
    let registry = registry();
    let report = diagnostic(925719, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(3)
        .expect("925719/r0-r2 must remain exact after Uuber's VOD-Life admission");
    assert_eq!(report.rounds.len(), 3);

    let round = &report.rounds[0];
    let owner = PlayerId::ALL
        .into_iter()
        .find(|player| matches!(
            &round.selected[*player].ability,
            CombatStatProjectionDispositionV1::ExecutePostRound {
                identity,
                effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryOrDefeat { life: 1, minimum: 1 },
                ..
            } if identity.id == 1628
        ))
        .expect("Uuber's exact source must be selected in 925719/r0");
    assert!(!round.round.cards[owner].won);
    // Prince Jr deals three to Uuber; even on this loss Uuber's effect reduces the live
    // opponent from 12 to 11, matching the server's 9 / 11 endpoint.
    assert_eq!(round.round.players[owner].life, 9);
    assert_eq!(round.round.players[owner.other()].life, 11);
}

#[test]
fn anita_courage_damage_to_life_is_exactly_ability_274_on_anita_level_three() {
    const ID: u32 = 274;
    const DESCRIPTION: &str = "Courage: +1 Life Per Dmg";
    let catalog = catalog();
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    source.rounds.clear();
    source.players[0].hand[0].key = CardKey::new(448, 3);
    source.players[0].hand[0].source_ability = Some(SourceModifier {
        id: ID,
        description: DESCRIPTION.to_owned(),
    });
    let prepared = CombatStatDiagnosticReplayV1::new(
        source,
        &catalog,
        &one_entry_registry(anita_courage_damage_to_life_entry(ID, DESCRIPTION)),
        PROJECTION,
    )
    .expect("the direct Anita Ability must prepare");
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][0].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            ref identity,
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
            ..
        } if identity.id == ID
    ));
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][0].ability,
        CombatStatSourcePlanV1::Execute {
            source_id: ID,
            predicate: CombatStatPredicateV1::OwnerMovesFirst,
            effect: urban_recreation_rust::engine::CombatStatEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
        }
    ));
}

#[test]
fn anita_courage_damage_to_life_variants_reject_or_remain_disabled_fail_closed() {
    const ID: u32 = 274;
    const DESCRIPTION: &str = "Courage: +1 Life Per Dmg";
    #[derive(Clone, Copy)]
    enum Mutation {
        CopyAbility,
        ValueMinimumOne,
    }
    let catalog = catalog();

    // A Bonus placement, copied alias, same-text alias, and malformed direct record must
    // never become an inert selected no-op. Each is retained in preparation but rejects
    // when selected.
    for (label, id, bonus, mutate) in [
        ("bonus", ID, true, None),
        ("copied alias", 900_274, false, Some(Mutation::CopyAbility)),
        ("same-text alias", 900_274, false, None),
        (
            "malformed direct record",
            ID,
            false,
            Some(Mutation::ValueMinimumOne),
        ),
    ] {
        let mut source = replay(875375, &catalog);
        clear_sources(&mut source);
        source.rounds.clear();
        let player = PlayerId::P2;
        source.players[player.index()].hand[0].key = CardKey::new(448, 3);
        let modifier = Some(SourceModifier {
            id,
            description: DESCRIPTION.to_owned(),
        });
        if bonus {
            source.players[player.index()].hand[0].source_bonus = modifier;
        } else {
            source.players[player.index()].hand[0].source_ability = modifier;
        }
        let mut entry = anita_courage_damage_to_life_entry(id, DESCRIPTION);
        if let Some(mutation) = mutate {
            match mutation {
                Mutation::CopyAbility => {
                    entry["abilityData"]["specialAction"] = serde_json::json!("copy_ability");
                }
                Mutation::ValueMinimumOne => {
                    entry["abilityData"]["valueMin"] = serde_json::json!(1);
                }
            }
        }
        let prepared = CombatStatDiagnosticReplayV1::new(
            source,
            &catalog,
            &one_entry_registry(entry),
            PROJECTION,
        )
        .unwrap_or_else(|error| panic!("{label} must prepare fail-closed: {error}"));
        let disposition = if bonus {
            &prepared.preparation()[player][0].bonus
        } else {
            &prepared.preparation()[player][0].ability
        };
        assert!(
            matches!(
                disposition,
                CombatStatProjectionDispositionV1::Disabled { .. }
            ),
            "{label}"
        );
        let plan = if bonus {
            prepared.new_game().card_plans()[player][0].bonus
        } else {
            prepared.new_game().card_plans()[player][0].ability
        };
        assert!(
            matches!(
                plan,
                CombatStatSourcePlanV1::RejectIfSelected { source_id } if source_id == id
            ),
            "{label}"
        );
    }

    // A copied Anita effect on any other card reaches the engine's identity guard during
    // immutable plan validation, before it can execute even if that card is selected.
    let mut wrong_card = replay(875032, &catalog);
    clear_sources(&mut wrong_card);
    wrong_card.rounds.clear();
    wrong_card.players[0].hand[0].source_ability = Some(SourceModifier {
        id: ID,
        description: DESCRIPTION.to_owned(),
    });
    assert!(matches!(
        CombatStatDiagnosticReplayV1::new(
            wrong_card,
            &catalog,
            &one_entry_registry(anita_courage_damage_to_life_entry(ID, DESCRIPTION)),
            PROJECTION,
        ),
        Err(CombatStatDiagnosticPreparationErrorV1::EnginePlan(_))
    ));
}

#[test]
fn anita_server_replays_pin_active_normal_fury_and_losing_courage() {
    let catalog = catalog();
    let registry = registry();
    for (battle_id, prefix, round_index, expected_damage, expected_life, won) in [
        // Normal Courage win: 8 + 3 Anita Life from resolved damage = 11.
        (1059454, 4, 2, 3, 11, true),
        // Fury win: final resolved damage is five, and the captured endpoint is 17.
        (1089346, 4, 0, 5, 17, true),
        // Courage is live but loses, so it gains no Life.
        (1069813, 4, 0, 5, 10, false),
    ] {
        let report = diagnostic(battle_id, &catalog, &registry)
            .execute_combat_stat_diagnostic_v1_prefix(prefix)
            .unwrap_or_else(|error| panic!("Anita fixture {battle_id}/{prefix}: {error}"));
        let round = &report.rounds[round_index];
        let owner = PlayerId::ALL
            .into_iter()
            .find(|player| matches!(
                round.selected[*player].ability,
                CombatStatProjectionDispositionV1::ExecutePostRound {
                    identity: ref id,
                    effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
                    ..
                } if id.id == 274
            ))
            .unwrap_or_else(|| panic!("battle {battle_id} must select Anita"));
        assert_eq!(
            round.round.cards[owner].damage, expected_damage,
            "battle {battle_id}"
        );
        assert_eq!(round.round.cards[owner].won, won, "battle {battle_id}");
        assert_eq!(
            round.round.players[owner].life, expected_life,
            "battle {battle_id}"
        );
    }

    // `damageAfter` is a capture transport field rather than replay ground truth.  The
    // immutable adapter keeps the server's transient resolved `damage` (three here), so
    // this evidence must not be silently rewritten to the transport value five.
    let transport_evidence = diagnostic(875375, &catalog, &registry);
    let round = &transport_evidence.replay().rounds[1];
    let owner = PlayerId::P2;
    let slot = usize::from(
        round
            .plays
            .iter()
            .find(|play| play.engine_player == EnginePlayer::P2)
            .expect("875375/r1 has P2's Anita play")
            .hand_index,
    );
    assert!(matches!(
        transport_evidence.preparation()[owner][slot].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            identity: ref id,
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
            ..
        } if id.id == 274
    ));
    assert_eq!(
        round.expected_card_results[owner.index()]
            .expect("server result")
            .damage,
        3
    );
}

#[test]
fn argos_defeat_capped_pillz_is_exact_and_fails_closed_when_selected() {
    let catalog = catalog();
    const DESCRIPTION: &str = "Defeat: +2 Pillz Max. 11";
    for (id, source_kind, admitted) in [
        (1158, CombatStatEffectSourceV1::Ability, true),
        (1158, CombatStatEffectSourceV1::Bonus, false),
        (900_108, CombatStatEffectSourceV1::Ability, false),
    ] {
        let registry = one_entry_registry(argos_defeat_capped_pillz_entry(id, DESCRIPTION));
        let mut source = replay(875032, &catalog);
        clear_sources(&mut source);
        let selected_slot = usize::from(
            source.rounds[0]
                .plays
                .iter()
                .find(|play| play.engine_player == EnginePlayer::P1)
                .unwrap()
                .hand_index,
        );
        let modifier = Some(SourceModifier {
            id,
            description: DESCRIPTION.to_owned(),
        });
        if source_kind == CombatStatEffectSourceV1::Ability {
            source.players[0].hand[selected_slot].source_ability = modifier;
        } else {
            source.players[0].hand[selected_slot].source_bonus = modifier;
        }
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        let disposition = if source_kind == CombatStatEffectSourceV1::Ability {
            &prepared.preparation()[PlayerId::P1][selected_slot].ability
        } else {
            &prepared.preparation()[PlayerId::P1][selected_slot].bonus
        };
        assert_eq!(
            matches!(
                disposition,
                CombatStatProjectionDispositionV1::ExecutePostRound {
                    effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainTwoPillzOnDefeatMaxEleven,
                    ..
                }
            ),
            admitted,
            "id={id}, source={source_kind:?}"
        );
        if !admitted {
            assert!(matches!(
                disposition,
                CombatStatProjectionDispositionV1::Disabled {
                    reason: CombatStatDisabledReasonV1::UnsupportedPostRoundResourceEffect { .. },
                    ..
                }
            ));
            let plan = if source_kind == CombatStatEffectSourceV1::Ability {
                prepared.new_game().card_plans()[PlayerId::P1][selected_slot].ability
            } else {
                prepared.new_game().card_plans()[PlayerId::P1][selected_slot].bonus
            };
            assert!(matches!(
                plan,
                CombatStatSourcePlanV1::RejectIfSelected { source_id } if source_id == id
            ));
        }
    }

    let mut malformed = argos_defeat_capped_pillz_entry(1158, DESCRIPTION);
    malformed["abilityData"]["valueMax"] = serde_json::json!(12);
    let registry = one_entry_registry(malformed);
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    source.players[0].hand[0].source_ability = Some(SourceModifier {
        id: 1158,
        description: DESCRIPTION.to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][0].ability,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: 1158 }
    ));

    let registry = one_entry_registry(argos_defeat_capped_pillz_entry(1158, "+2 Pillz"));
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    source.players[0].hand[0].source_ability = Some(SourceModifier {
        id: 1158,
        description: "+2 Pillz".to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][0].ability,
        CombatStatSourcePlanV1::RejectIfSelected { source_id: 1158 }
    ));
}

#[test]
fn server_replay_pins_argos_after_cost_and_riots_bonus_arithmetic() {
    let report = diagnostic(1092909, &catalog(), &registry())
        .execute_combat_stat_diagnostic_v1_prefix(2)
        .unwrap();
    let round = &report.rounds[1];
    assert!(matches!(
        round.selected[PlayerId::P2].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound { ref identity, effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainTwoPillzOnDefeatMaxEleven, .. }
            if identity.id == 1158
    ));
    assert!(!round.round.cards[PlayerId::P2].won);
    // 9 carried - 2 paid = 7; Riots bonus first gives 8, then Argos gives 2.
    assert_eq!(round.round.players[PlayerId::P2].pillz, 10);
}

#[test]
fn server_replays_pin_static_and_dynamic_vod_pillz_arithmetic() {
    let catalog = catalog();
    let registry = registry();

    let bonnie_l2 = diagnostic(946288, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(1)
        .unwrap();
    let round = &bonnie_l2.rounds[0];
    assert!(matches!(
        round.selected[PlayerId::P2].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound { ref identity, .. }
            if identity.id == 5085
    ));
    assert!(!round.round.cards[PlayerId::P2].won);
    // 12 initial - 2 paid + 1 VOD = 11.
    assert_eq!(round.round.players[PlayerId::P2].pillz, 11);

    let bonnie_l1 = diagnostic(1092660, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(1)
        .unwrap();
    let round = &bonnie_l1.rounds[0];
    assert!(matches!(
        round.selected[PlayerId::P2].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound { ref identity, .. }
            if identity.id == 5520
    ));
    assert!(!round.round.cards[PlayerId::P2].won);
    // 12 initial - 1 paid + 1 VOD = 12.
    assert_eq!(round.round.players[PlayerId::P2].pillz, 12);

    let copied_and_stacked = diagnostic(1093500, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(2)
        .unwrap();
    let copied = &copied_and_stacked.rounds[0];
    assert!(matches!(
        copied.selected[PlayerId::P2].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound { ref identity, .. }
            if identity.id == 1034
    ));
    assert!(copied.round.cards[PlayerId::P2].won);
    // Dynamically copied Ability:1034: 12 initial - 1 paid + 1 VOD = 12.
    assert_eq!(copied.round.players[PlayerId::P2].pillz, 12);

    let stacked = &copied_and_stacked.rounds[1];
    assert!(matches!(
        stacked.selected[PlayerId::P1].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound { ref identity, .. }
            if identity.id == 1375
    ));
    assert!(matches!(
        stacked.selected[PlayerId::P1].bonus,
        CombatStatProjectionDispositionV1::ExecutePostRound { ref identity, .. }
            if identity.id == 1034
    ));
    assert!(!stacked.round.cards[PlayerId::P1].won);
    // 12 carried - 0 paid + Ability:1375 + Bonus:1034 = 14.
    assert_eq!(stacked.round.players[PlayerId::P1].pillz, 14);
}

#[test]
fn server_replays_pin_riots_post_round_pillz_for_wins_losses_zero_bets_and_a_ko() {
    let catalog = catalog();
    let registry = registry();

    let knockout = diagnostic(1058366, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(3)
        .unwrap();
    for (round, attack, won, life, pillz) in [(0, 50, true, 12, 8), (2, 16, false, 0, 9)] {
        let report = &knockout.rounds[round];
        assert!(matches!(
            report.selected[PlayerId::P1].bonus,
            CombatStatProjectionDispositionV1::ExecutePostRound {
                ref identity,
                effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainOnePillzOnVictoryOrDefeat,
                ..
            } if identity.id == 1034
        ));
        assert_eq!(report.round.cards[PlayerId::P1].attack, attack);
        assert_eq!(report.round.cards[PlayerId::P1].won, won);
        assert_eq!(report.round.players[PlayerId::P1].life, life);
        assert_eq!(report.round.players[PlayerId::P1].pillz, pillz);
    }
    assert!(matches!(
        knockout.rounds[1].selected[PlayerId::P1].bonus,
        CombatStatProjectionDispositionV1::Execute { ref identity, .. } if identity.id == 37
    ));
    assert_eq!(knockout.rounds[1].round.cards[PlayerId::P1].attack, 12);
    assert!(!knockout.rounds[1].round.cards[PlayerId::P1].won);
    assert_eq!(knockout.rounds[1].round.players[PlayerId::P1].pillz, 8);
    assert_eq!(
        knockout.final_position.status,
        urban_recreation_rust::engine::MatchStatus::Won(PlayerId::P2)
    );

    let losses = diagnostic(1061897, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(4)
        .unwrap();
    for (round, attack, won, pillz) in [
        (0, 30, false, 9),
        (1, 36, true, 5),
        (2, 5, false, 6),
        (3, 28, false, 1),
    ] {
        let report = &losses.rounds[round];
        assert!(matches!(
            report.selected[PlayerId::P1].bonus,
            CombatStatProjectionDispositionV1::ExecutePostRound {
                ref identity,
                effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainOnePillzOnVictoryOrDefeat,
                ..
            } if identity.id == 1034
        ));
        assert_eq!(report.round.cards[PlayerId::P1].attack, attack);
        assert_eq!(report.round.cards[PlayerId::P1].won, won);
        assert_eq!(report.round.players[PlayerId::P1].pillz, pillz);
    }
}

#[test]
fn index_grammar_is_exact_and_nested_contexts_fail_closed() {
    let catalog = catalog();
    const EFFECT_ID: u32 = 900_102;
    let cases = [
        (
            "Asymmetry: Night: -2 Opp Power, Min 3",
            "asymmetry",
            "both",
            false,
        ),
        ("Symmetry: -2 Opp Power, Min 3", "asymmetry", "both", false),
        ("Asymmetry: -3 Opp Power, Min 3", "asymmetry", "both", false),
        (
            "Asymmetry: -2 Opp Power, Min 3",
            "asymmetry",
            "attacker",
            false,
        ),
        ("Asymmetry: -2 Opp Power, Min 3", "asymmetry", "both", true),
        ("Symmetry: -2 Opp Power, Min 3", "symmetry", "both", true),
    ];
    for (description, index, position, admitted) in cases {
        for bonus in [false, true] {
            let registry = one_entry_registry(index_numeric_entry(
                EFFECT_ID,
                description,
                index,
                position,
                2,
                3,
            ));
            let mut source = replay(875032, &catalog);
            clear_sources(&mut source);
            let selected_slot = usize::from(
                source.rounds[0]
                    .plays
                    .iter()
                    .find(|play| play.engine_player == EnginePlayer::P1)
                    .unwrap()
                    .hand_index,
            );
            let modifier = Some(SourceModifier {
                id: EFFECT_ID,
                description: description.to_owned(),
            });
            if bonus {
                source.players[0].hand[selected_slot].source_bonus = modifier;
            } else {
                source.players[0].hand[selected_slot].source_ability = modifier;
            }
            let prepared =
                CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
            assert_eq!(
                matches!(
                    if bonus {
                        &prepared.preparation()[PlayerId::P1][selected_slot].bonus
                    } else {
                        &prepared.preparation()[PlayerId::P1][selected_slot].ability
                    },
                    CombatStatProjectionDispositionV1::Execute { .. }
                ),
                admitted,
                "source={} {description}",
                if bonus { "bonus" } else { "ability" }
            );
            if !admitted {
                assert!(matches!(
                    prepared.execute_combat_stat_diagnostic_v1_prefix(1),
                    Err(CombatStatDiagnosticReplayErrorV1::Engine { .. })
                ));
            }
        }
    }

    for (description, index, side, attribute, action, value, minimum, predicate) in [
        (
            "Asymmetry: Power And Damage + 3",
            "asymmetry",
            "player",
            "pwr&dmg",
            "increase",
            3,
            0,
            CombatStatPredicateV1::SelectedHandSlotsDiffer,
        ),
        (
            "Symmetry: -2 Opp Pow. And Dam., Min 1",
            "symmetry",
            "opponent",
            "pwr&dmg",
            "decrease",
            2,
            1,
            CombatStatPredicateV1::SelectedHandSlotsMatch,
        ),
    ] {
        let mut entry = index_numeric_entry(EFFECT_ID, description, index, "both", value, minimum);
        entry["abilityData"]["sideAffected"] = serde_json::json!(side);
        entry["abilityData"]["attributeAffected"] = serde_json::json!(attribute);
        entry["abilityData"]["attributeAction"] = serde_json::json!(action);
        let registry = one_entry_registry(entry);
        let mut source = replay(875032, &catalog);
        clear_sources(&mut source);
        let selected_slot = usize::from(
            source.rounds[0]
                .plays
                .iter()
                .find(|play| play.engine_player == EnginePlayer::P1)
                .unwrap()
                .hand_index,
        );
        source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
            id: EFFECT_ID,
            description: description.to_owned(),
        });
        let prepared =
            CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
        assert!(matches!(
            prepared.preparation()[PlayerId::P1][selected_slot].ability,
            CombatStatProjectionDispositionV1::Execute {
                predicate: actual,
                ..
            } if actual == predicate
        ));
    }

    let mut nested = index_numeric_entry(
        EFFECT_ID,
        "Asymmetry: -2 Opp Power, Min 3",
        "asymmetry",
        "both",
        2,
        3,
    );
    nested["abilityData"]["currentRoundRequirement"] = serde_json::json!("win");
    let nested_registry = one_entry_registry(nested);
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    let selected_slot = usize::from(
        source.rounds[0]
            .plays
            .iter()
            .find(|play| play.engine_player == EnginePlayer::P1)
            .unwrap()
            .hand_index,
    );
    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: EFFECT_ID,
        description: "Asymmetry: -2 Opp Power, Min 3".to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &nested_registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.execute_combat_stat_diagnostic_v1_prefix(1),
        Err(CombatStatDiagnosticReplayErrorV1::Engine { .. })
    ));

    // An unadmitted conditional control still rejects before predicate evaluation rather
    // than becoming a successful no-op. Since revision 47 `Symmetry: Stop Opp. Bonus` is
    // admitted, so the clan-gated `Asymm.:` Stop stands in for it.
    let registry = registry();
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: 4999,
        description: "[clan:31][clan:46][clan:54][clan:49] Asymm.: Stop Opp. Ability".to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.execute_combat_stat_diagnostic_v1_prefix(1),
        Err(CombatStatDiagnosticReplayErrorV1::Engine { .. })
    ));
}

#[test]
fn round_scaled_grammar_is_exact_and_nested_contexts_fail_closed() {
    let catalog = catalog();
    const EFFECT_ID: u32 = 900_103;
    let cases = [
        (
            "Growth: -2 Opp Power, Min 3",
            true,
            false,
            "both",
            "any",
            false,
            Some(MagnitudeMultiplierV1::Growth),
        ),
        (
            "Degrowth: -2 Opp Power, Min 3",
            false,
            true,
            "both",
            "any",
            false,
            Some(MagnitudeMultiplierV1::Degrowth),
        ),
        (
            "Growth: -2 Opp Power, Min 3",
            false,
            true,
            "both",
            "any",
            false,
            None,
        ),
        (
            "Growth: -2 Opp Power, Min 3",
            true,
            true,
            "both",
            "any",
            false,
            None,
        ),
        (
            "Night: Growth: -2 Opp Power, Min 3",
            true,
            false,
            "both",
            "any",
            false,
            None,
        ),
        (
            "Growth: -2 Opp Power, Min 3",
            true,
            false,
            "attacker",
            "any",
            false,
            None,
        ),
        (
            "Growth: -2 Opp Power, Min 3",
            true,
            false,
            "both",
            "win",
            false,
            None,
        ),
        (
            "Growth: -2 Opp Power, Min 3",
            true,
            false,
            "both",
            "any",
            true,
            None,
        ),
        (
            "Growth: -1 Opp Power, Min 3",
            true,
            false,
            "both",
            "any",
            false,
            None,
        ),
    ];
    for (description, growth, degrowth, position, current, support, admitted) in cases {
        for bonus in [false, true] {
            let mut entry =
                round_scaled_numeric_entry(EFFECT_ID, description, growth, degrowth, 2, 3);
            entry["abilityData"]["positionRequirement"] = serde_json::json!(position);
            entry["abilityData"]["currentRoundRequirement"] = serde_json::json!(current);
            entry["abilityData"]["isSupport"] = serde_json::json!(support);
            let registry = one_entry_registry(entry);
            let mut source = replay(875032, &catalog);
            clear_sources(&mut source);
            let selected_slot = usize::from(
                source.rounds[0]
                    .plays
                    .iter()
                    .find(|play| play.engine_player == EnginePlayer::P1)
                    .unwrap()
                    .hand_index,
            );
            let modifier = Some(SourceModifier {
                id: EFFECT_ID,
                description: description.to_owned(),
            });
            if bonus {
                source.players[0].hand[selected_slot].source_bonus = modifier;
            } else {
                source.players[0].hand[selected_slot].source_ability = modifier;
            }
            let prepared =
                CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
            let disposition = if bonus {
                &prepared.preparation()[PlayerId::P1][selected_slot].bonus
            } else {
                &prepared.preparation()[PlayerId::P1][selected_slot].ability
            };
            assert_eq!(
                match disposition {
                    CombatStatProjectionDispositionV1::Execute {
                        effect:
                            SupportedEffectV1::ModifyCombatStat {
                                multiplier: actual, ..
                            },
                        predicate: CombatStatPredicateV1::Always,
                        ..
                    } => Some(*actual),
                    CombatStatProjectionDispositionV1::Absent
                    | CombatStatProjectionDispositionV1::Execute { .. }
                    | CombatStatProjectionDispositionV1::ExecutePostRound { .. }
                    | CombatStatProjectionDispositionV1::Disabled { .. } => None,
                },
                admitted,
                "source={} {description}",
                if bonus { "bonus" } else { "ability" }
            );
            if admitted.is_none() {
                assert!(matches!(
                    prepared.execute_combat_stat_diagnostic_v1_prefix(1),
                    Err(CombatStatDiagnosticReplayErrorV1::Engine { .. })
                ));
            }
        }
    }
}

#[test]
fn equalizer_grammar_is_exact_and_nested_contexts_fail_closed() {
    let catalog = catalog();
    const EFFECT_ID: u32 = 900_104;
    let cases = [
        (
            "Equalizer: -2 Opp Power, Min 3",
            true,
            "both",
            "any",
            false,
            Some(MagnitudeMultiplierV1::OpponentStars),
        ),
        (
            "Equalizer: Power +2",
            true,
            "both",
            "any",
            false,
            Some(MagnitudeMultiplierV1::OpponentStars),
        ),
        (
            "Equalizer: -1 Opp Power, Min 3",
            true,
            "both",
            "any",
            false,
            None,
        ),
        (
            "Equalizer: -2 Opp Power, Min 3",
            false,
            "both",
            "any",
            false,
            None,
        ),
        (
            "Equalizer:-2 Opp Power, Min 3",
            true,
            "both",
            "any",
            false,
            None,
        ),
        (
            "Equalizer: -2 Opp Power, Min 3",
            true,
            "attacker",
            "any",
            false,
            None,
        ),
        (
            "Equalizer: -2 Opp Power, Min 3",
            true,
            "both",
            "win",
            false,
            None,
        ),
        (
            "Equalizer: -2 Opp Power, Min 3",
            true,
            "both",
            "any",
            true,
            None,
        ),
    ];
    for (description, linked, position, current, growth, admitted) in cases {
        for bonus in [false, true] {
            let mut entry = equalizer_numeric_entry(EFFECT_ID, description, 2, 3);
            entry["abilityData"]["isOppStarsLinked"] = serde_json::json!(linked);
            entry["abilityData"]["positionRequirement"] = serde_json::json!(position);
            entry["abilityData"]["currentRoundRequirement"] = serde_json::json!(current);
            entry["abilityData"]["isOverdrive"] = serde_json::json!(growth);
            if description == "Equalizer: Power +2" {
                entry["abilityData"]["sideAffected"] = serde_json::json!("player");
                entry["abilityData"]["attributeAction"] = serde_json::json!("increase");
                entry["abilityData"]["valueMin"] = serde_json::json!(0);
            }
            let registry = one_entry_registry(entry);
            let mut source = replay(875032, &catalog);
            clear_sources(&mut source);
            let selected_slot = usize::from(
                source.rounds[0]
                    .plays
                    .iter()
                    .find(|play| play.engine_player == EnginePlayer::P1)
                    .unwrap()
                    .hand_index,
            );
            let modifier = Some(SourceModifier {
                id: EFFECT_ID,
                description: description.to_owned(),
            });
            if bonus {
                source.players[0].hand[selected_slot].source_bonus = modifier;
            } else {
                source.players[0].hand[selected_slot].source_ability = modifier;
            }
            let prepared =
                CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
            let disposition = if bonus {
                &prepared.preparation()[PlayerId::P1][selected_slot].bonus
            } else {
                &prepared.preparation()[PlayerId::P1][selected_slot].ability
            };
            assert_eq!(
                match disposition {
                    CombatStatProjectionDispositionV1::Execute {
                        effect:
                            SupportedEffectV1::ModifyCombatStat {
                                multiplier: actual, ..
                            },
                        predicate: CombatStatPredicateV1::Always,
                        ..
                    } => Some(*actual),
                    CombatStatProjectionDispositionV1::Absent
                    | CombatStatProjectionDispositionV1::Execute { .. }
                    | CombatStatProjectionDispositionV1::ExecutePostRound { .. }
                    | CombatStatProjectionDispositionV1::Disabled { .. } => None,
                },
                admitted,
                "source={} {description}",
                if bonus { "bonus" } else { "ability" }
            );
            if admitted.is_none() {
                assert!(matches!(
                    prepared.execute_combat_stat_diagnostic_v1_prefix(1),
                    Err(CombatStatDiagnosticReplayErrorV1::Engine { .. })
                ));
            }
        }
    }

    let mut life = equalizer_numeric_entry(EFFECT_ID, "Equalizer: +2 Life", 2, 0);
    life["abilityData"]["sideAffected"] = serde_json::json!("player");
    life["abilityData"]["attributeAffected"] = serde_json::json!("life");
    life["abilityData"]["attributeAction"] = serde_json::json!("increase");
    let registry = one_entry_registry(life);
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    let selected_slot = usize::from(
        source.rounds[0]
            .plays
            .iter()
            .find(|play| play.engine_player == EnginePlayer::P1)
            .unwrap()
            .hand_index,
    );
    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: EFFECT_ID,
        description: "Equalizer: +2 Life".to_owned(),
    });
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][selected_slot].ability,
        CombatStatProjectionDispositionV1::Disabled { .. }
    ));
}

#[test]
fn server_replays_pin_growth_degrowth_clamping_and_cancellation() {
    let catalog = catalog();
    let registry = registry();

    let growth = diagnostic(1089513, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(2)
        .unwrap();
    assert!(matches!(
        growth.rounds[1].selected[PlayerId::P1].ability,
        CombatStatProjectionDispositionV1::Execute {
            effect: SupportedEffectV1::ModifyCombatStat {
                multiplier: MagnitudeMultiplierV1::Growth,
                ..
            },
            predicate: CombatStatPredicateV1::Always,
            ..
        }
    ));
    assert_eq!(growth.rounds[1].round.cards[PlayerId::P1].attack, 16);

    let direct = diagnostic(874642, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(1)
        .unwrap();
    assert!(matches!(
        direct.rounds[0].selected[PlayerId::P2].ability,
        CombatStatProjectionDispositionV1::Execute {
            effect: SupportedEffectV1::ModifyCombatStat {
                multiplier: MagnitudeMultiplierV1::Degrowth,
                ..
            },
            ..
        }
    ));
    assert_eq!(direct.rounds[0].round.cards[PlayerId::P2].power, 7);
    assert_eq!(direct.rounds[0].round.cards[PlayerId::P2].damage, 5);
    assert_eq!(direct.rounds[0].round.cards[PlayerId::P2].attack, 28);

    let clamped = diagnostic(877812, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(1)
        .unwrap();
    assert!(matches!(
        clamped.rounds[0].selected[PlayerId::P1].ability,
        CombatStatProjectionDispositionV1::Execute {
            effect: SupportedEffectV1::ModifyCombatStat {
                multiplier: MagnitudeMultiplierV1::Degrowth,
                ..
            },
            ..
        }
    ));
    assert_eq!(clamped.rounds[0].round.cards[PlayerId::P2].damage, 1);

    let cancelled = diagnostic(878056, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(1)
        .unwrap();
    assert!(matches!(
        cancelled.rounds[0].selected[PlayerId::P1].ability,
        CombatStatProjectionDispositionV1::Execute {
            effect: SupportedEffectV1::ModifyCombatStat {
                multiplier: MagnitudeMultiplierV1::Degrowth,
                ..
            },
            ..
        }
    ));
    assert_eq!(cancelled.rounds[0].round.cards[PlayerId::P2].damage, 4);
}

#[test]
fn server_replays_pin_equalizer_to_the_selected_opponent_level() {
    let catalog = catalog();
    let registry = registry();

    let stopped_support = diagnostic(1059269, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(1)
        .unwrap();
    assert!(matches!(
        stopped_support.rounds[0].selected[PlayerId::P1].bonus,
        CombatStatProjectionDispositionV1::Execute {
            effect: SupportedEffectV1::ModifyCombatStat {
                multiplier: MagnitudeMultiplierV1::OpponentStars,
                ..
            },
            ..
        }
    ));
    assert_eq!(
        stopped_support.rounds[0].round.cards[PlayerId::P1].attack,
        7
    );
    assert_eq!(
        stopped_support.rounds[0].round.cards[PlayerId::P2].attack,
        5
    );

    let both_sources = diagnostic(1091585, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(1)
        .unwrap();
    assert!(matches!(
        both_sources.rounds[0].selected[PlayerId::P2].ability,
        CombatStatProjectionDispositionV1::Execute {
            effect: SupportedEffectV1::ModifyCombatStat {
                multiplier: MagnitudeMultiplierV1::OpponentStars,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        both_sources.rounds[0].selected[PlayerId::P2].bonus,
        CombatStatProjectionDispositionV1::Execute {
            effect: SupportedEffectV1::ModifyCombatStat {
                multiplier: MagnitudeMultiplierV1::OpponentStars,
                ..
            },
            ..
        }
    ));
    assert_eq!(both_sources.rounds[0].round.cards[PlayerId::P1].power, 10);
    assert_eq!(both_sources.rounds[0].round.cards[PlayerId::P1].attack, 51);
    assert_eq!(both_sources.rounds[0].round.cards[PlayerId::P2].power, 7);
    assert_eq!(both_sources.rounds[0].round.cards[PlayerId::P2].attack, 56);
}

#[test]
fn server_replays_pin_ordinary_support_ability_counts_and_arithmetic() {
    let catalog = catalog();
    let registry = registry();
    let cases = [
        (868094, 4297, 7, 2, 42),
        (875230, 266, 7, 4, 54),
        (877950, 412, 4, 5, 36),
    ];

    for (battle_id, ability_id, power, damage, attack) in cases {
        let report = diagnostic(battle_id, &catalog, &registry)
            .execute_combat_stat_diagnostic_v1_prefix(1)
            .unwrap();
        let round = &report.rounds[0];
        let player = PlayerId::ALL
            .into_iter()
            .find(|player| {
                matches!(
                    &round.selected[*player].ability,
                    CombatStatProjectionDispositionV1::Execute { identity, .. }
                        if identity.id == ability_id
                )
            })
            .unwrap_or_else(|| panic!("battle {battle_id} did not execute ability {ability_id}"));
        assert_eq!(round.selected[player].effective_clan_character_count, 4);
        assert_eq!(round.selected[player].source_ability_support_count, 4);
        assert_eq!(round.round.cards[player].power, power);
        assert_eq!(round.round.cards[player].damage, damage);
        assert_eq!(round.round.cards[player].attack, attack);
    }
}

#[test]
fn server_replays_pin_confidence_revenge_and_the_fixed_revenge_bonus() {
    let catalog = catalog();
    let registry = registry();

    let confidence = diagnostic(875032, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(2)
        .unwrap();
    let round = &confidence.rounds[1];
    let wesley = PlayerId::ALL
        .into_iter()
        .find(|player| {
            matches!(
                &round.selected[*player].ability,
                CombatStatProjectionDispositionV1::Execute {
                    identity,
                    predicate: CombatStatPredicateV1::OwnerWonPreviousRound,
                    ..
                } if identity.id == 520
            )
        })
        .unwrap();
    assert_eq!(round.round.cards[wesley.other()].power, 4);

    let revenge_reduction = diagnostic(945585, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(2)
        .unwrap();
    let round = &revenge_reduction.rounds[1];
    let lehrg = PlayerId::ALL
        .into_iter()
        .find(|player| {
            matches!(
                &round.selected[*player].ability,
                CombatStatProjectionDispositionV1::Execute {
                    identity,
                    predicate: CombatStatPredicateV1::OwnerLostPreviousRound,
                    ..
                } if identity.id == 585
            )
        })
        .unwrap();
    assert_eq!(round.round.cards[lehrg.other()].power, 4);

    let revenge_bonus = diagnostic(1023396, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(2)
        .unwrap();
    for (round_index, expected_power, expected_damage) in [(0, 8, 5), (1, 9, 4)] {
        let round = &revenge_bonus.rounds[round_index];
        let frozn = PlayerId::ALL
            .into_iter()
            .find(|player| {
                matches!(
                    &round.selected[*player].bonus,
                    CombatStatProjectionDispositionV1::Execute {
                        identity,
                        predicate: CombatStatPredicateV1::OwnerLostPreviousRound,
                        ..
                    } if identity.id == 801
                )
            })
            .unwrap();
        assert_eq!(round.round.cards[frozn].power, expected_power);
        assert_eq!(round.round.cards[frozn].damage, expected_damage);
    }

    let revenge_power_and_damage = diagnostic(874962, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(2)
        .unwrap();
    let round = &revenge_power_and_damage.rounds[1];
    let tina = PlayerId::ALL
        .into_iter()
        .find(|player| {
            matches!(
                &round.selected[*player].ability,
                CombatStatProjectionDispositionV1::Execute {
                    identity,
                    predicate: CombatStatPredicateV1::OwnerLostPreviousRound,
                    ..
                } if identity.id == 883
            )
        })
        .unwrap();
    assert_eq!(round.round.cards[tina].power, 5);
    assert_eq!(round.round.cards[tina].damage, 6);
}

#[test]
fn server_replays_pin_audited_defeat_recover_sources_and_resource_arithmetic() {
    let catalog = catalog();
    let registry = registry();

    let ordinary = diagnostic(901400, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(2)
        .unwrap();
    let round = &ordinary.rounds[1];
    let vortex = PlayerId::ALL
        .into_iter()
        .find(|player| {
            matches!(
                &round.selected[*player].bonus,
                CombatStatProjectionDispositionV1::ExecutePostRound {
                    identity,
                    ..
                } if identity.id == 577
            )
        })
        .unwrap();
    assert!(!round.round.cards[vortex].won);
    assert_eq!(round.round.players[vortex].pillz, 9); // 10 - 3 + ceil(3 * 2 / 3)
    assert_eq!(ordinary.final_position.players[vortex].pillz, 9);

    let ability = diagnostic(946400, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(1)
        .unwrap();
    let round = &ability.rounds[0];
    let ai_lycs = PlayerId::ALL
        .into_iter()
        .find(|player| {
            matches!(
                &round.selected[*player].ability,
                CombatStatProjectionDispositionV1::ExecutePostRound {
                    identity,
                    ..
                } if identity.id == 1418
            )
        })
        .unwrap();
    assert!(!round.round.cards[ai_lycs].won);
    assert_eq!(round.round.players[ai_lycs].pillz, 11); // 12 - 4 + ceil(4 * 2 / 3)

    // Arnie's only observed selection is Fury-inclusive. The preceding selected Spade
    // converts damage to Pillz (id 1090); since revision 31 that conversion executes, so
    // the two-round prefix replays exactly and 1024592 is a gate member above.
    let fury = diagnostic(1024592, &catalog, &registry);
    let arnie = PlayerId::ALL
        .into_iter()
        .find_map(|player| {
            fury.replay().players[player.index()]
                .hand
                .iter()
                .position(|card| {
                    card.source_ability
                        .as_ref()
                        .is_some_and(|source| source.id == 729)
                })
                .map(|slot| (player, slot))
        })
        .unwrap();
    assert!(matches!(
        fury.preparation()[arnie.0][arnie.1].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound { ref identity, .. }
            if identity.id == 729
    ));
    let spade = PlayerId::ALL
        .into_iter()
        .find_map(|player| {
            fury.replay().players[player.index()]
                .hand
                .iter()
                .position(|card| {
                    card.source_ability
                        .as_ref()
                        .is_some_and(|source| source.id == 1090)
                })
                .map(|slot| (player, slot))
        })
        .unwrap();
    assert!(matches!(
        fury.preparation()[spade.0][spade.1].ability,
        CombatStatProjectionDispositionV1::ExecutePostRound {
            ref identity,
            effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainPillzEqualToFinalDamageOnVictory,
            predicate: CombatStatPredicateV1::Always,
        } if identity.id == 1090
    ));
    let fury_report = fury.execute_combat_stat_diagnostic_v1_prefix(2).unwrap();
    // Spade: 12 - 10 for the Fury bet + 5 final Damage; Arnie then recovers ceil(8 * 2 / 3)
    // = 6 of his own Fury-inclusive 8 after losing round 1.
    assert_eq!(fury_report.rounds[0].round.players[spade.0].pillz, 7);
    assert_eq!(fury_report.rounds[1].round.players[arnie.0].pillz, 5);

    let stopped = diagnostic(945585, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(1)
        .unwrap();
    let round = &stopped.rounds[0];
    let deea = PlayerId::ALL
        .into_iter()
        .find(|player| {
            matches!(
                &round.selected[*player].bonus,
                CombatStatProjectionDispositionV1::ExecutePostRound {
                    identity,
                    ..
                } if identity.id == 577
            )
        })
        .unwrap();
    assert!(!round.round.cards[deea].won);
    assert_eq!(round.round.players[deea].pillz, 10); // Stop Opp. Bonus suppresses recovery.
}

#[test]
fn server_replays_pin_active_inactive_and_stopped_index_predicates() {
    let catalog = catalog();
    let registry = registry();

    let inactive = diagnostic(1011768, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(1)
        .unwrap();
    assert!(matches!(
        inactive.rounds[0].selected[PlayerId::P1].bonus,
        CombatStatProjectionDispositionV1::Execute {
            predicate: CombatStatPredicateV1::SelectedHandSlotsDiffer,
            ..
        }
    ));
    assert_eq!(inactive.rounds[0].round.cards[PlayerId::P1].damage, 3);

    let active = diagnostic(1011483, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(2)
        .unwrap();
    assert!(matches!(
        active.rounds[0].selected[PlayerId::P1].bonus,
        CombatStatProjectionDispositionV1::Execute {
            predicate: CombatStatPredicateV1::SelectedHandSlotsDiffer,
            ..
        }
    ));
    assert_eq!(active.rounds[0].round.cards[PlayerId::P1].damage, 5);
    assert!(matches!(
        active.rounds[1].selected[PlayerId::P1].ability,
        CombatStatProjectionDispositionV1::Execute {
            predicate: CombatStatPredicateV1::SelectedHandSlotsMatch,
            ..
        }
    ));
    assert_eq!(active.rounds[1].round.cards[PlayerId::P2].power, 4);

    let stopped = diagnostic(1011643, &catalog, &registry)
        .execute_combat_stat_diagnostic_v1_prefix(2)
        .unwrap();
    assert!(matches!(
        stopped.rounds[1].selected[PlayerId::P1].bonus,
        CombatStatProjectionDispositionV1::Execute {
            predicate: CombatStatPredicateV1::SelectedHandSlotsDiffer,
            ..
        }
    ));
    assert_eq!(stopped.rounds[1].round.cards[PlayerId::P1].damage, 4);
}
