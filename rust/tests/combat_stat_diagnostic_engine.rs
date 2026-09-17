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

fn absent(card: urban_recreation_rust::engine::BaseRulesCardSpec) -> CombatStatCardPlanV1 {
    CombatStatCardPlanV1 {
        key: card.key,
        effective_clan_id: card.clan_id,
        ability: CombatStatSourcePlanV1::Absent,
        bonus: CombatStatSourcePlanV1::Absent,
        source_bonus_support_count: 0,
        source_ability_support_count: 0,
    }
}

fn plans(base: &BaseRulesMatchSpec) -> ByPlayer<[CombatStatCardPlanV1; 4]> {
    ByPlayer::new(
        base.players[PlayerId::P1].hand.map(absent),
        base.players[PlayerId::P2].hand.map(absent),
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
fn confidence_and_revenge_are_owner_relative_and_restore_through_undo() {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    for slot in 0..2 {
        cards[PlayerId::P1][slot].ability = execute(
            560,
            CombatStatPredicateV1::OwnerWonPreviousRound,
            own(CombatStatAttributeV1::Power, 2),
        );
        cards[PlayerId::P2][slot].ability = execute(
            463,
            CombatStatPredicateV1::OwnerLostPreviousRound,
            own(CombatStatAttributeV1::Damage, 2),
        );
    }
    let mut game = game(base, cards);
    let before = game.position().clone();
    let before_hash = position_hash(&before);

    // Neither predicate has a predecessor in round zero, even though P1 moves first.
    let (first, undo_first) = game
        .make(input(PlayerId::P1, (0, 1, false), (0, 0, false)))
        .unwrap();
    assert_eq!(first.cards[PlayerId::P1].power, 6);
    assert_eq!(first.cards[PlayerId::P2].damage, 3);
    assert_eq!(game.position().previous_round_winner, Some(PlayerId::P1));
    let after_first = game.position().clone();
    let after_first_hash = position_hash(&after_first);

    // First mover is deliberately P2 now: prior outcome is owner-relative, not turn-relative.
    let (second, undo_second) = game
        .make(input(PlayerId::P2, (1, 0, false), (1, 0, false)))
        .unwrap();
    assert_eq!(second.cards[PlayerId::P1].power, 8);
    assert_eq!(second.cards[PlayerId::P2].damage, 5);
    assert_eq!(game.position().previous_round_winner, Some(PlayerId::P1));

    game.unmake(undo_second);
    assert_eq!(game.position(), &after_first);
    assert_eq!(position_hash(game.position()), after_first_hash);
    game.unmake(undo_first);
    assert_eq!(game.position(), &before);
    assert_eq!(position_hash(game.position()), before_hash);
}

#[test]
fn confidence_and_revenge_follow_the_other_owner_after_a_loss() {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    for slot in 0..2 {
        cards[PlayerId::P1][slot].ability = execute(
            463,
            CombatStatPredicateV1::OwnerLostPreviousRound,
            own(CombatStatAttributeV1::Damage, 2),
        );
        cards[PlayerId::P2][slot].ability = execute(
            560,
            CombatStatPredicateV1::OwnerWonPreviousRound,
            own(CombatStatAttributeV1::Power, 2),
        );
    }
    let mut game = game(base, cards);
    let (_, _) = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 1, false)))
        .unwrap();
    assert_eq!(game.position().previous_round_winner, Some(PlayerId::P2));

    let (report, _) = game
        .make(input(PlayerId::P1, (1, 0, false), (1, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].damage, 5);
    assert_eq!(report.cards[PlayerId::P2].power, 8);
}

