use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use urban_recreation_rust::catalog::CardKey;
use urban_recreation_rust::effect_registry::ResourceCancellationV1;
use urban_recreation_rust::engine::{
    BaseRulesError, BaseRulesMatchSpec, BaseRulesPlayerSpec, BaseRulesPosition,
    BaseRulesRoundInput, BaseRulesSelection, ByPlayer, ClanSetV1, CombatStatAffectedSideV1,
    CombatStatAttributeV1, CombatStatCardPlanV1, CombatStatDiagnosticErrorV1,
    CombatStatDiagnosticMatchSpecV1, CombatStatDiagnosticV1, CombatStatEffectSourceV1,
    CombatStatEffectV1, CombatStatMagnitudeV1, CombatStatOperationV1, CombatStatPlanErrorV1,
    CombatStatPredicateV1, CombatStatSourcePlanV1, CopiedSourceKindV1,
    InvalidCombatStatPlanReasonV1, MatchStatus, PlayerId, RoundScaleV1,
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

fn vod_spec(
    key: CardKey,
    source_id: u32,
    effect: CombatStatEffectV1,
) -> CombatStatDiagnosticMatchSpecV1 {
    let mut base = base_spec(6, 3);
    base.players[PlayerId::P1].hand[0].key = key;
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(source_id, CombatStatPredicateV1::Always, effect);
    CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    }
}

const LIANAH: CardKey = CardKey { id: 978, level: 3 };
const HEAL: CombatStatEffectV1 = CombatStatEffectV1::HealLifeOnVictory {
    life: 1,
    maximum: 20,
};

/// P1 holds Lianah Ld in slot 0 with her Heal; every other source is absent. Both hands
/// are 6/3, so the higher bet wins and a tie goes to the first mover.
fn lianah_spec(p1_life: u16) -> CombatStatDiagnosticMatchSpecV1 {
    let mut base = base_spec(6, 3);
    base.players[PlayerId::P1].initial_life = p1_life;
    base.players[PlayerId::P1].hand[0].key = LIANAH;
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(3526, CombatStatPredicateV1::Always, HEAL);
    CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    }
}

#[test]
fn heal_plan_is_ability_only_positive_below_its_cap_and_unconditional() {
    assert!(CombatStatDiagnosticV1::new(lianah_spec(12)).is_ok());
    // Generic by grammar: any id and any card may carry it, with any positive numbers.
    let mut other = lianah_spec(12);
    other.base_rules.players[PlayerId::P1].hand[0].key = CardKey::new(448, 3);
    other.cards[PlayerId::P1][0].key = CardKey::new(448, 3);
    other.cards[PlayerId::P1][0].ability = execute(
        963,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::HealLifeOnVictory {
            life: 2,
            maximum: 10,
        },
    );
    assert!(CombatStatDiagnosticV1::new(other).is_ok());

    let mut bonus = lianah_spec(12);
    bonus.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    bonus.cards[PlayerId::P1][0].bonus = execute(3526, CombatStatPredicateV1::Always, HEAL);
    bonus.cards[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(bonus),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::HealLifeSource,
            source: CombatStatEffectSourceV1::Bonus,
            ..
        })
    ));

    for effect in [
        CombatStatEffectV1::HealLifeOnVictory {
            life: 0,
            maximum: 20,
        },
        CombatStatEffectV1::HealLifeOnVictory {
            life: 1,
            maximum: 1,
        },
        CombatStatEffectV1::HealLifeOnVictory {
            life: 3,
            maximum: 2,
        },
    ] {
        let mut wrong_effect = lianah_spec(12);
        wrong_effect.cards[PlayerId::P1][0].ability =
            execute(3526, CombatStatPredicateV1::Always, effect);
        assert!(matches!(
            CombatStatDiagnosticV1::new(wrong_effect),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::HealLifeMagnitude,
                ..
            })
        ));
    }

    let mut wrong_predicate = lianah_spec(12);
    wrong_predicate.cards[PlayerId::P1][0].ability =
        execute(3526, CombatStatPredicateV1::OwnerMovesFirst, HEAL);
    assert!(matches!(
        CombatStatDiagnosticV1::new(wrong_predicate),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::HealLifePredicate,
            ..
        })
    ));
}

#[test]
fn heal_latches_on_a_win_pays_nothing_that_round_and_one_life_after_every_later_round() {
    let spec = lianah_spec(18);
    let mut game = game(spec.base_rules, spec.cards);
    let start = game.position().clone();
    let start_hash = position_hash(&start);

    // Round 1: Lianah wins. Nothing is paid, but the position now carries the latch.
    let (first, undo_first) = game
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(first.cards[PlayerId::P1].won);
    assert_eq!(first.players[PlayerId::P1].life, 18);
    assert_eq!(first.players[PlayerId::P2].life, 17);
    assert_eq!(game.position().latched[PlayerId::P1].len(), 1);
    assert!(game.position().latched[PlayerId::P2].is_empty());
    let after_first = game.position().clone();
    let after_first_hash = position_hash(&after_first);
    assert_ne!(after_first_hash, start_hash);

    // Round 2: P1 loses with a card that has no ability at all. Damage lands first, then
    // the latched Heal pays its one Life.
    let (second, undo_second) = game
        .make(input(PlayerId::P2, (1, 0, false), (1, 3, false)))
        .unwrap();
    assert!(!second.cards[PlayerId::P1].won);
    assert_eq!(second.players[PlayerId::P1].life, 18 - 3 + 1);
    assert_eq!(game.position().latched[PlayerId::P1].len(), 1);

    // Round 3: P1 wins again; the repeat still pays and nothing latches twice.
    let (third, undo_third) = game
        .make(input(PlayerId::P1, (2, 1, false), (2, 0, false)))
        .unwrap();
    assert!(third.cards[PlayerId::P1].won);
    assert_eq!(third.players[PlayerId::P1].life, 17);
    assert_eq!(game.position().latched[PlayerId::P1].len(), 1);

    // Round 4: another loss, another payment; the match then ends on Life.
    let (fourth, undo_fourth) = game
        .make(input(PlayerId::P2, (3, 0, false), (3, 2, false)))
        .unwrap();
    assert_eq!(fourth.players[PlayerId::P1].life, 17 - 3 + 1);
    assert_eq!(fourth.players[PlayerId::P2].life, 20 - 3 - 3);
    assert_eq!(fourth.status, MatchStatus::Won(PlayerId::P1));

    // Undo walks the latch back out exactly.
    game.unmake(undo_fourth);
    game.unmake(undo_third);
    game.unmake(undo_second);
    assert_eq!(game.position(), &after_first);
    assert_eq!(position_hash(game.position()), after_first_hash);
    game.unmake(undo_first);
    assert_eq!(game.position(), &start);
    assert_eq!(position_hash(game.position()), start_hash);
    assert!(game.position().latched[PlayerId::P1].is_empty());
}

#[test]
fn heal_is_capped_at_its_maximum_and_never_lowers_a_higher_life() {
    // 19 -> 20 -> 20: a player at the cap is left exactly where they are.
    let spec = lianah_spec(19);
    let mut diag = game(spec.base_rules, spec.cards);
    diag.make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    let (second, _) = diag
        .make(input(PlayerId::P1, (1, 2, false), (1, 0, false)))
        .unwrap();
    assert_eq!(second.players[PlayerId::P1].life, 20);
    let (third, _) = diag
        .make(input(PlayerId::P1, (2, 2, false), (2, 0, false)))
        .unwrap();
    assert_eq!(third.players[PlayerId::P1].life, 20);

    // A player already above the cap is not pulled down to it.
    let spec = lianah_spec(25);
    let mut diag = game(spec.base_rules, spec.cards);
    diag.make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    let (second, _) = diag
        .make(input(PlayerId::P1, (1, 2, false), (1, 0, false)))
        .unwrap();
    assert_eq!(second.players[PlayerId::P1].life, 25);
}

#[test]
fn heal_pays_after_the_rounds_own_victory_life_and_the_cap_sees_that_order() {
    // The owner is on 18 with the latch. Winning with a `+2 Life` bonus pays that first,
    // taking Life to 20, and the repeat then finds the cap already reached: 20, not 21.
    let mut spec = lianah_spec(18);
    spec.cards[PlayerId::P1][1].bonus = execute(
        401,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::GainLifeOnVictory { life: 2 },
    );
    spec.cards[PlayerId::P1][1].source_bonus_support_count = 1;
    let mut game = game(spec.base_rules, spec.cards);
    game.make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    let (second, _) = game
        .make(input(PlayerId::P1, (1, 2, false), (1, 0, false)))
        .unwrap();
    assert!(second.cards[PlayerId::P1].won);
    assert_eq!(second.players[PlayerId::P1].life, 20);
}

#[test]
fn heal_does_not_latch_on_a_loss_or_when_stopped_and_never_revives_a_ko() {
    // Losing the round it is played: no latch, so nothing is paid later.
    let spec = lianah_spec(17);
    let mut diag = game(spec.base_rules, spec.cards);
    let (first, _) = diag
        .make(input(PlayerId::P2, (0, 0, false), (0, 2, false)))
        .unwrap();
    assert!(!first.cards[PlayerId::P1].won);
    assert_eq!(first.players[PlayerId::P1].life, 14);
    assert!(diag.position().latched[PlayerId::P1].is_empty());
    let (second, _) = diag
        .make(input(PlayerId::P1, (1, 2, false), (1, 0, false)))
        .unwrap();
    assert_eq!(second.players[PlayerId::P1].life, 14);

    // Stopped in its own round: the win is not enough, the source has to be live then. A
    // later round without the Stop does not resurrect it.
    let mut spec = lianah_spec(17);
    spec.cards[PlayerId::P2][0].ability = execute(
        4437,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    let mut diag = game(spec.base_rules, spec.cards);
    let (first, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(first.cards[PlayerId::P1].won);
    assert!(diag.position().latched[PlayerId::P1].is_empty());
    let (second, _) = diag
        .make(input(PlayerId::P2, (1, 0, false), (1, 2, false)))
        .unwrap();
    assert_eq!(second.players[PlayerId::P1].life, 17 - 3);

    // A latched owner taken to zero stays at zero: the repeat is ordinary Life, not
    // Reanimate, and the KO is terminal.
    let spec = lianah_spec(3);
    let mut diag = game(spec.base_rules, spec.cards);
    diag.make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    let (second, _) = diag
        .make(input(PlayerId::P2, (1, 0, false), (1, 2, false)))
        .unwrap();
    assert_eq!(second.players[PlayerId::P1].life, 0);
    assert_eq!(second.status, MatchStatus::Won(PlayerId::P2));
}

#[test]
fn heal_pays_in_the_round_that_kos_the_opponent_as_in_capture_878093() {
    // Lianah Ld (8 + 2 Power, 3 Damage) beats Madlocks 40 to 36 on three pillz each and
    // takes the opponent to 9. Buck then wins the second round with 9 Damage for the KO,
    // and the server left Lianah's owner on 13: the latched Heal paid at the end of the
    // round that ended the match.
    let mut base = base_spec(6, 3);
    base.players[PlayerId::P1].initial_life = 12;
    base.players[PlayerId::P2].initial_life = 12;
    base.players[PlayerId::P1].hand[0] = urban_recreation_rust::engine::BaseRulesCardSpec {
        key: LIANAH,
        clan_id: 10,
        power: 10,
        damage: 3,
    };
    base.players[PlayerId::P1].hand[1].power = 6;
    base.players[PlayerId::P1].hand[1].damage = 7;
    base.players[PlayerId::P2].hand[0].power = 9;
    base.players[PlayerId::P2].hand[0].damage = 2;
    base.players[PlayerId::P2].hand[1].power = 7;
    base.players[PlayerId::P2].hand[1].damage = 6;
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(3526, CombatStatPredicateV1::Always, HEAL);
    let mut game = game(base, cards);
    let (first, _) = game
        .make(input(PlayerId::P1, (0, 3, false), (0, 3, false)))
        .unwrap();
    assert_eq!(first.cards[PlayerId::P1].attack, 40);
    assert_eq!(first.cards[PlayerId::P2].attack, 36);
    assert!(first.cards[PlayerId::P1].won);
    assert_eq!(first.players[PlayerId::P1].life, 12);
    assert_eq!(first.players[PlayerId::P2].life, 9);
    let (second, _) = game
        .make(input(PlayerId::P2, (1, 6, true), (1, 1, false)))
        .unwrap();
    assert!(second.cards[PlayerId::P1].won);
    assert_eq!(second.cards[PlayerId::P1].damage, 9);
    assert_eq!(second.players[PlayerId::P2].life, 0);
    assert_eq!(second.players[PlayerId::P1].life, 13);
    assert_eq!(second.status, MatchStatus::Won(PlayerId::P1));
}

/// P1 slot 0 carries `effect` in the Ability slot under `source_id`; everything else is
/// absent, both hands 6/3, both players on `life`.
fn permanent_spec(
    source_id: u32,
    effect: CombatStatEffectV1,
    life: u16,
) -> CombatStatDiagnosticMatchSpecV1 {
    let mut base = base_spec(6, 3);
    base.players[PlayerId::P1].initial_life = life;
    base.players[PlayerId::P2].initial_life = life;
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(source_id, CombatStatPredicateV1::Always, effect);
    CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    }
}

const TOXIN: CombatStatEffectV1 = CombatStatEffectV1::ToxinOpponentLifeOnVictory {
    life: 1,
    minimum: 0,
};
const POISON: CombatStatEffectV1 = CombatStatEffectV1::PoisonOpponentLifeOnVictory {
    life: 2,
    minimum: 3,
};
const REGEN: CombatStatEffectV1 = CombatStatEffectV1::RegenLifeOnVictory {
    life: 3,
    maximum: 6,
};

#[test]
fn toxin_pays_in_its_latching_round_after_damage_and_keeps_paying_after_its_owner_is_ko() {
    // Round 1: P1 wins with 3 Damage and the Toxin takes one more at once: 12 - 3 - 1 = 8,
    // as Zis does to AI-Lycs' owner in 963039/0.
    let spec = permanent_spec(1508, TOXIN, 12);
    let mut diag = game(spec.base_rules, spec.cards);
    let start = diag.position().clone();
    let (first, undo_first) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(first.cards[PlayerId::P1].won);
    assert_eq!(first.players[PlayerId::P2].life, 8);
    assert_eq!(first.players[PlayerId::P1].life, 12);
    assert_eq!(diag.position().latched[PlayerId::P1].len(), 1);
    // Round 2: P1 loses; the Toxin still pays on P2 after P1 takes damage.
    let (second, undo_second) = diag
        .make(input(PlayerId::P2, (1, 0, false), (1, 2, false)))
        .unwrap();
    assert!(!second.cards[PlayerId::P1].won);
    assert_eq!(second.players[PlayerId::P1].life, 9);
    assert_eq!(second.players[PlayerId::P2].life, 7);
    diag.unmake(undo_second);
    diag.unmake(undo_first);
    assert_eq!(diag.position(), &start);

    // The owner being knocked out does not spare the target: on 3 Life, P1 latches in
    // round 1 (P2 to 12 - 3 - 1 = 8), loses round 2 to a KO, and P2 still drops to 7, as
    // Fridlia Cr's Toxin does to Miyo's owner in 1091585/2.
    let mut spec = permanent_spec(1840, TOXIN, 12);
    spec.base_rules.players[PlayerId::P1].initial_life = 3;
    let mut diag = game(spec.base_rules, spec.cards);
    diag.make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    let (second, _) = diag
        .make(input(PlayerId::P2, (1, 0, false), (1, 2, false)))
        .unwrap();
    assert_eq!(second.players[PlayerId::P1].life, 0);
    assert_eq!(second.players[PlayerId::P2].life, 7);
    assert_eq!(second.status, MatchStatus::Won(PlayerId::P2));
}

#[test]
fn toxin_can_end_the_match_after_the_rounds_own_effects_as_in_capture_963039() {
    // Uuber's Victory-or-Defeat reduction lands first and takes the opponent from 2 to 1;
    // the Toxin latched a round earlier then takes the last point, as Regan's reduction
    // and Zis' Toxin do in 963039/2. Toxin first would have left them on 1.
    let mut spec = permanent_spec(1508, TOXIN, 12);
    spec.base_rules.players[PlayerId::P2].initial_life = 6;
    spec.cards[PlayerId::P1][1].ability = execute(
        1628,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ReduceOpponentLifeOnVictoryOrDefeat {
            life: 1,
            minimum: 1,
        },
    );
    let mut diag = game(spec.base_rules, spec.cards);
    let (first, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert_eq!(first.players[PlayerId::P2].life, 6 - 3 - 1);
    let (second, _) = diag
        .make(input(PlayerId::P2, (1, 0, false), (1, 2, false)))
        .unwrap();
    assert!(!second.cards[PlayerId::P1].won);
    assert_eq!(second.players[PlayerId::P2].life, 0);
    assert_eq!(second.status, MatchStatus::Won(PlayerId::P1));
    assert_eq!(second.players[PlayerId::P1].life, 9);
}

#[test]
fn poison_waits_a_round_stops_at_its_minimum_and_stacks_from_the_bonus_slot() {
    // A Freaks-style bonus Poison: latch round pays nothing, later rounds pay 2, never
    // below Min 3, and a second latched Poison is its own payment.
    let mut base = base_spec(6, 3);
    base.players[PlayerId::P2].initial_life = 12;
    let mut cards = plans(&base);
    for slot in 0..2 {
        cards[PlayerId::P1][slot].bonus = execute(206, CombatStatPredicateV1::Always, POISON);
        cards[PlayerId::P1][slot].source_bonus_support_count = 1;
    }
    let mut diag = game(base, cards);
    let (first, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(first.cards[PlayerId::P1].won);
    assert_eq!(first.players[PlayerId::P2].life, 9);
    // Round 2: a second Poison latches; only the first pays: 9 - 3 - 2 = 4.
    let (second, _) = diag
        .make(input(PlayerId::P1, (1, 2, false), (1, 0, false)))
        .unwrap();
    assert!(second.cards[PlayerId::P1].won);
    assert_eq!(second.players[PlayerId::P2].life, 4);
    assert_eq!(diag.position().latched[PlayerId::P1].len(), 2);
    // Round 3: P1 loses; both Poisons pay, the first to the Min 3 and the second nothing,
    // as the two Freaks latches report 2 and 0 in 926420/3.
    let (third, _) = diag
        .make(input(PlayerId::P2, (2, 0, false), (2, 2, false)))
        .unwrap();
    assert!(!third.cards[PlayerId::P1].won);
    assert_eq!(third.players[PlayerId::P2].life, 3);
    // Round 4: at the Min already, nothing moves; a target at 2 under Min 3 would also
    // stay put (1060510/2).
    let (fourth, _) = diag
        .make(input(PlayerId::P2, (3, 0, false), (3, 2, false)))
        .unwrap();
    assert_eq!(fourth.players[PlayerId::P2].life, 3);
}

#[test]
fn poison_pays_in_the_round_its_owner_is_knocked_out_as_in_capture_1092294() {
    // Araaknat's `Poison 2, Min 2` latches on 3 Life; the latch round pays nothing, and the
    // round that knocks its owner out still pays the two.
    let mut spec = permanent_spec(
        1385,
        CombatStatEffectV1::PoisonOpponentLifeOnVictory {
            life: 2,
            minimum: 2,
        },
        12,
    );
    spec.base_rules.players[PlayerId::P1].initial_life = 3;
    let mut diag_ko = game(spec.base_rules, spec.cards);
    let (first, _) = diag_ko
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert_eq!(first.players[PlayerId::P2].life, 9);
    let (second, _) = diag_ko
        .make(input(PlayerId::P2, (1, 0, false), (1, 2, false)))
        .unwrap();
    assert_eq!(second.players[PlayerId::P1].life, 0);
    assert_eq!(second.players[PlayerId::P2].life, 7);
    assert_eq!(second.status, MatchStatus::Won(PlayerId::P2));
}

#[test]
fn regen_pays_in_its_latching_round_and_is_capped_as_in_capture_1059149() {
    // Padre Frollo on 5 wins: Regen 3 pays at once but stops at Max 6; a later round at
    // the cap pays nothing; a player above the cap is left alone.
    let mut spec = permanent_spec(1458, REGEN, 5);
    spec.base_rules.players[PlayerId::P2].initial_life = 20;
    let mut diag = game(spec.base_rules, spec.cards);
    let (first, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(first.cards[PlayerId::P1].won);
    assert_eq!(first.players[PlayerId::P1].life, 6);
    let (second, _) = diag
        .make(input(PlayerId::P1, (1, 2, false), (1, 0, false)))
        .unwrap();
    assert_eq!(second.players[PlayerId::P1].life, 6);
    // Losing 3 from 6 leaves 3, and the repeat brings it straight back to the cap.
    let (third, _) = diag
        .make(input(PlayerId::P2, (2, 0, false), (2, 2, false)))
        .unwrap();
    assert_eq!(third.players[PlayerId::P1].life, 6);

    let spec = permanent_spec(1458, REGEN, 12);
    let mut diag = game(spec.base_rules, spec.cards);
    let (first, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert_eq!(first.players[PlayerId::P1].life, 12);
}

#[test]
fn stopped_or_losing_permanents_never_latch_and_plans_are_slot_and_magnitude_locked() {
    // A bonus Poison stopped by Stop Opp. Bonus in its winning round never latches.
    let mut base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].bonus = execute(206, CombatStatPredicateV1::Always, POISON);
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    cards[PlayerId::P2][0].ability = execute(
        130,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentBonus,
    );
    base.players[PlayerId::P2].initial_life = 12;
    let mut diag = game(base, cards);
    let (first, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(first.cards[PlayerId::P1].won);
    assert!(diag.position().latched[PlayerId::P1].is_empty());
    let (second, _) = diag
        .make(input(PlayerId::P1, (1, 2, false), (1, 0, false)))
        .unwrap();
    assert_eq!(second.players[PlayerId::P2].life, 12 - 3 - 3);

    // A losing Toxin latches nothing and pays nothing, in its round or later.
    let spec = permanent_spec(1197, TOXIN, 12);
    let mut diag = game(spec.base_rules, spec.cards);
    let (first, _) = diag
        .make(input(PlayerId::P2, (0, 0, false), (0, 2, false)))
        .unwrap();
    assert!(!first.cards[PlayerId::P1].won);
    assert_eq!(first.players[PlayerId::P2].life, 12);
    assert!(diag.position().latched[PlayerId::P1].is_empty());

    // Regen and Toxin are abilities only; Poison may be a bonus; magnitudes are positive
    // and Regen's cap exceeds its magnitude; none carries a condition of its own.
    for (id, effect) in [(1458, REGEN), (1197, TOXIN)] {
        let mut bonus = permanent_spec(id, effect, 12);
        bonus.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
        bonus.cards[PlayerId::P1][0].bonus = execute(id, CombatStatPredicateV1::Always, effect);
        bonus.cards[PlayerId::P1][0].source_bonus_support_count = 1;
        assert!(matches!(
            CombatStatDiagnosticV1::new(bonus),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::PermanentLifeSource,
                source: CombatStatEffectSourceV1::Bonus,
                ..
            })
        ));
    }
    for effect in [
        CombatStatEffectV1::RegenLifeOnVictory {
            life: 0,
            maximum: 6,
        },
        CombatStatEffectV1::RegenLifeOnVictory {
            life: 6,
            maximum: 6,
        },
        CombatStatEffectV1::PoisonOpponentLifeOnVictory {
            life: 0,
            minimum: 3,
        },
        CombatStatEffectV1::ToxinOpponentLifeOnVictory {
            life: 0,
            minimum: 0,
        },
    ] {
        assert!(matches!(
            CombatStatDiagnosticV1::new(permanent_spec(1, effect, 12)),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::PermanentLifeMagnitude,
                ..
            })
        ));
    }
    for effect in [REGEN, POISON, TOXIN] {
        let mut conditional = permanent_spec(1, effect, 12);
        conditional.cards[PlayerId::P1][0].ability =
            execute(1, CombatStatPredicateV1::OwnerMovesFirst, effect);
        assert!(matches!(
            CombatStatDiagnosticV1::new(conditional),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::PermanentLifePredicate,
                ..
            })
        ));
    }
}

