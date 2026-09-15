use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use urban_recreation_rust::catalog::CardKey;
use urban_recreation_rust::engine::{
    BaseRulesError, BaseRulesMatchSpec, BaseRulesPlayerSpec, BaseRulesPosition,
    BaseRulesRoundInput, BaseRulesSelection, ByPlayer, ClanBonusDiagnostic,
    ClanBonusDiagnosticError, ClanBonusDiagnosticMatchSpecV1, DiagnosticAffectedSideV1,
    DiagnosticCardPlanV1, DiagnosticCombatEffectV1, DiagnosticCombatStatV1,
    DiagnosticEffectSourceV1, DiagnosticMagnitudeV1, DiagnosticPlanErrorV1, DiagnosticSourcePlanV1,
    DiagnosticStatOperationV1, InvalidDiagnosticPlanReasonV1, PlayerId,
};

fn card(
    id: u32,
    level: u8,
    power: u16,
    damage: u16,
) -> urban_recreation_rust::engine::BaseRulesCardSpec {
    urban_recreation_rust::engine::BaseRulesCardSpec {
        key: CardKey::new(id, level),
        clan_id: id,
        power,
        damage,
    }
}

fn base_spec(power: u16, damage: u16) -> BaseRulesMatchSpec {
    let hand = |base| std::array::from_fn(|index| card(base + index as u32, 3, power, damage));
    BaseRulesMatchSpec {
        battle_rule_id: 10,
        night: false,
        players: ByPlayer::new(
            BaseRulesPlayerSpec {
                initial_life: 20,
                initial_pillz: 20,
                hand: hand(100),
            },
            BaseRulesPlayerSpec {
                initial_life: 20,
                initial_pillz: 20,
                hand: hand(200),
            },
        ),
    }
}

fn absent_plan(key: CardKey) -> DiagnosticCardPlanV1 {
    DiagnosticCardPlanV1 {
        key,
        ability: DiagnosticSourcePlanV1::Absent,
        bonus: DiagnosticSourcePlanV1::Absent,
        source_bonus_support_count: 0,
    }
}

fn plans(base: &BaseRulesMatchSpec) -> ByPlayer<[DiagnosticCardPlanV1; 4]> {
    ByPlayer::new(
        base.players[PlayerId::P1]
            .hand
            .map(|card| absent_plan(card.key)),
        base.players[PlayerId::P2]
            .hand
            .map(|card| absent_plan(card.key)),
    )
}

fn game(
    base: BaseRulesMatchSpec,
    cards: ByPlayer<[DiagnosticCardPlanV1; 4]>,
) -> ClanBonusDiagnostic {
    ClanBonusDiagnostic::new(ClanBonusDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    })
    .unwrap()
}

fn input(first: PlayerId, p1: (u8, u16, bool), p2: (u8, u16, bool)) -> BaseRulesRoundInput {
    BaseRulesRoundInput {
        first_mover: first,
        selections: ByPlayer::new(
            BaseRulesSelection::new(p1.0, p1.1, p1.2),
            BaseRulesSelection::new(p2.0, p2.1, p2.2),
        ),
    }
}

fn modifier(
    side: DiagnosticAffectedSideV1,
    stat: DiagnosticCombatStatV1,
    operation: DiagnosticStatOperationV1,
    value: u16,
    minimum: Option<u16>,
    maximum: Option<u16>,
    multiplier: DiagnosticMagnitudeV1,
) -> DiagnosticCombatEffectV1 {
    DiagnosticCombatEffectV1::ModifyCombatStat {
        side,
        stat,
        operation,
        value,
        minimum,
        maximum,
        multiplier,
    }
}

fn execute(id: u32, effect: DiagnosticCombatEffectV1) -> DiagnosticSourcePlanV1 {
    DiagnosticSourcePlanV1::Execute {
        source_id: id,
        effect,
    }
}

