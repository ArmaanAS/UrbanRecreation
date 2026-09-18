use std::collections::{hash_map::DefaultHasher, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use urban_recreation_rust::catalog::{CardKey, EffectiveCardCatalog};
use urban_recreation_rust::effect_registry::{
    EffectRegistryV1, MagnitudeMultiplierV1, SupportedEffectV1,
};
use urban_recreation_rust::engine::{
    derive_catalog_hand, BaseRulesRoundInput, BaseRulesSelection, ByPlayer,
    CatalogCombatStatMatchErrorV1, CatalogCombatStatMatchInputV1, CatalogCombatStatMatchV1,
    CatalogCombatStatPlayerInputV1, CatalogCombatStatProjectionV1,
    CatalogCombatStatSourceDispositionV1, CombatStatEffectSourceV1, CombatStatPostRoundEffectV1,
    CombatStatPredicateV1, CombatStatSourcePlanV1, CopiedSourceKindV1, EffectiveCatalogHandErrorV1,
    MatchStatus, PlayerId, CATALOG_CONTEXT_POLICY_SEMANTIC_REVISION_V1,
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

fn catalog_with_komboka_context(clan_id: u32, bonus_id: u32) -> EffectiveCardCatalog {
    let mut rows: serde_json::Value =
        serde_json::from_slice(&fs::read(root_path("data/data.json")).unwrap()).unwrap();
    for row in rows.as_array_mut().unwrap() {
        if row["clan_id"] == 54 {
            row["clan_id"] = serde_json::json!(clan_id);
            row["bonus_id"] = serde_json::json!(bonus_id);
        }
    }
    let rows = serde_json::to_vec(&rows).unwrap();
    let overrides = fs::read(root_path("data/battle_card_overrides.json")).unwrap();
    EffectiveCardCatalog::from_readers(rows.as_slice(), overrides.as_slice()).unwrap()
}

fn registry_with_malformed_komboka() -> EffectRegistryV1 {
    let mut effects: serde_json::Value =
        serde_json::from_slice(&fs::read(root_path("captures/abilities.json")).unwrap()).unwrap();
    effects["1714"]["abilityData"]["value"] = serde_json::json!(2);
    let effects = serde_json::to_vec(&effects).unwrap();
    EffectRegistryV1::from_reader(effects.as_slice()).unwrap()
}

fn catalog_with_vod_life_lookalike(key: CardKey) -> EffectiveCardCatalog {
    let mut rows: serde_json::Value =
        serde_json::from_slice(&fs::read(root_path("data/data.json")).unwrap()).unwrap();
    let row = rows
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["id"] == key.id && row["level"] == key.level)
        .unwrap();
    row["ability_id"] = serde_json::json!(1396);
    row["ability"] = serde_json::json!("Victory Or Defeat : +1 Life");
    let rows = serde_json::to_vec(&rows).unwrap();
    let overrides = fs::read(root_path("data/battle_card_overrides.json")).unwrap();
    EffectiveCardCatalog::from_readers(rows.as_slice(), overrides.as_slice()).unwrap()
}

fn catalog_with_vod_life_bonus() -> EffectiveCardCatalog {
    let mut rows: serde_json::Value =
        serde_json::from_slice(&fs::read(root_path("data/data.json")).unwrap()).unwrap();
    for row in rows.as_array_mut().unwrap() {
        if row["clan_id"] == 38 {
            row["bonus_id"] = serde_json::json!(1396);
            row["bonus"] = serde_json::json!("Victory Or Defeat : +1 Life");
        }
    }
    let rows = serde_json::to_vec(&rows).unwrap();
    let overrides = fs::read(root_path("data/battle_card_overrides.json")).unwrap();
    EffectiveCardCatalog::from_readers(rows.as_slice(), overrides.as_slice()).unwrap()
}

fn catalog_with_equalizer_opponent_life_bonus() -> EffectiveCardCatalog {
    let mut rows: serde_json::Value =
        serde_json::from_slice(&fs::read(root_path("data/data.json")).unwrap()).unwrap();
    for row in rows.as_array_mut().unwrap() {
        if row["clan_id"] == 28 {
            row["bonus_id"] = serde_json::json!(1415);
            row["bonus"] = serde_json::json!("Equalizer: - 1 Opp. Life Min 2");
        }
    }
    let rows = serde_json::to_vec(&rows).unwrap();
    let overrides = fs::read(root_path("data/battle_card_overrides.json")).unwrap();
    EffectiveCardCatalog::from_readers(rows.as_slice(), overrides.as_slice()).unwrap()
}

fn catalog_with_anita_courage_life_alias(key: CardKey) -> EffectiveCardCatalog {
    let mut rows: serde_json::Value =
        serde_json::from_slice(&fs::read(root_path("data/data.json")).unwrap()).unwrap();
    let row = rows
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["id"] == key.id && row["level"] == key.level)
        .unwrap();
    row["ability_id"] = serde_json::json!(274);
    row["ability"] = serde_json::json!("Courage: +1 Life Per Dmg");
    let rows = serde_json::to_vec(&rows).unwrap();
    let overrides = fs::read(root_path("data/battle_card_overrides.json")).unwrap();
    EffectiveCardCatalog::from_readers(rows.as_slice(), overrides.as_slice()).unwrap()
}

fn catalog_with_ability_alias(
    key: CardKey,
    ability_id: u32,
    ability: &str,
) -> EffectiveCardCatalog {
    let mut rows: serde_json::Value =
        serde_json::from_slice(&fs::read(root_path("data/data.json")).unwrap()).unwrap();
    let row = rows
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["id"] == key.id && row["level"] == key.level)
        .unwrap();
    row["ability_id"] = serde_json::json!(ability_id);
    row["ability"] = serde_json::json!(ability);
    let rows = serde_json::to_vec(&rows).unwrap();
    let overrides = fs::read(root_path("data/battle_card_overrides.json")).unwrap();
    EffectiveCardCatalog::from_readers(rows.as_slice(), overrides.as_slice()).unwrap()
}

fn catalog_with_anita_courage_life_bonus() -> EffectiveCardCatalog {
    let mut rows: serde_json::Value =
        serde_json::from_slice(&fs::read(root_path("data/data.json")).unwrap()).unwrap();
    for row in rows.as_array_mut().unwrap() {
        if row["clan_id"] == 28 {
            row["bonus_id"] = serde_json::json!(274);
            row["bonus"] = serde_json::json!("Courage: +1 Life Per Dmg");
        }
    }
    let rows = serde_json::to_vec(&rows).unwrap();
    let overrides = fs::read(root_path("data/battle_card_overrides.json")).unwrap();
    EffectiveCardCatalog::from_readers(rows.as_slice(), overrides.as_slice()).unwrap()
}