const ARCHIMEDES: CardKey = CardKey { id: 1325, level: 5 };
const VICTORY_PILLZ: CombatStatEffectV1 = CombatStatEffectV1::GainPillzOnVictory { pillz: 2 };

/// P1 holds Archimedes in slot 0 with his `+2 Pillz`; every other source is absent. Both
/// hands are 6/3, so the higher bet wins and a tie goes to the first mover.
fn archimedes_spec() -> CombatStatDiagnosticMatchSpecV1 {
    let mut base = base_spec(6, 3);
    base.players[PlayerId::P1].hand[0].key = ARCHIMEDES;
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(1150, CombatStatPredicateV1::Always, VICTORY_PILLZ);
    CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    }
}

#[test]
fn victory_pillz_plan_is_ability_only_positive_and_carries_only_confidence() {
    let mut bonus = archimedes_spec();
    bonus.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    bonus.cards[PlayerId::P1][0].bonus =
        execute(1150, CombatStatPredicateV1::Always, VICTORY_PILLZ);
    bonus.cards[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(bonus),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::VictoryPillzSource,
            ..
        })
    ));

    let mut zero = archimedes_spec();
    zero.cards[PlayerId::P1][0].ability = execute(
        1150,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::GainPillzOnVictory { pillz: 0 },
    );
    assert!(matches!(
        CombatStatDiagnosticV1::new(zero),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::VictoryPillzMagnitude,
            ..
        })
    ));

    // Revision 36 admits one predicate beside `Always`: the previous-round win that
    // `Confidence:` prints. Every other condition is still refused, so a `Revenge:` or a
    // hand-slot form cannot borrow the grammar.
    let mut confidence = archimedes_spec();
    confidence.cards[PlayerId::P1][0].ability = execute(
        1702,
        CombatStatPredicateV1::OwnerWonPreviousRound,
        VICTORY_PILLZ,
    );
    assert!(CombatStatDiagnosticV1::new(confidence).is_ok());

    for predicate in [
        CombatStatPredicateV1::OwnerLostPreviousRound,
        CombatStatPredicateV1::SelectedHandSlotsDiffer,
        CombatStatPredicateV1::SelectedHandSlotsMatch,
        CombatStatPredicateV1::OwnerMovesFirst,
        CombatStatPredicateV1::OwnerMovesSecond,
    ] {
        let mut conditional = archimedes_spec();
        conditional.cards[PlayerId::P1][0].ability = execute(1150, predicate, VICTORY_PILLZ);
        assert!(
            matches!(
                CombatStatDiagnosticV1::new(conditional),
                Err(CombatStatPlanErrorV1::InvalidExecute {
                    reason: InvalidCombatStatPlanReasonV1::VictoryPillzPredicate,
                    ..
                })
            ),
            "{predicate:?}",
        );
    }
}

#[test]
fn victory_pillz_pays_the_winner_after_the_bet_and_unmakes_exactly() {
    let spec = archimedes_spec();
    let mut diag = game(spec.base_rules, spec.cards);
    let start = diag.position().clone();
    let start_hash = position_hash(&start);

    // Winning with a one-Pillz bet: 20 - 1 + 2, as Archimedes reaches 13 from 12 with a
    // one-Pillz bet in capture 1093275/0. The opponent's Pillz are untouched.
    let (report, undo) = diag
        .make(input(PlayerId::P1, (0, 1, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].pillz, 21);
    assert_eq!(report.players[PlayerId::P2].pillz, 20);
    assert_eq!(report.players[PlayerId::P1].life, 20);
    assert_eq!(report.players[PlayerId::P2].life, 17);
    diag.unmake(undo);
    assert_eq!(diag.position(), &start);
    assert_eq!(position_hash(diag.position()), start_hash);

    // Losing pays nothing, as Archimedes in 1092992/0 and Zaveli in 1060510/1.
    let (report, _) = diag
        .make(input(PlayerId::P2, (0, 0, false), (0, 2, false)))
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].pillz, 20);
    assert_eq!(report.players[PlayerId::P2].pillz, 18);
}

#[test]
fn victory_pillz_still_pays_into_a_ko_but_not_when_stopped_or_after_an_overflow() {
    // Knocking the opponent out does not suppress the winner's own gain: capture
    // 1092454/3 has Archimedes at 7 - 5 (a Fury bet of two) + 2 + 1 (VOD) = 5 while his
    // opponent falls to zero.
    let mut spec = archimedes_spec();
    spec.base_rules.players[PlayerId::P2].initial_life = 3;
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 2, true), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 0);
    assert_eq!(report.players[PlayerId::P1].pillz, 20 - 5 + 2);
    assert_eq!(report.status, MatchStatus::Won(PlayerId::P1));

    // Stopped by the opposing selected card: the win is not enough, the source has to be
    // live, as Markus' Roots bonus shows in 1092066/0.
    let mut spec = archimedes_spec();
    spec.cards[PlayerId::P2][0].ability = execute(
        4437,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].pillz, 18);

    // An overflow is atomic: nothing of the round is committed.
    let mut spec = archimedes_spec();
    spec.base_rules.players[PlayerId::P1].initial_pillz = u16::MAX;
    spec.base_rules.players[PlayerId::P1].hand[0].power = 40;
    let mut diag = game(spec.base_rules, spec.cards);
    let before = diag.position().clone();
    assert!(matches!(
        diag.make(input(PlayerId::P1, (0, 0, false), (0, 0, false))),
        Err(CombatStatDiagnosticErrorV1::BaseRules(
            BaseRulesError::PillzIncreaseOverflow {
                player: PlayerId::P1
            }
        ))
    ));
    assert_eq!(diag.position(), &before);
}

const DALHIA: CardKey = CardKey { id: 519, level: 5 };
const OPPONENT_PILLZ: CombatStatEffectV1 = CombatStatEffectV1::ReduceOpponentPillzOnVictory {
    pillz: 3,
    minimum: 4,
};

/// P1 holds Dalhia Cr in slot 0 with her `-3 Opp Pillz. Min 4`; every other source is
/// absent and both hands are 6/3.
fn dalhia_spec(p2_pillz: u16) -> CombatStatDiagnosticMatchSpecV1 {
    let mut base = base_spec(6, 3);
    base.players[PlayerId::P1].hand[0].key = DALHIA;
    base.players[PlayerId::P2].initial_pillz = p2_pillz;
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(339, CombatStatPredicateV1::Always, OPPONENT_PILLZ);
    CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    }
}

#[test]
fn opposing_victory_pillz_plan_is_ability_only_positive_and_unconditional() {
    let mut bonus = dalhia_spec(20);
    bonus.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    bonus.cards[PlayerId::P1][0].bonus =
        execute(339, CombatStatPredicateV1::Always, OPPONENT_PILLZ);
    bonus.cards[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(bonus),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::VictoryOpponentPillzSource,
            ..
        })
    ));

    let mut zero = dalhia_spec(20);
    zero.cards[PlayerId::P1][0].ability = execute(
        339,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ReduceOpponentPillzOnVictory {
            pillz: 0,
            minimum: 4,
        },
    );
    assert!(matches!(
        CombatStatDiagnosticV1::new(zero),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::VictoryOpponentPillzMagnitude,
            ..
        })
    ));

    let mut conditional = dalhia_spec(20);
    conditional.cards[PlayerId::P1][0].ability = execute(
        339,
        CombatStatPredicateV1::SelectedHandSlotsMatch,
        OPPONENT_PILLZ,
    );
    assert!(matches!(
        CombatStatDiagnosticV1::new(conditional),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::VictoryOpponentPillzPredicate,
            ..
        })
    ));
}

#[test]
fn opposing_victory_pillz_reads_the_target_after_its_bet_clamps_at_the_floor_and_unmakes() {
    // Above the floor: the target's 20 - 2 for its own bet become 15.
    let spec = dalhia_spec(20);
    let mut diag = game(spec.base_rules, spec.cards);
    let start = diag.position().clone();
    let start_hash = position_hash(&start);
    let (report, undo) = diag
        .make(input(PlayerId::P1, (0, 3, false), (0, 2, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].pillz, 17);
    assert_eq!(report.players[PlayerId::P2].pillz, 15);
    diag.unmake(undo);
    assert_eq!(diag.position(), &start);
    assert_eq!(position_hash(diag.position()), start_hash);

    // Reaching the floor exactly, as Callie's 12 - 5 - 3 = 4 in capture 1131294/0.
    let spec = dalhia_spec(12);
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 6, false), (0, 5, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].pillz, 4);

    // Crossing the floor stops on it.
    let spec = dalhia_spec(6);
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 1, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].pillz, 4);

    // A target already at or below the floor is left alone rather than pulled up to it,
    // as AI-Lycs stays on his recovered 4 in 1091644/1.
    for p2_pillz in [4, 2] {
        let spec = dalhia_spec(p2_pillz);
        let mut diag = game(spec.base_rules, spec.cards);
        let (report, _) = diag
            .make(input(PlayerId::P1, (0, 1, false), (0, 0, false)))
            .unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P2].pillz, p2_pillz);
    }
}

#[test]
fn opposing_victory_pillz_takes_nothing_on_a_loss_or_when_stopped() {
    // Losing the round: Yomi Ld in 924413/0, Gil Cr in 956608/0.
    let spec = dalhia_spec(20);
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P2, (0, 0, false), (0, 2, false)))
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].pillz, 18);

    // Stopped by the opposing selected card.
    let mut spec = dalhia_spec(20);
    spec.cards[PlayerId::P2][0].ability = execute(
        4437,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].pillz, 20);
}

const RAMAK: CardKey = CardKey { id: 1988, level: 4 };
const PILLZ_PER_DAMAGE: CombatStatEffectV1 =
    CombatStatEffectV1::GainPillzEqualToFinalDamageOnVictory;

/// P1 holds Ramak in slot 0 with `Symmetry: +1 Pillz Per Damage` (or, with `symmetry`
/// false, a plain `+1 Pillz Per Damage`); every other source is absent and both hands are 6/3.
fn ramak_spec(symmetry: bool) -> CombatStatDiagnosticMatchSpecV1 {
    let mut base = base_spec(6, 3);
    base.players[PlayerId::P1].hand[0].key = RAMAK;
    let mut cards = plans(&base);
    let (source_id, predicate) = if symmetry {
        (1852, CombatStatPredicateV1::SelectedHandSlotsMatch)
    } else {
        (1090, CombatStatPredicateV1::Always)
    };
    cards[PlayerId::P1][0].ability = execute(source_id, predicate, PILLZ_PER_DAMAGE);
    CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    }
}

#[test]
fn pillz_per_damage_plan_is_ability_only_and_unconditional_or_symmetry() {
    let mut bonus = ramak_spec(false);
    bonus.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    bonus.cards[PlayerId::P1][0].bonus =
        execute(1090, CombatStatPredicateV1::Always, PILLZ_PER_DAMAGE);
    bonus.cards[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(bonus),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::VictoryPillzPerDamageSource,
            ..
        })
    ));

    for predicate in [
        CombatStatPredicateV1::SelectedHandSlotsDiffer,
        CombatStatPredicateV1::OwnerWonPreviousRound,
        CombatStatPredicateV1::OwnerMovesFirst,
    ] {
        let mut conditional = ramak_spec(false);
        conditional.cards[PlayerId::P1][0].ability = execute(1090, predicate, PILLZ_PER_DAMAGE);
        assert!(matches!(
            CombatStatDiagnosticV1::new(conditional),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::VictoryPillzPerDamagePredicate,
                ..
            })
        ));
    }
}

#[test]
fn pillz_per_damage_pays_the_winner_its_final_damage_and_unmakes_exactly() {
    // Plain form, asymmetric slots, Fury: 20 - 5 for the bet, + 5 final Damage, as Spade's
    // 12 - 10 + 5 = 7 in capture 1024592/0.
    let spec = ramak_spec(false);
    let mut diag = game(spec.base_rules, spec.cards);
    let start = diag.position().clone();
    let start_hash = position_hash(&start);
    let (report, undo) = diag
        .make(input(PlayerId::P1, (0, 2, true), (1, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.cards[PlayerId::P1].damage, 5);
    assert_eq!(report.players[PlayerId::P2].life, 15);
    assert_eq!(report.players[PlayerId::P1].pillz, 20 - 5 + 5);
    diag.unmake(undo);
    assert_eq!(diag.position(), &start);
    assert_eq!(position_hash(diag.position()), start_hash);

    // An opposing Damage reduction lowers the payment with the Damage.
    let mut spec = ramak_spec(false);
    spec.cards[PlayerId::P2][1].ability = execute(
        4210,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::Damage, 2, 1),
    );
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (1, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.cards[PlayerId::P1].damage, 1);
    assert_eq!(report.players[PlayerId::P1].pillz, 20 - 2 + 1);

    // Losing pays nothing (Grace in 1089933/0), and a stopped source pays nothing.
    let spec = ramak_spec(false);
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P2, (0, 0, false), (1, 2, false)))
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].pillz, 20);
    let mut spec = ramak_spec(false);
    spec.cards[PlayerId::P2][1].ability = execute(
        4437,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (1, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].pillz, 18);
}

#[test]
fn symmetry_pillz_per_damage_pays_only_when_both_selected_slots_match() {
    // Slot 0 against slot 0: 20 - 3 + 3, as Ramak's 12 - 5 + 4 = 11 in capture 1023274/1.
    let spec = ramak_spec(true);
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 3, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].pillz, 20 - 3 + 3);

    // Slot 0 against slot 2: the win is not enough, as in 1011183/3 and 1025102/1.
    let spec = ramak_spec(true);
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 3, false), (2, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].pillz, 17);
}

/// P1 slot 0 carries one post-round ability plan; every other source is absent and both
/// hands are 6/3.
fn single_ability_spec(
    key: CardKey,
    source_id: u32,
    predicate: CombatStatPredicateV1,
    effect: CombatStatEffectV1,
) -> CombatStatDiagnosticMatchSpecV1 {
    let mut base = base_spec(6, 3);
    base.players[PlayerId::P1].hand[0].key = key;
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(source_id, predicate, effect);
    CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    }
}

const LIFE_PER_DAMAGE_TWO: CombatStatEffectV1 =
    CombatStatEffectV1::GainLifePerFinalDamageOnVictory {
        life_per_damage: 2,
        maximum: 0,
    };

#[test]
fn life_per_damage_plan_is_ability_only_positive_and_plain_or_previous_round() {
    let key = CardKey::new(358, 4);
    let mut bonus =
        single_ability_spec(key, 189, CombatStatPredicateV1::Always, LIFE_PER_DAMAGE_TWO);
    bonus.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    bonus.cards[PlayerId::P1][0].bonus =
        execute(189, CombatStatPredicateV1::Always, LIFE_PER_DAMAGE_TWO);
    bonus.cards[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(bonus),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::VictoryLifePerDamageSource,
            ..
        })
    ));
    assert!(matches!(
        CombatStatDiagnosticV1::new(single_ability_spec(
            key,
            189,
            CombatStatPredicateV1::Always,
            CombatStatEffectV1::GainLifePerFinalDamageOnVictory {
                life_per_damage: 0,
                maximum: 0,
            },
        )),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::VictoryLifePerDamageMagnitude,
            ..
        })
    ));
    for predicate in [
        CombatStatPredicateV1::OwnerMovesFirst,
        CombatStatPredicateV1::SelectedHandSlotsMatch,
    ] {
        assert!(matches!(
            CombatStatDiagnosticV1::new(single_ability_spec(
                key,
                189,
                predicate,
                LIFE_PER_DAMAGE_TWO
            )),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::VictoryLifePerDamagePredicate,
                ..
            })
        ));
    }
}

#[test]
fn capped_life_per_damage_stops_at_its_maximum_and_pays_nothing_at_or_past_it() {
    // C Dusk's `1146` is the same conversion under a ceiling: 1130609/3 took 5 to exactly 8
    // with a Fury-inclusive 6 Damage, and 1131010/2 took 7 to 8 with a plain 4. Here the
    // owner starts at 20 with a 5-Damage Fury hit, so the uncapped +2 would reach 30.
    for (maximum, life) in [(24, 24), (30, 30), (34, 30), (20, 20), (18, 20)] {
        let spec = single_ability_spec(
            CardKey::new(1320, 3),
            1146,
            CombatStatPredicateV1::Always,
            CombatStatEffectV1::GainLifePerFinalDamageOnVictory {
                life_per_damage: 2,
                maximum,
            },
        );
        let mut diag = game(spec.base_rules, spec.cards);
        let start = diag.position().clone();
        let (report, undo) = diag
            .make(input(PlayerId::P1, (0, 2, true), (0, 0, false)))
            .unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(report.cards[PlayerId::P1].damage, 5);
        assert_eq!(report.players[PlayerId::P1].life, life, "Max. {maximum}");
        diag.unmake(undo);
        assert_eq!(diag.position(), &start);
    }

    // A cap has only ever been printed without a previous-round prefix, so the plan
    // validator refuses the two together.
    for predicate in [
        CombatStatPredicateV1::OwnerLostPreviousRound,
        CombatStatPredicateV1::OwnerWonPreviousRound,
    ] {
        assert!(matches!(
            CombatStatDiagnosticV1::new(single_ability_spec(
                CardKey::new(1320, 3),
                1146,
                predicate,
                CombatStatEffectV1::GainLifePerFinalDamageOnVictory {
                    life_per_damage: 2,
                    maximum: 24,
                },
            )),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::VictoryLifePerDamagePredicate,
                ..
            })
        ));
    }
}