#[test]
fn previous_round_fixed_bonus_is_inactive_then_active_and_stop_bonus_can_suppress_it() {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    for slot in 0..2 {
        cards[PlayerId::P1][slot].bonus = execute(
            801,
            CombatStatPredicateV1::OwnerLostPreviousRound,
            own(CombatStatAttributeV1::PowerAndDamage, 2),
        );
        // Every card has a distinct effective clan in this synthetic draw.
        cards[PlayerId::P1][slot].source_bonus_support_count = 1;
    }

    let mut active = game(base.clone(), cards.clone());
    let (first, _) = active
        .make(input(PlayerId::P1, (0, 0, false), (0, 1, false)))
        .unwrap();
    assert_eq!(first.cards[PlayerId::P1].power, 6);
    assert_eq!(first.cards[PlayerId::P1].damage, 3);
    assert_eq!(active.position().previous_round_winner, Some(PlayerId::P2));
    let (second, _) = active
        .make(input(PlayerId::P1, (1, 0, false), (1, 0, false)))
        .unwrap();
    assert_eq!(second.cards[PlayerId::P1].power, 8);
    assert_eq!(second.cards[PlayerId::P1].damage, 5);

    let mut stopped_cards = cards;
    stopped_cards[PlayerId::P2][1].ability = execute(
        40,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentBonus,
    );
    let mut stopped = game(base, stopped_cards);
    stopped
        .make(input(PlayerId::P1, (0, 0, false), (0, 1, false)))
        .unwrap();
    let (report, _) = stopped
        .make(input(PlayerId::P1, (1, 0, false), (1, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 6);
    assert_eq!(report.cards[PlayerId::P1].damage, 3);
}

#[test]
fn symmetry_and_asymmetry_compare_immutable_hand_slots_for_both_players() {
    for p1_slot in 0..4 {
        for p2_slot in 0..4 {
            for first_mover in PlayerId::ALL {
                let mut base = base_spec(6, 3);
                if p1_slot != p2_slot {
                    // Equal card identity on unequal slots must not turn Asymmetry off.
                    base.players[PlayerId::P2].hand[p2_slot as usize].key =
                        base.players[PlayerId::P1].hand[p1_slot as usize].key;
                }
                let mut cards = plans(&base);
                cards[PlayerId::P1][p1_slot as usize].ability = execute(
                    1,
                    CombatStatPredicateV1::SelectedHandSlotsMatch,
                    own(CombatStatAttributeV1::Power, 2),
                );
                cards[PlayerId::P1][p1_slot as usize].bonus = execute(
                    2,
                    CombatStatPredicateV1::SelectedHandSlotsDiffer,
                    own(CombatStatAttributeV1::Damage, 3),
                );
                cards[PlayerId::P1][p1_slot as usize].source_bonus_support_count = 1;
                cards[PlayerId::P2][p2_slot as usize].ability = execute(
                    3,
                    CombatStatPredicateV1::SelectedHandSlotsDiffer,
                    own(CombatStatAttributeV1::Power, 4),
                );
                cards[PlayerId::P2][p2_slot as usize].bonus = execute(
                    4,
                    CombatStatPredicateV1::SelectedHandSlotsMatch,
                    own(CombatStatAttributeV1::Damage, 5),
                );
                cards[PlayerId::P2][p2_slot as usize].source_bonus_support_count = 1;

                let mut game = game(base, cards);
                let initial = game.position().clone();
                let (report, undo) = game
                    .make(input(first_mover, (p1_slot, 0, false), (p2_slot, 0, false)))
                    .unwrap();
                if p1_slot == p2_slot {
                    assert_eq!(report.cards[PlayerId::P1].power, 8);
                    assert_eq!(report.cards[PlayerId::P1].damage, 3);
                    assert_eq!(report.cards[PlayerId::P2].power, 6);
                    assert_eq!(report.cards[PlayerId::P2].damage, 8);
                } else {
                    assert_eq!(report.cards[PlayerId::P1].power, 6);
                    assert_eq!(report.cards[PlayerId::P1].damage, 6);
                    assert_eq!(report.cards[PlayerId::P2].power, 10);
                    assert_eq!(report.cards[PlayerId::P2].damage, 3);
                }
                game.unmake(undo);
                assert_eq!(game.position(), &initial);
            }
        }
    }

    // Played cards do not compact the remaining hand: round two still compares the
    // original nonzero slots.
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][3].ability = execute(
        5,
        CombatStatPredicateV1::SelectedHandSlotsMatch,
        own(CombatStatAttributeV1::Power, 2),
    );
    let mut sequential = game(base, cards);
    sequential
        .make(input(PlayerId::P1, (0, 0, false), (1, 0, false)))
        .unwrap();
    let (report, _) = sequential
        .make(input(PlayerId::P2, (3, 0, false), (3, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].hand_slot.get(), 3);
    assert_eq!(report.cards[PlayerId::P1].power, 8);
}

#[test]
fn growth_and_degrowth_use_the_pre_commit_round_for_both_source_kinds() {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    for slot in 0..4 {
        cards[PlayerId::P1][slot].ability = execute(
            1,
            CombatStatPredicateV1::Always,
            modifier(
                CombatStatAffectedSideV1::Player,
                CombatStatAttributeV1::Power,
                CombatStatOperationV1::Increase,
                1,
                None,
                None,
                CombatStatMagnitudeV1::Growth,
            ),
        );
        cards[PlayerId::P2][slot].bonus = execute(
            2,
            CombatStatPredicateV1::Always,
            modifier(
                CombatStatAffectedSideV1::Player,
                CombatStatAttributeV1::Power,
                CombatStatOperationV1::Increase,
                1,
                None,
                None,
                CombatStatMagnitudeV1::Degrowth,
            ),
        );
        cards[PlayerId::P2][slot].source_bonus_support_count = 4;
        cards[PlayerId::P2][slot].effective_clan_id = 200;
    }

    let mut sequential = game(base, cards);
    let initial = sequential.position().clone();
    let (first, undo) = sequential
        .make(input(PlayerId::P2, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(first.cards[PlayerId::P1].power, 7);
    assert_eq!(first.cards[PlayerId::P2].power, 10);
    sequential.unmake(undo);
    assert_eq!(sequential.position(), &initial);

    for (slot, (growth, degrowth)) in [(7, 10), (8, 9), (9, 8), (10, 7)].into_iter().enumerate() {
        let first_mover = if slot % 2 == 0 {
            PlayerId::P1
        } else {
            PlayerId::P2
        };
        let (report, _) = sequential
            .make(input(
                first_mover,
                (slot as u8, 0, false),
                (slot as u8, 0, false),
            ))
            .unwrap();
        assert_eq!(report.round, slot as u8);
        assert_eq!(report.cards[PlayerId::P1].power, growth);
        assert_eq!(report.cards[PlayerId::P2].power, degrowth);
    }
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

    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        13,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::CancelOpponentCombatStatModifiers {
            stat: CombatStatAttributeV1::Power,
        },
    );
    cards[PlayerId::P2][1].ability = execute(
        14,
        CombatStatPredicateV1::SelectedHandSlotsDiffer,
        own(CombatStatAttributeV1::Power, 2),
    );
    let mut conditional_source = game(base, cards);
    let (report, _) = conditional_source
        .make(input(PlayerId::P1, (0, 0, false), (1, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].power, 6);

    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        15,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::CancelOpponentCombatStatModifiers {
            stat: CombatStatAttributeV1::Power,
        },
    );
    cards[PlayerId::P2][0].ability = execute(
        16,
        CombatStatPredicateV1::Always,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Power,
            CombatStatOperationV1::Increase,
            2,
            None,
            None,
            CombatStatMagnitudeV1::Degrowth,
        ),
    );
    let mut round_scaled_source = game(base, cards);
    let (report, _) = round_scaled_source
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].power, 6);

    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    for slot in 0..4 {
        cards[PlayerId::P2][slot].effective_clan_id = 200;
    }
    cards[PlayerId::P1][0].ability = execute(
        17,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::CancelOpponentCombatStatModifiers {
            stat: CombatStatAttributeV1::Power,
        },
    );
    cards[PlayerId::P2][0].ability = execute(
        18,
        CombatStatPredicateV1::Always,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Power,
            CombatStatOperationV1::Increase,
            2,
            None,
            None,
            CombatStatMagnitudeV1::SourceBonusSupport,
        ),
    );
    cards[PlayerId::P2][0].source_ability_support_count = 4;
    let mut support_source = game(base, cards);
    let (report, _) = support_source
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].power, 6);

    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    for slot in 0..4 {
        cards[PlayerId::P2][slot].effective_clan_id = 200;
    }
    cards[PlayerId::P1][0].ability = execute(
        19,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::CancelOpponentCombatStatModifiers {
            stat: CombatStatAttributeV1::Attack,
        },
    );
    cards[PlayerId::P2][0].ability = execute(
        20,
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
    cards[PlayerId::P2][0].source_ability_support_count = 4;
    let mut attack_support_source = game(base, cards);
    let (report, _) = attack_support_source
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].attack, 6);

    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    for slot in 0..4 {
        cards[PlayerId::P2][slot].effective_clan_id = 200;
    }
    cards[PlayerId::P1][0].ability = execute(
        21,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::CancelOpponentCombatStatModifiers {
            stat: CombatStatAttributeV1::Power,
        },
    );
    cards[PlayerId::P2][0].ability = execute(
        22,
        CombatStatPredicateV1::Always,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Damage,
            CombatStatOperationV1::Increase,
            1,
            None,
            None,
            CombatStatMagnitudeV1::SourceBonusSupport,
        ),
    );
    cards[PlayerId::P2][0].source_ability_support_count = 4;
    let mut nonmatching_support_source = game(base, cards);
    let (report, _) = nonmatching_support_source
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].damage, 7);
}

