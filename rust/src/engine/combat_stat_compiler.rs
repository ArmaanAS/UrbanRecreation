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

pub(crate) const COMBAT_STAT_COMPILER_POLICY_SEMANTIC_REVISION_V1: u16 = 14;

/// Recognize only the literal, immediate end-of-round Victory Life grammar.  Unlike the
/// identity-locked Pillz slices below, this is deliberately generic: any registry
/// definition with the complete reviewed structured shape may supply its positive fixed
/// magnitude, whether it came from an Ability or a Bonus.
pub(crate) fn classify_victory_life(
    definition: &EffectDefinitionV1,
    _source_kind: CombatStatEffectSourceV1,
) -> Option<u16> {
    let input = definition.structured_input();
    (has_victory_life_shape(definition)
        && definition.description() == format!("+{} Life", input.value))
    .then_some(input.value)
}

/// Structural half of the Victory Life boundary. Replay preparation uses this to reject
/// exact-shape sources whose description is malformed instead of silently disabling them.
pub(crate) fn has_victory_life_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0 && victory_life_shape_matches(input)
}

/// Recognize the immediate, surviving Defeat Life grammar. This deliberately admits card
/// abilities only: ordinary resource gains do not pay after the owner has been KO'd.
pub(crate) fn classify_defeat_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<u16> {
    let input = definition.structured_input();
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_defeat_life_shape(definition)
        && definition.description() == format!("Defeat: +{} Life", input.value))
    .then_some(input.value)
}

/// Structural half of the ordinary Defeat Life boundary. Its `value_min=1` is distinct
/// from Reanimate's zero-life exception and makes malformed near-misses rejectable.
pub(crate) fn has_defeat_life_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0 && defeat_life_shape_matches(input, 1)
}

/// Recognize the immediate Reanimate grammar. Reanimate is a card ability which revives
/// from zero after a loss; the commit phase owns that exceptional life-zero behavior.
pub(crate) fn classify_reanimate_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<u16> {
    let input = definition.structured_input();
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_reanimate_life_shape(definition)
        && definition.description() == format!("Reanimate: +{} Life", input.value))
    .then_some(input.value)
}

/// Structural half of the Reanimate boundary. `value_min=0` is its only intentional
/// difference from ordinary Defeat Life.
pub(crate) fn has_reanimate_life_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0 && defeat_life_shape_matches(input, 0)
}