#[test]
fn prefixed_victory_life_waits_for_the_predicate_its_prefix_names() {
    // Impudicus' `2638` pays its 3 only when the two selected hand slots differ, which is
    // how it paid in 1010898/0 (his slot 0 against Aneta's slot 1).
    let asymmetry = || {
        single_ability_spec(
            CardKey::new(2209, 3),
            2638,
            CombatStatPredicateV1::SelectedHandSlotsDiffer,
            CombatStatEffectV1::GainLifeOnVictory { life: 3 },
        )
    };
    let spec = asymmetry();
    let mut diag = game(spec.base_rules, spec.cards);
    let (differ, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (1, 0, false)))
        .unwrap();
    assert!(differ.cards[PlayerId::P1].won);
    assert_eq!(differ.players[PlayerId::P1].life, 23);

    let spec = asymmetry();
    let mut diag = game(spec.base_rules, spec.cards);
    let (matching, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(matching.cards[PlayerId::P1].won);
    assert_eq!(matching.players[PlayerId::P1].life, 20);

    // Barcius' `3546` is the Confidence form: nothing in a first round, the 3 in a round
    // after its owner won one. 925868/3 is the server's paying case.
    let confidence = || {
        single_ability_spec(
            CardKey::new(2386, 3),
            3546,
            CombatStatPredicateV1::OwnerWonPreviousRound,
            CombatStatEffectV1::GainLifeOnVictory { life: 3 },
        )
    };
    let spec = confidence();
    let mut diag = game(spec.base_rules, spec.cards);
    let (first, _) = diag
        .make(input(PlayerId::P1, (1, 2, false), (1, 0, false)))
        .unwrap();
    assert!(first.cards[PlayerId::P1].won);
    assert_eq!(first.players[PlayerId::P1].life, 20);
    let (second, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(second.cards[PlayerId::P1].won);
    assert_eq!(second.players[PlayerId::P1].life, 23);

    let spec = confidence();
    let mut diag = game(spec.base_rules, spec.cards);
    diag.make(input(PlayerId::P2, (1, 0, false), (1, 2, false)))
        .unwrap();
    let (after_loss, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(after_loss.cards[PlayerId::P1].won);
    assert_eq!(after_loss.players[PlayerId::P1].life, 17);
}

#[test]
fn confidence_victory_pillz_waits_for_a_round_its_own_side_won() {
    // Balixto's `1702`. The server pins the paying case once, in 924615/2: his side won
    // round 1, he wins round 2 on a bet of 5, and 7 - 5 + 4 = 6. Everything else this test
    // pins is a corpus round the gate cannot reach - 1092515/2 mismatches in its own round
    // 0 and 925781/1 carries `Tune Out`. 1073010/1 opened on a deferred `Brawl:` source
    // until semantic revision 40 admitted it; that draw is eligible now and is a candidate
    // gate fixture, but it is not one yet, so the engine still pins this arm.
    let confidence = || {
        single_ability_spec(
            CardKey::new(1707, 2),
            1702,
            CombatStatPredicateV1::OwnerWonPreviousRound,
            CombatStatEffectV1::GainPillzOnVictory { pillz: 4 },
        )
    };

    // A first round has no previous round to have won, so winning it pays nothing.
    let spec = confidence();
    let mut diag = game(spec.base_rules, spec.cards);
    let (first, _) = diag
        .make(input(PlayerId::P1, (1, 2, false), (1, 0, false)))
        .unwrap();
    assert!(first.cards[PlayerId::P1].won);
    assert_eq!(first.players[PlayerId::P1].pillz, 18); // 20 - 2, nothing added
                                                       // The round after one its own side won does pay.
    let (second, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(second.cards[PlayerId::P1].won);
    assert_eq!(second.players[PlayerId::P1].pillz, 20); // 18 - 2 + 4

    // Losing the previous round withholds it even from a won current round.
    let spec = confidence();
    let mut diag = game(spec.base_rules, spec.cards);
    diag.make(input(PlayerId::P2, (1, 0, false), (1, 2, false)))
        .unwrap();
    let (after_loss, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(after_loss.cards[PlayerId::P1].won);
    assert_eq!(after_loss.players[PlayerId::P1].pillz, 18); // 20 - 2, nothing added

    // And winning the previous round is not enough on its own: the current round still has
    // to be won, which is the outcome channel the plain grammar already carried.
    let spec = confidence();
    let mut diag = game(spec.base_rules, spec.cards);
    diag.make(input(PlayerId::P1, (1, 2, false), (1, 0, false)))
        .unwrap();
    let (lost_current, _) = diag
        .make(input(PlayerId::P2, (0, 0, false), (0, 2, false)))
        .unwrap();
    assert!(!lost_current.cards[PlayerId::P1].won);
    assert_eq!(lost_current.players[PlayerId::P1].pillz, 18); // 18 - 0, nothing added
}

#[test]
fn life_per_damage_pays_n_per_final_damage_point_and_revenge_waits_for_a_loss() {
    // +2 per point of a 5-Damage Fury hit: 20 + 10, while the opponent takes the 5. The
    // opponent's knockout in capture 1089830/2 changed nothing either.
    let spec = single_ability_spec(
        CardKey::new(358, 4),
        189,
        CombatStatPredicateV1::Always,
        LIFE_PER_DAMAGE_TWO,
    );
    let mut diag = game(spec.base_rules, spec.cards);
    let start = diag.position().clone();
    let (report, undo) = diag
        .make(input(PlayerId::P1, (0, 2, true), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.cards[PlayerId::P1].damage, 5);
    assert_eq!(report.players[PlayerId::P1].life, 30);
    assert_eq!(report.players[PlayerId::P2].life, 15);
    diag.unmake(undo);
    assert_eq!(diag.position(), &start);

    // A loss pays nothing, as Kenny Cr in 1093399/2.
    let spec = single_ability_spec(
        CardKey::new(358, 4),
        189,
        CombatStatPredicateV1::Always,
        LIFE_PER_DAMAGE_TWO,
    );
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P2, (0, 0, false), (0, 2, false)))
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].life, 17);

    // Revenge: La Garra's +1 pays only in a round after her owner lost one. Round 1 is a
    // win with no previous round, so nothing; round 2 follows a loss and pays the 3.
    let spec = single_ability_spec(
        CardKey::new(1817, 3),
        1661,
        CombatStatPredicateV1::OwnerLostPreviousRound,
        CombatStatEffectV1::GainLifePerFinalDamageOnVictory {
            life_per_damage: 1,
            maximum: 0,
        },
    );
    let mut diag = game(spec.base_rules, spec.cards);
    let (first, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(first.cards[PlayerId::P1].won);
    assert_eq!(first.players[PlayerId::P1].life, 20);
    let (second, _) = diag
        .make(input(PlayerId::P2, (1, 0, false), (1, 2, false)))
        .unwrap();
    assert!(!second.cards[PlayerId::P1].won);
    assert_eq!(second.players[PlayerId::P1].life, 17);
    let spec = single_ability_spec(
        CardKey::new(1817, 3),
        1661,
        CombatStatPredicateV1::OwnerLostPreviousRound,
        CombatStatEffectV1::GainLifePerFinalDamageOnVictory {
            life_per_damage: 1,
            maximum: 0,
        },
    );
    let mut diag = game(spec.base_rules, spec.cards);
    diag.make(input(PlayerId::P2, (1, 0, false), (1, 2, false)))
        .unwrap();
    let (revenge, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(revenge.cards[PlayerId::P1].won);
    assert_eq!(revenge.players[PlayerId::P1].life, 17 + 3);
}

#[test]
fn prefixed_permanents_latch_only_when_their_predicate_holds_in_the_latching_round() {
    // Demusa's Symmetry Toxin: slot 0 against slot 0 latches and pays at once; against slot
    // 1 the win latches nothing and no later round pays.
    let toxin = CombatStatEffectV1::ToxinOpponentLifeOnVictory {
        life: 3,
        minimum: 0,
    };
    let spec = single_ability_spec(
        CardKey::new(2620, 2),
        5092,
        CombatStatPredicateV1::SelectedHandSlotsMatch,
        toxin,
    );
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 20 - 3 - 3);
    assert_eq!(diag.position().latched[PlayerId::P1].len(), 1);
    let spec = single_ability_spec(
        CardKey::new(2620, 2),
        5092,
        CombatStatPredicateV1::SelectedHandSlotsMatch,
        toxin,
    );
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (1, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 17);
    assert!(diag.position().latched[PlayerId::P1].is_empty());
    let (later, _) = diag
        .make(input(PlayerId::P2, (1, 0, false), (0, 2, false)))
        .unwrap();
    assert_eq!(later.players[PlayerId::P2].life, 17);

    // Becos Pill's Revenge Poison latches only in a winning round after a loss, then waits
    // a round before paying like every Poison.
    let poison = CombatStatEffectV1::PoisonOpponentLifeOnVictory {
        life: 2,
        minimum: 0,
    };
    let spec = single_ability_spec(
        CardKey::new(2335, 2),
        3301,
        CombatStatPredicateV1::OwnerLostPreviousRound,
        poison,
    );
    let mut diag = game(spec.base_rules, spec.cards);
    diag.make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(diag.position().latched[PlayerId::P1].is_empty());
    let spec = single_ability_spec(
        CardKey::new(2335, 2),
        3301,
        CombatStatPredicateV1::OwnerLostPreviousRound,
        poison,
    );
    let mut diag = game(spec.base_rules, spec.cards);
    diag.make(input(PlayerId::P2, (1, 0, false), (1, 2, false)))
        .unwrap();
    let (latch, _) = diag
        .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    assert!(latch.cards[PlayerId::P1].won);
    assert_eq!(latch.players[PlayerId::P2].life, 17);
    assert_eq!(diag.position().latched[PlayerId::P1].len(), 1);
    let (pays, _) = diag
        .make(input(PlayerId::P1, (2, 2, false), (2, 0, false)))
        .unwrap();
    assert_eq!(pays.players[PlayerId::P2].life, 17 - 3 - 2);

    // The plan validator admits exactly those predicates for a permanent.
    for predicate in [
        CombatStatPredicateV1::OwnerMovesFirst,
        CombatStatPredicateV1::OwnerMovesSecond,
    ] {
        assert!(matches!(
            CombatStatDiagnosticV1::new(single_ability_spec(
                CardKey::new(2620, 2),
                5092,
                predicate,
                toxin
            )),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::PermanentLifePredicate,
                ..
            })
        ));
    }
    assert!(matches!(
        CombatStatDiagnosticV1::new(single_ability_spec(
            CardKey::new(2683, 2),
            5692,
            CombatStatPredicateV1::OwnerMovesFirst,
            CombatStatEffectV1::HealLifeOnVictory {
                life: 1,
                maximum: 16
            }
        )),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::HealLifePredicate,
            ..
        })
    ));
}

fn anita_spec(power: u16, damage: u16) -> CombatStatDiagnosticMatchSpecV1 {
    let mut base = base_spec(power, damage);
    base.players[PlayerId::P1].hand[0].key = CardKey::new(448, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        274,
        CombatStatPredicateV1::OwnerMovesFirst,
        CombatStatEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
    );
    CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    }
}

#[test]
fn anita_courage_damage_life_plan_is_exactly_identity_card_effect_and_predicate_locked() {
    assert!(CombatStatDiagnosticV1::new(anita_spec(7, 3)).is_ok());

    let mut wrong_card = anita_spec(7, 3);
    wrong_card.base_rules.players[PlayerId::P1].hand[0].key = CardKey::new(449, 3);
    wrong_card.cards[PlayerId::P1][0].key = CardKey::new(449, 3);
    assert!(matches!(
        CombatStatDiagnosticV1::new(wrong_card),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::AnitaCourageDamageToLifeCard,
            ..
        })
    ));

    let mut wrong_id = anita_spec(7, 3);
    wrong_id.cards[PlayerId::P1][0].ability = execute(
        275,
        CombatStatPredicateV1::OwnerMovesFirst,
        CombatStatEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
    );
    assert!(matches!(
        CombatStatDiagnosticV1::new(wrong_id),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::AnitaCourageDamageToLifeIdentity,
            ..
        })
    ));

    let mut wrong_effect = anita_spec(7, 3);
    wrong_effect.cards[PlayerId::P1][0].ability = execute(
        274,
        CombatStatPredicateV1::OwnerMovesFirst,
        CombatStatEffectV1::GainLifeOnVictory { life: 1 },
    );
    assert!(matches!(
        CombatStatDiagnosticV1::new(wrong_effect),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::AnitaCourageDamageToLifeEffect,
            ..
        })
    ));

    let mut wrong_predicate = anita_spec(7, 3);
    wrong_predicate.cards[PlayerId::P1][0].ability = execute(
        274,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
    );
    assert!(matches!(
        CombatStatDiagnosticV1::new(wrong_predicate),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::AnitaCourageDamageToLifePredicate,
            ..
        })
    ));
}

#[test]
fn anita_courage_damage_life_gains_final_damage_and_unmakes_exactly() {
    let mut game = CombatStatDiagnosticV1::new(anita_spec(7, 3)).unwrap();
    let before = game.position().clone();
    let before_hash = position_hash(&before);
    let (report, undo) = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.cards[PlayerId::P1].damage, 3);
    assert_eq!(report.players[PlayerId::P1].life, 23);
    assert_eq!(report.players[PlayerId::P2].life, 17);
    game.unmake(undo);
    assert_eq!(game.position(), &before);
    assert_eq!(position_hash(game.position()), before_hash);
}

#[test]
fn anita_courage_damage_life_uses_damage_after_reduction_then_fury() {
    let mut spec = anita_spec(7, 3);
    // Capture 1065557: Anita's base 3 is reduced to Min 2, then Fury makes the
    // resolved damage 4 and she gains exactly 4 Life. This is deliberately not the
    // transport `damageAfter` field guarded by capture 875375.
    spec.cards[PlayerId::P2][0].ability = execute(
        7_001,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::Damage, 2, 2),
    );
    let mut game = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 3, true), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].damage, 4);
    assert_eq!(report.players[PlayerId::P1].life, 24);
    assert_eq!(report.players[PlayerId::P2].life, 16);
}

#[test]
fn anita_courage_damage_life_is_suppressed_when_second_losing_or_stopped() {
    let mut second_spec = anita_spec(8, 3);
    second_spec.base_rules.players[PlayerId::P2].hand[0].power = 7;
    let mut second = CombatStatDiagnosticV1::new(second_spec).unwrap();
    let (report, _) = second
        .make(input(PlayerId::P2, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].life, 20);

    let mut losing_spec = anita_spec(4, 3);
    losing_spec.base_rules.players[PlayerId::P2].hand[0].power = 7;
    let mut losing = CombatStatDiagnosticV1::new(losing_spec).unwrap();
    let (report, _) = losing
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].life, 17);

    let mut stopped_spec = anita_spec(7, 3);
    stopped_spec.cards[PlayerId::P2][0].ability = execute(
        41,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    let mut stopped = CombatStatDiagnosticV1::new(stopped_spec).unwrap();
    let (report, _) = stopped
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].life, 20);
}

#[test]
fn anita_courage_damage_life_overflow_is_atomic() {
    let mut spec = anita_spec(7, 3);
    spec.base_rules.players[PlayerId::P1].initial_life = u16::MAX - 1;
    let mut game = CombatStatDiagnosticV1::new(spec).unwrap();
    let before = game.position().clone();
    let before_hash = position_hash(&before);
    assert_eq!(
        game.make(input(PlayerId::P1, (0, 0, false), (0, 0, false))),
        Err(CombatStatDiagnosticErrorV1::BaseRules(
            BaseRulesError::LifeIncreaseOverflow {
                player: PlayerId::P1
            }
        ))
    );
    assert_eq!(game.position(), &before);
    assert_eq!(position_hash(game.position()), before_hash);
}

#[test]
fn vod_life_public_plans_are_exact_registry_effect_and_predicate_locks() {
    let own_one = CombatStatEffectV1::GainLifeOnVictoryOrDefeat { life: 1 };
    let own_two = CombatStatEffectV1::GainLifeOnVictoryOrDefeat { life: 2 };
    let uuber = CombatStatEffectV1::ReduceOpponentLifeOnVictoryOrDefeat {
        life: 1,
        minimum: 1,
    };
    for (key, source_id, effect) in [
        (CardKey::new(1586, 2), 1396, own_one),
        (CardKey::new(1586, 3), 1396, own_one),
        (CardKey::new(1586, 4), 1396, own_one),
        (CardKey::new(1676, 2), 2944, own_two),
        (CardKey::new(820, 3), 5835, own_one),
        (CardKey::new(820, 4), 2992, own_one),
        (CardKey::new(2693, 2), 5799, own_one),
        (CardKey::new(2693, 4), 5799, own_one),
        (CardKey::new(2693, 5), 5802, own_two),
        (CardKey::new(1788, 2), 1628, uuber),
    ] {
        assert!(CombatStatDiagnosticV1::new(vod_spec(key, source_id, effect)).is_ok());
    }

    // The compact engine is also fed post-Copy capture results. It deliberately admits
    // either source kind and a non-canonical owner key; catalog construction supplies the
    // stricter printed-card boundary.
    let mut copied_bonus = vod_spec(CardKey::new(9_999, 1), 1396, own_one);
    copied_bonus.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    copied_bonus.cards[PlayerId::P1][0].bonus =
        execute(1396, CombatStatPredicateV1::Always, own_one);
    copied_bonus.cards[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(CombatStatDiagnosticV1::new(copied_bonus).is_ok());

    assert!(matches!(
        CombatStatDiagnosticV1::new(vod_spec(CardKey::new(9_999, 1), 999_1396, own_one)),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::VictoryOrDefeatLifeIdentity,
            ..
        })
    ));

    for effect in [
        own_two,
        CombatStatEffectV1::ReduceOpponentLifeOnVictoryOrDefeat {
            life: 1,
            minimum: 0,
        },
    ] {
        assert!(matches!(
            CombatStatDiagnosticV1::new(vod_spec(CardKey::new(1586, 2), 1396, effect)),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::VictoryOrDefeatLifeEffect,
                ..
            })
        ));
    }

    let mut conditional = vod_spec(CardKey::new(1586, 2), 1396, own_one);
    conditional.cards[PlayerId::P1][0].ability =
        execute(1396, CombatStatPredicateV1::OwnerWonPreviousRound, own_one);
    assert!(matches!(
        CombatStatDiagnosticV1::new(conditional),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::VictoryOrDefeatLifePredicate,
            ..
        })
    ));
}

#[test]
fn vod_own_life_handles_both_outcomes_koa_stop_and_undo() {
    let effect = CombatStatEffectV1::GainLifeOnVictoryOrDefeat { life: 1 };
    let mut losing = vod_spec(CardKey::new(1586, 2), 1396, effect);
    losing.base_rules.players[PlayerId::P1].initial_life = 7;
    losing.base_rules.players[PlayerId::P2].hand[0].power = 7;
    let mut losing = CombatStatDiagnosticV1::new(losing).unwrap();
    let before = losing.position().clone();
    let before_hash = position_hash(&before);
    let (report, undo) = losing
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].life, 5); // 7 - 3 + 1
    losing.unmake(undo);
    assert_eq!(losing.position(), &before);
    assert_eq!(position_hash(losing.position()), before_hash);

    let mut winning = vod_spec(CardKey::new(1586, 2), 1396, effect);
    winning.base_rules.players[PlayerId::P1].initial_life = 7;
    winning.base_rules.players[PlayerId::P1].hand[0].power = 40;
    let (report, _) = CombatStatDiagnosticV1::new(winning)
        .unwrap()
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].life, 8);

    let mut ko = vod_spec(CardKey::new(1586, 2), 1396, effect);
    ko.base_rules.players[PlayerId::P1].initial_life = 3;
    ko.base_rules.players[PlayerId::P2].hand[0].power = 7;
    let (report, _) = CombatStatDiagnosticV1::new(ko)
        .unwrap()
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].life, 0);
    assert_eq!(report.status, MatchStatus::Won(PlayerId::P2));

    let mut stopped = vod_spec(CardKey::new(1586, 2), 1396, effect);
    stopped.base_rules.players[PlayerId::P1].initial_life = 7;
    stopped.base_rules.players[PlayerId::P1].hand[0].power = 40;
    stopped.cards[PlayerId::P2][0].ability = execute(
        1,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    let (report, _) = CombatStatDiagnosticV1::new(stopped)
        .unwrap()
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].life, 7);

    let mut sob = vod_spec(CardKey::new(1586, 2), 1396, effect);
    sob.base_rules.players[PlayerId::P1].initial_life = 7;
    sob.base_rules.players[PlayerId::P1].hand[0].power = 40;
    sob.cards[PlayerId::P2][0].ability = execute(
        1,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentBonus,
    );
    let (report, _) = CombatStatDiagnosticV1::new(sob)
        .unwrap()
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].life, 8);
}

#[test]
fn vod_opponent_life_clamps_minimum_and_owner_ko_still_applies() {
    let effect = CombatStatEffectV1::ReduceOpponentLifeOnVictoryOrDefeat {
        life: 1,
        minimum: 1,
    };
    let mut minimum = vod_spec(CardKey::new(1788, 2), 1628, effect);
    minimum.base_rules.players[PlayerId::P1].initial_life = 7;
    minimum.base_rules.players[PlayerId::P2].initial_life = 2;
    minimum.base_rules.players[PlayerId::P2].hand[0].power = 7;
    let (report, _) = CombatStatDiagnosticV1::new(minimum)
        .unwrap()
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].life, 4);
    assert_eq!(report.players[PlayerId::P2].life, 1);

    let mut owner_ko = vod_spec(CardKey::new(1788, 2), 1628, effect);
    owner_ko.base_rules.players[PlayerId::P1].initial_life = 3;
    owner_ko.base_rules.players[PlayerId::P2].hand[0].power = 7;
    let (report, _) = CombatStatDiagnosticV1::new(owner_ko)
        .unwrap()
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].life, 0);
    assert_eq!(report.players[PlayerId::P2].life, 19);

    let mut target_ko = vod_spec(CardKey::new(1788, 2), 1628, effect);
    target_ko.base_rules.players[PlayerId::P1].hand[0].power = 40;
    target_ko.base_rules.players[PlayerId::P2].initial_life = 3;
    let (report, _) = CombatStatDiagnosticV1::new(target_ko)
        .unwrap()
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 0);

    // A copied VOD result can occupy the Bonus slot. SOB suppresses it, while SOA does
    // not; this is the same liveness split used by ordinary post-round sources.
    let mut copied_bonus = vod_spec(CardKey::new(9_999, 1), 1628, effect);
    copied_bonus.base_rules.players[PlayerId::P1].hand[0].damage = 0;
    copied_bonus.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    copied_bonus.cards[PlayerId::P1][0].bonus =
        execute(1628, CombatStatPredicateV1::Always, effect);
    copied_bonus.cards[PlayerId::P1][0].source_bonus_support_count = 1;
    copied_bonus.cards[PlayerId::P2][0].ability = execute(
        1,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentBonus,
    );
    let (report, _) = CombatStatDiagnosticV1::new(copied_bonus)
        .unwrap()
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 20);

    let mut copied_bonus = vod_spec(CardKey::new(9_999, 1), 1628, effect);
    copied_bonus.base_rules.players[PlayerId::P1].hand[0].damage = 0;
    copied_bonus.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    copied_bonus.cards[PlayerId::P1][0].bonus =
        execute(1628, CombatStatPredicateV1::Always, effect);
    copied_bonus.cards[PlayerId::P1][0].source_bonus_support_count = 1;
    copied_bonus.cards[PlayerId::P2][0].ability = execute(
        1,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    let (report, _) = CombatStatDiagnosticV1::new(copied_bonus)
        .unwrap()
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 19);
}

#[test]
fn equalizer_opponent_life_public_plans_are_exact_and_copy_can_use_either_source() {
    let equalizer = CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars {
        per_star: 1,
        minimum: 2,
    };
    for source_id in [1415, 4458] {
        assert!(CombatStatDiagnosticV1::new(vod_spec(
            CardKey::new(9_999, 1),
            source_id,
            equalizer,
        ))
        .is_ok());
    }

    let mut copied_bonus = vod_spec(CardKey::new(9_999, 1), 1415, equalizer);
    copied_bonus.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    copied_bonus.cards[PlayerId::P1][0].bonus =
        execute(1415, CombatStatPredicateV1::Always, equalizer);
    copied_bonus.cards[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(CombatStatDiagnosticV1::new(copied_bonus).is_ok());

    for effect in [
        CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars {
            per_star: 2,
            minimum: 2,
        },
        CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars {
            per_star: 1,
            minimum: 1,
        },
    ] {
        assert!(matches!(
            CombatStatDiagnosticV1::new(vod_spec(CardKey::new(9_999, 1), 1415, effect)),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::EqualizerOpponentLifeEffect,
                ..
            })
        ));
    }
    let mut conditional = vod_spec(CardKey::new(9_999, 1), 1415, equalizer);
    conditional.cards[PlayerId::P1][0].ability = execute(
        1415,
        CombatStatPredicateV1::OwnerWonPreviousRound,
        equalizer,
    );
    assert!(matches!(
        CombatStatDiagnosticV1::new(conditional),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::EqualizerOpponentLifePredicate,
            ..
        })
    ));
    assert!(matches!(
        CombatStatDiagnosticV1::new(vod_spec(CardKey::new(9_999, 1), 9_1415, equalizer)),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::EqualizerOpponentLifeIdentity,
            ..
        })
    ));
}

