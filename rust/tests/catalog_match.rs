use std::path::PathBuf;

use urban_recreation_rust::catalog::{CardKey, EffectiveCardCatalog};
use urban_recreation_rust::effect_registry::{
    EffectLookupError, EffectRegistryV1, MagnitudeMultiplierV1, SupportedEffectV1,
};
use urban_recreation_rust::engine::{
    derive_catalog_hand, BaseRulesRoundInput, BaseRulesSelection, ByPlayer,
    CatalogCombatStatMatchErrorV1, CatalogCombatStatMatchInputV1, CatalogCombatStatMatchV1,
    CatalogCombatStatPlayerInputV1, CatalogCombatStatProjectionV1,
    CatalogCombatStatSourceDispositionV1, CombatStatEffectSourceV1, CombatStatSourcePlanV1,
    EffectiveCatalogHandErrorV1, MatchStatus, PlayerId,
    CATALOG_CONTEXT_POLICY_SEMANTIC_REVISION_V1,
};
use urban_recreation_rust::replay::{
    load_corpus, COMBAT_STAT_DIAGNOSTIC_COMPILER_POLICY_SEMANTIC_REVISION_V1,
};

const PROJECTION: CatalogCombatStatProjectionV1 =
    CatalogCombatStatProjectionV1::RequireFullyExecutableDraws;

fn root_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(path)
}

fn catalog() -> EffectiveCardCatalog {
    EffectiveCardCatalog::load(
        root_path("data/data.json"),
        root_path("data/battle_card_overrides.json"),
    )
    .unwrap()
}

fn registry() -> EffectRegistryV1 {
    EffectRegistryV1::load(root_path("captures/abilities.json")).unwrap()
}

fn player(hand: [CardKey; 4]) -> CatalogCombatStatPlayerInputV1 {
    CatalogCombatStatPlayerInputV1 {
        initial_life: 12,
        initial_pillz: 12,
        hand,
    }
}

fn input(p1: [CardKey; 4], p2: [CardKey; 4], night: bool) -> CatalogCombatStatMatchInputV1 {
    CatalogCombatStatMatchInputV1 {
        battle_rule_id: 10,
        night,
        players: ByPlayer::new(player(p1), player(p2)),
    }
}

fn fully_supported_hands() -> ([CardKey; 4], [CardKey; 4]) {
    (
        [
            CardKey::new(123, 1),
            CardKey::new(124, 1),
            CardKey::new(138, 1),
            CardKey::new(139, 1),
        ],
        [
            CardKey::new(441, 1),
            CardKey::new(444, 1),
            CardKey::new(445, 1),
            CardKey::new(447, 1),
        ],
    )
}

