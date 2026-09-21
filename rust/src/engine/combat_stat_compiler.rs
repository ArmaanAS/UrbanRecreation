//! Cold, source-independent compilation for the bounded combat-stat projection.
//!
//! Replay captures and catalog-built matches use different identity lookups, but both must
//! apply exactly the same reviewed semantic policy before producing hot-path plans.

use super::CopiedSourceKindV1;
use super::{
    CombatStatAffectedSideV1, CombatStatAttributeV1, CombatStatEffectSourceV1, CombatStatEffectV1,
    CombatStatMagnitudeV1, CombatStatOperationV1, CombatStatPredicateV1,
};

/// Every source-copying Copy grammar this projection admits, as the exact printed text it
/// requires. `Reprisal:`, `Revenge:` and `Asymmetry:` reuse predicates the projection
/// already resolves before a round is prepared, so they gate the adoption itself rather
/// than needing new context. `Unison` (a draw-level clan-mate count), `Confidence`,
/// `Bet > N` and the clan-gated `Asy.` variant keep their own deferred grammars. Note the
/// site's own inconsistent punctuation: a copied Bonus loses the second colon under
/// `Reprisal`/`Revenge` but keeps it under `Asymmetry`.
const COPY_OPPONENT_SOURCE_GRAMMARS: [(&str, CopiedSourceKindV1, CombatStatPredicateV1); 8] = [
    (
        "Copy: Opp. Ability",
        CopiedSourceKindV1::Ability,
        CombatStatPredicateV1::Always,
    ),
    (
        "Copy: Opp. Bonus",
        CopiedSourceKindV1::Bonus,
        CombatStatPredicateV1::Always,
    ),
    (
        "Reprisal: Copy: Opp. Ability",
        CopiedSourceKindV1::Ability,
        CombatStatPredicateV1::OwnerMovesSecond,
    ),
    (
        "Reprisal: Copy Opp. Bonus",
        CopiedSourceKindV1::Bonus,
        CombatStatPredicateV1::OwnerMovesSecond,
    ),
    (
        "Revenge: Copy: Opp. Ability",
        CopiedSourceKindV1::Ability,
        CombatStatPredicateV1::OwnerLostPreviousRound,
    ),
    (
        "Revenge: Copy Opp. Bonus",
        CopiedSourceKindV1::Bonus,
        CombatStatPredicateV1::OwnerLostPreviousRound,
    ),
    (
        "Asymmetry: Copy: Opp. Ability",
        CopiedSourceKindV1::Ability,
        CombatStatPredicateV1::SelectedHandSlotsDiffer,
    ),
    (
        "Asymmetry: Copy: Opp. Bonus",
        CopiedSourceKindV1::Bonus,
        CombatStatPredicateV1::SelectedHandSlotsDiffer,
    ),
];

/// True for every printed text the Copy compiler recognizes, so the catalog boundary can
/// route a source here without repeating the table.
pub(crate) fn is_copy_opponent_source_description(description: &str) -> bool {
    COPY_OPPONENT_SOURCE_GRAMMARS
        .iter()
        .any(|(text, _, _)| *text == description)
}
use crate::effect_registry::{
    AffectedSideV1, AttributeActionV1, AttributeAffectedV1, BetPillzLinkV1, CombatStatV1,
    CompiledEffectV1, CurrentRoundRequirementV1, EffectDefinitionV1, IndexRequirementV1,
    MagnitudeMultiplierV1, PositionRequirementV1, PreviousRoundRequirementV1, SpecialActionV1,
    StatOperationV1, StructuredEffectV1, SupportedEffectV1,
};

pub(crate) const COMBAT_STAT_COMPILER_POLICY_SEMANTIC_REVISION_V1: u16 = 38;

/// Recognize the admitted Copy grammars. Like generic Victory Life these are admitted by
/// exact description and structured shape rather than a fixed id list, because the registry
/// carries many structurally identical Copy definitions. The condition, when there is one,
/// gates whether the opposing source is adopted at all; the adopted plan then keeps its own
/// predicate, so a copied Confidence effect still has to satisfy the copier's own history.
pub(crate) fn classify_copy_opponent_source(
    definition: &EffectDefinitionV1,
) -> Option<(CopiedSourceKindV1, CombatStatPredicateV1)> {
    let (_, copied, predicate) = COPY_OPPONENT_SOURCE_GRAMMARS
        .iter()
        .find(|(text, _, _)| *text == definition.description())?;
    let action = match copied {
        CopiedSourceKindV1::Ability => SpecialActionV1::CopyAbility,
        CopiedSourceKindV1::Bonus => SpecialActionV1::CopyBonus,
    };
    copy_opponent_source_shape_matches(definition.structured_input(), action, *predicate)
        .then_some((*copied, *predicate))
}

fn copy_opponent_source_shape_matches(
    input: &StructuredEffectV1,
    action: SpecialActionV1,
    predicate: CombatStatPredicateV1,
) -> bool {
    // Exactly one structured field may carry the condition, and it must be the one the
    // printed prefix names. Everything else stays neutral, so an unfamiliar nested context
    // cannot ride in beside a familiar prefix.
    let (position, previous_round, index) = match predicate {
        CombatStatPredicateV1::Always => (
            PositionRequirementV1::Both,
            PreviousRoundRequirementV1::Any,
            IndexRequirementV1::Any,
        ),
        CombatStatPredicateV1::OwnerMovesSecond => (
            PositionRequirementV1::Defender,
            PreviousRoundRequirementV1::Any,
            IndexRequirementV1::Any,
        ),
        CombatStatPredicateV1::OwnerLostPreviousRound => (
            PositionRequirementV1::Both,
            PreviousRoundRequirementV1::Lose,
            IndexRequirementV1::Any,
        ),
        CombatStatPredicateV1::SelectedHandSlotsDiffer => (
            PositionRequirementV1::Both,
            PreviousRoundRequirementV1::Any,
            IndexRequirementV1::Asymmetry,
        ),
        CombatStatPredicateV1::OwnerMovesFirst
        | CombatStatPredicateV1::OwnerWonPreviousRound
        | CombatStatPredicateV1::SelectedHandSlotsMatch => return false,
    };
    input.value == 0
        && input.value_min == 0
        && input.value_max == 0
        && input.value_condition == 0
        && input.position_requirement == position
        && input.previous_round_requirement == previous_round
        && input.current_round_requirement == CurrentRoundRequirementV1::Any
        && input.index_requirement == index
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
        && input.bet_pillz_link == BetPillzLinkV1::No
        && input.side_affected == AffectedSideV1::Player
        && input.attribute_affected == AttributeAffectedV1::None
        && input.attribute_action == AttributeActionV1::None
        && input.special_action == action
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

/// Recognize Anita's one reviewed Courage conversion only. It must stay outside the
/// generic numeric compiler because its magnitude is final resolved round damage.
pub(crate) fn classify_anita_courage_damage_to_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> bool {
    anita_courage_damage_to_life_identity_matches(source_kind, definition.id())
        && definition.description() == "Courage: +1 Life Per Dmg"
        && anita_courage_damage_to_life_shape_matches(definition.structured_input())
}

/// Shared identity gate for cold compilation and direct compact-plan validation.
pub(crate) fn anita_courage_damage_to_life_identity_matches(
    source_kind: CombatStatEffectSourceV1,
    definition_id: u32,
) -> bool {
    (source_kind, definition_id) == (CombatStatEffectSourceV1::Ability, 274)
}

fn anita_courage_damage_to_life_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value: ShapeFieldV1::Exact(1),
            // Courage lives in the position field, which every other admitted post-round
            // grammar requires neutral.
            position: PositionRequirementV1::Attacker,
            special: SpecialActionV1::ConvertDamageToLife,
            ..POST_ROUND_SHAPE
        },
    )
}

/// Recognize Komboka's exact clan-bonus composite Victory effect.  This remains outside
/// the ordinary numeric compiler because its two checked post-round mutations must stay
/// coupled, ordered, and identity-locked.
pub(crate) fn classify_komboka_victory_pillz_and_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> bool {
    komboka_victory_pillz_and_life_identity_matches(source_kind, definition.id())
        && definition.description() == "+1 Pillz And Life"
        && komboka_victory_pillz_and_life_shape_matches(definition.structured_input())
}

/// Shared identity gate for the cold compiler and direct compact-plan validation.
pub(crate) fn komboka_victory_pillz_and_life_identity_matches(
    source_kind: CombatStatEffectSourceV1,
    definition_id: u32,
) -> bool {
    (source_kind, definition_id) == (CombatStatEffectSourceV1::Bonus, 1714)
}

/// Recognize the two server-observed Reprisal Stop Opp. Ability definitions.  This is
/// deliberately separate from generic Stop Opp. Ability admission: only the exact
/// ability identities below may carry the defender-position predicate.
pub(crate) fn classify_reprisal_stop_opponent_ability(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> bool {
    reprisal_stop_opponent_ability_identity_matches(source_kind, definition.id())
        && definition.description() == "Reprisal: Stop Opp. Ability"
        && reprisal_stop_opponent_ability_shape_matches(definition.structured_input())
}

/// Shared identity gate for the cold compiler and direct compact-plan validation.
pub(crate) fn reprisal_stop_opponent_ability_identity_matches(
    source_kind: CombatStatEffectSourceV1,
    definition_id: u32,
) -> bool {
    source_kind == CombatStatEffectSourceV1::Ability && matches!(definition_id, 1310 | 2073)
}

fn reprisal_stop_opponent_ability_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value: ShapeFieldV1::Exact(0),
            position: PositionRequirementV1::Defender,
            current_round: CurrentRoundRequirementV1::Any,
            attribute: AttributeAffectedV1::None,
            action: AttributeActionV1::None,
            special: SpecialActionV1::StopAbility,
            ..POST_ROUND_SHAPE
        },
    )
}

/// Recognize only the literal, immediate end-of-round Victory Life grammar and the two
/// reviewed forms that put an already-resolved predicate on it.  Unlike the
/// identity-locked Pillz slices below, this is deliberately generic: any registry
/// definition with the complete reviewed structured shape may supply its positive fixed
/// magnitude, whether it came from an Ability or a Bonus.  The prefixed forms are card
/// abilities only - no clan bonus prints them - and each carries its condition in the one
/// field the plain grammar requires neutral, so the two can never be confused.
/// Returns `(life, predicate)`.
pub(crate) fn classify_victory_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, CombatStatPredicateV1)> {
    if !has_victory_life_shape(definition) {
        return None;
    }
    let input = definition.structured_input();
    let (predicate, text) = match (input.previous_round_requirement, input.index_requirement) {
        (PreviousRoundRequirementV1::Any, IndexRequirementV1::Any) => (
            CombatStatPredicateV1::Always,
            format!("+{} Life", input.value),
        ),
        (PreviousRoundRequirementV1::Win, IndexRequirementV1::Any) => (
            CombatStatPredicateV1::OwnerWonPreviousRound,
            format!("Confidence : +{} Life", input.value),
        ),
        (PreviousRoundRequirementV1::Any, IndexRequirementV1::Asymmetry) => (
            CombatStatPredicateV1::SelectedHandSlotsDiffer,
            format!("Asymmetry: +{} Life", input.value),
        ),
        _ => return None,
    };
    if predicate != CombatStatPredicateV1::Always
        && source_kind != CombatStatEffectSourceV1::Ability
    {
        return None;
    }
    (definition.description() == text).then_some((input.value, predicate))
}

/// Structural half of the Victory Life boundary. Replay preparation uses this to reject
/// exact-shape sources whose description is malformed instead of silently disabling them.
pub(crate) fn has_victory_life_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0 && victory_life_shape_matches(input)
}

/// Recognize the plain `+N Pillz` Victory grammar and its `Confidence:` form: the winner's
/// own Pillz rise by the printed amount at the end of the round, unconditionally or only
/// after a round its own side won. Like Victory Life it is admitted by exact text and
/// complete structured shape over every same-text registry record, but card abilities only
/// - no clan bonus prints either, so a Bonus slot carrying the text is a hazard, not a
/// generic source. The prefixed form carries its condition in the one field the plain
/// grammar requires neutral, so the two can never be confused; it prints `Confidence:`
/// tight, unlike Victory Life's spaced `Confidence :`, and the exact-text check is what
/// keeps those two apart. Returns `(pillz, predicate)`.
pub(crate) fn classify_victory_pillz(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, CombatStatPredicateV1)> {
    if source_kind != CombatStatEffectSourceV1::Ability || !has_victory_pillz_shape(definition) {
        return None;
    }
    let input = definition.structured_input();
    let (predicate, text) = match input.previous_round_requirement {
        PreviousRoundRequirementV1::Any => (
            CombatStatPredicateV1::Always,
            format!("+{} Pillz", input.value),
        ),
        PreviousRoundRequirementV1::Win => (
            CombatStatPredicateV1::OwnerWonPreviousRound,
            format!("Confidence: +{} Pillz", input.value),
        ),
        PreviousRoundRequirementV1::Lose => return None,
    };
    (definition.description() == text).then_some((input.value, predicate))
}