#[test]
fn equalizer_binds_revealed_stars_after_stop_and_make_unmake_is_exact() {
    let equalizer = CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars {
        per_star: 1,
        minimum: 2,
    };
    let mut scaled = vod_spec(CardKey::new(9_999, 1), 1415, equalizer);
    scaled.base_rules.players[PlayerId::P1].hand[0].power = 40;
    scaled.base_rules.players[PlayerId::P2].hand[0].key = CardKey::new(200, 5);
    scaled.cards[PlayerId::P2][0].key = CardKey::new(200, 5);
    let mut scaled = CombatStatDiagnosticV1::new(scaled).unwrap();
    let before = scaled.position().clone();
    let before_hash = position_hash(&before);
    let (report, undo) = scaled
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 12); // 20 - 3 damage - (1 * 5 stars)
    scaled.unmake(undo);
    assert_eq!(scaled.position(), &before);
    assert_eq!(position_hash(scaled.position()), before_hash);

    let mut minimum = vod_spec(CardKey::new(9_999, 1), 1415, equalizer);
    minimum.base_rules.players[PlayerId::P1].hand[0].power = 40;
    minimum.base_rules.players[PlayerId::P2].hand[0].key = CardKey::new(200, 5);
    minimum.cards[PlayerId::P2][0].key = CardKey::new(200, 5);
    minimum.base_rules.players[PlayerId::P2].initial_life = 6;
    let (report, _) = CombatStatDiagnosticV1::new(minimum)
        .unwrap()
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 2);

    // At commit time the target is already below or at Min: the effect must not raise
    // post-damage Life 0, 1, or 2. The preceding case pins the only allowed boundary
    // transition, post-damage 3 -> 2.
    for (initial_life, expected_life) in [(4, 1), (5, 2)] {
        let mut already_capped = vod_spec(CardKey::new(9_999, 1), 1415, equalizer);
        already_capped.base_rules.players[PlayerId::P1].hand[0].power = 40;
        already_capped.base_rules.players[PlayerId::P2].initial_life = initial_life;
        let (report, _) = CombatStatDiagnosticV1::new(already_capped)
            .unwrap()
            .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
            .unwrap();
        assert_eq!(report.players[PlayerId::P2].life, expected_life);
    }
    let mut target_ko = vod_spec(CardKey::new(9_999, 1), 1415, equalizer);
    target_ko.base_rules.players[PlayerId::P1].hand[0].power = 40;
    target_ko.base_rules.players[PlayerId::P2].initial_life = 3;
    let (report, _) = CombatStatDiagnosticV1::new(target_ko)
        .unwrap()
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 0);

    let mut losing = vod_spec(CardKey::new(9_999, 1), 1415, equalizer);
    losing.base_rules.players[PlayerId::P2].hand[0].power = 7;
    let (report, _) = CombatStatDiagnosticV1::new(losing)
        .unwrap()
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 20);

    let mut stopped = vod_spec(CardKey::new(9_999, 1), 1415, equalizer);
    stopped.base_rules.players[PlayerId::P1].hand[0].power = 40;
    stopped.base_rules.players[PlayerId::P2].hand[0].key = CardKey::new(200, 5);
    stopped.cards[PlayerId::P2][0].key = CardKey::new(200, 5);
    stopped.cards[PlayerId::P2][0].ability = execute(
        1,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    let (report, _) = CombatStatDiagnosticV1::new(stopped)
        .unwrap()
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 17);

    let mut copied_bonus = vod_spec(CardKey::new(9_999, 1), 1415, equalizer);
    copied_bonus.base_rules.players[PlayerId::P1].hand[0].power = 40;
    copied_bonus.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    copied_bonus.cards[PlayerId::P1][0].bonus =
        execute(1415, CombatStatPredicateV1::Always, equalizer);
    copied_bonus.cards[PlayerId::P1][0].source_bonus_support_count = 1;
    copied_bonus.cards[PlayerId::P2][0].ability = execute(
        1,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentBonus,
    );
    let (report, _) = CombatStatDiagnosticV1::new(copied_bonus)
        .unwrap()
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 17);
}

#[test]
fn vod_own_life_overflow_is_atomic() {
    let effect = CombatStatEffectV1::GainLifeOnVictoryOrDefeat { life: 2 };
    let mut overflow = vod_spec(CardKey::new(1676, 2), 2944, effect);
    overflow.base_rules.players[PlayerId::P1].initial_life = u16::MAX;
    overflow.base_rules.players[PlayerId::P1].hand[0].power = 40;
    let mut overflow = CombatStatDiagnosticV1::new(overflow).unwrap();
    let before = overflow.position().clone();
    assert!(matches!(
        overflow.make(input(PlayerId::P1, (0, 0, false), (0, 0, false))),
        Err(CombatStatDiagnosticErrorV1::BaseRules(
            BaseRulesError::LifeIncreaseOverflow {
                player: PlayerId::P1
            }
        ))
    ));
    assert_eq!(overflow.position(), &before);
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
        // Since revision 47 a Stop may carry the predicates the conditional-Stop grammar
        // admits; these are the ones it still may not.
        (
            execute(
                3,
                CombatStatPredicateV1::OwnerMovesSecond,
                CombatStatEffectV1::StopOpponentBonus,
            ),
            InvalidCombatStatPlanReasonV1::ConditionalControl,
        ),
        (
            execute(
                42,
                CombatStatPredicateV1::OwnerHandUnison,
                CombatStatEffectV1::StopOpponentAbility,
            ),
            InvalidCombatStatPlanReasonV1::ConditionalControl,
        ),
        (
            execute(
                4,
                CombatStatPredicateV1::MatchIsDay,
                CombatStatEffectV1::StopOpponentBonus,
            ),
            InvalidCombatStatPlanReasonV1::ConditionalControl,
        ),
        (
            execute(
                41,
                CombatStatPredicateV1::OwnerMovesFirst,
                CombatStatEffectV1::ProtectOwnAbility,
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

fn victory_opponent_life_spec(
    power: u16,
    damage: u16,
    opponent_life: u16,
) -> CombatStatDiagnosticMatchSpecV1 {
    let mut base = base_spec(power, damage);
    base.players[PlayerId::P2].initial_life = opponent_life;
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        1399,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ReduceOpponentLifeOnVictory {
            life: 5,
            minimum: 5,
        },
    );
    CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    }
}

#[test]
fn victory_opponent_life_plan_is_identity_magnitude_and_predicate_locked() {
    assert!(CombatStatDiagnosticV1::new(victory_opponent_life_spec(7, 3, 20)).is_ok());

    // The Berzerk Bonus identity carries its own magnitude and may not borrow Mou's.
    let mut berzerk = victory_opponent_life_spec(7, 3, 20);
    berzerk.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    berzerk.cards[PlayerId::P1][0].bonus = execute(
        680,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ReduceOpponentLifeOnVictory {
            life: 2,
            minimum: 2,
        },
    );
    berzerk.cards[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(CombatStatDiagnosticV1::new(berzerk).is_ok());

    // The Bonus identity keeps its own magnitude: it may not borrow a printed ability's.
    let mut swapped_magnitude = victory_opponent_life_spec(7, 3, 20);
    swapped_magnitude.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    swapped_magnitude.cards[PlayerId::P1][0].source_bonus_support_count = 1;
    swapped_magnitude.cards[PlayerId::P1][0].bonus = execute(
        680,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ReduceOpponentLifeOnVictory {
            life: 5,
            minimum: 5,
        },
    );
    assert!(matches!(
        CombatStatDiagnosticV1::new(swapped_magnitude),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::VictoryOpponentLifeMagnitude,
            ..
        })
    ));

    // Since revision 33 the unconditional reduction is a grammar, so any printed ability may
    // carry its own positive magnitude - but a zero one is still refused.
    let mut grammar = victory_opponent_life_spec(7, 3, 20);
    grammar.cards[PlayerId::P1][0].ability = execute(
        594,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ReduceOpponentLifeOnVictory {
            life: 3,
            minimum: 0,
        },
    );
    assert!(CombatStatDiagnosticV1::new(grammar).is_ok());

    let mut zero = victory_opponent_life_spec(7, 3, 20);
    zero.cards[PlayerId::P1][0].ability = execute(
        594,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ReduceOpponentLifeOnVictory {
            life: 0,
            minimum: 0,
        },
    );
    assert!(matches!(
        CombatStatDiagnosticV1::new(zero),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::VictoryOpponentLifeMagnitude,
            ..
        })
    ));

    // The grammar is a printed-ability grammar, and 680 is only ever the clan Bonus.
    let mut wrong_slot = victory_opponent_life_spec(7, 3, 20);
    wrong_slot.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    wrong_slot.cards[PlayerId::P1][0].source_bonus_support_count = 1;
    wrong_slot.cards[PlayerId::P1][0].bonus = execute(
        1399,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ReduceOpponentLifeOnVictory {
            life: 5,
            minimum: 5,
        },
    );
    assert!(matches!(
        CombatStatDiagnosticV1::new(wrong_slot),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::VictoryOpponentLifeIdentity,
            ..
        })
    ));

    // Growth `1730` is still refused however a caller labels it: it is a round-scaled
    // magnitude rather than a predicate, and it is not a reviewed identity at all.
    for unreviewed in [1730] {
        let mut foreign_id = victory_opponent_life_spec(7, 3, 20);
        foreign_id.cards[PlayerId::P1][0].ability = execute(
            unreviewed,
            CombatStatPredicateV1::Always,
            CombatStatEffectV1::ReduceOpponentLifeOnVictory {
                life: 3,
                minimum: 0,
            },
        );
        assert!(
            matches!(
                CombatStatDiagnosticV1::new(foreign_id),
                Err(CombatStatPlanErrorV1::InvalidExecute {
                    reason: InvalidCombatStatPlanReasonV1::VictoryOpponentLifeIdentity,
                    ..
                })
            ),
            "unreviewed {unreviewed}",
        );
    }

    // Courage `4533` became a reviewed identity in semantic revision 39, so a plan claiming
    // its magnitude under the wrong condition is now refused for the sharper reason: the
    // identity and the magnitude are its own, and only the predicate is wrong.
    let mut unconditional_courage = victory_opponent_life_spec(7, 3, 20);
    unconditional_courage.cards[PlayerId::P1][0].ability = execute(
        4533,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ReduceOpponentLifeOnVictory {
            life: 3,
            minimum: 0,
        },
    );
    assert!(matches!(
        CombatStatDiagnosticV1::new(unconditional_courage),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::VictoryOpponentLifePredicate,
            ..
        })
    ));

    // Every reviewed identity carries exactly one predicate, so a plan can neither add a
    // condition to an unconditional member nor swap in another condition for a conditional
    // one. Each reviewed id is checked against every predicate but its own.
    for (source_id, life, minimum, allowed) in [
        (1399, 5, 5, CombatStatPredicateV1::Always),
        (4708, 4, 0, CombatStatPredicateV1::SelectedHandSlotsMatch),
        (3016, 3, 0, CombatStatPredicateV1::OwnerWonPreviousRound),
        (4301, 3, 0, CombatStatPredicateV1::OwnerWonPreviousRound),
    ] {
        for predicate in [
            CombatStatPredicateV1::Always,
            CombatStatPredicateV1::OwnerMovesFirst,
            CombatStatPredicateV1::OwnerMovesSecond,
            CombatStatPredicateV1::OwnerWonPreviousRound,
            CombatStatPredicateV1::OwnerLostPreviousRound,
            CombatStatPredicateV1::SelectedHandSlotsMatch,
            CombatStatPredicateV1::SelectedHandSlotsDiffer,
        ] {
            let mut spec = victory_opponent_life_spec(7, 3, 20);
            spec.cards[PlayerId::P1][0].ability = execute(
                source_id,
                predicate,
                CombatStatEffectV1::ReduceOpponentLifeOnVictory { life, minimum },
            );
            let result = CombatStatDiagnosticV1::new(spec);
            if predicate == allowed {
                assert!(result.is_ok(), "{source_id} with {predicate:?}");
            } else {
                assert!(
                    matches!(
                        result,
                        Err(CombatStatPlanErrorV1::InvalidExecute {
                            reason: InvalidCombatStatPlanReasonV1::VictoryOpponentLifePredicate,
                            ..
                        })
                    ),
                    "{source_id} with {predicate:?}",
                );
            }
        }
    }
}

#[test]
fn a_conditional_victory_opponent_life_reduction_runs_only_when_its_condition_holds() {
    // Doela Noel's Symmetry reduction fires only when both players picked the same hand
    // slot. P2 starts on 20 Life and takes three printed damage either way.
    for (opponent_slot, expected) in [(0, 13), (1, 17)] {
        let mut spec = victory_opponent_life_spec(7, 3, 20);
        spec.cards[PlayerId::P1][0].ability = execute(
            4708,
            CombatStatPredicateV1::SelectedHandSlotsMatch,
            CombatStatEffectV1::ReduceOpponentLifeOnVictory {
                life: 4,
                minimum: 0,
            },
        );
        let mut game = CombatStatDiagnosticV1::new(spec).unwrap();
        let before = game.position().clone();
        let before_hash = position_hash(&before);
        let (report, undo) = game
            .make(input(
                PlayerId::P1,
                (0, 1, false),
                (opponent_slot, 0, false),
            ))
            .unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(
            report.players[PlayerId::P2].life,
            expected,
            "opponent slot {opponent_slot}",
        );
        game.unmake(undo);
        assert_eq!(game.position(), &before);
        assert_eq!(position_hash(game.position()), before_hash);
    }

    // Diabolus' Confidence reduction needs its own owner to have won the previous round, so
    // it can never fire in round zero.
    let spec = || {
        let mut spec = victory_opponent_life_spec(7, 3, 20);
        spec.cards[PlayerId::P1][0].ability = execute(
            3016,
            CombatStatPredicateV1::OwnerWonPreviousRound,
            CombatStatEffectV1::ReduceOpponentLifeOnVictory {
                life: 3,
                minimum: 0,
            },
        );
        spec
    };
    let mut game = CombatStatDiagnosticV1::new(spec()).unwrap();
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 1, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 17);

    let mut game = CombatStatDiagnosticV1::new(spec()).unwrap();
    game.make(input(PlayerId::P1, (1, 1, false), (1, 0, false)))
        .unwrap();
    assert_eq!(game.position().previous_round_winner, Some(PlayerId::P1));
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 1, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 20 - 3 - 3 - 3);
}

#[test]
fn victory_opponent_life_applies_after_damage_clamps_and_unmakes_exactly() {
    // An unclamped win takes the printed damage and then the complete five.
    let mut game = CombatStatDiagnosticV1::new(victory_opponent_life_spec(7, 2, 20)).unwrap();
    let before = game.position().clone();
    let before_hash = position_hash(&before);
    let (report, undo) = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 13);
    game.unmake(undo);
    assert_eq!(game.position(), &before);
    assert_eq!(position_hash(game.position()), before_hash);

    // Capture 1091473 round 0: 12 takes two damage and then the complete five, landing
    // exactly on the bound.
    let mut exact = CombatStatDiagnosticV1::new(victory_opponent_life_spec(7, 2, 12)).unwrap();
    let (report, _) = exact
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 5);

    // Capture 926367 round 1: post-damage life is above the bound but the complete five
    // would cross it, so the result clamps at Min 5 rather than going lower.
    let mut clamped = CombatStatDiagnosticV1::new(victory_opponent_life_spec(7, 2, 8)).unwrap();
    let (report, _) = clamped
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 5);

    // Capture 924257 round 3: a target already at or below the bound is left alone.
    let mut at_bound = CombatStatDiagnosticV1::new(victory_opponent_life_spec(7, 2, 5)).unwrap();
    let (report, _) = at_bound
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 3);

    // 1025102 keeps three consecutive defeats: a loss pays nothing at all.
    let mut losing_spec = victory_opponent_life_spec(4, 2, 20);
    losing_spec.base_rules.players[PlayerId::P2].hand[0].power = 7;
    let mut losing = CombatStatDiagnosticV1::new(losing_spec).unwrap();
    let (report, _) = losing
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 20);

    // Ordinary source liveness still applies.
    let mut stopped_spec = victory_opponent_life_spec(7, 2, 12);
    stopped_spec.cards[PlayerId::P2][0].ability = execute(
        41,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    let mut stopped = CombatStatDiagnosticV1::new(stopped_spec).unwrap();
    let (report, _) = stopped
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 10);
}

fn copy(source_id: u32, copied: CopiedSourceKindV1) -> CombatStatSourcePlanV1 {
    conditional_copy(source_id, copied, CombatStatPredicateV1::Always)
}

fn conditional_copy(
    source_id: u32,
    copied: CopiedSourceKindV1,
    predicate: CombatStatPredicateV1,
) -> CombatStatSourcePlanV1 {
    CombatStatSourcePlanV1::CopyOpponentSource {
        source_id,
        copied,
        predicate,
    }
}

/// P1 slot 0 copies the opposing card's named source. Every opposing card carries a
/// concrete plan, which is what makes the Copy admissible at all.
fn copy_spec(
    copied: CopiedSourceKindV1,
    opposing: CombatStatSourcePlanV1,
) -> CombatStatDiagnosticMatchSpecV1 {
    let base = base_spec(7, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = copy(846, copied);
    cards[PlayerId::P1][0].source_ability_support_count = 1;
    for slot in 0..4 {
        match copied {
            CopiedSourceKindV1::Ability => cards[PlayerId::P2][slot].ability = opposing,
            CopiedSourceKindV1::Bonus => {
                cards[PlayerId::P2][slot].bonus = opposing;
                cards[PlayerId::P2][slot].source_bonus_support_count = 1;
            }
        }
    }
    CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    }
}

#[test]
fn unconditional_copy_adopts_the_selected_opposing_source_and_unmakes_exactly() {
    // Copying an opposing Damage increase gives the copier that increase, not the opponent.
    let mut game = CombatStatDiagnosticV1::new(copy_spec(
        CopiedSourceKindV1::Bonus,
        execute(
            202,
            CombatStatPredicateV1::Always,
            own(CombatStatAttributeV1::Damage, 2),
        ),
    ))
    .unwrap();
    let before = game.position().clone();
    let before_hash = position_hash(&before);
    let (report, undo) = game
        .make(input(PlayerId::P1, (0, 1, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.cards[PlayerId::P1].damage, 5);
    game.unmake(undo);
    assert_eq!(game.position(), &before);
    assert_eq!(position_hash(game.position()), before_hash);
}

#[test]
fn unconditional_copy_takes_its_own_support_context_and_stays_stoppable() {
    // Capture 1078906 round 1: the copied Rescue Support counts the copier's own clan,
    // not the original owner's. Four clan-mates give Attack +12 over base 5 x 5.
    let base = {
        let mut base = base_spec(5, 3);
        for slot in 0..4 {
            base.players[PlayerId::P1].hand[slot].clan_id = 49;
        }
        base
    };
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = copy(846, CopiedSourceKindV1::Bonus);
    cards[PlayerId::P1][0].source_ability_support_count = 4;
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
        cards[PlayerId::P2][slot].source_bonus_support_count = 1;
    }
    let spec = CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    };
    let mut game = CombatStatDiagnosticV1::new(spec.clone()).unwrap();
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 4, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].attack, 25 + 12);

    // Capture 874590: the copied effect lives in the copier's own Ability slot, so an
    // opposing Stop Opp. Ability suppresses it entirely.
    let mut stopped_spec = spec;
    for slot in 0..4 {
        stopped_spec.cards[PlayerId::P2][slot].ability = execute(
            41,
            CombatStatPredicateV1::Always,
            CombatStatEffectV1::StopOpponentAbility,
        );
    }
    let mut stopped = CombatStatDiagnosticV1::new(stopped_spec).unwrap();
    let (report, _) = stopped
        .make(input(PlayerId::P1, (0, 4, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].attack, 25);
}

#[test]
fn copy_is_rejected_unless_every_opposing_target_is_already_concrete() {
    // A solver must be total, so one unresolvable opposing card closes the whole match.
    for hazard in [
        CombatStatSourcePlanV1::RejectIfSelected { source_id: 999 },
        CombatStatSourcePlanV1::Disabled { source_id: 999 },
        copy(846, CopiedSourceKindV1::Bonus),
    ] {
        let mut spec = copy_spec(
            CopiedSourceKindV1::Bonus,
            execute(
                202,
                CombatStatPredicateV1::Always,
                own(CombatStatAttributeV1::Damage, 2),
            ),
        );
        spec.cards[PlayerId::P2][2].bonus = hazard;
        spec.cards[PlayerId::P2][2].source_bonus_support_count = if matches!(
            hazard,
            CombatStatSourcePlanV1::Disabled { .. }
                | CombatStatSourcePlanV1::RejectIfSelected { .. }
                | CombatStatSourcePlanV1::CopyOpponentSource { .. }
        ) {
            1
        } else {
            0
        };
        assert!(matches!(
            CombatStatDiagnosticV1::new(spec),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::CopyOpponentSourceTarget,
                ..
            })
        ));
    }

    // An absent opposing source is concrete: the Copy simply adopts nothing.
    let mut game = CombatStatDiagnosticV1::new(copy_spec(
        CopiedSourceKindV1::Ability,
        CombatStatSourcePlanV1::Absent,
    ))
    .unwrap();
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].damage, 3);
}

#[test]
fn a_conditional_copy_adopts_only_when_its_own_condition_holds() {
    // Reprisal is about who moved, not who owns the copied source. P1 adopts an opposing
    // Damage +2 only in the round it replies second; base damage is 3.
    for (first_mover, expected) in [(PlayerId::P2, 5), (PlayerId::P1, 3)] {
        let mut spec = copy_spec(
            CopiedSourceKindV1::Bonus,
            execute(
                202,
                CombatStatPredicateV1::Always,
                own(CombatStatAttributeV1::Damage, 2),
            ),
        );
        spec.cards[PlayerId::P1][0].ability = conditional_copy(
            958,
            CopiedSourceKindV1::Bonus,
            CombatStatPredicateV1::OwnerMovesSecond,
        );
        let mut game = CombatStatDiagnosticV1::new(spec).unwrap();
        let before = game.position().clone();
        let before_hash = position_hash(&before);
        let (report, undo) = game
            .make(input(first_mover, (0, 1, false), (0, 0, false)))
            .unwrap();
        assert_eq!(
            report.cards[PlayerId::P1].damage,
            expected,
            "first mover {first_mover:?}",
        );
        game.unmake(undo);
        assert_eq!(game.position(), &before);
        assert_eq!(position_hash(game.position()), before_hash);
    }
}