fn registry_with_malformed_anita_courage_life() -> EffectRegistryV1 {
    let mut effects: serde_json::Value =
        serde_json::from_slice(&fs::read(root_path("captures/abilities.json")).unwrap()).unwrap();
    effects["274"]["abilityData"]["specialAction"] = serde_json::json!("convert_dmg_to_pillz");
    let effects = serde_json::to_vec(&effects).unwrap();
    EffectRegistryV1::from_reader(effects.as_slice()).unwrap()
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

fn position_hash(position: &urban_recreation_rust::engine::BaseRulesPosition) -> u64 {
    let mut hasher = DefaultHasher::new();
    position.hash(&mut hasher);
    hasher.finish()
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
    assert_eq!(
        mono.each_ref()
            .map(|card| card.effective.effective_clan_character_count),
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
    assert_eq!(
        split
            .each_ref()
            .map(|card| card.effective.effective_clan_character_count),
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

    // With O + A + A + B, Oculus infiltrates singleton B, leaving two effective
    // clans with immutable character counts of two.
    let oculus_singleton_infiltration = derive_catalog_hand(
        [
            CardKey::new(2302, 3),
            CardKey::new(2548, 3),
            CardKey::new(1584, 2),
            CardKey::new(833, 4),
        ],
        false,
        &catalog,
    )
    .unwrap();
    assert_eq!(
        oculus_singleton_infiltration
            .each_ref()
            .map(|card| card.effective.effective_clan_character_count),
        [2; 4]
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
fn strict_catalog_match_derives_ability_support_independently_of_bonus_activity() {
    let catalog = catalog();
    let registry = registry();
    let (_, fpc) = fully_supported_hands();

    let oscar = CatalogCombatStatMatchV1::new(
        input(
            [
                CardKey::new(549, 3),
                CardKey::new(129, 2),
                CardKey::new(130, 3),
                CardKey::new(216, 4),
            ],
            fpc,
            false,
        ),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    let oscar_card = &oscar.preparation()[PlayerId::P1][0];
    assert_eq!(oscar_card.effective_clan_character_count, 4);
    assert_eq!(oscar_card.source_ability_support_count, 4);
    assert_eq!(oscar_card.source_bonus_support_count, 4);
    let mut game = oscar.new_game();
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
    assert_eq!(report.cards[PlayerId::P1].power, 9);
    game.unmake(undo);
    assert_eq!(game.position(), &before);

    let nantosuelte = CatalogCombatStatMatchV1::new(
        input(
            [
                CardKey::new(2179, 3),
                CardKey::new(1980, 2),
                CardKey::new(1986, 3),
                CardKey::new(2013, 2),
            ],
            fpc,
            false,
        ),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    assert_eq!(
        nantosuelte.preparation()[PlayerId::P1][0].source_ability_support_count,
        4
    );
    let mut game = nantosuelte.new_game();
    let (report, _) = game
        .make(BaseRulesRoundInput {
            first_mover: PlayerId::P1,
            selections: ByPlayer::new(
                BaseRulesSelection::new(0, 0, false),
                BaseRulesSelection::new(0, 0, false),
            ),
        })
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].attack, 13);

    let taljion = CatalogCombatStatMatchV1::new(
        input(
            [
                CardKey::new(727, 3),
                CardKey::new(123, 1),
                CardKey::new(138, 1),
                CardKey::new(129, 2),
            ],
            fpc,
            false,
        ),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    let taljion_card = &taljion.preparation()[PlayerId::P1][0];
    assert_eq!(taljion_card.effective_clan_character_count, 1);
    assert_eq!(taljion_card.source_ability_support_count, 1);
    assert_eq!(taljion_card.source_bonus_support_count, 0);
    let mut game = taljion.new_game();
    let (report, _) = game
        .make(BaseRulesRoundInput {
            first_mover: PlayerId::P1,
            selections: ByPlayer::new(
                BaseRulesSelection::new(0, 0, false),
                BaseRulesSelection::new(0, 0, false),
            ),
        })
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].attack, 9);
}

#[test]
fn strict_catalog_match_executes_confidence_after_its_owner_wins() {
    let catalog = catalog();
    let registry = registry();
    let (_, p2) = fully_supported_hands();
    let prepared = CatalogCombatStatMatchV1::new(
        input(
            [
                CardKey::new(701, 3),  // Wesley
                CardKey::new(1707, 3), // Callie
                CardKey::new(1089, 2), // Sue
                CardKey::new(549, 3),  // Oscar
            ],
            p2,
            false,
        ),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        prepared.preparation()[PlayerId::P1][0].ability,
        CatalogCombatStatSourceDispositionV1::Execute {
            predicate: CombatStatPredicateV1::OwnerWonPreviousRound,
            ..
        }
    ));

    let mut game = prepared.new_game();
    let before = game.position().clone();
    let before_hash = position_hash(&before);
    let (first, undo_first) = game
        .make(BaseRulesRoundInput {
            first_mover: PlayerId::P1,
            selections: ByPlayer::new(
                BaseRulesSelection::new(1, 2, false), // Callie
                BaseRulesSelection::new(1, 0, false), // Slyde Cr
            ),
        })
        .unwrap();
    assert_eq!(first.cards[PlayerId::P1].key, CardKey::new(1707, 3));
    assert_eq!(first.cards[PlayerId::P2].key, CardKey::new(444, 1));
    assert!(first.cards[PlayerId::P1].won);
    let after_first = game.position().clone();
    let after_first_hash = position_hash(&after_first);

    let (second, undo_second) = game
        .make(BaseRulesRoundInput {
            first_mover: PlayerId::P2,
            selections: ByPlayer::new(
                BaseRulesSelection::new(0, 3, false), // Wesley
                BaseRulesSelection::new(0, 0, false), // Lea
            ),
        })
        .unwrap();
    assert_eq!(second.cards[PlayerId::P1].key, CardKey::new(701, 3));
    assert_eq!(second.cards[PlayerId::P2].key, CardKey::new(441, 1));
    // Lea's printed Power 5 is reduced by Wesley's active Confidence to its Min 4.
    assert_eq!(second.cards[PlayerId::P2].power, 4);

    game.unmake(undo_second);
    assert_eq!(game.position(), &after_first);
    assert_eq!(position_hash(game.position()), after_first_hash);
    game.unmake(undo_first);
    assert_eq!(game.position(), &before);
    assert_eq!(position_hash(game.position()), before_hash);
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
fn strict_catalog_match_executes_audited_ability_recovery_and_restores_undo() {
    let catalog = catalog();
    let registry = registry();
    let (_, rescue) = fully_supported_hands();
    let prepared = CatalogCombatStatMatchV1::new(
        input(
            [
                CardKey::new(1608, 3), // AI-Lycs, catalog/registry ability id 1418
                CardKey::new(123, 1),
                CardKey::new(124, 1),
                CardKey::new(138, 1),
            ],
            rescue,
            false,
        ),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    let CatalogCombatStatSourceDispositionV1::ExecutePostRound {
        identity,
        effect: CombatStatPostRoundEffectV1::RecoverPaidPillzOnDefeat,
        ..
    } = &prepared.preparation()[PlayerId::P1][0].ability
    else {
        panic!("AI-Lycs recovery was not admitted as post-round work")
    };
    assert_eq!(identity.catalog_id, Some(1418));
    assert_eq!(identity.registry_definition_id, 1418);
    assert_eq!(identity.registry_alias_ids.as_ref(), [577, 729, 1418, 2475]);
    assert_eq!(identity.description, "Defeat: Recover 2 Pillz Out Of 3");
    assert!(matches!(
        prepared.match_spec().cards[PlayerId::P1][0].ability,
        CombatStatSourcePlanV1::Execute {
            source_id: 1418,
            predicate: CombatStatPredicateV1::Always,
            effect: urban_recreation_rust::engine::CombatStatEffectV1::RecoverPaidPillzOnDefeat,
        }
    ));

    let mut game = prepared.new_game();
    let before = game.position().clone();
    let (report, undo) = game
        .make(BaseRulesRoundInput {
            first_mover: PlayerId::P1,
            selections: ByPlayer::new(
                BaseRulesSelection::new(0, 3, false),
                BaseRulesSelection::new(0, 3, false),
            ),
        })
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].pillz, 11); // 12 - 3 + ceil(3 * 2 / 3)
    assert_eq!(report.players[PlayerId::P1].life, 11);
    game.unmake(undo);
    assert_eq!(game.position(), &before);
}

#[test]
fn strict_catalog_match_executes_arnie_recovery_with_fury_cost() {
    let catalog = catalog();
    let registry = registry();
    let (_, rescue) = fully_supported_hands();
    let prepared = CatalogCombatStatMatchV1::new(
        input(
            [
                CardKey::new(907, 4), // Arnie, catalog/registry ability id 729
                CardKey::new(123, 1),
                CardKey::new(124, 1),
                CardKey::new(138, 1),
            ],
            rescue,
            false,
        ),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    let CatalogCombatStatSourceDispositionV1::ExecutePostRound { identity, .. } =
        &prepared.preparation()[PlayerId::P1][0].ability
    else {
        panic!("Arnie recovery was not admitted as post-round work")
    };
    assert_eq!(identity.catalog_id, Some(729));
    assert_eq!(identity.registry_definition_id, 729);

    let mut game = prepared.new_game();
    let before = game.position().clone();
    let (report, undo) = game
        .make(BaseRulesRoundInput {
            first_mover: PlayerId::P1,
            selections: ByPlayer::new(
                BaseRulesSelection::new(0, 4, true),
                BaseRulesSelection::new(0, 5, false),
            ),
        })
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.cards[PlayerId::P1].attack, 40);
    assert_eq!(report.players[PlayerId::P1].pillz, 10); // 12 - (4 + Fury 3) + ceil(7 * 2 / 3)
    game.unmake(undo);
    assert_eq!(game.position(), &before);
}

#[test]
fn strict_catalog_match_bridges_only_active_vortex_bonus_and_stop_bonus_disables_it() {
    let catalog = catalog();
    let registry = registry();
    let (_, rescue) = fully_supported_hands();
    let vortex = [
        CardKey::new(758, 1), // Deea
        CardKey::new(759, 1), // Sunder
        CardKey::new(123, 1),
        CardKey::new(124, 1),
    ];
    let prepared = CatalogCombatStatMatchV1::new(
        input(vortex, rescue, false),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    let CatalogCombatStatSourceDispositionV1::ExecutePostRound { identity, .. } =
        &prepared.preparation()[PlayerId::P1][0].bonus
    else {
        panic!("active Vortex bonus was not bridged to post-round recovery")
    };
    // Catalog bonus id 43 is intentionally preserved as catalog provenance, while the
    // registry identity is pinned to the capture definition 577 rather than registry id 43.
    assert_eq!(identity.catalog_id, Some(43));
    assert_eq!(identity.registry_definition_id, 577);
    assert_eq!(identity.registry_alias_ids.as_ref(), [577, 729, 1418, 2475]);
    assert!(matches!(
        prepared.match_spec().cards[PlayerId::P1][0].bonus,
        CombatStatSourcePlanV1::Execute {
            source_id: 577,
            predicate: CombatStatPredicateV1::Always,
            effect: urban_recreation_rust::engine::CombatStatEffectV1::RecoverPaidPillzOnDefeat,
        }
    ));

    let mut game = prepared.new_game();
    let before = game.position().clone();
    let (report, undo) = game
        .make(BaseRulesRoundInput {
            first_mover: PlayerId::P1,
            selections: ByPlayer::new(
                BaseRulesSelection::new(0, 3, false),
                BaseRulesSelection::new(0, 0, false),
            ),
        })
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].pillz, 11);
    game.unmake(undo);
    assert_eq!(game.position(), &before);

    let stop_bonus = [
        CardKey::new(773, 3), // Kobalth: Stop Opp. Bonus, with a live Vortex bonus
        CardKey::new(758, 1),
        CardKey::new(123, 1),
        CardKey::new(124, 1),
    ];
    let stopped = CatalogCombatStatMatchV1::new(
        input(vortex, stop_bonus, false),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        stopped.preparation()[PlayerId::P2][0].ability,
        CatalogCombatStatSourceDispositionV1::Execute { .. }
    ));
    let mut game = stopped.new_game();
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
    assert!(!report.cards[PlayerId::P1].won);
    // A live Vortex bonus would restore the minimum one Pillz after a zero-Pillz loss;
    // Kobalth's existing Stop Bonus behavior suppresses it without adding Stop Ability.
    assert_eq!(report.players[PlayerId::P1].pillz, 12);
    game.unmake(undo);
    assert_eq!(game.position(), &before);
}

#[test]
fn strict_catalog_match_bridges_the_active_riots_bonus_and_static_vod_abilities() {
    let catalog = catalog();
    let registry = registry();
    let (_, rescue) = fully_supported_hands();
    let riots = [
        CardKey::new(1208, 1), // Molder: no ability
        CardKey::new(1209, 1), // Pr Hartnell: no ability; activates the bonus
        CardKey::new(1212, 1), // Boomstock Cr: no ability
        CardKey::new(1214, 1), // De Couture: no ability
    ];
    let prepared =
        CatalogCombatStatMatchV1::new(input(riots, rescue, false), &catalog, &registry, PROJECTION)
            .unwrap();
    let CatalogCombatStatSourceDispositionV1::ExecutePostRound {
        identity,
        effect: CombatStatPostRoundEffectV1::GainOnePillzOnVictoryOrDefeat,
        ..
    } = &prepared.preparation()[PlayerId::P1][0].bonus
    else {
        panic!("active Riots bonus was not bridged to post-round Pillz")
    };
    assert_eq!(identity.catalog_id, Some(47));
    assert_eq!(identity.registry_definition_id, 1034);
    assert_eq!(
        identity.registry_alias_ids.as_ref(),
        [1034, 1375, 4111, 5085, 5520]
    );
    assert_eq!(identity.description, "Victory Or Defeat : +1 Pillz");
    assert!(matches!(
        prepared.match_spec().cards[PlayerId::P1][0].bonus,
        CombatStatSourcePlanV1::Execute {
            source_id: 1034,
            predicate: CombatStatPredicateV1::Always,
            effect:
                urban_recreation_rust::engine::CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat,
        }
    ));

    let mut game = prepared.new_game();
    let before = game.position().clone();
    let (loss, undo) = game
        .make(BaseRulesRoundInput {
            first_mover: PlayerId::P1,
            selections: ByPlayer::new(
                BaseRulesSelection::new(0, 0, false),
                BaseRulesSelection::new(0, 0, false),
            ),
        })
        .unwrap();
    assert!(!loss.cards[PlayerId::P1].won);
    assert_eq!(loss.players[PlayerId::P1].life, 11);
    assert_eq!(loss.players[PlayerId::P1].pillz, 13);
    game.unmake(undo);
    assert_eq!(game.position(), &before);

    let inactive = CatalogCombatStatMatchV1::new(
        input(
            [
                CardKey::new(1208, 1), // singleton Riots: its bonus is inactive
                CardKey::new(123, 1),
                CardKey::new(124, 1),
                CardKey::new(138, 1),
            ],
            rescue,
            false,
        ),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        inactive.preparation()[PlayerId::P1][0].bonus,
        CatalogCombatStatSourceDispositionV1::Absent
    ));
    for (key, id) in [
        (CardKey::new(1568, 3), 1375), // Pr Hide
        (CardKey::new(2513, 4), 4111), // Alba
        (CardKey::new(808, 2), 5085),  // Bonnie Ld
        (CardKey::new(808, 1), 5520),  // Bonnie Ld
    ] {
        let prepared = CatalogCombatStatMatchV1::new(
            input(
                [
                    key,
                    CardKey::new(123, 1),
                    CardKey::new(124, 1),
                    CardKey::new(138, 1),
                ],
                rescue,
                false,
            ),
            &catalog,
            &registry,
            PROJECTION,
        )
        .unwrap();
        assert!(matches!(
            prepared.preparation()[PlayerId::P1][0].ability,
            CatalogCombatStatSourceDispositionV1::ExecutePostRound {
                ref identity,
                effect: CombatStatPostRoundEffectV1::GainOnePillzOnVictoryOrDefeat,
                ..
            } if identity.catalog_id == Some(id) && identity.registry_definition_id == id
        ));
        assert!(matches!(
            prepared.match_spec().cards[PlayerId::P1][0].ability,
            CombatStatSourcePlanV1::Execute {
                source_id,
                predicate: CombatStatPredicateV1::Always,
                effect: urban_recreation_rust::engine::CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat,
            } if source_id == id
        ));
    }

    let argos = CatalogCombatStatMatchV1::new(
        input(
            [
                CardKey::new(1333, 2), // Argos: the sole printed Ability:1158 source
                CardKey::new(123, 1),
                CardKey::new(124, 1),
                CardKey::new(138, 1),
            ],
            rescue,
            false,
        ),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        argos.preparation()[PlayerId::P1][0].ability,
        CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            ref identity,
            effect: CombatStatPostRoundEffectV1::GainTwoPillzOnDefeatMaxEleven,
            ..
        } if identity.catalog_id == Some(1158) && identity.registry_definition_id == 1158
    ));
    assert!(matches!(
        argos.match_spec().cards[PlayerId::P1][0].ability,
        CombatStatSourcePlanV1::Execute {
            source_id: 1158,
            predicate: CombatStatPredicateV1::Always,
            effect:
                urban_recreation_rust::engine::CombatStatEffectV1::GainTwoPillzOnDefeatMaxEleven,
        }
    ));
    let argos_level_one = CatalogCombatStatMatchV1::new(
        input(
            [
                CardKey::new(1333, 1), // no printed ability: never synthesize 1158 by card name
                CardKey::new(123, 1),
                CardKey::new(124, 1),
                CardKey::new(138, 1),
            ],
            rescue,
            false,
        ),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        argos_level_one.preparation()[PlayerId::P1][0].ability,
        CatalogCombatStatSourceDispositionV1::Absent
    ));

    // Atess has the same printed text but none of her level-specific identities has an
    // audited registry definition. Description equality must not inherit a known alias.
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(
                [
                    CardKey::new(2340, 4),
                    CardKey::new(123, 1),
                    CardKey::new(124, 1),
                    CardKey::new(138, 1),
                ],
                rescue,
                false,
            ),
            &catalog,
            &registry,
            PROJECTION,
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            hand_slot,
            source_kind: CombatStatEffectSourceV1::Ability,
            catalog_id: Some(3322),
            ref description,
            ..
        }) if hand_slot.get() == 0 && description == "Victory Or Defeat : +1 Pillz"
    ));

    let stop_bonus = [
        CardKey::new(773, 3), // Kobalth: Stop Opp. Bonus
        CardKey::new(758, 1),
        CardKey::new(123, 1),
        CardKey::new(124, 1),
    ];
    let stopped = CatalogCombatStatMatchV1::new(
        input(riots, stop_bonus, false),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    let mut game = stopped.new_game();
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
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].pillz, 12);
    game.unmake(undo);
    assert_eq!(game.position(), &before);
}

