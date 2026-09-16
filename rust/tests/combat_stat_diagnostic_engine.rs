use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use urban_recreation_rust::catalog::CardKey;
use urban_recreation_rust::engine::{
    BaseRulesError, BaseRulesMatchSpec, BaseRulesPlayerSpec, BaseRulesPosition,
    BaseRulesRoundInput, BaseRulesSelection, ByPlayer, CombatStatAffectedSideV1,
    CombatStatAttributeV1, CombatStatCardPlanV1, CombatStatDiagnosticErrorV1,
    CombatStatDiagnosticMatchSpecV1, CombatStatDiagnosticV1, CombatStatEffectSourceV1,
    CombatStatEffectV1, CombatStatMagnitudeV1, CombatStatOperationV1, CombatStatPlanErrorV1,
    CombatStatPredicateV1, CombatStatSourcePlanV1, InvalidCombatStatPlanReasonV1, PlayerId,
};

fn card(id: u32, power: u16, damage: u16) -> urban_recreation_rust::engine::BaseRulesCardSpec {
    urban_recreation_rust::engine::BaseRulesCardSpec {
        key: CardKey::new(id, 3),
        clan_id: id,
        power,
        damage,
    }
}

fn base_spec(power: u16, damage: u16) -> BaseRulesMatchSpec {
    let hand = |base| std::array::from_fn(|index| card(base + index as u32, power, damage));
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

fn absent(key: CardKey) -> CombatStatCardPlanV1 {
    CombatStatCardPlanV1 {
        key,
        ability: CombatStatSourcePlanV1::Absent,
        bonus: CombatStatSourcePlanV1::Absent,
        source_bonus_support_count: 0,
    }
}

fn plans(base: &BaseRulesMatchSpec) -> ByPlayer<[CombatStatCardPlanV1; 4]> {
    ByPlayer::new(
        base.players[PlayerId::P1].hand.map(|card| absent(card.key)),
        base.players[PlayerId::P2].hand.map(|card| absent(card.key)),
    )
}

fn game(
    base: BaseRulesMatchSpec,
    cards: ByPlayer<[CombatStatCardPlanV1; 4]>,
) -> CombatStatDiagnosticV1 {
    CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
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
    side: CombatStatAffectedSideV1,
    stat: CombatStatAttributeV1,
    operation: CombatStatOperationV1,
    value: u16,
    minimum: Option<u16>,
    maximum: Option<u16>,
    multiplier: CombatStatMagnitudeV1,
) -> CombatStatEffectV1 {
    CombatStatEffectV1::ModifyCombatStat {
        side,
        stat,
        operation,
        value,
        minimum,
        maximum,
        multiplier,
    }
}

fn execute(
    id: u32,
    predicate: CombatStatPredicateV1,
    effect: CombatStatEffectV1,
) -> CombatStatSourcePlanV1 {
    CombatStatSourcePlanV1::Execute {
        source_id: id,
        predicate,
        effect,
    }
}

fn own(stat: CombatStatAttributeV1, value: u16) -> CombatStatEffectV1 {
    modifier(
        CombatStatAffectedSideV1::Player,
        stat,
        CombatStatOperationV1::Increase,
        value,
        None,
        None,
        CombatStatMagnitudeV1::Fixed,
    )
}

fn reduction(stat: CombatStatAttributeV1, value: u16, minimum: u16) -> CombatStatEffectV1 {
    modifier(
        CombatStatAffectedSideV1::Opponent,
        stat,
        CombatStatOperationV1::Decrease,
        value,
        Some(minimum),
        None,
        CombatStatMagnitudeV1::Fixed,
    )
}

fn position_hash(position: &BaseRulesPosition) -> u64 {
    let mut hasher = DefaultHasher::new();
    position.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn courage_and_reprisal_use_explicit_owner_relative_first_mover() {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    for slot in 0..2 {
        cards[PlayerId::P1][slot].ability = execute(
            1850,
            CombatStatPredicateV1::OwnerMovesFirst,
            own(CombatStatAttributeV1::Power, 2),
        );
        cards[PlayerId::P2][slot].ability = execute(
            4216,
            CombatStatPredicateV1::OwnerMovesSecond,
            own(CombatStatAttributeV1::Damage, 1),
        );
    }
    let mut active = game(base.clone(), cards.clone());
    for slot in 0..2 {
        let (report, _) = active
            .make(input(PlayerId::P1, (slot, 0, false), (slot, 0, false)))
            .unwrap();
        assert_eq!(report.cards[PlayerId::P1].power, 8);
        assert_eq!(report.cards[PlayerId::P2].damage, 4);
    }

    let mut inactive = game(base, cards);
    let (report, _) = inactive
        .make(input(PlayerId::P2, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 6);
    assert_eq!(report.cards[PlayerId::P2].damage, 3);
}

#[test]
fn opponent_reductions_are_stably_sorted_by_descending_minimum() {
    let base = base_spec(6, 3);
    let mut power_cards = plans(&base);
    // Capture 1011768: ability Min4 precedes bonus Min1, taking 6 -> 4 -> 2.
    power_cards[PlayerId::P2][0].bonus = execute(
        156,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::Power, 2, 1),
    );
    power_cards[PlayerId::P2][0].source_bonus_support_count = 1;
    power_cards[PlayerId::P2][0].ability = execute(
        4718,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::Power, 3, 4),
    );
    let mut power_game = game(base.clone(), power_cards);
    let (report, _) = power_game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 2);

    // Don Cr captures: bonus Min8 precedes ability Min2, taking attack 18 -> 8 -> 4.
    let mut attack_cards = plans(&base);
    attack_cards[PlayerId::P2][0].bonus = execute(
        6,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::Attack, 12, 8),
    );
    attack_cards[PlayerId::P2][0].source_bonus_support_count = 1;
    attack_cards[PlayerId::P2][0].ability = execute(
        999,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::Attack, 4, 2),
    );
    let mut attack_game = game(base.clone(), attack_cards);
    let (report, _) = attack_game
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].attack, 4);

    // Miss Stella capture 901292: ability Min11 precedes bonus Min3, 18 -> 11 -> 3.
    let mut miss_stella = plans(&base);
    miss_stella[PlayerId::P2][0].bonus = execute(
        40,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::Attack, 8, 3),
    );
    miss_stella[PlayerId::P2][0].source_bonus_support_count = 1;
    miss_stella[PlayerId::P2][0].ability = execute(
        1000,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::Attack, 8, 11),
    );
    let mut miss_stella_game = game(base, miss_stella);
    let (report, _) = miss_stella_game
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].attack, 3);

    // Excluded capture 1011643 independently shows a below-Min base Power of 1 stays 1.
    let below_min_base = base_spec(1, 3);
    let mut below_min_cards = plans(&below_min_base);
    below_min_cards[PlayerId::P2][0].ability = execute(
        1001,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::Power, 2, 2),
    );
    let mut below_min = game(below_min_base, below_min_cards);
    let (report, _) = below_min
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 1);
}