/// Strictly recognize the three replay identities audited for the diagnostic's Defeat
/// recovery effect. This intentionally does not broaden the registry compiler's generic
/// `RecoverPillz` support.
pub(crate) fn classify_defeat_recover_pillz(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> bool {
    let identity_matches = match source_kind {
        CombatStatEffectSourceV1::Ability => matches!(definition.id(), 729 | 1418),
        CombatStatEffectSourceV1::Bonus => definition.id() == 577,
    };
    identity_matches
        && definition.description() == "Defeat: Recover 2 Pillz Out Of 3"
        && defeat_recover_shape_matches(definition.structured_input())
}

/// Strictly recognize the audited Victory Or Defeat end-of-round Pillz sources. This is
/// intentionally separate from ordinary Pillz modifiers and Defeat recovery. The same
/// printed record appears in several unrelated sources, so identity and source kind are
/// both part of the executable grammar.
pub(crate) fn classify_victory_or_defeat_pillz(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> bool {
    victory_or_defeat_pillz_identity_matches(source_kind, definition.id())
        && definition.description() == "Victory Or Defeat : +1 Pillz"
        && victory_or_defeat_pillz_shape_matches(definition.structured_input())
}

/// Shared identity gate for compiler output and caller-provided compact plans.
pub(crate) fn victory_or_defeat_pillz_identity_matches(
    source_kind: CombatStatEffectSourceV1,
    definition_id: u32,
) -> bool {
    matches!(
        (source_kind, definition_id),
        (CombatStatEffectSourceV1::Bonus, 1034)
            | (
                CombatStatEffectSourceV1::Ability,
                1034 | 1375 | 4111 | 5085 | 5520
            )
    )
}

/// Strictly recognize Argos' printed Defeat Pillz effect. Its cap is applied after a
/// live clan bonus in the shared END phase, so it requires a distinct typed path.
pub(crate) fn classify_argos_defeat_capped_pillz(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> bool {
    argos_defeat_capped_pillz_identity_matches(source_kind, definition.id())
        && definition.description() == "Defeat: +2 Pillz Max. 11"
        && argos_defeat_capped_pillz_shape_matches(definition.structured_input())
}

/// Shared identity gate for compiler output and caller-provided compact plans.
pub(crate) fn argos_defeat_capped_pillz_identity_matches(
    source_kind: CombatStatEffectSourceV1,
    definition_id: u32,
) -> bool {
    (source_kind, definition_id) == (CombatStatEffectSourceV1::Ability, 1158)
}

pub(crate) fn classify_combat_stat_effect(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    // Recovery has its own post-round execution channel. Keep it out of this combat-stat
    // return type so neither generic numeric admission nor cancellation can reinterpret it.
    if classify_defeat_recover_pillz(definition, source_kind) {
        return None;
    }
    if classify_argos_defeat_capped_pillz(definition, source_kind) {
        return None;
    }
    if classify_victory_life(definition, source_kind).is_some() {
        return None;
    }
    if classify_defeat_life(definition, source_kind).is_some()
        || classify_reanimate_life(definition, source_kind).is_some()
    {
        return None;
    }
    // Victory Or Defeat Pillz likewise has its own post-round execution channel.
    // Keep it out of generic numeric classification in case the registry compiler later
    // broadens its Pillz support.
    if classify_victory_or_defeat_pillz(definition, source_kind) {
        return None;
    }
    // Model-specific conditions take precedence over the registry's model-neutral output.
    // Keep the unconditional guard below as well, so a future registry compiler expansion
    // cannot silently erase a condition by returning Supported first.
    if let Some(classified) = classify_equalizer_numeric(definition) {
        return Some(classified);
    }
    if let Some(classified) = classify_round_scaled_numeric(definition) {
        return Some(classified);
    }
    if let Some(classified) = classify_position_numeric(definition, source_kind) {
        return Some(classified);
    }
    if let Some(classified) = classify_previous_round_numeric(definition, source_kind) {
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
        SupportedEffectV1::StopOpponentAbility
        | SupportedEffectV1::StopOpponentBonus
        | SupportedEffectV1::CancelOpponentCombatStatModifiers { .. } => true,
        SupportedEffectV1::ModifyCombatStat {
            side,
            stat,
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
                // Support: Power And Damage has no reviewed observed record. Keep it
                // outside this narrow projection even though the registry can represent it.
                && !(source_kind == CombatStatEffectSourceV1::Ability
                    && multiplier == MagnitudeMultiplierV1::Support
                    && matches!(stat, CombatStatV1::PowerAndDamage))
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

fn classify_previous_round_numeric(
    definition: &EffectDefinitionV1,
    _source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    // This exact fixed numeric Confidence/Revenge grammar is observed for both card
    // abilities and bonuses. Other temporal bonus shapes remain outside the projection.
    let input = definition.structured_input();
    let predicate = match input.previous_round_requirement {
        PreviousRoundRequirementV1::Win => CombatStatPredicateV1::OwnerWonPreviousRound,
        PreviousRoundRequirementV1::Lose => CombatStatPredicateV1::OwnerLostPreviousRound,
        PreviousRoundRequirementV1::Any => return None,
    };
    if !neutral_except_previous_round(input) {
        return None;
    }
    let effect = numeric_effect(input, MagnitudeMultiplierV1::Fixed)?;
    previous_round_description_matches(definition.description(), predicate, effect)
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

fn classify_equalizer_numeric(
    definition: &EffectDefinitionV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    if !input.is_opponent_stars_linked || !neutral_except_equalizer(input) {
        return None;
    }
    let effect = numeric_effect(input, MagnitudeMultiplierV1::OpponentStars)?;
    equalizer_description_matches(definition.description(), effect)
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

fn neutral_except_equalizer(input: &StructuredEffectV1) -> bool {
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
        && !input.is_overdrive
        && !input.is_divide
        && !input.is_life_linked
        && !input.is_pillz_linked
        && !input.is_lost_life_linked
        && !input.is_lost_pillz_linked
        && input.is_opponent_stars_linked
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

fn neutral_except_previous_round(input: &StructuredEffectV1) -> bool {
    input.position_requirement == PositionRequirementV1::Both
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

fn defeat_recover_shape_matches(input: &StructuredEffectV1) -> bool {
    input.value == 2
        && input.value_min == 3
        && input.value_max == 0
        && input.value_condition == 0
        && input.position_requirement == PositionRequirementV1::Both
        && input.previous_round_requirement == PreviousRoundRequirementV1::Any
        && input.current_round_requirement == CurrentRoundRequirementV1::Lose
        && input.index_requirement == IndexRequirementV1::Any
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
        && input.bet_pillz_link == BetPillzLinkV1::No
        && input.side_affected == AffectedSideV1::Player
        && input.attribute_affected == AttributeAffectedV1::Pillz
        && input.attribute_action == AttributeActionV1::Increase
        && input.special_action == SpecialActionV1::RecoverPillz
        && !input.is_inverted
        && !input.is_support
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

fn victory_life_shape_matches(input: &StructuredEffectV1) -> bool {
    input.value_min == 0
        && input.value_max == 0
        && input.value_condition == 0
        && input.position_requirement == PositionRequirementV1::Both
        && input.previous_round_requirement == PreviousRoundRequirementV1::Any
        && input.current_round_requirement == CurrentRoundRequirementV1::Win
        && input.index_requirement == IndexRequirementV1::Any
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
        && input.bet_pillz_link == BetPillzLinkV1::No
        && input.side_affected == AffectedSideV1::Player
        && input.attribute_affected == AttributeAffectedV1::Life
        && input.attribute_action == AttributeActionV1::Increase
        && input.special_action == SpecialActionV1::None
        && !input.is_inverted
        && !input.is_support
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

fn defeat_life_shape_matches(input: &StructuredEffectV1, minimum: u16) -> bool {
    input.value_min == minimum
        && input.value_max == 0
        && input.value_condition == 0
        && input.position_requirement == PositionRequirementV1::Both
        && input.previous_round_requirement == PreviousRoundRequirementV1::Any
        && input.current_round_requirement == CurrentRoundRequirementV1::Lose
        && input.index_requirement == IndexRequirementV1::Any
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
        && input.bet_pillz_link == BetPillzLinkV1::No
        && input.side_affected == AffectedSideV1::Player
        && input.attribute_affected == AttributeAffectedV1::Life
        && input.attribute_action == AttributeActionV1::Increase
        && input.special_action == SpecialActionV1::None
        && !input.is_inverted
        && !input.is_support
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

fn victory_or_defeat_pillz_shape_matches(input: &StructuredEffectV1) -> bool {
    input.value == 1
        && input.value_min == 0
        && input.value_max == 0
        && input.value_condition == 0
        && input.position_requirement == PositionRequirementV1::Both
        && input.previous_round_requirement == PreviousRoundRequirementV1::Any
        && input.current_round_requirement == CurrentRoundRequirementV1::Any
        && input.index_requirement == IndexRequirementV1::Any
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
        && input.bet_pillz_link == BetPillzLinkV1::No
        && input.side_affected == AffectedSideV1::Player
        && input.attribute_affected == AttributeAffectedV1::Pillz
        && input.attribute_action == AttributeActionV1::Increase
        && input.special_action == SpecialActionV1::None
        && !input.is_inverted
        && !input.is_support
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

fn argos_defeat_capped_pillz_shape_matches(input: &StructuredEffectV1) -> bool {
    input.value == 2
        && input.value_min == 0
        && input.value_max == 11
        && input.value_condition == 0
        && input.position_requirement == PositionRequirementV1::Both
        && input.previous_round_requirement == PreviousRoundRequirementV1::Any
        && input.current_round_requirement == CurrentRoundRequirementV1::Lose
        && input.index_requirement == IndexRequirementV1::Any
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
        && input.bet_pillz_link == BetPillzLinkV1::No
        && input.side_affected == AffectedSideV1::Player
        && input.attribute_affected == AttributeAffectedV1::Pillz
        && input.attribute_action == AttributeActionV1::Increase
        && input.special_action == SpecialActionV1::None
        && !input.is_inverted
        && !input.is_support
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
        | CombatStatPredicateV1::OwnerWonPreviousRound
        | CombatStatPredicateV1::OwnerLostPreviousRound
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
        | CombatStatPredicateV1::OwnerMovesSecond
        | CombatStatPredicateV1::OwnerWonPreviousRound
        | CombatStatPredicateV1::OwnerLostPreviousRound => return false,
    };
    numeric_description_body_matches(
        description.strip_prefix(prefix).unwrap_or(""),
        effect,
        MagnitudeMultiplierV1::Fixed,
    )
}

fn previous_round_description_matches(
    description: &str,
    predicate: CombatStatPredicateV1,
    effect: SupportedEffectV1,
) -> bool {
    let body = match predicate {
        CombatStatPredicateV1::OwnerWonPreviousRound => description
            .strip_prefix("Confidence: ")
            .or_else(|| description.strip_prefix("Confidence : ")),
        CombatStatPredicateV1::OwnerLostPreviousRound => description.strip_prefix("Revenge: "),
        CombatStatPredicateV1::Always
        | CombatStatPredicateV1::OwnerMovesFirst
        | CombatStatPredicateV1::OwnerMovesSecond
        | CombatStatPredicateV1::SelectedHandSlotsMatch
        | CombatStatPredicateV1::SelectedHandSlotsDiffer => None,
    };
    body.is_some_and(|body| {
        numeric_description_body_matches(body, effect, MagnitudeMultiplierV1::Fixed)
    })
}

fn round_scaled_description_matches(description: &str, effect: SupportedEffectV1) -> bool {
    let multiplier = match effect {
        SupportedEffectV1::ModifyCombatStat { multiplier, .. } => multiplier,
        SupportedEffectV1::StopOpponentAbility
        | SupportedEffectV1::StopOpponentBonus
        | SupportedEffectV1::CancelOpponentCombatStatModifiers { .. } => return false,
    };
    let prefix = match multiplier {
        MagnitudeMultiplierV1::Growth => "Growth: ",
        MagnitudeMultiplierV1::Degrowth => "Degrowth: ",
        MagnitudeMultiplierV1::Fixed
        | MagnitudeMultiplierV1::Support
        | MagnitudeMultiplierV1::OpponentStars => return false,
    };
    numeric_description_body_matches(
        description.strip_prefix(prefix).unwrap_or(""),
        effect,
        multiplier,
    )
}

fn equalizer_description_matches(description: &str, effect: SupportedEffectV1) -> bool {
    numeric_description_body_matches(
        description.strip_prefix("Equalizer: ").unwrap_or(""),
        effect,
        MagnitudeMultiplierV1::OpponentStars,
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
                MagnitudeMultiplierV1::OpponentStars => CombatStatMagnitudeV1::OpponentStars,
            },
        }),
        SupportedEffectV1::StopOpponentAbility => Some(CombatStatEffectV1::StopOpponentAbility),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect_registry::EffectRegistryV1;
    use std::collections::BTreeSet;
    use std::fs::File;
    use std::path::PathBuf;

    fn registry() -> EffectRegistryV1 {
        EffectRegistryV1::load(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json"),
        )
        .unwrap()
    }

    #[test]
    fn observed_previous_round_inventory_is_exact_and_fail_closed() {
        let registry = registry();
        let admitted = BTreeSet::from([
            463, 465, 478, 520, 553, 555, 556, 560, 585, 634, 784, 801, 859, 883, 884, 921, 938,
            965, 1053, 1091, 1107, 1278, 1286, 1303, 1395, 1417, 1839, 2628, 2657, 3827, 3829,
            4316, 4399, 4464, 4623, 4711, 4838, 5406, 5881,
        ]);
        let deferred = BTreeSet::from([
            490, 589, 814, 1409, 1643, 1652, 1661, 1680, 1702, 1713, 1719, 1751, 1810, 2113, 2582,
            3016, 3301, 3546, 4301, 4449, 4972,
        ]);
        let observed: BTreeSet<_> = registry
            .iter()
            .filter_map(|(id, definition)| {
                (definition.structured_input().previous_round_requirement
                    != PreviousRoundRequirementV1::Any)
                    .then_some(id)
            })
            .collect();
        assert_eq!(observed, admitted.union(&deferred).copied().collect());

        for source in [
            CombatStatEffectSourceV1::Ability,
            CombatStatEffectSourceV1::Bonus,
        ] {
            let classified: BTreeSet<_> = registry
                .iter()
                .filter_map(|(id, definition)| {
                    classify_combat_stat_effect(definition, source).and_then(|(_, predicate)| {
                        matches!(
                            predicate,
                            CombatStatPredicateV1::OwnerWonPreviousRound
                                | CombatStatPredicateV1::OwnerLostPreviousRound
                        )
                        .then_some(id)
                    })
                })
                .collect();
            assert_eq!(classified, admitted, "{source:?}");
        }
    }

    #[test]
    fn defeat_recover_admits_only_the_audited_identity_and_shape() {
        let registry = registry();
        assert!(classify_defeat_recover_pillz(
            registry
                .lookup_capture(577, "Defeat: Recover 2 Pillz Out Of 3")
                .unwrap(),
            CombatStatEffectSourceV1::Bonus,
        ));
        assert!(classify_defeat_recover_pillz(
            registry
                .lookup_capture(1418, "Defeat: Recover 2 Pillz Out Of 3")
                .unwrap(),
            CombatStatEffectSourceV1::Ability,
        ));
        assert!(!classify_defeat_recover_pillz(
            registry
                .lookup_capture(577, "Defeat: Recover 2 Pillz Out Of 3")
                .unwrap(),
            CombatStatEffectSourceV1::Ability,
        ));
        assert!(!classify_defeat_recover_pillz(
            registry
                .lookup_capture(1418, "Defeat: Recover 2 Pillz Out Of 3")
                .unwrap(),
            CombatStatEffectSourceV1::Bonus,
        ));
        assert!(classify_defeat_recover_pillz(
            registry
                .lookup_capture(729, "Defeat: Recover 2 Pillz Out Of 3")
                .unwrap(),
            CombatStatEffectSourceV1::Ability,
        ));
        for id in [2475] {
            let definition = registry
                .lookup_capture(id, "Defeat: Recover 2 Pillz Out Of 3")
                .unwrap();
            assert!(!classify_defeat_recover_pillz(
                definition,
                CombatStatEffectSourceV1::Bonus,
            ));
            assert!(!classify_defeat_recover_pillz(
                definition,
                CombatStatEffectSourceV1::Ability,
            ));
        }
    }

    #[test]
    fn victory_or_defeat_admits_only_the_exact_source_identity_and_shape() {
        let registry = registry();
        let bonus = registry
            .lookup_capture(1034, "Victory Or Defeat : +1 Pillz")
            .unwrap();
        assert!(classify_victory_or_defeat_pillz(
            bonus,
            CombatStatEffectSourceV1::Bonus,
        ));
        assert!(classify_victory_or_defeat_pillz(
            bonus,
            CombatStatEffectSourceV1::Ability,
        ));
        assert!(!classify_victory_or_defeat_pillz(
            registry
                .lookup_capture(1035, "Defeat: Recover 1 Pillz Out Of 2")
                .unwrap(),
            CombatStatEffectSourceV1::Bonus,
        ));
        for id in [1034, 1375, 4111, 5085, 5520] {
            let definition = registry
                .lookup_capture(id, "Victory Or Defeat : +1 Pillz")
                .unwrap();
            assert!(
                classify_victory_or_defeat_pillz(definition, CombatStatEffectSourceV1::Ability),
                "ability id={id}"
            );
        }
        assert!(!classify_victory_or_defeat_pillz(
            registry
                .lookup_capture(1375, "Victory Or Defeat : +1 Pillz")
                .unwrap(),
            CombatStatEffectSourceV1::Bonus,
        ));

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let mut malformed: serde_json::Value =
            serde_json::from_reader(File::open(path).unwrap()).unwrap();
        malformed["1034"]["abilityData"]["valueMin"] = serde_json::json!(1);
        let malformed =
            EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                .unwrap();
        assert!(!classify_victory_or_defeat_pillz(
            malformed
                .lookup_capture(1034, "Victory Or Defeat : +1 Pillz")
                .unwrap(),
            CombatStatEffectSourceV1::Bonus,
        ));
    }

    #[test]
    fn victory_life_requires_positive_literal_description_and_complete_neutral_shape() {
        let registry = registry();
        let dave = registry.lookup_capture(888, "+2 Life").unwrap();
        for source in [
            CombatStatEffectSourceV1::Ability,
            CombatStatEffectSourceV1::Bonus,
        ] {
            assert_eq!(classify_victory_life(dave, source), Some(2));
        }

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value = serde_json::from_reader(File::open(path).unwrap()).unwrap();
        for (field, value) in [
            ("valueMin", serde_json::json!(1)),
            ("currentRoundRequirement", serde_json::json!("any")),
            ("isLifeLinked", serde_json::json!(true)),
            ("isPermanent", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed["888"]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            assert_eq!(
                classify_victory_life(
                    malformed.lookup_capture(888, "+2 Life").unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                None,
                "mutated field {field}"
            );
        }
        let mut malformed = source;
        malformed["888"]["description"] = serde_json::json!("+2 Life ");
        let malformed =
            EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                .unwrap();
        assert_eq!(
            classify_victory_life(
                malformed.lookup_capture(888, "+2 Life ").unwrap(),
                CombatStatEffectSourceV1::Ability,
            ),
            None
        );
    }

    #[test]
    fn defeat_life_and_reanimate_require_their_exact_ability_only_shapes() {
        let registry = registry();
        let defeat = registry.lookup_capture(862, "Defeat: +2 Life").unwrap();
        assert_eq!(
            classify_defeat_life(defeat, CombatStatEffectSourceV1::Ability),
            Some(2)
        );
        assert_eq!(
            classify_defeat_life(defeat, CombatStatEffectSourceV1::Bonus),
            None
        );
        let reanimate = registry.lookup_capture(4951, "Reanimate: +2 Life").unwrap();
        assert_eq!(
            classify_reanimate_life(reanimate, CombatStatEffectSourceV1::Ability),
            Some(2)
        );
        assert_eq!(
            classify_reanimate_life(reanimate, CombatStatEffectSourceV1::Bonus),
            None
        );

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value = serde_json::from_reader(File::open(path).unwrap()).unwrap();
        for (id, description, classifier, field, value) in [
            (
                "862",
                "Defeat: +2 Life",
                classify_defeat_life
                    as fn(&EffectDefinitionV1, CombatStatEffectSourceV1) -> Option<u16>,
                "valueMin",
                serde_json::json!(0),
            ),
            (
                "4951",
                "Reanimate: +2 Life",
                classify_reanimate_life
                    as fn(&EffectDefinitionV1, CombatStatEffectSourceV1) -> Option<u16>,
                "valueMin",
                serde_json::json!(1),
            ),
        ] {
            let mut malformed = source.clone();
            malformed[id]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            assert_eq!(
                classifier(
                    malformed
                        .lookup_capture(id.parse().unwrap(), description)
                        .unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                None,
                "id={id} mutated {field}"
            );
        }
        for (field, value) in [
            ("currentRoundRequirement", serde_json::json!("any")),
            ("isLifeLinked", serde_json::json!(true)),
            ("isPermanent", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed["4951"]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            assert_eq!(
                classify_reanimate_life(
                    malformed
                        .lookup_capture(4951, "Reanimate: +2 Life")
                        .unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                None,
                "mutated Reanimate {field}"
            );
        }
        let mut malformed = source;
        malformed["862"]["description"] = serde_json::json!("Defeat: +2 Life ");
        let malformed =
            EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                .unwrap();
        assert_eq!(
            classify_defeat_life(
                malformed.lookup_capture(862, "Defeat: +2 Life ").unwrap(),
                CombatStatEffectSourceV1::Ability,
            ),
            None
        );
    }

    #[test]
    fn argos_defeat_capped_pillz_admits_only_ability_1158_and_its_full_shape() {
        let registry = registry();
        let argos = registry
            .lookup_capture(1158, "Defeat: +2 Pillz Max. 11")
            .unwrap();
        assert!(classify_argos_defeat_capped_pillz(
            argos,
            CombatStatEffectSourceV1::Ability,
        ));
        assert!(!classify_argos_defeat_capped_pillz(
            argos,
            CombatStatEffectSourceV1::Bonus,
        ));
        assert!(!classify_argos_defeat_capped_pillz(
            registry
                .lookup_capture(1034, "Victory Or Defeat : +1 Pillz")
                .unwrap(),
            CombatStatEffectSourceV1::Ability,
        ));

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value = serde_json::from_reader(File::open(path).unwrap()).unwrap();
        for (field, value) in [
            ("valueMax", serde_json::json!(12)),
            ("currentRoundRequirement", serde_json::json!("any")),
            ("specialAction", serde_json::json!("recover_pillz")),
            ("isSupport", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed["1158"]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            assert!(
                !classify_argos_defeat_capped_pillz(
                    malformed
                        .lookup_capture(1158, "Defeat: +2 Pillz Max. 11")
                        .unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                "mutated field {field}"
            );
        }
    }
}