#[test]
fn strict_catalog_match_admits_the_plain_heal_grammar_and_latches_it_on_a_win() {
    let catalog = catalog();
    let registry = registry();
    let (_, opponent) = fully_supported_hands();
    let lianah = CardKey::new(978, 3);
    let hand = [
        lianah,
        CardKey::new(123, 1),
        CardKey::new(124, 1),
        CardKey::new(138, 1),
    ];
    let prepared = CatalogCombatStatMatchV1::new(
        input(hand, opponent, false),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    let CatalogCombatStatSourceDispositionV1::ExecutePostRound {
        identity,
        effect,
        predicate,
    } = &prepared.preparation()[PlayerId::P1][0].ability
    else {
        panic!("Lianah Ld L3 was not prepared as a latching Heal")
    };
    assert_eq!(identity.catalog_id, Some(3526));
    assert_eq!(identity.registry_definition_id, 3526);
    assert_eq!(identity.registry_alias_ids.as_ref(), [3526]);
    assert_eq!(
        *effect,
        CombatStatPostRoundEffectV1::HealLifeOnVictory {
            life: 1,
            maximum: 20
        }
    );
    assert_eq!(*predicate, CombatStatPredicateV1::Always);
    assert!(matches!(
        prepared.match_spec().cards[PlayerId::P1][0].ability,
        CombatStatSourcePlanV1::Execute {
            source_id: 3526,
            predicate: CombatStatPredicateV1::Always,
            effect: urban_recreation_rust::engine::CombatStatEffectV1::HealLifeOnVictory {
                life: 1,
                maximum: 20
            },
        }
    ));

    // The strict match plays the latch: a won round pays nothing, the next round pays one
    // Life after that round's damage, and undo restores the latch-free position exactly.
    let mut game = prepared.new_game();
    let before = game.position().clone();
    let before_hash = position_hash(&before);
    let (first, undo_first) = game
        .make(BaseRulesRoundInput {
            first_mover: PlayerId::P1,
            selections: ByPlayer::new(
                BaseRulesSelection::new(0, 3, false),
                BaseRulesSelection::new(0, 0, false),
            ),
        })
        .unwrap();
    assert!(first.cards[PlayerId::P1].won);
    assert_eq!(first.players[PlayerId::P1].life, 12);
    assert_eq!(game.position().latched[PlayerId::P1].len(), 1);
    let after_first = game.position().clone();
    let after_first_hash = position_hash(&after_first);
    let (second, undo_second) = game
        .make(BaseRulesRoundInput {
            first_mover: PlayerId::P2,
            selections: ByPlayer::new(
                BaseRulesSelection::new(1, 0, false),
                BaseRulesSelection::new(1, 4, false),
            ),
        })
        .unwrap();
    assert!(second.cards[PlayerId::P2].won);
    assert_eq!(
        second.players[PlayerId::P1].life,
        12 - second.cards[PlayerId::P2].damage + 1
    );
    game.unmake(undo_second);
    assert_eq!(game.position(), &after_first);
    assert_eq!(position_hash(game.position()), after_first_hash);
    game.unmake(undo_first);
    assert_eq!(game.position(), &before);
    assert_eq!(position_hash(game.position()), before_hash);
    assert!(game.position().latched[PlayerId::P1].is_empty());

    // Lianah's lower levels print `Heal 1 Max. 14` and `Heal 1 Max. 17` under ids the
    // registry never captured; neither level constructs.
    for key in [CardKey::new(978, 1), CardKey::new(978, 2)] {
        assert!(CatalogCombatStatMatchV1::new(
            input(
                [
                    key,
                    CardKey::new(123, 1),
                    CardKey::new(124, 1),
                    CardKey::new(138, 1)
                ],
                opponent,
                false
            ),
            &catalog,
            &registry,
            PROJECTION,
        )
        .is_err());
    }

    // Every other plain record is the same grammar and is admitted with its own numbers:
    // Campbell level 4 (`963`) and level 3 (`4625`), Bose (`1501`), Loretta (`751`).
    for (key, catalog_id, description, life, maximum) in [
        (CardKey::new(1137, 4), 963, "Heal 1 Max. 15", 1, 15),
        (CardKey::new(1137, 3), 4625, "Heal 1 Max. 15", 1, 15),
        (CardKey::new(1672, 2), 1501, "Heal 2 Max. 10", 2, 10),
        (CardKey::new(929, 2), 751, "Heal 2 Max. 10", 2, 10),
    ] {
        let prepared = CatalogCombatStatMatchV1::new(
            input(
                [
                    key,
                    CardKey::new(123, 1),
                    CardKey::new(124, 1),
                    CardKey::new(138, 1),
                ],
                opponent,
                false,
            ),
            &catalog,
            &registry,
            PROJECTION,
        )
        .unwrap_or_else(|error| panic!("{key:?} {description}: {error}"));
        let CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity, effect, ..
        } = &prepared.preparation()[PlayerId::P1][0].ability
        else {
            panic!("{key:?} {description} was not prepared as a latching Heal")
        };
        assert_eq!(identity.catalog_id, Some(catalog_id));
        // The text resolves to one definition; structurally identical records such as
        // Campbell's `4625` are its aliases, and the row's id must be one of them.
        assert!(identity.registry_alias_ids.contains(&catalog_id));
        assert!(identity
            .registry_alias_ids
            .contains(&identity.registry_definition_id));
        assert_eq!(identity.description, description);
        assert_eq!(
            *effect,
            CombatStatPostRoundEffectV1::HealLifeOnVictory { life, maximum }
        );
    }

    // Campbell level 2 prints the same text under `4624`, an id the registry never
    // captured, so it is refused: the row must be a real alias of the resolved definition.
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(
                [
                    CardKey::new(1137, 2),
                    CardKey::new(123, 1),
                    CardKey::new(124, 1),
                    CardKey::new(138, 1)
                ],
                opponent,
                false
            ),
            &catalog,
            &registry,
            PROJECTION,
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            source_kind: CombatStatEffectSourceV1::Ability,
            catalog_id: Some(4624),
            ..
        })
    ));

    // A same-text row under a numeric identity that is not a registry alias of that text
    // cannot latch: description equality alone is never authority for a Life effect.
    let mismatch = catalog_with_ability_alias(CardKey::new(123, 1), 963, "Heal 1 Max. 20");
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input([CardKey::new(123, 1), CardKey::new(124, 1), CardKey::new(138, 1), CardKey::new(139, 1)], opponent, false),
            &mismatch,
            &registry,
            PROJECTION,
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            hand_slot,
            source_kind: CombatStatEffectSourceV1::Ability,
            catalog_id: Some(963),
            ref description,
            registry_definition_id: 3526,
            ..
        }) if hand_slot.get() == 0 && description == "Heal 1 Max. 20"
    ));

    // The other latch conditions are other grammars and stay closed.
    for (catalog_id, description) in [
        (1625, "Defeat : Heal 1 Max. 13"),
        (5692, "Asymmetry: Heal 1 Max. 16"),
    ] {
        let conditional = catalog_with_ability_alias(CardKey::new(123, 1), catalog_id, description);
        assert!(matches!(
            CatalogCombatStatMatchV1::new(
                input([CardKey::new(123, 1), CardKey::new(124, 1), CardKey::new(138, 1), CardKey::new(139, 1)], opponent, false),
                &conditional,
                &registry,
                PROJECTION,
            ),
            Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
                player: PlayerId::P1,
                source_kind: CombatStatEffectSourceV1::Ability,
                catalog_id: Some(actual),
                ..
            }) if actual == catalog_id
        ));
    }
}