#[test]
fn equalizer_uses_the_selected_opponent_level_for_ability_and_bonus() {
    for opponent_stars in 1_u8..=5 {
        let mut base = base_spec(6, 3);
        base.players[PlayerId::P2].hand[0].key = CardKey::new(200, opponent_stars);
        let mut cards = plans(&base);
        cards[PlayerId::P1][0].ability = execute(
            1342,
            CombatStatPredicateV1::Always,
            modifier(
                CombatStatAffectedSideV1::Player,
                CombatStatAttributeV1::Power,
                CombatStatOperationV1::Increase,
                1,
                None,
                None,
                CombatStatMagnitudeV1::OpponentStars,
            ),
        );
        cards[PlayerId::P1][0].bonus = execute(
            1338,
            CombatStatPredicateV1::Always,
            modifier(
                CombatStatAffectedSideV1::Opponent,
                CombatStatAttributeV1::Attack,
                CombatStatOperationV1::Decrease,
                3,
                Some(5),
                None,
                CombatStatMagnitudeV1::OpponentStars,
            ),
        );
        cards[PlayerId::P1][0].source_bonus_support_count = 1;

        let mut equalizer = game(base, cards);
        let initial = equalizer.position().clone();
        let (report, undo) = equalizer
            .make(input(PlayerId::P1, (0, 0, false), (0, 2, false)))
            .unwrap();
        assert_eq!(
            report.cards[PlayerId::P1].power,
            6 + u16::from(opponent_stars)
        );
        assert_eq!(
            report.cards[PlayerId::P2].attack,
            (18_u32 - 3 * u32::from(opponent_stars)).max(5)
        );
        equalizer.unmake(undo);
        assert_eq!(equalizer.position(), &initial);
    }
}