/// Structural half of the Victory Pillz boundary, so replay preparation can reject a
/// complete shape under malformed text instead of silently disabling it.
pub(crate) fn has_victory_pillz_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0 && victory_pillz_shape_matches(input)
}

/// Recognize the plain `-N Opp Pillz. Min M` Victory grammar: the winner takes N Pillz
/// from the opposing player, never below M, after that player's bet has been paid. Exact
/// text and complete structured shape over every same-text registry record, card
/// abilities only. Returns `(pillz, minimum)`.
pub(crate) fn classify_victory_opponent_pillz(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16)> {
    let input = definition.structured_input();
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_victory_opponent_pillz_shape(definition)
        && definition.description()
            == format!("-{} Opp Pillz. Min {}", input.value, input.value_min))
    .then_some((input.value, input.value_min))
}

/// Structural half of the opposing Victory Pillz boundary.
pub(crate) fn has_victory_opponent_pillz_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0 && victory_opponent_pillz_shape_matches(input)
}

/// Recognize the losing-side `Defeat: -N Opp. Pillz, Min M` grammar: the owner lost the
/// round, so the opposing player's Pillz fall by N, never below M, read after both bets
/// have been paid. It is the Victory reduction's sibling on the other outcome channel and
/// prints a different text - dotted `Opp.` with a comma before `Min`, under a `Defeat:`
/// prefix - so the two can never be confused. Exact text and complete structured shape,
/// card abilities only. Returns `(pillz, minimum)`.
pub(crate) fn classify_defeat_opponent_pillz(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16)> {
    let input = definition.structured_input();
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_defeat_opponent_pillz_shape(definition)
        && definition.description()
            == format!(
                "Defeat: -{} Opp. Pillz, Min {}",
                input.value, input.value_min
            ))
    .then_some((input.value, input.value_min))
}

/// Structural half of the opposing Defeat Pillz boundary. The clan-gated `4673` carries a
/// clan requirement, which `shape_matches` requires empty, so it is not in this family.
pub(crate) fn has_defeat_opponent_pillz_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0 && defeat_opponent_pillz_shape_matches(input)
}

/// Recognize the `+1 Pillz Per Damage` conversion and its `Symmetry:` form: the winner's
/// own Pillz rise by the final resolved Damage its card dealt. Like Anita's Life conversion
/// the magnitude is bound at resolution, so it stays outside the numeric compiler; unlike
/// Anita's it is admitted by exact text and shape over every same-text record, card
/// abilities only. Returns the one predicate the printed prefix names.
pub(crate) fn classify_victory_pillz_per_damage(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<CombatStatPredicateV1> {
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let (predicate, text) = match definition.structured_input().index_requirement {
        IndexRequirementV1::Any => (CombatStatPredicateV1::Always, "+1 Pillz Per Damage"),
        IndexRequirementV1::Symmetry => (
            CombatStatPredicateV1::SelectedHandSlotsMatch,
            "Symmetry: +1 Pillz Per Damage",
        ),
        IndexRequirementV1::Asymmetry => return None,
    };
    (has_victory_pillz_per_damage_shape(definition) && definition.description() == text)
        .then_some(predicate)
}

/// Structural half of the Pillz-per-Damage boundary, over any hand-slot condition.
pub(crate) fn has_victory_pillz_per_damage_shape(definition: &EffectDefinitionV1) -> bool {
    victory_pillz_per_damage_shape_matches(definition.structured_input())
}

/// Recognize the `+N Life Per Damage` conversion, its capped `Max. M` form and its
/// `Revenge:` and `Confidence:` forms: the winner's own Life rises by N for every point of
/// final resolved Damage its card dealt, and a capped record never carries its owner past
/// M - the same bound `Heal N Max. M` already models on the latch. Anita's Courage form
/// stays identity-locked because it is the one record whose position field carries the
/// condition; these carry theirs, if any, in the previous-round field. The cap and a
/// previous-round prefix have never been observed on one record, so that combination has
/// no reviewed text and rejects. Exact text and shape over every same-text record, card
/// abilities only. Returns `(life_per_damage, maximum, predicate)`, `maximum` 0 meaning
/// uncapped.
pub(crate) fn classify_victory_life_per_damage(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16, CombatStatPredicateV1)> {
    if source_kind != CombatStatEffectSourceV1::Ability
        || !has_victory_life_per_damage_shape(definition)
    {
        return None;
    }
    let input = definition.structured_input();
    let (predicate, text) = match (input.previous_round_requirement, input.value_max) {
        (PreviousRoundRequirementV1::Any, 0) => (
            CombatStatPredicateV1::Always,
            format!("+{} Life Per Damage", input.value),
        ),
        (PreviousRoundRequirementV1::Any, maximum) => (
            CombatStatPredicateV1::Always,
            format!("+{} Life Per Damage Max. {}", input.value, maximum),
        ),
        (PreviousRoundRequirementV1::Lose, 0) => (
            CombatStatPredicateV1::OwnerLostPreviousRound,
            format!("Revenge: +{} Life Per Damage", input.value),
        ),
        (PreviousRoundRequirementV1::Win, 0) => (
            CombatStatPredicateV1::OwnerWonPreviousRound,
            format!("Confidence: +{} Life Per Dmg.", input.value),
        ),
        (PreviousRoundRequirementV1::Lose | PreviousRoundRequirementV1::Win, _) => return None,
    };
    (definition.description() == text).then_some((input.value, input.value_max, predicate))
}

/// Structural half of the Life-per-Damage boundary, over any previous-round condition and
/// either the capped or the uncapped magnitude.
pub(crate) fn has_victory_life_per_damage_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0 && victory_life_per_damage_shape_matches(input)
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

/// The deliberately small Victory Or Defeat Life slice which has been audited against
/// canonical card provenance.  Its effects happen after round damage, so they must not
/// be admitted through the ordinary numeric compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VictoryOrDefeatLifeEffectV1 {
    GainLife { life: u16 },
    ReduceOpponentLife { life: u16, minimum: u16 },
}

/// Recognize the Victory Or Defeat Life sources. The own-Life gains stay a closed set of
/// reviewed identities: printed descriptions recur in unrelated catalog rows, so source
/// kind and capture identity are both part of that grammar.
///
/// The opposing reduction is the same effect the Victory and `Defeat:` channels already
/// execute, so it is admitted the same way they are - exact printed text, complete neutral
/// shape and card abilities only - over every `Victory Or Defeat: - N Opp. Life Min M`
/// record. Uuber's `1628` stays identity-locked because it is the one such record a clan
/// Bonus also prints, and no other magnitude may ride that id.
pub(crate) fn classify_victory_or_defeat_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<VictoryOrDefeatLifeEffectV1> {
    let reviewed = match definition.id() {
        1396 | 2992 | 5835 | 5799 => Some(VictoryOrDefeatLifeEffectV1::GainLife { life: 1 }),
        5802 | 2944 => Some(VictoryOrDefeatLifeEffectV1::GainLife { life: 2 }),
        1628 => Some(VictoryOrDefeatLifeEffectV1::ReduceOpponentLife {
            life: 1,
            minimum: 1,
        }),
        _ => None,
    };
    if let Some(effect) = reviewed {
        if !victory_or_defeat_life_identity_matches(source_kind, definition.id()) {
            return None;
        }
        let input = definition.structured_input();
        return match effect {
            VictoryOrDefeatLifeEffectV1::GainLife { life } => (definition.description()
                == format!("Victory Or Defeat : +{life} Life")
                && victory_or_defeat_life_shape_matches(
                    input,
                    life,
                    1,
                    AffectedSideV1::Player,
                    AttributeActionV1::Increase,
                ))
            .then_some(effect),
            VictoryOrDefeatLifeEffectV1::ReduceOpponentLife { life, minimum } => (definition
                .description()
                == format!("Victory Or Defeat: - {life} Opp. Life Min {minimum}")
                && victory_or_defeat_life_shape_matches(
                    input,
                    life,
                    minimum,
                    AffectedSideV1::Opponent,
                    AttributeActionV1::Decrease,
                ))
            .then_some(effect),
        };
    }
    let input = definition.structured_input();
    let (life, minimum) = (input.value, input.value_min);
    (source_kind == CombatStatEffectSourceV1::Ability
        && life > 0
        && definition.description()
            == format!("Victory Or Defeat: - {life} Opp. Life Min {minimum}")
        && victory_or_defeat_life_shape_matches(
            input,
            life,
            minimum,
            AffectedSideV1::Opponent,
            AttributeActionV1::Decrease,
        ))
    .then_some(VictoryOrDefeatLifeEffectV1::ReduceOpponentLife { life, minimum })
}

/// Structural half of the Victory Or Defeat opponent-Life boundary, so replay preparation
/// can reject the complete reviewed shape under malformed text rather than disabling it.
pub(crate) fn has_victory_or_defeat_opponent_life_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && victory_or_defeat_life_shape_matches(
            input,
            input.value,
            input.value_min,
            AffectedSideV1::Opponent,
            AttributeActionV1::Decrease,
        )
}

/// Shared identity gate for the cold compiler and direct compact-plan validation.
pub(crate) fn victory_or_defeat_life_identity_matches(
    source_kind: CombatStatEffectSourceV1,
    definition_id: u32,
) -> bool {
    matches!(
        source_kind,
        CombatStatEffectSourceV1::Ability | CombatStatEffectSourceV1::Bonus
    ) && matches!(
        definition_id,
        1396 | 2992 | 5835 | 5799 | 5802 | 2944 | 1628
    )
}

/// The Victory opponent-Life identities that are *not* covered by the plain grammar below:
/// the one clan Bonus that prints the reduction, and the conditional forms whose printed
/// text differs record by record. The source kind is authority in both directions, so `680`
/// is only ever the clan Bonus and each conditional only ever a printed Ability.
///
/// The conditional members reuse predicates the projection already resolves before a round
/// is prepared. Courage `4533` (Ligea level 3) and Growth `1730` deliberately stay out:
/// `4533` has no selected observation anywhere in the corpus, and `1730` is a round-scaled
/// magnitude rather than a predicate, which a post-round plan cannot carry today.
const VICTORY_OPPONENT_LIFE_IDENTITIES: [(
    u32,
    &str,
    CombatStatEffectSourceV1,
    u16,
    u16,
    CombatStatPredicateV1,
); 4] = [
    (
        680,
        "-2 Opp. Life Min 2",
        CombatStatEffectSourceV1::Bonus,
        2,
        2,
        CombatStatPredicateV1::Always,
    ),
    (
        4708,
        "Symmetry: - 4 Opp. Life Min 0",
        CombatStatEffectSourceV1::Ability,
        4,
        0,
        CombatStatPredicateV1::SelectedHandSlotsMatch,
    ),
    // Diabolus prints the same effect at both of her levels under two registry ids whose
    // structured records and descriptions are byte-identical, so each is admitted on its
    // own evidence rather than one being treated as an alias of the other.
    (
        3016,
        "Confidence: -3 Opp. Life, Min 0",
        CombatStatEffectSourceV1::Ability,
        3,
        0,
        CombatStatPredicateV1::OwnerWonPreviousRound,
    ),
    (
        4301,
        "Confidence: -3 Opp. Life, Min 0",
        CombatStatEffectSourceV1::Ability,
        3,
        0,
        CombatStatPredicateV1::OwnerWonPreviousRound,
    ),
];

