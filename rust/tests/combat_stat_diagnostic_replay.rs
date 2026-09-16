use std::collections::BTreeSet;
use std::fs::File;
use std::path::PathBuf;

use urban_recreation_rust::catalog::CardCatalog;
use urban_recreation_rust::effect_registry::EffectRegistryV1;
use urban_recreation_rust::engine::{CombatStatPredicateV1, PlayerId};
use urban_recreation_rust::replay::{
    adapt_capture, CapturedGame, CombatStatDiagnosticPreparationErrorV1,
    CombatStatDiagnosticProjectionV1, CombatStatDiagnosticReplayErrorV1,
    CombatStatDiagnosticReplayV1, CombatStatDisabledReasonV1, CombatStatProjectionDispositionV1,
    CombatStatWholeHandHazardSourceV1, EnginePlayer, ReplayCaseV1, ReplayClassification,
    SourceModifier, COMBAT_STAT_DIAGNOSTIC_COMPILER_POLICY_SEMANTIC_REVISION_V1,
};

const COMBAT_STAT_PREFIX_FIXTURES: &[(u64, usize)] = &[
    (875032, 1),
    (875155, 1),
    (1088323, 1),
    (1081463, 1),
    (1089513, 1),
    (901400, 1),
    (874837, 2),
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
fn fixed_server_backed_gate_is_exactly_eight_sequential_prefix_rounds() {
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
    assert_eq!(rounds, 8);
    assert_eq!(
        execute_ids,
        BTreeSet::from([
            37, 42, 93, 156, 266, 612, 871, 916, 1163, 1372, 1536, 2299, 4216, 5026, 5273, 5763,
        ])
    );
    assert_eq!(disabled_ids, BTreeSet::from([274, 577, 4459]));
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
fn support_abilities_and_capped_increases_are_visible_but_disabled() {
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
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::SupportAbility { .. },
            ..
        }
    ));
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][1].ability,
        CombatStatProjectionDispositionV1::Disabled {
            reason: CombatStatDisabledReasonV1::CappedIncrease { .. },
            ..
        }
    ));

    for (id, description) in [(266, "Support: Attack +3"), (2969, "Power +6, Max. 8")] {
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
fn selected_degrowth_is_rejected_before_the_excluded_second_round() {
    let catalog = catalog();
    let registry = registry();
    let prepared = diagnostic(877812, &catalog, &registry);
    let error = prepared
        .execute_combat_stat_diagnostic_v1_prefix(2)
        .unwrap_err();
    let CombatStatDiagnosticReplayErrorV1::Engine { context, .. } = error else {
        panic!("expected selected Degrowth rejection")
    };
    assert_eq!(context.round, 0);
}

#[test]
fn selected_asymmetry_is_rejected_instead_of_fixture_specialized() {
    let catalog = catalog();
    let registry = registry();
    for battle_id in [1011643, 1011768] {
        let prepared = diagnostic(battle_id, &catalog, &registry);
        let error = prepared
            .execute_combat_stat_diagnostic_v1_prefix(1)
            .unwrap_err();
        let CombatStatDiagnosticReplayErrorV1::Engine {
            context, selected, ..
        } = error
        else {
            panic!("expected selected Asymmetry rejection for {battle_id}")
        };
        assert_eq!(context.round, 0);
        assert!(selected.0.iter().any(|card| matches!(
            card.bonus,
            CombatStatProjectionDispositionV1::Disabled { .. }
        )));
    }
}