fn position_hash(position: &BaseRulesPosition) -> u64 {
    let mut hasher = DefaultHasher::new();
    position.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn support_uses_immutable_whole_draw_after_cards_are_played() {
    let base = base_spec(6, 1);
    let mut cards = plans(&base);
    let support = modifier(
        DiagnosticAffectedSideV1::Player,
        DiagnosticCombatStatV1::Attack,
        DiagnosticStatOperationV1::Increase,
        3,
        None,
        None,
        DiagnosticMagnitudeV1::SourceBonusSupport,
    );
    for slot in 0..4 {
        cards[PlayerId::P1][slot].bonus = execute(266, support);
        cards[PlayerId::P1][slot].source_bonus_support_count = 4;
    }
    let mut game = game(base, cards);
    let (first, _) = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    let (second, _) = game
        .make(input(PlayerId::P1, (1, 0, false), (1, 0, false)))
        .unwrap();
    assert_eq!(first.cards[PlayerId::P1].attack, 18);
    assert_eq!(second.cards[PlayerId::P1].attack, 18);
}

#[test]
fn self_power_and_attack_resolve_before_opponent_reductions() {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].bonus = execute(
        43,
        modifier(
            DiagnosticAffectedSideV1::Player,
            DiagnosticCombatStatV1::Power,
            DiagnosticStatOperationV1::Increase,
            2,
            None,
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    cards[PlayerId::P2][0].bonus = execute(
        6,
        modifier(
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticCombatStatV1::Attack,
            DiagnosticStatOperationV1::Decrease,
            12,
            Some(8),
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let expected_spec = ClanBonusDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    };
    let mut game = ClanBonusDiagnostic::new(expected_spec.clone()).unwrap();
    assert_eq!(game.match_spec(), &expected_spec);
    assert_eq!(game.base_rules_spec(), &expected_spec.base_rules);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 6, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 8);
    assert_eq!(report.cards[PlayerId::P1].attack, 44);
}

#[test]
fn damage_reduction_precedes_fury_and_power_and_damage_splits() {
    let base = base_spec(8, 0);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].bonus = execute(
        38,
        modifier(
            DiagnosticAffectedSideV1::Player,
            DiagnosticCombatStatV1::Damage,
            DiagnosticStatOperationV1::Increase,
            2,
            None,
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    cards[PlayerId::P2][0].bonus = execute(
        1536,
        modifier(
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticCombatStatV1::PowerAndDamage,
            DiagnosticStatOperationV1::Decrease,
            1,
            Some(1),
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let mut game = game(base, cards);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 1, true), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 7);
    assert_eq!(report.cards[PlayerId::P1].damage, 3);
}

#[test]
fn stop_and_cancellation_controls_preserve_projected_arithmetic() {
    let base = base_spec(6, 2);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].bonus = execute(130, DiagnosticCombatEffectV1::StopOpponentBonus);
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    cards[PlayerId::P2][0].bonus = execute(333, DiagnosticCombatEffectV1::StopOpponentBonus);
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    // With pure mutual Stop Bonus there is no combat-stat observation to distinguish which
    // control fired. This only pins the stable no-order/no-panic control-cycle invariant.
    let mut mutual = game(base.clone(), cards);
    let (first, _) = mutual
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(first.cards[PlayerId::P1].attack, 6);
    assert_eq!(first.cards[PlayerId::P2].attack, 6);

    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(2299, DiagnosticCombatEffectV1::StopOpponentBonus);
    cards[PlayerId::P2][0].bonus = execute(
        43,
        modifier(
            DiagnosticAffectedSideV1::Player,
            DiagnosticCombatStatV1::Power,
            DiagnosticStatOperationV1::Increase,
            2,
            None,
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let mut stopped = game(base.clone(), cards);
    let (report, _) = stopped
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].power, 6);

    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        1164,
        DiagnosticCombatEffectV1::CancelOpponentCombatStatModifiers {
            stat: DiagnosticCombatStatV1::PowerAndDamage,
        },
    );
    cards[PlayerId::P2][0].bonus = execute(
        4618,
        modifier(
            DiagnosticAffectedSideV1::Player,
            DiagnosticCombatStatV1::PowerAndDamage,
            DiagnosticStatOperationV1::Increase,
            2,
            None,
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let mut cancelled = game(base, cards);
    let (report, _) = cancelled
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].power, 6);
    assert_eq!(report.cards[PlayerId::P2].damage, 2);
}

#[test]
fn cancellation_is_component_wise_and_never_removes_base_stats_or_fury() {
    let base = base_spec(6, 2);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        1164,
        DiagnosticCombatEffectV1::CancelOpponentCombatStatModifiers {
            stat: DiagnosticCombatStatV1::Power,
        },
    );
    cards[PlayerId::P2][0].bonus = execute(
        4618,
        modifier(
            DiagnosticAffectedSideV1::Player,
            DiagnosticCombatStatV1::PowerAndDamage,
            DiagnosticStatOperationV1::Increase,
            2,
            None,
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let mut component = game(base.clone(), cards);
    let (report, _) = component
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].power, 6);
    assert_eq!(report.cards[PlayerId::P2].damage, 4);

    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        7,
        DiagnosticCombatEffectV1::CancelOpponentCombatStatModifiers {
            stat: DiagnosticCombatStatV1::Damage,
        },
    );
    cards[PlayerId::P2][0].bonus = execute(
        38,
        modifier(
            DiagnosticAffectedSideV1::Player,
            DiagnosticCombatStatV1::Damage,
            DiagnosticStatOperationV1::Increase,
            2,
            None,
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let mut fury = game(base, cards);
    let (report, _) = fury
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, true)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].damage, 4);
}