#[test]
fn equalizer_recomputes_for_sibling_opponent_selections() {
    let mut base = base_spec(6, 3);
    base.players[PlayerId::P2].hand[0].key = CardKey::new(200, 1);
    base.players[PlayerId::P2].hand[1].key = CardKey::new(201, 5);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        1342,
        CombatStatPredicateV1::Always,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Power,
            CombatStatOperationV1::Increase,
            1,
            None,
            None,
            CombatStatMagnitudeV1::OpponentStars,
        ),
    );

    let mut equalizer = game(base, cards);
    let initial = equalizer.position().clone();
    for (opponent_slot, expected_power) in [(0, 7), (1, 11)] {
        let (report, undo) = equalizer
            .make(input(
                PlayerId::P1,
                (0, 0, false),
                (opponent_slot, 0, false),
            ))
            .unwrap();
        assert_eq!(report.cards[PlayerId::P1].power, expected_power);
        equalizer.unmake(undo);
        assert_eq!(equalizer.position(), &initial);
    }
}

#[test]
fn source_bonus_context_groups_by_effective_clan_not_source_id() {
    let base = base_spec(6, 2);
    let mut cards = plans(&base);
    for (slot, source_id) in [(0, 23), (1, 25)] {
        cards[PlayerId::P1][slot].effective_clan_id = 999;
        cards[PlayerId::P1][slot].bonus = execute(
            source_id,
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
        cards[PlayerId::P1][slot].source_bonus_support_count = 2;
    }

    let mut grouped = game(base.clone(), cards.clone());
    let (report, _) = grouped
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].attack, 12);

    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
            base_rules: base,
            cards,
        }),
        Err(CombatStatPlanErrorV1::InvalidSourceBonusContext {
            player: PlayerId::P1,
            source_id: Some(23),
            expected: 2,
            actual: 1,
            ..
        })
    ));
}

