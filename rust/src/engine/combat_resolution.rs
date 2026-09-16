//! Private, allocation-free combat resolution shared by projected diagnostic models.
//!
//! Replay preparation owns admission and predicates. This module receives only compact,
//! already-admitted effects for the two selected cards.

use super::clan_bonus_diagnostic::{
    DiagnosticAffectedSideV1, DiagnosticCombatEffectV1, DiagnosticCombatStatV1,
    DiagnosticMagnitudeV1, DiagnosticStatOperationV1,
};
use super::{
    BaseRulesCardResult, ByPlayer, PlayerId, PostRoundEffect, PostRoundPlan, PreparedSelection,
    ValidatedSelection, FURY_DAMAGE, MAX_ROUNDS,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CombatResolutionArithmeticStage {
    EffectMagnitude,
    Power,
    Damage,
    Attack,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CombatResolutionError {
    pub player: PlayerId,
    pub stage: CombatResolutionArithmeticStage,
}

#[derive(Clone, Copy, Default)]
pub(super) struct ResolutionSourcePlan {
    pub effect: Option<DiagnosticCombatEffectV1>,
    pub post_round: Option<PostRoundEffect>,
    pub support_count: u16,
}

#[derive(Clone, Copy, Default)]
pub(super) struct ResolutionCardPlan {
    pub ability: ResolutionSourcePlan,
    pub bonus: ResolutionSourcePlan,
}

#[derive(Clone, Copy)]
pub(super) struct PreparedCombatResolution {
    pub selections: ByPlayer<PreparedSelection>,
    pub post_round: ByPlayer<PostRoundPlan>,
}

#[derive(Clone, Copy, Default)]
struct CancellationMask {
    attack: bool,
    damage: bool,
    power: bool,
}

impl CancellationMask {
    fn insert(&mut self, stat: DiagnosticCombatStatV1) {
        match stat {
            DiagnosticCombatStatV1::Attack => self.attack = true,
            DiagnosticCombatStatV1::Damage => self.damage = true,
            DiagnosticCombatStatV1::Power => self.power = true,
            DiagnosticCombatStatV1::PowerAndDamage => {
                self.power = true;
                self.damage = true;
            }
        }
    }

    fn contains(self, stat: DiagnosticCombatStatV1) -> bool {
        match stat {
            DiagnosticCombatStatV1::Attack => self.attack,
            DiagnosticCombatStatV1::Damage => self.damage,
            DiagnosticCombatStatV1::Power => self.power,
            DiagnosticCombatStatV1::PowerAndDamage => self.power || self.damage,
        }
    }
}

fn is_stop_bonus(source: ResolutionSourcePlan) -> bool {
    matches!(
        source.effect,
        Some(DiagnosticCombatEffectV1::StopOpponentBonus)
    )
}

fn add_cancellation(mask: &mut CancellationMask, source: ResolutionSourcePlan) {
    if let Some(DiagnosticCombatEffectV1::CancelOpponentCombatStatModifiers { stat }) =
        source.effect
    {
        mask.insert(stat);
    }
}

pub(super) fn prepare_combat_resolution(
    validated: ByPlayer<ValidatedSelection>,
    selected_plans: ByPlayer<ResolutionCardPlan>,
    rounds_played: u8,
) -> Result<ByPlayer<PreparedSelection>, CombatResolutionError> {
    Ok(
        prepare_combat_resolution_with_post_round(validated, selected_plans, rounds_played)?
            .selections,
    )
}

pub(super) fn prepare_combat_resolution_with_post_round(
    validated: ByPlayer<ValidatedSelection>,
    selected_plans: ByPlayer<ResolutionCardPlan>,
    rounds_played: u8,
) -> Result<PreparedCombatResolution, CombatResolutionError> {
    let opponent_stars = ByPlayer::new(
        u16::from(validated[PlayerId::P2].card.key.level),
        u16::from(validated[PlayerId::P1].card.key.level),
    );
    let mut bonus_live = ByPlayer::new(
        source_is_live(selected_plans[PlayerId::P1].bonus),
        source_is_live(selected_plans[PlayerId::P2].bonus),
    );

    // Ability-origin Stop Bonus resolves before the bonus-vs-bonus dependency.
    for player in PlayerId::ALL {
        if is_stop_bonus(selected_plans[player].ability) {
            bonus_live[player.other()] = false;
        }
    }

    // Surviving bonus-origin Stop Bonus is simultaneous. Snapshot both results before
    // mutating either side so player iteration order cannot affect mutual Stop Bonus.
    let bonus_stops = ByPlayer::new(
        bonus_live[PlayerId::P1] && is_stop_bonus(selected_plans[PlayerId::P1].bonus),
        bonus_live[PlayerId::P2] && is_stop_bonus(selected_plans[PlayerId::P2].bonus),
    );
    if bonus_stops[PlayerId::P1] {
        bonus_live[PlayerId::P2] = false;
    }
    if bonus_stops[PlayerId::P2] {
        bonus_live[PlayerId::P1] = false;
    }

    let mut cancellations = ByPlayer::new(CancellationMask::default(), CancellationMask::default());
    for player in PlayerId::ALL {
        add_cancellation(&mut cancellations[player], selected_plans[player].ability);
        if bonus_live[player] {
            add_cancellation(&mut cancellations[player], selected_plans[player].bonus);
        }
    }

    let mut power = ByPlayer::new(
        validated[PlayerId::P1].card.power,
        validated[PlayerId::P2].card.power,
    );
    let mut damage = ByPlayer::new(
        validated[PlayerId::P1].card.damage,
        validated[PlayerId::P2].card.damage,
    );

    // Source compilation is Bonus then Ability. Own increases retain that stable order.
    for origin in PlayerId::ALL {
        if bonus_live[origin] {
            apply_power_damage_effect(
                origin,
                DiagnosticAffectedSideV1::Player,
                DiagnosticStatOperationV1::Increase,
                selected_plans[origin].bonus,
                cancellations[origin.other()],
                rounds_played,
                opponent_stars[origin],
                &mut power,
                &mut damage,
            )?;
        }
        apply_power_damage_effect(
            origin,
            DiagnosticAffectedSideV1::Player,
            DiagnosticStatOperationV1::Increase,
            selected_plans[origin].ability,
            cancellations[origin.other()],
            rounds_played,
            opponent_stars[origin],
            &mut power,
            &mut damage,
        )?;
    }

    // Server-backed TypeScript semantics stable-sort opponent reductions by descending
    // Min. Equal-Min effects retain source compilation order (Bonus then Ability).
    for origin in PlayerId::ALL {
        let bonus = if bonus_live[origin] {
            selected_plans[origin].bonus
        } else {
            ResolutionSourcePlan::default()
        };
        apply_ordered_power_damage_reductions(
            origin,
            bonus,
            selected_plans[origin].ability,
            cancellations[origin.other()],
            rounds_played,
            opponent_stars[origin],
            &mut power,
            &mut damage,
        )?;
    }

    // Fury is added after Power/Damage modifiers.
    for player in PlayerId::ALL {
        if validated[player].selection.fury {
            damage[player] =
                damage[player]
                    .checked_add(FURY_DAMAGE)
                    .ok_or(CombatResolutionError {
                        player,
                        stage: CombatResolutionArithmeticStage::Damage,
                    })?;
        }
    }

    let mut attack = ByPlayer::new(0_u32, 0_u32);
    for player in PlayerId::ALL {
        attack[player] = u32::from(power[player])
            .checked_mul(u32::from(validated[player].selection.pillz) + 1)
            .ok_or(CombatResolutionError {
                player,
                stage: CombatResolutionArithmeticStage::Attack,
            })?;
    }

    // Own Attack increases retain Bonus then Ability source order.
    for origin in PlayerId::ALL {
        if bonus_live[origin] {
            apply_attack_effect(
                origin,
                DiagnosticAffectedSideV1::Player,
                DiagnosticStatOperationV1::Increase,
                selected_plans[origin].bonus,
                cancellations[origin.other()],
                rounds_played,
                opponent_stars[origin],
                &mut attack,
            )?;
        }
        apply_attack_effect(
            origin,
            DiagnosticAffectedSideV1::Player,
            DiagnosticStatOperationV1::Increase,
            selected_plans[origin].ability,
            cancellations[origin.other()],
            rounds_played,
            opponent_stars[origin],
            &mut attack,
        )?;
    }

    // Opponent Attack reductions use the same stable descending-Min ordering.
    for origin in PlayerId::ALL {
        let bonus = if bonus_live[origin] {
            selected_plans[origin].bonus
        } else {
            ResolutionSourcePlan::default()
        };
        apply_ordered_attack_reductions(
            origin,
            bonus,
            selected_plans[origin].ability,
            cancellations[origin.other()],
            rounds_played,
            opponent_stars[origin],
            &mut attack,
        )?;
    }

    let selections = ByPlayer::new(
        finish_selection(
            validated[PlayerId::P1],
            power[PlayerId::P1],
            damage[PlayerId::P1],
            attack[PlayerId::P1],
        ),
        finish_selection(
            validated[PlayerId::P2],
            power[PlayerId::P2],
            damage[PlayerId::P2],
            attack[PlayerId::P2],
        ),
    );
    let post_round = ByPlayer::new(
        PostRoundPlan {
            ability: selected_plans[PlayerId::P1].ability.post_round,
            bonus: bonus_live[PlayerId::P1]
                .then_some(selected_plans[PlayerId::P1].bonus.post_round)
                .flatten(),
        },
        PostRoundPlan {
            ability: selected_plans[PlayerId::P2].ability.post_round,
            bonus: bonus_live[PlayerId::P2]
                .then_some(selected_plans[PlayerId::P2].bonus.post_round)
                .flatten(),
        },
    );
    Ok(PreparedCombatResolution {
        selections,
        post_round,
    })
}

fn source_is_live(source: ResolutionSourcePlan) -> bool {
    source.effect.is_some() || source.post_round.is_some()
}

fn finish_selection(
    selected: ValidatedSelection,
    power: u16,
    damage: u16,
    attack: u32,
) -> PreparedSelection {
    PreparedSelection {
        slot: selected.slot,
        cost: selected.cost,
        card: selected.card,
        result: BaseRulesCardResult {
            key: selected.card.key,
            hand_slot: selected.slot,
            power,
            damage,
            attack,
            won: false,
        },
    }
}

fn apply_ordered_power_damage_reductions(
    origin: PlayerId,
    bonus: ResolutionSourcePlan,
    ability: ResolutionSourcePlan,
    opponent_cancellation: CancellationMask,
    rounds_played: u8,
    opponent_stars: u16,
    power: &mut ByPlayer<u16>,
    damage: &mut ByPlayer<u16>,
) -> Result<(), CombatResolutionError> {
    let bonus_min = power_damage_reduction_min(bonus.effect);
    let ability_min = power_damage_reduction_min(ability.effect);
    if ability_min > bonus_min {
        apply_power_damage_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            ability,
            opponent_cancellation,
            rounds_played,
            opponent_stars,
            power,
            damage,
        )?;
        apply_power_damage_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            bonus,
            opponent_cancellation,
            rounds_played,
            opponent_stars,
            power,
            damage,
        )
    } else {
        apply_power_damage_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            bonus,
            opponent_cancellation,
            rounds_played,
            opponent_stars,
            power,
            damage,
        )?;
        apply_power_damage_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            ability,
            opponent_cancellation,
            rounds_played,
            opponent_stars,
            power,
            damage,
        )
    }
}

