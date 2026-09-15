use std::fs::File;
use std::path::PathBuf;

use urban_recreation_rust::catalog::{CardCatalog, CardKey};
use urban_recreation_rust::effect_registry::{CompiledEffectV1, EffectRegistryV1};
use urban_recreation_rust::engine::{MatchStatus, PlayerId};
use urban_recreation_rust::replay::{
    adapt_capture, BaseRulesReplay, CapturedGame, ClanBonusDiagnosticPreparationErrorV1,
    ClanBonusDiagnosticProjectionV1, ClanBonusDiagnosticReplayErrorV1, ClanBonusDiagnosticReplayV1,
    DiagnosticDisabledReasonV1, DiagnosticProjectionDispositionV1, EnginePlayer, ReplayCaseV1,
    ReplayClassification, SourceModifier,
};

const BASE_RULES_PREFIX_FIXTURES: &[(u64, usize)] = &[
    (874520, 1),
    (876464, 1),
    (878056, 1),
    (878120, 1),
    (924257, 1),
    (924890, 1),
    (925087, 1),
    (926367, 1),
    (943231, 1),
    (964352, 1),
    (1023946, 1),
    (1065231, 2),
    (1065673, 1),
    (1069506, 2),
    (1072885, 1),
    (1081879, 1),
    (1087884, 1),
    (1093399, 1),
];

const CLAN_BONUS_ADDITIONAL_PREFIX_FIXTURES: &[(u64, usize)] = &[
    (874887, 1),
    (875322, 1),
    (875375, 1),
    (876712, 2),
    (877436, 1),
    (877812, 1),
    (878093, 1),
    (901186, 1),
    (1059895, 1),
    (1065308, 1),
    (1065812, 1),
    (1066210, 1),
    (1069813, 1),
    (1078999, 2),
    (1081037, 1),
    (1088123, 1),
    (1089001, 1),
    (1090817, 1),
];

const PROJECTION: ClanBonusDiagnosticProjectionV1 =
    ClanBonusDiagnosticProjectionV1::DisableOrdinaryAbilitiesAndOutOfSliceBonuses;

const AUDITED_FIXED_COMBAT_SOURCE_BONUS_IDS: &[u32] = &[
    6, 7, 36, 37, 38, 39, 40, 42, 43, 90, 93, 156, 186, 202, 266, 612, 916, 1442, 1536, 1845, 4620,
];

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

fn stop_modifier_entry(id: u32, description: &str, attribute: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "unlockLevel": 0,
        "description": description,
        "longDescription": description,
        "abilityData": {
            "value": 0,
            "valueMin": 0,
            "valueMax": 0,
            "valueCondition": 0,
            "positionRequirement": "both",
            "previousRoundRequirement": "any",
            "currentRoundRequirement": "any",
            "indexRequirement": "any",
            "clanRequirement": "",
            "oppClanRequirement": "",
            "previousClanRequirement": "",
            "betPillzLink": "no",
            "sideAffected": "opponent",
            "attributeAffected": attribute,
            "attributeAction": "stop_modif",
            "specialAction": "none",
            "isInverted": false,
            "isSupport": false,
            "isAntiSupport": false,
            "isOverdrive": false,
            "isDivide": false,
            "isLifeLinked": false,
            "isPillzLinked": false,
            "isLostLifeLinked": false,
            "isLostPillzLinked": false,
            "isOppStarsLinked": false,
            "isClanmatesCountLinked": false,
            "isAntiClanmatesCountLinked": false,
            "isPermanent": false,
            "isImmediatePermanent": false
        }
    })
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
) -> ClanBonusDiagnosticReplayV1 {
    ClanBonusDiagnosticReplayV1::new(replay(id, catalog), catalog, registry, PROJECTION)
        .unwrap_or_else(|error| panic!("prepare clan-bonus-diagnostic-v1 battle {id}: {error}"))
}