#[test]
fn cancellation_suppresses_opponent_sources_but_not_base_stats_or_fury() {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        7,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::CancelOpponentCombatStatModifiers {
            stat: CombatStatAttributeV1::Damage,
        },
    );
    cards[PlayerId::P2][0].ability = execute(
        56,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::Damage, 4, 1),
    );
    let mut fury_game = game(base, cards);
    let (report, _) = fury_game
        .make(input(PlayerId::P1, (0, 0, true), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].damage, 5);

    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        8,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::CancelOpponentCombatStatModifiers {
            stat: CombatStatAttributeV1::PowerAndDamage,
        },
    );
    cards[PlayerId::P2][0].ability = execute(
        9,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Power, 2),
    );
    cards[PlayerId::P2][0].bonus = execute(
        10,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Damage, 2),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let mut both_sources = game(base, cards);
    let (report, _) = both_sources
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].power, 6);
    assert_eq!(report.cards[PlayerId::P2].damage, 3);
}

#[test]
fn stop_bonus_suppresses_existing_support_bonus() {
    let base = base_spec(6, 2);
    let mut cards = plans(&base);
    for slot in 0..4 {
        cards[PlayerId::P2][slot].bonus = execute(
            266,
            CombatStatPredicateV1::Always,
            modifier(
                CombatStatAffectedSideV1::Player,
                CombatStatAttributeV1::Attack,
                CombatStatOperationV1::Increase,
                3,
                None,
                None,
                CombatStatMagnitudeV1::SourceBonusSupport,
            ),
        );
        cards[PlayerId::P2][slot].source_bonus_support_count = 4;
    }
    cards[PlayerId::P1][0].ability = execute(
        2299,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentBonus,
    );
    let mut support_game = game(base, cards);
    let (report, _) = support_game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].attack, 6);

    let base = base_spec(6, 2);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        2299,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentBonus,
    );
    cards[PlayerId::P2][0].ability = execute(
        11,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Power, 2),
    );
    cards[PlayerId::P2][0].bonus = execute(
        12,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Damage, 2),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let mut preserved_ability = game(base, cards);
    let (report, _) = preserved_ability
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].power, 8);
    assert_eq!(report.cards[PlayerId::P2].damage, 2);
}