#[test]
fn effective_clans_cover_oculus_shapes_night_and_runtime_overrides_without_mutation() {
    let catalog = catalog();
    let mono = derive_catalog_hand(
        [
            CardKey::new(2118, 3),
            CardKey::new(1035, 3),
            CardKey::new(370, 5),
            CardKey::new(1918, 3),
        ],
        false,
        &catalog,
    )
    .unwrap();
    assert_eq!(
        mono.each_ref().map(|card| card.effective.effective_clan_id),
        [10; 4]
    );
    assert_eq!(
        mono.each_ref()
            .map(|card| card.bonus.as_ref().unwrap().description.as_str()),
        ["Power +2"; 4]
    );
    assert_eq!(
        mono.each_ref()
            .map(|card| card.effective.source_bonus_support_count),
        [4; 4]
    );

    let split = derive_catalog_hand(
        [
            CardKey::new(2118, 3),
            CardKey::new(1529, 4),
            CardKey::new(370, 5),
            CardKey::new(1918, 3),
        ],
        false,
        &catalog,
    )
    .unwrap();
    assert_eq!(split[0].effective.effective_clan_id, 49);
    assert_eq!(
        split
            .each_ref()
            .map(|card| card.effective.effective_clan_id),
        [49, 49, 10, 10]
    );
    assert_eq!(
        split
            .each_ref()
            .map(|card| card.effective.source_bonus_support_count),
        [2; 4]
    );

    let dynamic_copy = derive_catalog_hand(
        [
            CardKey::new(2094, 1),
            CardKey::new(123, 1),
            CardKey::new(124, 1),
            CardKey::new(2243, 1),
        ],
        false,
        &catalog,
    )
    .unwrap();
    assert_eq!(dynamic_copy[0].effective.effective_clan_id, 57);
    assert_eq!(dynamic_copy[0].effective.active_bonus_clan_id, Some(57));
    assert_eq!(
        dynamic_copy[0].bonus.as_ref().unwrap().description,
        "Copy: Opp. Ability"
    );

    let three_clans = derive_catalog_hand(
        [
            CardKey::new(2118, 3),
            CardKey::new(1529, 4),
            CardKey::new(370, 5),
            CardKey::new(1610, 4),
        ],
        false,
        &catalog,
    )
    .unwrap();
    assert_eq!(three_clans[0].effective.effective_clan_id, 56);
    assert!(three_clans.iter().all(|card| card.bonus.is_none()));

    let multiple_oculus = derive_catalog_hand(
        [
            CardKey::new(2074, 4),
            CardKey::new(2075, 4),
            CardKey::new(123, 1),
            CardKey::new(124, 1),
        ],
        false,
        &catalog,
    )
    .unwrap();
    assert_eq!(
        multiple_oculus
            .each_ref()
            .map(|card| card.effective.effective_clan_id),
        [56, 56, 25, 25]
    );
    assert!(multiple_oculus[0].bonus.is_none());
    assert!(multiple_oculus[1].bonus.is_none());
    assert_eq!(
        multiple_oculus[2].bonus.as_ref().unwrap().description,
        "Damage +2"
    );

    let day = derive_catalog_hand(
        [
            CardKey::new(1630, 2),
            CardKey::new(1631, 2),
            CardKey::new(123, 1),
            CardKey::new(641, 4),
        ],
        false,
        &catalog,
    )
    .unwrap();
    let night = derive_catalog_hand(
        [
            CardKey::new(1630, 2),
            CardKey::new(1631, 2),
            CardKey::new(123, 1),
            CardKey::new(641, 4),
        ],
        true,
        &catalog,
    )
    .unwrap();
    assert_eq!(
        day[0].ability.as_ref().unwrap().description,
        "Day: Stop Opp. Bonus"
    );
    assert_eq!(
        night[0].ability.as_ref().unwrap().description,
        "Night: Stop Opp. Ability"
    );
    assert_eq!(night[0].ability.as_ref().unwrap().catalog_id, None);
    assert_eq!(night[1].ability.as_ref().unwrap().description, "Power +9");
    assert_eq!(
        day[0].bonus.as_ref().unwrap().description,
        "Day: Power And Damage + 1"
    );
    assert_eq!(
        night[0].bonus.as_ref().unwrap().description,
        "Night: -1 Opp Pow. And Damage, Min 1"
    );
    assert_eq!(night[0].bonus.as_ref().unwrap().catalog_id, None);

    let quetzal = derive_catalog_hand(
        [
            CardKey::new(1577, 3),
            CardKey::new(123, 1),
            CardKey::new(124, 1),
            CardKey::new(138, 1),
        ],
        false,
        &catalog,
    )
    .unwrap();
    assert_eq!(
        quetzal[0].ability.as_ref().unwrap().description,
        "Stop Opp. Bonus"
    );
    assert_eq!(quetzal[0].ability.as_ref().unwrap().catalog_id, Some(5927));
    assert_eq!(catalog.get(CardKey::new(2118, 3)).unwrap().clan_id, 56);
}