#[test]
fn strict_catalog_match_admits_only_anitas_exact_courage_damage_life_ability() {
    let catalog = catalog();
    let registry = registry();
    let (_, opponent) = fully_supported_hands();
    let anita_hand = [
        CardKey::new(448, 3),
        CardKey::new(441, 1),
        CardKey::new(444, 1),
        CardKey::new(445, 1),
    ];
    let prepared = CatalogCombatStatMatchV1::new(
        input(anita_hand, opponent, false),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    let CatalogCombatStatSourceDispositionV1::ExecutePostRound {
        identity, effect, ..
    } = &prepared.preparation()[PlayerId::P1][0].ability
    else {
        panic!("Anita L3 was not prepared as Courage damage-to-Life")
    };
    assert_eq!(identity.catalog_id, Some(274));
    assert_eq!(identity.registry_definition_id, 274);
    // Ellie's own definition 843 is now captured and is structurally identical to Anita's
    // 274, so the shared text resolves to both. The alias set is provenance: admission stays
    // locked to Anita's card key and catalog id, which the Ellie/Lorea cases below prove.
    assert_eq!(identity.registry_alias_ids.as_ref(), [274, 843]);
    assert_eq!(
        *effect,
        CombatStatPostRoundEffectV1::GainLifeEqualToFinalDamageOnCourageVictory
    );
    assert!(matches!(
        prepared.match_spec().cards[PlayerId::P1][0].ability,
        CombatStatSourcePlanV1::Execute {
            source_id: 274,
            predicate: CombatStatPredicateV1::OwnerMovesFirst,
            effect: urban_recreation_rust::engine::CombatStatEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
        }
    ));

    // Anita's lower levels print no ability. Even a malformed catalog that gives either
    // level the exact source text and id cannot borrow her level-three authority.
    for key in [CardKey::new(448, 1), CardKey::new(448, 2)] {
        let lookalike = catalog_with_anita_courage_life_alias(key);
        assert!(matches!(
            CatalogCombatStatMatchV1::new(
                input([key, CardKey::new(123, 1), CardKey::new(124, 1), CardKey::new(138, 1)], opponent, false),
                &lookalike,
                &registry,
                PROJECTION,
            ),
            Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
                player: PlayerId::P1,
                hand_slot,
                source_kind: CombatStatEffectSourceV1::Ability,
                catalog_id: Some(274),
                ref description,
                ..
            }) if hand_slot.get() == 0 && description == "Courage: +1 Life Per Dmg"
        ));
    }

    // Ellie and Lorea naturally print exactly the same text. Their distinct ability ids
    // are not aliases for Anita's catalog row.
    for (key, catalog_id) in [(CardKey::new(1017, 2), 843), (CardKey::new(1183, 2), 1010)] {
        assert!(matches!(
            CatalogCombatStatMatchV1::new(
                input([key, CardKey::new(123, 1), CardKey::new(124, 1), CardKey::new(138, 1)], opponent, false),
                &catalog,
                &registry,
                PROJECTION,
            ),
            Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
                player: PlayerId::P1,
                hand_slot,
                source_kind: CombatStatEffectSourceV1::Ability,
                catalog_id: Some(actual_catalog_id),
                ref description,
                ..
            }) if hand_slot.get() == 0
                && actual_catalog_id == catalog_id
                && description == "Courage: +1 Life Per Dmg"
        ));
    }

    // A captured Copy may use Ability or Bonus provenance dynamically, but no canonical
    // catalog bonus can materialize Anita's effect. An invented active Uppers bonus is
    // therefore rejected before it can borrow registry id 274.
    let bonus_lookalike = catalog_with_anita_courage_life_bonus();
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(
                [
                    CardKey::new(143, 1),
                    CardKey::new(165, 1),
                    CardKey::new(166, 1),
                    CardKey::new(167, 1),
                ],
                opponent,
                false,
            ),
            &bonus_lookalike,
            &registry,
            PROJECTION,
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            source_kind: CombatStatEffectSourceV1::Bonus,
            catalog_id: Some(274),
            ref description,
            ..
        }) if description == "Courage: +1 Life Per Dmg"
    ));

    // Copy is resolved from live capture data, not canonical card rows. Gwen cannot turn
    // an opposing Anita into catalog source 274 merely because the eventual dynamic
    // source would be a card Ability.
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(
                [
                    CardKey::new(159, 3), // Gwen: Copy: Opp. Ability.
                    CardKey::new(123, 1),
                    CardKey::new(124, 1),
                    CardKey::new(138, 1),
                ],
                anita_hand,
                false,
            ),
            &catalog,
            &registry,
            PROJECTION,
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            hand_slot,
            source_kind: CombatStatEffectSourceV1::Ability,
            catalog_id: Some(5558),
            ref description,
            ..
        }) if hand_slot.get() == 0 && description == "Copy: Opp. Ability"
    ));

    // An exact card/id/description remains non-executable if the registry definition is
    // malformed. Strict construction must not promote an alias group by text alone.
    let malformed = registry_with_malformed_anita_courage_life();
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(anita_hand, opponent, false),
            &catalog,
            &malformed,
            PROJECTION,
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            hand_slot,
            source_kind: CombatStatEffectSourceV1::Ability,
            catalog_id: Some(274),
            ref description,
            registry_definition_id: 274,
            ..
        }) if hand_slot.get() == 0 && description == "Courage: +1 Life Per Dmg"
    ));
}

