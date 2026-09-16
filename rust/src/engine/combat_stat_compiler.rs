//! Cold, source-independent compilation for the bounded combat-stat projection.
//!
//! Replay captures and catalog-built matches use different identity lookups, but both must
//! apply exactly the same reviewed semantic policy before producing hot-path plans.

use super::{
    CombatStatAffectedSideV1, CombatStatAttributeV1, CombatStatEffectSourceV1, CombatStatEffectV1,
    CombatStatMagnitudeV1, CombatStatOperationV1, CombatStatPredicateV1,
};
use crate::effect_registry::{
    AffectedSideV1, AttributeActionV1, AttributeAffectedV1, BetPillzLinkV1, CombatStatV1,
    CompiledEffectV1, CurrentRoundRequirementV1, EffectDefinitionV1, IndexRequirementV1,
    MagnitudeMultiplierV1, PositionRequirementV1, PreviousRoundRequirementV1, SpecialActionV1,
    StatOperationV1, StructuredEffectV1, SupportedEffectV1,
};

pub(crate) const COMBAT_STAT_COMPILER_POLICY_SEMANTIC_REVISION_V1: u16 = 4;

pub(crate) fn classify_combat_stat_effect(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    // Model-specific conditions take precedence over the registry's model-neutral output.
    // Keep the unconditional guard below as well, so a future registry compiler expansion
    // cannot silently erase a condition by returning Supported first.
    if let Some(classified) = classify_round_scaled_numeric(definition) {
        return Some(classified);
    }
    if let Some(classified) = classify_position_numeric(definition, source_kind) {
        return Some(classified);
    }
    if let Some(classified) = classify_index_numeric(definition) {
        return Some(classified);
    }
    let input = definition.structured_input();
    if input.position_requirement == PositionRequirementV1::Both && neutral_except_position(input) {
        if let CompiledEffectV1::Supported(effect) = definition.compiled() {
            if admitted_supported_effect(*effect, source_kind) {
                return Some((*effect, CombatStatPredicateV1::Always));
            }
        }
    }
    None
}

fn admitted_supported_effect(
    effect: SupportedEffectV1,
    source_kind: CombatStatEffectSourceV1,
) -> bool {
    match effect {
        SupportedEffectV1::StopOpponentBonus
        | SupportedEffectV1::CancelOpponentCombatStatModifiers { .. } => true,
        SupportedEffectV1::ModifyCombatStat {
            side,
            operation,
            maximum,
            multiplier,
            ..
        } => {
            matches!(
                (side, operation),
                (AffectedSideV1::Player, StatOperationV1::Increase)
                    | (AffectedSideV1::Opponent, StatOperationV1::Decrease)
            ) && !(operation == StatOperationV1::Increase && maximum.is_some())
                && !(source_kind == CombatStatEffectSourceV1::Ability
                    && multiplier == MagnitudeMultiplierV1::Support)
        }
    }
}

fn classify_position_numeric(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let input = definition.structured_input();
    let predicate = match input.position_requirement {
        PositionRequirementV1::Attacker if definition.description().starts_with("Courage:") => {
            CombatStatPredicateV1::OwnerMovesFirst
        }
        PositionRequirementV1::Defender if definition.description().starts_with("Reprisal:") => {
            CombatStatPredicateV1::OwnerMovesSecond
        }
        PositionRequirementV1::Both
        | PositionRequirementV1::Attacker
        | PositionRequirementV1::Defender => return None,
    };
    if !neutral_except_position(input) {
        return None;
    }
    let effect = numeric_effect(input, MagnitudeMultiplierV1::Fixed)?;
    position_description_matches(definition.description(), predicate, effect)
        .then_some((effect, predicate))
}