#[test]
fn impossible_execute_plans_fail_at_construction() {
    let base = base_spec(6, 2);
    let cases = [
        (
            execute(
                1,
                CombatStatPredicateV1::Always,
                modifier(
                    CombatStatAffectedSideV1::Player,
                    CombatStatAttributeV1::Attack,
                    CombatStatOperationV1::Increase,
                    2,
                    None,
                    None,
                    CombatStatMagnitudeV1::SourceBonusSupport,
                ),
            ),
            InvalidCombatStatPlanReasonV1::SupportAbility,
        ),
        (
            execute(
                2,
                CombatStatPredicateV1::Always,
                modifier(
                    CombatStatAffectedSideV1::Player,
                    CombatStatAttributeV1::Power,
                    CombatStatOperationV1::Increase,
                    6,
                    None,
                    Some(8),
                    CombatStatMagnitudeV1::Fixed,
                ),
            ),
            InvalidCombatStatPlanReasonV1::CappedIncrease,
        ),
        (
            execute(
                3,
                CombatStatPredicateV1::OwnerMovesFirst,
                CombatStatEffectV1::StopOpponentBonus,
            ),
            InvalidCombatStatPlanReasonV1::ConditionalControl,
        ),
    ];
    for (plan, reason) in cases {
        let mut cards = plans(&base);
        cards[PlayerId::P1][0].ability = plan;
        let error = CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
            base_rules: base.clone(),
            cards,
        })
        .unwrap_err();
        assert!(matches!(
            error,
            CombatStatPlanErrorV1::InvalidExecute {
                source: CombatStatEffectSourceV1::Ability,
                reason: actual,
                ..
            } if actual == reason
        ));
    }

    let mut capped_bonus = plans(&base);
    capped_bonus[PlayerId::P1][0].bonus = execute(
        4,
        CombatStatPredicateV1::Always,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Power,
            CombatStatOperationV1::Increase,
            2,
            None,
            Some(8),
            CombatStatMagnitudeV1::Fixed,
        ),
    );
    capped_bonus[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
            base_rules: base.clone(),
            cards: capped_bonus,
        }),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            source: CombatStatEffectSourceV1::Bonus,
            reason: InvalidCombatStatPlanReasonV1::CappedIncrease,
            ..
        })
    ));

    let mut conditional_bonus = plans(&base);
    conditional_bonus[PlayerId::P1][0].bonus = execute(
        5,
        CombatStatPredicateV1::OwnerMovesFirst,
        own(CombatStatAttributeV1::Power, 2),
    );
    conditional_bonus[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
            base_rules: base.clone(),
            cards: conditional_bonus,
        }),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::ConditionalBonus,
            ..
        })
    ));

    let mut invalid_context = plans(&base);
    invalid_context[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
            base_rules: base,
            cards: invalid_context,
        }),
        Err(CombatStatPlanErrorV1::InvalidSourceBonusContext { .. })
    ));
}