#[derive(Default)]
struct DispositionCounts {
    absent: usize,
    execute: usize,
    disabled: usize,
}

impl DispositionCounts {
    fn add_report(
        &mut self,
        report: &urban_recreation_rust::replay::ClanBonusDiagnosticReplayReportV1,
    ) {
        for round in &report.rounds {
            for player in PlayerId::ALL {
                for disposition in [
                    &round.selected[player].ability,
                    &round.selected[player].bonus,
                ] {
                    match disposition {
                        DiagnosticProjectionDispositionV1::Absent => self.absent += 1,
                        DiagnosticProjectionDispositionV1::Execute { .. } => self.execute += 1,
                        DiagnosticProjectionDispositionV1::Disabled { .. } => self.disabled += 1,
                    }
                }
            }
        }
    }
}

#[test]
fn fixed_gate_preserves_20_base_rounds_and_adds_20_more() {
    let catalog = catalog();
    let registry = registry();
    let mut base_rounds = 0;
    let mut dispositions = DispositionCounts::default();
    for &(battle_id, prefix) in BASE_RULES_PREFIX_FIXTURES {
        let source = replay(battle_id, &catalog);
        BaseRulesReplay::new(source.clone(), &catalog)
            .unwrap()
            .execute_effects_disabled_prefix(prefix)
            .unwrap_or_else(|error| panic!("base fixture {battle_id}: {error}"));
        let report = ClanBonusDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION)
            .unwrap()
            .execute_clan_bonus_diagnostic_v1_prefix(prefix)
            .unwrap_or_else(|error| {
                panic!("preserved clan-bonus-diagnostic-v1 fixture {battle_id}: {error}")
            });
        base_rounds += report.rounds.len();
        dispositions.add_report(&report);
    }
    assert_eq!(base_rounds, 20);

    let mut additional_rounds = 0;
    for &(battle_id, prefix) in CLAN_BONUS_ADDITIONAL_PREFIX_FIXTURES {
        let report = diagnostic(battle_id, &catalog, &registry)
            .execute_clan_bonus_diagnostic_v1_prefix(prefix)
            .unwrap_or_else(|error| {
                panic!("additional clan-bonus-diagnostic-v1 fixture {battle_id}: {error}")
            });
        additional_rounds += report.rounds.len();
        dispositions.add_report(&report);
    }
    assert_eq!(additional_rounds, 20);
    assert_eq!(base_rounds + additional_rounds, 40);
    assert_eq!(
        dispositions.absent + dispositions.execute + dispositions.disabled,
        160
    );
    assert_eq!(dispositions.execute, 69);
    assert_eq!(dispositions.disabled, 89);
    assert_eq!(dispositions.absent, 2);
}