fn apply_ordered_attack_reductions(
    origin: PlayerId,
    bonus: ResolutionSourcePlan,
    ability: ResolutionSourcePlan,
    opponent_cancellation: CancellationMask,
    rounds_played: u8,
    opponent_stars: u16,
    attack: &mut ByPlayer<u32>,
) -> Result<(), CombatResolutionError> {
    let bonus_min = attack_reduction_min(bonus.effect);
    let ability_min = attack_reduction_min(ability.effect);
    if ability_min > bonus_min {
        apply_attack_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            ability,
            opponent_cancellation,
            rounds_played,
            opponent_stars,
            attack,
        )?;
        apply_attack_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            bonus,
            opponent_cancellation,
            rounds_played,
            opponent_stars,
            attack,
        )
    } else {
        apply_attack_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            bonus,
            opponent_cancellation,
            rounds_played,
            opponent_stars,
            attack,
        )?;
        apply_attack_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            ability,
            opponent_cancellation,
            rounds_played,
            opponent_stars,
            attack,
        )
    }
}

fn power_damage_reduction_min(effect: Option<DiagnosticCombatEffectV1>) -> Option<u16> {
    match effect {
        Some(DiagnosticCombatEffectV1::ModifyCombatStat {
            side: DiagnosticAffectedSideV1::Opponent,
            stat:
                DiagnosticCombatStatV1::Power
                | DiagnosticCombatStatV1::Damage
                | DiagnosticCombatStatV1::PowerAndDamage,
            operation: DiagnosticStatOperationV1::Decrease,
            minimum,
            ..
        }) => Some(minimum.unwrap_or(0)),
        _ => None,
    }
}