fn classify_index_numeric(
    definition: &EffectDefinitionV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    let predicate = match input.index_requirement {
        IndexRequirementV1::Symmetry => CombatStatPredicateV1::SelectedHandSlotsMatch,
        IndexRequirementV1::Asymmetry => CombatStatPredicateV1::SelectedHandSlotsDiffer,
        IndexRequirementV1::Any => return None,
    };
    if !neutral_except_index(input) {
        return None;
    }
    let effect = numeric_effect(input, MagnitudeMultiplierV1::Fixed)?;
    index_description_matches(definition.description(), predicate, effect)
        .then_some((effect, predicate))
}

fn classify_round_scaled_numeric(
    definition: &EffectDefinitionV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    let multiplier = match (input.is_overdrive, input.is_divide) {
        (true, false) => MagnitudeMultiplierV1::Growth,
        (false, true) => MagnitudeMultiplierV1::Degrowth,
        (false, false) | (true, true) => return None,
    };
    if !neutral_except_round_scaled_magnitude(input) {
        return None;
    }
    let effect = numeric_effect(input, multiplier)?;
    round_scaled_description_matches(definition.description(), effect)
        .then_some((effect, CombatStatPredicateV1::Always))
}

fn numeric_effect(
    input: &StructuredEffectV1,
    multiplier: MagnitudeMultiplierV1,
) -> Option<SupportedEffectV1> {
    if input.special_action != SpecialActionV1::None || input.is_support || input.value == 0 {
        return None;
    }
    let operation = match input.attribute_action {
        AttributeActionV1::Increase => StatOperationV1::Increase,
        AttributeActionV1::Decrease => StatOperationV1::Decrease,
        _ => return None,
    };
    if !matches!(
        (input.side_affected, operation),
        (AffectedSideV1::Player, StatOperationV1::Increase)
            | (AffectedSideV1::Opponent, StatOperationV1::Decrease)
    ) || (operation == StatOperationV1::Increase
        && (input.value_min != 0 || input.value_max != 0))
        || (operation == StatOperationV1::Decrease && input.value_max != 0)
    {
        return None;
    }
    let stat = match input.attribute_affected {
        AttributeAffectedV1::Attack => CombatStatV1::Attack,
        AttributeAffectedV1::Damage => CombatStatV1::Damage,
        AttributeAffectedV1::Power => CombatStatV1::Power,
        AttributeAffectedV1::PowerAndDamage => CombatStatV1::PowerAndDamage,
        _ => return None,
    };
    Some(SupportedEffectV1::ModifyCombatStat {
        side: input.side_affected,
        stat,
        operation,
        value: input.value,
        minimum: (operation == StatOperationV1::Decrease).then_some(input.value_min),
        maximum: None,
        multiplier,
    })
}

fn neutral_except_round_scaled_magnitude(input: &StructuredEffectV1) -> bool {
    input.position_requirement == PositionRequirementV1::Both
        && input.previous_round_requirement == PreviousRoundRequirementV1::Any
        && input.current_round_requirement == CurrentRoundRequirementV1::Any
        && input.index_requirement == IndexRequirementV1::Any
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
        && input.bet_pillz_link == BetPillzLinkV1::No
        && input.value_condition == 0
        && !input.is_inverted
        && !input.is_support
        && !input.is_anti_support
        && !input.is_life_linked
        && !input.is_pillz_linked
        && !input.is_lost_life_linked
        && !input.is_lost_pillz_linked
        && !input.is_opponent_stars_linked
        && !input.is_clanmates_count_linked
        && !input.is_anti_clanmates_count_linked
        && !input.is_permanent
        && !input.is_immediate_permanent
}

fn neutral_except_position(input: &StructuredEffectV1) -> bool {
    input.previous_round_requirement == PreviousRoundRequirementV1::Any
        && input.current_round_requirement == CurrentRoundRequirementV1::Any
        && input.index_requirement == IndexRequirementV1::Any
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
        && input.bet_pillz_link == BetPillzLinkV1::No
        && input.value_condition == 0
        && !input.is_inverted
        && !input.is_anti_support
        && !input.is_overdrive
        && !input.is_divide
        && !input.is_life_linked
        && !input.is_pillz_linked
        && !input.is_lost_life_linked
        && !input.is_lost_pillz_linked
        && !input.is_opponent_stars_linked
        && !input.is_clanmates_count_linked
        && !input.is_anti_clanmates_count_linked
        && !input.is_permanent
        && !input.is_immediate_permanent
}