#[test]
fn a_copied_effect_keeps_its_own_predicate_against_the_copier() {
    // Adopting a Confidence effect does not inherit the opponent's history: the copier has
    // to have won the previous round itself. Round zero has no previous winner at all, so
    // both conditions are false and the copier keeps its base damage of 3.
    let spec = || {
        let mut spec = copy_spec(
            CopiedSourceKindV1::Bonus,
            execute(
                202,
                CombatStatPredicateV1::OwnerWonPreviousRound,
                own(CombatStatAttributeV1::Damage, 2),
            ),
        );
        spec.cards[PlayerId::P1][0].ability = conditional_copy(
            958,
            CopiedSourceKindV1::Bonus,
            CombatStatPredicateV1::OwnerMovesSecond,
        );
        spec
    };
    let mut game = CombatStatDiagnosticV1::new(spec()).unwrap();
    let (report, _) = game
        .make(input(PlayerId::P2, (0, 1, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].damage, 3);

    // Once P1 has won a round, the same adopted Confidence does apply to P1.
    let mut game = CombatStatDiagnosticV1::new(spec()).unwrap();
    game.make(input(PlayerId::P2, (1, 1, false), (1, 0, false)))
        .unwrap();
    assert_eq!(game.position().previous_round_winner, Some(PlayerId::P1));
    let (report, _) = game
        .make(input(PlayerId::P2, (0, 1, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].damage, 5);
}

/// Battle 949439 r0 in miniature: Nebula's `Protection: Power And Damage` against an
/// opposing `-2 Opp Power, Min 5`. The server reported 7 Power, not 5.
#[test]
fn protection_refuses_an_opposing_reduction() {
    let base = base_spec(7, 4);
    let mut protected = plans(&base);
    protected[PlayerId::P1][0].ability = execute(
        1355,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ProtectOwnCombatStat {
            stat: CombatStatAttributeV1::PowerAndDamage,
        },
    );
    protected[PlayerId::P2][0].ability = execute(
        916,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::PowerAndDamage, 1, 3),
    );
    let mut with_protection = game(base.clone(), protected.clone());
    let (report, _) = with_protection
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 7);
    assert_eq!(report.cards[PlayerId::P1].damage, 4);

    // The same round without the Protection is the reduction the server did not report.
    let mut plans = protected;
    plans[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    let mut without_protection = game(base, plans);
    let (report, _) = without_protection
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 6);
    assert_eq!(report.cards[PlayerId::P1].damage, 3);
}

/// Protection defends against the opposing character only: the owner's own increase still
/// lands, and the protected card's own reduction still reaches an unprotected opponent.
#[test]
fn protection_leaves_own_increases_and_its_own_reduction_alone() {
    let base = base_spec(7, 4);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        1355,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ProtectOwnCombatStat {
            stat: CombatStatAttributeV1::PowerAndDamage,
        },
    );
    cards[PlayerId::P1][0].bonus = execute(
        43,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Power, 2),
    );
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    cards[PlayerId::P2][0].ability = execute(
        916,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::Power, 1, 3),
    );
    cards[PlayerId::P2][0].bonus = execute(
        1536,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Power, 1),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let mut game = game(base, cards);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 9);
    assert_eq!(report.cards[PlayerId::P2].power, 8);
}

/// A Protection whose own source was stopped protects nothing.
#[test]
fn a_stopped_protection_protects_nothing() {
    let base = base_spec(7, 4);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        1355,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ProtectOwnCombatStat {
            stat: CombatStatAttributeV1::PowerAndDamage,
        },
    );
    cards[PlayerId::P2][0].ability = execute(
        40,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    cards[PlayerId::P2][0].bonus = execute(
        916,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::Power, 2, 1),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let mut game = game(base, cards);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 5);
}

/// Battle 926525 r0: Lumia Cr stops Andy Ld's Ability, the Skeelz `Protection: Ability`
/// bonus keeps it alive, and its "-20 Opp Attack, Min 5" still takes Lumia Cr's own 36
/// Attack to the 16 the server reported.
#[test]
fn a_protected_ability_survives_an_opposing_stop() {
    let base = base_spec(6, 4);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        5462,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::Attack, 20, 5),
    );
    cards[PlayerId::P1][0].bonus = execute(
        461,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ProtectOwnAbility,
    );
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    cards[PlayerId::P2][0].ability = execute(
        40,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    let mut with_protection = game(base.clone(), cards.clone());
    let (report, _) = with_protection
        .make(input(PlayerId::P1, (0, 5, false), (0, 5, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].attack, 16);

    // Without the protecting bonus the Stop lands and the reduction never runs.
    let mut plans = cards;
    plans[PlayerId::P1][0].bonus = CombatStatSourcePlanV1::Absent;
    plans[PlayerId::P1][0].source_bonus_support_count = 0;
    let mut without_protection = game(base, plans);
    let (report, _) = without_protection
        .make(input(PlayerId::P1, (0, 5, false), (0, 5, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].attack, 36);
}

/// Protection restores a source after the Stop graph has run, which is where TypeScript
/// applies it too (PRE3 after PRE4). A source that comes back therefore keeps its combat
/// effect but has already missed its chance to stop anything.
#[test]
fn a_restored_source_does_not_fire_a_stop_of_its_own() {
    let base = base_spec(6, 4);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        1846,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentBonus,
    );
    cards[PlayerId::P1][0].bonus = execute(
        461,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ProtectOwnAbility,
    );
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    cards[PlayerId::P2][0].ability = execute(
        40,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    cards[PlayerId::P2][0].bonus = execute(
        43,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Power, 3),
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let mut game = game(base, cards);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].power, 9);
}

/// Battle 1025031 r0: Natasha copies Nantosuelte's *printed* 4 Damage, not the 7 its
/// Asymmetry bonus had made of it, and her own `Damage +2` then produces the reported 6.
#[test]
fn a_stat_copy_takes_the_printed_value_and_runs_before_own_increases() {
    let base = base_spec(8, 1);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        1513,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::CopyOpponentPrintedCombatStat {
            stat: CombatStatAttributeV1::Damage,
        },
    );
    cards[PlayerId::P1][0].bonus = execute(
        43,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Damage, 2),
    );
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    // The opposing card raises its own printed 1 Damage to 4; the copy must not see 4.
    cards[PlayerId::P2][0].ability = execute(
        1844,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Damage, 3),
    );
    let mut game = game(base, cards);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].damage, 4);
    // Copied printed 1, then its own +2. Reading the opponent's resolved 4 would give 6.
    assert_eq!(report.cards[PlayerId::P1].damage, 3);
}

/// Battle 1065812 r1: Joana copies Sue's printed 6 Power and Sue's own `-1 Opp Power And
/// Damage, Min 3` then leaves the reported 5, so the copy lands before opposing reductions.
#[test]
fn a_stat_copy_lands_before_an_opposing_reduction() {
    let base = base_spec(5, 5);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        421,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::CopyOpponentPrintedCombatStat {
            stat: CombatStatAttributeV1::Power,
        },
    );
    cards[PlayerId::P2][0].ability = execute(
        916,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::PowerAndDamage, 1, 3),
    );
    let mut game = game(base, cards);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 4);
}

/// A stopped Copy copies nothing, and the pair grammar writes both stats at once.
#[test]
fn a_stopped_stat_copy_copies_nothing_and_the_pair_writes_both() {
    let base = base_spec(6, 2);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        5525,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::CopyOpponentPrintedCombatStat {
            stat: CombatStatAttributeV1::PowerAndDamage,
        },
    );
    let mut pair = game(base.clone(), cards.clone());
    let (report, _) = pair
        .make(input(PlayerId::P1, (0, 0, false), (1, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 6);
    assert_eq!(report.cards[PlayerId::P1].damage, 2);

    let mut stopped = cards;
    stopped[PlayerId::P2][1].ability = execute(
        40,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    let mut game = game(base, stopped);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 0, false), (1, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 6);
    assert_eq!(report.cards[PlayerId::P1].damage, 2);
}

/// An Asymmetry Copy adopts only when the two selected hand slots differ. Every captured
/// Asymmetry Copy reached its capture already rewritten to the source it adopted, so the
/// negative branch rests on the structured `indexRequirement` this predicate already
/// serves elsewhere rather than on a round of its own.
#[test]
fn an_asymmetry_copy_adopts_only_on_differing_hand_slots() {
    for (p2_slot, expected) in [(1_u8, 5_u16), (0, 3)] {
        let mut spec = copy_spec(
            CopiedSourceKindV1::Bonus,
            execute(
                202,
                CombatStatPredicateV1::Always,
                own(CombatStatAttributeV1::Damage, 2),
            ),
        );
        spec.cards[PlayerId::P1][0].ability = conditional_copy(
            2482,
            CopiedSourceKindV1::Bonus,
            CombatStatPredicateV1::SelectedHandSlotsDiffer,
        );
        let mut game = CombatStatDiagnosticV1::new(spec).unwrap();
        let (report, _) = game
            .make(input(PlayerId::P1, (0, 0, false), (p2_slot, 0, false)))
            .unwrap();
        assert_eq!(
            report.cards[PlayerId::P1].damage,
            expected,
            "opposing slot {p2_slot}",
        );
    }
}

/// `+N Attack Per Opp. Damage` scales by the opposing card's Damage as the Attack phase
/// sees it: resolved, but before Fury. Battle 1130726 r3 is the one that separates the
/// two - Goran's +2 is worth 4 against a Fury Uuber, not 8.
#[test]
fn attack_per_opponent_damage_reads_the_damage_before_fury() {
    let base = base_spec(8, 2);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        4806,
        CombatStatPredicateV1::Always,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Attack,
            CombatStatOperationV1::Increase,
            2,
            None,
            None,
            CombatStatMagnitudeV1::OpponentDamage,
        ),
    );
    let mut plain = game(base.clone(), cards.clone());
    let (report, _) = plain
        .make(input(PlayerId::P1, (0, 3, false), (0, 3, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].attack, 36); // 8 x 4 + 2 x 2

    let mut furious = game(base.clone(), cards.clone());
    let (report, _) = furious
        .make(input(PlayerId::P1, (0, 3, false), (0, 3, true)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].damage, 4); // 2 printed + 2 Fury
    assert_eq!(report.cards[PlayerId::P1].attack, 36); // still 2 x 2, not 2 x 4

    // A reduction of the opposing Damage does count: it is resolved before this phase.
    let mut reduced = cards;
    reduced[PlayerId::P1][0].bonus = execute(
        916,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::Damage, 1, 0),
    );
    reduced[PlayerId::P1][0].source_bonus_support_count = 1;
    let mut game = game(base, reduced);
    let (report, _) = game
        .make(input(PlayerId::P1, (0, 3, false), (0, 3, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].damage, 1);
    assert_eq!(report.cards[PlayerId::P1].attack, 34); // 32 + 2 x 1
}

/// `Xantiax: -N Life, Min. M` charges both players whatever the round did, floors each of
/// them at the Min independently, and revives neither from it.
#[test]
fn both_players_life_reduction_charges_each_side_whatever_the_outcome() {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        1379,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ReduceBothPlayersLife {
            life: 3,
            minimum: 0,
        },
    );

    // A losing owner pays, and so does the winner who has just damaged it.
    let mut losing = game(base.clone(), cards.clone());
    let (report, _) = losing
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].won, false);
    assert_eq!(report.players[PlayerId::P1].life, 14); // 20 - 3 damage - 3
    assert_eq!(report.players[PlayerId::P2].life, 17); // 20 - 3

    // A winning owner pays exactly the same. This is the shape 1080464/2 pins.
    let mut winning = game(base.clone(), cards.clone());
    let (report, _) = winning
        .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].won, true);
    assert_eq!(report.players[PlayerId::P1].life, 17); // 20 - 3
    assert_eq!(report.players[PlayerId::P2].life, 14); // 20 - 3 damage - 3

    // Each side floors independently: an owner the round's damage has already taken to
    // zero is not charged again and is never revived by the clamp, while the opposing
    // player, sitting on two, is taken down to the Min rather than past it. 1058151/3 is
    // the captured half of this.
    let mut spec = CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    };
    spec.base_rules.players[PlayerId::P1].initial_life = 3;
    spec.base_rules.players[PlayerId::P2].initial_life = 2;
    let mut clamped = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = clamped
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].life, 0); // 3 - 3 damage, then nothing to take
    assert_eq!(report.players[PlayerId::P2].life, 0); // 2 - 3 floored at Min 0
}

/// The losing-side Pillz reduction carries the same plan guards as its Victory sibling:
/// the Ability slot only, a positive magnitude, and no predicate.
#[test]
fn opposing_defeat_pillz_plan_is_ability_only_positive_and_unconditional() {
    let base = base_spec(6, 3);
    let effect = CombatStatEffectV1::ReduceOpponentPillzOnDefeat {
        pillz: 2,
        minimum: 4,
    };
    let spec = |plan: CombatStatSourcePlanV1, bonus: bool| {
        let mut cards = plans(&base);
        if bonus {
            cards[PlayerId::P1][0].bonus = plan;
            cards[PlayerId::P1][0].source_bonus_support_count = 1;
        } else {
            cards[PlayerId::P1][0].ability = plan;
        }
        CombatStatDiagnosticMatchSpecV1 {
            base_rules: base.clone(),
            cards,
        }
    };

    assert!(matches!(
        CombatStatDiagnosticV1::new(spec(
            execute(912, CombatStatPredicateV1::Always, effect),
            true
        )),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::DefeatOpponentPillzSource,
            ..
        })
    ));
    assert!(matches!(
        CombatStatDiagnosticV1::new(spec(
            execute(
                912,
                CombatStatPredicateV1::Always,
                CombatStatEffectV1::ReduceOpponentPillzOnDefeat {
                    pillz: 0,
                    minimum: 4,
                },
            ),
            false
        )),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::DefeatOpponentPillzMagnitude,
            ..
        })
    ));
    assert!(matches!(
        CombatStatDiagnosticV1::new(spec(
            execute(912, CombatStatPredicateV1::SelectedHandSlotsMatch, effect),
            false
        )),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::DefeatOpponentPillzPredicate,
            ..
        })
    ));
}

/// `Defeat: -N Opp. Pillz, Min M` is the Victory reduction's arithmetic on the losing
/// side's trigger. Captures 1092515/0 and 1092201/2 pin the paying case away from the
/// floor; the floor, the winning case and the knocked-out owner are pinned here, the last
/// of those by composition with the reviewed opponent-Life sibling rather than by its own
/// observation - the corpus has no round where a knocked-out owner carries this ability.
#[test]
fn defeat_opponent_pillz_triggers_on_a_loss_and_respects_its_minimum() {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        912,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ReduceOpponentPillzOnDefeat {
            pillz: 2,
            minimum: 4,
        },
    );

    // Losing pays, and the target is read after its own bet: 20 - 5 - 2 = 13.
    let mut losing = game(base.clone(), cards.clone());
    let start = losing.position().clone();
    let (report, undo) = losing
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].pillz, 13);
    losing.unmake(undo);
    assert_eq!(losing.position(), &start);

    // Winning does not pay: the trigger is the loss, not the ability being selected.
    let mut winning = game(base.clone(), cards.clone());
    let (report, _) = winning
        .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].pillz, 20);

    // Crossing the floor stops on it, and a target already at or below it is left alone
    // rather than pulled up - the same clamp the Victory sibling carries. The last case is
    // the one the corpus holds (1088480/2): the target's own bet has already taken it below
    // the floor, so the ability finds nothing to take and the round distinguishes nothing.
    for (initial, bet, expected) in [(11, 5, 4), (6, 1, 4), (5, 1, 4), (4, 1, 3), (6, 5, 1)] {
        let mut spec = CombatStatDiagnosticMatchSpecV1 {
            base_rules: base.clone(),
            cards: cards.clone(),
        };
        spec.base_rules.players[PlayerId::P2].initial_pillz = initial;
        let mut clamped = CombatStatDiagnosticV1::new(spec).unwrap();
        let (report, _) = clamped
            .make(input(PlayerId::P1, (0, 0, false), (0, bet, false)))
            .unwrap();
        assert!(!report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P2].pillz, expected);
    }

    // An owner this round has knocked out still pays it, exactly as the reviewed
    // opponent-Life sibling does.
    let mut spec = CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    };
    spec.base_rules.players[PlayerId::P1].initial_life = 3;
    let mut knocked_out = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = knocked_out
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].life, 0);
    assert_eq!(report.players[PlayerId::P2].pillz, 13);
}

/// `Defeat: -N Opp. Life, Min M` pays out when its owner loses the round, leaves a target
/// already at or below the Min alone, and still pays after its owner is taken to zero.
#[test]
fn defeat_opponent_life_triggers_on_a_loss_and_respects_its_minimum() {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        959,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ReduceOpponentLifeOnDefeat {
            life: 2,
            minimum: 1,
        },
    );
    // Losing pays.
    let mut losing = game(base.clone(), cards.clone());
    let (report, _) = losing
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].won, false);
    assert_eq!(report.players[PlayerId::P2].life, 18);

    // Winning does not.
    let mut winning = game(base.clone(), cards.clone());
    let (report, _) = winning
        .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].won, true);
    assert_eq!(report.players[PlayerId::P1].life, 20);
    // Only the 3 combat damage it just dealt, with nothing added by the ability.
    assert_eq!(report.players[PlayerId::P2].life, 17);

    // A target at the Min keeps its life, and a KO'd owner still pays out.
    let mut spec = CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    };
    spec.base_rules.players[PlayerId::P1].initial_life = 3;
    spec.base_rules.players[PlayerId::P2].initial_life = 1;
    let mut clamped = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = clamped
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].life, 0);
    assert_eq!(report.players[PlayerId::P2].life, 1);
}

/// `Killshot: -N Opp. Life Min M` asks the attack ratio, not the winner. The corpus pins the
/// paying and the non-paying halves (1337321/1 against 1337230/0) but reaches neither the
/// exact-double boundary, the Min clamp, nor the case the reference makes possible and the
/// obvious `owner == winner && ratio` spelling would get wrong: equal attacks, where the
/// ratio holds for the side that did not win the round.
#[test]
fn killshot_opponent_life_triggers_on_the_attack_ratio_and_not_on_the_win() {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        1959,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ReduceOpponentLifeOnKillshot {
            life: 2,
            minimum: 1,
        },
    );

    // Doubling pays: 6 x 6 = 36 against 6 x 1 = 6. Three combat damage, then the two.
    let mut doubled = game(base.clone(), cards.clone());
    let (report, _) = doubled
        .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 15);

    // Exactly double is inside the boundary: 6 x 2 = 12 against 6 x 1 = 6.
    let mut exact = game(base.clone(), cards.clone());
    let (report, _) = exact
        .make(input(PlayerId::P1, (0, 1, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 15);

    // Winning without doubling pays nothing, which is battle 1337230 round 0: 6 x 3 = 18
    // against 6 x 2 = 12 wins the round but falls short of the 24 it needed.
    let mut short = game(base.clone(), cards.clone());
    let (report, _) = short
        .make(input(PlayerId::P1, (0, 2, false), (0, 1, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 17);

    // A target already at the Min is left alone rather than pulled up to it. Zero printed
    // damage keeps the combat ledger out of the way so the clamp is the only thing moving.
    let quiet = base_spec(6, 0);
    let mut quiet_cards = plans(&quiet);
    quiet_cards[PlayerId::P1][0].ability = cards[PlayerId::P1][0].ability.clone();
    let mut spec = CombatStatDiagnosticMatchSpecV1 {
        base_rules: quiet,
        cards: quiet_cards,
    };
    spec.base_rules.players[PlayerId::P2].initial_life = 2;
    let mut clamped = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = clamped
        .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 1);

    // The case that fixes the spelling of the guard. At zero attack on both sides the
    // ratio `a >= 2b` holds trivially, while `round_winner` still has to give the round to
    // somebody - and it does not give it to the Killshot's owner. The reference condition
    // (`Condition::Killshot`) carries no win requirement, unlike its `Backlash` neighbour,
    // so the owner pays out here even though it lost the round. Written as
    // `owner == winner && ratio` this round would silently pay nothing.
    let stalled = base_spec(0, 3);
    let mut stalled_cards = plans(&stalled);
    stalled_cards[PlayerId::P1][0].ability = cards[PlayerId::P1][0].ability.clone();
    let mut tied = game(stalled, stalled_cards);
    let (report, _) = tied
        .make(input(PlayerId::P2, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 18);
}

/// Courage on the Victory opponent-Life reduction composes two pieces the projection had
/// already pinned separately: the Min-clamped reduction itself, and `OwnerMovesFirst` on a
/// post-round plan. The corpus reaches the paying round (1091848/1) but not the round where
/// the owner wins having moved second, nor the Min clamp on this predicate, so both are
/// pinned here.
#[test]
fn courage_victory_opponent_life_needs_the_owner_to_move_first() {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        4533,
        CombatStatPredicateV1::OwnerMovesFirst,
        CombatStatEffectV1::ReduceOpponentLifeOnVictory {
            life: 3,
            minimum: 0,
        },
    );

    // Moving first and winning pays: three combat damage, then the three.
    let mut first = game(base.clone(), cards.clone());
    let (report, _) = first
        .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 14);

    // Winning having moved second pays nothing at all.
    let mut second = game(base.clone(), cards.clone());
    let (report, _) = second
        .make(input(PlayerId::P2, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 17);

    // Losing pays nothing either, however the owner moved.
    let mut lost = game(base, cards.clone());
    let (report, _) = lost
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 20);

    // Ligea's floored form leaves a target already at the Min alone rather than pulling it
    // up. Zero printed damage keeps the combat ledger out of the way.
    let quiet = base_spec(6, 0);
    let mut quiet_cards = plans(&quiet);
    quiet_cards[PlayerId::P1][0].ability = execute(
        4531,
        CombatStatPredicateV1::OwnerMovesFirst,
        CombatStatEffectV1::ReduceOpponentLifeOnVictory {
            life: 3,
            minimum: 1,
        },
    );
    let mut spec = CombatStatDiagnosticMatchSpecV1 {
        base_rules: quiet,
        cards: quiet_cards,
    };
    spec.base_rules.players[PlayerId::P2].initial_life = 1;
    let mut clamped = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = clamped
        .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 1);
}