#[test]
fn ability_support_uses_its_own_immutable_effective_clan_context() {
    let base = base_spec(6, 2);
    let mut singleton = plans(&base);
    singleton[PlayerId::P1][0].ability = execute(
        701,
        CombatStatPredicateV1::Always,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Power,
            CombatStatOperationV1::Increase,
            2,
            None,
            None,
            CombatStatMagnitudeV1::SourceBonusSupport,
        ),
    );
    singleton[PlayerId::P1][0].source_ability_support_count = 1;
    let mut singleton_game = game(base.clone(), singleton);
    let before = singleton_game.position().clone();
    let (report, undo) = singleton_game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 8);
    singleton_game.unmake(undo);
    assert_eq!(singleton_game.position(), &before);

    let mut four = plans(&base);
    for player in PlayerId::ALL {
        for slot in 0..4 {
            four[player][slot].effective_clan_id = 999 + player.index() as u32;
        }
    }
    four[PlayerId::P1][0].ability = execute(
        702,
        CombatStatPredicateV1::Always,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Power,
            CombatStatOperationV1::Increase,
            2,
            None,
            None,
            CombatStatMagnitudeV1::SourceBonusSupport,
        ),
    );
    four[PlayerId::P1][0].source_ability_support_count = 4;
    four[PlayerId::P2][0].ability = execute(
        703,
        CombatStatPredicateV1::Always,
        modifier(
            CombatStatAffectedSideV1::Opponent,
            CombatStatAttributeV1::Attack,
            CombatStatOperationV1::Decrease,
            3,
            Some(2),
            None,
            CombatStatMagnitudeV1::SourceBonusSupport,
        ),
    );
    four[PlayerId::P2][0].source_ability_support_count = 4;
    let mut four_game = game(base.clone(), four.clone());
    let before = four_game.position().clone();
    let (_, first_undo) = four_game
        .make(input(PlayerId::P1, (1, 0, false), (1, 0, false)))
        .unwrap();
    let (report, undo) = four_game
        .make(input(PlayerId::P2, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 14);
    assert_eq!(report.cards[PlayerId::P1].attack, 2);
    four_game.unmake(undo);
    four_game.unmake(first_undo);
    assert_eq!(four_game.position(), &before);

    // Ability and bonus contexts are independently validated: no bonus exists here,
    // so its count stays zero while the executable abilities use four.
    assert_eq!(four[PlayerId::P1][0].source_bonus_support_count, 0);
    assert_eq!(four[PlayerId::P2][0].source_bonus_support_count, 0);
    four[PlayerId::P1][0].source_ability_support_count = 3;
    assert!(matches!(
        CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
            base_rules: base.clone(),
            cards: four,
        }),
        Err(CombatStatPlanErrorV1::InvalidAbilitySupportContext {
            player: PlayerId::P1,
            source_id: Some(702),
            expected: 4,
            actual: 3,
            ..
        })
    ));

    let mut non_support = plans(&base);
    non_support[PlayerId::P1][0].ability = execute(
        704,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Power, 2),
    );
    non_support[PlayerId::P1][0].source_ability_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
            base_rules: base,
            cards: non_support,
        }),
        Err(CombatStatPlanErrorV1::InvalidAbilitySupportContext {
            source_id: Some(704),
            expected: 0,
            actual: 1,
            ..
        })
    ));
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
        cards[PlayerId::P2][slot].effective_clan_id = 200;
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

    let base = base_spec(6, 2);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        2299,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentBonus,
    );
    cards[PlayerId::P2][0].bonus = execute(
        17,
        CombatStatPredicateV1::Always,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Power,
            CombatStatOperationV1::Increase,
            2,
            None,
            None,
            CombatStatMagnitudeV1::Growth,
        ),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let mut round_scaled_bonus = game(base, cards);
    let (report, _) = round_scaled_bonus
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].power, 6);
}