#[test]
fn strict_catalog_match_separates_same_text_clans_and_executes_support() {
    let catalog = catalog();
    let registry = registry();
    let (p1, p2) = fully_supported_hands();
    let prepared =
        CatalogCombatStatMatchV1::new(input(p1, p2, false), &catalog, &registry, PROJECTION)
            .unwrap();

    let p1_cards = &prepared.preparation()[PlayerId::P1];
    assert_eq!(
        p1_cards.each_ref().map(|card| card.effective_clan_id),
        [25, 25, 27, 27]
    );
    assert_eq!(
        p1_cards
            .each_ref()
            .map(|card| card.source_bonus_support_count),
        [2; 4]
    );
    let CatalogCombatStatSourceDispositionV1::Execute { identity: fpc, .. } = &p1_cards[0].bonus
    else {
        panic!("Fang Pi Clang bonus was not executable")
    };
    let CatalogCombatStatSourceDispositionV1::Execute {
        identity: junta, ..
    } = &p1_cards[2].bonus
    else {
        panic!("La Junta bonus was not executable")
    };
    assert_eq!((fpc.catalog_id, junta.catalog_id), (Some(23), Some(25)));
    assert_eq!(fpc.description, junta.description);
    assert_eq!(fpc.registry_definition_id, junta.registry_definition_id);

    let CatalogCombatStatSourceDispositionV1::Execute {
        identity: rescue, ..
    } = &prepared.preparation()[PlayerId::P2][0].bonus
    else {
        panic!("Rescue bonus was not executable")
    };
    assert_eq!(rescue.catalog_id, Some(39));
    assert_eq!(rescue.registry_definition_id, 266);
    assert_eq!(rescue.registry_alias_ids.as_ref(), [266, 546, 5841]);
    assert!(!rescue.registry_alias_ids.contains(&39));

    let mut game = prepared.new_game();
    let before = game.position().clone();
    let (report, undo) = game
        .make(BaseRulesRoundInput {
            first_mover: PlayerId::P1,
            selections: ByPlayer::new(
                BaseRulesSelection::new(0, 0, false),
                BaseRulesSelection::new(0, 0, false),
            ),
        })
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].damage, 4);
    assert_eq!(report.cards[PlayerId::P1].attack, 3);
    assert_eq!(report.cards[PlayerId::P2].attack, 17);
    assert!(report.cards[PlayerId::P2].won);
    assert_eq!(report.players[PlayerId::P1].life, 11);
    assert_eq!(report.status, MatchStatus::Playing);
    game.unmake(undo);
    assert_eq!(game.position(), &before);
}

#[test]
fn strict_catalog_match_executes_equalizer_from_the_selected_opponent_level() {
    let catalog = catalog();
    let registry = registry();
    let hive = [
        CardKey::new(1536, 5),
        CardKey::new(1534, 1),
        CardKey::new(1535, 1),
        CardKey::new(1556, 1),
    ];
    let opponent = [
        CardKey::new(123, 3),
        CardKey::new(124, 1),
        CardKey::new(138, 1),
        CardKey::new(139, 1),
    ];
    let prepared = CatalogCombatStatMatchV1::new(
        input(hive, opponent, false),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    for disposition in [
        &prepared.preparation()[PlayerId::P1][0].ability,
        &prepared.preparation()[PlayerId::P1][0].bonus,
    ] {
        assert!(matches!(
            disposition,
            CatalogCombatStatSourceDispositionV1::Execute {
                effect: SupportedEffectV1::ModifyCombatStat {
                    multiplier: MagnitudeMultiplierV1::OpponentStars,
                    ..
                },
                predicate: urban_recreation_rust::engine::CombatStatPredicateV1::Always,
                ..
            }
        ));
    }

    let mut game = prepared.new_game();
    let before = game.position().clone();
    let (report, undo) = game
        .make(BaseRulesRoundInput {
            first_mover: PlayerId::P1,
            selections: ByPlayer::new(
                BaseRulesSelection::new(0, 0, false),
                BaseRulesSelection::new(0, 4, false),
            ),
        })
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 8);
    assert_eq!(report.cards[PlayerId::P1].attack, 8);
    assert_eq!(report.cards[PlayerId::P2].attack, 11);
    game.unmake(undo);
    assert_eq!(game.position(), &before);
}