fn attack_reduction_min(effect: Option<DiagnosticCombatEffectV1>) -> Option<u16> {
    match effect {
        Some(DiagnosticCombatEffectV1::ModifyCombatStat {
            side: DiagnosticAffectedSideV1::Opponent,
            stat: DiagnosticCombatStatV1::Attack,
            operation: DiagnosticStatOperationV1::Decrease,
            minimum,
            ..
        }) => Some(minimum.unwrap_or(0)),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_power_damage_effect(
    origin: PlayerId,
    expected_side: DiagnosticAffectedSideV1,
    expected_operation: DiagnosticStatOperationV1,
    source: ResolutionSourcePlan,
    opponent_cancellation: CancellationMask,
    rounds_played: u8,
    opponent_stars: u16,
    power: &mut ByPlayer<u16>,
    damage: &mut ByPlayer<u16>,
) -> Result<(), CombatResolutionError> {
    let Some(DiagnosticCombatEffectV1::ModifyCombatStat {
        side,
        stat,
        operation,
        value,
        minimum,
        maximum,
        multiplier,
    }) = source.effect
    else {
        return Ok(());
    };
    if side != expected_side || operation != expected_operation {
        return Ok(());
    }
    let affects_power = matches!(
        stat,
        DiagnosticCombatStatV1::Power | DiagnosticCombatStatV1::PowerAndDamage
    );
    let affects_damage = matches!(
        stat,
        DiagnosticCombatStatV1::Damage | DiagnosticCombatStatV1::PowerAndDamage
    );
    if !affects_power && !affects_damage {
        return Ok(());
    }
    let target = if side == DiagnosticAffectedSideV1::Player {
        origin
    } else {
        origin.other()
    };
    let amount = effect_amount(
        origin,
        value,
        multiplier,
        source.support_count,
        rounds_played,
        opponent_stars,
    )?;
    if affects_power && !opponent_cancellation.contains(DiagnosticCombatStatV1::Power) {
        power[target] = apply_u16_modifier(
            origin,
            CombatResolutionArithmeticStage::Power,
            power[target],
            operation,
            amount,
            minimum,
            maximum,
        )?;
    }
    if affects_damage && !opponent_cancellation.contains(DiagnosticCombatStatV1::Damage) {
        damage[target] = apply_u16_modifier(
            origin,
            CombatResolutionArithmeticStage::Damage,
            damage[target],
            operation,
            amount,
            minimum,
            maximum,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_attack_effect(
    origin: PlayerId,
    expected_side: DiagnosticAffectedSideV1,
    expected_operation: DiagnosticStatOperationV1,
    source: ResolutionSourcePlan,
    opponent_cancellation: CancellationMask,
    rounds_played: u8,
    opponent_stars: u16,
    attack: &mut ByPlayer<u32>,
) -> Result<(), CombatResolutionError> {
    let Some(DiagnosticCombatEffectV1::ModifyCombatStat {
        side,
        stat: DiagnosticCombatStatV1::Attack,
        operation,
        value,
        minimum,
        maximum,
        multiplier,
    }) = source.effect
    else {
        return Ok(());
    };
    if side != expected_side
        || operation != expected_operation
        || opponent_cancellation.contains(DiagnosticCombatStatV1::Attack)
    {
        return Ok(());
    }
    let target = if side == DiagnosticAffectedSideV1::Player {
        origin
    } else {
        origin.other()
    };
    let amount = effect_amount(
        origin,
        value,
        multiplier,
        source.support_count,
        rounds_played,
        opponent_stars,
    )?;
    attack[target] = apply_u32_modifier(
        origin,
        attack[target],
        operation,
        amount,
        minimum.map(u32::from),
        maximum.map(u32::from),
    )?;
    Ok(())
}

fn effect_amount(
    player: PlayerId,
    value: u16,
    multiplier: DiagnosticMagnitudeV1,
    support_count: u16,
    rounds_played: u8,
    opponent_stars: u16,
) -> Result<u32, CombatResolutionError> {
    let multiplier = match multiplier {
        DiagnosticMagnitudeV1::Fixed => 1,
        DiagnosticMagnitudeV1::SourceBonusSupport => u32::from(support_count),
        DiagnosticMagnitudeV1::Growth => u32::from(rounds_played) + 1,
        DiagnosticMagnitudeV1::Degrowth => u32::from(MAX_ROUNDS.checked_sub(rounds_played).ok_or(
            CombatResolutionError {
                player,
                stage: CombatResolutionArithmeticStage::EffectMagnitude,
            },
        )?),
        DiagnosticMagnitudeV1::OpponentStars => u32::from(opponent_stars),
    };
    u32::from(value)
        .checked_mul(multiplier)
        .ok_or(CombatResolutionError {
            player,
            stage: CombatResolutionArithmeticStage::EffectMagnitude,
        })
}

fn apply_u16_modifier(
    player: PlayerId,
    stage: CombatResolutionArithmeticStage,
    current: u16,
    operation: DiagnosticStatOperationV1,
    amount: u32,
    minimum: Option<u16>,
    maximum: Option<u16>,
) -> Result<u16, CombatResolutionError> {
    let current = u32::from(current);
    let next = match operation {
        DiagnosticStatOperationV1::Increase => match maximum.map(u32::from) {
            Some(maximum) if current < maximum => current
                .checked_add(amount)
                .ok_or(CombatResolutionError { player, stage })?
                .min(maximum),
            Some(_) => current,
            None => current
                .checked_add(amount)
                .ok_or(CombatResolutionError { player, stage })?,
        },
        DiagnosticStatOperationV1::Decrease => match minimum.map(u32::from) {
            Some(minimum) if current > minimum => current.saturating_sub(amount).max(minimum),
            Some(_) => current,
            None => current.saturating_sub(amount),
        },
    };
    u16::try_from(next).map_err(|_| CombatResolutionError { player, stage })
}

fn apply_u32_modifier(
    player: PlayerId,
    current: u32,
    operation: DiagnosticStatOperationV1,
    amount: u32,
    minimum: Option<u32>,
    maximum: Option<u32>,
) -> Result<u32, CombatResolutionError> {
    match operation {
        DiagnosticStatOperationV1::Increase => match maximum {
            Some(maximum) if current < maximum => current
                .checked_add(amount)
                .ok_or(CombatResolutionError {
                    player,
                    stage: CombatResolutionArithmeticStage::Attack,
                })
                .map(|value| value.min(maximum)),
            Some(_) => Ok(current),
            None => current.checked_add(amount).ok_or(CombatResolutionError {
                player,
                stage: CombatResolutionArithmeticStage::Attack,
            }),
        },
        DiagnosticStatOperationV1::Decrease => Ok(match minimum {
            Some(minimum) if current > minimum => current.saturating_sub(amount).max(minimum),
            Some(_) => current,
            None => current.saturating_sub(amount),
        }),
    }
}