/// Brawl is an anti-support magnitude: the effect is multiplied by the number of distinct
/// characters in the OPPOSING hand sharing the OPPOSING selected card's effective clan.
///
/// Every Brawl round in the corpus has a count of exactly 4, because every opposing hand in
/// it is mono-clan with four distinct characters. So the server pins the magnitude against
/// `Fixed` but never separates "distinct characters of the opposing selected card's clan"
/// from "opposing hand size", and never varies the count at all. Those are the arms pinned
/// here: counts of 1, 2 and 4, the dedup by character id, and the fact that the number is
/// read from the opposing hand rather than the owner's.
#[test]
fn brawl_scales_by_the_opposing_hands_clan_count() {
    // Power +1 under the Brawl magnitude, so the resolved Power reads the count directly.
    let brawl = modifier(
        CombatStatAffectedSideV1::Player,
        CombatStatAttributeV1::Power,
        CombatStatOperationV1::Increase,
        1,
        None,
        None,
        CombatStatMagnitudeV1::AntiSupport,
    );

    // `base_spec` gives every card its own clan, so the opposing count starts at 1.
    let one = base_spec(6, 3);
    let mut cards = plans(&one);
    cards[PlayerId::P1][0].ability = execute(1488, CombatStatPredicateV1::Always, brawl);
    let mut lone = game(one.clone(), cards.clone());
    let (report, _) = lone
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 7);

    // Two opposing cards sharing the selected card's clan doubles it, and the two that do
    // not share it contribute nothing.
    let mut pair = plans(&one);
    pair[PlayerId::P1][0].ability = execute(1488, CombatStatPredicateV1::Always, brawl);
    pair[PlayerId::P2][0].effective_clan_id = 900;
    pair[PlayerId::P2][1].effective_clan_id = 900;
    let mut two = game(one.clone(), pair);
    let (report, _) = two
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 8);

    // A mono-clan opposing hand of four distinct characters is the corpus's only case.
    let mut quad = plans(&one);
    quad[PlayerId::P1][0].ability = execute(1488, CombatStatPredicateV1::Always, brawl);
    for slot in 0..4 {
        quad[PlayerId::P2][slot].effective_clan_id = 900;
    }
    let mut four = game(one.clone(), quad.clone());
    let (report, _) = four
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 10);

    // The count is over the OPPOSING hand, so stacking the owner's own hand changes
    // nothing. This is what separates Brawl from Support.
    let mut mine = plans(&one);
    mine[PlayerId::P1][0].ability = execute(1488, CombatStatPredicateV1::Always, brawl);
    for slot in 0..4 {
        mine[PlayerId::P1][slot].effective_clan_id = 900;
    }
    let mut own_hand = game(one.clone(), mine);
    let (report, _) = own_hand
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 7);

    // Characters are counted distinctly: a duplicate character id in the opposing hand is
    // counted once, not twice, which no corpus round exercises.
    let mut duplicated = one.clone();
    duplicated.players[PlayerId::P2].hand[1] = duplicated.players[PlayerId::P2].hand[0];
    let mut dup_cards = plans(&duplicated);
    dup_cards[PlayerId::P1][0].ability = execute(1488, CombatStatPredicateV1::Always, brawl);
    for slot in 0..4 {
        dup_cards[PlayerId::P2][slot].effective_clan_id = 900;
    }
    let mut duplicate = game(duplicated, dup_cards);
    let (report, _) = duplicate
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 9);
}

/// `Defeat: Poison N, Min M` is the first admitted permanent whose trigger is not a win.
/// The corpus pins the paying side well - 1130484 latches on a lost round 0, then pays -1 in
/// rounds 1 and 2, the second of which the server names in `postRoundAbilities` with
/// `isPermanent: true` - and pins the negative side in 1066589/2, where the same card wins
/// and nothing latches. What it does not reach is the owner knocked out by the very round
/// that latches, nor the Min floor, so both are pinned here.
#[test]
fn defeat_poison_latches_on_a_loss_and_repeats_from_the_next_round() {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        4561,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::PoisonOpponentLifeOnDefeat {
            life: 1,
            minimum: 3,
        },
    );

    // Losing latches it, and Poison does not pay in its latching round.
    let mut losing = game(base.clone(), cards.clone());
    let (report, _) = losing
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 20);
    // The next round pays it, whoever wins that round.
    let (report, _) = losing
        .make(input(PlayerId::P1, (1, 5, false), (1, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 16);

    // Winning latches nothing at all, which is battle 1066589 round 2.
    let mut winning = game(base.clone(), cards.clone());
    let (report, _) = winning
        .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    let (report, _) = winning
        .make(input(PlayerId::P1, (1, 0, false), (1, 5, false)))
        .unwrap();
    // Only the 3 combat damage from the round it won, with nothing latched behind it.
    assert_eq!(report.players[PlayerId::P2].life, 17);

    // An owner knocked out by the round it loses still latches, and the permanent goes on
    // paying from the opposing side afterwards. The repeat loop's own guards decide that,
    // which is why the latch arm does not test the owner's life.
    let mut spec = CombatStatDiagnosticMatchSpecV1 {
        base_rules: base.clone(),
        cards: cards.clone(),
    };
    spec.base_rules.players[PlayerId::P1].initial_life = 3;
    let mut knocked_out = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = knocked_out
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].life, 0);
    assert_eq!(report.players[PlayerId::P2].life, 20);

    // A target already at the Min is left alone rather than pushed below it.
    let mut floored = CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    };
    floored.base_rules.players[PlayerId::P2].initial_life = 3;
    let mut clamped = CombatStatDiagnosticV1::new(floored).unwrap();
    clamped
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    let (report, _) = clamped
        .make(input(PlayerId::P1, (1, 0, false), (1, 5, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 3);
}

/// P1 holds a post-round `Brawl:` source in slot 0; every other source is absent, both hands
/// are 6/3 and each card starts in its own clan. `opposing_clan_mates` of P2's four cards
/// then share the clan of P2's slot 0, which is the card every round below selects, so the
/// anti-support count is `opposing_clan_mates.max(1)`.
fn brawl_post_round_spec(
    effect: CombatStatEffectV1,
    opposing_clan_mates: usize,
) -> CombatStatDiagnosticMatchSpecV1 {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(2893, CombatStatPredicateV1::Always, effect);
    for slot in 0..opposing_clan_mates {
        cards[PlayerId::P2][slot].effective_clan_id = 900;
    }
    CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    }
}

const BRAWL_OPPONENT_LIFE: CombatStatEffectV1 =
    CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerAntiSupport {
        per_count: 1,
        minimum: 3,
    };

/// The corpus pins the post-round Brawl reduction at a count of 4 only - 1093451/1,
/// 1058545/0, and the Min 0 floor in 964213/3 - so the counts of 1 and 2, the Min 3 floor
/// binding, a stopped source and exact make/unmake are pinned here.
#[test]
fn brawl_opponent_life_scales_by_the_count_clamps_once_and_unmakes() {
    for (mates, life) in [(1, 16), (2, 15), (4, 13)] {
        let spec = brawl_post_round_spec(BRAWL_OPPONENT_LIFE, mates);
        let mut diag = game(spec.base_rules, spec.cards);
        let start = diag.position().clone();
        let start_hash = position_hash(&start);
        let (report, undo) = diag
            .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
            .unwrap();
        assert!(report.cards[PlayerId::P1].won);
        // Three combat Damage, then the count.
        assert_eq!(report.players[PlayerId::P2].life, life);
        diag.unmake(undo);
        assert_eq!(diag.position(), &start);
        assert_eq!(position_hash(diag.position()), start_hash);
    }

    // The clamp is applied once, after multiplying: 8 - 3 = 5, then - 4 stops at 3.
    let mut spec = brawl_post_round_spec(BRAWL_OPPONENT_LIFE, 4);
    spec.base_rules.players[PlayerId::P2].initial_life = 8;
    let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 3);

    // A target the combat Damage already took to or below the Min is left alone.
    let mut spec = brawl_post_round_spec(BRAWL_OPPONENT_LIFE, 4);
    spec.base_rules.players[PlayerId::P2].initial_life = 5;
    let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 2);

    // A loss pays nothing, as Macey Rook in 1024524/2.
    let spec = brawl_post_round_spec(BRAWL_OPPONENT_LIFE, 4);
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 20);

    // A stopped source pays nothing: only the combat Damage lands.
    let mut spec = brawl_post_round_spec(BRAWL_OPPONENT_LIFE, 4);
    spec.cards[PlayerId::P2][0].ability = execute(
        4437,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].life, 17);

    // An overflowing magnitude is refused atomically rather than wrapped.
    let spec = brawl_post_round_spec(
        CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerAntiSupport {
            per_count: u16::MAX,
            minimum: 0,
        },
        2,
    );
    let mut diag = game(spec.base_rules, spec.cards);
    let before = diag.position().clone();
    assert!(matches!(
        diag.make(input(PlayerId::P1, (0, 5, false), (0, 0, false))),
        Err(CombatStatDiagnosticErrorV1::ArithmeticOverflow {
            player: PlayerId::P1,
            ..
        })
    ));
    assert_eq!(diag.position(), &before);
}

/// Sirrena's capped gain reaches exactly its Max of 9 three times in the corpus (1066739/0,
/// 1091848/0, 1092020/0: 12 - 7 + 4) but never binds below the uncapped sum, and the
/// uncapped `4583` pays once (1130527/0). The binding cap, an owner already at or above it,
/// and the other counts are pinned here.
#[test]
fn brawl_own_pillz_scales_by_the_count_and_respects_its_cap() {
    let capped = CombatStatEffectV1::GainPillzOnVictoryPerAntiSupport {
        per_count: 1,
        maximum: 9,
    };
    let uncapped = CombatStatEffectV1::GainPillzOnVictoryPerAntiSupport {
        per_count: 1,
        maximum: 0,
    };
    // (effect, count, starting Pillz, bet, final Pillz)
    for (effect, mates, pillz, bet, expected) in [
        (uncapped, 1, 20, 5, 16),
        (uncapped, 4, 20, 5, 19),
        (uncapped, 4, 12, 6, 10),
        // The corpus's case, landing exactly on the Max.
        (capped, 4, 12, 7, 9),
        // The cap binding: 12 - 5 + 4 would be 11.
        (capped, 4, 12, 5, 9),
        (capped, 2, 12, 7, 7),
        // An owner still at or above the Max after its bet gains nothing and is not lowered.
        (capped, 4, 12, 2, 10),
        (capped, 4, 12, 3, 9),
    ] {
        let mut spec = brawl_post_round_spec(effect, mates);
        spec.base_rules.players[PlayerId::P1].initial_pillz = pillz;
        let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
        let start = diag.position().clone();
        let (report, undo) = diag
            .make(input(PlayerId::P1, (0, bet, false), (0, 0, false)))
            .unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(
            report.players[PlayerId::P1].pillz,
            expected,
            "{effect:?} x{mates} from {pillz} betting {bet}"
        );
        diag.unmake(undo);
        assert_eq!(diag.position(), &start);
    }

    // A loss gains nothing.
    let spec = brawl_post_round_spec(capped, 4);
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].pillz, 20);
}

/// Newell's `-1 Opp. Pillz, Min 1` is selected once in the corpus and loses (1078669/1), so
/// the paying arithmetic rests on the revision-30 reduction arm it binds to; it is pinned
/// here at each count, onto and across the floor, and at the floor.
#[test]
fn brawl_opponent_pillz_scales_by_the_count_and_clamps_at_its_floor() {
    let effect = CombatStatEffectV1::ReduceOpponentPillzOnVictoryPerAntiSupport {
        per_count: 1,
        minimum: 1,
    };
    // (count, opposing Pillz, final opposing Pillz), the target betting nothing.
    for (mates, pillz, expected) in [(1, 20, 19), (4, 20, 16), (4, 5, 1), (4, 3, 1), (4, 1, 1)] {
        let mut spec = brawl_post_round_spec(effect, mates);
        spec.base_rules.players[PlayerId::P2].initial_pillz = pillz;
        let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
        let (report, _) = diag
            .make(input(PlayerId::P1, (0, 1, false), (0, 0, false)))
            .unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P2].pillz, expected);
    }

    let spec = brawl_post_round_spec(effect, 4);
    let mut diag = game(spec.base_rules, spec.cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert!(!report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P2].pillz, 15);
}

#[test]
fn brawl_post_round_plan_is_ability_only_positive_and_unconditional() {
    for effect in [
        BRAWL_OPPONENT_LIFE,
        CombatStatEffectV1::ReduceOpponentPillzOnVictoryPerAntiSupport {
            per_count: 1,
            minimum: 1,
        },
        CombatStatEffectV1::GainPillzOnVictoryPerAntiSupport {
            per_count: 1,
            maximum: 9,
        },
    ] {
        let mut bonus = brawl_post_round_spec(effect, 4);
        bonus.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
        bonus.cards[PlayerId::P1][0].bonus = execute(2893, CombatStatPredicateV1::Always, effect);
        bonus.cards[PlayerId::P1][0].source_bonus_support_count = 1;
        assert!(matches!(
            CombatStatDiagnosticV1::new(bonus),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::BrawlPostRoundSource,
                ..
            })
        ));

        let mut conditional = brawl_post_round_spec(effect, 4);
        conditional.cards[PlayerId::P1][0].ability =
            execute(2893, CombatStatPredicateV1::OwnerMovesFirst, effect);
        assert!(matches!(
            CombatStatDiagnosticV1::new(conditional),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::BrawlPostRoundPredicate,
                ..
            })
        ));
    }
    for zero in [
        CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerAntiSupport {
            per_count: 0,
            minimum: 0,
        },
        CombatStatEffectV1::ReduceOpponentPillzOnVictoryPerAntiSupport {
            per_count: 0,
            minimum: 1,
        },
        CombatStatEffectV1::GainPillzOnVictoryPerAntiSupport {
            per_count: 0,
            maximum: 0,
        },
    ] {
        assert!(matches!(
            CombatStatDiagnosticV1::new(brawl_post_round_spec(zero, 4)),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::BrawlPostRoundMagnitude,
                ..
            })
        ));
    }
}

/// P1's slot 0 prints 5/2 and P2's slot 0 prints 7/6, so any swap is visible in both
/// directions; every other source is absent until a test adds one.
fn exchange_spec(stat: CombatStatAttributeV1) -> CombatStatDiagnosticMatchSpecV1 {
    let mut base = base_spec(5, 2);
    base.players[PlayerId::P2].hand[0].power = 7;
    base.players[PlayerId::P2].hand[0].damage = 6;
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        1592,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ExchangePrintedCombatStat { stat },
    );
    CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    }
}

fn exchange_round(
    spec: CombatStatDiagnosticMatchSpecV1,
    first: PlayerId,
) -> ((u16, u16), (u16, u16)) {
    let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
    let start = diag.position().clone();
    let (report, undo) = diag
        .make(input(first, (0, 0, false), (0, 0, false)))
        .unwrap();
    let stats = (
        (
            report.cards[PlayerId::P1].power,
            report.cards[PlayerId::P1].damage,
        ),
        (
            report.cards[PlayerId::P2].power,
            report.cards[PlayerId::P2].damage,
        ),
    );
    diag.unmake(undo);
    assert_eq!(diag.position(), &start);
    stats
}

/// Every selected Exchange round in the corpus is a clean swap of printed values, such as
/// Lagertha Cr's 5 against Uuber's 7 in 867116/0, and `Damage Exchange` swaps the other
/// stat (901004/0); the pair form writes both. Who moves first does not matter.
#[test]
fn an_exchange_swaps_the_two_printed_values() {
    for first in PlayerId::ALL {
        assert_eq!(
            exchange_round(exchange_spec(CombatStatAttributeV1::Power), first),
            ((7, 2), (5, 6)),
        );
        assert_eq!(
            exchange_round(exchange_spec(CombatStatAttributeV1::Damage), first),
            ((5, 6), (7, 2)),
        );
        assert_eq!(
            exchange_round(exchange_spec(CombatStatAttributeV1::PowerAndDamage), first),
            ((7, 6), (5, 2)),
        );
    }
}

/// 1087884/1: Sue's `-1 Opp Power And Damage, Min 3` takes the 6 Lagertha Cr swapped to
/// her down to 5, so the swap lands before an opposing reduction. 1080007/2: Tina's own
/// `Revenge: Power +2` lands on the 5 she received. The owner's own increase, which no
/// corpus round shows, lands on the swapped value the same way, and in either orientation
/// of the two owners, since both sides read printed values.
#[test]
fn an_exchange_lands_before_every_increase_and_reduction() {
    let mut reduced = exchange_spec(CombatStatAttributeV1::Power);
    reduced.cards[PlayerId::P2][0].ability = execute(
        916,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::PowerAndDamage, 1, 3),
    );
    // Power 7 - 1; the printed 2 Damage is already under the Min 3 and is left alone.
    assert_eq!(exchange_round(reduced, PlayerId::P1).0, (6, 2));

    let mut opposing_increase = exchange_spec(CombatStatAttributeV1::Power);
    opposing_increase.cards[PlayerId::P2][0].ability = execute(
        1844,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Power, 2),
    );
    assert_eq!(exchange_round(opposing_increase, PlayerId::P1).1, (7, 6));

    let mut own_increase = exchange_spec(CombatStatAttributeV1::Power);
    own_increase.cards[PlayerId::P1][0].bonus = execute(
        43,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Power, 2),
    );
    own_increase.cards[PlayerId::P1][0].source_bonus_support_count = 1;
    assert_eq!(exchange_round(own_increase, PlayerId::P1).0, (9, 2));

    // The same Exchange owned by P2 instead.
    let mut base = base_spec(7, 6);
    base.players[PlayerId::P2].hand[0].power = 5;
    base.players[PlayerId::P2].hand[0].damage = 2;
    let mut cards = plans(&base);
    cards[PlayerId::P2][0].ability = execute(
        1592,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ExchangePrintedCombatStat {
            stat: CombatStatAttributeV1::Power,
        },
    );
    cards[PlayerId::P1][0].ability = execute(
        1844,
        CombatStatPredicateV1::Always,
        own(CombatStatAttributeV1::Power, 2),
    );
    let spec = CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    };
    assert_eq!(exchange_round(spec, PlayerId::P2), ((7, 6), (7, 2)));
}

/// 1066210/0: Spidee's Reprisal `Stop Opp. Ability` leaves both printed values where they
/// were. 948108/3: the opposing `Protection: Power And Damage` does not refuse the swap,
/// since Protection only ever refuses a reduction. An opposing Cancel of the stat skips the
/// whole swap, as in the reference, which no corpus round shows.
#[test]
fn an_exchange_is_stopped_or_cancelled_whole_and_never_refused_by_protection() {
    let mut stopped = exchange_spec(CombatStatAttributeV1::Power);
    stopped.cards[PlayerId::P2][0].ability = execute(
        40,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    assert_eq!(exchange_round(stopped, PlayerId::P1), ((5, 2), (7, 6)));

    let mut protected = exchange_spec(CombatStatAttributeV1::PowerAndDamage);
    protected.cards[PlayerId::P2][0].ability = execute(
        2434,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ProtectOwnCombatStat {
            stat: CombatStatAttributeV1::PowerAndDamage,
        },
    );
    assert_eq!(exchange_round(protected, PlayerId::P1), ((7, 6), (5, 2)));

    let mut cancelled = exchange_spec(CombatStatAttributeV1::Power);
    cancelled.cards[PlayerId::P2][0].ability = execute(
        5859,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::CancelOpponentCombatStatModifiers {
            stat: CombatStatAttributeV1::Power,
        },
    );
    assert_eq!(exchange_round(cancelled, PlayerId::P1), ((5, 2), (7, 6)));
}

/// Two Exchanges of the same stat each swap printed values, so together they are one swap
/// rather than a swap and its undoing: both sides write the other's printed value.
#[test]
fn two_exchanges_of_one_stat_are_one_swap() {
    let mut double = exchange_spec(CombatStatAttributeV1::Power);
    double.cards[PlayerId::P2][0].ability = execute(
        1592,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ExchangePrintedCombatStat {
            stat: CombatStatAttributeV1::Power,
        },
    );
    assert_eq!(exchange_round(double, PlayerId::P1), ((7, 2), (5, 6)));
}

/// `Night:` and `Day:` are a match constant. The corpus only ever shows each form in a
/// match of its own kind - the server sends the active variant and the catalog selects it -
/// so a source in the other kind of match, which only a Copy or a malformed capture could
/// produce, is pinned here: it stays present and never fires.
#[test]
fn night_and_day_sources_fire_only_in_their_own_kind_of_match() {
    let night_bonus = CombatStatEffectV1::ModifyCombatStat {
        side: CombatStatAffectedSideV1::Opponent,
        stat: CombatStatAttributeV1::PowerAndDamage,
        operation: CombatStatOperationV1::Decrease,
        value: 1,
        minimum: Some(1),
        maximum: None,
        multiplier: CombatStatMagnitudeV1::Fixed,
    };
    for (predicate, night, fires) in [
        (CombatStatPredicateV1::MatchIsNight, true, true),
        (CombatStatPredicateV1::MatchIsNight, false, false),
        (CombatStatPredicateV1::MatchIsDay, false, true),
        (CombatStatPredicateV1::MatchIsDay, true, false),
    ] {
        let mut base = base_spec(6, 3);
        base.night = night;
        let mut cards = plans(&base);
        // The GhosTown night bonus is a clan bonus, so the Bonus slot must admit it.
        cards[PlayerId::P1][0].bonus = execute(1442, predicate, night_bonus);
        cards[PlayerId::P1][0].source_bonus_support_count = 1;
        let mut diag = game(base, cards);
        let start = diag.position().clone();
        let (report, undo) = diag
            .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
            .unwrap();
        let expected = if fires { (5, 2) } else { (6, 3) };
        assert_eq!(
            (
                report.cards[PlayerId::P2].power,
                report.cards[PlayerId::P2].damage
            ),
            expected,
            "{predicate:?} at night = {night}",
        );
        // It is the same match constant in every round.
        diag.unmake(undo);
        assert_eq!(diag.position(), &start);
        let (_, _) = diag
            .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
            .unwrap();
        let (report, _) = diag
            .make(input(PlayerId::P2, (1, 0, false), (1, 0, false)))
            .unwrap();
        assert_eq!(report.cards[PlayerId::P2].power, 6);
    }
}

fn life_left_spec(
    effect: CombatStatEffectV1,
    power: u16,
    p1_life: u16,
    p2_life: u16,
) -> CombatStatDiagnosticMatchSpecV1 {
    let mut base = base_spec(power, 2);
    base.players[PlayerId::P1].initial_life = p1_life;
    base.players[PlayerId::P2].initial_life = p2_life;
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(1788, CombatStatPredicateV1::Always, effect);
    CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    }
}