#[test]
fn stop_opponent_ability_preserves_bonus_and_stops_ability_stats() {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        11,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Power, 2),
    );
    cards[PlayerId::P1][0].bonus = execute(
        12,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Damage, 2),
    );
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    cards[PlayerId::P2][0].ability = execute(
        41,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );

    let mut game = game(base, cards);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 6);
    assert_eq!(report.cards[PlayerId::P1].damage, 5);
}

#[test]
fn stop_opponent_ability_suppresses_ability_post_round_and_unmake_is_exact() {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        1034,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat,
    );
    cards[PlayerId::P2][0].ability = execute(
        41,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );

    let mut game = game(base, cards);
    let before = game.position().clone();
    let before_hash = position_hash(&before);
    let (report, undo) = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].pillz, 20);
    game.unmake(undo);
    assert_eq!(game.position(), &before);
    assert_eq!(position_hash(game.position()), before_hash);
}

#[test]
fn soa_and_sob_resolve_by_source_dependency_and_cycles_restore_exactly() {
    // An SOA kills the opposing ability-origin SOB before it can stop the bonus.
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        41,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    cards[PlayerId::P2][0].ability = execute(
        2299,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentBonus,
    );
    cards[PlayerId::P2][0].bonus = execute(
        12,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Damage, 2),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let mut soa_first = game(base, cards);
    let (report, _) = soa_first
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].damage, 5);

    // Conversely, SOA wins over an ability-origin SOB because the SOB is its target;
    // that SOB cannot then suppress the SOA owner's bonus.
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        2299,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentBonus,
    );
    cards[PlayerId::P1][0].bonus = execute(
        12,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Damage, 2),
    );
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    cards[PlayerId::P2][0].ability = execute(
        41,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    let mut soa_second = game(base, cards);
    let (report, _) = soa_second
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].damage, 5);

    // Cross-source mutual stops are a real PRE4 cycle.  The fixed P1/bonus-first
    // fallback must terminate and must leave no mutable state outside BaseRulesUndo.
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].bonus = execute(
        41,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    cards[PlayerId::P2][0].ability = execute(
        2299,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentBonus,
    );
    let mut cycle = game(base, cards);
    let before = cycle.position().clone();
    let before_hash = position_hash(&before);
    let (_, undo) = cycle
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    cycle.unmake(undo);
    assert_eq!(cycle.position(), &before);
    assert_eq!(position_hash(cycle.position()), before_hash);
}

