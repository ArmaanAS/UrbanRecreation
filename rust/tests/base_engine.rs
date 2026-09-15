use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use urban_recreation_rust::catalog::CardKey;
use urban_recreation_rust::engine::{
    BaseRulesCardSpec, BaseRulesError, BaseRulesGame, BaseRulesMatchSpec, BaseRulesPlayerSpec,
    BaseRulesPosition, BaseRulesRoundInput, BaseRulesSelection, ByPlayer, MatchStatus, PlayerId,
};

fn card(id: u32, level: u8, power: u16, damage: u16) -> BaseRulesCardSpec {
    BaseRulesCardSpec {
        key: CardKey::new(id, level),
        clan_id: id,
        power,
        damage,
    }
}

fn spec(
    p1_resources: (u16, u16),
    p2_resources: (u16, u16),
    p1: [(u8, u16, u16); 4],
    p2: [(u8, u16, u16); 4],
) -> BaseRulesMatchSpec {
    let make_hand = |base, entries: [(u8, u16, u16); 4]| {
        entries.map(|(level, power, damage)| {
            let id = base + u32::from(level);
            card(id, level, power, damage)
        })
    };
    BaseRulesMatchSpec {
        battle_rule_id: 10,
        night: false,
        players: ByPlayer::new(
            BaseRulesPlayerSpec {
                initial_life: p1_resources.0,
                initial_pillz: p1_resources.1,
                hand: make_hand(100, p1),
            },
            BaseRulesPlayerSpec {
                initial_life: p2_resources.0,
                initial_pillz: p2_resources.1,
                hand: make_hand(200, p2),
            },
        ),
    }
}

fn plain_spec() -> BaseRulesMatchSpec {
    spec(
        (12, 12),
        (12, 12),
        [(2, 7, 3), (2, 7, 3), (2, 7, 3), (2, 7, 3)],
        [(3, 6, 2), (3, 6, 2), (3, 6, 2), (3, 6, 2)],
    )
}

fn input(first_mover: PlayerId, p1: (u8, u16, bool), p2: (u8, u16, bool)) -> BaseRulesRoundInput {
    BaseRulesRoundInput {
        first_mover,
        selections: ByPlayer::new(
            BaseRulesSelection::new(p1.0, p1.1, p1.2),
            BaseRulesSelection::new(p2.0, p2.1, p2.2),
        ),
    }
}

fn position_hash(position: &BaseRulesPosition) -> u64 {
    let mut hasher = DefaultHasher::new();
    position.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn base_rules_include_free_pill_and_fury_cost_and_damage() {
    let mut free = BaseRulesGame::new(plain_spec());
    let (report, _) = free
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].attack, 7);
    assert_eq!(report.cards[PlayerId::P2].attack, 6);
    assert_eq!(report.players[PlayerId::P2].life, 9);
    assert_eq!(report.players[PlayerId::P1].pillz, 12);

    let mut fury = BaseRulesGame::new(plain_spec());
    let (report, _) = fury
        .make(input(PlayerId::P1, (0, 1, true), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].damage, 5);
    assert_eq!(report.players[PlayerId::P2].life, 7);
    assert_eq!(report.players[PlayerId::P1].pillz, 8);
}