#[test]
fn strict_constructor_preserves_context_provenance_and_the_live_override() {
    let catalog = catalog();
    let registry = registry();
    let (_, p2) = fully_supported_hands();
    let input = CatalogCombatStatMatchInputV1 {
        battle_rule_id: 77,
        night: false,
        players: ByPlayer::new(
            CatalogCombatStatPlayerInputV1 {
                initial_life: 14,
                initial_pillz: 9,
                hand: [
                    CardKey::new(1577, 3),
                    CardKey::new(123, 1),
                    CardKey::new(124, 1),
                    CardKey::new(138, 1),
                ],
            },
            CatalogCombatStatPlayerInputV1 {
                initial_life: 11,
                initial_pillz: 7,
                hand: p2,
            },
        ),
    };
    let prepared =
        CatalogCombatStatMatchV1::new(input.clone(), &catalog, &registry, PROJECTION).unwrap();
    assert_eq!(prepared.input(), &input);
    let base = &prepared.match_spec().base_rules;
    assert_eq!((base.battle_rule_id, base.night), (77, false));
    assert_eq!(
        (
            base.players[PlayerId::P1].initial_life,
            base.players[PlayerId::P1].initial_pillz,
            base.players[PlayerId::P2].initial_life,
            base.players[PlayerId::P2].initial_pillz,
        ),
        (14, 9, 11, 7)
    );
    assert_eq!(
        (
            base.players[PlayerId::P1].hand[0].power,
            base.players[PlayerId::P1].hand[0].damage,
        ),
        (7, 4)
    );
    let CatalogCombatStatSourceDispositionV1::Execute {
        identity: quetzal, ..
    } = &prepared.preparation()[PlayerId::P1][0].ability
    else {
        panic!("Quetzal override ability was not executable")
    };
    assert_eq!(quetzal.catalog_id, Some(5927));
    assert!(quetzal.registry_alias_ids.contains(&5927));
    let CombatStatSourcePlanV1::Execute { source_id, .. } =
        prepared.match_spec().cards[PlayerId::P1][0].ability
    else {
        panic!("Quetzal compact ability was not executable")
    };
    assert_eq!(source_id, quetzal.registry_definition_id);

    let provenance = prepared.provenance();
    assert_eq!(provenance.projection, PROJECTION);
    assert_eq!(
        provenance.effect_registry_schema_version,
        registry.schema_version()
    );
    assert_eq!(
        provenance.effect_registry_source_fingerprint_fnv1a64,
        registry.source_fingerprint_fnv1a64()
    );
    assert_eq!(
        provenance.effective_catalog_source_fingerprint_fnv1a64,
        catalog.source_fingerprint_fnv1a64()
    );
    assert_eq!(
        provenance.compiler_policy_semantic_revision,
        COMBAT_STAT_DIAGNOSTIC_COMPILER_POLICY_SEMANTIC_REVISION_V1
    );
    assert_eq!(
        provenance.catalog_context_policy_semantic_revision,
        CATALOG_CONTEXT_POLICY_SEMANTIC_REVISION_V1
    );

    let game = prepared.new_game();
    assert_eq!(game.position().players[PlayerId::P1].life, 14);
    assert_eq!(game.position().players[PlayerId::P1].pillz, 9);
    assert_eq!(game.position().players[PlayerId::P2].life, 11);
    assert_eq!(game.position().players[PlayerId::P2].pillz, 7);
    assert_eq!(game.position().rounds_played, 0);
}

#[test]
fn strict_constructor_derives_oculus_and_rejects_dynamic_or_temporal_sources() {
    let catalog = catalog();
    let registry = registry();
    let (_, p2) = fully_supported_hands();

    let oculus = CatalogCombatStatMatchV1::new(
        input(
            [
                CardKey::new(2094, 1),
                CardKey::new(123, 1),
                CardKey::new(124, 1),
                CardKey::new(125, 1),
            ],
            p2,
            false,
        ),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    assert_eq!(
        oculus.preparation()[PlayerId::P1]
            .each_ref()
            .map(|card| card.effective_clan_id),
        [25; 4]
    );
    assert_eq!(
        oculus.preparation()[PlayerId::P1]
            .each_ref()
            .map(|card| card.source_bonus_support_count),
        [4; 4]
    );

    let night = [
        CardKey::new(1631, 1),
        CardKey::new(1632, 1),
        CardKey::new(1633, 1),
        CardKey::new(1637, 1),
    ];
    assert!(matches!(
        CatalogCombatStatMatchV1::new(input(night, p2, true), &catalog, &registry, PROJECTION),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            source_kind: CombatStatEffectSourceV1::Bonus,
            ref description,
            ..
        }) if description == "Night: -1 Opp Pow. And Damage, Min 1"
    ));

    let oblivion = [
        CardKey::new(2243, 1),
        CardKey::new(2244, 1),
        CardKey::new(123, 1),
        CardKey::new(138, 1),
    ];
    assert!(matches!(
        CatalogCombatStatMatchV1::new(input(oblivion, p2, false), &catalog, &registry, PROJECTION),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            source_kind: CombatStatEffectSourceV1::Bonus,
            ref description,
            ..
        }) if description == "Copy: Opp. Ability"
    ));
}