#[test]
fn impossible_execute_plans_fail_at_construction() {
    let base = base_spec(6, 2);
    let cases = [
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
        (
            execute(
                42,
                CombatStatPredicateV1::OwnerMovesFirst,
                CombatStatEffectV1::StopOpponentAbility,
            ),
            InvalidCombatStatPlanReasonV1::ConditionalControl,
        ),
        (
            execute(
                4,
                CombatStatPredicateV1::SelectedHandSlotsMatch,
                CombatStatEffectV1::StopOpponentBonus,
            ),
            InvalidCombatStatPlanReasonV1::ConditionalControl,
        ),
        (
            execute(
                41,
                CombatStatPredicateV1::OwnerWonPreviousRound,
                CombatStatEffectV1::StopOpponentBonus,
            ),
            InvalidCombatStatPlanReasonV1::ConditionalControl,
        ),
        (
            execute(
                5,
                CombatStatPredicateV1::OwnerMovesFirst,
                modifier(
                    CombatStatAffectedSideV1::Player,
                    CombatStatAttributeV1::Power,
                    CombatStatOperationV1::Increase,
                    1,
                    None,
                    None,
                    CombatStatMagnitudeV1::Growth,
                ),
            ),
            InvalidCombatStatPlanReasonV1::CompoundPredicateAndMagnitude,
        ),
        (
            execute(
                6,
                CombatStatPredicateV1::SelectedHandSlotsDiffer,
                modifier(
                    CombatStatAffectedSideV1::Player,
                    CombatStatAttributeV1::Power,
                    CombatStatOperationV1::Increase,
                    1,
                    None,
                    None,
                    CombatStatMagnitudeV1::OpponentStars,
                ),
            ),
            InvalidCombatStatPlanReasonV1::CompoundPredicateAndMagnitude,
        ),
        (
            execute(
                61,
                CombatStatPredicateV1::OwnerLostPreviousRound,
                modifier(
                    CombatStatAffectedSideV1::Player,
                    CombatStatAttributeV1::Power,
                    CombatStatOperationV1::Increase,
                    1,
                    None,
                    None,
                    CombatStatMagnitudeV1::Growth,
                ),
            ),
            InvalidCombatStatPlanReasonV1::CompoundPredicateAndMagnitude,
        ),
        (
            execute(
                7,
                CombatStatPredicateV1::OwnerMovesFirst,
                modifier(
                    CombatStatAffectedSideV1::Player,
                    CombatStatAttributeV1::Power,
                    CombatStatOperationV1::Increase,
                    1,
                    None,
                    None,
                    CombatStatMagnitudeV1::SourceBonusSupport,
                ),
            ),
            InvalidCombatStatPlanReasonV1::SupportAbility,
        ),
        (
            execute(
                8,
                CombatStatPredicateV1::Always,
                modifier(
                    CombatStatAffectedSideV1::Player,
                    CombatStatAttributeV1::PowerAndDamage,
                    CombatStatOperationV1::Increase,
                    1,
                    None,
                    None,
                    CombatStatMagnitudeV1::SourceBonusSupport,
                ),
            ),
            InvalidCombatStatPlanReasonV1::SupportAbility,
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
        6,
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
        7,
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

    let mut previous_round_fixed_bonus = plans(&base);
    previous_round_fixed_bonus[PlayerId::P1][0].bonus = execute(
        71,
        CombatStatPredicateV1::OwnerWonPreviousRound,
        own(CombatStatAttributeV1::Power, 2),
    );
    previous_round_fixed_bonus[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(
        CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
            base_rules: base.clone(),
            cards: previous_round_fixed_bonus,
        })
        .is_ok()
    );

    let mut conditional_support_bonus = plans(&base);
    conditional_support_bonus[PlayerId::P1][0].bonus = execute(
        8,
        CombatStatPredicateV1::SelectedHandSlotsDiffer,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Attack,
            CombatStatOperationV1::Increase,
            2,
            None,
            None,
            CombatStatMagnitudeV1::SourceBonusSupport,
        ),
    );
    conditional_support_bonus[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
            base_rules: base.clone(),
            cards: conditional_support_bonus,
        }),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            source: CombatStatEffectSourceV1::Bonus,
            reason: InvalidCombatStatPlanReasonV1::ConditionalBonus,
            ..
        })
    ));

    let mut previous_round_support_bonus = plans(&base);
    previous_round_support_bonus[PlayerId::P1][0].bonus = execute(
        81,
        CombatStatPredicateV1::OwnerLostPreviousRound,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Attack,
            CombatStatOperationV1::Increase,
            2,
            None,
            None,
            CombatStatMagnitudeV1::SourceBonusSupport,
        ),
    );
    previous_round_support_bonus[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
            base_rules: base.clone(),
            cards: previous_round_support_bonus,
        }),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            source: CombatStatEffectSourceV1::Bonus,
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

    let base = base_spec(u16::MAX - 3, 0);
    let mut cards = plans(&base);
    cards[PlayerId::P1][3].ability = execute(
        4,
        CombatStatPredicateV1::Always,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Power,
            CombatStatOperationV1::Increase,
            1,
            None,
            None,
            CombatStatMagnitudeV1::Growth,
        ),
    );
    let mut late_growth = game(base, cards);
    for slot in 0..3 {
        late_growth
            .make(input(PlayerId::P1, (slot, 0, false), (slot, 0, false)))
            .unwrap();
    }
    let before = late_growth.position().clone();
    let hash = position_hash(&before);
    assert!(matches!(
        late_growth.make(input(PlayerId::P1, (3, 0, false), (3, 0, false))),
        Err(CombatStatDiagnosticErrorV1::ArithmeticOverflow {
            player: PlayerId::P1,
            ..
        })
    ));
    assert_eq!(late_growth.position(), &before);
    assert_eq!(position_hash(late_growth.position()), hash);
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