#[test]
fn ties_use_lower_level_then_explicit_first_mover() {
    let lower_level = spec((12, 12), (12, 12), [(2, 6, 3); 4], [(3, 6, 3); 4]);
    let mut game = BaseRulesGame::new(lower_level);
    let (report, _) = game
        .make(input(PlayerId::P2, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);

    let equal_level = spec((12, 12), (12, 12), [(2, 6, 3); 4], [(2, 6, 3); 4]);
    let mut game = BaseRulesGame::new(equal_level);
    let (report, _) = game
        .make(input(PlayerId::P2, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P2].won);
}

#[test]
fn first_mover_is_explicit_and_may_repeat_between_rounds() {
    let equal = spec((12, 12), (12, 12), [(2, 6, 1); 4], [(2, 6, 1); 4]);
    let mut game = BaseRulesGame::new(equal);
    let (first, _) = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    let (second, _) = game
        .make(input(PlayerId::P1, (1, 0, false), (1, 0, false)))
        .unwrap();
    assert!(first.cards[PlayerId::P1].won);
    assert!(second.cards[PlayerId::P1].won);
}

#[test]
fn asymmetric_initial_resources_are_independent() {
    let asymmetric = spec((15, 8), (9, 4), [(2, 7, 3); 4], [(3, 6, 2); 4]);
    let mut game = BaseRulesGame::new(asymmetric);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 2, false), (0, 1, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].life, 15);
    assert_eq!(report.players[PlayerId::P2].life, 6);
    assert_eq!(report.players[PlayerId::P1].pillz, 6);
    assert_eq!(report.players[PlayerId::P2].pillz, 3);
}

#[test]
fn invalid_repeated_and_over_budget_rounds_are_atomic() {
    let mut game = BaseRulesGame::new(plain_spec());
    for (round, expected) in [
        (
            input(PlayerId::P1, (4, 0, false), (0, 0, false)),
            "InvalidHandSlot",
        ),
        (
            input(PlayerId::P1, (0, 0, false), (4, 0, false)),
            "InvalidHandSlot",
        ),
        (
            input(PlayerId::P1, (0, 13, false), (0, 0, false)),
            "InsufficientPillz",
        ),
        (
            input(PlayerId::P1, (0, u16::MAX, true), (0, 0, false)),
            "CostOverflow",
        ),
    ] {
        let before = game.position().clone();
        let hash = position_hash(&before);
        let error = game.make(round).unwrap_err();
        assert!(format!("{error:?}").starts_with(expected));
        assert_eq!(game.position(), &before);
        assert_eq!(position_hash(game.position()), hash);
    }

    game.make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    let before = game.position().clone();
    let error = game
        .make(input(PlayerId::P2, (0, 0, false), (1, 0, false)))
        .unwrap_err();
    assert!(matches!(
        error,
        BaseRulesError::CardAlreadyPlayed {
            player: PlayerId::P1,
            ..
        }
    ));
    assert_eq!(game.position(), &before);
}

#[test]
fn post_knockout_and_fifth_round_attempts_leave_state_unchanged() {
    let knockout = spec((12, 12), (5, 12), [(2, 8, 9); 4], [(3, 1, 1); 4]);
    let mut game = BaseRulesGame::new(knockout);
    game.make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(game.position().status, MatchStatus::Won(PlayerId::P1));
    let before = game.position().clone();
    assert!(matches!(
        game.make(input(PlayerId::P2, (1, 0, false), (1, 0, false))),
        Err(BaseRulesError::MatchFinished { .. })
    ));
    assert_eq!(game.position(), &before);

    let equal = spec((12, 12), (12, 12), [(2, 6, 0); 4], [(2, 6, 0); 4]);
    let mut game = BaseRulesGame::new(equal);
    for slot in 0..4 {
        game.make(input(PlayerId::P1, (slot, 0, false), (slot, 0, false)))
            .unwrap();
    }
    assert_eq!(game.position().status, MatchStatus::Draw);
    let before = game.position().clone();
    assert!(matches!(
        game.make(input(PlayerId::P1, (0, 0, false), (0, 0, false))),
        Err(BaseRulesError::RoundLimitReached { rounds_played: 4 })
    ));
    assert_eq!(game.position(), &before);
}

#[test]
fn initial_zero_life_and_double_zero_are_terminal() {
    for (resources, expected) in [
        (((0, 12), (12, 12)), MatchStatus::Won(PlayerId::P2)),
        (((12, 12), (0, 12)), MatchStatus::Won(PlayerId::P1)),
        (((0, 12), (0, 12)), MatchStatus::Draw),
    ] {
        let match_spec = spec(resources.0, resources.1, [(2, 6, 1); 4], [(2, 6, 1); 4]);
        let mut game = BaseRulesGame::new(match_spec);
        assert_eq!(game.position().status, expected);
        let before = game.position().clone();
        assert!(matches!(
            game.make(input(PlayerId::P1, (0, 0, false), (0, 0, false))),
            Err(BaseRulesError::MatchFinished { .. })
        ));
        assert_eq!(game.position(), &before);
    }
}