#[test]
fn strict_catalog_match_admits_only_card_key_locked_victory_or_defeat_life_sources() {
    let catalog = catalog();
    let registry = registry();
    let (_, rescue) = fully_supported_hands();
    for (key, catalog_id, registry_definition_id, life, reduces_opponent) in [
        (CardKey::new(1586, 2), 1396, 1396, 1, false), // Schumi L2
        (CardKey::new(820, 3), 5835, 5835, 1, false),  // Scott Ld L3
        (CardKey::new(820, 4), 2992, 2992, 1, false),  // Scott Ld L4
        (CardKey::new(2693, 2), 5799, 5799, 1, false), // Zerkov L2
        (CardKey::new(2693, 3), 5800, 5799, 1, false), // Zerkov L3 catalog alias
        (CardKey::new(2693, 4), 5801, 5799, 1, false), // Zerkov L4 catalog alias
        (CardKey::new(2693, 5), 5802, 5802, 2, false), // Zerkov L5
        (CardKey::new(1676, 2), 2944, 2944, 2, false), // Kora Mail Ld L2
        (CardKey::new(1788, 2), 1628, 1628, 1, true),  // Uuber L2
    ] {
        let prepared = CatalogCombatStatMatchV1::new(
            input(
                [
                    key,
                    CardKey::new(123, 1),
                    CardKey::new(124, 1),
                    CardKey::new(138, 1),
                ],
                rescue,
                false,
            ),
            &catalog,
            &registry,
            PROJECTION,
        )
        .unwrap_or_else(|error| panic!("{key:?} was not catalog-executable: {error}"));
        let CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity, effect, ..
        } = &prepared.preparation()[PlayerId::P1][0].ability
        else {
            panic!("{key:?} was not prepared as Victory Or Defeat Life")
        };
        assert_eq!(identity.catalog_id, Some(catalog_id));
        assert_eq!(identity.registry_definition_id, registry_definition_id);
        assert!(identity
            .registry_alias_ids
            .contains(&registry_definition_id));
        if reduces_opponent {
            assert_eq!(
                *effect,
                CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryOrDefeat {
                    life,
                    minimum: 1,
                }
            );
            assert!(matches!(
                prepared.match_spec().cards[PlayerId::P1][0].ability,
                CombatStatSourcePlanV1::Execute {
                    source_id,
                    predicate: CombatStatPredicateV1::Always,
                    effect: urban_recreation_rust::engine::CombatStatEffectV1::ReduceOpponentLifeOnVictoryOrDefeat {
                        life: 1,
                        minimum: 1,
                    },
                } if source_id == registry_definition_id
            ));
        } else {
            assert_eq!(
                *effect,
                CombatStatPostRoundEffectV1::GainLifeOnVictoryOrDefeat { life }
            );
            assert!(matches!(
                prepared.match_spec().cards[PlayerId::P1][0].ability,
                CombatStatSourcePlanV1::Execute {
                    source_id,
                    predicate: CombatStatPredicateV1::Always,
                    effect: urban_recreation_rust::engine::CombatStatEffectV1::GainLifeOnVictoryOrDefeat {
                        life: effect_life,
                    },
                } if source_id == registry_definition_id && effect_life == life
            ));
        }
    }

    // Scott L2 prints the same +1 Life text under catalog Ability:5834. It was never a
    // captured execution authority, so source text cannot borrow Scott L3's 5835 id.
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(
                [
                    CardKey::new(820, 2),
                    CardKey::new(123, 1),
                    CardKey::new(124, 1),
                    CardKey::new(138, 1),
                ],
                rescue,
                false,
            ),
            &catalog,
            &registry,
            PROJECTION,
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            hand_slot,
            source_kind: CombatStatEffectSourceV1::Ability,
            catalog_id: Some(5834),
            ref description,
            ..
        }) if hand_slot.get() == 0 && description == "Victory Or Defeat : +1 Life"
    ));

    // A different canonical card cannot claim Schumi's source id, even if its printed
    // text and numeric source are both changed to look identical.
    let lookalike = catalog_with_vod_life_lookalike(CardKey::new(1676, 2));
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(
                [
                    CardKey::new(1676, 2),
                    CardKey::new(123, 1),
                    CardKey::new(124, 1),
                    CardKey::new(138, 1),
                ],
                rescue,
                false,
            ),
            &lookalike,
            &registry,
            PROJECTION,
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            hand_slot,
            source_kind: CombatStatEffectSourceV1::Ability,
            catalog_id: Some(1396),
            ref description,
            ..
        }) if hand_slot.get() == 0 && description == "Victory Or Defeat : +1 Life"
    ));

    // Captured Copy may carry the reviewed identities as a Bonus, but the catalog has no
    // direct bonus authority for this family and must reject the synthetic source.
    let bonus_lookalike = catalog_with_vod_life_bonus();
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(
                [
                    CardKey::new(1586, 2),
                    CardKey::new(1676, 1),
                    CardKey::new(123, 1),
                    CardKey::new(124, 1),
                ],
                rescue,
                false,
            ),
            &bonus_lookalike,
            &registry,
            PROJECTION,
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            hand_slot,
            source_kind: CombatStatEffectSourceV1::Bonus,
            catalog_id: Some(1396),
            ref description,
            ..
        }) if hand_slot.get() == 0 && description == "Victory Or Defeat : +1 Life"
    ));
}

#[test]
fn strict_catalog_match_admits_only_two_printed_equalizer_opponent_life_sources() {
    let catalog = catalog();
    let registry = registry();
    let (_, rescue) = fully_supported_hands();
    for (key, catalog_id) in [(CardKey::new(1605, 2), 1415), (CardKey::new(841, 2), 4458)] {
        let prepared = CatalogCombatStatMatchV1::new(
            input(
                [
                    key,
                    CardKey::new(123, 1),
                    CardKey::new(124, 1),
                    CardKey::new(138, 1),
                ],
                rescue,
                false,
            ),
            &catalog,
            &registry,
            PROJECTION,
        )
        .unwrap_or_else(|error| panic!("{key:?} was not catalog-executable: {error}"));
        let CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity, effect, ..
        } = &prepared.preparation()[PlayerId::P1][0].ability
        else {
            panic!("{key:?} was not prepared as Equalizer opponent-Life")
        };
        assert_eq!(identity.catalog_id, Some(catalog_id));
        assert_eq!(identity.registry_definition_id, catalog_id);
        assert_eq!(identity.registry_alias_ids.as_ref(), [1415, 4458]);
        assert_eq!(
            *effect,
            CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars {
                per_star: 1,
                minimum: 2,
            }
        );
        assert!(matches!(
            prepared.match_spec().cards[PlayerId::P1][0].ability,
            CombatStatSourcePlanV1::Execute {
                source_id,
                predicate: CombatStatPredicateV1::Always,
                effect: urban_recreation_rust::engine::CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars { per_star: 1, minimum: 2 },
            } if source_id == catalog_id
        ));
    }

    // Gail L1 shares the text but not Gail L2's reviewed id; O Riley and El Cazador are
    // adjacent printed Equalizer Life forms. None may borrow the two canonical sources.
    for (key, catalog_id, description) in [
        (CardKey::new(841, 1), 5536, "Equalizer: - 1 Opp. Life Min 2"),
        (
            CardKey::new(2519, 2),
            4455,
            "Equalizer: - 1 Opp. Life Min 2",
        ),
        (
            CardKey::new(2519, 3),
            4125,
            "Equalizer: - 1 Opp. Life Min 2",
        ),
        (
            CardKey::new(2260, 3),
            5793,
            "Equalizer: - 1 Opp. Life Min 0",
        ),
    ] {
        assert!(
            matches!(
                CatalogCombatStatMatchV1::new(
                    input(
                        [
                            key,
                            CardKey::new(123, 1),
                            CardKey::new(124, 1),
                            CardKey::new(138, 1),
                        ],
                        rescue,
                        false,
                    ),
                    &catalog,
                    &registry,
                    PROJECTION,
                ),
                Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
                    player: PlayerId::P1,
                    hand_slot,
                    source_kind: CombatStatEffectSourceV1::Ability,
                    catalog_id: Some(actual_catalog_id),
                    description: ref actual_description,
                    ..
                }) if hand_slot.get() == 0
                    && actual_catalog_id == catalog_id
                    && actual_description == description
            ),
            "{key:?} must remain outside the exact Equalizer Life catalog slice"
        );
    }

    // Copy's captured Bonus provenance has no catalog authority. A synthetic clan bonus
    // bearing the exact reviewed id must therefore reject rather than becoming executable.
    let bonus_lookalike = catalog_with_equalizer_opponent_life_bonus();
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(
                [
                    CardKey::new(1605, 2),
                    CardKey::new(143, 1),
                    CardKey::new(165, 1),
                    CardKey::new(166, 1),
                ],
                rescue,
                false,
            ),
            &bonus_lookalike,
            &registry,
            PROJECTION,
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            source_kind: CombatStatEffectSourceV1::Bonus,
            catalog_id: Some(1415),
            ref description,
            ..
        }) if description == "Equalizer: - 1 Opp. Life Min 2"
    ));
}

