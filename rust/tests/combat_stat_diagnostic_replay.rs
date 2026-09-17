use std::collections::BTreeSet;
use std::fs::File;
use std::path::PathBuf;

use urban_recreation_rust::catalog::CardCatalog;
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

const COMBAT_STAT_PREFIX_FIXTURES: &[(u64, usize)] = &[
    (875032, 2),
    (875155, 1),
    (1088323, 2),
    (1089001, 1),
    (1024673, 2),
    (1081463, 1),
    (1089513, 2),
    (901400, 2),
    (874837, 2),
    (1011643, 2),
    (1011768, 1),
    (1011483, 2),
    (877812, 2),
    (874642, 1),
    (1059269, 1),
    (1091585, 1),
    (868094, 1),
    (875230, 1),
    (877950, 1),
    (945585, 2),
    (1023396, 2),
    (874962, 2),
    (946400, 1),
    (1058366, 3),
    (1061897, 4),
    (946288, 1),
    (1092660, 1),
    (1093500, 2),
    (1092909, 2),
    (877636, 4),
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

fn victory_life_entry(id: u32, life: u16) -> serde_json::Value {
    let mut entry = numeric_entry(id, &format!("+{life} Life"), "both", life, 0);
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("win");
    entry["abilityData"]["sideAffected"] = serde_json::json!("player");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("life");
    entry["abilityData"]["attributeAction"] = serde_json::json!("increase");
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

fn argos_defeat_capped_pillz_entry(id: u32, description: &str) -> serde_json::Value {
    let mut entry = numeric_entry(id, description, "both", 2, 0);
    entry["abilityData"]["valueMax"] = serde_json::json!(11);
    entry["abilityData"]["currentRoundRequirement"] = serde_json::json!("lose");
    entry["abilityData"]["sideAffected"] = serde_json::json!("player");
    entry["abilityData"]["attributeAffected"] = serde_json::json!("pillz");
    entry["abilityData"]["attributeAction"] = serde_json::json!("increase");
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
fn fixed_server_backed_gate_is_exactly_fifty_two_sequential_prefix_rounds() {
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
    assert_eq!(rounds, 52);
    assert_eq!(
        execute_ids,
        BTreeSet::from([
            6, 36, 37, 39, 40, 42, 56, 57, 73, 90, 93, 94, 130, 156, 257, 266, 310, 333, 377, 401,
            412, 520, 577, 578, 585, 612, 741, 801, 844, 871, 883, 888, 916, 980, 1034, 1047, 1158,
            1163, 1241, 1335, 1338, 1342, 1359, 1372, 1375, 1418, 1536, 1578, 1688, 1694, 1770,
            1844, 1845, 1848, 1850, 2299, 2329, 2412, 2535, 2881, 2965, 3677, 3864, 3865, 3897,
            4041, 4216, 4297, 4299, 4399, 4711, 4718, 4757, 5026, 5085, 5273, 5520, 5763, 5852,
        ])
    );
    assert_eq!(
        disabled_ids,
        BTreeSet::from([274, 809, 854, 1399, 1852, 2317, 4303, 4458, 4459, 4695, 5283,])
    );
    assert_eq!(absent, 2);
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
    assert_eq!(provenance.compiler_policy_semantic_revision, 14);
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
        14
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
        } if identity.id == 4635
    ));
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
        1330, 1735, 1805, 2535, 2556, 3197, 3475, 3719, 4068, 4297, 4593, 4824, 4839, 4857, 5483,
        5841,
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
    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: 425,
        description: "Courage: Stop Opp. Ability".to_owned(),
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
        assert!(matches!(
            prepared.preparation()[control_player][control_slot].ability,
            CombatStatProjectionDispositionV1::Disabled {
                reason: CombatStatDisabledReasonV1::UnsupportedPromisedControl { .. },
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
    let prepared =
        CombatStatDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.new_game().card_plans()[PlayerId::P1][0].bonus,
        CombatStatSourcePlanV1::Disabled { source_id: 900_107 }
    ));
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
        CombatStatProjectionDispositionV1::ExecutePostRound { ref identity, effect: urban_recreation_rust::engine::CombatStatPostRoundEffectV1::GainTwoPillzOnDefeatMaxEleven }
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

    // The selected slots differ, so Symmetry is false. Conditional controls still reject
    // before predicate evaluation rather than becoming a successful no-op.
    let registry = registry();
    let mut source = replay(875032, &catalog);
    clear_sources(&mut source);
    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: 4525,
        description: "Symmetry: Stop Opp. Bonus".to_owned(),
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
    // converts damage to Pillz (id 1090), still intentionally disabled, so this capture is
    // preparation evidence rather than a whole-prefix gate member.
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
        CombatStatProjectionDispositionV1::Disabled { ref identity, .. }
            if identity.id == 1090
    ));
    assert!(matches!(
        fury.execute_combat_stat_diagnostic_v1_prefix(2),
        Err(CombatStatDiagnosticReplayErrorV1::Mismatch { .. })
    ));

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