fn life_left_power(maximum: u16) -> CombatStatEffectV1 {
    modifier(
        CombatStatAffectedSideV1::Player,
        CombatStatAttributeV1::Power,
        CombatStatOperationV1::Increase,
        1,
        None,
        Some(maximum),
        CombatStatMagnitudeV1::OwnerLife,
    )
}

/// `Per Life Left` reads the owner's Life at the start of the round - never the opponent's,
/// never the base Life - and a Max clamps the final stat, once, before opposing reductions.
/// The corpus pins the owner (1079650/3, where the two Lives differ), the round-start read
/// (losing rounds such as 1079173/2) and the clamp (Sir Taco, Noeptus, KinGreow); a stat
/// already above its Max, and the Copy that reads the copier's own Life, are pinned here.
#[test]
fn per_life_left_reads_the_owners_round_start_life_and_clamps_the_stat() {
    for (power, p1_life, p2_life, maximum, expected) in [
        // Below the Max: 1 + 5, whatever the opponent's Life.
        (1, 5, 3, 13, 6),
        (1, 5, 17, 13, 6),
        // The Max binds on the final stat, not on the bonus: 1 + 12 stops at 8.
        (1, 12, 12, 8, 8),
        // A printed stat already above its Max is left alone rather than lowered.
        (9, 12, 12, 8, 9),
    ] {
        let spec = life_left_spec(life_left_power(maximum), power, p1_life, p2_life);
        let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
        let start = diag.position().clone();
        let (report, undo) = diag
            .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
            .unwrap();
        assert_eq!(
            report.cards[PlayerId::P1].power,
            expected,
            "power {power}, life {p1_life} against {p2_life}, Max {maximum}",
        );
        diag.unmake(undo);
        assert_eq!(diag.position(), &start);
    }

    // The next round reads the Life the previous round left: P1 loses 2 in round one.
    let spec = life_left_spec(life_left_power(13), 1, 10, 10);
    let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (1, 0, false), (1, 5, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].life, 8);
    let (report, _) = diag
        .make(input(PlayerId::P2, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 1 + 8);

    // An opposing reduction lands after the clamp: 1 + 12 -> 8, then -1 -> 7.
    let mut spec = life_left_spec(life_left_power(8), 1, 12, 12);
    spec.cards[PlayerId::P2][0].ability = execute(
        916,
        CombatStatPredicateV1::Always,
        reduction(CombatStatAttributeV1::Power, 1, 1),
    );
    let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 7);

    // The Attack form adds the Life itself, and the opposing Attack reduction takes the
    // owner's Life from the opposing attack, never below its Min.
    let attack = modifier(
        CombatStatAffectedSideV1::Player,
        CombatStatAttributeV1::Attack,
        CombatStatOperationV1::Increase,
        1,
        None,
        None,
        CombatStatMagnitudeV1::OwnerLife,
    );
    let mut diag = CombatStatDiagnosticV1::new(life_left_spec(attack, 6, 9, 7)).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 1, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].attack, 6 * 2 + 9);
    let opposing = modifier(
        CombatStatAffectedSideV1::Opponent,
        CombatStatAttributeV1::Attack,
        CombatStatOperationV1::Decrease,
        1,
        Some(2),
        None,
        CombatStatMagnitudeV1::OwnerLife,
    );
    let mut diag = CombatStatDiagnosticV1::new(life_left_spec(opposing, 6, 9, 7)).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 0, false), (0, 1, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].attack, 6 * 2 - 9);
    let mut diag = CombatStatDiagnosticV1::new(life_left_spec(opposing, 6, 11, 7)).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 0, false), (0, 1, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].attack, 2);
}

/// A Copy adopting a `Per Life Left` ability makes the copier its owner, so the copier's
/// Life is the one read, as the reference does. No corpus round shows it.
#[test]
fn a_copied_per_life_left_reads_the_copiers_life() {
    let mut spec = copy_spec(
        CopiedSourceKindV1::Ability,
        execute(1788, CombatStatPredicateV1::Always, life_left_power(13)),
    );
    spec.base_rules.players[PlayerId::P1].initial_life = 4;
    spec.base_rules.players[PlayerId::P2].initial_life = 10;
    let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    // Both print 7 Power: the copier gains its own 4, the original owner its own 10, and
    // the Max of 13 binds only on the latter.
    assert_eq!(report.cards[PlayerId::P1].power, 11);
    assert_eq!(report.cards[PlayerId::P2].power, 13);
}

/// `Unison :` is a gate on the owner's whole hand sharing the selected card's effective
/// clan, not the multiplier the registry's clan-mates link suggests (1069608/2 pays the
/// printed +3 with four clan-mates, not +12). The corpus's only non-mono Unison round is on
/// a Stop body (1079813/3), so the gate's negative half, the Oculus that completes it and a
/// copier judged on its own hand are pinned here.
#[test]
fn unison_needs_the_owners_whole_hand_in_one_effective_clan() {
    let unison_power = execute(
        5318,
        CombatStatPredicateV1::OwnerHandUnison,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Power,
            CombatStatOperationV1::Increase,
            3,
            None,
            None,
            CombatStatMagnitudeV1::Fixed,
        ),
    );
    // (P1 hand clans, P1 power)
    for (clans, expected) in [
        // `base_spec` gives every card its own clan: not Unison.
        ([None, None, None, None], 6),
        // Three of four is still not Unison.
        ([Some(900), Some(900), Some(900), None], 6),
        // All four, whether native or by an infiltrating Oculus's effective clan.
        ([Some(900), Some(900), Some(900), Some(900)], 9),
    ] {
        let base = base_spec(6, 3);
        let mut cards = plans(&base);
        cards[PlayerId::P1][0].ability = unison_power;
        for (slot, clan) in clans.iter().enumerate() {
            if let Some(clan) = clan {
                cards[PlayerId::P1][slot].effective_clan_id = *clan;
            }
        }
        let mut diag = game(base, cards);
        let start = diag.position().clone();
        let (report, undo) = diag
            .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
            .unwrap();
        assert_eq!(report.cards[PlayerId::P1].power, expected, "{clans:?}");
        diag.unmake(undo);
        assert_eq!(diag.position(), &start);
    }

    // A copier adopting the opposing Unison ability is judged on its own hand: here the
    // copier's hand is mixed while the original owner's is mono, so only the owner pays.
    let mut spec = copy_spec(CopiedSourceKindV1::Ability, unison_power);
    for slot in 0..4 {
        spec.cards[PlayerId::P2][slot].effective_clan_id = 900;
    }
    let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].power, 7);
    assert_eq!(report.cards[PlayerId::P2].power, 10);

    // No clan bonus prints a Unison, so a Bonus-slot plan carrying the predicate is refused.
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].bonus = unison_power;
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
            base_rules: base,
            cards,
        }),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::ConditionalBonus,
            ..
        })
    ));
}

/// A conditional Stop is live only when its own condition holds, which is decided before the
/// Stop graph. The corpus pins Courage, Revenge and Asymmetry paying; Courage Stop Opp.
/// Ability is never selected and Confidence's one selection proves nothing, so both are
/// pinned here, together with the Bonus-slot refusal.
#[test]
fn a_conditional_stop_is_live_only_when_its_condition_holds() {
    let power_up = modifier(
        CombatStatAffectedSideV1::Player,
        CombatStatAttributeV1::Power,
        CombatStatOperationV1::Increase,
        3,
        None,
        None,
        CombatStatMagnitudeV1::Fixed,
    );
    let spec = |predicate| {
        let base = base_spec(6, 3);
        let mut cards = plans(&base);
        cards[PlayerId::P1][0].ability =
            execute(425, predicate, CombatStatEffectV1::StopOpponentAbility);
        cards[PlayerId::P1][1].ability =
            execute(425, predicate, CombatStatEffectV1::StopOpponentAbility);
        for slot in 0..4 {
            cards[PlayerId::P2][slot].ability =
                execute(1844, CombatStatPredicateV1::Always, power_up);
        }
        (base, cards)
    };

    // Courage: the Stop fires only when its owner moves first.
    let (base, cards) = spec(CombatStatPredicateV1::OwnerMovesFirst);
    let mut first = game(base.clone(), cards.clone());
    let (report, _) = first
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].power, 6);
    let mut second = game(base, cards);
    let (report, _) = second
        .make(input(PlayerId::P2, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].power, 9);

    // Confidence: nothing in round one, then live after a round its owner won.
    let (base, cards) = spec(CombatStatPredicateV1::OwnerWonPreviousRound);
    let mut diag = game(base, cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.cards[PlayerId::P2].power, 9);
    let (report, _) = diag
        .make(input(PlayerId::P2, (1, 0, false), (1, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P2].power, 6);

    // No clan bonus prints a conditional Stop.
    let (base, mut cards) = spec(CombatStatPredicateV1::OwnerMovesFirst);
    cards[PlayerId::P1][2].bonus = execute(
        287,
        CombatStatPredicateV1::OwnerMovesFirst,
        CombatStatEffectV1::StopOpponentBonus,
    );
    cards[PlayerId::P1][2].source_bonus_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
            base_rules: base,
            cards,
        }),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::ConditionalControl,
            ..
        })
    ));
}

fn stop_triggered_spec() -> CombatStatDiagnosticMatchSpecV1 {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        1474,
        CombatStatPredicateV1::OwnerAbilityStopped,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Damage,
            CombatStatOperationV1::Increase,
            4,
            None,
            None,
            CombatStatMagnitudeV1::Fixed,
        ),
    );
    CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    }
}

/// `Stop:` pays only when its owner's own ability is stopped. Eleven selected rounds pin the
/// half where it is not, and none pins the half where it is, so construction refuses any
/// match in which an opposing source - a `Stop Opp. Ability` in any slot under any
/// predicate, or a Copy that could adopt one - could stop it, and within every other match
/// the effect never fires. An opposing `Stop Opp. Bonus` stops nothing it needs (963931/3).
#[test]
fn a_stop_triggered_source_never_fires_where_nothing_can_stop_it() {
    let mut diag = CombatStatDiagnosticV1::new(stop_triggered_spec()).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].damage, 3);

    let mut stop_bonus = stop_triggered_spec();
    stop_bonus.cards[PlayerId::P2][0].ability = execute(
        40,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentBonus,
    );
    assert!(CombatStatDiagnosticV1::new(stop_bonus).is_ok());

    for opposing in [
        execute(
            40,
            CombatStatPredicateV1::Always,
            CombatStatEffectV1::StopOpponentAbility,
        ),
        execute(
            425,
            CombatStatPredicateV1::OwnerMovesFirst,
            CombatStatEffectV1::StopOpponentAbility,
        ),
        CombatStatSourcePlanV1::CopyOpponentSource {
            source_id: 846,
            copied: CopiedSourceKindV1::Ability,
            predicate: CombatStatPredicateV1::Always,
        },
    ] {
        let mut spec = stop_triggered_spec();
        // An unselected card is enough: the refusal is about what could happen in any line.
        spec.cards[PlayerId::P2][3].ability = opposing;
        spec.cards[PlayerId::P2][3].source_ability_support_count = u16::from(matches!(
            opposing,
            CombatStatSourcePlanV1::CopyOpponentSource { .. }
        ));
        assert!(
            matches!(
                CombatStatDiagnosticV1::new(spec),
                Err(CombatStatPlanErrorV1::InvalidExecute {
                    reason: InvalidCombatStatPlanReasonV1::StopTriggeredAgainstStopAbility,
                    ..
                })
            ),
            "{opposing:?}",
        );
    }

    // No clan bonus prints one.
    let mut bonus = stop_triggered_spec();
    bonus.cards[PlayerId::P1][0].bonus = bonus.cards[PlayerId::P1][0].ability;
    bonus.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    bonus.cards[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(bonus),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::ConditionalBonus,
            ..
        })
    ));
}

fn cancel(resources: ResourceCancellationV1) -> CombatStatSourcePlanV1 {
    execute(
        1172,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::CancelOpponentResourceModifiers { resources },
    )
}

/// `Cancel Opp. Life Modif.` drops the opposing selected card's end-of-round Life effects for
/// the round; the canceller's own are untouched, and a stopped canceller cancels nothing. The
/// corpus pins the Life half four times (1337321/0, 1337265/0, 943231/0, 1131208/0); the
/// arms it cannot reach, and every context the construction refuses, are pinned here.
#[test]
fn a_life_cancel_drops_only_the_opposing_life_effects_of_the_round() {
    let reduction = execute(
        512,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::ReduceOpponentLifeOnVictory {
            life: 3,
            minimum: 0,
        },
    );
    let spec = |opposing: CombatStatSourcePlanV1| {
        let base = base_spec(6, 3);
        let mut cards = plans(&base);
        cards[PlayerId::P1][0].ability = cancel(ResourceCancellationV1::Life);
        cards[PlayerId::P2][0].ability = opposing;
        (base, cards)
    };

    // P2 wins and its Victory reduction is cancelled: only the 3 combat Damage lands.
    let (base, cards) = spec(reduction);
    let mut diag = game(base, cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert!(report.cards[PlayerId::P2].won);
    assert_eq!(report.players[PlayerId::P1].life, 17);

    // A Pillz effect is not a Life effect: the Life cancel leaves it alone.
    let (base, cards) = spec(execute(
        1150,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::GainPillzOnVictory { pillz: 2 },
    ));
    let mut diag = game(base, cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].pillz, 20 - 5 + 2);

    // The canceller's own end-of-round work is not cancelled (1131225/3).
    let (base, mut cards) = spec(CombatStatSourcePlanV1::Absent);
    cards[PlayerId::P1][0].bonus = execute(
        43,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::GainLifeOnVictory { life: 3 },
    );
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    let mut diag = game(base, cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].life, 20 + 3);

    // A stopped canceller cancels nothing.
    let (base, mut cards) = spec(reduction);
    cards[PlayerId::P2][0].bonus = execute(
        40,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::StopOpponentAbility,
    );
    cards[PlayerId::P2][0].source_bonus_support_count = 1;
    let mut diag = game(base, cards);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].life, 20 - 3 - 3);

    // Construction refuses a canceller facing anything whose cancellation is unpinned: a
    // permanent, a compound, a both-players reduction, or a Copy - in any opposing slot.
    for opposing in [
        execute(
            206,
            CombatStatPredicateV1::Always,
            CombatStatEffectV1::PoisonOpponentLifeOnVictory {
                life: 1,
                minimum: 0,
            },
        ),
        execute(
            1768,
            CombatStatPredicateV1::Always,
            CombatStatEffectV1::GainPillzAndLifeOnKillshot { amount: 2 },
        ),
        execute(
            1379,
            CombatStatPredicateV1::Always,
            CombatStatEffectV1::ReduceBothPlayersLife {
                life: 1,
                minimum: 0,
            },
        ),
        CombatStatSourcePlanV1::CopyOpponentSource {
            source_id: 846,
            copied: CopiedSourceKindV1::Ability,
            predicate: CombatStatPredicateV1::Always,
        },
    ] {
        let base = base_spec(6, 3);
        let mut cards = plans(&base);
        cards[PlayerId::P1][0].ability = cancel(ResourceCancellationV1::Life);
        cards[PlayerId::P2][3].ability = opposing;
        cards[PlayerId::P2][3].source_ability_support_count = u16::from(matches!(
            opposing,
            CombatStatSourcePlanV1::CopyOpponentSource { .. }
        ));
        assert!(
            matches!(
                CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
                    base_rules: base,
                    cards,
                }),
                Err(CombatStatPlanErrorV1::InvalidExecute {
                    reason:
                        InvalidCombatStatPlanReasonV1::ResourceCancellationAgainstUnpinnedEffect,
                    ..
                })
            ),
            "{opposing:?}",
        );
    }

    // The Pillz and Life form, whose Pillz half no round has shown paying, is also refused
    // against any opposing Pillz effect; the Life form is not.
    for (resources, refused) in [
        (ResourceCancellationV1::PillzAndLife, true),
        (ResourceCancellationV1::Life, false),
    ] {
        let base = base_spec(6, 3);
        let mut cards = plans(&base);
        cards[PlayerId::P1][0].ability = cancel(resources);
        cards[PlayerId::P2][3].ability = execute(
            1150,
            CombatStatPredicateV1::Always,
            CombatStatEffectV1::GainPillzOnVictory { pillz: 2 },
        );
        assert_eq!(
            CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
                base_rules: base,
                cards,
            })
            .is_err(),
            refused,
            "{resources:?}",
        );
    }
}

/// `Killshot: +N Pillz And Life` is the Komboka pair of own gains on the Killshot trigger:
/// the owner's final attack at least doubling the opposing one, not the winner, and a living
/// owner. The corpus pins it paying at 38 against 14 (1337321/2) and not at 66 against 36
/// (956805/0); exactly double and a knocked-out owner are pinned here.
#[test]
fn killshot_pillz_and_life_pays_a_living_owner_at_double_the_opposing_attack() {
    let spec = || {
        let base = base_spec(6, 3);
        let mut cards = plans(&base);
        cards[PlayerId::P1][0].ability = execute(
            1768,
            CombatStatPredicateV1::Always,
            CombatStatEffectV1::GainPillzAndLifeOnKillshot { amount: 2 },
        );
        (base, cards)
    };
    // (P1 bet, P2 bet, pays): attack = 6 x (bet + 1).
    for (p1, p2, pays) in [(3, 1, true), (4, 1, true), (2, 1, false), (0, 5, false)] {
        let (base, cards) = spec();
        let mut diag = game(base, cards);
        let start = diag.position().clone();
        let (report, undo) = diag
            .make(input(PlayerId::P1, (0, p1, false), (0, p2, false)))
            .unwrap();
        let gain = if pays { 2 } else { 0 };
        assert_eq!(
            report.players[PlayerId::P1].pillz,
            20 - p1 + gain,
            "{p1} vs {p2}"
        );
        diag.unmake(undo);
        assert_eq!(diag.position(), &start);
    }
    // No clan bonus prints it.
    let (base, mut cards) = spec();
    cards[PlayerId::P1][0].bonus = cards[PlayerId::P1][0].ability;
    cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
    cards[PlayerId::P1][0].source_bonus_support_count = 1;
    assert!(matches!(
        CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
            base_rules: base,
            cards,
        }),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::KillshotPillzAndLifeSource,
            ..
        })
    ));
}

/// A conditional Exchange or stat Copy is the unconditional overwrite gated by its
/// predicate: when the predicate fails, nothing is swapped or copied. Every one of the eight
/// selected conditional Exchange rounds in the corpus is a paying round, so the false branch
/// is pinned here, together with a Unison source Copy on a mixed hand.
#[test]
fn a_conditional_exchange_or_copy_does_nothing_when_its_predicate_fails() {
    for (first, swaps) in [(PlayerId::P1, true), (PlayerId::P2, false)] {
        let mut spec = exchange_spec(CombatStatAttributeV1::Power);
        spec.cards[PlayerId::P1][0].ability = execute(
            3622,
            CombatStatPredicateV1::OwnerMovesFirst,
            CombatStatEffectV1::ExchangePrintedCombatStat {
                stat: CombatStatAttributeV1::Power,
            },
        );
        let expected = if swaps {
            ((7, 2), (5, 6))
        } else {
            ((5, 2), (7, 6))
        };
        assert_eq!(
            exchange_round(spec, first),
            expected,
            "{first:?} moves first"
        );
    }

    // `Unison : Copy: Opp. Ability` on a mixed hand adopts nothing; on a mono hand it adopts
    // the opposing ability.
    let power_up = execute(
        1844,
        CombatStatPredicateV1::Always,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Power,
            CombatStatOperationV1::Increase,
            3,
            None,
            None,
            CombatStatMagnitudeV1::Fixed,
        ),
    );
    for (mono, expected) in [(false, 7), (true, 10)] {
        let mut spec = copy_spec(CopiedSourceKindV1::Ability, power_up);
        spec.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::CopyOpponentSource {
            source_id: 3994,
            copied: CopiedSourceKindV1::Ability,
            predicate: CombatStatPredicateV1::OwnerHandUnison,
        };
        if mono {
            for slot in 0..4 {
                spec.cards[PlayerId::P1][slot].effective_clan_id = 900;
            }
            spec.cards[PlayerId::P1][0].source_ability_support_count = 4;
        }
        let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
        let (report, _) = diag
            .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
            .unwrap();
        assert_eq!(report.cards[PlayerId::P1].power, expected, "mono {mono}");
    }
}

fn owner_pillz_spec(
    multiplier: CombatStatMagnitudeV1,
    predicate: CombatStatPredicateV1,
) -> CombatStatDiagnosticMatchSpecV1 {
    let base = base_spec(6, 3);
    let mut cards = plans(&base);
    cards[PlayerId::P1][0].ability = execute(
        955,
        predicate,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Attack,
            CombatStatOperationV1::Increase,
            1,
            None,
            None,
            multiplier,
        ),
    );
    CombatStatDiagnosticMatchSpecV1 {
        base_rules: base,
        cards,
    }
}