fn neutral_except_index(input: &StructuredEffectV1) -> bool {
    input.position_requirement == PositionRequirementV1::Both
        && input.previous_round_requirement == PreviousRoundRequirementV1::Any
        && input.current_round_requirement == CurrentRoundRequirementV1::Any
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
        && input.bet_pillz_link == BetPillzLinkV1::No
        && input.value_condition == 0
        && !input.is_inverted
        && !input.is_anti_support
        && !input.is_overdrive
        && !input.is_divide
        && !input.is_life_linked
        && !input.is_pillz_linked
        && !input.is_lost_life_linked
        && !input.is_lost_pillz_linked
        && !input.is_opponent_stars_linked
        && !input.is_clanmates_count_linked
        && !input.is_anti_clanmates_count_linked
        && !input.is_permanent
        && !input.is_immediate_permanent
}

fn position_description_matches(
    description: &str,
    predicate: CombatStatPredicateV1,
    effect: SupportedEffectV1,
) -> bool {
    let prefix = match predicate {
        CombatStatPredicateV1::OwnerMovesFirst => "Courage: ",
        CombatStatPredicateV1::OwnerMovesSecond => "Reprisal: ",
        CombatStatPredicateV1::Always
        | CombatStatPredicateV1::SelectedHandSlotsMatch
        | CombatStatPredicateV1::SelectedHandSlotsDiffer => return false,
    };
    numeric_description_body_matches(
        description.strip_prefix(prefix).unwrap_or(""),
        effect,
        MagnitudeMultiplierV1::Fixed,
    )
}

fn index_description_matches(
    description: &str,
    predicate: CombatStatPredicateV1,
    effect: SupportedEffectV1,
) -> bool {
    let prefix = match predicate {
        CombatStatPredicateV1::SelectedHandSlotsMatch => "Symmetry: ",
        CombatStatPredicateV1::SelectedHandSlotsDiffer => "Asymmetry: ",
        CombatStatPredicateV1::Always
        | CombatStatPredicateV1::OwnerMovesFirst
        | CombatStatPredicateV1::OwnerMovesSecond => return false,
    };
    numeric_description_body_matches(
        description.strip_prefix(prefix).unwrap_or(""),
        effect,
        MagnitudeMultiplierV1::Fixed,
    )
}

fn round_scaled_description_matches(description: &str, effect: SupportedEffectV1) -> bool {
    let multiplier = match effect {
        SupportedEffectV1::ModifyCombatStat { multiplier, .. } => multiplier,
        SupportedEffectV1::StopOpponentBonus
        | SupportedEffectV1::CancelOpponentCombatStatModifiers { .. } => return false,
    };
    let prefix = match multiplier {
        MagnitudeMultiplierV1::Growth => "Growth: ",
        MagnitudeMultiplierV1::Degrowth => "Degrowth: ",
        MagnitudeMultiplierV1::Fixed | MagnitudeMultiplierV1::Support => return false,
    };
    numeric_description_body_matches(
        description.strip_prefix(prefix).unwrap_or(""),
        effect,
        multiplier,
    )
}