#[test]
fn complete_battles_remain_explicit_and_server_arithmetic_is_visible() {
    let catalog = catalog();
    let registry = registry();

    let existing = diagnostic(1065231, &catalog, &registry)
        .execute_clan_bonus_diagnostic_v1()
        .unwrap();
    assert_eq!(existing.rounds.len(), 2);
    assert_eq!(
        existing.final_position.status,
        MatchStatus::Won(PlayerId::P1)
    );

    let added = diagnostic(876712, &catalog, &registry)
        .execute_clan_bonus_diagnostic_v1()
        .unwrap();
    assert_eq!(added.rounds.len(), 2);
    assert_eq!(added.rounds[0].round.cards[PlayerId::P2].attack, 3);
    assert_eq!(added.rounds[1].round.cards[PlayerId::P2].attack, 18);

    let power_support = diagnostic(874887, &catalog, &registry)
        .execute_clan_bonus_diagnostic_v1_prefix(1)
        .unwrap();
    assert_eq!(power_support.rounds[0].round.cards[PlayerId::P1].power, 7);
    assert_eq!(power_support.rounds[0].round.cards[PlayerId::P1].attack, 28);
    assert_eq!(power_support.rounds[0].round.cards[PlayerId::P2].attack, 48);

    let ordered = diagnostic(877436, &catalog, &registry)
        .execute_clan_bonus_diagnostic_v1_prefix(1)
        .unwrap();
    assert_eq!(ordered.rounds[0].round.cards[PlayerId::P2].power, 8);
    assert_eq!(ordered.rounds[0].round.cards[PlayerId::P2].attack, 44);

    let stopped = diagnostic(1069813, &catalog, &registry)
        .execute_clan_bonus_diagnostic_v1_prefix(1)
        .unwrap();
    assert_eq!(stopped.rounds[0].round.cards[PlayerId::P1].power, 5);
    assert_eq!(stopped.rounds[0].round.cards[PlayerId::P1].attack, 30);

    let cancelled = diagnostic(1072885, &catalog, &registry)
        .execute_clan_bonus_diagnostic_v1_prefix(1)
        .unwrap();
    assert_eq!(cancelled.rounds[0].round.cards[PlayerId::P2].attack, 48);

    let played_support = diagnostic(1078999, &catalog, &registry)
        .execute_clan_bonus_diagnostic_v1_prefix(2)
        .unwrap();
    assert_eq!(
        played_support.rounds[1].selected[PlayerId::P2].source_bonus_support_count,
        4
    );
    assert_eq!(
        played_support.rounds[1].round.cards[PlayerId::P2].attack,
        60
    );
}

#[test]
fn dispositions_distinguish_absent_executed_and_disabled_sources() {
    let catalog = catalog();
    let registry = registry();

    let inactive = diagnostic(901186, &catalog, &registry);
    assert!(matches!(
        inactive.preparation()[PlayerId::P1][0].bonus,
        DiagnosticProjectionDispositionV1::Absent
    ));

    let prepared = diagnostic(874887, &catalog, &registry);
    assert!(matches!(
        prepared.preparation()[PlayerId::P2][3].bonus,
        DiagnosticProjectionDispositionV1::Execute { .. }
    ));
    assert!(matches!(
        prepared.preparation()[PlayerId::P2][3].ability,
        DiagnosticProjectionDispositionV1::Disabled {
            reason: DiagnosticDisabledReasonV1::OrdinaryAbility { .. },
            ..
        }
    ));
    let report = prepared.execute_clan_bonus_diagnostic_v1_prefix(1).unwrap();
    assert_eq!(
        report.rounds[0].selected[PlayerId::P2].source_bonus_support_count,
        4
    );
    assert!(matches!(
        report.rounds[0].selected[PlayerId::P2].ability,
        DiagnosticProjectionDispositionV1::Disabled { .. }
    ));

    let out_of_slice = diagnostic(876712, &catalog, &registry);
    assert!(matches!(
        out_of_slice.preparation()[PlayerId::P2][0].bonus,
        DiagnosticProjectionDispositionV1::Disabled {
            reason: DiagnosticDisabledReasonV1::OutOfSliceBonus { .. },
            ..
        }
    ));
}

#[test]
fn source_bonus_context_groups_oculus_by_exact_id_without_inferred_clans() {
    let catalog = catalog();
    let registry = registry();
    let prepared = diagnostic(1081037, &catalog, &registry);
    let p1 = &prepared.preparation()[PlayerId::P1];

    assert_eq!(p1[0].key, CardKey::new(2425, 3));
    assert_eq!(p1[0].source_bonus_support_count, 2);
    assert_eq!(p1[1].source_bonus_support_count, 2);
    let DiagnosticProjectionDispositionV1::Execute { identity, .. } = &p1[0].bonus else {
        panic!("captured Oculus bonus was not executable")
    };
    assert_eq!(identity.id, 42);
}