#[test]
fn directional_minimum_and_maximum_never_move_a_value_the_wrong_way() {
    let base = base_spec(3, 1);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].bonus = execute(
        1,
        modifier(
            DiagnosticAffectedSideV1::Player,
            DiagnosticCombatStatV1::Power,
            DiagnosticStatOperationV1::Increase,
            2,
            None,
            Some(2),
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    cards[PlayerId::P2][0].bonus = execute(
        2,
        modifier(
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticCombatStatV1::Attack,
            DiagnosticStatOperationV1::Decrease,
            8,
            Some(8),
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let mut bounded = game(base.clone(), cards);
    let (report, _) = bounded
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 3);
    assert_eq!(report.cards[PlayerId::P1].attack, 3);

    let mut cards = plans(&base);
    cards[PlayerId::P2][0].bonus = execute(
        3,
        modifier(
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticCombatStatV1::Attack,
            DiagnosticStatOperationV1::Decrease,
            8,
            Some(0),
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let mut zero = game(base, cards);
    let (report, _) = zero
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].attack, 0);
}

#[test]
fn unsupported_control_is_lazy_atomic_and_p2_validation_wins_first() {
    let base = base_spec(6, 1);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = DiagnosticSourcePlanV1::RejectIfSelected { source_id: 999 };
    let mut game = game(base, cards);
    let before = game.position().clone();
    let before_hash = position_hash(&before);

    let error = game
        .make(input(PlayerId::P1, (0, 0, false), (4, 0, false)))
        .unwrap_err();
    assert!(matches!(
        error,
        ClanBonusDiagnosticError::BaseRules(BaseRulesError::InvalidHandSlot {
            player: PlayerId::P2,
            ..
        })
    ));
    assert_eq!(game.position(), &before);

    let error = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap_err();
    assert!(matches!(
        error,
        ClanBonusDiagnosticError::UnsupportedSelectedControl {
            player: PlayerId::P1,
            source: DiagnosticEffectSourceV1::Ability,
            source_id: 999,
            ..
        }
    ));
    assert_eq!(game.position(), &before);
    assert_eq!(position_hash(game.position()), before_hash);

    game.make(input(PlayerId::P1, (1, 0, false), (0, 0, false)))
        .unwrap();
}

#[test]
fn diagnostic_arithmetic_overflow_is_atomic() {
    let base = base_spec(u16::MAX, 1);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].bonus = execute(
        43,
        modifier(
            DiagnosticAffectedSideV1::Player,
            DiagnosticCombatStatV1::Power,
            DiagnosticStatOperationV1::Increase,
            1,
            None,
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    let mut game = game(base, cards);
    let before = game.position().clone();
    let before_hash = position_hash(&before);
    assert!(matches!(
        game.make(input(PlayerId::P1, (0, 0, false), (0, 0, false))),
        Err(ClanBonusDiagnosticError::ArithmeticOverflow {
            player: PlayerId::P1,
            ..
        })
    ));
    assert_eq!(game.position(), &before);
    assert_eq!(position_hash(game.position()), before_hash);
}

#[test]
fn invalid_execute_plans_are_rejected_instead_of_becoming_noops() {
    let base = base_spec(6, 1);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        7,
        modifier(
            DiagnosticAffectedSideV1::Player,
            DiagnosticCombatStatV1::Power,
            DiagnosticStatOperationV1::Increase,
            2,
            None,
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    let error = ClanBonusDiagnostic::new(ClanBonusDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        DiagnosticPlanErrorV1::InvalidExecute {
            reason: InvalidDiagnosticPlanReasonV1::AbilityCombatModifier,
            ..
        }
    ));
}

#[test]
fn source_bonus_context_counts_distinct_character_ids_exactly() {
    let base = base_spec(6, 1);

    let mut too_large = plans(&base);
    too_large[PlayerId::P1][0].bonus = execute(
        266,
        modifier(
            DiagnosticAffectedSideV1::Player,
            DiagnosticCombatStatV1::Attack,
            DiagnosticStatOperationV1::Increase,
            3,
            None,
            None,
            DiagnosticMagnitudeV1::SourceBonusSupport,
        ),
    );
    too_large[PlayerId::P1][0].source_bonus_support_count = 5;
    let error = ClanBonusDiagnostic::new(ClanBonusDiagnosticMatchSpecV1 {
        base_rules: base.clone(),
        cards: too_large,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        DiagnosticPlanErrorV1::InvalidSourceBonusContext {
            player: PlayerId::P1,
            source_id: Some(266),
            expected: 1,
            actual: 5,
            ..
        }
    ));

    let mut inconsistent = plans(&base);
    for slot in 0..4 {
        inconsistent[PlayerId::P1][slot].bonus = DiagnosticSourcePlanV1::Disabled { source_id: 42 };
        inconsistent[PlayerId::P1][slot].source_bonus_support_count = 4;
    }
    inconsistent[PlayerId::P1][1].source_bonus_support_count = 3;
    let error = ClanBonusDiagnostic::new(ClanBonusDiagnosticMatchSpecV1 {
        base_rules: base.clone(),
        cards: inconsistent,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        DiagnosticPlanErrorV1::InvalidSourceBonusContext {
            player: PlayerId::P1,
            source_id: Some(42),
            expected: 4,
            actual: 3,
            ..
        }
    ));

    let mut absent_nonzero = plans(&base);
    absent_nonzero[PlayerId::P1][0].source_bonus_support_count = 1;
    let error = ClanBonusDiagnostic::new(ClanBonusDiagnosticMatchSpecV1 {
        base_rules: base.clone(),
        cards: absent_nonzero,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        DiagnosticPlanErrorV1::InvalidSourceBonusContext {
            player: PlayerId::P1,
            source_id: None,
            expected: 0,
            actual: 1,
            ..
        }
    ));

    let mut duplicate_base = base;
    duplicate_base.players[PlayerId::P1].hand[1].key = CardKey::new(
        duplicate_base.players[PlayerId::P1].hand[0].key.id,
        duplicate_base.players[PlayerId::P1].hand[0].key.level + 1,
    );
    let mut duplicate_ids = plans(&duplicate_base);
    for slot in 0..3 {
        duplicate_ids[PlayerId::P1][slot].bonus =
            DiagnosticSourcePlanV1::Disabled { source_id: 42 };
        duplicate_ids[PlayerId::P1][slot].source_bonus_support_count = 2;
    }
    ClanBonusDiagnostic::new(ClanBonusDiagnosticMatchSpecV1 {
        base_rules: duplicate_base.clone(),
        cards: duplicate_ids.clone(),
    })
    .unwrap();
    duplicate_ids[PlayerId::P1][0].source_bonus_support_count = 3;
    let error = ClanBonusDiagnostic::new(ClanBonusDiagnosticMatchSpecV1 {
        base_rules: duplicate_base,
        cards: duplicate_ids,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        DiagnosticPlanErrorV1::InvalidSourceBonusContext {
            expected: 2,
            actual: 3,
            ..
        }
    ));
}

#[test]
fn opponent_origin_cancellation_preserves_the_cancellers_own_reduction() {
    // Capture 1089974 round index 2 is the server observation: Dookor's cancellation
    // suppresses Sue's -1 opponent Power and Damage, while Dookor's own opponent Power
    // reduction still takes Sue from 6 to 4. Numeric abilities remain disabled by replay
    // preparation, so this focused resolver test places both numeric effects in the
    // diagnostic's executable bonus slots while preserving their opposing origins.
    let mut base = base_spec(6, 4);
    base.players[PlayerId::P2].hand[0].damage = 3;
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        1589,
        DiagnosticCombatEffectV1::CancelOpponentCombatStatModifiers {
            stat: DiagnosticCombatStatV1::PowerAndDamage,
        },
    );
    cards[PlayerId::P1][0].bonus = execute(
        1578,
        modifier(
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticCombatStatV1::Power,
            DiagnosticStatOperationV1::Decrease,
            3,
            Some(4),
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    cards[PlayerId::P2][0].bonus = execute(
        916,
        modifier(
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticCombatStatV1::PowerAndDamage,
            DiagnosticStatOperationV1::Decrease,
            1,
            Some(3),
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;

    let mut game = game(base, cards);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 6);
    assert_eq!(report.cards[PlayerId::P1].damage, 4);
    assert_eq!(report.cards[PlayerId::P2].power, 4);
    assert_eq!(report.cards[PlayerId::P2].damage, 3);
}

#[test]
fn mirrored_card_identity_and_slot_keep_player_specific_plans() {
    let mut base = base_spec(6, 2);
    base.players[PlayerId::P2].hand[0].key = base.players[PlayerId::P1].hand[0].key;
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].bonus = execute(
        43,
        modifier(
            DiagnosticAffectedSideV1::Player,
            DiagnosticCombatStatV1::Power,
            DiagnosticStatOperationV1::Increase,
            2,
            None,
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    cards[PlayerId::P2][0].bonus = execute(
        37,
        modifier(
            DiagnosticAffectedSideV1::Player,
            DiagnosticCombatStatV1::Attack,
            DiagnosticStatOperationV1::Increase,
            8,
            None,
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;

    let mut game = game(base, cards);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 8);
    assert_eq!(report.cards[PlayerId::P1].attack, 8);
    assert_eq!(report.cards[PlayerId::P2].power, 6);
    assert_eq!(report.cards[PlayerId::P2].attack, 14);
}

#[test]
fn make_unmake_isolates_siblings_and_simultaneous_games() {
    let base = base_spec(7, 2);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].bonus = execute(
        37,
        modifier(
            DiagnosticAffectedSideV1::Player,
            DiagnosticCombatStatV1::Attack,
            DiagnosticStatOperationV1::Increase,
            8,
            None,
            None,
            DiagnosticMagnitudeV1::Fixed,
        ),
    );
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    let mut first = game(base.clone(), cards.clone());
    let second = game(base, cards);
    let initial = first.position().clone();
    let (_, undo) = first
        .make(input(PlayerId::P1, (0, 1, false), (0, 0, false)))
        .unwrap();
    assert_eq!(second.position(), &initial);
    first.unmake(undo);
    assert_eq!(first.position(), &initial);

    let (_, sibling) = first
        .make(input(PlayerId::P2, (1, 0, false), (1, 1, false)))
        .unwrap();
    first.unmake(sibling);
    assert_eq!(first.position(), &initial);
}