fn numeric_description_body_matches(
    body: &str,
    effect: SupportedEffectV1,
    expected_multiplier: MagnitudeMultiplierV1,
) -> bool {
    let SupportedEffectV1::ModifyCombatStat {
        side,
        stat,
        operation,
        value,
        minimum,
        maximum: None,
        multiplier,
    } = effect
    else {
        return false;
    };
    if multiplier != expected_multiplier {
        return false;
    }
    match (side, stat, operation, minimum) {
        (AffectedSideV1::Player, CombatStatV1::Power, StatOperationV1::Increase, None) => {
            body == format!("Power +{value}")
        }
        (AffectedSideV1::Player, CombatStatV1::Damage, StatOperationV1::Increase, None) => {
            body == format!("Damage +{value}")
        }
        (AffectedSideV1::Player, CombatStatV1::Attack, StatOperationV1::Increase, None) => {
            body == format!("Attack +{value}")
        }
        (AffectedSideV1::Player, CombatStatV1::PowerAndDamage, StatOperationV1::Increase, None) => {
            body == format!("Power And Damage +{value}")
                || body == format!("Power And Damage + {value}")
                || body == format!("Pow. & Dam. +{value}")
        }
        (AffectedSideV1::Opponent, CombatStatV1::Power, StatOperationV1::Decrease, Some(min)) => {
            body == format!("-{value} Opp Power, Min {min}")
                || body == format!("-{value} Opp. Power, Min {min}")
        }
        (AffectedSideV1::Opponent, CombatStatV1::Damage, StatOperationV1::Decrease, Some(min)) => {
            body == format!("-{value} Opp Damage, Min {min}")
                || body == format!("-{value} Opp. Damage, Min {min}")
        }
        (AffectedSideV1::Opponent, CombatStatV1::Attack, StatOperationV1::Decrease, Some(min)) => {
            body == format!("-{value} Opp Attack, Min {min}")
                || body == format!("-{value} Opp. Attack, Min {min}")
        }
        (
            AffectedSideV1::Opponent,
            CombatStatV1::PowerAndDamage,
            StatOperationV1::Decrease,
            Some(min),
        ) => {
            body == format!("-{value} Opp Power And Damage, Min {min}")
                || body == format!("-{value} Opp Pow. & Dam., Min {min}")
                || body == format!("-{value} Opp Pow. And Dam., Min {min}")
                || body == format!("-{value} Opp Pow. & Dmg,min {min}")
        }
        _ => false,
    }
}

pub(crate) fn compact_effect(effect: SupportedEffectV1) -> Option<CombatStatEffectV1> {
    match effect {
        SupportedEffectV1::ModifyCombatStat {
            side,
            stat,
            operation,
            value,
            minimum,
            maximum,
            multiplier,
        } => Some(CombatStatEffectV1::ModifyCombatStat {
            side: match side {
                AffectedSideV1::Opponent => CombatStatAffectedSideV1::Opponent,
                AffectedSideV1::Player => CombatStatAffectedSideV1::Player,
                AffectedSideV1::Both => return None,
            },
            stat: compact_stat(stat),
            operation: match operation {
                StatOperationV1::Decrease => CombatStatOperationV1::Decrease,
                StatOperationV1::Increase => CombatStatOperationV1::Increase,
            },
            value,
            minimum,
            maximum,
            multiplier: match multiplier {
                MagnitudeMultiplierV1::Fixed => CombatStatMagnitudeV1::Fixed,
                MagnitudeMultiplierV1::Support => CombatStatMagnitudeV1::SourceBonusSupport,
                MagnitudeMultiplierV1::Growth => CombatStatMagnitudeV1::Growth,
                MagnitudeMultiplierV1::Degrowth => CombatStatMagnitudeV1::Degrowth,
            },
        }),
        SupportedEffectV1::StopOpponentBonus => Some(CombatStatEffectV1::StopOpponentBonus),
        SupportedEffectV1::CancelOpponentCombatStatModifiers { stat } => {
            Some(CombatStatEffectV1::CancelOpponentCombatStatModifiers {
                stat: compact_stat(stat),
            })
        }
    }
}

const fn compact_stat(stat: CombatStatV1) -> CombatStatAttributeV1 {
    match stat {
        CombatStatV1::Attack => CombatStatAttributeV1::Attack,
        CombatStatV1::Damage => CombatStatAttributeV1::Damage,
        CombatStatV1::Power => CombatStatAttributeV1::Power,
        CombatStatV1::PowerAndDamage => CombatStatAttributeV1::PowerAndDamage,
    }
}