#[test]
fn four_round_status_compares_life() {
    for (p1_damage, p2_damage, expected) in [
        (1, 0, MatchStatus::Won(PlayerId::P1)),
        (0, 1, MatchStatus::Won(PlayerId::P2)),
        (0, 0, MatchStatus::Draw),
    ] {
        let match_spec = spec(
            (12, 12),
            (12, 12),
            [(2, 6, p1_damage); 4],
            [(2, 6, p2_damage); 4],
        );
        let mut game = BaseRulesGame::new(match_spec);
        for slot in 0..4 {
            let first = if p1_damage >= p2_damage {
                PlayerId::P1
            } else {
                PlayerId::P2
            };
            game.make(input(first, (slot, 0, false), (slot, 0, false)))
                .unwrap();
        }
        assert_eq!(game.position().status, expected);
    }
}

#[test]
fn attack_uses_u32_beyond_u16_range() {
    let wide = spec((12, 50), (12, 50), [(2, 2_000, 1); 4], [(3, 1, 1); 4]);
    let mut game = BaseRulesGame::new(wide);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 40, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].attack, 82_000);
}

#[test]
fn maximum_base_attack_fits_u32_exactly() {
    let maximum = spec(
        (12, u16::MAX),
        (12, 0),
        [(2, u16::MAX, 1); 4],
        [(3, 1, 1); 4],
    );
    let mut game = BaseRulesGame::new(maximum);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, u16::MAX, false), (0, 0, false)))
        .unwrap();
    assert_eq!(
        report.cards[PlayerId::P1].attack,
        u32::from(u16::MAX) * (u32::from(u16::MAX) + 1)
    );
}

#[test]
fn fury_damage_overflow_is_atomic() {
    let maximum_damage = spec((12, 3), (12, 0), [(2, 8, u16::MAX); 4], [(3, 1, 1); 4]);
    let mut game = BaseRulesGame::new(maximum_damage);
    let before = game.position().clone();
    let before_hash = position_hash(&before);

    assert!(matches!(
        game.make(input(PlayerId::P1, (0, 0, true), (0, 0, false))),
        Err(BaseRulesError::DamageOverflow {
            player: PlayerId::P1
        })
    ));
    assert_eq!(game.position(), &before);
    assert_eq!(position_hash(game.position()), before_hash);
}

#[test]
fn make_unmake_restores_exact_prefix_hash_and_isolates_siblings_and_games() {
    let mut game = BaseRulesGame::new(plain_spec());
    let initial = game.position().clone();
    let (_, undo_first) = game
        .make(input(PlayerId::P1, (0, 1, false), (0, 0, false)))
        .unwrap();
    let prefix = game.position().clone();
    let prefix_hash = position_hash(&prefix);

    let (_, sibling_a) = game
        .make(input(PlayerId::P2, (1, 0, false), (1, 2, false)))
        .unwrap();
    game.unmake(sibling_a);
    assert_eq!(game.position(), &prefix);
    assert_eq!(position_hash(game.position()), prefix_hash);

    let (_, sibling_b) = game
        .make(input(PlayerId::P1, (2, 2, false), (2, 0, false)))
        .unwrap();
    game.unmake(sibling_b);
    assert_eq!(game.position(), &prefix);

    let other = BaseRulesGame::new(plain_spec());
    assert_eq!(other.position(), &initial);
    assert_ne!(other.position(), game.position());

    game.unmake(undo_first);
    assert_eq!(game.position(), &initial);
    assert_eq!(position_hash(game.position()), position_hash(&initial));
}