#[test]
fn support_count_uses_distinct_character_id_not_levels_or_slots() {
    let catalog = catalog();
    let registry = registry();
    let mut source = replay(874887, &catalog);
    source.rounds.clear();
    source.players[0].hand[0].key = CardKey::new(123, 1);
    source.players[0].hand[1].key = CardKey::new(123, 2);
    source.players[0].hand[0].source_bonus = Some(SourceModifier {
        id: 266,
        description: "Support: Attack +3".to_owned(),
    });
    source.players[0].hand[1].source_bonus = source.players[0].hand[0].source_bonus.clone();
    source.players[0].hand[2].source_bonus = None;
    source.players[0].hand[3].source_bonus = None;

    let prepared =
        ClanBonusDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert_eq!(
        prepared.preparation()[PlayerId::P1][0].source_bonus_support_count,
        1
    );
    assert_eq!(
        prepared.preparation()[PlayerId::P1][1].source_bonus_support_count,
        1
    );
}

#[test]
fn diagnostic_context_uses_engine_player_order_after_capture_normalization() {
    let catalog = catalog();
    let registry = registry();
    let prepared = diagnostic(874887, &catalog, &registry);
    assert_eq!(prepared.replay().players[0].engine_player, EnginePlayer::P1);
    assert_eq!(prepared.replay().players[1].engine_player, EnginePlayer::P2);
}

#[test]
fn unsupported_control_is_visible_when_unplayed_and_rejected_only_when_selected() {
    let catalog = catalog();
    let registry = registry();
    let conditional_stop = SourceModifier {
        id: 287,
        description: "Courage: Stop Opp. Bonus".to_owned(),
    };

    let mut unplayed = replay(874887, &catalog);
    unplayed.players[0].hand[0].source_ability = Some(conditional_stop.clone());
    let unplayed =
        ClanBonusDiagnosticReplayV1::new(unplayed, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        unplayed.preparation()[PlayerId::P1][0].ability,
        DiagnosticProjectionDispositionV1::Disabled {
            reason: DiagnosticDisabledReasonV1::UnsupportedPromisedControl { .. },
            ..
        }
    ));
    unplayed.execute_clan_bonus_diagnostic_v1_prefix(1).unwrap();

    let mut selected = replay(874887, &catalog);
    selected.players[0].hand[1].source_ability = Some(conditional_stop);
    let selected =
        ClanBonusDiagnosticReplayV1::new(selected, &catalog, &registry, PROJECTION).unwrap();
    let error = selected
        .execute_clan_bonus_diagnostic_v1_prefix(1)
        .unwrap_err();
    assert!(matches!(
        error,
        ClanBonusDiagnosticReplayErrorV1::Engine { .. }
    ));
    assert!(error.to_string().contains("clan-bonus-diagnostic-v1"));
    assert!(error
        .to_string()
        .contains("unsupported Ability control 287"));
}