/// Recognize the Victory opponent-Life reductions. They are post-round resource work rather
/// than combat-stat modifiers, so they stay out of the generic numeric compiler.
///
/// The unconditional form is a grammar like its `Defeat:` sibling: exact printed text,
/// complete neutral structured shape, card abilities only. The registry carries fourteen
/// structurally identical `-N Opp. Life Min M` ability records and the printed text has to
/// agree with both numbers before either is used, so no magnitude can be smuggled in under
/// a text that does not name it. The one clan Bonus that prints the reduction and the
/// conditional forms, whose printed text differs record by record, stay identity-locked.
///
/// Everything else remains fail-closed: the capped, compound and clan-gated neighbours, the
/// complete shape under prefixed text such as `Night: -2 Opp. Life Min 0`, and the same-text
/// catalog ids that have no registry definition at all (Rakhan `978`, Milovan `498`,
/// Fraser `1289`).
pub(crate) fn classify_victory_opponent_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16, CombatStatPredicateV1)> {
    if let Some((_, description, _, life, minimum, predicate)) = VICTORY_OPPONENT_LIFE_IDENTITIES
        .iter()
        .find(|(id, _, kind, _, _, _)| *id == definition.id() && *kind == source_kind)
    {
        return (definition.description() == *description
            && victory_opponent_life_shape_matches(
                definition.structured_input(),
                *life,
                *minimum,
                *predicate,
            ))
        .then_some((*life, *minimum, *predicate));
    }
    // A reserved id never reaches the grammar under the wrong source kind.
    if victory_opponent_life_id_is_identity_locked(definition.id()) {
        return None;
    }
    let input = definition.structured_input();
    let (life, minimum) = (input.value, input.value_min);
    (source_kind == CombatStatEffectSourceV1::Ability
        && life > 0
        && definition.description() == format!("-{life} Opp. Life Min {minimum}")
        && victory_opponent_life_shape_matches(input, life, minimum, CombatStatPredicateV1::Always))
    .then_some((life, minimum, CombatStatPredicateV1::Always))
}

/// Structural half of the Victory opponent-Life boundary, so replay preparation can reject
/// the complete reviewed shape under malformed or prefixed text instead of silently
/// disabling it.
pub(crate) fn has_victory_opponent_life_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && victory_opponent_life_shape_matches(
            input,
            input.value,
            input.value_min,
            CombatStatPredicateV1::Always,
        )
}

/// The ids the grammar must never admit on its own: each is pinned to one source kind and
/// one magnitude by `VICTORY_OPPONENT_LIFE_IDENTITIES`.
pub(crate) fn victory_opponent_life_id_is_identity_locked(definition_id: u32) -> bool {
    VICTORY_OPPONENT_LIFE_IDENTITIES
        .iter()
        .any(|(id, _, _, _, _, _)| *id == definition_id)
}

/// Shared identity gate for cold compilation and direct compact-plan validation.
pub(crate) fn victory_opponent_life_identity_matches(
    source_kind: CombatStatEffectSourceV1,
    definition_id: u32,
) -> bool {
    VICTORY_OPPONENT_LIFE_IDENTITIES
        .iter()
        .any(|(id, _, kind, _, _, _)| *id == definition_id && *kind == source_kind)
}

/// The one predicate a reviewed Victory opponent-Life identity may carry, so a compact plan
/// handed straight to the engine cannot swap in a different condition.
pub(crate) fn victory_opponent_life_predicate(definition_id: u32) -> Option<CombatStatPredicateV1> {
    VICTORY_OPPONENT_LIFE_IDENTITIES
        .iter()
        .find(|(id, _, _, _, _, _)| *id == definition_id)
        .map(|(_, _, _, _, _, predicate)| *predicate)
}

fn victory_opponent_life_shape_matches(
    input: &StructuredEffectV1,
    life: u16,
    minimum: u16,
    predicate: CombatStatPredicateV1,
) -> bool {
    // Exactly one structured field carries the condition, and it must be the one the printed
    // text names. Every other context field stays neutral.
    let (position, previous_round, index) = match predicate {
        CombatStatPredicateV1::Always => (
            PositionRequirementV1::Both,
            PreviousRoundRequirementV1::Any,
            IndexRequirementV1::Any,
        ),
        CombatStatPredicateV1::SelectedHandSlotsMatch => (
            PositionRequirementV1::Both,
            PreviousRoundRequirementV1::Any,
            IndexRequirementV1::Symmetry,
        ),
        CombatStatPredicateV1::OwnerWonPreviousRound => (
            PositionRequirementV1::Both,
            PreviousRoundRequirementV1::Win,
            IndexRequirementV1::Any,
        ),
        CombatStatPredicateV1::OwnerMovesFirst
        | CombatStatPredicateV1::OwnerMovesSecond
        | CombatStatPredicateV1::OwnerLostPreviousRound
        | CombatStatPredicateV1::SelectedHandSlotsDiffer => return false,
    };
    input.value == life
        && input.value_min == minimum
        && input.value_max == 0
        && input.value_condition == 0
        && input.position_requirement == position
        && input.previous_round_requirement == previous_round
        && input.current_round_requirement == CurrentRoundRequirementV1::Win
        && input.index_requirement == index
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
        && input.bet_pillz_link == BetPillzLinkV1::No
        && input.side_affected == AffectedSideV1::Opponent
        && input.attribute_affected == AttributeAffectedV1::Life
        && input.attribute_action == AttributeActionV1::Decrease
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

/// Recognize the two reviewed Equalizer opponent-Life effects.  This is a Victory-only
/// post-round reduction whose magnitude is bound from the revealed opposing card's stars
/// after source liveness is known, rather than a normal combat-stat modifier.
/// `Defeat: -N Opp. Life, Min M` is the losing-side sibling of the Victory reduction and
/// shares its execution channel. Like generic Victory Life it is admitted by exact printed
/// text and a neutral structured shape rather than by an id list: the registry carries six
/// structurally identical records across four Min values, and the printed text has to agree
/// with both numbers before either is used.
pub(crate) fn classify_defeat_opponent_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16)> {
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let input = definition.structured_input();
    let (life, minimum) = (input.value, input.value_min);
    (life > 0
        && definition.description() == format!("Defeat: -{life} Opp. Life, Min {minimum}")
        && defeat_opponent_life_shape_matches(input))
    .then_some((life, minimum))
}

fn defeat_opponent_life_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value_min: ShapeFieldV1::Read,
            current_round: CurrentRoundRequirementV1::Lose,
            side: AffectedSideV1::Opponent,
            action: AttributeActionV1::Decrease,
            ..POST_ROUND_SHAPE
        },
    )
}

/// `Killshot: -N Opp. Life Min M` is the same opponent-Life reduction on the `sureshot`
/// current-round channel: it pays when the owner's final attack is at least double the
/// opposing one, which is a different question from winning the round. The registry has
/// always parsed `sureshot`; nothing had ever asked it, so this is the first grammar whose
/// trigger reads the resolved attacks rather than the round's winner.
///
/// The printed text is the plain Victory spelling under a prefix - unspaced `Min`, no comma
/// before it - and not the `Defeat:` comma form, so the two sibling grammars cannot be
/// admitted by each other's text.
pub(crate) fn classify_killshot_opponent_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16)> {
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let input = definition.structured_input();
    let (life, minimum) = (input.value, input.value_min);
    (life > 0
        && definition.description() == format!("Killshot: -{life} Opp. Life Min {minimum}")
        && killshot_opponent_life_shape_matches(input))
    .then_some((life, minimum))
}

fn killshot_opponent_life_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value_min: ShapeFieldV1::Read,
            current_round: CurrentRoundRequirementV1::Sureshot,
            side: AffectedSideV1::Opponent,
            action: AttributeActionV1::Decrease,
            ..POST_ROUND_SHAPE
        },
    )
}

/// Structural half of the Killshot opponent-Life boundary, so replay preparation can call
/// the complete reviewed shape under malformed or differently prefixed text a hazard rather
/// than silently disabling it.
pub(crate) fn has_killshot_opponent_life_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0 && killshot_opponent_life_shape_matches(input)
}

/// Recognize `Xantiax: -N Life, Min. M`: the only admitted post-round grammar that names
/// no outcome and no beneficiary. Both players lose N, neither below M, whatever the round
/// did. `Xantiax` is flavour on the printed text, not a condition - the structured record
/// asks for no outcome, no previous round, no position and no hand slot, and reaches both
/// sides at once, which is the shape no other admitted grammar has. Exact text and complete
/// shape over every same-text registry record, card abilities only: no clan bonus prints it,
/// so a Bonus slot carrying the text is a hazard rather than a generic source.
/// Returns `(life, minimum)`.
pub(crate) fn classify_both_players_life_reduction(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16)> {
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let input = definition.structured_input();
    let (life, minimum) = (input.value, input.value_min);
    (life > 0
        && definition.description() == format!("Xantiax: -{life} Life, Min. {minimum}")
        && both_players_life_reduction_shape_matches(input))
    .then_some((life, minimum))
}

/// Structural half of the boundary, so replay preparation can reject a complete shape under
/// malformed text instead of silently disabling it.
pub(crate) fn has_both_players_life_reduction_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0 && both_players_life_reduction_shape_matches(input)
}

fn both_players_life_reduction_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value_min: ShapeFieldV1::Read,
            // No outcome channel at all, which is what makes this grammar unmistakable:
            // every neighbouring Life reduction names `win` or `lose` here.
            current_round: CurrentRoundRequirementV1::Any,
            side: AffectedSideV1::Both,
            action: AttributeActionV1::Decrease,
            ..POST_ROUND_SHAPE
        },
    )
}

pub(crate) fn classify_equalizer_opponent_life_on_victory(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16)> {
    let input = definition.structured_input();
    (equalizer_opponent_life_on_victory_identity_matches(source_kind, definition.id())
        && definition.description() == "Equalizer: - 1 Opp. Life Min 2"
        && equalizer_opponent_life_on_victory_shape_matches(input))
    .then_some((input.value, input.value_min))
}

/// Shared identity gate for compiler output and direct compact-plan validation. Captured
/// Copy can materialise either source kind, so neither source slot is privileged here.
pub(crate) fn equalizer_opponent_life_on_victory_identity_matches(
    source_kind: CombatStatEffectSourceV1,
    definition_id: u32,
) -> bool {
    matches!(
        source_kind,
        CombatStatEffectSourceV1::Ability | CombatStatEffectSourceV1::Bonus
    ) && matches!(definition_id, 1415 | 4458)
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

/// Recognize the plain `Heal N Max. M` grammar, the projection's repeating own-Life effect:
/// the round its card wins latches it and pays nothing, and every later round then pays
/// `life` while the owner is below `maximum`. Like generic Victory Life it is admitted by
/// exact printed text and complete structured shape rather than an id list, because the
/// registry carries eight structurally identical records of it; the printed numbers are
/// authority and a record whose text disagrees with its own magnitude or cap is refused.
/// Card abilities only: no clan bonus prints a Heal. `Defeat : Heal`, `Asymmetry: Heal` and
/// the clan-gated form are different texts with different latch conditions and stay closed.
/// Returns `(life, maximum)`.
pub(crate) fn classify_heal_life_on_victory(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    let (predicate, prefix) = permanent_condition(input)?;
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_heal_life_on_victory_shape(definition)
        && definition.description()
            == format!("{prefix}Heal {} Max. {}", input.value, input.value_max))
    .then_some((input.value, input.value_max, predicate))
}

/// The one condition a plain permanent may carry, and the prefix its text then prints. The
/// plain record carries none; `Symmetry:`/`Asymmetry:` sit in the hand-slot field and
/// `Revenge:`/`Confidence:` in the previous-round field, exactly as they do on fixed numeric
/// abilities. The latch is judged once, in the latching round, so the predicate is
/// evaluated where every plan predicate is and the latched effect itself carries none.
fn permanent_condition(
    input: &StructuredEffectV1,
) -> Option<(CombatStatPredicateV1, &'static str)> {
    match (input.previous_round_requirement, input.index_requirement) {
        (PreviousRoundRequirementV1::Any, IndexRequirementV1::Any) => {
            Some((CombatStatPredicateV1::Always, ""))
        }
        (PreviousRoundRequirementV1::Any, IndexRequirementV1::Symmetry) => {
            Some((CombatStatPredicateV1::SelectedHandSlotsMatch, "Symmetry: "))
        }
        (PreviousRoundRequirementV1::Any, IndexRequirementV1::Asymmetry) => Some((
            CombatStatPredicateV1::SelectedHandSlotsDiffer,
            "Asymmetry: ",
        )),
        (PreviousRoundRequirementV1::Lose, IndexRequirementV1::Any) => {
            Some((CombatStatPredicateV1::OwnerLostPreviousRound, "Revenge: "))
        }
        (PreviousRoundRequirementV1::Win, IndexRequirementV1::Any) => {
            Some((CombatStatPredicateV1::OwnerWonPreviousRound, "Confidence: "))
        }
        _ => None,
    }
}

/// True for every predicate a permanent's plan may carry.
pub(crate) fn permanent_predicate_admitted(predicate: CombatStatPredicateV1) -> bool {
    matches!(
        predicate,
        CombatStatPredicateV1::Always
            | CombatStatPredicateV1::SelectedHandSlotsMatch
            | CombatStatPredicateV1::SelectedHandSlotsDiffer
            | CombatStatPredicateV1::OwnerLostPreviousRound
            | CombatStatPredicateV1::OwnerWonPreviousRound
    )
}

/// Structural half of the Heal boundary. Replay preparation uses this to reject a source
/// carrying the exact permanent shape under malformed text instead of disabling it.
pub(crate) fn has_heal_life_on_victory_shape(definition: &EffectDefinitionV1) -> bool {
    heal_life_on_victory_shape_matches(definition.structured_input())
}

