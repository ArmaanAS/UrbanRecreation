use std::collections::BTreeSet;
use std::fs::File;
use std::path::PathBuf;

use urban_recreation_rust::catalog::CardCatalog;
use urban_recreation_rust::effect_registry::{
    AttributeAffectedV1, EffectRegistryV1, MagnitudeMultiplierV1, SupportedEffectV1,
};
use urban_recreation_rust::engine::{CombatStatPredicateV1, PlayerId};
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
    (1088323, 1),
    (1081463, 1),
    (1089513, 2),
    (901400, 1),
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
fn fixed_server_backed_gate_is_exactly_twenty_nine_sequential_prefix_rounds() {
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
                        CombatStatProjectionDispositionV1::Disabled { identity, .. } => {
                            disabled_ids.insert(identity.id);
                        }
                    }
                }
            }
        }
    }
    assert_eq!(rounds, 29);
    assert_eq!(
        execute_ids,
        BTreeSet::from([
            6, 37, 39, 40, 42, 56, 93, 130, 156, 266, 412, 520, 578, 585, 612, 741, 801, 871, 883,
            916, 980, 1163, 1241, 1338, 1342, 1359, 1372, 1536, 1578, 1694, 1770, 1844, 1845, 1848,
            1850, 2299, 2881, 3865, 3897, 4216, 4297, 4718, 4757, 5026, 5273, 5763,
        ])
    );
    assert_eq!(
        disabled_ids,
        BTreeSet::from([274, 377, 401, 577, 809, 854, 1399, 1852, 2317, 4303, 4458, 4459,])
    );
    assert_eq!(absent, 0);
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
    assert_eq!(provenance.compiler_policy_semantic_revision, 7);
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
        id: 41,
        description: "Stop Opp. Ability".to_owned(),
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