#[test]
fn power_and_attack_stop_modifier_is_a_promised_control_but_life_and_pillz_is_not() {
    const POWER_AND_ATTACK_ID: u32 = 900_001;
    const LIFE_AND_PILLZ_ID: u32 = 900_002;
    const POWER_AND_ATTACK: &str = "Cancel Opp. Power And Attack Modif.";
    const LIFE_AND_PILLZ: &str = "Cancel Opp. Life And Pillz Modif.";

    let mut entries = serde_json::Map::new();
    entries.insert(
        POWER_AND_ATTACK_ID.to_string(),
        stop_modifier_entry(POWER_AND_ATTACK_ID, POWER_AND_ATTACK, "pwr&atk"),
    );
    entries.insert(
        LIFE_AND_PILLZ_ID.to_string(),
        stop_modifier_entry(LIFE_AND_PILLZ_ID, LIFE_AND_PILLZ, "life&pillz"),
    );
    let bytes = serde_json::to_vec(&serde_json::Value::Object(entries)).unwrap();
    let registry = EffectRegistryV1::from_reader(bytes.as_slice()).unwrap();
    let catalog = catalog();
    let mut source = replay(874887, &catalog);
    for player in &mut source.players {
        for card in &mut player.hand {
            card.source_ability = None;
            card.source_bonus = None;
        }
    }
    let selected_slot = usize::from(
        source.rounds[0]
            .plays
            .iter()
            .find(|play| play.engine_player == EnginePlayer::P1)
            .unwrap()
            .hand_index,
    );
    let other_slot = (selected_slot + 1) % 4;
    source.players[0].hand[selected_slot].source_ability = Some(SourceModifier {
        id: POWER_AND_ATTACK_ID,
        description: POWER_AND_ATTACK.to_owned(),
    });
    source.players[0].hand[other_slot].source_ability = Some(SourceModifier {
        id: LIFE_AND_PILLZ_ID,
        description: LIFE_AND_PILLZ.to_owned(),
    });

    let prepared =
        ClanBonusDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][selected_slot].ability,
        DiagnosticProjectionDispositionV1::Disabled {
            reason: DiagnosticDisabledReasonV1::UnsupportedPromisedControl { .. },
            ..
        }
    ));
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][other_slot].ability,
        DiagnosticProjectionDispositionV1::Disabled {
            reason: DiagnosticDisabledReasonV1::OrdinaryAbility { .. },
            ..
        }
    ));

    let error = prepared
        .execute_clan_bonus_diagnostic_v1_prefix(1)
        .unwrap_err();
    let ClanBonusDiagnosticReplayErrorV1::Engine {
        selected, source, ..
    } = error
    else {
        panic!("expected selected promised-control rejection")
    };
    assert!(matches!(
        selected[PlayerId::P1].ability,
        DiagnosticProjectionDispositionV1::Disabled {
            reason: DiagnosticDisabledReasonV1::UnsupportedPromisedControl { .. },
            ..
        }
    ));
    assert!(source
        .to_string()
        .contains("unsupported Ability control 900001"));
}

#[test]
fn diagnostic_mismatch_retains_selected_dispositions() {
    let catalog = catalog();
    let registry = registry();
    let mut source = replay(874887, &catalog);
    source.rounds[0].expected_card_results[0]
        .as_mut()
        .unwrap()
        .power += 1;
    let error = ClanBonusDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION)
        .unwrap()
        .execute_clan_bonus_diagnostic_v1_prefix(1)
        .unwrap_err();
    let ClanBonusDiagnosticReplayErrorV1::Mismatch { selected, .. } = error else {
        panic!("expected projected server-field mismatch")
    };
    assert!(matches!(
        selected[PlayerId::P1].ability,
        DiagnosticProjectionDispositionV1::Disabled { .. }
    ));
}

#[test]
fn strict_capture_identity_conflicts_are_fatal_during_preparation() {
    let catalog = catalog();
    let registry = registry();
    let mut source = replay(874887, &catalog);
    source.players[1].hand[3]
        .source_bonus
        .as_mut()
        .unwrap()
        .description = "Attack +3".to_owned();
    let error =
        ClanBonusDiagnosticReplayV1::new(source, &catalog, &registry, PROJECTION).unwrap_err();
    assert!(matches!(
        error,
        ClanBonusDiagnosticPreparationErrorV1::Lookup { .. }
    ));
    assert!(error.to_string().contains("description mismatch"));
}

#[test]
fn audited_source_bonus_ids_are_shape_driven_and_night_1442_stays_deferred() {
    let registry = registry();
    assert_eq!(AUDITED_FIXED_COMBAT_SOURCE_BONUS_IDS.len(), 21);
    for &id in AUDITED_FIXED_COMBAT_SOURCE_BONUS_IDS {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing audited source bonus id {id}"));
        if id == 1442 {
            assert!(matches!(
                definition.compiled(),
                CompiledEffectV1::Unsupported(_)
            ));
        } else {
            assert!(matches!(
                definition.compiled(),
                CompiledEffectV1::Supported(_)
            ));
        }
    }
}