#[test]
fn strict_catalog_match_rejects_duplicate_leader_and_any_unsupported_source() {
    let catalog = catalog();
    let registry = registry();
    let (p1, p2) = fully_supported_hands();

    let mut duplicate = p1;
    duplicate[1] = CardKey::new(123, 2);
    assert!(matches!(
        CatalogCombatStatMatchV1::new(input(duplicate, p2, false), &catalog, &registry, PROJECTION),
        Err(CatalogCombatStatMatchErrorV1::DuplicateCharacter {
            player: PlayerId::P1,
            character_id: 123,
            ..
        })
    ));

    let mut leader = p1;
    leader[0] = CardKey::new(271, 5);
    assert!(matches!(
        CatalogCombatStatMatchV1::new(input(leader, p2, false), &catalog, &registry, PROJECTION),
        Err(CatalogCombatStatMatchErrorV1::WholeHandLeaderHazard {
            player: PlayerId::P1,
            ..
        })
    ));

    let mut unsupported = p1;
    unsupported[0] = CardKey::new(448, 3);
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(unsupported, p2, false),
            &catalog,
            &registry,
            PROJECTION
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            ..
        })
    ));

    let mut ambiguous = p1;
    ambiguous[0] = CardKey::new(137, 3);
    assert!(matches!(
        CatalogCombatStatMatchV1::new(input(ambiguous, p2, false), &catalog, &registry, PROJECTION),
        Err(CatalogCombatStatMatchErrorV1::Lookup {
            player: PlayerId::P1,
            hand_slot,
            source_kind: CombatStatEffectSourceV1::Ability,
            source: EffectLookupError::AmbiguousDescription { .. },
            ..
        }) if hand_slot.get() == 0
    ));

    let mut unknown = p2;
    unknown[2] = CardKey::new(u32::MAX, 1);
    assert!(matches!(
        CatalogCombatStatMatchV1::new(input(p1, unknown, false), &catalog, &registry, PROJECTION),
        Err(CatalogCombatStatMatchErrorV1::Hand {
            player: PlayerId::P2,
            source: EffectiveCatalogHandErrorV1::MissingCard { hand_slot, key },
        }) if hand_slot.get() == 2 && key == CardKey::new(u32::MAX, 1)
    ));

    let mut unknown_level = p2;
    unknown_level[3] = CardKey::new(441, 99);
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(p1, unknown_level, false),
            &catalog,
            &registry,
            PROJECTION
        ),
        Err(CatalogCombatStatMatchErrorV1::Hand {
            player: PlayerId::P2,
            source: EffectiveCatalogHandErrorV1::MissingCard { hand_slot, key },
        }) if hand_slot.get() == 3 && key == CardKey::new(441, 99)
    ));
}

#[test]
fn catalog_bonus_derivation_matches_the_complete_capture_corpus_except_dynamic_copy() {
    let catalog = catalog();
    let corpus = load_corpus(root_path("captures/games"), root_path("data/data.json")).unwrap();
    assert!(corpus.errors.is_empty());

    assert!(corpus.ready.len() >= 322);
    let mut observations = 0_usize;
    let mut checked_non_dynamic = 0_usize;
    let mut dynamic_copy_slots = 0_usize;
    let mut dynamic_replacements = 0_usize;
    for replay in &corpus.ready {
        for player in 0..2 {
            let keys = std::array::from_fn(|slot| replay.players[player].hand[slot].key);
            let derived = derive_catalog_hand(keys, replay.metadata.night, &catalog).unwrap();
            for slot in 0..4 {
                observations += 1;
                let expected = derived[slot]
                    .bonus
                    .as_ref()
                    .map(|bonus| bonus.description.as_str());
                let captured = replay.players[player].hand[slot]
                    .source_bonus
                    .as_ref()
                    .map(|bonus| bonus.description.as_str());
                if derived[slot].effective.active_bonus_clan_id == Some(57) {
                    assert_eq!(expected, Some("Copy: Opp. Ability"));
                    dynamic_copy_slots += 1;
                    dynamic_replacements += usize::from(expected != captured);
                    continue;
                }
                checked_non_dynamic += 1;
                assert_eq!(
                    expected, captured,
                    "unexpected catalog/capture bonus mismatch in battle {} player {player} slot {slot}",
                    replay.metadata.battle_id
                );
            }
        }
    }
    assert!(observations >= 2_576);
    assert!(checked_non_dynamic >= 2_538);
    assert!(dynamic_copy_slots > 0);
    assert!(dynamic_replacements > 0);
}