#[test]
fn strict_catalog_match_preserves_dave_catalog_and_registry_life_identity() {
    let catalog = catalog();
    let registry = registry();
    let dave_hand = [
        CardKey::new(488, 4),
        CardKey::new(1062, 2),
        CardKey::new(1019, 2),
        CardKey::new(1918, 3),
    ];
    let opponent = [
        CardKey::new(128, 3),
        CardKey::new(432, 4),
        CardKey::new(486, 3),
        CardKey::new(166, 3),
    ];
    let prepared = CatalogCombatStatMatchV1::new(
        input(dave_hand, opponent, false),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    let CatalogCombatStatSourceDispositionV1::ExecutePostRound {
        identity, effect, ..
    } = &prepared.preparation()[PlayerId::P1][1].ability
    else {
        panic!("Dave's catalog ability was not prepared as Victory Life")
    };
    assert_eq!(identity.catalog_id, Some(888));
    // The catalog's stable source id is an alias, not the registry's canonical first
    // definition.  Strict construction keeps both identities rather than collapsing it
    // to description text or pretending the catalog id is the registry definition.
    assert_eq!(identity.registry_definition_id, 401);
    assert_eq!(identity.registry_alias_ids.as_ref(), [401, 888]);
    assert_eq!(
        *effect,
        CombatStatPostRoundEffectV1::GainLifeOnVictory { life: 2 }
    );
    assert!(matches!(
        prepared.match_spec().cards[PlayerId::P1][1].ability,
        CombatStatSourcePlanV1::Execute {
            source_id: 401,
            predicate: CombatStatPredicateV1::Always,
            effect: urban_recreation_rust::engine::CombatStatEffectV1::GainLifeOnVictory {
                life: 2
            },
        }
    ));
}

#[test]
fn strict_catalog_match_admits_alias_bound_defeat_life_and_lobos_reanimate_only() {
    let catalog = catalog();
    let registry = registry();
    let (_, opponent) = fully_supported_hands();
    let rescue = [
        CardKey::new(453, 3), // Lobo: Reanimate: +2 Life, Ability:4951
        CardKey::new(441, 1),
        CardKey::new(444, 1),
        CardKey::new(445, 1),
    ];
    let lobo = CatalogCombatStatMatchV1::new(
        input(rescue, opponent, false),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    let CatalogCombatStatSourceDispositionV1::ExecutePostRound {
        identity, effect, ..
    } = &lobo.preparation()[PlayerId::P1][0].ability
    else {
        panic!("Lobo's observed Reanimate was not prepared")
    };
    assert_eq!(identity.catalog_id, Some(4951));
    assert_eq!(identity.registry_definition_id, 4951);
    assert_eq!(identity.registry_alias_ids.as_ref(), [4951]);
    assert_eq!(
        *effect,
        CombatStatPostRoundEffectV1::ReanimateLife { life: 2 }
    );
    assert!(matches!(
        lobo.match_spec().cards[PlayerId::P1][0].ability,
        CombatStatSourcePlanV1::Execute {
            source_id: 4951,
            predicate: CombatStatPredicateV1::Always,
            effect: urban_recreation_rust::engine::CombatStatEffectV1::ReanimateLife { life: 2 },
        }
    ));

    let eugene_hand = [
        CardKey::new(1035, 3), // Eugene: Defeat: +2 Life, Ability:862
        CardKey::new(123, 1),
        CardKey::new(124, 1),
        CardKey::new(138, 1),
    ];
    let eugene = CatalogCombatStatMatchV1::new(
        input(eugene_hand, opponent, false),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    let CatalogCombatStatSourceDispositionV1::ExecutePostRound {
        identity, effect, ..
    } = &eugene.preparation()[PlayerId::P1][0].ability
    else {
        panic!("Eugene's Defeat Life was not prepared")
    };
    assert_eq!(identity.catalog_id, Some(862));
    assert_eq!(identity.registry_definition_id, 862);
    assert_eq!(identity.registry_alias_ids.as_ref(), [862, 1020, 4635]);
    assert_eq!(
        *effect,
        CombatStatPostRoundEffectV1::GainLifeOnDefeat { life: 2 }
    );

    // Eugene level 2 prints identical text under catalog Ability:5089, which is not an
    // alias of captured definition 862. Strict admission must reject text borrowing.
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(
                [
                    CardKey::new(1035, 2),
                    CardKey::new(123, 1),
                    CardKey::new(124, 1),
                    CardKey::new(138, 1),
                ],
                opponent,
                false,
            ),
            &catalog,
            &registry,
            PROJECTION,
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            hand_slot,
            source_kind: CombatStatEffectSourceV1::Ability,
            catalog_id: Some(5089),
            ref description,
            registry_definition_id: 862,
            ..
        }) if hand_slot.get() == 0 && description == "Defeat: +2 Life"
    ));
}

#[test]
fn strict_catalog_match_bridges_only_the_active_jungo_victory_life_bonus() {
    let catalog = catalog();
    let registry = registry();
    let jungo = [
        CardKey::new(584, 1),
        CardKey::new(585, 1),
        CardKey::new(587, 1),
        CardKey::new(589, 1),
    ];
    let (_, opponent) = fully_supported_hands();
    let prepared = CatalogCombatStatMatchV1::new(
        input(jungo, opponent, false),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();

    for slot in 0..4 {
        let CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity, effect, ..
        } = &prepared.preparation()[PlayerId::P1][slot].bonus
        else {
            panic!("active Jungo bonus in slot {slot} was not executable")
        };
        assert_eq!(identity.catalog_id, Some(41));
        assert_eq!(identity.registry_definition_id, 401);
        assert_eq!(identity.registry_alias_ids.as_ref(), [401, 888]);
        assert_eq!(
            *effect,
            CombatStatPostRoundEffectV1::GainLifeOnVictory { life: 2 }
        );
        assert!(matches!(
            prepared.match_spec().cards[PlayerId::P1][slot].bonus,
            CombatStatSourcePlanV1::Execute {
                source_id: 401,
                predicate: CombatStatPredicateV1::Always,
                effect: urban_recreation_rust::engine::CombatStatEffectV1::GainLifeOnVictory {
                    life: 2
                },
            }
        ));
    }

    let mut game = prepared.new_game();
    let before = game.position().clone();
    let (report, undo) = game
        .make(BaseRulesRoundInput {
            first_mover: PlayerId::P1,
            selections: ByPlayer::new(
                BaseRulesSelection::new(2, 12, false),
                BaseRulesSelection::new(0, 0, false),
            ),
        })
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].life, 14);
    game.unmake(undo);
    assert_eq!(game.position(), &before);
}

#[test]
fn strict_catalog_match_bridges_only_active_roots_and_gheist_soa_bonuses() {
    let catalog = catalog();
    let registry = registry();
    let rescue = [
        CardKey::new(1089, 2), // Sue: -1 Opp Power And Damage, Min 3
        CardKey::new(441, 1),
        CardKey::new(444, 1),
        CardKey::new(445, 1),
    ];
    let cases = [
        (
            [
                CardKey::new(141, 1), // Ataoualpet, active Roots bonus
                CardKey::new(171, 1),
                CardKey::new(123, 1),
                CardKey::new(124, 1),
            ],
            41,
            (2, 4),
        ),
        (
            [
                CardKey::new(240, 1), // Lilith, active GHEIST bonus
                CardKey::new(241, 1),
                CardKey::new(123, 1),
                CardKey::new(124, 1),
            ],
            94,
            (5, 1),
        ),
    ];

    for (soa_hand, registry_definition_id, expected_stats) in cases {
        let prepared = CatalogCombatStatMatchV1::new(
            input(soa_hand, rescue, false),
            &catalog,
            &registry,
            PROJECTION,
        )
        .unwrap();
        let CatalogCombatStatSourceDispositionV1::Execute {
            identity,
            effect: SupportedEffectV1::StopOpponentAbility,
            predicate: CombatStatPredicateV1::Always,
        } = &prepared.preparation()[PlayerId::P1][0].bonus
        else {
            panic!("active SOA clan bonus was not executable")
        };
        assert_eq!(identity.registry_definition_id, registry_definition_id);
        assert!(identity
            .registry_alias_ids
            .contains(&registry_definition_id));

        let mut game = prepared.new_game();
        let before = game.position().clone();
        let before_hash = position_hash(&before);
        let (round, undo) = game
            .make(BaseRulesRoundInput {
                first_mover: PlayerId::P1,
                selections: ByPlayer::new(
                    BaseRulesSelection::new(0, 0, false),
                    BaseRulesSelection::new(0, 0, false),
                ),
            })
            .unwrap();
        assert_eq!(
            (
                round.cards[PlayerId::P1].power,
                round.cards[PlayerId::P1].damage,
            ),
            expected_stats
        );
        // SOA leaves the opposing Rescue bonus intact.
        assert_eq!(round.cards[PlayerId::P2].attack, 18);
        game.unmake(undo);
        assert_eq!(game.position(), &before);
        assert_eq!(position_hash(game.position()), before_hash);
    }
}

#[test]
fn strict_catalog_match_bridges_only_the_active_komboka_victory_pillz_and_life_bonus() {
    let base_catalog = catalog();
    let base_registry = registry();
    let (_, rescue) = fully_supported_hands();
    let komboka = [
        CardKey::new(1868, 1), // Kubra: no ability
        CardKey::new(1870, 1), // Pavam Cr: no ability
        CardKey::new(1875, 1), // Duygu: no ability
        CardKey::new(1876, 2), // Seta: no ability
    ];
    let prepared = CatalogCombatStatMatchV1::new(
        input(komboka, rescue, false),
        &base_catalog,
        &base_registry,
        PROJECTION,
    )
    .unwrap();

    for slot in 0..4 {
        let CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity,
            effect: CombatStatPostRoundEffectV1::GainOnePillzAndLifeOnVictory,
            ..
        } = &prepared.preparation()[PlayerId::P1][slot].bonus
        else {
            panic!("active Komboka bonus in slot {slot} was not executable")
        };
        assert_eq!(identity.catalog_id, Some(53));
        assert_eq!(identity.registry_definition_id, 1714);
        // Carnibox Ability:3356 is a structural alias, never catalog execution authority.
        assert_eq!(identity.registry_alias_ids.as_ref(), [1714, 3356]);
        assert!(matches!(
            prepared.match_spec().cards[PlayerId::P1][slot].bonus,
            CombatStatSourcePlanV1::Execute {
                source_id: 1714,
                predicate: CombatStatPredicateV1::Always,
                effect:
                    urban_recreation_rust::engine::CombatStatEffectV1::GainOnePillzAndLifeOnVictory,
            }
        ));
    }

    let mut game = prepared.new_game();
    let before = game.position().clone();
    let (report, undo) = game
        .make(BaseRulesRoundInput {
            first_mover: PlayerId::P1,
            selections: ByPlayer::new(
                BaseRulesSelection::new(0, 12, false),
                BaseRulesSelection::new(0, 0, false),
            ),
        })
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].life, 13);
    assert_eq!(report.players[PlayerId::P1].pillz, 1);
    game.unmake(undo);
    assert_eq!(game.position(), &before);

    let inactive = CatalogCombatStatMatchV1::new(
        input(
            [
                CardKey::new(1868, 1), // singleton Komboka: no active clan bonus
                CardKey::new(123, 1),
                CardKey::new(124, 1),
                CardKey::new(138, 1),
            ],
            rescue,
            false,
        ),
        &base_catalog,
        &base_registry,
        PROJECTION,
    )
    .unwrap();
    assert!(matches!(
        inactive.preparation()[PlayerId::P1][0].bonus,
        CatalogCombatStatSourceDispositionV1::Absent
    ));

    // The same text and structural alias carried by Carnibox's Ability:3356 must stay
    // outside the bonus-only clan bridge.
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(
                [
                    CardKey::new(2344, 2), // Carnibox Ability:3356
                    CardKey::new(123, 1),
                    CardKey::new(124, 1),
                    CardKey::new(138, 1),
                ],
                rescue,
                false,
            ),
            &base_catalog,
            &base_registry,
            PROJECTION,
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            hand_slot,
            source_kind: CombatStatEffectSourceV1::Ability,
            catalog_id: Some(3356),
            ref description,
            registry_definition_id: 1714,
            ..
        }) if hand_slot.get() == 0 && description == "+1 Pillz And Life"
    ));

    for (catalog, registry, expected_catalog_id) in [
        (catalog_with_komboka_context(54, 999), registry(), Some(999)),
        (catalog_with_komboka_context(61, 53), registry(), Some(53)),
        (catalog(), registry_with_malformed_komboka(), Some(53)),
    ] {
        assert!(matches!(
            CatalogCombatStatMatchV1::new(
                input(komboka, rescue, false),
                &catalog,
                &registry,
                PROJECTION,
            ),
            Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
                player: PlayerId::P1,
                hand_slot,
                source_kind: CombatStatEffectSourceV1::Bonus,
                catalog_id,
                ref description,
                registry_definition_id: 1714,
                ..
            }) if hand_slot.get() == 0 && catalog_id == expected_catalog_id
                && description == "+1 Pillz And Life"
        ));
    }
}