/// `Regen N, Max. M`: Heal's immediate sibling, admitted the same way. Note the site's
/// comma, which Heal's text does not carry. Returns `(life, maximum)`.
pub(crate) fn classify_regen_life_on_victory(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    let (predicate, prefix) = permanent_condition(input)?;
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_regen_life_on_victory_shape(definition)
        && definition.description()
            == format!("{prefix}Regen {}, Max. {}", input.value, input.value_max))
    .then_some((input.value, input.value_max, predicate))
}

pub(crate) fn has_regen_life_on_victory_shape(definition: &EffectDefinitionV1) -> bool {
    permanent_own_life_shape_matches(definition.structured_input(), true)
}

/// `Poison N, Min M`: the opposing player loses N Life at the end of every round after the
/// latch, never below M. Freaks print it as their clan bonus, so both slots are admitted.
/// Returns `(life, minimum)`.
pub(crate) fn classify_poison_opponent_life_on_victory(
    definition: &EffectDefinitionV1,
    _source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    let (predicate, prefix) = permanent_condition(input)?;
    (has_poison_opponent_life_on_victory_shape(definition)
        && definition.description()
            == format!("{prefix}Poison {}, Min {}", input.value, input.value_min))
    .then_some((input.value, input.value_min, predicate))
}

pub(crate) fn has_poison_opponent_life_on_victory_shape(definition: &EffectDefinitionV1) -> bool {
    permanent_opponent_life_shape_matches(definition.structured_input(), false)
}

/// `Toxin N, Min M`: Poison that also pays in its latching round. Card abilities only.
/// Returns `(life, minimum)`.
pub(crate) fn classify_toxin_opponent_life_on_victory(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    let (predicate, prefix) = permanent_condition(input)?;
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_toxin_opponent_life_on_victory_shape(definition)
        && definition.description()
            == format!("{prefix}Toxin {}, Min {}", input.value, input.value_min))
    .then_some((input.value, input.value_min, predicate))
}

pub(crate) fn has_toxin_opponent_life_on_victory_shape(definition: &EffectDefinitionV1) -> bool {
    permanent_opponent_life_shape_matches(definition.structured_input(), true)
}

pub(crate) fn classify_combat_stat_effect(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    if classify_anita_courage_damage_to_life(definition, source_kind) {
        return None;
    }
    // A permanent is post-round work with its own latch channel. The registry already
    // refuses it as `Permanent`, but keep the guard so a later widening cannot turn it
    // into an ordinary one-round effect.
    if classify_heal_life_on_victory(definition, source_kind).is_some()
        || classify_regen_life_on_victory(definition, source_kind).is_some()
        || classify_poison_opponent_life_on_victory(definition, source_kind).is_some()
        || classify_toxin_opponent_life_on_victory(definition, source_kind).is_some()
    {
        return None;
    }
    if classify_copy_opponent_source(definition).is_some() {
        return None;
    }
    // Unconditional Victory opponent-Life is post-round resource work with its own
    // execution channel; generic numeric admission must never reinterpret it.
    if classify_victory_opponent_life(definition, source_kind).is_some() {
        return None;
    }
    // The losing-side opponent-Life reduction has the same post-round execution channel.
    if classify_defeat_opponent_life(definition, source_kind).is_some() {
        return None;
    }
    // So does the Killshot reduction, on the `sureshot` channel.
    if classify_killshot_opponent_life(definition, source_kind).is_some() {
        return None;
    }
    // Recovery has its own post-round execution channel. Keep it out of this combat-stat
    // return type so neither generic numeric admission nor cancellation can reinterpret it.
    if classify_defeat_recover_pillz(definition, source_kind) {
        return None;
    }
    if classify_argos_defeat_capped_pillz(definition, source_kind) {
        return None;
    }
    if classify_komboka_victory_pillz_and_life(definition, source_kind) {
        return None;
    }
    if classify_victory_life(definition, source_kind).is_some()
        || classify_victory_pillz(definition, source_kind).is_some()
        || classify_victory_opponent_pillz(definition, source_kind).is_some()
        || classify_victory_pillz_per_damage(definition, source_kind).is_some()
        || classify_victory_life_per_damage(definition, source_kind).is_some()
    {
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
    if classify_victory_or_defeat_life(definition, source_kind).is_some() {
        return None;
    }
    if classify_equalizer_opponent_life_on_victory(definition, source_kind).is_some() {
        return None;
    }
    if classify_reprisal_stop_opponent_ability(definition, source_kind) {
        return Some((
            SupportedEffectV1::StopOpponentAbility,
            CombatStatPredicateV1::OwnerMovesSecond,
        ));
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
        // Protection is admitted from either slot: `Protection: Power And Damage` and
        // `Protection: Bonus` are printed abilities, `Protection: Ability` is the Skeelz
        // bonus. The registry's exact-description gate is what keeps the family narrow.
        SupportedEffectV1::ProtectOwnCombatStat { .. }
        | SupportedEffectV1::ProtectOwnAbility
        | SupportedEffectV1::ProtectOwnBonus
        | SupportedEffectV1::CopyOpponentPrintedCombatStat { .. } => true,
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
    shape_matches(
        input,
        PostRoundShapeV1 {
            value: ShapeFieldV1::Exact(2),
            value_min: ShapeFieldV1::Exact(3),
            current_round: CurrentRoundRequirementV1::Lose,
            attribute: AttributeAffectedV1::Pillz,
            special: SpecialActionV1::RecoverPillz,
            ..POST_ROUND_SHAPE
        },
    )
}

/// How a grammar constrains one numeric field of a structured record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShapeFieldV1 {
    /// The field must hold exactly this value.
    Exact(u16),
    /// The grammar reads the field and renders it into the text it expects, so any value is
    /// structurally acceptable and the exact-text check is what pins it.
    Read,
}

impl ShapeFieldV1 {
    const fn accepts(self, actual: u16) -> bool {
        match self {
            Self::Exact(expected) => actual == expected,
            Self::Read => true,
        }
    }
}

/// The structured fields one post-round grammar constrains.
///
/// Every admitted grammar asks for the same neutral record in all but a handful of fields,
/// so a grammar is written as `POST_ROUND_SHAPE` with those few overridden rather than as
/// its own thirty-line conjunction. The fields absent from this struct are the ones no
/// admitted grammar has ever varied - the clan gates, the bet link, `valueCondition` and
/// every magnitude flag - and `shape_matches` requires all of them neutral, which is what
/// keeps the projection fail-closed as the table grows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PostRoundShapeV1 {
    pub(crate) value: ShapeFieldV1,
    pub(crate) value_min: ShapeFieldV1,
    pub(crate) value_max: ShapeFieldV1,
    pub(crate) position: PositionRequirementV1,
    /// The `(previous round, hand slot)` pairs the grammar admits, as pairs rather than two
    /// independent lists: fixed Victory Life takes a won previous round or a differing hand
    /// slot, never both at once, and a cross product would silently admit the combination
    /// no card prints.
    pub(crate) conditions: &'static [(PreviousRoundRequirementV1, IndexRequirementV1)],
    pub(crate) current_round: CurrentRoundRequirementV1,
    pub(crate) side: AffectedSideV1,
    pub(crate) attribute: AttributeAffectedV1,
    pub(crate) action: AttributeActionV1,
    pub(crate) special: SpecialActionV1,
    pub(crate) opponent_stars_linked: bool,
}

/// The one unconditional slot: no previous-round requirement and no hand-slot requirement.
const UNCONDITIONAL: &[(PreviousRoundRequirementV1, IndexRequirementV1)] =
    &[(PreviousRoundRequirementV1::Any, IndexRequirementV1::Any)];

/// The neutral post-round record: an unconditional fixed gain of the owner's own Life on a
/// won round, with no bounds and no special action. Every grammar below is this with the
/// fields it actually differs in overridden.
const POST_ROUND_SHAPE: PostRoundShapeV1 = PostRoundShapeV1 {
    value: ShapeFieldV1::Read,
    value_min: ShapeFieldV1::Exact(0),
    value_max: ShapeFieldV1::Exact(0),
    position: PositionRequirementV1::Both,
    conditions: UNCONDITIONAL,
    current_round: CurrentRoundRequirementV1::Win,
    side: AffectedSideV1::Player,
    attribute: AttributeAffectedV1::Life,
    action: AttributeActionV1::Increase,
    special: SpecialActionV1::None,
    opponent_stars_linked: false,
};

/// True when `input` is exactly the record `shape` describes. The fields the shape does not
/// name must all be neutral: a clan gate, a bet link, a `valueCondition`, a Support or
/// per-X magnitude or a permanence flag takes a record out of every admitted post-round
/// grammar, whatever its text says.
fn shape_matches(input: &StructuredEffectV1, shape: PostRoundShapeV1) -> bool {
    shape.value.accepts(input.value)
        && shape.value_min.accepts(input.value_min)
        && shape.value_max.accepts(input.value_max)
        && input.position_requirement == shape.position
        && shape
            .conditions
            .contains(&(input.previous_round_requirement, input.index_requirement))
        && input.current_round_requirement == shape.current_round
        && input.side_affected == shape.side
        && input.attribute_affected == shape.attribute
        && input.attribute_action == shape.action
        && input.special_action == shape.special
        && input.is_opponent_stars_linked == shape.opponent_stars_linked
        && input.value_condition == 0
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
        && input.bet_pillz_link == BetPillzLinkV1::No
        && !input.is_inverted
        && !input.is_support
        && !input.is_anti_support
        && !input.is_overdrive
        && !input.is_divide
        && !input.is_life_linked
        && !input.is_pillz_linked
        && !input.is_lost_life_linked
        && !input.is_lost_pillz_linked
        && !input.is_clanmates_count_linked
        && !input.is_anti_clanmates_count_linked
        && !input.is_permanent
        && !input.is_immediate_permanent
}

fn victory_life_shape_matches(input: &StructuredEffectV1) -> bool {
    // The three condition slots fixed Victory Life prints: none, `Confidence :`'s won
    // previous round, `Asymmetry:`'s differing hand slots. A `Revenge:` Life, a Courage
    // position or a clan gate keeps its visible-but-disabled record instead.
    const CONDITIONS: &[(PreviousRoundRequirementV1, IndexRequirementV1)] = &[
        (PreviousRoundRequirementV1::Any, IndexRequirementV1::Any),
        (PreviousRoundRequirementV1::Win, IndexRequirementV1::Any),
        (
            PreviousRoundRequirementV1::Any,
            IndexRequirementV1::Asymmetry,
        ),
    ];
    shape_matches(
        input,
        PostRoundShapeV1 {
            conditions: CONDITIONS,
            ..POST_ROUND_SHAPE
        },
    )
}

fn victory_pillz_shape_matches(input: &StructuredEffectV1) -> bool {
    // No condition at all, or the won previous round `Confidence:` names. A `Revenge:`
    // Pillz keeps its visible-but-disabled record rather than becoming a near-miss hazard.
    const CONDITIONS: &[(PreviousRoundRequirementV1, IndexRequirementV1)] = &[
        (PreviousRoundRequirementV1::Any, IndexRequirementV1::Any),
        (PreviousRoundRequirementV1::Win, IndexRequirementV1::Any),
    ];
    shape_matches(
        input,
        PostRoundShapeV1 {
            conditions: CONDITIONS,
            attribute: AttributeAffectedV1::Pillz,
            ..POST_ROUND_SHAPE
        },
    )
}

fn victory_opponent_pillz_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value_min: ShapeFieldV1::Read,
            side: AffectedSideV1::Opponent,
            attribute: AttributeAffectedV1::Pillz,
            action: AttributeActionV1::Decrease,
            ..POST_ROUND_SHAPE
        },
    )
}

fn defeat_opponent_pillz_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value_min: ShapeFieldV1::Read,
            current_round: CurrentRoundRequirementV1::Lose,
            side: AffectedSideV1::Opponent,
            attribute: AttributeAffectedV1::Pillz,
            action: AttributeActionV1::Decrease,
            ..POST_ROUND_SHAPE
        },
    )
}