#[test]
fn selected_hazards_validation_and_overflow_are_atomic() {
    let base = base_spec(u16::MAX, 2);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::RejectIfSelected { source_id: 999 };
    cards[PlayerId::P1][1].ability = execute(
        1,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Power, 1),
    );
    let mut atomic_game = game(base, cards);
    let before = atomic_game.position().clone();
    let hash = position_hash(&before);

    assert!(matches!(
        atomic_game.make(input(PlayerId::P1, (0, 0, false), (4, 0, false))),
        Err(CombatStatDiagnosticErrorV1::BaseRules(
            BaseRulesError::InvalidHandSlot {
                player: PlayerId::P2,
                ..
            }
        ))
    ));
    assert!(matches!(
        atomic_game.make(input(PlayerId::P1, (0, 0, false), (0, 0, false))),
        Err(CombatStatDiagnosticErrorV1::UnsupportedSelectedHazard {
            player: PlayerId::P1,
            source_id: 999,
            ..
        })
    ));
    assert!(matches!(
        atomic_game.make(input(PlayerId::P1, (1, 0, false), (0, 0, false))),
        Err(CombatStatDiagnosticErrorV1::ArithmeticOverflow {
            player: PlayerId::P1,
            ..
        })
    ));
    assert_eq!(atomic_game.position(), &before);
    assert_eq!(position_hash(atomic_game.position()), hash);

    let base = base_spec(6, 2);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        2,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Power, 1),
    );
    cards[PlayerId::P2][0].ability = execute(
        3,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Power, u16::MAX),
    );
    let mut second_source = game(base, cards);
    let before = second_source.position().clone();
    let hash = position_hash(&before);
    assert!(matches!(
        second_source.make(input(PlayerId::P1, (0, 0, false), (0, 0, false))),
        Err(CombatStatDiagnosticErrorV1::ArithmeticOverflow {
            player: PlayerId::P2,
            ..
        })
    ));
    assert_eq!(second_source.position(), &before);
    assert_eq!(position_hash(second_source.position()), hash);
}

#[test]
fn mirrored_card_keys_keep_player_specific_predicates_and_plans() {
    let mut base = base_spec(6, 2);
    base.players[PlayerId::P2].hand[0].key = base.players[PlayerId::P1].hand[0].key;
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        1,
        CombatStatPredicateV1::OwnerMovesFirst,
        own(CombatStatAttributeV1::Power, 2),
    );
    cards[PlayerId::P2][0].ability = execute(
        2,
        CombatStatPredicateV1::OwnerMovesSecond,
        own(CombatStatAttributeV1::Damage, 1),
    );
    let mut game = game(base, cards);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 8);
    assert_eq!(report.cards[PlayerId::P1].damage, 2);
    assert_eq!(report.cards[PlayerId::P2].power, 6);
    assert_eq!(report.cards[PlayerId::P2].damage, 3);
}

#[test]
fn make_unmake_isolates_siblings_and_simultaneous_games() {
    let base = base_spec(7, 2);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        37,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Attack, 8),
    );
    let expected = CombatStatDiagnosticMatchSpecV1 {
        base_rules: base.clone(),
        cards: cards.clone(),
    };
    let mut first = CombatStatDiagnosticV1::new(expected.clone()).unwrap();
    let second = game(base, cards);
    assert_eq!(first.match_spec(), &expected);
    let initial = first.position().clone();
    let (_, undo) = first
        .make(input(PlayerId::P1, (0, 1, false), (0, 0, false)))
        .unwrap();
    assert_eq!(second.position(), &initial);
    let after_first = first.position().clone();
    let (_, nested) = first
        .make(input(PlayerId::P1, (1, 0, false), (1, 0, false)))
        .unwrap();
    first.unmake(nested);
    assert_eq!(first.position(), &after_first);
    first.unmake(undo);
    assert_eq!(first.position(), &initial);

    let (_, sibling) = first
        .make(input(PlayerId::P2, (1, 0, false), (1, 1, false)))
        .unwrap();
    first.unmake(sibling);
    assert_eq!(first.position(), &initial);
}