#[test]
fn strict_catalog_match_admits_only_the_two_observed_reprisal_soa_aliases() {
    let catalog = catalog();
    let registry = registry();
    let (_, rescue) = fully_supported_hands();
    let cases = [
        (
            [
                CardKey::new(1498, 4), // Spidee, captured registry alias 1310
                CardKey::new(441, 1),
                CardKey::new(444, 1),
                CardKey::new(445, 1),
            ],
            1310,
        ),
        (
            [
                CardKey::new(2042, 3), // Bulza Cr, captured registry alias 2073
                CardKey::new(281, 2),
                CardKey::new(282, 1),
                CardKey::new(287, 1),
            ],
            2073,
        ),
    ];

    for (hand, source_id) in cases {
        let prepared = CatalogCombatStatMatchV1::new(
            input(hand, rescue, false),
            &catalog,
            &registry,
            PROJECTION,
        )
        .unwrap_or_else(|error| panic!("reprisal SOA {source_id} was not executable: {error}"));
        let CatalogCombatStatSourceDispositionV1::Execute {
            identity,
            effect: SupportedEffectV1::StopOpponentAbility,
            predicate: CombatStatPredicateV1::OwnerMovesSecond,
        } = &prepared.preparation()[PlayerId::P1][0].ability
        else {
            panic!("reprisal SOA {source_id} did not compile to its exact defender control")
        };
        assert_eq!(identity.catalog_id, Some(source_id));
        assert_eq!(identity.registry_definition_id, source_id);
        assert_eq!(identity.registry_alias_ids.as_ref(), [1310, 2073]);
    }

    // These printed same-text cards are not observed execution authorities.  Each must
    // remain a strict selected-source rejection even though it belongs to the same text
    // family as Spidee level 4 and Bulza Cr.
    for (key, source_id) in [
        (CardKey::new(1136, 3), 964),  // Carmen
        (CardKey::new(1288, 2), 1115), // Harmonia
        (CardKey::new(1498, 3), 4394), // Spidee level 3
        (CardKey::new(1530, 5), 1337), // Leone Cr
        (CardKey::new(2688, 5), 5762), // Jax Draven
    ] {
        let mut hand = fully_supported_hands().0;
        hand[0] = key;
        assert!(matches!(
            CatalogCombatStatMatchV1::new(input(hand, rescue, false), &catalog, &registry, PROJECTION),
            Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
                player: PlayerId::P1,
                hand_slot,
                source_kind: CombatStatEffectSourceV1::Ability,
                catalog_id: Some(id),
                ref description,
                ..
            }) if hand_slot.get() == 0 && id == source_id
                && description == "Reprisal: Stop Opp. Ability"
        ));
    }
}