fn victory_pillz_per_damage_shape_matches(input: &StructuredEffectV1) -> bool {
    // The conversion leaves the hand-slot field free - `Symmetry:` is a printed form of it -
    // and the classifier narrows that to the predicates it has evidence for. The structural
    // half stays as wide as the grammar so a hand-slot near-miss is a hazard, not a no-op.
    const CONDITIONS: &[(PreviousRoundRequirementV1, IndexRequirementV1)] = &[
        (PreviousRoundRequirementV1::Any, IndexRequirementV1::Any),
        (
            PreviousRoundRequirementV1::Any,
            IndexRequirementV1::Symmetry,
        ),
        (
            PreviousRoundRequirementV1::Any,
            IndexRequirementV1::Asymmetry,
        ),
    ];
    shape_matches(
        input,
        PostRoundShapeV1 {
            value: ShapeFieldV1::Exact(1),
            conditions: CONDITIONS,
            attribute: AttributeAffectedV1::Pillz,
            special: SpecialActionV1::ConvertDamageToPillz,
            ..POST_ROUND_SHAPE
        },
    )
}

fn victory_life_per_damage_shape_matches(input: &StructuredEffectV1) -> bool {
    // The Life conversion leaves the previous-round field free instead - `Revenge:` and
    // `Confidence:` are printed forms - and reads a `Max. M` cap. The classifier narrows
    // the predicate and refuses a cap under a prefix, which no card prints together.
    const CONDITIONS: &[(PreviousRoundRequirementV1, IndexRequirementV1)] = &[
        (PreviousRoundRequirementV1::Any, IndexRequirementV1::Any),
        (PreviousRoundRequirementV1::Win, IndexRequirementV1::Any),
        (PreviousRoundRequirementV1::Lose, IndexRequirementV1::Any),
    ];
    shape_matches(
        input,
        PostRoundShapeV1 {
            value_max: ShapeFieldV1::Read,
            conditions: CONDITIONS,
            special: SpecialActionV1::ConvertDamageToLife,
            ..POST_ROUND_SHAPE
        },
    )
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
    shape_matches(
        input,
        PostRoundShapeV1 {
            value: ShapeFieldV1::Exact(1),
            current_round: CurrentRoundRequirementV1::Any,
            attribute: AttributeAffectedV1::Pillz,
            ..POST_ROUND_SHAPE
        },
    )
}

fn victory_or_defeat_life_shape_matches(
    input: &StructuredEffectV1,
    life: u16,
    minimum: u16,
    side_affected: AffectedSideV1,
    attribute_action: AttributeActionV1,
) -> bool {
    input.value == life
        && input.value_min == minimum
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
        && input.side_affected == side_affected
        && input.attribute_affected == AttributeAffectedV1::Life
        && input.attribute_action == attribute_action
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

fn equalizer_opponent_life_on_victory_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value: ShapeFieldV1::Exact(1),
            value_min: ShapeFieldV1::Exact(2),
            side: AffectedSideV1::Opponent,
            action: AttributeActionV1::Decrease,
            // The magnitude is the opposing card's stars, which is the one place an
            // admitted post-round grammar reads a per-X flag.
            opponent_stars_linked: true,
            ..POST_ROUND_SHAPE
        },
    )
}

fn argos_defeat_capped_pillz_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value: ShapeFieldV1::Exact(2),
            value_max: ShapeFieldV1::Exact(11),
            current_round: CurrentRoundRequirementV1::Lose,
            attribute: AttributeAffectedV1::Pillz,
            ..POST_ROUND_SHAPE
        },
    )
}

/// The complete structured shape every plain `Heal N Max. M` record carries: an own-Life
/// increase of `value` on a won round, capped at `value_max`, permanent but not immediate.
/// `valueMin` is 1 on every such record and is not a floor the effect ever reads.
fn heal_life_on_victory_shape_matches(input: &StructuredEffectV1) -> bool {
    permanent_own_life_shape_matches(input, false)
}

/// Heal and Regen share one shape apart from `isImmediatePermanent`, which is the only
/// structured trace of whether the latching round pays.
fn permanent_own_life_shape_matches(input: &StructuredEffectV1, immediate: bool) -> bool {
    input.value > 0
        && input.value_min == 1
        && input.value_max > input.value
        && permanent_life_neutral_shape_matches(
            input,
            AffectedSideV1::Player,
            AttributeActionV1::Increase,
            immediate,
        )
}

/// Poison and Toxin likewise: an opposing-Life decrease of `value` bounded below by
/// `value_min`, with no cap, distinguished only by `isImmediatePermanent`.
fn permanent_opponent_life_shape_matches(input: &StructuredEffectV1, immediate: bool) -> bool {
    input.value > 0
        && input.value_max == 0
        && permanent_life_neutral_shape_matches(
            input,
            AffectedSideV1::Opponent,
            AttributeActionV1::Decrease,
            immediate,
        )
}

fn permanent_life_neutral_shape_matches(
    input: &StructuredEffectV1,
    side: AffectedSideV1,
    action: AttributeActionV1,
    immediate: bool,
) -> bool {
    input.value_condition == 0
        && input.position_requirement == PositionRequirementV1::Both
        && permanent_condition(input).is_some()
        && input.current_round_requirement == CurrentRoundRequirementV1::Win
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
        && input.bet_pillz_link == BetPillzLinkV1::No
        && input.side_affected == side
        && input.attribute_affected == AttributeAffectedV1::Life
        && input.attribute_action == action
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
        && input.is_permanent
        && input.is_immediate_permanent == immediate
}