/// `Per Pillz Left` reads the owner's Pillz at the start of the round, before this round's
/// bet (1065557/0, 1088580/0, 867173/3), and `Per Pillz Lost` the match-start Pillz less
/// that (1060510/3, 1058545/3). A Pillz count above the match start, which no capture
/// reaches, loses nothing.
#[test]
fn per_pillz_magnitudes_read_the_round_start_pillz_before_the_bet() {
    // Round one: 20 Pillz left, none lost, whatever the bet.
    let spec = owner_pillz_spec(
        CombatStatMagnitudeV1::OwnerPillz,
        CombatStatPredicateV1::Always,
    );
    let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 4, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].attack, 6 * 5 + 20);

    let spec = owner_pillz_spec(
        CombatStatMagnitudeV1::OwnerPillzLost,
        CombatStatPredicateV1::Always,
    );
    let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 4, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].attack, 6 * 5);

    // After a round that cost 7, the next round reads 13 left and 7 lost.
    for (multiplier, bonus) in [
        (CombatStatMagnitudeV1::OwnerPillz, 13),
        (CombatStatMagnitudeV1::OwnerPillzLost, 7),
    ] {
        let mut spec = owner_pillz_spec(multiplier, CombatStatPredicateV1::Always);
        spec.cards[PlayerId::P1][1].ability = spec.cards[PlayerId::P1][0].ability;
        spec.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Absent;
        let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
        diag.make(input(PlayerId::P1, (0, 7, false), (0, 0, false)))
            .unwrap();
        let (report, _) = diag
            .make(input(PlayerId::P2, (1, 2, false), (1, 0, false)))
            .unwrap();
        assert_eq!(
            report.cards[PlayerId::P1].attack,
            6 * 3 + bonus,
            "{multiplier:?}"
        );
    }

    // More Pillz than the match started with is no loss at all.
    let mut spec = owner_pillz_spec(
        CombatStatMagnitudeV1::OwnerPillzLost,
        CombatStatPredicateV1::Always,
    );
    spec.cards[PlayerId::P1][1].ability = spec.cards[PlayerId::P1][0].ability;
    spec.cards[PlayerId::P1][0].ability = execute(
        1150,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::GainPillzOnVictory { pillz: 5 },
    );
    let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].pillz, 25);
    let (report, _) = diag
        .make(input(PlayerId::P2, (1, 0, false), (1, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].attack, 6);

    // The Unison form is gated on the whole hand, and only Pillz Left may carry it.
    let spec = owner_pillz_spec(
        CombatStatMagnitudeV1::OwnerPillz,
        CombatStatPredicateV1::OwnerHandUnison,
    );
    let mut diag = CombatStatDiagnosticV1::new(spec).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.cards[PlayerId::P1].attack, 6);
    assert!(matches!(
        CombatStatDiagnosticV1::new(owner_pillz_spec(
            CombatStatMagnitudeV1::OwnerPillzLost,
            CombatStatPredicateV1::OwnerHandUnison
        )),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::CompoundPredicateAndMagnitude,
            ..
        })
    ));
}

/// `+N Life Per Opp. Damage` pays a living winner N per point of the losing card's final
/// Damage (1078736/1, 1065673/3) and nothing on a loss (926226/3, 948654/0). Whether an
/// opposing Fury counts is unpinned - neither paying opponent furied - and it is read as the
/// final Damage, Fury included, as the reference and the own-Damage conversions do.
#[test]
fn life_per_opposing_damage_pays_the_winner_the_losing_cards_final_damage() {
    let spec = || {
        let base = base_spec(6, 3);
        let mut cards = plans(&base);
        cards[PlayerId::P1][0].ability = execute(
            3779,
            CombatStatPredicateV1::Always,
            CombatStatEffectV1::GainLifePerOpponentFinalDamageOnVictory { life_per_damage: 1 },
        );
        CombatStatDiagnosticMatchSpecV1 {
            base_rules: base,
            cards,
        }
    };
    let mut diag = CombatStatDiagnosticV1::new(spec()).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].life, 20 + 3);

    // The losing card's Fury is part of its final Damage.
    let mut diag = CombatStatDiagnosticV1::new(spec()).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 9, false), (0, 0, true)))
        .unwrap();
    assert!(report.cards[PlayerId::P1].won);
    assert_eq!(report.players[PlayerId::P1].life, 20 + 3 + 2);

    // A loss pays nothing.
    let mut diag = CombatStatDiagnosticV1::new(spec()).unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P1].life, 20 - 3);
}

/// `Defeat: +N Pillz` and `Defeat: +N Pillz And Life` pay a loser the round has not knocked
/// out, Pillz first. Kubra's two knockouts pin that neither half pays at zero (876712/1,
/// 877023/1); a knocked-out owner of the plain Pillz form and the Bonus refusal are pinned
/// here.
#[test]
fn defeat_pillz_gains_pay_only_a_living_loser() {
    for (effect, pillz_gain, life_gain) in [
        (CombatStatEffectV1::GainPillzOnDefeat { pillz: 2 }, 2, 0),
        (
            CombatStatEffectV1::GainPillzAndLifeOnDefeat { amount: 1 },
            1,
            1,
        ),
    ] {
        let spec = |initial_life| {
            let mut base = base_spec(6, 3);
            base.players[PlayerId::P1].initial_life = initial_life;
            let mut cards = plans(&base);
            cards[PlayerId::P1][0].ability = execute(2222, CombatStatPredicateV1::Always, effect);
            game(base, cards)
        };
        // A surviving loss pays.
        let mut diag = spec(20);
        let (report, _) = diag
            .make(input(PlayerId::P1, (0, 2, false), (0, 5, false)))
            .unwrap();
        assert!(!report.cards[PlayerId::P1].won);
        assert_eq!(
            report.players[PlayerId::P1].pillz,
            20 - 2 + pillz_gain,
            "{effect:?}"
        );
        assert_eq!(
            report.players[PlayerId::P1].life,
            20 - 3 + life_gain,
            "{effect:?}"
        );
        // A win pays nothing.
        let mut diag = spec(20);
        let (report, _) = diag
            .make(input(PlayerId::P1, (0, 5, false), (0, 0, false)))
            .unwrap();
        assert_eq!(report.players[PlayerId::P1].pillz, 15, "{effect:?}");
        // A knockout pays nothing at all.
        let mut diag = spec(3);
        let (report, _) = diag
            .make(input(PlayerId::P1, (0, 2, false), (0, 5, false)))
            .unwrap();
        assert_eq!(report.players[PlayerId::P1].life, 0, "{effect:?}");
        assert_eq!(report.players[PlayerId::P1].pillz, 18, "{effect:?}");

        let mut base = base_spec(6, 3);
        base.players[PlayerId::P1].initial_life = 20;
        let mut cards = plans(&base);
        cards[PlayerId::P1][0].bonus = execute(2222, CombatStatPredicateV1::Always, effect);
        cards[PlayerId::P1][0].source_bonus_support_count = 1;
        assert!(matches!(
            CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
                base_rules: base,
                cards,
            }),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::DefeatPillzSource,
                ..
            })
        ));
    }
}

/// The `Growth:`/`Degrowth:` Victory grammars scale the printed amount by the zero-based
/// round plus one, or four less it, then pay and clamp it as the plain grammar does. The
/// corpus pins factors 3 and 4 on the opposing Life reduction (876796/2, 877167/3), 3 and 4
/// on the Life gain, and a Degrowth factor of 4 on the Pillz gain (1093173/0); the rest,
/// the floor and a loss are pinned here.
#[test]
fn round_scaled_victory_effects_scale_by_the_round_and_clamp_once() {
    let life_reduction = CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerRound {
        per_round: 1,
        minimum: 4,
        scale: RoundScaleV1::Growth,
    };
    let spec = |effect| {
        let base = base_spec(6, 0);
        let mut cards = plans(&base);
        for slot in 0..4 {
            cards[PlayerId::P1][slot].ability =
                execute(1730, CombatStatPredicateV1::Always, effect);
        }
        game(base, cards)
    };
    // P1 wins every round with zero Damage, so only the effect moves the target's Life.
    let mut diag = spec(life_reduction);
    let mut life = 20;
    for round in 0..4u8 {
        let first = if round % 2 == 0 {
            PlayerId::P1
        } else {
            PlayerId::P2
        };
        let (report, _) = diag
            .make(input(first, (round, 2, false), (round, 0, false)))
            .unwrap();
        life -= u16::from(round) + 1;
        assert_eq!(report.players[PlayerId::P2].life, life, "round {round}");
    }
    // 20 - 1 - 2 - 3 - 4 = 10; the Min 4 never binds here, so pin it separately.
    let mut base = base_spec(6, 0);
    base.players[PlayerId::P2].initial_life = 6;
    let mut cards = plans(&base);
    for slot in 0..4 {
        cards[PlayerId::P1][slot].ability =
            execute(1730, CombatStatPredicateV1::Always, life_reduction);
    }
    let mut diag = game(base, cards);
    diag.make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
        .unwrap();
    let (report, _) = diag
        .make(input(PlayerId::P2, (1, 2, false), (1, 0, false)))
        .unwrap();
    // 6 - 1 = 5, then 5 - 2 stops at the Min of 4.
    assert_eq!(report.players[PlayerId::P2].life, 4);

    // Degrowth counts down, and the own gains and the opposing Pillz reduction bind to the
    // plain arms the same way.
    for (effect, round_one_check) in [
        (
            CombatStatEffectV1::GainPillzOnVictoryPerRound {
                per_round: 1,
                scale: RoundScaleV1::Degrowth,
            },
            (PlayerId::P1, 20 - 2 + 4),
        ),
        (
            CombatStatEffectV1::GainLifeOnVictoryPerRound {
                per_round: 1,
                scale: RoundScaleV1::Growth,
            },
            (PlayerId::P1, 20 + 1),
        ),
        (
            CombatStatEffectV1::ReduceOpponentPillzOnVictoryPerRound {
                per_round: 1,
                minimum: 0,
                scale: RoundScaleV1::Degrowth,
            },
            (PlayerId::P2, 20 - 4),
        ),
    ] {
        let mut diag = spec(effect);
        let (report, _) = diag
            .make(input(PlayerId::P1, (0, 2, false), (0, 0, false)))
            .unwrap();
        let (player, expected) = round_one_check;
        let actual = match effect {
            CombatStatEffectV1::GainLifeOnVictoryPerRound { .. } => report.players[player].life,
            _ => report.players[player].pillz,
        };
        assert_eq!(actual, expected, "{effect:?}");
    }

    // A loss pays nothing.
    let mut diag = spec(life_reduction);
    let (report, _) = diag
        .make(input(PlayerId::P1, (0, 0, false), (0, 5, false)))
        .unwrap();
    assert_eq!(report.players[PlayerId::P2].life, 20);
}

/// `+N Dam./ Life Lost Max. M` (`5113`) reads its owner's match-start Life less their
/// round-start Life: nothing in round 0, N per point lost after that, the final Damage
/// clamped to M (924890/3 binds it at 6), and nothing when the owner's effective clan is not
/// listed. Every captured round reads an owner whose Life has only fallen, which cannot tell
/// the net shortfall from every point ever lost, so a match where the reader's Life could
/// rise is refused: an own Life gain, an opposing one that an own Copy could adopt, or an
/// opposing Copy that could adopt the source into a hand with a Life gain of its own.
#[test]
fn life_lost_damage_reads_the_owner_shortfall_and_refuses_a_life_that_can_rise() {
    let set = |ids: &[u32]| ClanSetV1::from_ids(ids).unwrap();
    let life_lost = |listed: u32| {
        execute(
            5113,
            CombatStatPredicateV1::OwnerClanIn(set(&[listed])),
            modifier(
                CombatStatAffectedSideV1::Player,
                CombatStatAttributeV1::Damage,
                CombatStatOperationV1::Increase,
                1,
                None,
                Some(8),
                CombatStatMagnitudeV1::OwnerLifeLost,
            ),
        )
    };
    // P1 loses `losses` rounds on its plain cards (3 Life each), then plays the gated one.
    for (losses, listed, damage) in [(0, 1, 3), (1, 1, 3 + 3), (2, 1, 8), (2, 2, 3)] {
        let (base, mut cards) = clan_gate_spec();
        cards[PlayerId::P1][0].ability = life_lost(listed);
        let mut diag = game(base, cards);
        let start = diag.position().clone();
        let mut first = PlayerId::P1;
        let mut undo = Vec::new();
        for round in 0..losses {
            let (report, step) = diag
                .make(input(first, (1 + round, 0, false), (round, 2, false)))
                .unwrap();
            assert!(!report.cards[PlayerId::P1].won);
            undo.push(step);
            first = first.other();
        }
        let (report, step) = diag
            .make(input(first, (0, 0, false), (losses, 0, false)))
            .unwrap();
        assert_eq!(
            report.cards[PlayerId::P1].damage,
            damage,
            "{losses} losses, listed {listed}"
        );
        undo.push(step);
        for step in undo.into_iter().rev() {
            diag.unmake(step);
        }
        assert_eq!(diag.position(), &start);
    }

    let gain = execute(
        377,
        CombatStatPredicateV1::Always,
        CombatStatEffectV1::GainLifeOnVictory { life: 3 },
    );
    let copy = CombatStatSourcePlanV1::CopyOpponentSource {
        source_id: 4497,
        copied: CopiedSourceKindV1::Ability,
        predicate: CombatStatPredicateV1::Always,
    };
    let refused = |own_gain: bool, own_copy: bool, opposing_gain: bool, opposing_copy: bool| {
        let (base, mut cards) = clan_gate_spec();
        cards[PlayerId::P1][0].ability = life_lost(1);
        if own_gain {
            cards[PlayerId::P1][3].ability = gain;
        }
        if own_copy {
            cards[PlayerId::P1][2].ability = copy;
            cards[PlayerId::P1][2].source_ability_support_count = 1;
        }
        if opposing_gain {
            cards[PlayerId::P2][1].ability = gain;
        }
        if opposing_copy {
            cards[PlayerId::P2][2].ability = copy;
            cards[PlayerId::P2][2].source_ability_support_count = 1;
        }
        match CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
            base_rules: base,
            cards,
        }) {
            Ok(_) => false,
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::LifeLostOwnerLifeCanRise,
                ..
            }) => true,
            Err(error) => panic!("unexpected {error:?}"),
        }
    };
    assert!(refused(true, false, false, false));
    assert!(refused(false, true, true, false));
    // An opposing Copy adopting the source reads its own owner's Life, which that owner's
    // own gain could raise.
    assert!(refused(false, false, true, true));
    // An opposing gain raises only its own owner, and a Copy with no gain to adopt raises
    // nothing, so each alone is admitted.
    assert!(!refused(false, false, true, false));
    assert!(!refused(false, true, false, false));
    assert!(!refused(false, false, false, true));
}

/// `Bet > N Pillz:` and `Bet < N Pillz:` compare the owner's `pillzUsed` - the bet plus the
/// free pill, Fury's three excluded - strictly with N. The corpus pins both edges:
/// 1207064/0 and 945791/1 win at exactly N and pay nothing, and 1093129/3 wins on a Fury
/// that would have cleared the gate and pays nothing. The gate works the same on a
/// combat-stat body, on a post-round Victory gain and on a Copy's adoption.
#[test]
fn bet_gates_compare_the_owner_pillz_used_strictly_and_ignore_fury() {
    use CombatStatPredicateV1::{OwnerPillzUsedAbove as Above, OwnerPillzUsedBelow as Below};
    let cases = [
        (Above(3), 2, false, false),
        (Above(3), 3, false, true),
        (Above(3), 2, true, false),
        (Below(3), 1, false, true),
        (Below(3), 2, false, false),
        (Below(3), 1, true, true),
    ];
    for (predicate, bet, fury, fires) in cases {
        let base = base_spec(6, 3);
        let mut cards = plans(&base);
        cards[PlayerId::P1][0].ability =
            execute(5401, predicate, own(CombatStatAttributeV1::Power, 3));
        let mut diag = game(base, cards);
        let (report, _) = diag
            .make(input(PlayerId::P1, (0, bet, fury), (0, 0, false)))
            .unwrap();
        assert_eq!(
            report.cards[PlayerId::P1].power,
            if fires { 9 } else { 6 },
            "{predicate:?} bet {bet} fury {fury}"
        );
    }

    // A won round's Victory Life, gated.
    for (bet, fury, life) in [(2, false, 20), (3, false, 23), (2, true, 20)] {
        let base = base_spec(6, 3);
        let mut cards = plans(&base);
        cards[PlayerId::P1][0].ability = execute(
            4657,
            Above(3),
            CombatStatEffectV1::GainLifeOnVictory { life: 3 },
        );
        let mut diag = game(base, cards);
        let (report, _) = diag
            .make(input(PlayerId::P1, (0, bet, fury), (0, 0, false)))
            .unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(
            report.players[PlayerId::P1].life,
            life,
            "bet {bet} fury {fury}"
        );
    }

    // A gated Copy adopts the opposing ability only when the gate holds (943231/1 pins the
    // refusal at exactly N: Madlocks does not take Miyo's Stop Opp. Bonus).
    for (bet, power) in [(2, 6), (3, 9)] {
        let base = base_spec(6, 3);
        let mut cards = plans(&base);
        cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::CopyOpponentSource {
            source_id: 5304,
            copied: CopiedSourceKindV1::Ability,
            predicate: Above(3),
        };
        cards[PlayerId::P1][0].source_ability_support_count = 1;
        cards[PlayerId::P2][0].ability = execute(
            310,
            CombatStatPredicateV1::Always,
            own(CombatStatAttributeV1::Power, 3),
        );
        let mut diag = game(base, cards);
        let (report, _) = diag
            .make(input(PlayerId::P1, (0, bet, false), (0, 0, false)))
            .unwrap();
        assert_eq!(report.cards[PlayerId::P1].power, power, "bet {bet}");
    }
}

/// Hands whose canonical clans are 1..4 for P1 and 11..14 for P2, so a clan set can name
/// them; effective clans start equal to the canonical ones.
fn clan_gate_spec() -> (BaseRulesMatchSpec, ByPlayer<[CombatStatCardPlanV1; 4]>) {
    let mut base = base_spec(6, 3);
    for slot in 0..4 {
        base.players[PlayerId::P1].hand[slot].clan_id = 1 + slot as u32;
        base.players[PlayerId::P2].hand[slot].clan_id = 11 + slot as u32;
    }
    let cards = plans(&base);
    (base, cards)
}

fn power_up_under(predicate: CombatStatPredicateV1) -> CombatStatSourcePlanV1 {
    execute(
        4667,
        predicate,
        modifier(
            CombatStatAffectedSideV1::Player,
            CombatStatAttributeV1::Power,
            CombatStatOperationV1::Increase,
            3,
            None,
            None,
            CombatStatMagnitudeV1::Fixed,
        ),
    )
}

/// The three clan gates: `[clan:..]` reads the owner's selected card's effective clan,
/// `Versus [clan:..]` any canonical clan in the opposing hand, and `After [clan:..]` the
/// canonical clan of the card the owner played in the previous round, which never holds in
/// round 0. The corpus pins all three paying and refusing; these pin the edges.
#[test]
fn clan_gates_read_the_owner_card_the_opposing_hand_and_the_previous_card() {
    let set = |ids: &[u32]| ClanSetV1::from_ids(ids).unwrap();
    for (predicate, fires) in [
        (CombatStatPredicateV1::OwnerClanIn(set(&[1])), true),
        (CombatStatPredicateV1::OwnerClanIn(set(&[2])), false),
        // Any card in the opposing hand, not only the one it faces.
        (CombatStatPredicateV1::OpponentHandHasClan(set(&[13])), true),
        (CombatStatPredicateV1::OpponentHandHasClan(set(&[5])), false),
        // No previous card in round 0.
        (
            CombatStatPredicateV1::OwnerPreviousCardClanIn(set(&[1, 2, 3, 4])),
            false,
        ),
    ] {
        let (base, mut cards) = clan_gate_spec();
        cards[PlayerId::P1][0].ability = power_up_under(predicate);
        let mut diag = game(base, cards);
        let (report, _) = diag
            .make(input(PlayerId::P1, (0, 0, false), (0, 0, false)))
            .unwrap();
        assert_eq!(
            report.cards[PlayerId::P1].power,
            if fires { 9 } else { 6 },
            "{predicate:?}"
        );
    }

    // `After`: round 0 plays the clan-2 card, round 1 the gated one.
    for (listed, fires) in [(2, true), (3, false)] {
        let (base, mut cards) = clan_gate_spec();
        cards[PlayerId::P1][2].ability =
            power_up_under(CombatStatPredicateV1::OwnerPreviousCardClanIn(set(&[
                listed,
            ])));
        let mut diag = game(base, cards);
        let start = diag.position().clone();
        let (_, first) = diag
            .make(input(PlayerId::P1, (1, 0, false), (1, 0, false)))
            .unwrap();
        let (report, second) = diag
            .make(input(PlayerId::P2, (2, 0, false), (2, 0, false)))
            .unwrap();
        assert_eq!(
            report.cards[PlayerId::P1].power,
            if fires { 9 } else { 6 },
            "after {listed}"
        );
        diag.unmake(second);
        diag.unmake(first);
        assert_eq!(diag.position(), &start);
    }

    // Only `After` is printed on a clan bonus; the other two are refused from the Bonus slot.
    for (predicate, allowed) in [
        (
            CombatStatPredicateV1::OwnerPreviousCardClanIn(set(&[2])),
            true,
        ),
        (CombatStatPredicateV1::OwnerClanIn(set(&[1])), false),
        (
            CombatStatPredicateV1::OpponentHandHasClan(set(&[11])),
            false,
        ),
    ] {
        let (base, mut cards) = clan_gate_spec();
        cards[PlayerId::P1][0].bonus = power_up_under(predicate);
        cards[PlayerId::P1][0].source_bonus_support_count = 1;
        assert_eq!(
            CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
                base_rules: base,
                cards,
            })
            .is_ok(),
            allowed,
            "{predicate:?}",
        );
    }

    // `After` and `Versus` read canonical clans. An infiltrating Oculus whose effective clan
    // and Oculus itself fall on opposite sides of the list is the case no round separates,
    // so the match is refused.
    let (mut base, mut cards) = clan_gate_spec();
    base.players[PlayerId::P2].hand[3].clan_id = 56;
    cards[PlayerId::P2][3].effective_clan_id = 11;
    cards[PlayerId::P1][0].ability =
        power_up_under(CombatStatPredicateV1::OpponentHandHasClan(set(&[11])));
    assert!(matches!(
        CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 {
            base_rules: base,
            cards,
        }),
        Err(CombatStatPlanErrorV1::InvalidExecute {
            reason: InvalidCombatStatPlanReasonV1::AmbiguousOculusClanGate,
            ..
        })
    ));
}