#[test]
fn strict_catalog_match_pins_the_active_piranas_stop_bonus_identity() {
    let catalog = catalog();
    let registry = registry();
    let (_, opponent) = fully_supported_hands();
    let prepared = CatalogCombatStatMatchV1::new(
        input(
            [
                CardKey::new(2349, 3), // Baldassare
                CardKey::new(1962, 2), // Sooko Cr: activates the Piranas bonus
                CardKey::new(123, 1),
                CardKey::new(124, 1),
            ],
            opponent,
            false,
        ),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    for slot in 0..2 {
        let CatalogCombatStatSourceDispositionV1::Execute {
            identity,
            effect: SupportedEffectV1::StopOpponentBonus,
            predicate: CombatStatPredicateV1::Always,
        } = &prepared.preparation()[PlayerId::P1][slot].bonus
        else {
            panic!("active Piranas Stop Bonus in slot {slot} was not executable")
        };
        assert_eq!(identity.catalog_id, Some(40));
        assert_eq!(identity.registry_definition_id, 333);
        assert!(identity.registry_alias_ids.contains(&333));
    }
}

#[test]
fn strict_catalog_match_admits_unconditional_copy_and_rejects_conditional_variants() {
    let catalog = catalog();
    let registry = registry();
    let (_, p2) = fully_supported_hands();
    let with = |key: CardKey| {
        [
            key,
            CardKey::new(123, 1),
            CardKey::new(124, 1),
            CardKey::new(138, 1),
        ]
    };

    // Saki level three prints `Copy: Opp. Bonus` under catalog id 846, which is itself a
    // registry definition of that exact text and shape. It carries no effect of its own:
    // the plan only names which opposing source it adopts.
    let prepared = CatalogCombatStatMatchV1::new(
        input(with(CardKey::new(1020, 3)), p2, false),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    let CatalogCombatStatSourceDispositionV1::CopyOpponentSource {
        identity,
        copied,
        predicate,
    } = &prepared.preparation()[PlayerId::P1][0].ability
    else {
        panic!("Saki L3 was not prepared as an unconditional Copy")
    };
    assert_eq!(identity.catalog_id, Some(846));
    assert_eq!(identity.registry_definition_id, 846);
    assert_eq!(*copied, CopiedSourceKindV1::Bonus);
    assert_eq!(*predicate, CombatStatPredicateV1::Always);
    assert!(matches!(
        prepared.match_spec().cards[PlayerId::P1][0].ability,
        CombatStatSourcePlanV1::CopyOpponentSource {
            source_id: 846,
            copied: CopiedSourceKindV1::Bonus,
            predicate: CombatStatPredicateV1::Always,
        }
    ));

    // The reviewed conditional Copies are admitted with the predicate their printed text
    // names. Noctezuma Cr level three prints `Reprisal: Copy Opp. Bonus` under catalog id
    // 958; Nexus level three prints `Revenge: Copy Opp. Bonus` under 1751.
    for (key, catalog_id, description, copied, predicate) in [
        (
            CardKey::new(1134, 3),
            958,
            "Reprisal: Copy Opp. Bonus",
            CopiedSourceKindV1::Bonus,
            CombatStatPredicateV1::OwnerMovesSecond,
        ),
        (
            CardKey::new(1901, 4),
            1751,
            "Revenge: Copy Opp. Bonus",
            CopiedSourceKindV1::Bonus,
            CombatStatPredicateV1::OwnerLostPreviousRound,
        ),
        (
            CardKey::new(2605, 5),
            4972,
            "Revenge: Copy: Opp. Ability",
            CopiedSourceKindV1::Ability,
            CombatStatPredicateV1::OwnerLostPreviousRound,
        ),
    ] {
        let prepared = CatalogCombatStatMatchV1::new(
            input(with(key), p2, false),
            &catalog,
            &registry,
            PROJECTION,
        )
        .unwrap_or_else(|error| panic!("{key:?} {description}: {error}"));
        let CatalogCombatStatSourceDispositionV1::CopyOpponentSource {
            identity,
            copied: actual_copied,
            predicate: actual_predicate,
        } = &prepared.preparation()[PlayerId::P1][0].ability
        else {
            panic!("{key:?} was not prepared as a conditional Copy")
        };
        assert_eq!(identity.catalog_id, Some(catalog_id));
        assert_eq!(identity.registry_definition_id, catalog_id);
        assert_eq!(identity.description, description);
        assert_eq!(*actual_copied, copied);
        assert_eq!(*actual_predicate, predicate);
        assert_eq!(
            prepared.match_spec().cards[PlayerId::P1][0].ability,
            CombatStatSourcePlanV1::CopyOpponentSource {
                source_id: catalog_id,
                copied,
                predicate,
            }
        );
    }

    // Every Copy grammar outside the reviewed set stays fail-closed, including a Reprisal
    // whose payload is a stat rather than a source. Nexus and XU91 print an admitted text
    // but under a catalog id that is not a registry definition of it, so they stay closed
    // for the same reason Lorna's `752` does: description alone never admits.
    for (key, description) in [
        (CardKey::new(1596, 2), "Confidence: Copy: Opp. Power"),
        (CardKey::new(2520, 1), "Reprisal: Copy: Opp. Damage"),
        (CardKey::new(1531, 3), "Revenge: Copy Opp. Bonus"),
        (CardKey::new(1965, 4), "Revenge: Copy Opp. Bonus"),
    ] {
        assert!(
            matches!(
                CatalogCombatStatMatchV1::new(
                    input(with(key), p2, false),
                    &catalog,
                    &registry,
                    PROJECTION,
                ),
                Err(CatalogCombatStatMatchErrorV1::UnsupportedSource { .. })
            ),
            "{key:?} {description} must stay fail-closed",
        );
    }

    // A printed Copy whose catalog id is not itself a registry definition of that text
    // cannot borrow one by description alone.
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(with(CardKey::new(930, 3)), p2, false), // Lorna, catalog ability 752.
            &catalog,
            &registry,
            PROJECTION,
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            catalog_id: Some(752),
            ref description,
            ..
        }) if description == "Copy: Opp. Bonus"
    ));
}
#[test]
fn strict_catalog_match_defers_sasl_recovery_alias() {
    let catalog = catalog();
    let registry = registry();
    let (_, rescue) = fully_supported_hands();
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(
                [
                    CardKey::new(1178, 4), // Sasl Lovelace, catalog ability id 2475
                    CardKey::new(123, 1),
                    CardKey::new(124, 1),
                    CardKey::new(138, 1),
                ],
                rescue,
                false,
            ),
            &catalog,
            &registry,
            PROJECTION,
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            hand_slot,
            source_kind: CombatStatEffectSourceV1::Ability,
            catalog_id: Some(2475),
            ref description,
            ..
        }) if hand_slot.get() == 0 && description == "Defeat: Recover 2 Pillz Out Of 3"
    ));
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
    assert_eq!(provenance.catalog_context_policy_semantic_revision, 3);

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
    unsupported[0] = CardKey::new(157, 3); // Noon Steevens: Tune Out, a deferred grammar.
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

    let mut malformed_control = p1;
    malformed_control[0] = CardKey::new(1051, 2); // Angelo: non-zero SOA control value.
    assert!(matches!(
        CatalogCombatStatMatchV1::new(
            input(malformed_control, p2, false),
            &catalog,
            &registry,
            PROJECTION
        ),
        Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player: PlayerId::P1,
            hand_slot,
            source_kind: CombatStatEffectSourceV1::Ability,
            registry_definition_id: 877,
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

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StrictCoverageCapture {
    id: u64,
    battle_rule_id: u32,
    night: bool,
    players: Vec<StrictCoveragePlayer>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StrictCoveragePlayer {
    base_life: u16,
    base_pillz: u16,
    hand: Vec<StrictCoverageCard>,
}

#[derive(serde::Deserialize)]
struct StrictCoverageCard {
    id: u32,
    level: u8,
    index: u8,
}

/// This is intentionally a catalog-only scan rather than a replay gate: all 328 safe
/// capture files with two complete four-card hands participate, including the six that do
/// not have a replayable move history.  Keeping the eligible ID set in one test makes any
/// future documentation claim reproducible from canonical catalog context and the shared
/// registry, rather than from a hand-maintained list.
#[test]
fn strict_catalog_coverage_of_all_complete_captured_draws_is_pinned() {
    let catalog = catalog();
    let registry = registry();
    let mut scanned = 0_usize;
    let mut eligible = BTreeSet::new();
    let mut paths = fs::read_dir(root_path("captures/games"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    paths.sort();

    for path in paths {
        let bytes = fs::read(&path).unwrap();
        let capture: StrictCoverageCapture = serde_json::from_slice(&bytes)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        if capture.players.len() != 2 || capture.players.iter().any(|player| player.hand.len() != 4)
        {
            continue;
        }
        scanned += 1;
        let make_player = |player: &StrictCoveragePlayer| CatalogCombatStatPlayerInputV1 {
            initial_life: player.base_life,
            initial_pillz: player.base_pillz,
            hand: std::array::from_fn(|index| {
                let card = player
                    .hand
                    .iter()
                    .find(|card| usize::from(card.index) == index)
                    .unwrap_or_else(|| {
                        panic!("capture {} has no card at hand index {index}", capture.id)
                    });
                CardKey::new(card.id, card.level)
            }),
        };
        let input = CatalogCombatStatMatchInputV1 {
            battle_rule_id: capture.battle_rule_id,
            night: capture.night,
            players: ByPlayer::new(
                make_player(&capture.players[0]),
                make_player(&capture.players[1]),
            ),
        };
        if CatalogCombatStatMatchV1::new(input, &catalog, &registry, PROJECTION).is_ok() {
            eligible.insert(capture.id);
        }
    }

    assert_eq!(scanned, 359);
    assert_eq!(
        eligible,
        BTreeSet::from([
            830285, 869944, 874520, 875098, 875322, 877636, 877687, 877773, 877812, 877860, 877950,
            878011, 878056, 924257, 924320, 925254, 925674, 925719, 925796, 943111, 946112, 947228,
            949750, 970972, 1011712, 1024673, 1058366, 1059030, 1059454, 1060052, 1060199, 1061897,
            1065812, 1069813, 1070207, 1072715, 1078906, 1079482, 1080877, 1081463, 1089346,
            1090607, 1091235, 1092909, 1130833,
        ])
    );
}

/// Berzerk is a whole-clan bonus, so a strict hand needs four Berzerk characters to carry
/// it. These are canonical clan-46 cards with no ability of their own.
fn berzerk_hand() -> [CardKey; 4] {
    [
        CardKey::new(860, 1),
        CardKey::new(861, 1),
        CardKey::new(862, 1),
        CardKey::new(869, 1),
    ]
}

#[test]
fn strict_catalog_match_admits_only_the_two_reviewed_victory_opponent_life_identities() {
    let catalog = catalog();
    let registry = registry();
    let (_, opponent) = fully_supported_hands();

    // Mou level three's printed Ability is the only catalog authority for `1399`.
    let mou_hand = [
        CardKey::new(1589, 3),
        CardKey::new(123, 1),
        CardKey::new(124, 1),
        CardKey::new(138, 1),
    ];
    let prepared = CatalogCombatStatMatchV1::new(
        input(mou_hand, opponent, false),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    let CatalogCombatStatSourceDispositionV1::ExecutePostRound {
        identity, effect, ..
    } = &prepared.preparation()[PlayerId::P1][0].ability
    else {
        panic!("Mou L3 was not prepared as unconditional Victory opponent-Life")
    };
    assert_eq!(identity.catalog_id, Some(1399));
    assert_eq!(identity.registry_definition_id, 1399);
    assert_eq!(
        *effect,
        CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictory {
            life: 5,
            minimum: 5
        }
    );

    // Rakhan, Milovan and Fraser print exactly the same text under their own catalog ids,
    // which have no registry definition at all. Description equality never transfers.
    for key in [
        CardKey::new(1151, 3), // Rakhan, catalog ability 978, "-5 Opp. Life Min 5".
        CardKey::new(679, 3),  // Milovan, catalog ability 498, "-2 Opp. Life Min 2".
        CardKey::new(1477, 4), // Fraser, catalog ability 1289, "-2 Opp. Life Min 2".
    ] {
        let hand = [
            key,
            CardKey::new(123, 1),
            CardKey::new(124, 1),
            CardKey::new(138, 1),
        ];
        assert!(
            CatalogCombatStatMatchV1::new(
                input(hand, opponent, false),
                &catalog,
                &registry,
                PROJECTION,
            )
            .is_err(),
            "same-text card {key:?} must stay fail-closed",
        );
    }

    // The active Berzerk clan bonus bridges catalog bonus id 44 to registry `680`.
    let prepared = CatalogCombatStatMatchV1::new(
        input(berzerk_hand(), opponent, false),
        &catalog,
        &registry,
        PROJECTION,
    )
    .unwrap();
    let CatalogCombatStatSourceDispositionV1::ExecutePostRound {
        identity, effect, ..
    } = &prepared.preparation()[PlayerId::P1][0].bonus
    else {
        panic!("the active Berzerk bonus was not prepared")
    };
    assert_eq!(identity.catalog_id, Some(44));
    assert_eq!(identity.registry_definition_id, 680);
    assert_eq!(
        *effect,
        CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictory {
            life: 2,
            minimum: 2
        }
    );
}

#[test]
fn strict_catalog_match_admits_the_reviewed_conditional_victory_opponent_life_cards() {
    let catalog = catalog();
    let registry = registry();
    let (_, opponent) = fully_supported_hands();
    let with = |key: CardKey| {
        [
            key,
            CardKey::new(123, 1),
            CardKey::new(124, 1),
            CardKey::new(138, 1),
        ]
    };

    // Each reviewed conditional member is bound to one exact card, level and catalog id, and
    // carries the predicate its own printed text names. Diabolus prints the effect at both
    // levels under two distinct, byte-identical registry records.
    for (key, catalog_id, life, predicate) in [
        (
            CardKey::new(2058, 2),
            4708,
            4,
            CombatStatPredicateV1::SelectedHandSlotsMatch,
        ),
        (
            CardKey::new(2270, 1),
            4301,
            3,
            CombatStatPredicateV1::OwnerWonPreviousRound,
        ),
        (
            CardKey::new(2270, 2),
            3016,
            3,
            CombatStatPredicateV1::OwnerWonPreviousRound,
        ),
    ] {
        let prepared = CatalogCombatStatMatchV1::new(
            input(with(key), opponent, false),
            &catalog,
            &registry,
            PROJECTION,
        )
        .unwrap_or_else(|error| panic!("{key:?}: {error}"));
        let CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity,
            effect,
            predicate: actual_predicate,
        } = &prepared.preparation()[PlayerId::P1][0].ability
        else {
            panic!("{key:?} was not prepared as conditional Victory opponent-Life")
        };
        assert_eq!(identity.catalog_id, Some(catalog_id));
        assert_eq!(identity.registry_definition_id, catalog_id);
        assert_eq!(
            *effect,
            CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictory { life, minimum: 0 }
        );
        assert_eq!(*actual_predicate, predicate);
        assert_eq!(
            prepared.match_spec().cards[PlayerId::P1][0].ability,
            CombatStatSourcePlanV1::Execute {
                source_id: catalog_id,
                predicate,
                effect:
                    urban_recreation_rust::engine::CombatStatEffectV1::ReduceOpponentLifeOnVictory {
                        life,
                        minimum: 0,
                    },
            }
        );
    }

    // Doela Noel level one prints the same text under catalog id 4843, which has no registry
    // definition, so it stays fail-closed without any special handling. Ligea level three's
    // Courage `4533` and Bekum's Growth `1730` share the structure but have no admitted
    // evidence and no predicate a post-round plan can carry, respectively.
    for key in [
        CardKey::new(2058, 1), // Doela Noel L1, catalog ability 4843.
        CardKey::new(2556, 3), // Ligea L3, Courage: - 3 Opp. Life Min 0.
        CardKey::new(1882, 2), // Bekum, Growth: - 1 Opp. Life Min 4.
    ] {
        assert!(
            CatalogCombatStatMatchV1::new(
                input(with(key), opponent, false),
                &catalog,
                &registry,
                PROJECTION,
            )
            .is_err(),
            "{key:?} must stay fail-closed",
        );
    }
}