fn komboka_victory_pillz_and_life_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value: ShapeFieldV1::Exact(1),
            attribute: AttributeAffectedV1::LifeAndPillz,
            ..POST_ROUND_SHAPE
        },
    )
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
        | SupportedEffectV1::CancelOpponentCombatStatModifiers { .. }
        | SupportedEffectV1::ProtectOwnCombatStat { .. }
        | SupportedEffectV1::ProtectOwnAbility
        | SupportedEffectV1::ProtectOwnBonus
        | SupportedEffectV1::CopyOpponentPrintedCombatStat { .. } => return false,
    };
    let prefix = match multiplier {
        MagnitudeMultiplierV1::Growth => "Growth: ",
        MagnitudeMultiplierV1::Degrowth => "Degrowth: ",
        MagnitudeMultiplierV1::Fixed
        | MagnitudeMultiplierV1::Support
        | MagnitudeMultiplierV1::OpponentStars
        | MagnitudeMultiplierV1::OpponentDamage => return false,
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
                // Hattori `304`/`961` print the abbreviated stat. Same grammar, same shape:
                // the registry's structured fields are identical, only the spelling differs.
                || body == format!("-{value} Opp. Dmg, Min {min}")
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
                MagnitudeMultiplierV1::OpponentDamage => CombatStatMagnitudeV1::OpponentDamage,
            },
        }),
        SupportedEffectV1::StopOpponentAbility => Some(CombatStatEffectV1::StopOpponentAbility),
        SupportedEffectV1::StopOpponentBonus => Some(CombatStatEffectV1::StopOpponentBonus),
        SupportedEffectV1::CancelOpponentCombatStatModifiers { stat } => {
            Some(CombatStatEffectV1::CancelOpponentCombatStatModifiers {
                stat: compact_stat(stat),
            })
        }
        SupportedEffectV1::ProtectOwnCombatStat { stat } => {
            Some(CombatStatEffectV1::ProtectOwnCombatStat {
                stat: compact_stat(stat),
            })
        }
        SupportedEffectV1::ProtectOwnAbility => Some(CombatStatEffectV1::ProtectOwnAbility),
        SupportedEffectV1::ProtectOwnBonus => Some(CombatStatEffectV1::ProtectOwnBonus),
        SupportedEffectV1::CopyOpponentPrintedCombatStat { stat } => {
            Some(CombatStatEffectV1::CopyOpponentPrintedCombatStat {
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
    fn anita_courage_damage_life_is_exact_identity_description_and_shape_locked() {
        let registry = registry();
        let anita = registry
            .lookup_capture(274, "Courage: +1 Life Per Dmg")
            .unwrap();
        assert!(classify_anita_courage_damage_to_life(
            anita,
            CombatStatEffectSourceV1::Ability,
        ));
        assert!(!classify_anita_courage_damage_to_life(
            anita,
            CombatStatEffectSourceV1::Bonus,
        ));
        assert!(!anita_courage_damage_to_life_identity_matches(
            CombatStatEffectSourceV1::Ability,
            275,
        ));

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value = serde_json::from_reader(File::open(path).unwrap()).unwrap();
        for (field, value) in [
            ("value", serde_json::json!(2)),
            ("positionRequirement", serde_json::json!("both")),
            ("currentRoundRequirement", serde_json::json!("any")),
            ("specialAction", serde_json::json!("convert_dmg_to_pillz")),
            ("isPermanent", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed["274"]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            assert!(
                !classify_anita_courage_damage_to_life(
                    malformed
                        .lookup_capture(274, "Courage: +1 Life Per Dmg")
                        .unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                "mutated field {field}",
            );
        }
    }

    #[test]
    fn defeat_opponent_life_is_admitted_by_grammar_and_stays_ability_only() {
        let registry = registry();

        for (id, description, minimum) in [
            (1165, "Defeat: -2 Opp. Life, Min 0", 0),
            (959, "Defeat: -2 Opp. Life, Min 1", 1),
            (5434, "Defeat: -2 Opp. Life, Min 1", 1),
            (587, "Defeat: -2 Opp. Life, Min 2", 2),
            (5332, "Defeat: -2 Opp. Life, Min 3", 3),
            (5333, "Defeat: -2 Opp. Life, Min 3", 3),
        ] {
            let definition = registry.lookup_capture(id, description).unwrap();
            assert_eq!(
                classify_defeat_opponent_life(definition, CombatStatEffectSourceV1::Ability),
                Some((2, minimum)),
                "definition {id}",
            );
            // No clan bonus prints this text, so a bonus claiming it is not the same source.
            assert_eq!(
                classify_defeat_opponent_life(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "definition {id} as a bonus",
            );
            // It is post-round work, never a combat-stat modifier.
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                None,
                "definition {id} as a combat stat",
            );
        }

        // The printed numbers are authority: a record whose text disagrees with its own
        // structured magnitude or bound is refused rather than trusted either way.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (field, value) in [
            ("value", serde_json::json!(3)),
            ("valueMin", serde_json::json!(2)),
            ("currentRoundRequirement", serde_json::json!("win")),
        ] {
            let mut malformed = source.clone();
            malformed["959"]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            assert_eq!(
                classify_defeat_opponent_life(
                    malformed
                        .lookup_capture(959, "Defeat: -2 Opp. Life, Min 1")
                        .unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                None,
                "malformed {field}",
            );
        }
    }

    #[test]
    fn killshot_opponent_life_is_admitted_by_grammar_and_stays_ability_only() {
        let registry = registry();

        for (id, description, life, minimum) in [
            (1779, "Killshot: -2 Opp. Life Min 2", 2, 2),
            (1959, "Killshot: -3 Opp. Life Min 2", 3, 2),
            (4459, "Killshot: -3 Opp. Life Min 0", 3, 0),
            (5461, "Killshot: -3 Opp. Life Min 0", 3, 0),
            (5530, "Killshot: -3 Opp. Life Min 0", 3, 0),
            (1670, "Killshot: -4 Opp. Life Min 0", 4, 0),
            (4785, "Killshot: -5 Opp. Life Min 0", 5, 0),
            (1204, "Killshot: -6 Opp. Life Min 0", 6, 0),
        ] {
            let definition = registry.lookup_capture(id, description).unwrap();
            assert_eq!(
                classify_killshot_opponent_life(definition, CombatStatEffectSourceV1::Ability),
                Some((life, minimum)),
                "definition {id}",
            );
            // No clan bonus prints this text, so a bonus claiming it is not the same source.
            assert_eq!(
                classify_killshot_opponent_life(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "definition {id} as a bonus",
            );
            // It is post-round work, never a combat-stat modifier.
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                None,
                "definition {id} as a combat stat",
            );
            // The sibling grammars must not claim it: they differ in the current-round
            // requirement and, for Defeat, in the comma the text prints before `Min`.
            assert_eq!(
                classify_victory_opponent_life(definition, CombatStatEffectSourceV1::Ability),
                None,
                "definition {id} as Victory",
            );
            assert_eq!(
                classify_defeat_opponent_life(definition, CombatStatEffectSourceV1::Ability),
                None,
                "definition {id} as Defeat",
            );
        }

        // The printed numbers are authority, and so is the `sureshot` channel: a record
        // whose text disagrees with its own structured magnitude or bound, or which asks a
        // different outcome, is refused rather than trusted either way. `win` is the case
        // that matters most - it is the plain Victory reduction wearing Killshot's text.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (field, value) in [
            ("value", serde_json::json!(4)),
            ("valueMin", serde_json::json!(1)),
            ("currentRoundRequirement", serde_json::json!("win")),
            ("currentRoundRequirement", serde_json::json!("lose")),
            ("currentRoundRequirement", serde_json::json!("any")),
            ("sideAffected", serde_json::json!("player")),
            ("attributeAction", serde_json::json!("increase")),
            ("isPermanent", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed["1959"]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            assert_eq!(
                classify_killshot_opponent_life(
                    malformed
                        .lookup_capture(1959, "Killshot: -3 Opp. Life Min 2")
                        .unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                None,
                "malformed {field} = {value}",
            );
        }

        // The other Killshot grammars share the `sureshot` channel and must stay out of
        // this one: they move a different resource, or the owner's own.
        for (id, description) in [
            (2250, "Killshot: +3 Pillz"),
            (1231, "Killshot: +3 Life"),
            (1768, "Killshot: +2 Pillz And Life"),
            (2497, "Killshot: Toxin 1, Min 0"),
            (5776, "Killshot: -2 Opp. Pillz And Life, Min 0"),
        ] {
            let definition = registry.lookup_capture(id, description).unwrap();
            assert_eq!(
                classify_killshot_opponent_life(definition, CombatStatEffectSourceV1::Ability),
                None,
                "definition {id} is a different Killshot grammar",
            );
        }
    }

    #[test]
    fn copy_is_admitted_by_grammar_and_excludes_every_unreviewed_variant() {
        let registry = registry();
        // The registry holds many structurally identical Copy definitions; each is admitted
        // on its own grammar rather than through a fixed identity list. Every admitted
        // grammar appears here, including the conditional ones and the site's inconsistent
        // punctuation between a copied Bonus and a copied Ability.
        for (id, description, copied, predicate) in [
            (
                764,
                "Copy: Opp. Bonus",
                CopiedSourceKindV1::Bonus,
                CombatStatPredicateV1::Always,
            ),
            (
                846,
                "Copy: Opp. Bonus",
                CopiedSourceKindV1::Bonus,
                CombatStatPredicateV1::Always,
            ),
            (
                4774,
                "Copy: Opp. Bonus",
                CopiedSourceKindV1::Bonus,
                CombatStatPredicateV1::Always,
            ),
            (
                2918,
                "Copy: Opp. Ability",
                CopiedSourceKindV1::Ability,
                CombatStatPredicateV1::Always,
            ),
            (
                4497,
                "Copy: Opp. Ability",
                CopiedSourceKindV1::Ability,
                CombatStatPredicateV1::Always,
            ),
            (
                958,
                "Reprisal: Copy Opp. Bonus",
                CopiedSourceKindV1::Bonus,
                CombatStatPredicateV1::OwnerMovesSecond,
            ),
            (
                5453,
                "Reprisal: Copy Opp. Bonus",
                CopiedSourceKindV1::Bonus,
                CombatStatPredicateV1::OwnerMovesSecond,
            ),
            (
                3101,
                "Reprisal: Copy: Opp. Ability",
                CopiedSourceKindV1::Ability,
                CombatStatPredicateV1::OwnerMovesSecond,
            ),
            (
                1751,
                "Revenge: Copy Opp. Bonus",
                CopiedSourceKindV1::Bonus,
                CombatStatPredicateV1::OwnerLostPreviousRound,
            ),
            (
                4972,
                "Revenge: Copy: Opp. Ability",
                CopiedSourceKindV1::Ability,
                CombatStatPredicateV1::OwnerLostPreviousRound,
            ),
            (
                3291,
                "Asymmetry: Copy: Opp. Ability",
                CopiedSourceKindV1::Ability,
                CombatStatPredicateV1::SelectedHandSlotsDiffer,
            ),
            (
                2482,
                "Asymmetry: Copy: Opp. Bonus",
                CopiedSourceKindV1::Bonus,
                CombatStatPredicateV1::SelectedHandSlotsDiffer,
            ),
        ] {
            assert_eq!(
                classify_copy_opponent_source(registry.lookup_capture(id, description).unwrap()),
                Some((copied, predicate)),
                "grammar {id}",
            );
        }
        // Every other conditional keeps its own deferred grammar, and a stat-copying
        // variant is never a source copy: since revision 24 the unconditional ones are
        // admitted as their own effect, and `4126` matters in particular because it is a
        // Reprisal Copy, but of a stat.
        for (id, description) in [
            (3994, "Unison : Copy: Opp. Ability"),
            (1409, "Confidence: Copy: Opp. Power"),
            (5304, "Bet > 3 Pillz: Copy: Opp. Ability"),
            (
                5073,
                "[clan:46][clan:58][clan:40][clan:55][clan:42][clan:50] Asy. : Copy: Opp. Ability",
            ),
            (315, "Copy: Opp. Power"),
            (1513, "Copy: Opp. Damage"),
            (2673, "Copy: Power And Damage Opp."),
            (4126, "Reprisal: Copy: Opp. Damage"),
        ] {
            assert_eq!(
                classify_copy_opponent_source(registry.lookup_capture(id, description).unwrap()),
                None,
                "variant {id}",
            );
        }
        // A conditional grammar may not borrow another condition's structured field: the
        // printed prefix and the structured record have to name the same one.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let crossed: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (id, field, value) in [
            // Reprisal's condition lives in positionRequirement, not previousRoundRequirement.
            (958, "positionRequirement", serde_json::json!("both")),
            (958, "previousRoundRequirement", serde_json::json!("lose")),
            // ...and Revenge's the other way around.
            (1751, "previousRoundRequirement", serde_json::json!("any")),
            (1751, "positionRequirement", serde_json::json!("defender")),
        ] {
            let mut malformed = crossed.clone();
            malformed[id.to_string()]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            let definition = malformed.get(id).unwrap();
            assert_eq!(
                classify_copy_opponent_source(definition),
                None,
                "crossed {id} {field}",
            );
        }
        // An otherwise-exact record with any extra context is not this grammar.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value = serde_json::from_reader(File::open(path).unwrap()).unwrap();
        for (field, value) in [
            ("specialAction", serde_json::json!("copy_bonus")),
            ("currentRoundRequirement", serde_json::json!("win")),
            ("indexRequirement", serde_json::json!("symmetry")),
            ("isPermanent", serde_json::json!(true)),
            ("value", serde_json::json!(1)),
        ] {
            let mut malformed = source.clone();
            malformed["2918"]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            assert_eq!(
                classify_copy_opponent_source(
                    malformed
                        .lookup_capture(2918, "Copy: Opp. Ability")
                        .unwrap()
                ),
                None,
                "mutated field {field}",
            );
        }
    }

    #[test]
    fn victory_pillz_carries_only_the_confidence_predicate_its_prefix_names() {
        let registry = registry();
        // The plain grammar is unchanged and still unconditional.
        let plain = registry.lookup_capture(337, "+3 Pillz").unwrap();
        assert_eq!(
            classify_victory_pillz(plain, CombatStatEffectSourceV1::Ability),
            Some((3, CombatStatPredicateV1::Always))
        );
        // Both printed `Confidence:` records carry the previous-round predicate.
        for (id, description, pillz) in [
            (1702, "Confidence: +4 Pillz", 4),
            (4449, "Confidence: +2 Pillz", 2),
        ] {
            let confidence = registry.lookup_capture(id, description).unwrap();
            assert_eq!(
                classify_victory_pillz(confidence, CombatStatEffectSourceV1::Ability),
                Some((pillz, CombatStatPredicateV1::OwnerWonPreviousRound)),
                "confidence {id}",
            );
            // No clan bonus prints it, so the Bonus slot stays a hazard.
            assert_eq!(
                classify_victory_pillz(confidence, CombatStatEffectSourceV1::Bonus),
                None,
                "confidence {id} as bonus",
            );
        }
        // Victory Life's `Confidence :` is spaced and affects the other resource, so the
        // exact-text check keeps the two prefixed grammars apart in both directions.
        let confidence_life = registry
            .lookup_capture(814, "Confidence : +4 Life")
            .unwrap();
        assert_eq!(
            classify_victory_pillz(confidence_life, CombatStatEffectSourceV1::Ability),
            None
        );
        // The other prefixed Pillz forms differ in a structured field and keep their
        // visible-but-disabled records rather than borrowing this grammar.
        for (id, description) in [(2250, "Killshot: +3 Pillz"), (4645, "Killshot: +2 Pillz")] {
            let other = registry.lookup_capture(id, description).unwrap();
            assert_eq!(
                classify_victory_pillz(other, CombatStatEffectSourceV1::Ability),
                None,
                "other {id}",
            );
            assert!(!has_victory_pillz_shape(other), "other {id} shape");
        }
    }

    #[test]
    fn both_players_life_reduction_is_source_kind_text_and_shape_locked() {
        let registry = registry();
        // The two printed levels are separate registry records with the same text and
        // shape, so each is admitted on its own rather than as an alias of the other.
        for id in [1379, 5198] {
            let xantiax = registry
                .lookup_capture(id, "Xantiax: -3 Life, Min. 0")
                .unwrap();
            assert_eq!(
                classify_both_players_life_reduction(xantiax, CombatStatEffectSourceV1::Ability),
                Some((3, 0)),
                "xantiax {id}",
            );
            // No clan bonus prints it, so the Bonus slot is a hazard rather than a source.
            assert_eq!(
                classify_both_players_life_reduction(xantiax, CombatStatEffectSourceV1::Bonus),
                None,
                "xantiax {id} as bonus",
            );
            assert!(has_both_players_life_reduction_shape(xantiax), "shape {id}");
        }
        // Every neighbouring Life reduction names an outcome or a single side, which is
        // exactly what this grammar requires neutral, so none of them can reach it.
        for (id, description) in [
            (680, "-2 Opp. Life Min 2"),
            (1399, "-5 Opp. Life Min 5"),
            (1628, "Victory Or Defeat: - 1 Opp. Life Min 1"),
            (4708, "Symmetry: - 4 Opp. Life Min 0"),
        ] {
            let other = registry.lookup_capture(id, description).unwrap();
            assert_eq!(
                classify_both_players_life_reduction(other, CombatStatEffectSourceV1::Ability),
                None,
                "neighbour {id}",
            );
            assert!(
                !has_both_players_life_reduction_shape(other),
                "neighbour {id} shape",
            );
        }
    }

    #[test]
    fn victory_opponent_life_is_identity_source_kind_and_shape_locked() {
        let registry = registry();
        let mou = registry.lookup_capture(1399, "-5 Opp. Life Min 5").unwrap();
        let berzerk = registry.lookup_capture(680, "-2 Opp. Life Min 2").unwrap();
        assert_eq!(
            classify_victory_opponent_life(mou, CombatStatEffectSourceV1::Ability),
            Some((5, 5, CombatStatPredicateV1::Always))
        );
        assert_eq!(
            classify_victory_opponent_life(berzerk, CombatStatEffectSourceV1::Bonus),
            Some((2, 2, CombatStatPredicateV1::Always))
        );
        // The source kind is authority in both directions.
        assert_eq!(
            classify_victory_opponent_life(mou, CombatStatEffectSourceV1::Bonus),
            None
        );
        assert_eq!(
            classify_victory_opponent_life(berzerk, CombatStatEffectSourceV1::Ability),
            None
        );
        // The reviewed conditional members carry the one predicate their printed text names.
        // Diabolus prints the same effect at both levels under two byte-identical registry
        // records, so each is admitted on its own rather than as an alias of the other.
        for (id, description, life, predicate) in [
            (
                4708,
                "Symmetry: - 4 Opp. Life Min 0",
                4,
                CombatStatPredicateV1::SelectedHandSlotsMatch,
            ),
            (
                3016,
                "Confidence: -3 Opp. Life, Min 0",
                3,
                CombatStatPredicateV1::OwnerWonPreviousRound,
            ),
            (
                4301,
                "Confidence: -3 Opp. Life, Min 0",
                3,
                CombatStatPredicateV1::OwnerWonPreviousRound,
            ),
        ] {
            let conditional = registry.lookup_capture(id, description).unwrap();
            assert_eq!(
                classify_victory_opponent_life(conditional, CombatStatEffectSourceV1::Ability),
                Some((life, 0, predicate)),
                "conditional {id}",
            );
            // None of them is a clan bonus, and none may be borrowed through that slot.
            assert_eq!(
                classify_victory_opponent_life(conditional, CombatStatEffectSourceV1::Bonus),
                None,
                "conditional {id} as bonus",
            );
        }
        // Courage `4533` has no selected observation anywhere in the corpus and Growth
        // `1730` is a round-scaled magnitude rather than a predicate, so both stay deferred
        // although they share this exact structure.
        for (id, description) in [
            (4533, "Courage: - 3 Opp. Life Min 0"),
            (1730, "Growth: - 1 Opp. Life Min 4"),
        ] {
            let deferred = registry.lookup_capture(id, description).unwrap();
            assert_eq!(
                classify_victory_opponent_life(deferred, CombatStatEffectSourceV1::Ability),
                None,
                "deferred {id}",
            );
        }
        // A reviewed conditional may not pair its magnitude with a different condition.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let crossed: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (id, field, value) in [
            (4708, "indexRequirement", serde_json::json!("asymmetry")),
            (4708, "previousRoundRequirement", serde_json::json!("win")),
            (3016, "previousRoundRequirement", serde_json::json!("lose")),
            (3016, "positionRequirement", serde_json::json!("attacker")),
        ] {
            let mut malformed = crossed.clone();
            malformed[id.to_string()]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            assert_eq!(
                classify_victory_opponent_life(
                    malformed.get(id).unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                None,
                "crossed {id} {field}",
            );
        }
        // A mutated record loses its exact identity even under the right id and text.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value = serde_json::from_reader(File::open(path).unwrap()).unwrap();
        for (field, value) in [
            ("value", serde_json::json!(4)),
            ("valueMin", serde_json::json!(0)),
            ("currentRoundRequirement", serde_json::json!("any")),
            ("indexRequirement", serde_json::json!("symmetry")),
            ("sideAffected", serde_json::json!("player")),
            ("isOverdrive", serde_json::json!(true)),
            ("isPermanent", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed["1399"]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            assert_eq!(
                classify_victory_opponent_life(
                    malformed
                        .lookup_capture(1399, "-5 Opp. Life Min 5")
                        .unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                None,
                "mutated field {field}",
            );
        }
    }

    #[test]
    fn unconditional_opponent_life_is_admitted_by_grammar_on_both_outcome_channels() {
        let registry = registry();

        // Every printed ability that carries the plain Victory reduction, whatever its
        // magnitude and bound. `680` is absent because no ability prints it.
        for (id, life, minimum) in [
            (512, 3, 3),
            (524, 2, 1),
            (594, 3, 0),
            (602, 4, 0),
            (769, 3, 0),
            (842, 3, 3),
            (935, 2, 1),
            (1002, 2, 0),
            (1399, 5, 5),
            (3491, 6, 0),
            (3571, 3, 1),
            (3716, 2, 0),
            (4948, 5, 1),
        ] {
            let description = format!("-{life} Opp. Life Min {minimum}");
            let definition = registry.lookup_capture(id, &description).unwrap();
            assert_eq!(
                classify_victory_opponent_life(definition, CombatStatEffectSourceV1::Ability),
                Some((life, minimum, CombatStatPredicateV1::Always)),
                "definition {id}",
            );
            // Only the reviewed Berzerk record is a clan bonus; nothing else may claim that
            // slot, and none of these is ever a combat-stat modifier.
            assert_eq!(
                classify_victory_opponent_life(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "definition {id} as a bonus",
            );
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                None,
                "definition {id} as a combat stat",
            );
        }

        // The same grammar on the channel that pays whatever the outcome. Uuber's `1628`
        // is the one member a clan bonus also prints, so it keeps its reviewed identity.
        for (id, life, minimum, bonus_too) in [
            (1386, 1, 0, false),
            (1628, 1, 1, true),
            (1726, 2, 1, false),
            (3367, 1, 0, false),
            (4331, 2, 4, false),
        ] {
            let description = format!("Victory Or Defeat: - {life} Opp. Life Min {minimum}");
            let definition = registry.lookup_capture(id, &description).unwrap();
            let expected = Some(VictoryOrDefeatLifeEffectV1::ReduceOpponentLife { life, minimum });
            assert_eq!(
                classify_victory_or_defeat_life(definition, CombatStatEffectSourceV1::Ability),
                expected,
                "definition {id}",
            );
            assert_eq!(
                classify_victory_or_defeat_life(definition, CombatStatEffectSourceV1::Bonus),
                if bonus_too { expected } else { None },
                "definition {id} as a bonus",
            );
        }

        // `Night:` prints the complete Victory shape under text the grammar does not name,
        // so it is refused here and reported as a structural near-miss instead.
        let night = registry
            .lookup_capture(4750, "Night: -2 Opp. Life Min 0")
            .unwrap();
        assert_eq!(
            classify_victory_opponent_life(night, CombatStatEffectSourceV1::Ability),
            None,
        );
        assert!(has_victory_opponent_life_shape(night));

        // The printed numbers are authority on both channels: a record whose text disagrees
        // with its own magnitude or bound is refused rather than trusted either way.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (field, victory_value, victory_or_defeat_value) in [
            ("value", serde_json::json!(2), serde_json::json!(3)),
            ("valueMin", serde_json::json!(1), serde_json::json!(2)),
            ("valueMax", serde_json::json!(6), serde_json::json!(6)),
            (
                "currentRoundRequirement",
                serde_json::json!("lose"),
                serde_json::json!("lose"),
            ),
            (
                "indexRequirement",
                serde_json::json!("symmetry"),
                serde_json::json!("symmetry"),
            ),
            (
                "isPermanent",
                serde_json::json!(true),
                serde_json::json!(true),
            ),
        ] {
            let mut malformed = source.clone();
            malformed["594"]["abilityData"][field] = victory_value;
            malformed["1726"]["abilityData"][field] = victory_or_defeat_value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            assert_eq!(
                classify_victory_opponent_life(
                    malformed.get(594).unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                None,
                "malformed 594 {field}",
            );
            assert_eq!(
                classify_victory_or_defeat_life(
                    malformed.get(1726).unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                None,
                "malformed 1726 {field}",
            );
        }
    }

    #[test]
    fn observed_previous_round_inventory_is_exact_and_fail_closed() {
        let registry = registry();
        // `591` arrived with the 2026-09-20 Dojo captures, and it is admitted rather than
        // deferred even though it prints `Confidence : -1 Opp. Power, Min 1` with a space
        // before the colon: the registry compiler's prefix match already tolerates that
        // spacing, so the ordinary reviewed Confidence grammar picks it up unchanged. The
        // inventory is the only thing that had to learn about it.
        let admitted = BTreeSet::from([
            463, 465, 478, 520, 553, 555, 556, 560, 585, 591, 634, 784, 801, 859, 883, 884, 921,
            938, 965, 1053, 1091, 1107, 1278, 1286, 1303, 1395, 1417, 1839, 2628, 2657, 3827, 3829,
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
    fn victory_or_defeat_life_is_identity_description_and_shape_locked() {
        let registry = registry();
        for (id, description, expected) in [
            (
                1396,
                "Victory Or Defeat : +1 Life",
                VictoryOrDefeatLifeEffectV1::GainLife { life: 1 },
            ),
            (
                2992,
                "Victory Or Defeat : +1 Life",
                VictoryOrDefeatLifeEffectV1::GainLife { life: 1 },
            ),
            (
                5835,
                "Victory Or Defeat : +1 Life",
                VictoryOrDefeatLifeEffectV1::GainLife { life: 1 },
            ),
            (
                5799,
                "Victory Or Defeat : +1 Life",
                VictoryOrDefeatLifeEffectV1::GainLife { life: 1 },
            ),
            (
                5802,
                "Victory Or Defeat : +2 Life",
                VictoryOrDefeatLifeEffectV1::GainLife { life: 2 },
            ),
            (
                2944,
                "Victory Or Defeat : +2 Life",
                VictoryOrDefeatLifeEffectV1::GainLife { life: 2 },
            ),
            (
                1628,
                "Victory Or Defeat: - 1 Opp. Life Min 1",
                VictoryOrDefeatLifeEffectV1::ReduceOpponentLife {
                    life: 1,
                    minimum: 1,
                },
            ),
        ] {
            let definition = registry.lookup_capture(id, description).unwrap();
            for source_kind in [
                CombatStatEffectSourceV1::Ability,
                CombatStatEffectSourceV1::Bonus,
            ] {
                assert_eq!(
                    classify_victory_or_defeat_life(definition, source_kind),
                    Some(expected),
                    "id={id} source={source_kind:?}",
                );
            }
        }

        let same_text_non_authority = registry
            .lookup_capture(5835, "Victory Or Defeat : +1 Life")
            .unwrap();
        assert!(!victory_or_defeat_life_identity_matches(
            CombatStatEffectSourceV1::Ability,
            5834,
        ));
        assert_eq!(
            classify_victory_or_defeat_life(
                same_text_non_authority,
                CombatStatEffectSourceV1::Ability,
            ),
            Some(VictoryOrDefeatLifeEffectV1::GainLife { life: 1 }),
        );

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value = serde_json::from_reader(File::open(path).unwrap()).unwrap();
        for (field, value) in [
            ("value", serde_json::json!(2)),
            ("valueMin", serde_json::json!(0)),
            ("currentRoundRequirement", serde_json::json!("win")),
            ("sideAffected", serde_json::json!("opponent")),
            ("attributeAction", serde_json::json!("decrease")),
            ("isPermanent", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed["1396"]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            assert_eq!(
                classify_victory_or_defeat_life(
                    malformed
                        .lookup_capture(1396, "Victory Or Defeat : +1 Life")
                        .unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                None,
                "mutated field {field}",
            );
        }

        let mut malformed = source;
        malformed["1628"]["description"] =
            serde_json::json!("Victory Or Defeat : - 1 Opp. Life Min 1");
        let malformed =
            EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                .unwrap();
        assert_eq!(
            classify_victory_or_defeat_life(
                malformed
                    .lookup_capture(1628, "Victory Or Defeat : - 1 Opp. Life Min 1")
                    .unwrap(),
                CombatStatEffectSourceV1::Bonus,
            ),
            None,
        );
    }

    #[test]
    fn equalizer_opponent_life_is_exact_identity_description_and_shape_locked() {
        let registry = registry();
        for id in [1415, 4458] {
            let definition = registry
                .lookup_capture(id, "Equalizer: - 1 Opp. Life Min 2")
                .unwrap();
            for source_kind in [
                CombatStatEffectSourceV1::Ability,
                CombatStatEffectSourceV1::Bonus,
            ] {
                assert_eq!(
                    classify_equalizer_opponent_life_on_victory(definition, source_kind),
                    Some((1, 2)),
                    "id={id} source={source_kind:?}",
                );
            }
        }
        assert!(!equalizer_opponent_life_on_victory_identity_matches(
            CombatStatEffectSourceV1::Ability,
            1414,
        ));

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value = serde_json::from_reader(File::open(path).unwrap()).unwrap();
        for (field, value) in [
            ("value", serde_json::json!(2)),
            ("valueMin", serde_json::json!(1)),
            ("currentRoundRequirement", serde_json::json!("any")),
            ("sideAffected", serde_json::json!("player")),
            ("attributeAction", serde_json::json!("increase")),
            ("isOppStarsLinked", serde_json::json!(false)),
            ("isPermanent", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed["1415"]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            assert_eq!(
                classify_equalizer_opponent_life_on_victory(
                    malformed
                        .lookup_capture(1415, "Equalizer: - 1 Opp. Life Min 2")
                        .unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                None,
                "mutated field {field}",
            );
        }
    }

    #[test]
    fn reprisal_stop_opponent_ability_is_identity_and_shape_locked() {
        let registry = registry();
        for id in [1310, 2073] {
            let definition = registry
                .lookup_capture(id, "Reprisal: Stop Opp. Ability")
                .unwrap();
            assert!(classify_reprisal_stop_opponent_ability(
                definition,
                CombatStatEffectSourceV1::Ability,
            ));
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                Some((
                    SupportedEffectV1::StopOpponentAbility,
                    CombatStatPredicateV1::OwnerMovesSecond,
                )),
            );
            assert!(!classify_reprisal_stop_opponent_ability(
                definition,
                CombatStatEffectSourceV1::Bonus,
            ));
        }

        // The existing unconditional admission stays independent of the new identity gate.
        let unconditional = registry.lookup_capture(1341, "Stop Opp. Ability").unwrap();
        assert_eq!(
            classify_combat_stat_effect(unconditional, CombatStatEffectSourceV1::Ability),
            Some((
                SupportedEffectV1::StopOpponentAbility,
                CombatStatPredicateV1::Always,
            )),
        );

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value = serde_json::from_reader(File::open(path).unwrap()).unwrap();
        for (field, value) in [
            ("value", serde_json::json!(1)),
            ("positionRequirement", serde_json::json!("both")),
            ("currentRoundRequirement", serde_json::json!("win")),
            ("attributeAffected", serde_json::json!("pwr")),
            ("specialAction", serde_json::json!("stop_bonus")),
            ("isSupport", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed["1310"]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            assert!(
                !classify_reprisal_stop_opponent_ability(
                    malformed
                        .lookup_capture(1310, "Reprisal: Stop Opp. Ability")
                        .unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                "mutated field {field}",
            );
        }
        let mut malformed = source;
        malformed["1310"]["description"] = serde_json::json!("Reprisal: Stop Opp. Bonus");
        let malformed =
            EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                .unwrap();
        assert!(!classify_reprisal_stop_opponent_ability(
            malformed
                .lookup_capture(1310, "Reprisal: Stop Opp. Bonus")
                .unwrap(),
            CombatStatEffectSourceV1::Ability,
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
            assert_eq!(
                classify_victory_life(dave, source),
                Some((2, CombatStatPredicateV1::Always))
            );
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

    #[test]
    fn heal_life_admits_the_plain_ability_grammar_by_text_and_full_permanent_shape() {
        let registry = registry();
        // Every plain record in the registry, each with its own printed numbers.
        for (id, description, expected) in [
            (649, "Heal 1 Max. 6", (1, 6)),
            (751, "Heal 2 Max. 10", (2, 10)),
            (963, "Heal 1 Max. 15", (1, 15)),
            (1501, "Heal 2 Max. 10", (2, 10)),
            (3118, "Heal 1 Max. 18", (1, 18)),
            (3526, "Heal 1 Max. 20", (1, 20)),
            (4625, "Heal 1 Max. 15", (1, 15)),
            (5341, "Heal 1 Max. 18", (1, 18)),
        ] {
            let definition = registry.lookup_capture(id, description).unwrap();
            let (life, maximum) = expected;
            assert_eq!(
                classify_heal_life_on_victory(definition, CombatStatEffectSourceV1::Ability),
                Some((life, maximum, CombatStatPredicateV1::Always)),
                "{id} {description}"
            );
            assert_eq!(
                classify_heal_life_on_victory(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "{id} {description} as a bonus"
            );
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                None,
                "{id} {description} must never be an ordinary one-round effect"
            );
        }
        // The hand-slot prefix is admitted as the plan's predicate since revision 32.
        assert_eq!(
            classify_heal_life_on_victory(
                registry
                    .lookup_capture(5692, "Asymmetry: Heal 1 Max. 16")
                    .unwrap(),
                CombatStatEffectSourceV1::Ability,
            ),
            Some((1, 16, CombatStatPredicateV1::SelectedHandSlotsDiffer))
        );
        // Other latch conditions are other texts and other structured fields.
        for (id, description) in [
            (1625, "Defeat : Heal 1 Max. 13"),
            (898, "Defeat : Heal 1 Max. 15"),
            (
                5578,
                "[clan:26][clan:37][clan:55][clan:10] Defeat: Heal 1, Max 14",
            ),
        ] {
            assert_eq!(
                classify_heal_life_on_victory(
                    registry.lookup_capture(id, description).unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                None,
                "{id} {description}"
            );
        }

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value = serde_json::from_reader(File::open(path).unwrap()).unwrap();
        for (field, value) in [
            ("value", serde_json::json!(2)),
            ("valueMax", serde_json::json!(19)),
            ("valueMin", serde_json::json!(0)),
            ("currentRoundRequirement", serde_json::json!("any")),
            ("indexRequirement", serde_json::json!("asymmetry")),
            ("isPermanent", serde_json::json!(false)),
            ("isImmediatePermanent", serde_json::json!(true)),
            ("sideAffected", serde_json::json!("opponent")),
            ("attributeAffected", serde_json::json!("pillz")),
        ] {
            let mut malformed = source.clone();
            malformed["3526"]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            assert_eq!(
                classify_heal_life_on_victory(
                    malformed.lookup_capture(3526, "Heal 1 Max. 20").unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                None,
                "mutated field {field}"
            );
        }
    }

    #[test]
    fn toxin_poison_and_regen_admit_their_plain_grammars_and_refuse_every_prefixed_form() {
        let registry = registry();
        for (id, description) in [
            (1197, "Toxin 1, Min 0"),
            (1508, "Toxin 1, Min 0"),
            (1840, "Toxin 1, Min 0"),
            (4730, "Toxin 1, Min 0"),
            (5037, "Toxin 1, Min 0"),
            (5098, "Toxin 1, Min 0"),
            (5638, "Toxin 1, Min 0"),
            (5639, "Toxin 1, Min 0"),
            (5640, "Toxin 1, Min 0"),
        ] {
            let definition = registry.lookup_capture(id, description).unwrap();
            assert_eq!(
                classify_toxin_opponent_life_on_victory(
                    definition,
                    CombatStatEffectSourceV1::Ability
                ),
                Some((1, 0, CombatStatPredicateV1::Always)),
                "{id}"
            );
            assert_eq!(
                classify_toxin_opponent_life_on_victory(
                    definition,
                    CombatStatEffectSourceV1::Bonus
                ),
                None,
                "{id} as a bonus"
            );
            assert_eq!(
                classify_poison_opponent_life_on_victory(
                    definition,
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "{id} is not Poison"
            );
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                None
            );
        }
        for (id, description, expected) in [
            (206, "Poison 2, Min 3", (2, 3)),
            (325, "Poison 1, Min 5", (1, 5)),
            (509, "Poison 2, Min 1", (2, 1)),
            (566, "Poison 1, Min 0", (1, 0)),
            (582, "Poison 2, Min 4", (2, 4)),
            (682, "Poison 1, Min 2", (1, 2)),
            (1345, "Poison 2, Min 4", (2, 4)),
            (1385, "Poison 2, Min 2", (2, 2)),
            (3088, "Poison 1, Min 0", (1, 0)),
            (3603, "Poison 1, Min 0", (1, 0)),
            (5901, "Poison 1, Min 2", (1, 2)),
        ] {
            let definition = registry.lookup_capture(id, description).unwrap();
            for source_kind in [
                CombatStatEffectSourceV1::Ability,
                CombatStatEffectSourceV1::Bonus,
            ] {
                let (life, minimum) = expected;
                assert_eq!(
                    classify_poison_opponent_life_on_victory(definition, source_kind),
                    Some((life, minimum, CombatStatPredicateV1::Always)),
                    "{id} {source_kind:?}"
                );
                assert_eq!(
                    classify_toxin_opponent_life_on_victory(definition, source_kind),
                    None
                );
                assert_eq!(classify_combat_stat_effect(definition, source_kind), None);
            }
        }
        for (id, description, expected) in [
            (1458, "Regen 3, Max. 6", (3, 6)),
            (3433, "Regen 2, Max. 8", (2, 8)),
        ] {
            let definition = registry.lookup_capture(id, description).unwrap();
            let (life, maximum) = expected;
            assert_eq!(
                classify_regen_life_on_victory(definition, CombatStatEffectSourceV1::Ability),
                Some((life, maximum, CombatStatPredicateV1::Always)),
                "{id}"
            );
            assert_eq!(
                classify_regen_life_on_victory(definition, CombatStatEffectSourceV1::Bonus),
                None
            );
            assert_eq!(
                classify_heal_life_on_victory(definition, CombatStatEffectSourceV1::Ability),
                None
            );
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                None
            );
        }
        // Since revision 32 a hand-slot or previous-round prefix is the plan's predicate.
        assert_eq!(
            classify_toxin_opponent_life_on_victory(
                registry
                    .lookup_capture(5092, "Symmetry: Toxin 3, Min 0")
                    .unwrap(),
                CombatStatEffectSourceV1::Ability,
            ),
            Some((3, 0, CombatStatPredicateV1::SelectedHandSlotsMatch))
        );
        assert_eq!(
            classify_poison_opponent_life_on_victory(
                registry
                    .lookup_capture(3301, "Revenge: Poison 2, Min 0")
                    .unwrap(),
                CombatStatEffectSourceV1::Ability,
            ),
            Some((2, 0, CombatStatPredicateV1::OwnerLostPreviousRound))
        );
        assert_eq!(
            classify_regen_life_on_victory(
                registry
                    .lookup_capture(5693, "Asymmetry: Regen 1, Max. 17")
                    .unwrap(),
                CombatStatEffectSourceV1::Ability,
            ),
            Some((1, 17, CombatStatPredicateV1::SelectedHandSlotsDiffer))
        );
        // Every other prefixed form is another text with another structured condition.
        for (id, description) in [
            (2497, "Killshot: Toxin 1, Min 0"),
            (4210, "Victory Or Defeat: Toxin 1, Min 0"),
            (5316, "Unison : Toxin 1, Min 0"),
            (
                5613,
                "[clan:55][clan:50][clan:49][clan:44][clan:60] Toxin 1, Min 1",
            ),
            (1266, "Growth: Poison 1, Min 2"),
            (1282, "Growth: Poison 1, Min 1"),
            (4033, "Unison : Poison 1, Min 2"),
            (4124, "Backlash: Poison 1, Min 3"),
            (4561, "Defeat: Poison 1, Min 3"),
            (5594, "Perfect: Regen 1, Max. 17"),
        ] {
            let definition = registry.lookup_capture(id, description).unwrap();
            for source_kind in [
                CombatStatEffectSourceV1::Ability,
                CombatStatEffectSourceV1::Bonus,
            ] {
                assert_eq!(
                    classify_toxin_opponent_life_on_victory(definition, source_kind),
                    None,
                    "{id}"
                );
                assert_eq!(
                    classify_poison_opponent_life_on_victory(definition, source_kind),
                    None,
                    "{id}"
                );
                assert_eq!(
                    classify_regen_life_on_victory(definition, source_kind),
                    None,
                    "{id}"
                );
            }
        }
        // Immediacy is the one structured field that tells Toxin from Poison and Regen from
        // Heal; flipping it on a record makes its own text a lie and closes it.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value = serde_json::from_reader(File::open(path).unwrap()).unwrap();
        for (id, description, field, value) in [
            (
                "1197",
                "Toxin 1, Min 0",
                "isImmediatePermanent",
                serde_json::json!(false),
            ),
            (
                "206",
                "Poison 2, Min 3",
                "isImmediatePermanent",
                serde_json::json!(true),
            ),
            ("206", "Poison 2, Min 3", "valueMin", serde_json::json!(2)),
            (
                "206",
                "Poison 2, Min 3",
                "sideAffected",
                serde_json::json!("player"),
            ),
            (
                "1458",
                "Regen 3, Max. 6",
                "isImmediatePermanent",
                serde_json::json!(false),
            ),
            ("1458", "Regen 3, Max. 6", "valueMax", serde_json::json!(7)),
        ] {
            let mut malformed = source.clone();
            malformed[id]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            let definition = malformed
                .lookup_capture(id.parse().unwrap(), description)
                .unwrap();
            assert_eq!(
                classify_toxin_opponent_life_on_victory(
                    definition,
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "{id} {field}"
            );
            assert_eq!(
                classify_poison_opponent_life_on_victory(
                    definition,
                    CombatStatEffectSourceV1::Bonus
                ),
                None,
                "{id} {field}"
            );
            assert_eq!(
                classify_regen_life_on_victory(definition, CombatStatEffectSourceV1::Ability),
                None,
                "{id} {field}"
            );
        }
    }

    #[test]
    fn komboka_victory_pillz_and_life_is_bonus_1714_with_the_complete_reviewed_shape() {
        let registry = registry();
        let komboka = registry.lookup_capture(1714, "+1 Pillz And Life").unwrap();
        assert!(classify_komboka_victory_pillz_and_life(
            komboka,
            CombatStatEffectSourceV1::Bonus,
        ));
        assert!(!classify_komboka_victory_pillz_and_life(
            komboka,
            CombatStatEffectSourceV1::Ability,
        ));
        assert_eq!(
            classify_combat_stat_effect(komboka, CombatStatEffectSourceV1::Bonus),
            None,
        );

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value = serde_json::from_reader(File::open(path).unwrap()).unwrap();
        for (field, value) in [
            ("value", serde_json::json!(2)),
            ("valueMin", serde_json::json!(1)),
            ("positionRequirement", serde_json::json!("attacker")),
            ("previousRoundRequirement", serde_json::json!("win")),
            ("currentRoundRequirement", serde_json::json!("any")),
            ("attributeAffected", serde_json::json!("life")),
            ("attributeAction", serde_json::json!("decrease")),
            ("isPillzLinked", serde_json::json!(true)),
            ("isPermanent", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed["1714"]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                    .unwrap();
            assert!(
                !classify_komboka_victory_pillz_and_life(
                    malformed.lookup_capture(1714, "+1 Pillz And Life").unwrap(),
                    CombatStatEffectSourceV1::Bonus,
                ),
                "mutated field {field}",
            );
        }
        let mut malformed = source;
        malformed["1714"]["description"] = serde_json::json!("+1 Life And Pillz");
        let malformed =
            EffectRegistryV1::from_reader(serde_json::to_vec(&malformed).unwrap().as_slice())
                .unwrap();
        assert!(!classify_komboka_victory_pillz_and_life(
            malformed.lookup_capture(1714, "+1 Life And Pillz").unwrap(),
            CombatStatEffectSourceV1::Bonus,
        ));
    }
}
