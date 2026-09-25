//! Cold, source-independent compilation for the bounded combat-stat projection.
//!
//! Replay captures and catalog-built matches use different identity lookups, but both must
//! apply exactly the same reviewed semantic policy before producing hot-path plans.

use super::CopiedSourceKindV1;
use super::{
    ClanConjunctV1, ClanSetV1, CombatStatAffectedSideV1, CombatStatAttributeV1,
    CombatStatCardPlanV1, CombatStatEffectSourceV1, CombatStatEffectV1, CombatStatMagnitudeV1,
    CombatStatOperationV1, CombatStatPostRoundEffectV1, CombatStatPredicateV1,
    CombatStatSourcePlanV1, RoundScaleV1, HAND_SIZE,
};

/// Every source-copying Copy grammar this projection admits, as the exact printed text it
/// requires. `Reprisal:`, `Revenge:` and `Asymmetry:` reuse predicates the projection
/// already resolves before a round is prepared, so they gate the adoption itself rather
/// than needing new context. `Unison` (a draw-level clan-mate count), `Confidence`,
/// `Bet > N` and the clan-gated forms (`CLAN_GATED_COPY_GRAMMARS`) keep their own
/// grammars. Note the
/// site's own inconsistent punctuation: a copied Bonus loses the second colon under
/// `Reprisal`/`Revenge` but keeps it under `Asymmetry`.
const COPY_OPPONENT_SOURCE_GRAMMARS: [(&str, CopiedSourceKindV1, CombatStatPredicateV1); 10] = [
    (
        "Unison : Copy: Opp. Ability",
        CopiedSourceKindV1::Ability,
        CombatStatPredicateV1::OwnerHandUnison,
    ),
    (
        "Unison : Copy: Opp. Bonus",
        CopiedSourceKindV1::Bonus,
        CombatStatPredicateV1::OwnerHandUnison,
    ),
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

/// The Copy bodies printed under the owner-clan gate `[clan:A][clan:B] `, and the second
/// condition each carries beside it: Yayoi's `[clan:..] Copy: Opp. Ability` (`4132`) and
/// Hypnos' `[clan:..] Asy. : Copy: Opp. Ability` (`5073`). Revision 70. Both copy the
/// opposing Ability; no clan-gated Bonus Copy is printed.
const CLAN_GATED_COPY_GRAMMARS: [(&str, Option<ClanConjunctV1>); 2] = [
    ("Copy: Opp. Ability", None),
    (
        "Asy. : Copy: Opp. Ability",
        Some(ClanConjunctV1::SelectedHandSlotsDiffer),
    ),
];

/// True for every printed text the Copy compiler recognizes, so the catalog boundary can
/// route a source here without repeating the table.
pub(crate) fn is_copy_opponent_source_description(description: &str) -> bool {
    COPY_OPPONENT_SOURCE_GRAMMARS
        .iter()
        .any(|(text, _, _)| *text == description)
        || bet_gated_copy_body(description).is_some()
        || split_clan_tags(description).is_some_and(|(_, body)| {
            CLAN_GATED_COPY_GRAMMARS
                .iter()
                .any(|(text, _)| *text == body)
        })
}

/// Split a printed `[clan:A][clan:B] X` into its clan ids, in printed order, and `X`: at
/// least one tag, each a plain decimal id, then exactly one space. Text alone - the
/// classifiers still require the record's own `clanRequirement` to rebuild the same tags.
pub(crate) fn split_clan_tags(description: &str) -> Option<(Vec<u32>, &str)> {
    let mut ids = Vec::new();
    let mut rest = description;
    while let Some(tail) = rest.strip_prefix("[clan:") {
        let (id, tail) = tail.split_once(']')?;
        if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        ids.push(id.parse().ok()?);
        rest = tail;
    }
    if ids.is_empty() {
        return None;
    }
    Some((ids, rest.strip_prefix(' ')?))
}

/// The owner-clan gate read from the record: its `clanRequirement` as a set, both other clan
/// lists empty, and the text after the exact `[clan:A][clan:B] ` prefix rebuilt from that
/// list - the rule revision 54 set for `classify_clan_gated`.
fn owner_clan_gate<'a>(
    input: &StructuredEffectV1,
    description: &'a str,
) -> Option<(ClanSetV1, &'a str)> {
    let ids = input.clan_requirement.as_slice();
    if ids.is_empty()
        || !input.opponent_clan_requirement.is_empty()
        || !input.previous_clan_requirement.is_empty()
    {
        return None;
    }
    let set = ClanSetV1::from_ids(ids)?;
    let tags: String = ids.iter().map(|id| format!("[clan:{id}]")).collect();
    Some((
        set,
        description.strip_prefix(tags.as_str())?.strip_prefix(' ')?,
    ))
}

/// The record with its owner-clan gate cleared, so the ungated grammar's own shape can judge
/// everything else about it. No existing shape is loosened: each clan-gated classifier asks
/// the plain one about this copy, and the plain classifiers still see the gate.
fn without_owner_clan_gate(input: &StructuredEffectV1) -> StructuredEffectV1 {
    let mut plain = input.clone();
    plain.clan_requirement = ClanIdsV1::default();
    plain
}

/// `[clan:A][clan:B] Copy: Opp. Ability` and its `Asy. :` form: the adoption happens only
/// when the copier's own effective clan is listed, and for `Asy. :` also only when the two
/// selected cards sit in different slots. The record must be the ungated grammar's once the
/// gate is cleared. Revision 21's totality rule applies as to every Copy.
fn classify_clan_gated_copy(
    definition: &EffectDefinitionV1,
) -> Option<(CopiedSourceKindV1, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    let (set, body) = owner_clan_gate(input, definition.description())?;
    let (_, conjunct) = CLAN_GATED_COPY_GRAMMARS
        .iter()
        .find(|(text, _)| *text == body)?;
    let (shape_predicate, predicate) = match conjunct {
        None => (
            CombatStatPredicateV1::Always,
            CombatStatPredicateV1::OwnerClanIn(set),
        ),
        Some(conjunct) => (
            conjunct.predicate(),
            CombatStatPredicateV1::OwnerClanInAnd(set, *conjunct),
        ),
    };
    copy_opponent_source_shape_matches(
        &without_owner_clan_gate(input),
        SpecialActionV1::CopyAbility,
        shape_predicate,
        false,
    )
    .then_some((CopiedSourceKindV1::Ability, predicate))
}

/// `Bet > N Pillz: Copy: Opp. Ability` (and its Bonus twin): the unconditional Copy text
/// under a Bet prefix. Returns the unconditional body; the threshold is read from the
/// structured record, which must print exactly this prefix.
fn bet_gated_copy_body(description: &str) -> Option<&str> {
    let (threshold, body) = description.strip_prefix("Bet > ")?.split_once(" Pillz: ")?;
    (threshold.parse::<u8>().is_ok() && matches!(body, "Copy: Opp. Ability" | "Copy: Opp. Bonus"))
        .then_some(body)
}
use crate::effect_registry::{
    AffectedSideV1, AttributeActionV1, AttributeAffectedV1, BetPillzLinkV1, ClanIdsV1,
    CombatStatV1, CompiledEffectV1, CurrentRoundRequirementV1, EffectDefinitionV1,
    IndexRequirementV1, MagnitudeMultiplierV1, PositionRequirementV1, PreviousRoundRequirementV1,
    SpecialActionV1, StatOperationV1, StructuredEffectV1, SupportedEffectV1,
};

pub(crate) const COMBAT_STAT_COMPILER_POLICY_SEMANTIC_REVISION_V1: u16 = 72;

/// Recognize the admitted Copy grammars. Like generic Victory Life these are admitted by
/// exact description and structured shape rather than a fixed id list, because the registry
/// carries many structurally identical Copy definitions. The condition, when there is one,
/// gates whether the opposing source is adopted at all; the adopted plan then keeps its own
/// predicate, so a copied Confidence effect still has to satisfy the copier's own history.
pub(crate) fn classify_copy_opponent_source(
    definition: &EffectDefinitionV1,
) -> Option<(CopiedSourceKindV1, CombatStatPredicateV1)> {
    let description = definition.description();
    let input = definition.structured_input();
    if !input.clan_requirement.is_empty() {
        return classify_clan_gated_copy(definition);
    }
    // A Bet-gated Copy is the unconditional row with the gate as its predicate: the adoption
    // itself happens only when the owner's `pillzUsed` clears the threshold.
    let (text, bet) = match bet_gated_copy_body(description) {
        Some(body) => {
            let (predicate, prefix) = bet_gate(input)?;
            if !matches!(predicate, CombatStatPredicateV1::OwnerPillzUsedAbove(_))
                || description.strip_prefix(prefix.as_str()) != Some(body)
            {
                return None;
            }
            (body, Some(predicate))
        }
        None => (description, None),
    };
    let (_, copied, predicate) = COPY_OPPONENT_SOURCE_GRAMMARS
        .iter()
        .find(|(row, _, _)| *row == text)?;
    if bet.is_some() && *predicate != CombatStatPredicateV1::Always {
        return None;
    }
    let action = match copied {
        CopiedSourceKindV1::Ability => SpecialActionV1::CopyAbility,
        CopiedSourceKindV1::Bonus => SpecialActionV1::CopyBonus,
    };
    copy_opponent_source_shape_matches(input, action, *predicate, bet.is_some())
        .then_some((*copied, bet.unwrap_or(*predicate)))
}

fn copy_opponent_source_shape_matches(
    input: &StructuredEffectV1,
    action: SpecialActionV1,
    predicate: CombatStatPredicateV1,
    bet_gated: bool,
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
        CombatStatPredicateV1::OwnerHandUnison => (
            PositionRequirementV1::Both,
            PreviousRoundRequirementV1::Any,
            IndexRequirementV1::Any,
        ),
        CombatStatPredicateV1::OwnerMovesFirst
        | CombatStatPredicateV1::OwnerWonPreviousRound
        | CombatStatPredicateV1::SelectedHandSlotsMatch
        | CombatStatPredicateV1::MatchIsNight
        | CombatStatPredicateV1::MatchIsDay
        | CombatStatPredicateV1::OwnerAbilityStopped
        | CombatStatPredicateV1::OwnerClanIn(_)
        | CombatStatPredicateV1::OwnerPreviousCardClanIn(_)
        | CombatStatPredicateV1::OpponentHandHasClan(_)
        | CombatStatPredicateV1::OwnerPillzUsedAbove(_)
        | CombatStatPredicateV1::OwnerPillzUsedBelow(_)
        | CombatStatPredicateV1::OwnerWonPreviousRoundAtNight
        | CombatStatPredicateV1::OwnerClanInAnd(..) => return false,
    };
    let unison = predicate == CombatStatPredicateV1::OwnerHandUnison;
    let bet_fields_match = if bet_gated {
        input.bet_pillz_link == BetPillzLinkV1::More && input.value_condition > 0
    } else {
        input.bet_pillz_link == BetPillzLinkV1::No && input.value_condition == 0
    };
    input.value == 0
        && input.value_min == 0
        && input.value_max == 0
        && bet_fields_match
        && input.position_requirement == position
        && input.previous_round_requirement == previous_round
        && input.current_round_requirement == CurrentRoundRequirementV1::Any
        && input.index_requirement == index
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
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
        && input.is_clanmates_count_linked == unison
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

/// Recognize `Stop Opp. Ability` and `Stop Opp. Bonus` under the condition prefixes whose
/// predicate the projection already resolves: `Courage:`, `Confidence:`, `Revenge:`,
/// `Asymmetry:`, `Symmetry:`, `Night:` and, since revision 68, `Unison :` - the owner's whole
/// hand sharing the selected card's effective clan, which the structured record marks with
/// its clan-mates flag rather than a condition field. Every one of those predicates is
/// decided before the Stop graph, so a Stop whose condition fails is simply not live there -
/// which is the reference's `Events.executeCancels` order and already how `active_effect`
/// and `source_liveness` treat Reprisal. Reprisal keeps its identity lock and is not admitted
/// here: nothing below maps the defender position.
///
/// The structured record carries exactly one condition field and the printed prefix must
/// name it; `Night:` carries none and is read from the text, like the Night numerics. Card
/// abilities only. Returns the Stop and its predicate.
pub(crate) fn classify_conditional_stop(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    const WON_PREVIOUS: &[(PreviousRoundRequirementV1, IndexRequirementV1)] =
        &[(PreviousRoundRequirementV1::Win, IndexRequirementV1::Any)];
    const LOST_PREVIOUS: &[(PreviousRoundRequirementV1, IndexRequirementV1)] =
        &[(PreviousRoundRequirementV1::Lose, IndexRequirementV1::Any)];
    const SLOTS_DIFFER: &[(PreviousRoundRequirementV1, IndexRequirementV1)] = &[(
        PreviousRoundRequirementV1::Any,
        IndexRequirementV1::Asymmetry,
    )];
    const SLOTS_MATCH: &[(PreviousRoundRequirementV1, IndexRequirementV1)] = &[(
        PreviousRoundRequirementV1::Any,
        IndexRequirementV1::Symmetry,
    )];
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let input = definition.structured_input();
    let (effect, target) = match input.special_action {
        SpecialActionV1::StopAbility => (SupportedEffectV1::StopOpponentAbility, "Ability"),
        SpecialActionV1::StopBonus => (SupportedEffectV1::StopOpponentBonus, "Bonus"),
        _ => return None,
    };
    let description = definition.description();
    // Revision 70: Kupanda's `[clan:..] Asymm.: Stop Opp. Ability` (`4999`) - the owner-clan
    // gate and the differing hand slots together. Its record is the `Asymmetry:` Stop's once
    // the gate is cleared, and `Asymm.:` is the spelling it prints.
    if !input.clan_requirement.is_empty() {
        let (set, body) = owner_clan_gate(input, description)?;
        let shape = PostRoundShapeV1 {
            value: ShapeFieldV1::Exact(0),
            conditions: SLOTS_DIFFER,
            current_round: CurrentRoundRequirementV1::Any,
            attribute: AttributeAffectedV1::None,
            action: AttributeActionV1::None,
            special: input.special_action,
            ..POST_ROUND_SHAPE
        };
        return (body == format!("Asymm.: Stop Opp. {target}")
            && shape_matches(&without_owner_clan_gate(input), shape))
        .then_some((
            effect,
            CombatStatPredicateV1::OwnerClanInAnd(set, ClanConjunctV1::SelectedHandSlotsDiffer),
        ));
    }
    let (predicate, prefix, position, conditions) = if description.starts_with("Night: ") {
        (
            CombatStatPredicateV1::MatchIsNight,
            "Night: ",
            PositionRequirementV1::Both,
            UNCONDITIONAL,
        )
    } else if input.is_clanmates_count_linked {
        (
            CombatStatPredicateV1::OwnerHandUnison,
            "Unison : ",
            PositionRequirementV1::Both,
            UNCONDITIONAL,
        )
    } else {
        match (
            input.position_requirement,
            input.previous_round_requirement,
            input.index_requirement,
        ) {
            (
                PositionRequirementV1::Attacker,
                PreviousRoundRequirementV1::Any,
                IndexRequirementV1::Any,
            ) => (
                CombatStatPredicateV1::OwnerMovesFirst,
                "Courage: ",
                PositionRequirementV1::Attacker,
                UNCONDITIONAL,
            ),
            (
                PositionRequirementV1::Both,
                PreviousRoundRequirementV1::Win,
                IndexRequirementV1::Any,
            ) => (
                CombatStatPredicateV1::OwnerWonPreviousRound,
                "Confidence: ",
                PositionRequirementV1::Both,
                WON_PREVIOUS,
            ),
            (
                PositionRequirementV1::Both,
                PreviousRoundRequirementV1::Lose,
                IndexRequirementV1::Any,
            ) => (
                CombatStatPredicateV1::OwnerLostPreviousRound,
                "Revenge: ",
                PositionRequirementV1::Both,
                LOST_PREVIOUS,
            ),
            (
                PositionRequirementV1::Both,
                PreviousRoundRequirementV1::Any,
                IndexRequirementV1::Asymmetry,
            ) => (
                CombatStatPredicateV1::SelectedHandSlotsDiffer,
                "Asymmetry: ",
                PositionRequirementV1::Both,
                SLOTS_DIFFER,
            ),
            (
                PositionRequirementV1::Both,
                PreviousRoundRequirementV1::Any,
                IndexRequirementV1::Symmetry,
            ) => (
                CombatStatPredicateV1::SelectedHandSlotsMatch,
                "Symmetry: ",
                PositionRequirementV1::Both,
                SLOTS_MATCH,
            ),
            _ => return None,
        }
    };
    let shape = PostRoundShapeV1 {
        value: ShapeFieldV1::Exact(0),
        position,
        conditions,
        current_round: CurrentRoundRequirementV1::Any,
        attribute: AttributeAffectedV1::None,
        action: AttributeActionV1::None,
        special: input.special_action,
        clanmates_count: predicate == CombatStatPredicateV1::OwnerHandUnison,
        ..POST_ROUND_SHAPE
    };
    (shape_matches(input, shape) && description == format!("{prefix}Stop Opp. {target}"))
        .then_some((effect, predicate))
}

/// The predicates a conditional Stop plan may carry. `OwnerMovesSecond` is deliberately
/// absent: the Reprisal Stop stays identity-locked to its two reviewed definitions. The
/// compound clan gate is admitted only with the differing hand slots it is printed with.
pub(crate) fn conditional_stop_predicate_admitted(predicate: CombatStatPredicateV1) -> bool {
    matches!(
        predicate,
        CombatStatPredicateV1::OwnerClanInAnd(_, ClanConjunctV1::SelectedHandSlotsDiffer)
            | CombatStatPredicateV1::OwnerMovesFirst
            | CombatStatPredicateV1::OwnerWonPreviousRound
            | CombatStatPredicateV1::OwnerLostPreviousRound
            | CombatStatPredicateV1::SelectedHandSlotsMatch
            | CombatStatPredicateV1::SelectedHandSlotsDiffer
            | CombatStatPredicateV1::MatchIsNight
            | CombatStatPredicateV1::OwnerHandUnison
            | CombatStatPredicateV1::OwnerClanIn(_)
            | CombatStatPredicateV1::OwnerPreviousCardClanIn(_)
            | CombatStatPredicateV1::OwnerPillzUsedAbove(_)
    )
}

/// Recognize only the literal, immediate end-of-round Victory Life grammar and the three
/// reviewed forms that put an already-resolved predicate on it.  Unlike the
/// identity-locked Pillz slices below, this is deliberately generic: any registry
/// definition with the complete reviewed structured shape may supply its positive fixed
/// magnitude, whether it came from an Ability or a Bonus.  The prefixed forms are card
/// abilities only - no clan bonus prints them - and each carries its condition in the one
/// field the plain grammar requires neutral, so the two can never be confused. `Courage:`
/// is the first move, which Anita's conversion and the Courage opponent-Life identities
/// already carry on a post-round plan.
/// Returns `(life, predicate)`.
pub(crate) fn classify_victory_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, CombatStatPredicateV1)> {
    if !has_victory_life_shape(definition) {
        return None;
    }
    let input = definition.structured_input();
    let (predicate, text) = match (
        input.position_requirement,
        input.previous_round_requirement,
        input.index_requirement,
    ) {
        (PositionRequirementV1::Both, PreviousRoundRequirementV1::Any, IndexRequirementV1::Any) => {
            (
                CombatStatPredicateV1::Always,
                format!("+{} Life", input.value),
            )
        }
        (PositionRequirementV1::Both, PreviousRoundRequirementV1::Win, IndexRequirementV1::Any) => {
            (
                CombatStatPredicateV1::OwnerWonPreviousRound,
                format!("Confidence : +{} Life", input.value),
            )
        }
        (
            PositionRequirementV1::Both,
            PreviousRoundRequirementV1::Any,
            IndexRequirementV1::Asymmetry,
        ) => (
            CombatStatPredicateV1::SelectedHandSlotsDiffer,
            format!("Asymmetry: +{} Life", input.value),
        ),
        (
            PositionRequirementV1::Attacker,
            PreviousRoundRequirementV1::Any,
            IndexRequirementV1::Any,
        ) => (
            CombatStatPredicateV1::OwnerMovesFirst,
            format!("Courage: +{} Life", input.value),
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

/// Recognize the plain `+N Pillz` Victory grammar and its `Confidence:` and `Courage:`
/// forms: the winner's own Pillz rise by the printed amount at the end of the round,
/// unconditionally, only after a round its own side won, or only when its card moved first.
/// Like Victory Life it is admitted by exact text and complete structured shape over every
/// same-text registry record, but card abilities only - no clan bonus prints any of them, so
/// a Bonus slot carrying the text is a hazard, not a generic source. Each prefixed form
/// carries its condition in one field the plain grammar requires neutral, so they can never
/// be confused; `Confidence:` prints tight, unlike Victory Life's spaced `Confidence :`, and
/// the exact-text check is what keeps those two apart. Returns `(pillz, predicate)`.
pub(crate) fn classify_victory_pillz(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, CombatStatPredicateV1)> {
    if source_kind != CombatStatEffectSourceV1::Ability || !has_victory_pillz_shape(definition) {
        return None;
    }
    let input = definition.structured_input();
    let (predicate, text) = match (input.position_requirement, input.previous_round_requirement) {
        (PositionRequirementV1::Both, PreviousRoundRequirementV1::Any) => (
            CombatStatPredicateV1::Always,
            format!("+{} Pillz", input.value),
        ),
        (PositionRequirementV1::Both, PreviousRoundRequirementV1::Win) => (
            CombatStatPredicateV1::OwnerWonPreviousRound,
            format!("Confidence: +{} Pillz", input.value),
        ),
        (PositionRequirementV1::Attacker, PreviousRoundRequirementV1::Any) => (
            CombatStatPredicateV1::OwnerMovesFirst,
            format!("Courage: +{} Pillz", input.value),
        ),
        _ => return None,
    };
    (definition.description() == text).then_some((input.value, predicate))
}

/// Structural half of the Victory Pillz boundary, so replay preparation can reject a
/// complete shape under malformed text instead of silently disabling it.
pub(crate) fn has_victory_pillz_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0 && victory_pillz_shape_matches(input)
}

/// Recognize the capped Victory Pillz grammar `+N Pillz Max. M` (Mandrak Cr's `1139`) and
/// its `Night:` form (Nox Ld's night ability `4747`): a living winner's own Pillz rise by N,
/// never past M, and an owner already at or above M gains nothing - the arithmetic the
/// admitted `Brawl: +N Pillz, Max. M` already binds to. It is the plain grammar's record
/// with `valueMax` read as the cap, which every other Victory Pillz grammar requires to be
/// zero, so neither can pass for the other. `Night:` is a match constant with no structured
/// trace and is read from the text, as for the Night numerics. Exact text rebuilt from the
/// record, card abilities only. Returns `(pillz, maximum, predicate)`.
pub(crate) fn classify_victory_pillz_max(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16, CombatStatPredicateV1)> {
    if source_kind != CombatStatEffectSourceV1::Ability || !has_victory_pillz_max_shape(definition)
    {
        return None;
    }
    let input = definition.structured_input();
    let body = format!("+{} Pillz Max. {}", input.value, input.value_max);
    let description = definition.description();
    let predicate = if description == body {
        CombatStatPredicateV1::Always
    } else if description.strip_prefix("Night: ") == Some(body.as_str()) {
        CombatStatPredicateV1::MatchIsNight
    } else {
        return None;
    };
    Some((input.value, input.value_max, predicate))
}

/// Structural half of the capped Victory Pillz boundary: the unconditional Victory Pillz
/// record with a positive cap.
pub(crate) fn has_victory_pillz_max_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && input.value_max > 0
        && shape_matches(
            input,
            PostRoundShapeV1 {
                value_max: ShapeFieldV1::Read,
                attribute: AttributeAffectedV1::Pillz,
                ..POST_ROUND_SHAPE
            },
        )
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

/// Recognize the opposing compound `-N Opp. Pillz And Life, Min M`: the round winner takes
/// N from the opposing player's Pillz and N from their Life, each read after both bets and
/// the round's damage and each clamped at M on its own - a resource already at or below M
/// is left alone (963847/2: Life 2 clamps to 1, Pillz 0 stays 0). Exact text and complete
/// structured shape, card abilities only. Returns `(amount, minimum)`.
pub(crate) fn classify_victory_opponent_pillz_and_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16)> {
    let input = definition.structured_input();
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_victory_opponent_pillz_and_life_shape(definition)
        && definition.description()
            == format!(
                "-{} Opp. Pillz And Life, Min {}",
                input.value, input.value_min
            ))
    .then_some((input.value, input.value_min))
}

/// Structural half of the opposing compound boundary.
pub(crate) fn has_victory_opponent_pillz_and_life_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && shape_matches(
            input,
            PostRoundShapeV1 {
                value_min: ShapeFieldV1::Read,
                side: AffectedSideV1::Opponent,
                attribute: AttributeAffectedV1::LifeAndPillz,
                action: AttributeActionV1::Decrease,
                ..POST_ROUND_SHAPE
            },
        )
}

/// Recognize `Victory Or Defeat : +N Pillz` for N of two or more: the owner gains N Pillz
/// whatever the round's outcome. The one-Pillz text stays the reviewed identity set, because
/// it recurs in unrelated rows; a larger amount has only its own records. Exact text and
/// complete shape, card abilities only. Returns the Pillz.
pub(crate) fn classify_victory_or_defeat_pillz_amount(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<u16> {
    let pillz = definition.structured_input().value;
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_victory_or_defeat_pillz_amount_shape(definition)
        && definition.description() == format!("Victory Or Defeat : +{pillz} Pillz"))
    .then_some(pillz)
}

pub(crate) fn has_victory_or_defeat_pillz_amount_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value >= 2
        && shape_matches(
            input,
            PostRoundShapeV1 {
                current_round: CurrentRoundRequirementV1::Any,
                attribute: AttributeAffectedV1::Pillz,
                ..POST_ROUND_SHAPE
            },
        )
}

/// Recognize `Victory Or Defeat: +N Life Per Damage`: a living owner gains N Life per point
/// of its own card's final Damage, Fury included, won or lost (1066589/0 pays a loss).
/// It reads the `valueMin` 1 every Victory Or Defeat Life gain carries. Card abilities only.
/// Returns N.
pub(crate) fn classify_victory_or_defeat_life_per_damage(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<u16> {
    let value = definition.structured_input().value;
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_victory_or_defeat_life_per_damage_shape(definition)
        && definition.description() == format!("Victory Or Defeat: +{value} Life Per Damage"))
    .then_some(value)
}

pub(crate) fn has_victory_or_defeat_life_per_damage_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && shape_matches(
            input,
            PostRoundShapeV1 {
                value_min: ShapeFieldV1::Exact(1),
                current_round: CurrentRoundRequirementV1::Any,
                special: SpecialActionV1::ConvertDamageToLife,
                ..POST_ROUND_SHAPE
            },
        )
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

/// Recognize `Backlash: - N Life Min M` for M of one or more: the Min-clamped Life reduction
/// turned on its own owner, paid when the owner wins the round. `Backlash` is not a trigger
/// of its own: the record carries the outcome in `currentRoundRequirement` and the target in
/// `sideAffected: player`, which no other admitted reduction names. The server pays it on a
/// win only (945871/1 and 946913/0 pay, 945724/0 and 1414458/2 are losses and do not) and
/// names the owner as the payer, a knocked-out opponent notwithstanding (1131144/2). A `Min
/// 0` record could knock its own owner out, which no round shows, so it is refused here and
/// replay rejects it when selected. Exact text rebuilt from the record's own numbers, card
/// abilities only: no clan bonus prints one. The `Defeat: Backlash:` form (`2417`), the
/// Pillz form (`1401`) and the own-Life Poison (`4124`) are other structures and stay
/// closed. Returns `(life, minimum)`.
pub(crate) fn classify_backlash_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16)> {
    let input = definition.structured_input();
    (source_kind == CombatStatEffectSourceV1::Ability
        && input.value_min > 0
        && has_backlash_life_shape(definition)
        && definition.description()
            == format!("Backlash: - {} Life Min {}", input.value, input.value_min))
    .then_some((input.value, input.value_min))
}

/// Structural half of the Backlash Life boundary, `Min 0` included, so replay preparation
/// rejects the complete shape under malformed text, and the refused `Min 0` records, instead
/// of disabling them.
pub(crate) fn has_backlash_life_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && shape_matches(
            input,
            PostRoundShapeV1 {
                value_min: ShapeFieldV1::Read,
                action: AttributeActionV1::Decrease,
                ..POST_ROUND_SHAPE
            },
        )
}

/// Recognize the capped `Defeat: +N Life, Max. M`: a loser the round has not knocked out
/// gains N Life, never past M, and an owner already at or above M gains nothing - the Defeat
/// Life grammar with Heal's cap. The server binds the cap (1131114/0: Tiwi Ld's owner goes 14
/// - 5 = 9 and is paid 2, not 3, stopping at 11) and reaches it exactly (1130977/3: Lola Cr's
/// 12 - 5 + 3 = 10), and a win pays nothing (925204/1). `valueMin` is 1, as on every Defeat
/// Life record, and `valueMax` must exceed the gain; the uncapped grammar requires `valueMax`
/// 0, so neither can pass for the other. Exact text, card abilities only. Returns `(life,
/// maximum)`.
pub(crate) fn classify_defeat_capped_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16)> {
    let input = definition.structured_input();
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_defeat_capped_life_shape(definition)
        && definition.description()
            == format!("Defeat: +{} Life, Max. {}", input.value, input.value_max))
    .then_some((input.value, input.value_max))
}

/// Structural half of the capped Defeat Life boundary.
pub(crate) fn has_defeat_capped_life_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && input.value_max > input.value
        && shape_matches(
            input,
            PostRoundShapeV1 {
                value_min: ShapeFieldV1::Exact(1),
                value_max: ShapeFieldV1::Read,
                current_round: CurrentRoundRequirementV1::Lose,
                ..POST_ROUND_SHAPE
            },
        )
}

/// Recognize `Defeat: +N Opp. Pillz`: the owner lost the round, so the opposing player gains
/// N Pillz. It is the losing-side opposing Pillz reduction's channel with the opposite
/// action and no floor, and the one round that selects it pins both of its unusual halves:
/// 1130425/2 pays from an owner the round has just knocked out (Pr SenQ, 3 to 0) into a
/// player whose bet has emptied their pool (Sue, 3 - 3 = 0, then 1). Exact text, card
/// abilities only. Returns the Pillz.
pub(crate) fn classify_defeat_opponent_pillz_gain(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<u16> {
    let pillz = definition.structured_input().value;
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_defeat_opponent_pillz_gain_shape(definition)
        && definition.description() == format!("Defeat: +{pillz} Opp. Pillz"))
    .then_some(pillz)
}

/// Structural half of the Defeat opposing Pillz gift boundary.
pub(crate) fn has_defeat_opponent_pillz_gain_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && shape_matches(
            input,
            PostRoundShapeV1 {
                current_round: CurrentRoundRequirementV1::Lose,
                side: AffectedSideV1::Opponent,
                attribute: AttributeAffectedV1::Pillz,
                ..POST_ROUND_SHAPE
            },
        )
}

/// Recognize `Corrupt N Min. M` (Nega D Ld's `5286`, `Corrupt 2 Min. 5`): whether its card
/// wins or loses, the owner's own Life falls by N at the end of the round, never below M, and
/// an owner already at or below M is left alone. It is Xantiax's own half on its own: the
/// record asks for no outcome and no condition, like Xantiax's, but names `sideAffected:
/// player` where Xantiax names `both`, so neither can pass for the other, and the Victory
/// Backlash names `win`. The server pins the floor twice (1065308/2 and 1066210/2: Nega wins
/// a knockout round and its owner goes 6 to 5, a post quantity of 1); the unclamped amount
/// and the losing side rest on Xantiax's own-side arm. A `Min 0` record could knock its own
/// owner out, which no round shows, so it is refused here and replay rejects it when
/// selected. Exact text rebuilt from the record's own numbers, card abilities only. Returns
/// `(life, minimum)`.
pub(crate) fn classify_corrupt_own_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16)> {
    let input = definition.structured_input();
    (source_kind == CombatStatEffectSourceV1::Ability
        && input.value_min > 0
        && has_corrupt_own_life_shape(definition)
        && definition.description() == format!("Corrupt {} Min. {}", input.value, input.value_min))
    .then_some((input.value, input.value_min))
}

/// Structural half of the Corrupt boundary, `Min 0` included, so replay preparation rejects
/// the complete shape under malformed text, and a refused `Min 0` record, instead of
/// disabling it.
pub(crate) fn has_corrupt_own_life_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && shape_matches(
            input,
            PostRoundShapeV1 {
                value_min: ShapeFieldV1::Read,
                current_round: CurrentRoundRequirementV1::Any,
                action: AttributeActionV1::Decrease,
                ..POST_ROUND_SHAPE
            },
        )
}

/// Recognize the `Revenge:` form of `+N Attack Per Opp. Power` (Betul's `1719`, printed with
/// a space after the sign: `Revenge: + 2 Attack Per Opp. Power`). The registry compiles the
/// plain form (`1785`, `4661`) with the `OpponentPower` magnitude and refuses this one only
/// because it is conditional; `numeric_effect` refuses any special action, so the
/// previous-round numeric classifier never reaches it. The record is the plain one with
/// `previousRoundRequirement: lose` and nothing else, and the plan carries Revenge's
/// already-resolved predicate. 1066337/3 pays (Betul lost round 2: 8 x 10 + 2 x 7 = 94) and
/// 926071/1 does not (Betul won round 0: 24 less Hive's Equalizer 12 = 12). Card abilities
/// only.
fn classify_revenge_attack_per_opponent_power(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    (source_kind == CombatStatEffectSourceV1::Ability
        && input.previous_round_requirement == PreviousRoundRequirementV1::Lose
        && neutral_except_previous_round(input)
        && input.special_action == SpecialActionV1::ConvertOpponentPowerToAttack
        && input.attribute_affected == AttributeAffectedV1::Attack
        && input.attribute_action == AttributeActionV1::Increase
        && input.side_affected == AffectedSideV1::Player
        && !input.is_support
        && input.value > 0
        && input.value_min == 0
        && input.value_max == 0
        && definition.description() == format!("Revenge: + {} Attack Per Opp. Power", input.value))
    .then_some((
        SupportedEffectV1::ModifyCombatStat {
            side: AffectedSideV1::Player,
            stat: CombatStatV1::Attack,
            operation: StatOperationV1::Increase,
            value: input.value,
            minimum: None,
            maximum: None,
            multiplier: MagnitudeMultiplierV1::OpponentPower,
        },
        CombatStatPredicateV1::OwnerLostPreviousRound,
    ))
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

/// `+N Life Per Opp. Damage`: the winner's Life rises by N per point of the opposing
/// card's final resolved Damage. Unconditional, uncapped, card abilities only.
pub(crate) fn classify_victory_life_per_opponent_damage(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<u16> {
    if source_kind != CombatStatEffectSourceV1::Ability
        || !has_victory_life_per_opponent_damage_shape(definition)
    {
        return None;
    }
    let value = definition.structured_input().value;
    (definition.description() == format!("+{value} Life Per Opp. Damage")).then_some(value)
}

pub(crate) fn has_victory_life_per_opponent_damage_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && shape_matches(
            input,
            PostRoundShapeV1 {
                special: SpecialActionV1::ConvertOpponentDamageToLife,
                ..POST_ROUND_SHAPE
            },
        )
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
    input.value > 0 && defeat_life_shape_matches(input, 1, false)
}

/// `Unison: Defeat: +N Life`: ordinary Defeat Life under the Unison gate. The record is the
/// Defeat Life record with the clan-mates link set and nothing else changed, `valueMin` 1
/// included; the site prints this prefix without the space `Unison :` has elsewhere. Card
/// abilities only. Returns the Life.
pub(crate) fn classify_unison_defeat_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<u16> {
    let input = definition.structured_input();
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_unison_defeat_life_shape(definition)
        && definition.description() == format!("Unison: Defeat: +{} Life", input.value))
    .then_some(input.value)
}

pub(crate) fn has_unison_defeat_life_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0 && defeat_life_shape_matches(input, 1, true)
}

/// `Unison : +N Pillz And Life`: a living winner whose hand is one effective clan gains N
/// Pillz and then N Life. Korakine's record reads `valueMin` 2 beside its value of 2, and the
/// shape reads it exactly, so neither the Komboka compound (`valueMin` 0, no link) nor Kubra's
/// Defeat compound (`valueMin` 1, `lose`) can pass for it. Card abilities only. Returns the
/// amount.
pub(crate) fn classify_unison_pillz_and_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<u16> {
    let input = definition.structured_input();
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_unison_pillz_and_life_shape(definition)
        && definition.description() == format!("Unison : +{} Pillz And Life", input.value))
    .then_some(input.value)
}

pub(crate) fn has_unison_pillz_and_life_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && input.value_min == input.value
        && shape_matches(
            input,
            PostRoundShapeV1 {
                value_min: ShapeFieldV1::Read,
                attribute: AttributeAffectedV1::LifeAndPillz,
                clanmates_count: true,
                ..POST_ROUND_SHAPE
            },
        )
}

/// Recognize `Defeat: +N Pillz`: a loser the round has not knocked out gains N Pillz. The
/// Defeat Life grammar on the other resource, by exact text and complete shape, card
/// abilities only. `valueMin` must be zero, which keeps Argos' capped form and the Recover
/// records out.
pub(crate) fn classify_defeat_pillz(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<u16> {
    let pillz = definition.structured_input().value;
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_defeat_pillz_shape(definition)
        && definition.description() == format!("Defeat: +{pillz} Pillz"))
    .then_some(pillz)
}

pub(crate) fn has_defeat_pillz_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && shape_matches(
            input,
            PostRoundShapeV1 {
                current_round: CurrentRoundRequirementV1::Lose,
                attribute: AttributeAffectedV1::Pillz,
                ..POST_ROUND_SHAPE
            },
        )
}

/// Recognize `Defeat: +N Pillz And Life`, the compound on the same channel: a living loser
/// gains N Pillz and N Life. Kubra's record carries `valueMin` 1, like Defeat Life's, and
/// the shape reads it exactly so the Komboka Victory compound can never be confused with it.
pub(crate) fn classify_defeat_pillz_and_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<u16> {
    let amount = definition.structured_input().value;
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_defeat_pillz_and_life_shape(definition)
        && definition.description() == format!("Defeat: +{amount} Pillz And Life"))
    .then_some(amount)
}

pub(crate) fn has_defeat_pillz_and_life_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && shape_matches(
            input,
            PostRoundShapeV1 {
                value_min: ShapeFieldV1::Exact(1),
                current_round: CurrentRoundRequirementV1::Lose,
                attribute: AttributeAffectedV1::LifeAndPillz,
                ..POST_ROUND_SHAPE
            },
        )
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
    input.value > 0 && defeat_life_shape_matches(input, 0, false)
}

/// The Recover grammar's typed reading: which outcome pays, the printed ratio, and the
/// predicate its prefix names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RecoverPillzV1 {
    pub(crate) on_victory: bool,
    pub(crate) numerator: u16,
    pub(crate) denominator: u16,
    pub(crate) predicate: CombatStatPredicateV1,
}

impl RecoverPillzV1 {
    pub(crate) const fn effects(self) -> (CombatStatPostRoundEffectV1, CombatStatEffectV1) {
        let (numerator, denominator) = (self.numerator, self.denominator);
        if self.on_victory {
            (
                CombatStatPostRoundEffectV1::RecoverPaidPillzOnVictory {
                    numerator,
                    denominator,
                },
                CombatStatEffectV1::RecoverPaidPillzOnVictory {
                    numerator,
                    denominator,
                },
            )
        } else {
            (
                CombatStatPostRoundEffectV1::RecoverPaidPillzOnDefeat {
                    numerator,
                    denominator,
                },
                CombatStatEffectV1::RecoverPaidPillzOnDefeat {
                    numerator,
                    denominator,
                },
            )
        }
    }
}

/// `Recover N Pillz Out Of M` under its three printed prefixes: `Defeat: ` pays the loser,
/// no prefix pays the winner, and `Unison : ` pays the winner when the owner's hand is one
/// effective clan. Admitted by exact text and complete structured shape (the registry's
/// `value`/`valueMin` are the ratio), card abilities only except the Defeat form, which the
/// Vortex clan prints as its bonus. The ratio must be a proper fraction; nothing printed is
/// otherwise, and a zero denominator would be a division by zero in the engine arm.
pub(crate) fn classify_recover_pillz(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<RecoverPillzV1> {
    let input = definition.structured_input();
    let (numerator, denominator) = (input.value, input.value_min);
    if numerator == 0 || numerator >= denominator {
        return None;
    }
    let (prefix, on_victory, predicate) = match input.current_round_requirement {
        CurrentRoundRequirementV1::Lose if !input.is_clanmates_count_linked => {
            ("Defeat: ", false, CombatStatPredicateV1::Always)
        }
        CurrentRoundRequirementV1::Win if !input.is_clanmates_count_linked => {
            ("", true, CombatStatPredicateV1::Always)
        }
        CurrentRoundRequirementV1::Win => {
            ("Unison : ", true, CombatStatPredicateV1::OwnerHandUnison)
        }
        _ => return None,
    };
    let source_admitted = source_kind == CombatStatEffectSourceV1::Ability || !on_victory;
    (source_admitted
        && has_recover_pillz_shape(definition)
        && definition.description()
            == format!("{prefix}Recover {numerator} Pillz Out Of {denominator}"))
    .then_some(RecoverPillzV1 {
        on_victory,
        numerator,
        denominator,
        predicate,
    })
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
/// is prepared. Growth `1730` deliberately stays out: it is a round-scaled magnitude rather
/// than a predicate, which a post-round plan cannot carry today. The Courage members are in
/// as of semantic revision 39; they were previously grouped with `1730` under one candidate
/// line, which is why the family read as one draw rather than the two it measures.
const VICTORY_OPPONENT_LIFE_IDENTITIES: [(
    u32,
    &str,
    CombatStatEffectSourceV1,
    u16,
    u16,
    CombatStatPredicateV1,
); 8] = [
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
    // Dragomer Cr level 3. Its levels 4 and 5 print `3001` and `2302`, and neither has a
    // registry definition at all, so both stay fail-closed with no special handling -
    // the Doela Noel level-one `4843` case again.
    (
        3314,
        "Courage: - 1 Opp. Life Min 0",
        CombatStatEffectSourceV1::Ability,
        1,
        0,
        CombatStatPredicateV1::OwnerMovesFirst,
    ),
    // Ligea prints the reduction at all three of her levels. Levels 1 and 2 carry
    // byte-identical records under two ids, exactly as Diabolus does above, so each is
    // admitted on its own evidence rather than one aliasing the other.
    (
        4531,
        "Courage: - 3 Opp. Life Min 1",
        CombatStatEffectSourceV1::Ability,
        3,
        1,
        CombatStatPredicateV1::OwnerMovesFirst,
    ),
    (
        4532,
        "Courage: - 3 Opp. Life Min 1",
        CombatStatEffectSourceV1::Ability,
        3,
        1,
        CombatStatPredicateV1::OwnerMovesFirst,
    ),
    (
        4533,
        "Courage: - 3 Opp. Life Min 0",
        CombatStatEffectSourceV1::Ability,
        3,
        0,
        CombatStatPredicateV1::OwnerMovesFirst,
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
/// Since revision 69 the grammar has a second printed form, `Night: -N Opp. Life Min M`
/// (Lyra's night ability `4750`), under the `MatchIsNight` match constant. `Night:` leaves no
/// structured trace, so the record is the unconditional one and the text alone names it -
/// exactly how the Night numerics and the Night Stops are read.
///
/// Everything else remains fail-closed here: the capped and compound neighbours, the
/// complete shape under other prefixed text, and the same-text catalog ids that have no
/// registry definition at all (Rakhan `978`, Milovan `498`, Fraser `1289`). The clan-gated
/// `5392` has its own classifier since revision 70 (`classify_clan_gated_post_round`).
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
    let body = format!("-{life} Opp. Life Min {minimum}");
    let description = definition.description();
    let predicate = if description == body {
        CombatStatPredicateV1::Always
    } else if description.strip_prefix("Night: ") == Some(body.as_str()) {
        CombatStatPredicateV1::MatchIsNight
    } else {
        return None;
    };
    (source_kind == CombatStatEffectSourceV1::Ability
        && life > 0
        && victory_opponent_life_shape_matches(input, life, minimum, predicate))
    .then_some((life, minimum, predicate))
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
        // `Night:` is a match constant with no structured trace, so its record is the
        // unconditional one and the printed prefix is the only thing that names it.
        CombatStatPredicateV1::Always | CombatStatPredicateV1::MatchIsNight => (
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
        // Courage carries its condition in the position field rather than the previous
        // round or the hand slot, which is the one structural slot this grammar had not
        // yet been shown. Everything else about the record is the reviewed Victory shape.
        CombatStatPredicateV1::OwnerMovesFirst => (
            PositionRequirementV1::Attacker,
            PreviousRoundRequirementV1::Any,
            IndexRequirementV1::Any,
        ),
        CombatStatPredicateV1::OwnerMovesSecond
        | CombatStatPredicateV1::OwnerLostPreviousRound
        | CombatStatPredicateV1::SelectedHandSlotsDiffer
        | CombatStatPredicateV1::MatchIsDay
        | CombatStatPredicateV1::OwnerHandUnison
        | CombatStatPredicateV1::OwnerAbilityStopped
        | CombatStatPredicateV1::OwnerClanIn(_)
        | CombatStatPredicateV1::OwnerPreviousCardClanIn(_)
        | CombatStatPredicateV1::OpponentHandHasClan(_)
        | CombatStatPredicateV1::OwnerPillzUsedAbove(_)
        | CombatStatPredicateV1::OwnerPillzUsedBelow(_)
        | CombatStatPredicateV1::OwnerWonPreviousRoundAtNight
        | CombatStatPredicateV1::OwnerClanInAnd(..) => return false,
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

/// Recognize `Killshot: +N Pillz And Life`: an owner whose final attack at least doubles the
/// opposing one gains N Pillz and N Life. It is the Komboka compound gain on the Killshot
/// trigger revision 38 admitted, by exact text and complete shape, card abilities only.
pub(crate) fn classify_killshot_pillz_and_life(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<u16> {
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let amount = definition.structured_input().value;
    (has_killshot_pillz_and_life_shape(definition)
        && definition.description() == format!("Killshot: +{amount} Pillz And Life"))
    .then_some(amount)
}

pub(crate) fn has_killshot_pillz_and_life_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && shape_matches(
            input,
            PostRoundShapeV1 {
                current_round: CurrentRoundRequirementV1::Sureshot,
                attribute: AttributeAffectedV1::LifeAndPillz,
                ..POST_ROUND_SHAPE
            },
        )
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

/// What one of the remaining Killshot grammars pays once the attack ratio holds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KillshotPostRoundEffectV1 {
    /// `Killshot: +N Pillz`.
    GainPillz { pillz: u16 },
    /// `Killshot: +N Life`, its capped `Killshot: +N Life Max. M` and its `Unison:` form.
    /// `maximum == 0` is the uncapped form.
    GainLife { life: u16, maximum: u16 },
    /// `Killshot: Toxin N, Min M`: the plain Toxin permanent, latched by the ratio.
    ToxinOpponentLife { life: u16, minimum: u16 },
}

impl KillshotPostRoundEffectV1 {
    /// The public and compact representations, which catalog and replay preparation must
    /// build identically.
    pub(crate) fn effects(self) -> (CombatStatPostRoundEffectV1, CombatStatEffectV1) {
        match self {
            Self::GainPillz { pillz } => (
                CombatStatPostRoundEffectV1::GainPillzOnKillshot { pillz },
                CombatStatEffectV1::GainPillzOnKillshot { pillz },
            ),
            Self::GainLife { life, maximum } => (
                CombatStatPostRoundEffectV1::GainLifeOnKillshot { life, maximum },
                CombatStatEffectV1::GainLifeOnKillshot { life, maximum },
            ),
            Self::ToxinOpponentLife { life, minimum } => (
                CombatStatPostRoundEffectV1::ToxinOpponentLifeOnKillshot { life, minimum },
                CombatStatEffectV1::ToxinOpponentLifeOnKillshot { life, minimum },
            ),
        }
    }
}

/// Recognize the Killshot grammars that compose pieces the projection already executes: the
/// two halves of the `Killshot: +N Pillz And Life` compound on their own (`Killshot: +N
/// Pillz`, `Killshot: +N Life`), the Life half under Heal's cap (`Killshot: +N Life Max. M`)
/// and under the `Unison:` hand gate (`Unison: Killshot: +N Life`, printed tight, unlike the
/// spaced `Unison :` of the numeric forms), and the plain immediate Toxin latched by the
/// ratio instead of by a win (`Killshot: Toxin N, Min M`). Each is the shape of the grammar
/// it composes with `sureshot` in the current-round field, and the Unison form additionally
/// carries the clan-mates link, which is the gate and not a magnitude. Exact text rebuilt
/// from the record's own numbers, card abilities only: no clan bonus prints a Killshot.
///
/// The opposing `Killshot: -N Opp. Pillz And Life, Min M` compound, a capped or gated Pillz
/// form, a capped Unison form and a Killshot on any other permanent have no shape here and
/// stay closed. Returns the effect and the one predicate the printed prefix names.
pub(crate) fn classify_killshot_post_round(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(KillshotPostRoundEffectV1, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    if source_kind != CombatStatEffectSourceV1::Ability || input.value == 0 {
        return None;
    }
    let (effect, predicate, text) = if killshot_own_pillz_shape_matches(input) {
        (
            KillshotPostRoundEffectV1::GainPillz { pillz: input.value },
            CombatStatPredicateV1::Always,
            format!("Killshot: +{} Pillz", input.value),
        )
    } else if killshot_own_life_shape_matches(input, false) {
        (
            KillshotPostRoundEffectV1::GainLife {
                life: input.value,
                maximum: input.value_max,
            },
            CombatStatPredicateV1::Always,
            if input.value_max == 0 {
                format!("Killshot: +{} Life", input.value)
            } else {
                format!("Killshot: +{} Life Max. {}", input.value, input.value_max)
            },
        )
    } else if killshot_own_life_shape_matches(input, true) && input.value_max == 0 {
        (
            KillshotPostRoundEffectV1::GainLife {
                life: input.value,
                maximum: 0,
            },
            CombatStatPredicateV1::OwnerHandUnison,
            format!("Unison: Killshot: +{} Life", input.value),
        )
    } else if killshot_toxin_shape_matches(input) {
        (
            KillshotPostRoundEffectV1::ToxinOpponentLife {
                life: input.value,
                minimum: input.value_min,
            },
            CombatStatPredicateV1::Always,
            format!("Killshot: Toxin {}, Min {}", input.value, input.value_min),
        )
    } else {
        return None;
    };
    (definition.description() == text).then_some((effect, predicate))
}

/// Structural half of the Killshot boundary above, so replay preparation can call a complete
/// shape under malformed text a hazard rather than silently disabling it.
pub(crate) fn has_killshot_post_round_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && (killshot_own_pillz_shape_matches(input)
            || killshot_own_life_shape_matches(input, false)
            || killshot_own_life_shape_matches(input, true)
            || killshot_toxin_shape_matches(input))
}

fn killshot_own_pillz_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            current_round: CurrentRoundRequirementV1::Sureshot,
            attribute: AttributeAffectedV1::Pillz,
            ..POST_ROUND_SHAPE
        },
    )
}

/// The own Life gain reads a `Max. M` cap, and `clanmates_count` is the `Unison:` gate.
fn killshot_own_life_shape_matches(input: &StructuredEffectV1, clanmates_count: bool) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value_max: ShapeFieldV1::Read,
            current_round: CurrentRoundRequirementV1::Sureshot,
            clanmates_count,
            ..POST_ROUND_SHAPE
        },
    )
}

/// The plain immediate Toxin structure, unconditional, on the `sureshot` channel.
fn killshot_toxin_shape_matches(input: &StructuredEffectV1) -> bool {
    permanent_opponent_life_shape_matches_on(input, true, CurrentRoundRequirementV1::Sureshot)
        && matches!(
            permanent_condition(input),
            Some((CombatStatPredicateV1::Always, _))
        )
}

/// Recognize `Xantiax: -N Life, Min. M`: the first admitted post-round grammar that names
/// no outcome and no beneficiary, and still the only one that reaches both players (Corrupt
/// is its own half). Both players lose N, neither below M, whatever the round did.
/// `Xantiax` is flavour on the printed text, not a condition - the structured record
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

/// The resource a `Victory Or Defeat : +N Players ...` gain writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BothPlayersGainV1 {
    Life,
    Pillz,
}

/// Recognize `Victory Or Defeat : +N Players Pillz`: whatever the round's outcome, both
/// players gain N, a knocked-out one included (1024592/2, 1024732/3). It is the Victory Or
/// Defeat own gain with `sideAffected: both`, the value only Xantiax had used, and on the
/// increase action Xantiax never takes, so neither grammar can reach the other. Exact text
/// and complete shape over every same-text registry record, card abilities only: no clan
/// bonus prints it. Returns the resource and N.
///
/// `+N Players Life` has the same shape with `valueMin` 1 and pays in four captured rounds,
/// but none of them knocks a player out. Since the Pillz form pays a player the round has
/// knocked out, the server applies these gains after a knockout, and whether the Life form
/// then revives a player at 0 is exactly what no round shows; it stays closed (a selected
/// hazard in replay) until one does.
pub(crate) fn classify_victory_or_defeat_both_players_gain(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(BothPlayersGainV1, u16)> {
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let resource = both_players_gain_shape(definition)?;
    if resource == BothPlayersGainV1::Life {
        return None;
    }
    let amount = definition.structured_input().value;
    let noun = match resource {
        BothPlayersGainV1::Life => "Life",
        BothPlayersGainV1::Pillz => "Pillz",
    };
    (definition.description() == format!("Victory Or Defeat : +{amount} Players {noun}"))
        .then_some((resource, amount))
}

/// Structural half of the boundary, so replay preparation can reject a complete shape under
/// malformed text instead of silently disabling it.
pub(crate) fn has_victory_or_defeat_both_players_gain_shape(
    definition: &EffectDefinitionV1,
) -> bool {
    both_players_gain_shape(definition).is_some()
}

fn both_players_gain_shape(definition: &EffectDefinitionV1) -> Option<BothPlayersGainV1> {
    let input = definition.structured_input();
    if input.value == 0 {
        return None;
    }
    let shape = |attribute, value_min| {
        shape_matches(
            input,
            PostRoundShapeV1 {
                value_min: ShapeFieldV1::Exact(value_min),
                current_round: CurrentRoundRequirementV1::Any,
                side: AffectedSideV1::Both,
                attribute,
                ..POST_ROUND_SHAPE
            },
        )
    };
    if shape(AttributeAffectedV1::Life, 1) {
        Some(BothPlayersGainV1::Life)
    } else if shape(AttributeAffectedV1::Pillz, 0) {
        Some(BothPlayersGainV1::Pillz)
    } else {
        None
    }
}

/// `Equalizer: - N Opp. Life Min M`: a won round reduces the opposing player's Life by N
/// per star of the opposing selected card, never below M. The two reviewed identities keep
/// their exact record and either source slot, because a captured Copy materialises them as
/// a Bonus (924669/2). Every other record is the grammar the two of them pinned: exact text
/// rebuilt from the record's own numbers over the complete stars-linked shape, card abilities
/// only - which is what admits El Cazador's Min 0 `5793`.
pub(crate) fn classify_equalizer_opponent_life_on_victory(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16)> {
    let input = definition.structured_input();
    if equalizer_opponent_life_id_is_identity_locked(definition.id()) {
        return (equalizer_opponent_life_on_victory_identity_matches(source_kind, definition.id())
            && definition.description() == "Equalizer: - 1 Opp. Life Min 2"
            && equalizer_opponent_life_on_victory_shape_matches(input))
        .then_some((input.value, input.value_min));
    }
    let (per_star, minimum) = (input.value, input.value_min);
    (source_kind == CombatStatEffectSourceV1::Ability
        && per_star > 0
        && definition.description() == format!("Equalizer: - {per_star} Opp. Life Min {minimum}")
        && equalizer_opponent_life_grammar_shape_matches(input))
    .then_some((per_star, minimum))
}

/// The two reviewed ids that never reach the grammar, so neither can be relabelled.
pub(crate) const fn equalizer_opponent_life_id_is_identity_locked(definition_id: u32) -> bool {
    matches!(definition_id, 1415 | 4458)
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
    ) && equalizer_opponent_life_id_is_identity_locked(definition_id)
}

/// What a `Growth:`/`Degrowth:` post-round grammar pays, before resolution binds the round.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RoundScaledPostRoundEffectV1 {
    ReduceOpponentLife { per_round: u16, minimum: u16 },
    ReduceOpponentPillz { per_round: u16, minimum: u16 },
    GainLife { per_round: u16 },
    GainPillz { per_round: u16 },
}

impl RoundScaledPostRoundEffectV1 {
    pub(crate) fn effects(
        self,
        scale: RoundScaleV1,
    ) -> (CombatStatPostRoundEffectV1, CombatStatEffectV1) {
        match self {
            Self::ReduceOpponentLife { per_round, minimum } => (
                CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryPerRound {
                    per_round,
                    minimum,
                    scale,
                },
                CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerRound {
                    per_round,
                    minimum,
                    scale,
                },
            ),
            Self::ReduceOpponentPillz { per_round, minimum } => (
                CombatStatPostRoundEffectV1::ReduceOpponentPillzOnVictoryPerRound {
                    per_round,
                    minimum,
                    scale,
                },
                CombatStatEffectV1::ReduceOpponentPillzOnVictoryPerRound {
                    per_round,
                    minimum,
                    scale,
                },
            ),
            Self::GainLife { per_round } => (
                CombatStatPostRoundEffectV1::GainLifeOnVictoryPerRound { per_round, scale },
                CombatStatEffectV1::GainLifeOnVictoryPerRound { per_round, scale },
            ),
            Self::GainPillz { per_round } => (
                CombatStatPostRoundEffectV1::GainPillzOnVictoryPerRound { per_round, scale },
                CombatStatEffectV1::GainPillzOnVictoryPerRound { per_round, scale },
            ),
        }
    }
}

/// Recognize the `Growth:` and `Degrowth:` forms of the plain Victory grammars - the
/// opposing Life reduction, the opposing Pillz reduction, and the own Life and Pillz gains:
/// the printed amount is multiplied by the zero-based round plus one (Growth) or four less
/// it (Degrowth), then paid and clamped by the arm that pays the plain grammar. The registry
/// carries the scaling as `isOverdrive`/`isDivide` and nothing else differs. Exact text
/// rebuilt from the record's own numbers, card abilities only; the permanent Growth Poison
/// has its own structure and is not this family.
pub(crate) fn classify_round_scaled_post_round(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(RoundScaleV1, RoundScaledPostRoundEffectV1)> {
    let input = definition.structured_input();
    if source_kind != CombatStatEffectSourceV1::Ability || input.value == 0 {
        return None;
    }
    let (scale, prefix) = match (input.is_overdrive, input.is_divide) {
        (true, false) => (RoundScaleV1::Growth, "Growth: "),
        (false, true) => (RoundScaleV1::Degrowth, "Degrowth: "),
        _ => return None,
    };
    let shape = |fields: PostRoundShapeV1| {
        shape_matches(
            input,
            PostRoundShapeV1 {
                round_scale: Some(scale),
                ..fields
            },
        )
    };
    let (effect, body) = if shape(PostRoundShapeV1 {
        value_min: ShapeFieldV1::Read,
        side: AffectedSideV1::Opponent,
        action: AttributeActionV1::Decrease,
        ..POST_ROUND_SHAPE
    }) {
        (
            RoundScaledPostRoundEffectV1::ReduceOpponentLife {
                per_round: input.value,
                minimum: input.value_min,
            },
            format!("- {} Opp. Life Min {}", input.value, input.value_min),
        )
    } else if shape(PostRoundShapeV1 {
        value_min: ShapeFieldV1::Read,
        side: AffectedSideV1::Opponent,
        attribute: AttributeAffectedV1::Pillz,
        action: AttributeActionV1::Decrease,
        ..POST_ROUND_SHAPE
    }) {
        (
            RoundScaledPostRoundEffectV1::ReduceOpponentPillz {
                per_round: input.value,
                minimum: input.value_min,
            },
            format!("-{} Opp Pillz. Min {}", input.value, input.value_min),
        )
    } else if shape(POST_ROUND_SHAPE) {
        (
            RoundScaledPostRoundEffectV1::GainLife {
                per_round: input.value,
            },
            format!("+{} Life", input.value),
        )
    } else if shape(PostRoundShapeV1 {
        attribute: AttributeAffectedV1::Pillz,
        ..POST_ROUND_SHAPE
    }) {
        (
            RoundScaledPostRoundEffectV1::GainPillz {
                per_round: input.value,
            },
            format!("+{} Pillz", input.value),
        )
    } else {
        return None;
    };
    (definition.description() == format!("{prefix}{body}")).then_some((scale, effect))
}

/// Structural half of the round-scaled post-round boundary.
pub(crate) fn has_round_scaled_post_round_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    if input.value == 0 {
        return false;
    }
    let Some(scale) = (match (input.is_overdrive, input.is_divide) {
        (true, false) => Some(RoundScaleV1::Growth),
        (false, true) => Some(RoundScaleV1::Degrowth),
        _ => None,
    }) else {
        return false;
    };
    [
        PostRoundShapeV1 {
            value_min: ShapeFieldV1::Read,
            side: AffectedSideV1::Opponent,
            action: AttributeActionV1::Decrease,
            ..POST_ROUND_SHAPE
        },
        PostRoundShapeV1 {
            value_min: ShapeFieldV1::Read,
            side: AffectedSideV1::Opponent,
            attribute: AttributeAffectedV1::Pillz,
            action: AttributeActionV1::Decrease,
            ..POST_ROUND_SHAPE
        },
        POST_ROUND_SHAPE,
        PostRoundShapeV1 {
            attribute: AttributeAffectedV1::Pillz,
            ..POST_ROUND_SHAPE
        },
    ]
    .into_iter()
    .any(|fields| {
        shape_matches(
            input,
            PostRoundShapeV1 {
                round_scale: Some(scale),
                ..fields
            },
        )
    })
}

/// What a post-round `Brawl:` grammar pays, before resolution binds the count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BrawlPostRoundEffectV1 {
    ReduceOpponentLife {
        per_count: u16,
        minimum: u16,
    },
    ReduceOpponentPillz {
        per_count: u16,
        minimum: u16,
    },
    /// `maximum == 0` is the uncapped form.
    GainPillz {
        per_count: u16,
        maximum: u16,
    },
}

impl BrawlPostRoundEffectV1 {
    /// The public and compact representations, which catalog and replay preparation must
    /// build identically.
    pub(crate) fn effects(self) -> (CombatStatPostRoundEffectV1, CombatStatEffectV1) {
        match self {
            Self::ReduceOpponentLife { per_count, minimum } => (
                CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryPerAntiSupport {
                    per_count,
                    minimum,
                },
                CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerAntiSupport {
                    per_count,
                    minimum,
                },
            ),
            Self::ReduceOpponentPillz { per_count, minimum } => (
                CombatStatPostRoundEffectV1::ReduceOpponentPillzOnVictoryPerAntiSupport {
                    per_count,
                    minimum,
                },
                CombatStatEffectV1::ReduceOpponentPillzOnVictoryPerAntiSupport {
                    per_count,
                    minimum,
                },
            ),
            Self::GainPillz { per_count, maximum } => (
                CombatStatPostRoundEffectV1::GainPillzOnVictoryPerAntiSupport {
                    per_count,
                    maximum,
                },
                CombatStatEffectV1::GainPillzOnVictoryPerAntiSupport { per_count, maximum },
            ),
        }
    }
}

/// Recognize the post-round `Brawl:` grammars: a won round pays the printed amount once per
/// distinct character in the opposing hand sharing the opposing selected card's effective
/// clan - the anti-support count combat-stat Brawl already reads - onto the opposing Life,
/// the opposing Pillz or the owner's own Pillz. Each is the plain Victory grammar of that
/// resource with the one per-X flag every other post-round shape requires false, so the
/// count is bound at resolution the way Equalizer's stars are and the bound effect is paid by
/// the arm that already pays the plain grammar. Exact printed text rebuilt from the record's
/// own numbers, complete structured shape, card abilities only: no clan bonus prints one.
/// What a `Bet > N Pillz:` post-round grammar pays: the plain Victory Life gain and the
/// plain Victory opponent-Life and opponent-Pillz reductions, each gated on the owner's
/// `pillzUsed`. Only `Bet >` is printed over these bodies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BetGatedPostRoundEffectV1 {
    GainLife { life: u16 },
    ReduceOpponentLife { life: u16, minimum: u16 },
    ReduceOpponentPillz { pillz: u16, minimum: u16 },
}

impl BetGatedPostRoundEffectV1 {
    pub(crate) fn effects(self) -> (CombatStatPostRoundEffectV1, CombatStatEffectV1) {
        match self {
            Self::GainLife { life } => (
                CombatStatPostRoundEffectV1::GainLifeOnVictory { life },
                CombatStatEffectV1::GainLifeOnVictory { life },
            ),
            Self::ReduceOpponentLife { life, minimum } => (
                CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictory { life, minimum },
                CombatStatEffectV1::ReduceOpponentLifeOnVictory { life, minimum },
            ),
            Self::ReduceOpponentPillz { pillz, minimum } => (
                CombatStatPostRoundEffectV1::ReduceOpponentPillzOnVictory { pillz, minimum },
                CombatStatEffectV1::ReduceOpponentPillzOnVictory { pillz, minimum },
            ),
        }
    }
}

/// Recognize `Bet > N Pillz:` over the three plain Victory bodies: exact text rebuilt from
/// the structured gate and magnitudes, and the complete shape of the plain grammar with
/// only the bet fields set. Card abilities, and the Zenith clan bonus for the Life gain.
pub(crate) fn classify_bet_gated_post_round(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(BetGatedPostRoundEffectV1, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    if input.value == 0 {
        return None;
    }
    let (predicate, prefix) = bet_gate(input)?;
    if !matches!(predicate, CombatStatPredicateV1::OwnerPillzUsedAbove(_)) {
        return None;
    }
    let (effect, body) = if bet_gated_shape_matches(input, POST_ROUND_SHAPE) {
        (
            BetGatedPostRoundEffectV1::GainLife { life: input.value },
            format!("+{} Life", input.value),
        )
    } else if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    } else if bet_gated_shape_matches(input, BET_OPPONENT_LIFE_SHAPE) {
        (
            BetGatedPostRoundEffectV1::ReduceOpponentLife {
                life: input.value,
                minimum: input.value_min,
            },
            format!("-{} Opp. Life Min {}", input.value, input.value_min),
        )
    } else if bet_gated_shape_matches(input, BET_OPPONENT_PILLZ_SHAPE) {
        (
            BetGatedPostRoundEffectV1::ReduceOpponentPillz {
                pillz: input.value,
                minimum: input.value_min,
            },
            format!("-{} Opp Pillz. Min {}", input.value, input.value_min),
        )
    } else {
        return None;
    };
    (definition.description() == format!("{prefix}{body}")).then_some((effect, predicate))
}

const BET_OPPONENT_LIFE_SHAPE: PostRoundShapeV1 = PostRoundShapeV1 {
    value_min: ShapeFieldV1::Read,
    side: AffectedSideV1::Opponent,
    action: AttributeActionV1::Decrease,
    ..POST_ROUND_SHAPE
};

const BET_OPPONENT_PILLZ_SHAPE: PostRoundShapeV1 = PostRoundShapeV1 {
    value_min: ShapeFieldV1::Read,
    side: AffectedSideV1::Opponent,
    attribute: AttributeAffectedV1::Pillz,
    action: AttributeActionV1::Decrease,
    ..POST_ROUND_SHAPE
};

fn bet_gated_shape_matches(input: &StructuredEffectV1, shape: PostRoundShapeV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            bet_gated: true,
            ..shape
        },
    )
}

/// Structural half of the Bet post-round boundary, so replay preparation can reject a
/// complete shape under malformed text instead of silently disabling it.
pub(crate) fn has_bet_gated_post_round_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && [
            POST_ROUND_SHAPE,
            BET_OPPONENT_LIFE_SHAPE,
            BET_OPPONENT_PILLZ_SHAPE,
        ]
        .into_iter()
        .any(|shape| bet_gated_shape_matches(input, shape))
}

pub(crate) fn classify_brawl_post_round(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<BrawlPostRoundEffectV1> {
    let input = definition.structured_input();
    if source_kind != CombatStatEffectSourceV1::Ability || input.value == 0 {
        return None;
    }
    let (effect, text) = if brawl_opponent_life_shape_matches(input) {
        (
            BrawlPostRoundEffectV1::ReduceOpponentLife {
                per_count: input.value,
                minimum: input.value_min,
            },
            format!("Brawl: - {} Opp. Life Min {}", input.value, input.value_min),
        )
    } else if brawl_opponent_pillz_shape_matches(input) {
        (
            BrawlPostRoundEffectV1::ReduceOpponentPillz {
                per_count: input.value,
                minimum: input.value_min,
            },
            format!(
                "Brawl: -{} Opp. Pillz, Min {}",
                input.value, input.value_min
            ),
        )
    } else if brawl_own_pillz_shape_matches(input) {
        (
            BrawlPostRoundEffectV1::GainPillz {
                per_count: input.value,
                maximum: input.value_max,
            },
            if input.value_max == 0 {
                format!("Brawl: +{} Pillz", input.value)
            } else {
                format!("Brawl: +{} Pillz, Max. {}", input.value, input.value_max)
            },
        )
    } else {
        return None;
    };
    (definition.description() == text).then_some(effect)
}

/// Structural half of the post-round Brawl boundary, so replay preparation can reject a
/// complete shape under malformed text instead of silently disabling it.
pub(crate) fn has_brawl_post_round_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && (brawl_opponent_life_shape_matches(input)
            || brawl_opponent_pillz_shape_matches(input)
            || brawl_own_pillz_shape_matches(input))
}

/// What a post-round `Support:` grammar pays, before resolution binds the count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SupportPostRoundEffectV1 {
    ReduceOpponentLife { per_count: u16, minimum: u16 },
    GainLife { per_count: u16 },
    GainPillz { per_count: u16 },
}

impl SupportPostRoundEffectV1 {
    /// The public and compact representations, which catalog and replay preparation must
    /// build identically.
    pub(crate) fn effects(self) -> (CombatStatPostRoundEffectV1, CombatStatEffectV1) {
        match self {
            Self::ReduceOpponentLife { per_count, minimum } => (
                CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryPerSupport {
                    per_count,
                    minimum,
                },
                CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerSupport { per_count, minimum },
            ),
            Self::GainLife { per_count } => (
                CombatStatPostRoundEffectV1::GainLifeOnVictoryPerSupport { per_count },
                CombatStatEffectV1::GainLifeOnVictoryPerSupport { per_count },
            ),
            Self::GainPillz { per_count } => (
                CombatStatPostRoundEffectV1::GainPillzOnVictoryPerSupport { per_count },
                CombatStatEffectV1::GainPillzOnVictoryPerSupport { per_count },
            ),
        }
    }
}

/// Recognize the post-round `Support:` grammars: a won round pays the printed amount once
/// per distinct character in the owner's hand sharing the owner's selected card's effective
/// clan - the Support count the combat-stat Support abilities already read - onto the
/// opposing Life or the owner's own Life or Pillz. Each is the plain Victory grammar of that
/// resource with `isSupport` set, which every other post-round shape requires false, so the
/// count is bound at resolution the way Brawl's is and the bound effect is paid by the arm
/// that already pays the plain grammar. Exact printed text rebuilt from the record's own
/// numbers - note the space in `+ 1 Pillz` - complete structured shape, card abilities only:
/// no clan bonus prints one. `Support: Dope 1, Max. 4` is a Pillz permanent and not this
/// family.
pub(crate) fn classify_support_post_round(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<SupportPostRoundEffectV1> {
    let input = definition.structured_input();
    if source_kind != CombatStatEffectSourceV1::Ability || input.value == 0 {
        return None;
    }
    let (effect, text) = if support_opponent_life_shape_matches(input) {
        (
            SupportPostRoundEffectV1::ReduceOpponentLife {
                per_count: input.value,
                minimum: input.value_min,
            },
            format!(
                "Support: -{} Opp. Life, Min {}",
                input.value, input.value_min
            ),
        )
    } else if support_own_life_shape_matches(input) {
        (
            SupportPostRoundEffectV1::GainLife {
                per_count: input.value,
            },
            format!("Support: +{} Life", input.value),
        )
    } else if support_own_pillz_shape_matches(input) {
        (
            SupportPostRoundEffectV1::GainPillz {
                per_count: input.value,
            },
            format!("Support: + {} Pillz", input.value),
        )
    } else {
        return None;
    };
    (definition.description() == text).then_some(effect)
}

/// Structural half of the post-round Support boundary, so replay preparation can reject a
/// complete shape under malformed text instead of silently disabling it.
pub(crate) fn has_support_post_round_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && (support_opponent_life_shape_matches(input)
            || support_own_life_shape_matches(input)
            || support_own_pillz_shape_matches(input))
}

fn support_opponent_life_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value_min: ShapeFieldV1::Read,
            side: AffectedSideV1::Opponent,
            action: AttributeActionV1::Decrease,
            support: true,
            ..POST_ROUND_SHAPE
        },
    )
}

fn support_own_life_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            support: true,
            ..POST_ROUND_SHAPE
        },
    )
}

fn support_own_pillz_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            attribute: AttributeAffectedV1::Pillz,
            support: true,
            ..POST_ROUND_SHAPE
        },
    )
}

/// What a post-round `Equalizer:` own gain pays, before resolution binds the stars.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EqualizerPostRoundGainV1 {
    Life { per_star: u16 },
    Pillz { per_star: u16 },
}

impl EqualizerPostRoundGainV1 {
    pub(crate) fn effects(self) -> (CombatStatPostRoundEffectV1, CombatStatEffectV1) {
        match self {
            Self::Life { per_star } => (
                CombatStatPostRoundEffectV1::GainLifeOnVictoryPerOpponentStars { per_star },
                CombatStatEffectV1::GainLifeOnVictoryPerOpponentStars { per_star },
            ),
            Self::Pillz { per_star } => (
                CombatStatPostRoundEffectV1::GainPillzOnVictoryPerOpponentStars { per_star },
                CombatStatEffectV1::GainPillzOnVictoryPerOpponentStars { per_star },
            ),
        }
    }
}

/// Recognize `Equalizer: +N Life` and `Equalizer: +N Pillz`: a won round gives the owner N
/// Life or Pillz per star of the opposing selected card - the magnitude the Equalizer
/// opponent-Life reduction and the combat-stat Equalizers already bind - paid by the arm
/// that pays the plain Victory gain. Exact text, complete stars-linked shape, card abilities
/// only; the capped, compound, clan-gated and Victory-or-Defeat forms differ in a structured
/// field and stay closed.
pub(crate) fn classify_equalizer_post_round_gain(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<EqualizerPostRoundGainV1> {
    let input = definition.structured_input();
    if source_kind != CombatStatEffectSourceV1::Ability || input.value == 0 {
        return None;
    }
    let (gain, text) = if equalizer_own_gain_shape_matches(input, AttributeAffectedV1::Life) {
        (
            EqualizerPostRoundGainV1::Life {
                per_star: input.value,
            },
            format!("Equalizer: +{} Life", input.value),
        )
    } else if equalizer_own_gain_shape_matches(input, AttributeAffectedV1::Pillz) {
        (
            EqualizerPostRoundGainV1::Pillz {
                per_star: input.value,
            },
            format!("Equalizer: +{} Pillz", input.value),
        )
    } else {
        return None;
    };
    (definition.description() == text).then_some(gain)
}

/// Structural half of the post-round Equalizer boundary: the opponent-Life reduction at any
/// numbers and the two own gains.
pub(crate) fn has_equalizer_post_round_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    input.value > 0
        && (equalizer_opponent_life_grammar_shape_matches(input)
            || equalizer_own_gain_shape_matches(input, AttributeAffectedV1::Life)
            || equalizer_own_gain_shape_matches(input, AttributeAffectedV1::Pillz))
}

fn equalizer_opponent_life_grammar_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value_min: ShapeFieldV1::Read,
            side: AffectedSideV1::Opponent,
            action: AttributeActionV1::Decrease,
            opponent_stars_linked: true,
            ..POST_ROUND_SHAPE
        },
    )
}

fn equalizer_own_gain_shape_matches(
    input: &StructuredEffectV1,
    attribute: AttributeAffectedV1,
) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            attribute,
            opponent_stars_linked: true,
            ..POST_ROUND_SHAPE
        },
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

/// Revision 70: the owner-clan gate `[clan:A][clan:B] ` over end-of-round bodies the
/// projection already executes, each by the exact text its one printed record carries and by
/// the ungated grammar's complete shape once the gate is cleared (`without_owner_clan_gate`),
/// so no existing gate is loosened. Card abilities only.
/// - `- N Opp. Life Min M` (Phalloide Ld `5392`, spaced minus), the Victory opponent-Life
///   reduction;
/// - `-N Opp Pillz. Min M` (Dark Kaizerin `4037`, `4038`), the Victory opponent-Pillz one;
/// - `Equalizer: +N Life` / `Equalizer: +N Pillz` (Dark Mandrak `5616`, Synapsburg `5165`);
/// - `Toxin N, Min M` (Dark Eloxia `5613`), on the Victory latch;
/// - `Repris.: Consume N, Min M` (Dunkelstern `5275`), on the latch with Reprisal's second
///   move, which its record carries in the position field.
/// The gate is decided at resolution from the owner's effective clan, so an Oculus that
/// infiltrates a listed clan pays and one that does not holds a source that never fires.
/// Returns the public effect, the compact effect and the predicate.
pub(crate) fn classify_clan_gated_post_round(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(
    CombatStatPostRoundEffectV1,
    CombatStatEffectV1,
    CombatStatPredicateV1,
)> {
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let input = definition.structured_input();
    let (set, body) = owner_clan_gate(input, definition.description())?;
    let plain = without_owner_clan_gate(input);
    let (value, minimum) = (plain.value, plain.value_min);
    if value == 0 {
        return None;
    }
    let gated = CombatStatPredicateV1::OwnerClanIn(set);
    let unconditional_latch =
        || permanent_condition(&plain) == Some((CombatStatPredicateV1::Always, ""));
    if victory_opponent_life_shape_matches(&plain, value, minimum, CombatStatPredicateV1::Always)
        && body == format!("- {value} Opp. Life Min {minimum}")
    {
        return Some((
            CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictory {
                life: value,
                minimum,
            },
            CombatStatEffectV1::ReduceOpponentLifeOnVictory {
                life: value,
                minimum,
            },
            gated,
        ));
    }
    if victory_opponent_pillz_shape_matches(&plain)
        && body == format!("-{value} Opp Pillz. Min {minimum}")
    {
        return Some((
            CombatStatPostRoundEffectV1::ReduceOpponentPillzOnVictory {
                pillz: value,
                minimum,
            },
            CombatStatEffectV1::ReduceOpponentPillzOnVictory {
                pillz: value,
                minimum,
            },
            gated,
        ));
    }
    for (attribute, name, gain) in [
        (
            AttributeAffectedV1::Life,
            "Life",
            EqualizerPostRoundGainV1::Life { per_star: value },
        ),
        (
            AttributeAffectedV1::Pillz,
            "Pillz",
            EqualizerPostRoundGainV1::Pillz { per_star: value },
        ),
    ] {
        if equalizer_own_gain_shape_matches(&plain, attribute)
            && body == format!("Equalizer: +{value} {name}")
        {
            let (effect, compact_effect) = gain.effects();
            return Some((effect, compact_effect, gated));
        }
    }
    if permanent_opponent_life_shape_matches(&plain, true)
        && unconditional_latch()
        && body == format!("Toxin {value}, Min {minimum}")
    {
        return Some((
            CombatStatPostRoundEffectV1::ToxinOpponentLifeOnVictory {
                life: value,
                minimum,
            },
            CombatStatEffectV1::ToxinOpponentLifeOnVictory {
                life: value,
                minimum,
            },
            gated,
        ));
    }
    if plain.position_requirement == PositionRequirementV1::Defender
        && body == format!("Repris.: Consume {value}, Min {minimum}")
    {
        let mut both = plain.clone();
        both.position_requirement = PositionRequirementV1::Both;
        if consume_opponent_pillz_shape_matches(&both)
            && permanent_condition(&both) == Some((CombatStatPredicateV1::Always, ""))
        {
            return Some((
                CombatStatPostRoundEffectV1::ConsumeOpponentPillzOnVictory {
                    pillz: value,
                    minimum,
                },
                CombatStatEffectV1::ConsumeOpponentPillzOnVictory {
                    pillz: value,
                    minimum,
                },
                CombatStatPredicateV1::OwnerClanInAnd(set, ClanConjunctV1::OwnerMovesSecond),
            ));
        }
    }
    None
}

/// Structural half of the clan-gated end-of-round boundary: a record carrying only the
/// owner-clan gate that is, once the gate is cleared, the complete shape of one of the
/// bodies above - whatever its text says.
pub(crate) fn has_clan_gated_post_round_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    if input.clan_requirement.is_empty()
        || !input.opponent_clan_requirement.is_empty()
        || !input.previous_clan_requirement.is_empty()
        || input.value == 0
    {
        return false;
    }
    let plain = without_owner_clan_gate(input);
    let unconditional_latch = |input: &StructuredEffectV1| {
        permanent_condition(input) == Some((CombatStatPredicateV1::Always, ""))
    };
    let mut both = plain.clone();
    both.position_requirement = PositionRequirementV1::Both;
    victory_opponent_life_shape_matches(
        &plain,
        plain.value,
        plain.value_min,
        CombatStatPredicateV1::Always,
    ) || victory_opponent_pillz_shape_matches(&plain)
        || equalizer_own_gain_shape_matches(&plain, AttributeAffectedV1::Life)
        || equalizer_own_gain_shape_matches(&plain, AttributeAffectedV1::Pillz)
        || (permanent_opponent_life_shape_matches(&plain, true) && unconditional_latch(&plain))
        || (plain.position_requirement == PositionRequirementV1::Defender
            && consume_opponent_pillz_shape_matches(&both)
            && unconditional_latch(&both))
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

/// `Defeat: Poison N, Min M` is the plain Poison latch on the losing side: the owner having
/// lost the round is what latches it, and every later round pays as usual. It is the first
/// admitted permanent whose trigger is not a win, so the engine gains a `LatchOnDefeat`
/// beside `LatchOnVictory`; the repeat loop and `LatchedEffectV1` are trigger-agnostic and
/// are reused unchanged.
///
/// Note the printed text has no space before its colon, unlike the `Defeat : Heal` forms.
pub(crate) fn classify_poison_opponent_life_on_defeat(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16)> {
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let input = definition.structured_input();
    (has_poison_opponent_life_on_defeat_shape(definition)
        && definition.description()
            == format!("Defeat: Poison {}, Min {}", input.value, input.value_min))
    .then_some((input.value, input.value_min))
}

pub(crate) fn has_poison_opponent_life_on_defeat_shape(definition: &EffectDefinitionV1) -> bool {
    permanent_opponent_life_shape_matches_on(
        definition.structured_input(),
        false,
        CurrentRoundRequirementV1::Lose,
    )
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

/// `Consume N, Min M`: the first Pillz permanent. A won round latches it and pays at once;
/// every later round then takes `pillz` from the opposing player's Pillz while above
/// `minimum`. Card abilities only: no clan prints it as a bonus. The `Unison :` and
/// clan-gated `Repris.:` forms carry the clan-mates link, or a clan list and a position, so
/// no shape here reaches them. Returns `(pillz, minimum)`.
pub(crate) fn classify_consume_opponent_pillz_on_victory(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    let (predicate, prefix) = permanent_condition(input)?;
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_consume_opponent_pillz_on_victory_shape(definition)
        && definition.description()
            == format!("{prefix}Consume {}, Min {}", input.value, input.value_min))
    .then_some((input.value, input.value_min, predicate))
}

pub(crate) fn has_consume_opponent_pillz_on_victory_shape(definition: &EffectDefinitionV1) -> bool {
    consume_opponent_pillz_shape_matches(definition.structured_input())
}

/// `Combust N, Min M`: the first compound permanent. A won round latches it and pays
/// nothing; every later round takes `amount` Life and `amount` Pillz from the opposing
/// player, each only while above `minimum`. Card abilities only. `Mindwipe` moves the same
/// resources but pays at once and has no captured paying round, so it is not this grammar.
/// Returns `(amount, minimum)`.
pub(crate) fn classify_combust_opponent_life_and_pillz_on_victory(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    let (predicate, prefix) = permanent_condition(input)?;
    (source_kind == CombatStatEffectSourceV1::Ability
        && has_combust_opponent_life_and_pillz_on_victory_shape(definition)
        && definition.description()
            == format!("{prefix}Combust {}, Min {}", input.value, input.value_min))
    .then_some((input.value, input.value_min, predicate))
}

pub(crate) fn has_combust_opponent_life_and_pillz_on_victory_shape(
    definition: &EffectDefinitionV1,
) -> bool {
    combust_opponent_life_and_pillz_shape_matches(definition.structured_input())
}

/// Which outcome latches an admitted Dope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DopeLatchV1 {
    /// `Dope N, Max. M`: a won round.
    Victory,
    /// `Defeat: Dope N, Max. M`: a lost one.
    Defeat,
}

impl DopeLatchV1 {
    /// The public and compact plans, which catalog and replay preparation must build
    /// identically.
    pub(crate) const fn effects(
        self,
        pillz: u16,
        maximum: u16,
    ) -> (CombatStatPostRoundEffectV1, CombatStatEffectV1) {
        match self {
            Self::Victory => (
                CombatStatPostRoundEffectV1::DopePillzOnVictory { pillz, maximum },
                CombatStatEffectV1::DopePillzOnVictory { pillz, maximum },
            ),
            Self::Defeat => (
                CombatStatPostRoundEffectV1::DopePillzOnDefeat { pillz, maximum },
                CombatStatEffectV1::DopePillzOnDefeat { pillz, maximum },
            ),
        }
    }
}

/// `Dope N, Max. M` and `Defeat: Dope N, Max. M`: Regen on the owner's Pillz. The round that
/// latches it pays at once - `isImmediatePermanent`, as on Regen - and every later round then
/// raises the owner's Pillz by N while below M, never past it. Card abilities only: no clan
/// prints one. The `Support:` form scales by the clan count and is another grammar.
/// Returns `(pillz, maximum, latch)`.
pub(crate) fn classify_dope_pillz(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(u16, u16, DopeLatchV1)> {
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let input = definition.structured_input();
    let (latch, prefix) = if dope_pillz_shape_matches(input, CurrentRoundRequirementV1::Win) {
        (DopeLatchV1::Victory, "")
    } else if dope_pillz_shape_matches(input, CurrentRoundRequirementV1::Lose) {
        (DopeLatchV1::Defeat, "Defeat: ")
    } else {
        return None;
    };
    (definition.description() == format!("{prefix}Dope {}, Max. {}", input.value, input.value_max))
        .then_some((input.value, input.value_max, latch))
}

/// Structural half of the Dope boundary, on either latch, so replay preparation can reject
/// the complete shape under malformed text instead of disabling it.
pub(crate) fn has_dope_pillz_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    dope_pillz_shape_matches(input, CurrentRoundRequirementV1::Win)
        || dope_pillz_shape_matches(input, CurrentRoundRequirementV1::Lose)
}

/// An own-Pillz increase of `value` capped at `value_max`, permanent and immediate, with no
/// floor and no condition beside the outcome that latches it.
fn dope_pillz_shape_matches(
    input: &StructuredEffectV1,
    current_round: CurrentRoundRequirementV1,
) -> bool {
    input.value > 0
        && input.value_min == 0
        && input.value_max > input.value
        && input.previous_round_requirement == PreviousRoundRequirementV1::Any
        && input.index_requirement == IndexRequirementV1::Any
        && permanent_life_neutral_shape_matches(
            input,
            AffectedSideV1::Player,
            AttributeAffectedV1::Pillz,
            AttributeActionV1::Increase,
            true,
            current_round,
        )
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
        || classify_consume_opponent_pillz_on_victory(definition, source_kind).is_some()
        || classify_combust_opponent_life_and_pillz_on_victory(definition, source_kind).is_some()
        || classify_dope_pillz(definition, source_kind).is_some()
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
    // So does the Killshot reduction, on the `sureshot` channel, its compound gain and the
    // Killshot own gains and Toxin latch.
    if classify_killshot_opponent_life(definition, source_kind).is_some()
        || classify_killshot_pillz_and_life(definition, source_kind).is_some()
        || classify_killshot_post_round(definition, source_kind).is_some()
    {
        return None;
    }
    // Recovery has its own post-round execution channel. Keep it out of this combat-stat
    // return type so neither generic numeric admission nor cancellation can reinterpret it.
    if classify_recover_pillz(definition, source_kind).is_some() {
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
        || classify_victory_pillz_max(definition, source_kind).is_some()
        || classify_victory_opponent_pillz(definition, source_kind).is_some()
        || classify_victory_pillz_per_damage(definition, source_kind).is_some()
        || classify_victory_life_per_damage(definition, source_kind).is_some()
        || classify_victory_life_per_opponent_damage(definition, source_kind).is_some()
    {
        return None;
    }
    if classify_defeat_life(definition, source_kind).is_some()
        || classify_reanimate_life(definition, source_kind).is_some()
        || classify_defeat_pillz(definition, source_kind).is_some()
        || classify_defeat_pillz_and_life(definition, source_kind).is_some()
        || classify_backlash_life(definition, source_kind).is_some()
        || classify_defeat_capped_life(definition, source_kind).is_some()
        || classify_defeat_opponent_pillz_gain(definition, source_kind).is_some()
        || classify_corrupt_own_life(definition, source_kind).is_some()
    {
        return None;
    }
    // Victory Or Defeat Pillz likewise has its own post-round execution channel.
    // Keep it out of generic numeric classification in case the registry compiler later
    // broadens its Pillz support.
    if classify_victory_or_defeat_pillz(definition, source_kind) {
        return None;
    }
    if classify_victory_or_defeat_life(definition, source_kind).is_some()
        || classify_victory_or_defeat_both_players_gain(definition, source_kind).is_some()
    {
        return None;
    }
    if classify_equalizer_opponent_life_on_victory(definition, source_kind).is_some() {
        return None;
    }
    // The post-round Brawl grammars bind their count on the same channel as Equalizer's.
    // `classify_brawl_numeric` already refuses a Life or Pillz record, so this is the
    // convention every post-round grammar keeps rather than a live guard.
    if classify_brawl_post_round(definition, source_kind).is_some()
        || classify_round_scaled_post_round(definition, source_kind).is_some()
        || classify_bet_gated_post_round(definition, source_kind).is_some()
        || classify_support_post_round(definition, source_kind).is_some()
        || classify_equalizer_post_round_gain(definition, source_kind).is_some()
        || classify_clan_gated_post_round(definition, source_kind).is_some()
    {
        return None;
    }
    if classify_reprisal_stop_opponent_ability(definition, source_kind) {
        return Some((
            SupportedEffectV1::StopOpponentAbility,
            CombatStatPredicateV1::OwnerMovesSecond,
        ));
    }
    if let Some(classified) = classify_conditional_stop(definition, source_kind) {
        return Some(classified);
    }
    // Model-specific conditions take precedence over the registry's model-neutral output.
    // Keep the unconditional guard below as well, so a future registry compiler expansion
    // cannot silently erase a condition by returning Supported first.
    if let Some(classified) = classify_conditional_stat_copy(definition, source_kind) {
        return Some(classified);
    }
    if let Some(classified) = classify_equalizer_numeric(definition) {
        return Some(classified);
    }
    if let Some(classified) = classify_brawl_numeric(definition) {
        return Some(classified);
    }
    if let Some(classified) = classify_life_left_numeric(definition) {
        return Some(classified);
    }
    if let Some(classified) = classify_pillz_numeric(definition, source_kind) {
        return Some(classified);
    }
    if let Some(classified) = classify_round_scaled_numeric(definition) {
        return Some(classified);
    }
    if let Some(classified) = classify_unison_numeric(definition, source_kind) {
        return Some(classified);
    }
    if let Some(classified) = classify_position_numeric(definition, source_kind) {
        return Some(classified);
    }
    if let Some(classified) = classify_revenge_attack_per_opponent_power(definition, source_kind) {
        return Some(classified);
    }
    if let Some(classified) = classify_previous_round_numeric(definition, source_kind) {
        return Some(classified);
    }
    if let Some(classified) = classify_index_numeric(definition) {
        return Some(classified);
    }
    if let Some(classified) = classify_night_confidence_numeric(definition, source_kind) {
        return Some(classified);
    }
    if let Some(classified) = classify_day_night_numeric(definition, source_kind) {
        return Some(classified);
    }
    if let Some(classified) = classify_stop_triggered_numeric(definition, source_kind) {
        return Some(classified);
    }
    if let Some(classified) = classify_clan_gated(definition, source_kind) {
        return Some(classified);
    }
    if let Some(classified) = classify_bet_gated(definition, source_kind) {
        return Some(classified);
    }
    if let Some(classified) = classify_cards_numeric(definition, source_kind) {
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
        // No clan bonus prints an Exchange, an Impose or a resource canceller.
        SupportedEffectV1::ExchangePrintedCombatStat { .. }
        | SupportedEffectV1::ImposePrintedCombatStat { .. }
        | SupportedEffectV1::CancelOpponentResourceModifiers { .. } => {
            source_kind == CombatStatEffectSourceV1::Ability
        }
        // `Tune Out` has only ever been observed as the Cosmohnuts clan bonus. Noon Steevens
        // prints the same text as an ability, whose catalog levels own no registry definition
        // and whose Stop Opp. Ability liveness no round has shown, so that slot stays closed.
        SupportedEffectV1::SimplifyAttackToPillz => source_kind == CombatStatEffectSourceV1::Bonus,
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
                // `+N Attack Per Opp. Power` is printed by card abilities only.
                && !(source_kind != CombatStatEffectSourceV1::Ability
                    && multiplier == MagnitudeMultiplierV1::OpponentPower)
        }
    }
}

/// `Unison :` over a fixed numeric body. The registry marks the prefix with
/// `isClanmatesCountLinked` and refuses it as a linked magnitude, but it is a gate, not a
/// magnitude: the printed amount applies once when every card in the owner's hand shares
/// the owner's selected card's effective clan.
fn classify_unison_numeric(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let input = definition.structured_input();
    if !input.is_clanmates_count_linked || !neutral_except_clanmates_count(input) {
        return None;
    }
    let effect = numeric_effect(input, MagnitudeMultiplierV1::Fixed)?;
    let body = definition
        .description()
        .strip_prefix("Unison : ")
        .or_else(|| definition.description().strip_prefix("Unison: "))?;
    numeric_description_body_matches(body, effect, MagnitudeMultiplierV1::Fixed)
        .then_some((effect, CombatStatPredicateV1::OwnerHandUnison))
}

fn neutral_except_clanmates_count(input: &StructuredEffectV1) -> bool {
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
        && !input.is_opponent_stars_linked
        && input.is_clanmates_count_linked
        && !input.is_anti_clanmates_count_linked
        && !input.is_permanent
        && !input.is_immediate_permanent
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

/// Recognize the `Night:` and `Day:` forms of the plain fixed numeric grammar. The prefix is
/// a match constant - Clint City is at night or it is not - and the registry records no
/// structured trace of it at all, so the record must be the neutral unconditional numeric
/// shape and the text after the prefix one of that grammar's printed spellings. Both slots:
/// the GhosTown clan bonus prints `Night: -1 Opp Pow. And Damage, Min 1`.
///
/// The catalog selects a card's night ability and its clan's night bonus exactly when the
/// match is at night, and the server only ever sends the active variant, so in practice
/// the predicate always holds. It is a predicate rather than an assumption so that a
/// `Night:` source in a daylight match - a Copy, or a malformed capture - stays present and
/// never fires instead of acting unconditionally.
///
/// One record is admitted by identity over a stray bound. Djanghost Ld's night ability
/// `5391`, `Night: -4 Opp Power, Min 4`, carries `valueMax` 4 beside `value` 4 and `valueMin`
/// 4, and it is the only decreasing combat-stat record in the registry with a `valueMax`, so
/// `numeric_effect` refuses it. The printed text names no maximum and the server applies
/// none: 1025279/0 takes Doela Noel from 8 to 4 and GhosTown's night bonus to 3 (Attack 21),
/// and 1025413/0 takes Galahad from 6, floored at 4, and then to 3, which also pins the
/// ability before the bonus with each clamp applied in turn. So that one definition id, from
/// the Ability slot, with that exact text and the three equal numbers, compiles as the
/// ordinary decrease with no maximum; `numeric_effect` is not relaxed for anything else.
fn classify_day_night_numeric(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    let description = definition.description();
    let (predicate, body) = if let Some(body) = description.strip_prefix("Night: ") {
        (CombatStatPredicateV1::MatchIsNight, body)
    } else if let Some(body) = description.strip_prefix("Day: ") {
        (CombatStatPredicateV1::MatchIsDay, body)
    } else {
        return None;
    };
    let input = definition.structured_input();
    if input.position_requirement != PositionRequirementV1::Both || !neutral_except_position(input)
    {
        return None;
    }
    let effect = if djanghost_night_stray_maximum(definition, source_kind) {
        let mut input = input.clone();
        input.value_max = 0;
        numeric_effect(&input, MagnitudeMultiplierV1::Fixed)?
    } else {
        numeric_effect(input, MagnitudeMultiplierV1::Fixed)?
    };
    numeric_description_body_matches(body, effect, MagnitudeMultiplierV1::Fixed)
        .then_some((effect, predicate))
}

/// Djanghost Ld's night ability, the one Night numeric admitted over a stray `valueMax`.
const DJANGHOST_NIGHT_POWER_REDUCTION_ID: u32 = 5391;

/// Whether `definition` is exactly the reviewed Djanghost Ld record: its id, from the
/// Ability slot, its printed text, and `valueMax` equal to `value` and `valueMin`.
fn djanghost_night_stray_maximum(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> bool {
    let input = definition.structured_input();
    (source_kind, definition.id())
        == (
            CombatStatEffectSourceV1::Ability,
            DJANGHOST_NIGHT_POWER_REDUCTION_ID,
        )
        && definition.description() == "Night: -4 Opp Power, Min 4"
        && input.value == 4
        && input.value_min == 4
        && input.value_max == 4
}

/// Recognize the compound `Night: Confid.: <numeric body>` (Schwarz's night ability `1643`,
/// `Night: Confid.: -2 Opp Pow. & Damage, Min 3`): the plain fixed numeric grammar under
/// both the `Night:` match constant and `Confidence`'s won previous round. The record
/// carries the previous-round half in its one condition field, as every Confidence numeric
/// does, and nothing of the night half, which is read from the text as for the Night
/// numerics; the plan carries the conjunction as one predicate,
/// `OwnerWonPreviousRoundAtNight`. Card abilities only: no clan bonus prints it.
fn classify_night_confidence_numeric(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    let body = definition.description().strip_prefix("Night: Confid.: ")?;
    let input = definition.structured_input();
    if source_kind != CombatStatEffectSourceV1::Ability
        || input.previous_round_requirement != PreviousRoundRequirementV1::Win
        || !neutral_except_previous_round(input)
    {
        return None;
    }
    let effect = numeric_effect(input, MagnitudeMultiplierV1::Fixed)?;
    numeric_description_body_matches(body, effect, MagnitudeMultiplierV1::Fixed)
        .then_some((effect, CombatStatPredicateV1::OwnerWonPreviousRoundAtNight))
}

/// Recognize the `Cards` grammar: one fixed change to a combat stat of *both* selected
/// cards, `Cards <stat> +N` or `-N Cards <stat>, Min M`. The registry marks it
/// `sideAffected: both` and refuses it as a description context, so it is admitted here by
/// exact text over the neutral unconditional shape, the Night/Day route. Each card is
/// clamped on its own, and a card already at or below Min is left alone (1079078/3: Sue's 3
/// Damage under Rajesh's Min 4). Damage and Attack only - the two stats the server has shown
/// (Cards Damage in eight rounds, Cards Attack in 1078555/1) - and card abilities only, since
/// no clan bonus prints it.
fn classify_cards_numeric(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let input = definition.structured_input();
    if input.side_affected != AffectedSideV1::Both
        || input.position_requirement != PositionRequirementV1::Both
        || input.special_action != SpecialActionV1::None
        || input.is_support
        || input.value == 0
        || !neutral_except_position(input)
    {
        return None;
    }
    let operation = match input.attribute_action {
        AttributeActionV1::Increase if input.value_min == 0 && input.value_max == 0 => {
            StatOperationV1::Increase
        }
        AttributeActionV1::Decrease if input.value_max == 0 => StatOperationV1::Decrease,
        _ => return None,
    };
    let stat = match input.attribute_affected {
        AttributeAffectedV1::Attack => CombatStatV1::Attack,
        AttributeAffectedV1::Damage => CombatStatV1::Damage,
        _ => return None,
    };
    let effect = SupportedEffectV1::ModifyCombatStat {
        side: AffectedSideV1::Both,
        stat,
        operation,
        value: input.value,
        minimum: (operation == StatOperationV1::Decrease).then_some(input.value_min),
        maximum: None,
        multiplier: MagnitudeMultiplierV1::Fixed,
    };
    cards_description_matches(definition.description(), effect)
        .then_some((effect, CombatStatPredicateV1::Always))
}

/// The printed `Cards` texts, rebuilt from the record's own numbers: `Cards Damage +2`,
/// `-2 Cards Damage, Min 1`, `-7 Cards Attack, Min 0`.
fn cards_description_matches(description: &str, effect: SupportedEffectV1) -> bool {
    let SupportedEffectV1::ModifyCombatStat {
        side: AffectedSideV1::Both,
        stat,
        operation,
        value,
        minimum,
        maximum: None,
        multiplier: MagnitudeMultiplierV1::Fixed,
    } = effect
    else {
        return false;
    };
    let stat = match stat {
        CombatStatV1::Attack => "Attack",
        CombatStatV1::Damage => "Damage",
        CombatStatV1::Power | CombatStatV1::PowerAndDamage => return false,
    };
    match (operation, minimum) {
        (StatOperationV1::Increase, None) => description == format!("Cards {stat} +{value}"),
        (StatOperationV1::Decrease, Some(min)) => {
            description == format!("-{value} Cards {stat}, Min {min}")
        }
        _ => false,
    }
}

/// Recognize the stat Copies and Exchanges under a condition prefix whose predicate the
/// projection already resolves: `Courage:`, `Reprisal:`, `Confidence:`, `Revenge:`,
/// `Asymmetry:`, `Symmetry:` and `Unison :`. The record is the unconditional Copy or
/// Exchange with exactly one condition field set - or, for Unison, the clan-mates link - so
/// the overwrite in the Copy phase simply does not happen when the predicate fails. The
/// printed body must be one of the unconditional grammars' exact texts. Card abilities only.
pub(crate) fn classify_conditional_stat_copy(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    const WON_PREVIOUS: &[(PreviousRoundRequirementV1, IndexRequirementV1)] =
        &[(PreviousRoundRequirementV1::Win, IndexRequirementV1::Any)];
    const LOST_PREVIOUS: &[(PreviousRoundRequirementV1, IndexRequirementV1)] =
        &[(PreviousRoundRequirementV1::Lose, IndexRequirementV1::Any)];
    const SLOTS_DIFFER: &[(PreviousRoundRequirementV1, IndexRequirementV1)] = &[(
        PreviousRoundRequirementV1::Any,
        IndexRequirementV1::Asymmetry,
    )];
    const SLOTS_MATCH: &[(PreviousRoundRequirementV1, IndexRequirementV1)] = &[(
        PreviousRoundRequirementV1::Any,
        IndexRequirementV1::Symmetry,
    )];
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let input = definition.structured_input();
    if input.attribute_action != AttributeActionV1::Copy
        || input.special_action != SpecialActionV1::None
    {
        return None;
    }
    let (stat, exchange_body, copy_body) = match input.attribute_affected {
        AttributeAffectedV1::Power => (CombatStatV1::Power, "Power Exchange", "Copy: Opp. Power"),
        AttributeAffectedV1::Damage => {
            (CombatStatV1::Damage, "Damage Exchange", "Copy: Opp. Damage")
        }
        AttributeAffectedV1::PowerAndDamage => (
            CombatStatV1::PowerAndDamage,
            "Power And Damage Exchange",
            "Copy: Power And Damage Opp.",
        ),
        _ => return None,
    };
    let (effect, body) = match input.side_affected {
        AffectedSideV1::Both => (
            SupportedEffectV1::ExchangePrintedCombatStat { stat },
            exchange_body,
        ),
        AffectedSideV1::Player => (
            SupportedEffectV1::CopyOpponentPrintedCombatStat { stat },
            copy_body,
        ),
        _ => return None,
    };
    let (predicate, prefix) = if input.is_clanmates_count_linked {
        if !neutral_except_clanmates_count(input)
            || input.value != 0
            || input.value_min != 0
            || input.value_max != 0
        {
            return None;
        }
        (CombatStatPredicateV1::OwnerHandUnison, "Unison : ")
    } else {
        let (predicate, prefix, position, conditions) = match (
            input.position_requirement,
            input.previous_round_requirement,
            input.index_requirement,
        ) {
            (
                PositionRequirementV1::Attacker,
                PreviousRoundRequirementV1::Any,
                IndexRequirementV1::Any,
            ) => (
                CombatStatPredicateV1::OwnerMovesFirst,
                "Courage: ",
                PositionRequirementV1::Attacker,
                UNCONDITIONAL,
            ),
            (
                PositionRequirementV1::Defender,
                PreviousRoundRequirementV1::Any,
                IndexRequirementV1::Any,
            ) => (
                CombatStatPredicateV1::OwnerMovesSecond,
                "Reprisal: ",
                PositionRequirementV1::Defender,
                UNCONDITIONAL,
            ),
            (
                PositionRequirementV1::Both,
                PreviousRoundRequirementV1::Win,
                IndexRequirementV1::Any,
            ) => (
                CombatStatPredicateV1::OwnerWonPreviousRound,
                "Confidence: ",
                PositionRequirementV1::Both,
                WON_PREVIOUS,
            ),
            (
                PositionRequirementV1::Both,
                PreviousRoundRequirementV1::Lose,
                IndexRequirementV1::Any,
            ) => (
                CombatStatPredicateV1::OwnerLostPreviousRound,
                "Revenge: ",
                PositionRequirementV1::Both,
                LOST_PREVIOUS,
            ),
            (
                PositionRequirementV1::Both,
                PreviousRoundRequirementV1::Any,
                IndexRequirementV1::Asymmetry,
            ) => (
                CombatStatPredicateV1::SelectedHandSlotsDiffer,
                "Asymmetry: ",
                PositionRequirementV1::Both,
                SLOTS_DIFFER,
            ),
            (
                PositionRequirementV1::Both,
                PreviousRoundRequirementV1::Any,
                IndexRequirementV1::Symmetry,
            ) => (
                CombatStatPredicateV1::SelectedHandSlotsMatch,
                "Symmetry: ",
                PositionRequirementV1::Both,
                SLOTS_MATCH,
            ),
            _ => return None,
        };
        let shape = PostRoundShapeV1 {
            value: ShapeFieldV1::Exact(0),
            position,
            conditions,
            current_round: CurrentRoundRequirementV1::Any,
            side: input.side_affected,
            attribute: input.attribute_affected,
            action: AttributeActionV1::Copy,
            special: SpecialActionV1::None,
            ..POST_ROUND_SHAPE
        };
        if !shape_matches(input, shape) {
            return None;
        }
        (predicate, prefix)
    };
    (definition.description().strip_prefix(prefix) == Some(body)).then_some((effect, predicate))
}

/// True when any opposing source could stop an ability: an executable `Stop Opp. Ability`
/// in either slot of any card, or a Copy, which could adopt one.
pub(crate) fn opponent_can_stop_an_ability(opponent: &[CombatStatCardPlanV1; HAND_SIZE]) -> bool {
    opponent.iter().any(|card| {
        [card.ability, card.bonus].into_iter().any(|plan| {
            matches!(
                plan,
                CombatStatSourcePlanV1::Execute {
                    effect: CombatStatEffectV1::StopOpponentAbility,
                    ..
                } | CombatStatSourcePlanV1::CopyOpponentSource { .. }
            )
        })
    })
}

/// Recognize `Stop:` over the plain fixed numeric body: the effect fires only when the
/// owner's own ability is stopped by the opposing character. The registry carries the
/// prefix as `isInverted`, which every other grammar requires false, and nothing else
/// differs from the plain record. Card abilities only - no clan bonus prints one.
///
/// Every selected `Stop:` round in the corpus is a round in which it did not fire, and the
/// projection models exactly that: the predicate never holds, and construction refuses a
/// match in which an opposing source could stop the ability (see
/// `validate_stop_triggered_context`).
fn classify_stop_triggered_numeric(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let input = definition.structured_input();
    if !input.is_inverted
        || input.position_requirement != PositionRequirementV1::Both
        || !neutral_except_inverted(input)
    {
        return None;
    }
    let effect = numeric_effect(input, MagnitudeMultiplierV1::Fixed)?;
    let body = definition.description().strip_prefix("Stop: ")?;
    numeric_description_body_matches(body, effect, MagnitudeMultiplierV1::Fixed)
        .then_some((effect, CombatStatPredicateV1::OwnerAbilityStopped))
}

/// The three clan gates over the plain fixed numeric body and over `Stop Opp. Bonus`:
/// `[clan:A][clan:B] X` (`clanRequirement`, the owner's selected card's effective clan),
/// `After [clan:A] : X` (`previousClanRequirement`, the canonical clan of the card the
/// owner played in the previous round) and `Versus [clan:A] : X` (`oppClanRequirement`,
/// any canonical clan in the opposing hand). Exactly one list may be set, every other field
/// must be the neutral unconditional shape, and the printed prefix must be rebuilt exactly
/// from that list, so no existing `neutral_except_*` gate is touched.
fn classify_clan_gated(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    let own = input.clan_requirement.as_slice();
    let opposing = input.opponent_clan_requirement.as_slice();
    let previous = input.previous_clan_requirement.as_slice();
    enum Gate {
        OwnerCard,
        OpposingHand,
        PreviousCard,
    }
    let (ids, gate) = match (own.is_empty(), opposing.is_empty(), previous.is_empty()) {
        (false, true, true) => (own, Gate::OwnerCard),
        (true, false, true) => (opposing, Gate::OpposingHand),
        (true, true, false) => (previous, Gate::PreviousCard),
        _ => return None,
    };
    // Only After is printed on a clan bonus (the Tolvack bonus `5585`).
    if !matches!(gate, Gate::PreviousCard) && source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let set = ClanSetV1::from_ids(ids)?;
    let tags: String = ids.iter().map(|id| format!("[clan:{id}]")).collect();
    let description = definition.description();
    let (predicate, body) = match gate {
        Gate::OwnerCard => (
            CombatStatPredicateV1::OwnerClanIn(set),
            description.strip_prefix(&format!("{tags} "))?,
        ),
        Gate::OpposingHand => (
            CombatStatPredicateV1::OpponentHandHasClan(set),
            description.strip_prefix(&format!("Versus {tags} : "))?,
        ),
        Gate::PreviousCard => (
            CombatStatPredicateV1::OwnerPreviousCardClanIn(set),
            description
                .strip_prefix(&format!("After {tags} : "))
                .or_else(|| description.strip_prefix(&format!("After {tags}: ")))?,
        ),
    };
    // Revision 70: one more already-resolved condition beside the owner-clan gate -
    // `Courage:` in the position field or `Asy. :` in the hand-slot field.
    if matches!(gate, Gate::OwnerCard)
        && (input.position_requirement != PositionRequirementV1::Both
            || input.index_requirement != IndexRequirementV1::Any)
    {
        return classify_clan_gated_compound(input, body, set);
    }
    if input.position_requirement != PositionRequirementV1::Both {
        return None;
    }
    if !neutral_except_clan_gate(input) {
        // The one other shape: a magnitude body under the owner-clan gate.
        return match gate {
            Gate::OwnerCard if source_kind == CombatStatEffectSourceV1::Ability => {
                classify_clan_gated_magnitude(input, body).map(|effect| (effect, predicate))
            }
            Gate::OwnerCard | Gate::OpposingHand | Gate::PreviousCard => None,
        };
    }
    if input.special_action == SpecialActionV1::StopBonus {
        return (!matches!(gate, Gate::OpposingHand)
            && input.value == 0
            && input.value_min == 0
            && input.value_max == 0
            && input.side_affected == AffectedSideV1::Player
            && input.attribute_affected == AttributeAffectedV1::None
            && input.attribute_action == AttributeActionV1::None
            && body == "Stop Opp. Bonus")
            .then_some((SupportedEffectV1::StopOpponentBonus, predicate));
    }
    let effect = numeric_effect(input, MagnitudeMultiplierV1::Fixed)?;
    numeric_description_body_matches(body, effect, MagnitudeMultiplierV1::Fixed)
        .then_some((effect, predicate))
}

/// The plain fixed numeric body under the owner-clan gate and exactly one more condition:
/// Dark Nunavik's `[clan:..] Courage: Power +4` (`4680`, `5299`), the first move, and
/// Hypnos' `[clan:..] Asy. : -3 Opp Dam., Min 1` (`5072`), the differing hand slots. The
/// record must be `neutral_except_clan_gate` once that one field is cleared, so no second
/// condition and no magnitude can ride along; the plan carries both as `OwnerClanInAnd`.
/// Card abilities only - the caller has already refused a Bonus.
fn classify_clan_gated_compound(
    input: &StructuredEffectV1,
    body: &str,
    set: ClanSetV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    let mut plain = input.clone();
    let (conjunct, body) = match (input.position_requirement, input.index_requirement) {
        (PositionRequirementV1::Attacker, IndexRequirementV1::Any) => {
            plain.position_requirement = PositionRequirementV1::Both;
            (
                ClanConjunctV1::OwnerMovesFirst,
                body.strip_prefix("Courage: ")?,
            )
        }
        (PositionRequirementV1::Both, IndexRequirementV1::Asymmetry) => {
            plain.index_requirement = IndexRequirementV1::Any;
            (
                ClanConjunctV1::SelectedHandSlotsDiffer,
                body.strip_prefix("Asy. : ")?,
            )
        }
        _ => return None,
    };
    if !neutral_except_clan_gate(&plain) {
        return None;
    }
    let effect = numeric_effect(&plain, MagnitudeMultiplierV1::Fixed)?;
    numeric_description_body_matches(body, effect, MagnitudeMultiplierV1::Fixed)
        .then_some((effect, CombatStatPredicateV1::OwnerClanInAnd(set, conjunct)))
}

/// A magnitude body under `[clan:A][clan:B]`: `Growth:`/`Degrowth:`, `Equalizer:` and
/// `Brawl:` over the plain numeric text, and `+N Dam./ Life Lost Max. M`. Exactly one
/// magnitude flag may be set, and the record must be `neutral_except_clan_gate` once it is
/// cleared - so no existing gate is loosened and no second condition can ride along.
fn classify_clan_gated_magnitude(
    input: &StructuredEffectV1,
    body: &str,
) -> Option<SupportedEffectV1> {
    let multiplier = match (
        input.is_overdrive,
        input.is_divide,
        input.is_opponent_stars_linked,
        input.is_anti_support,
        input.is_lost_life_linked,
    ) {
        (true, false, false, false, false) => MagnitudeMultiplierV1::Growth,
        (false, true, false, false, false) => MagnitudeMultiplierV1::Degrowth,
        (false, false, true, false, false) => MagnitudeMultiplierV1::OpponentStars,
        (false, false, false, true, false) => MagnitudeMultiplierV1::AntiSupport,
        (false, false, false, false, true) => MagnitudeMultiplierV1::OwnerLifeLost,
        _ => return None,
    };
    let mut neutral = input.clone();
    neutral.is_overdrive = false;
    neutral.is_divide = false;
    neutral.is_opponent_stars_linked = false;
    neutral.is_anti_support = false;
    neutral.is_lost_life_linked = false;
    if !neutral_except_clan_gate(&neutral) {
        return None;
    }
    if multiplier == MagnitudeMultiplierV1::OwnerLifeLost {
        return clan_gated_life_lost_effect(input, body);
    }
    let effect = numeric_effect(input, multiplier)?;
    let matches = match multiplier {
        MagnitudeMultiplierV1::Growth | MagnitudeMultiplierV1::Degrowth => {
            round_scaled_description_matches(body, effect)
        }
        MagnitudeMultiplierV1::OpponentStars => equalizer_description_matches(body, effect),
        MagnitudeMultiplierV1::AntiSupport => brawl_description_matches(body, effect),
        _ => false,
    };
    matches.then_some(effect)
}

/// `+N Dam./ Life Lost Max. M` (`5113`): the owner's Damage rises by N per point of Life
/// lost since the match began, and the final Damage is clamped to M, the `Per Life Left`
/// Max rule. Only the Damage form is printed.
fn clan_gated_life_lost_effect(
    input: &StructuredEffectV1,
    body: &str,
) -> Option<SupportedEffectV1> {
    if input.special_action != SpecialActionV1::None
        || input.value == 0
        || input.value_min != 0
        || input.value_max == 0
        || input.side_affected != AffectedSideV1::Player
        || input.attribute_action != AttributeActionV1::Increase
        || input.attribute_affected != AttributeAffectedV1::Damage
    {
        return None;
    }
    (body == format!("+{} Dam./ Life Lost Max. {}", input.value, input.value_max)).then_some(
        SupportedEffectV1::ModifyCombatStat {
            side: AffectedSideV1::Player,
            stat: CombatStatV1::Damage,
            operation: StatOperationV1::Increase,
            value: input.value,
            minimum: None,
            maximum: Some(input.value_max),
            multiplier: MagnitudeMultiplierV1::OwnerLifeLost,
        },
    )
}

fn neutral_except_clan_gate(input: &StructuredEffectV1) -> bool {
    input.previous_round_requirement == PreviousRoundRequirementV1::Any
        && input.current_round_requirement == CurrentRoundRequirementV1::Any
        && input.index_requirement == IndexRequirementV1::Any
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
        && !input.is_opponent_stars_linked
        && !input.is_clanmates_count_linked
        && !input.is_anti_clanmates_count_linked
        && !input.is_permanent
        && !input.is_immediate_permanent
}

fn neutral_except_inverted(input: &StructuredEffectV1) -> bool {
    input.previous_round_requirement == PreviousRoundRequirementV1::Any
        && input.current_round_requirement == CurrentRoundRequirementV1::Any
        && input.index_requirement == IndexRequirementV1::Any
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
        && input.bet_pillz_link == BetPillzLinkV1::No
        && input.value_condition == 0
        && input.is_inverted
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

/// The `Bet` gate read from the structured record: `betPillzLink` names the comparison and
/// `valueCondition` the threshold, compared with the owner's `pillzUsed` (free pill in,
/// Fury out). Returns the predicate and the exact prefix the text must print.
fn bet_gate(input: &StructuredEffectV1) -> Option<(CombatStatPredicateV1, String)> {
    let threshold = u8::try_from(input.value_condition)
        .ok()
        .filter(|n| *n > 0)?;
    match input.bet_pillz_link {
        BetPillzLinkV1::More => Some((
            CombatStatPredicateV1::OwnerPillzUsedAbove(threshold),
            format!("Bet > {threshold} Pillz: "),
        )),
        BetPillzLinkV1::Less => Some((
            CombatStatPredicateV1::OwnerPillzUsedBelow(threshold),
            format!("Bet < {threshold} Pillz: "),
        )),
        BetPillzLinkV1::No => None,
    }
}

/// `Bet > N Pillz:` and `Bet < N Pillz:` over the plain fixed numeric body, and `Bet > N
/// Pillz:` over `Stop Opp. Bonus`. Like the clan gates this is one orthogonal classifier:
/// the gate is the only structured field that may be set, the printed prefix is rebuilt
/// from it, and no existing `neutral_except_*` gate is touched. Card abilities only - the
/// one clan bonus that prints a Bet gate prints it over Victory Life.
fn classify_bet_gated(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    let input = definition.structured_input();
    let (predicate, prefix) = bet_gate(input)?;
    if !neutral_except_bet_gate(input) || input.position_requirement != PositionRequirementV1::Both
    {
        return None;
    }
    let body = definition.description().strip_prefix(prefix.as_str())?;
    if input.special_action == SpecialActionV1::StopBonus {
        return (matches!(predicate, CombatStatPredicateV1::OwnerPillzUsedAbove(_))
            && input.value == 0
            && input.value_min == 0
            && input.value_max == 0
            && input.side_affected == AffectedSideV1::Player
            && input.attribute_affected == AttributeAffectedV1::None
            && input.attribute_action == AttributeActionV1::None
            && body == "Stop Opp. Bonus")
            .then_some((SupportedEffectV1::StopOpponentBonus, predicate));
    }
    let effect = numeric_effect(input, MagnitudeMultiplierV1::Fixed)?;
    numeric_description_body_matches(body, effect, MagnitudeMultiplierV1::Fixed)
        .then_some((effect, predicate))
}

fn neutral_except_bet_gate(input: &StructuredEffectV1) -> bool {
    input.previous_round_requirement == PreviousRoundRequirementV1::Any
        && input.current_round_requirement == CurrentRoundRequirementV1::Any
        && input.index_requirement == IndexRequirementV1::Any
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
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

/// Recognize the `Brawl:` grammars. Brawl is an *anti-support* magnitude, not a round
/// counter: the effect is multiplied by the number of distinct characters in the opposing
/// hand sharing the opposing selected card's effective clan, which is `Per::Brawl` in the
/// reference and the exact mirror of `Per::Support`.
///
/// It is admitted the way Equalizer is rather than the way Support is. `is_anti_support`
/// stays an unsupported link in the registry compiler, so the effect is built here from the
/// structured input and the text checked through `numeric_description_body_matches`, which
/// carries the alternate printed spellings the registry's own strict expectation does not.
fn classify_brawl_numeric(
    definition: &EffectDefinitionV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    if !input.is_anti_support || !neutral_except_anti_support(input) {
        return None;
    }
    let effect = numeric_effect(input, MagnitudeMultiplierV1::AntiSupport)?;
    brawl_description_matches(definition.description(), effect)
        .then_some((effect, CombatStatPredicateV1::Always))
}

/// `+N Power|Damage Per Life Left Max. M`, `+N Attack Per Life Left` and
/// `-N Opp Att. Per Life Left, Min M`: scaled by the owner's Life at round start.
fn classify_life_left_numeric(
    definition: &EffectDefinitionV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    if !input.is_life_linked || !neutral_except_life_linked(input) {
        return None;
    }
    if input.special_action != SpecialActionV1::None || input.is_support || input.value == 0 {
        return None;
    }
    let (side, operation) = match (input.side_affected, input.attribute_action) {
        (AffectedSideV1::Player, AttributeActionV1::Increase) => {
            (AffectedSideV1::Player, StatOperationV1::Increase)
        }
        (AffectedSideV1::Opponent, AttributeActionV1::Decrease) => {
            (AffectedSideV1::Opponent, StatOperationV1::Decrease)
        }
        _ => return None,
    };
    let stat = match input.attribute_affected {
        AttributeAffectedV1::Attack => CombatStatV1::Attack,
        AttributeAffectedV1::Damage => CombatStatV1::Damage,
        AttributeAffectedV1::Power => CombatStatV1::Power,
        _ => return None,
    };
    let v = input.value;
    let expected = match (side, stat) {
        (AffectedSideV1::Player, CombatStatV1::Power | CombatStatV1::Damage) => {
            if input.value_min != 0 || input.value_max == 0 {
                return None;
            }
            let name = if stat == CombatStatV1::Power {
                "Power"
            } else {
                "Damage"
            };
            format!("+{v} {name} Per Life Left Max. {}", input.value_max)
        }
        (AffectedSideV1::Player, CombatStatV1::Attack) => {
            if input.value_min != 0 || input.value_max != 0 {
                return None;
            }
            format!("+{v} Attack Per Life Left")
        }
        (AffectedSideV1::Opponent, CombatStatV1::Attack) => {
            if input.value_max != 0 {
                return None;
            }
            format!("-{v} Opp Att. Per Life Left, Min {}", input.value_min)
        }
        _ => return None,
    };
    if definition.description() != expected {
        return None;
    }
    Some((
        SupportedEffectV1::ModifyCombatStat {
            side,
            stat,
            operation,
            value: v,
            minimum: (operation == StatOperationV1::Decrease).then_some(input.value_min),
            maximum: (operation == StatOperationV1::Increase && input.value_max != 0)
                .then_some(input.value_max),
            multiplier: MagnitudeMultiplierV1::OwnerLife,
        },
        CombatStatPredicateV1::Always,
    ))
}

/// `+N Atk Per Pillz Left` (and its `Unison :` form), `-N Opp Att. Per Pillz Left, Min M`
/// and `+N Attack Per Pillz Lost`: scaled by the owner's Pillz at round start, before the
/// bet, or by the Pillz spent since the match began.
fn classify_pillz_numeric(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(SupportedEffectV1, CombatStatPredicateV1)> {
    let input = definition.structured_input();
    let (multiplier, unison) = match (
        input.is_pillz_linked,
        input.is_lost_pillz_linked,
        input.is_clanmates_count_linked,
    ) {
        (true, false, false) => (MagnitudeMultiplierV1::OwnerPillz, false),
        (true, false, true) => (MagnitudeMultiplierV1::OwnerPillz, true),
        (false, true, false) => (MagnitudeMultiplierV1::OwnerPillzLost, false),
        _ => return None,
    };
    if !neutral_except_pillz_linked(input) {
        return None;
    }
    if unison && source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    if input.special_action != SpecialActionV1::None || input.is_support || input.value == 0 {
        return None;
    }
    let v = input.value;
    let (side, operation, expected) = match (
        multiplier,
        input.side_affected,
        input.attribute_action,
        input.attribute_affected,
    ) {
        (
            MagnitudeMultiplierV1::OwnerPillz,
            AffectedSideV1::Player,
            AttributeActionV1::Increase,
            AttributeAffectedV1::Attack,
        ) if input.value_min == 0 && input.value_max == 0 => (
            AffectedSideV1::Player,
            StatOperationV1::Increase,
            if unison {
                format!("Unison : +{v} Atk Per Pillz Left")
            } else {
                format!("+{v} Atk Per Pillz Left")
            },
        ),
        (
            MagnitudeMultiplierV1::OwnerPillz,
            AffectedSideV1::Opponent,
            AttributeActionV1::Decrease,
            AttributeAffectedV1::Attack,
        ) if !unison && input.value_max == 0 => (
            AffectedSideV1::Opponent,
            StatOperationV1::Decrease,
            format!("-{v} Opp Att. Per Pillz Left, Min {}", input.value_min),
        ),
        (
            MagnitudeMultiplierV1::OwnerPillzLost,
            AffectedSideV1::Player,
            AttributeActionV1::Increase,
            AttributeAffectedV1::Attack,
        ) if input.value_min == 0 && input.value_max == 0 => (
            AffectedSideV1::Player,
            StatOperationV1::Increase,
            format!("+{v} Attack Per Pillz Lost"),
        ),
        _ => return None,
    };
    if definition.description() != expected {
        return None;
    }
    Some((
        SupportedEffectV1::ModifyCombatStat {
            side,
            stat: CombatStatV1::Attack,
            operation,
            value: v,
            minimum: (operation == StatOperationV1::Decrease).then_some(input.value_min),
            maximum: None,
            multiplier,
        },
        if unison {
            CombatStatPredicateV1::OwnerHandUnison
        } else {
            CombatStatPredicateV1::Always
        },
    ))
}

/// The Pillz-link gate: every flag neutral except exactly one of the two Pillz links, and
/// the clan-mates link, which only the `Unison :` form may carry (checked by the caller).
fn neutral_except_pillz_linked(input: &StructuredEffectV1) -> bool {
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
        && (input.is_pillz_linked != input.is_lost_pillz_linked)
        && !input.is_lost_life_linked
        && !input.is_opponent_stars_linked
        && !input.is_anti_clanmates_count_linked
        && !input.is_permanent
        && !input.is_immediate_permanent
}

fn neutral_except_life_linked(input: &StructuredEffectV1) -> bool {
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
        && input.is_life_linked
        && !input.is_pillz_linked
        && !input.is_lost_life_linked
        && !input.is_lost_pillz_linked
        && !input.is_opponent_stars_linked
        && !input.is_clanmates_count_linked
        && !input.is_anti_clanmates_count_linked
        && !input.is_permanent
        && !input.is_immediate_permanent
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

/// The Brawl gate. It is `neutral_except_equalizer` with the two magnitude flags swapped,
/// and it deliberately keeps every other flag neutral: a clan-gated Brawl (`3666`, `3667`)
/// or a Brawl carrying a position, previous-round or hand-slot condition keeps its existing
/// record rather than riding this grammar.
fn neutral_except_anti_support(input: &StructuredEffectV1) -> bool {
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
        && input.is_anti_support
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

/// Structural half of the Recover boundary: the outcome channel is Lose, or Win with or
/// without the Unison clan-mates link, and `value`/`valueMin` are read as the ratio.
pub(crate) fn has_recover_pillz_shape(definition: &EffectDefinitionV1) -> bool {
    let input = definition.structured_input();
    let shape = PostRoundShapeV1 {
        value_min: ShapeFieldV1::Read,
        attribute: AttributeAffectedV1::Pillz,
        special: SpecialActionV1::RecoverPillz,
        ..POST_ROUND_SHAPE
    };
    [
        PostRoundShapeV1 {
            current_round: CurrentRoundRequirementV1::Lose,
            ..shape
        },
        shape,
        PostRoundShapeV1 {
            clanmates_count: true,
            ..shape
        },
    ]
    .into_iter()
    .any(|shape| shape_matches(input, shape))
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
    /// The Brawl magnitude. Like the stars link it is a per-X flag every other grammar
    /// requires false, so a grammar that does not name it can never admit an anti-support
    /// record.
    pub(crate) anti_support: bool,
    /// The Support magnitude, the owner's own effective-clan character count. The same kind
    /// of per-X flag: only the post-round `Support:` grammars set it.
    pub(crate) support: bool,
    /// The `Growth:`/`Degrowth:` round scaling, carried as `isOverdrive`/`isDivide`. `None`
    /// for every other grammar, which then requires both flags false as before.
    pub(crate) round_scale: Option<RoundScaleV1>,
    /// The `Bet > N Pillz:` gate, carried as `betPillzLink` and `valueCondition`. False for
    /// every other grammar, which then requires both neutral as before.
    pub(crate) bet_gated: bool,
    /// The `Unison:` gate, carried as `isClanmatesCountLinked`. Only `Unison: Killshot: +N
    /// Life`, `Unison : Recover N Pillz Out Of M` and `Unison : +N Pillz And Life` name it;
    /// every other grammar requires the flag false, so none of them can admit a Unison record.
    pub(crate) clanmates_count: bool,
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
    anti_support: false,
    support: false,
    round_scale: None,
    bet_gated: false,
    clanmates_count: false,
};

/// True when `input` is exactly the record `shape` describes. The fields the shape does not
/// name must all be neutral: a clan gate, a bet link, a `valueCondition`, a Support or
/// per-X magnitude the shape does not name, or a permanence flag takes a record out of every
/// admitted post-round grammar, whatever its text says.
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
        && if shape.bet_gated {
            input.bet_pillz_link == BetPillzLinkV1::More && input.value_condition > 0
        } else {
            input.bet_pillz_link == BetPillzLinkV1::No && input.value_condition == 0
        }
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
        && !input.is_inverted
        && input.is_support == shape.support
        && input.is_anti_support == shape.anti_support
        && input.is_overdrive == (shape.round_scale == Some(RoundScaleV1::Growth))
        && input.is_divide == (shape.round_scale == Some(RoundScaleV1::Degrowth))
        && !input.is_life_linked
        && !input.is_pillz_linked
        && !input.is_lost_life_linked
        && !input.is_lost_pillz_linked
        && input.is_clanmates_count_linked == shape.clanmates_count
        && !input.is_anti_clanmates_count_linked
        && !input.is_permanent
        && !input.is_immediate_permanent
}

fn victory_life_shape_matches(input: &StructuredEffectV1) -> bool {
    // The four condition slots fixed Victory Life prints: none, `Confidence :`'s won
    // previous round, `Asymmetry:`'s differing hand slots, and `Courage:`'s first move. A
    // `Revenge:` Life or a clan gate keeps its visible-but-disabled record instead.
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
    ) || courage_shape_matches(input, AttributeAffectedV1::Life)
}

fn victory_pillz_shape_matches(input: &StructuredEffectV1) -> bool {
    // No condition at all, the won previous round `Confidence:` names, or the first move
    // `Courage:` names. A `Revenge:` Pillz keeps its visible-but-disabled record rather than
    // becoming a near-miss hazard.
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
    ) || courage_shape_matches(input, AttributeAffectedV1::Pillz)
}

/// `Courage:` over a fixed own gain: the condition lives in the position field and is never
/// printed beside a previous-round or hand-slot one, so the position is the only field that
/// differs from the plain grammar - the same slot Anita's conversion and the Courage
/// opponent-Life identities carry it in.
fn courage_shape_matches(input: &StructuredEffectV1, attribute: AttributeAffectedV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            position: PositionRequirementV1::Attacker,
            attribute,
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

/// `clanmates` is the `Unison:` gate, carried as `isClanmatesCountLinked`.
fn defeat_life_shape_matches(input: &StructuredEffectV1, minimum: u16, clanmates: bool) -> bool {
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
        && input.is_clanmates_count_linked == clanmates
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

fn brawl_opponent_life_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value_min: ShapeFieldV1::Read,
            side: AffectedSideV1::Opponent,
            action: AttributeActionV1::Decrease,
            anti_support: true,
            ..POST_ROUND_SHAPE
        },
    )
}

fn brawl_opponent_pillz_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value_min: ShapeFieldV1::Read,
            side: AffectedSideV1::Opponent,
            attribute: AttributeAffectedV1::Pillz,
            action: AttributeActionV1::Decrease,
            anti_support: true,
            ..POST_ROUND_SHAPE
        },
    )
}

fn brawl_own_pillz_shape_matches(input: &StructuredEffectV1) -> bool {
    shape_matches(
        input,
        PostRoundShapeV1 {
            value_max: ShapeFieldV1::Read,
            attribute: AttributeAffectedV1::Pillz,
            anti_support: true,
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
            AttributeAffectedV1::Life,
            AttributeActionV1::Increase,
            immediate,
            CurrentRoundRequirementV1::Win,
        )
}

/// Poison and Toxin likewise: an opposing-Life decrease of `value` bounded below by
/// `value_min`, with no cap, distinguished only by `isImmediatePermanent`.
fn permanent_opponent_life_shape_matches(input: &StructuredEffectV1, immediate: bool) -> bool {
    permanent_opponent_life_shape_matches_on(input, immediate, CurrentRoundRequirementV1::Win)
}

fn permanent_opponent_life_shape_matches_on(
    input: &StructuredEffectV1,
    immediate: bool,
    current_round: CurrentRoundRequirementV1,
) -> bool {
    input.value > 0
        && input.value_max == 0
        && permanent_life_neutral_shape_matches(
            input,
            AffectedSideV1::Opponent,
            AttributeAffectedV1::Life,
            AttributeActionV1::Decrease,
            immediate,
            current_round,
        )
}

/// `Consume N, Min M`: Toxin's shape on the opposing Pillz - a decrease of `value` bounded
/// below by `value_min`, no cap, won round, immediate.
fn consume_opponent_pillz_shape_matches(input: &StructuredEffectV1) -> bool {
    input.value > 0
        && input.value_max == 0
        && permanent_life_neutral_shape_matches(
            input,
            AffectedSideV1::Opponent,
            AttributeAffectedV1::Pillz,
            AttributeActionV1::Decrease,
            true,
            CurrentRoundRequirementV1::Win,
        )
}

/// `Combust N, Min M`: Poison's shape on both opposing resources - `life&pillz`, delayed.
fn combust_opponent_life_and_pillz_shape_matches(input: &StructuredEffectV1) -> bool {
    input.value > 0
        && input.value_max == 0
        && permanent_life_neutral_shape_matches(
            input,
            AffectedSideV1::Opponent,
            AttributeAffectedV1::LifeAndPillz,
            AttributeActionV1::Decrease,
            false,
            CurrentRoundRequirementV1::Win,
        )
}

fn permanent_life_neutral_shape_matches(
    input: &StructuredEffectV1,
    side: AffectedSideV1,
    // Which resource the permanent moves. Every permanent before Consume and Combust moved
    // Life only.
    attribute: AttributeAffectedV1,
    action: AttributeActionV1,
    immediate: bool,
    // Which round outcome latches it. Every permanent admitted before semantic revision 41
    // latched on a win, and `Defeat: Poison` is the first that latches on a loss.
    current_round: CurrentRoundRequirementV1,
) -> bool {
    input.value_condition == 0
        && input.position_requirement == PositionRequirementV1::Both
        && permanent_condition(input).is_some()
        && input.current_round_requirement == current_round
        && input.clan_requirement.is_empty()
        && input.opponent_clan_requirement.is_empty()
        && input.previous_clan_requirement.is_empty()
        && input.bet_pillz_link == BetPillzLinkV1::No
        && input.side_affected == side
        && input.attribute_affected == attribute
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
        | CombatStatPredicateV1::SelectedHandSlotsDiffer
        | CombatStatPredicateV1::MatchIsNight
        | CombatStatPredicateV1::MatchIsDay
        | CombatStatPredicateV1::OwnerHandUnison
        | CombatStatPredicateV1::OwnerAbilityStopped
        | CombatStatPredicateV1::OwnerClanIn(_)
        | CombatStatPredicateV1::OwnerPreviousCardClanIn(_)
        | CombatStatPredicateV1::OpponentHandHasClan(_)
        | CombatStatPredicateV1::OwnerPillzUsedAbove(_)
        | CombatStatPredicateV1::OwnerPillzUsedBelow(_)
        | CombatStatPredicateV1::OwnerWonPreviousRoundAtNight
        | CombatStatPredicateV1::OwnerClanInAnd(..) => return false,
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
        | CombatStatPredicateV1::OwnerLostPreviousRound
        | CombatStatPredicateV1::MatchIsNight
        | CombatStatPredicateV1::MatchIsDay
        | CombatStatPredicateV1::OwnerHandUnison
        | CombatStatPredicateV1::OwnerAbilityStopped
        | CombatStatPredicateV1::OwnerClanIn(_)
        | CombatStatPredicateV1::OwnerPreviousCardClanIn(_)
        | CombatStatPredicateV1::OpponentHandHasClan(_)
        | CombatStatPredicateV1::OwnerPillzUsedAbove(_)
        | CombatStatPredicateV1::OwnerPillzUsedBelow(_)
        | CombatStatPredicateV1::OwnerWonPreviousRoundAtNight
        | CombatStatPredicateV1::OwnerClanInAnd(..) => return false,
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
        | CombatStatPredicateV1::SelectedHandSlotsDiffer
        | CombatStatPredicateV1::MatchIsNight
        | CombatStatPredicateV1::MatchIsDay
        | CombatStatPredicateV1::OwnerHandUnison
        | CombatStatPredicateV1::OwnerAbilityStopped
        | CombatStatPredicateV1::OwnerClanIn(_)
        | CombatStatPredicateV1::OwnerPreviousCardClanIn(_)
        | CombatStatPredicateV1::OpponentHandHasClan(_)
        | CombatStatPredicateV1::OwnerPillzUsedAbove(_)
        | CombatStatPredicateV1::OwnerPillzUsedBelow(_)
        | CombatStatPredicateV1::OwnerWonPreviousRoundAtNight
        | CombatStatPredicateV1::OwnerClanInAnd(..) => None,
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
        | SupportedEffectV1::CopyOpponentPrintedCombatStat { .. }
        | SupportedEffectV1::ExchangePrintedCombatStat { .. }
        | SupportedEffectV1::ImposePrintedCombatStat { .. }
        | SupportedEffectV1::CancelOpponentResourceModifiers { .. }
        | SupportedEffectV1::SimplifyAttackToPillz => return false,
    };
    let prefix = match multiplier {
        MagnitudeMultiplierV1::Growth => "Growth: ",
        MagnitudeMultiplierV1::Degrowth => "Degrowth: ",
        MagnitudeMultiplierV1::Fixed
        | MagnitudeMultiplierV1::Support
        | MagnitudeMultiplierV1::AntiSupport
        | MagnitudeMultiplierV1::OpponentStars
        | MagnitudeMultiplierV1::OpponentDamage
        | MagnitudeMultiplierV1::OpponentPower
        | MagnitudeMultiplierV1::OwnerLife
        | MagnitudeMultiplierV1::OwnerPillz
        | MagnitudeMultiplierV1::OwnerPillzLost
        | MagnitudeMultiplierV1::OwnerLifeLost => return false,
    };
    numeric_description_body_matches(
        description.strip_prefix(prefix).unwrap_or(""),
        effect,
        multiplier,
    )
}

fn brawl_description_matches(description: &str, effect: SupportedEffectV1) -> bool {
    numeric_description_body_matches(
        description.strip_prefix("Brawl: ").unwrap_or(""),
        effect,
        MagnitudeMultiplierV1::AntiSupport,
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
            // `1490`, `1703` and `3948` print `Brawl: Damage + 1` with a space either side
            // of the sign, the same abbreviation-style variance Hattori's `304` had. The
            // Power-And-Damage arm below already carries its spaced form.
            body == format!("Damage +{value}") || body == format!("Damage + {value}")
        }
        (AffectedSideV1::Player, CombatStatV1::Attack, StatOperationV1::Increase, None) => {
            // `908` prints `Stop: Atk. +N`, and Dark Majestic's `5606` `Equalizer: Att. +3`.
            body == format!("Attack +{value}")
                || body == format!("Atk. +{value}")
                || body == format!("Att. +{value}")
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
                // Hypnos `5072`: `[clan:..] Asy. : -3 Opp Dam., Min 1`, the only record that
                // prints this spelling.
                || body == format!("-{value} Opp Dam., Min {min}")
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
                // The GhosTown bonus `1442` and Figaro's day ability `1622`.
                || body == format!("-{value} Opp Pow. And Damage, Min {min}")
                || body == format!("-{value} Opp Power & Damage, Min {min}")
                || body == format!("-{value} Opp Pow. & Dam., Min {min}")
                || body == format!("-{value} Opp Pow. And Dam., Min {min}")
                || body == format!("-{value} Opp Pow. & Dmg,min {min}")
                // Pistache `5681`: `After [clan:27][clan:29]: -2 Opp. Pow. & Dam., Min 2`.
                || body == format!("-{value} Opp. Pow. & Dam., Min {min}")
                // Schwarz `1643`: `Night: Confid.: -2 Opp Pow. & Damage, Min 3`.
                || body == format!("-{value} Opp Pow. & Damage, Min {min}")
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
                // Only `classify_cards_numeric` builds this side; the registry never emits it.
                AffectedSideV1::Both => CombatStatAffectedSideV1::Both,
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
                MagnitudeMultiplierV1::AntiSupport => CombatStatMagnitudeV1::AntiSupport,
                MagnitudeMultiplierV1::OpponentDamage => CombatStatMagnitudeV1::OpponentDamage,
                MagnitudeMultiplierV1::OpponentPower => CombatStatMagnitudeV1::OpponentPower,
                MagnitudeMultiplierV1::OwnerLife => CombatStatMagnitudeV1::OwnerLife,
                MagnitudeMultiplierV1::OwnerPillz => CombatStatMagnitudeV1::OwnerPillz,
                MagnitudeMultiplierV1::OwnerPillzLost => CombatStatMagnitudeV1::OwnerPillzLost,
                MagnitudeMultiplierV1::OwnerLifeLost => CombatStatMagnitudeV1::OwnerLifeLost,
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
        SupportedEffectV1::ImposePrintedCombatStat { stat } => {
            Some(CombatStatEffectV1::ImposePrintedCombatStat {
                stat: compact_stat(stat),
            })
        }
        SupportedEffectV1::ExchangePrintedCombatStat { stat } => {
            Some(CombatStatEffectV1::ExchangePrintedCombatStat {
                stat: compact_stat(stat),
            })
        }
        SupportedEffectV1::CancelOpponentResourceModifiers { resources } => {
            Some(CombatStatEffectV1::CancelOpponentResourceModifiers { resources })
        }
        SupportedEffectV1::SimplifyAttackToPillz => Some(CombatStatEffectV1::SimplifyAttackToPillz),
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
    fn clan_gates_are_admitted_by_the_exact_prefix_rebuilt_from_their_list() {
        let registry = registry();
        for id in [4667, 5353, 5909, 5814, 5911, 5912, 2931] {
            let classified = classify_combat_stat_effect(
                registry.get(id).expect("registry definition"),
                CombatStatEffectSourceV1::Ability,
            );
            assert!(
                matches!(classified, Some((_, CombatStatPredicateV1::OwnerClanIn(_)))),
                "{id}: {classified:?}"
            );
        }
        for id in [
            5585, 5681, 5750, 5779, 5780, 5820, 5847, 5853, 5854, 5855, 5738,
        ] {
            let classified = classify_combat_stat_effect(
                registry.get(id).expect("registry definition"),
                CombatStatEffectSourceV1::Ability,
            );
            assert!(
                matches!(
                    classified,
                    Some((_, CombatStatPredicateV1::OwnerPreviousCardClanIn(_)))
                ),
                "{id}: {classified:?}"
            );
        }
        for id in [2461, 3737, 3739] {
            let classified = classify_combat_stat_effect(
                registry.get(id).expect("registry definition"),
                CombatStatEffectSourceV1::Ability,
            );
            assert!(
                matches!(
                    classified,
                    Some((_, CombatStatPredicateV1::OpponentHandHasClan(_)))
                ),
                "{id}: {classified:?}"
            );
            assert_eq!(
                classify_combat_stat_effect(
                    registry.get(id).unwrap(),
                    CombatStatEffectSourceV1::Bonus
                ),
                None
            );
        }
        // The Tolvack bonus is the one clan-gated clan bonus.
        assert!(classify_combat_stat_effect(
            registry.get(5585).unwrap(),
            CombatStatEffectSourceV1::Bonus
        )
        .is_some());
        // A prefix whose tags disagree with the record's list is refused, and so is a record
        // carrying two lists.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        let mut retagged = source.clone();
        let text = retagged["4667"]["description"].as_str().unwrap().to_owned();
        retagged["4667"]["description"] =
            serde_json::json!(text.replacen("[clan:25]", "[clan:26]", 1));
        let retagged = EffectRegistryV1::from_reader(retagged.to_string().as_bytes()).unwrap();
        assert_eq!(
            classify_combat_stat_effect(
                retagged.get(4667).unwrap(),
                CombatStatEffectSourceV1::Ability
            ),
            None
        );
        let mut two_lists = source.clone();
        two_lists["4667"]["abilityData"]["oppClanRequirement"] = serde_json::json!("11");
        if let Ok(two_lists) = EffectRegistryV1::from_reader(two_lists.to_string().as_bytes()) {
            assert_eq!(
                classify_combat_stat_effect(
                    two_lists.get(4667).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None
            );
        }
    }

    #[test]
    fn clan_gated_magnitudes_are_admitted_under_the_owner_clan_gate_only() {
        let registry = registry();
        for (id, expected) in [
            (4672, MagnitudeMultiplierV1::Growth),
            (5603, MagnitudeMultiplierV1::Growth),
            (5604, MagnitudeMultiplierV1::Growth),
            (5619, MagnitudeMultiplierV1::Degrowth),
            (5606, MagnitudeMultiplierV1::OpponentStars),
            (5906, MagnitudeMultiplierV1::OpponentStars),
            (5908, MagnitudeMultiplierV1::OpponentStars),
            (3666, MagnitudeMultiplierV1::AntiSupport),
            (3667, MagnitudeMultiplierV1::AntiSupport),
            (5113, MagnitudeMultiplierV1::OwnerLifeLost),
        ] {
            let definition = registry.get(id).expect("registry definition");
            let classified =
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability);
            assert!(
                matches!(
                    classified,
                    Some((
                        SupportedEffectV1::ModifyCombatStat { multiplier, .. },
                        CombatStatPredicateV1::OwnerClanIn(_),
                    )) if multiplier == expected
                ),
                "{id}: {classified:?}"
            );
            // No clan bonus prints one.
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "{id} as a bonus"
            );
        }
        // `5113` is the one capped increase here, and its Max is the printed one.
        assert!(matches!(
            classify_combat_stat_effect(
                registry.get(5113).unwrap(),
                CombatStatEffectSourceV1::Ability
            ),
            Some((
                SupportedEffectV1::ModifyCombatStat {
                    stat: CombatStatV1::Damage,
                    operation: StatOperationV1::Increase,
                    value: 1,
                    minimum: None,
                    maximum: Some(6),
                    ..
                },
                _,
            ))
        ));

        // Two magnitudes at once, a second condition riding along, a Life Lost record whose
        // numbers or stat disagree with its text, and a magnitude under `After` are refused.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        let after_text = source["5604"]["description"]
            .as_str()
            .unwrap()
            .replacen("[clan:", "After [clan:", 1)
            .replacen("] Growth", "] : Growth", 1);
        for (id, field, value) in [
            ("5604", "isDivide", serde_json::json!(true)),
            ("5604", "isOppStarsLinked", serde_json::json!(true)),
            ("5604", "valueCondition", serde_json::json!(1)),
            ("5113", "valueMax", serde_json::json!(7)),
            ("5113", "attributeAffected", serde_json::json!("pwr")),
            ("5113", "isLifeLinked", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed[id]["abilityData"][field] = value;
            if let Ok(malformed) = EffectRegistryV1::from_reader(malformed.to_string().as_bytes()) {
                assert_eq!(
                    classify_combat_stat_effect(
                        malformed.get(id.parse().unwrap()).unwrap(),
                        CombatStatEffectSourceV1::Ability
                    ),
                    None,
                    "{id} {field}"
                );
            }
        }
        let mut after = source.clone();
        after["5604"]["abilityData"]["previousClanRequirement"] =
            after["5604"]["abilityData"]["clanRequirement"].clone();
        after["5604"]["abilityData"]["clanRequirement"] = serde_json::json!("");
        after["5604"]["description"] = serde_json::json!(after_text);
        if let Ok(after) = EffectRegistryV1::from_reader(after.to_string().as_bytes()) {
            assert_eq!(
                classify_combat_stat_effect(
                    after.get(5604).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None
            );
        }

        // The post-round Equalizer forms are other grammars, and clan-gated Courage carries
        // two conditions (both since revision 70, tested below).
        for id in [5165, 5616] {
            assert_eq!(
                classify_combat_stat_effect(
                    registry.get(id).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "{id}"
            );
        }
        for id in [4680, 5299] {
            assert!(
                matches!(
                    classify_combat_stat_effect(
                        registry.get(id).unwrap(),
                        CombatStatEffectSourceV1::Ability
                    ),
                    Some((
                        _,
                        CombatStatPredicateV1::OwnerClanInAnd(_, ClanConjunctV1::OwnerMovesFirst)
                    ))
                ),
                "{id}"
            );
        }
    }

    #[test]
    fn bet_gates_are_read_from_the_record_and_admitted_by_the_exact_prefix() {
        use CombatStatPredicateV1::{OwnerPillzUsedAbove as Above, OwnerPillzUsedBelow as Below};
        let registry = registry();
        for (id, expected, threshold) in [
            (4657, BetGatedPostRoundEffectV1::GainLife { life: 3 }, 3),
            (4866, BetGatedPostRoundEffectV1::GainLife { life: 3 }, 4),
            (4893, BetGatedPostRoundEffectV1::GainLife { life: 2 }, 6),
            (
                4870,
                BetGatedPostRoundEffectV1::ReduceOpponentLife {
                    life: 5,
                    minimum: 0,
                },
                11,
            ),
            (
                5387,
                BetGatedPostRoundEffectV1::ReduceOpponentLife {
                    life: 2,
                    minimum: 0,
                },
                3,
            ),
            (
                5388,
                BetGatedPostRoundEffectV1::ReduceOpponentLife {
                    life: 3,
                    minimum: 0,
                },
                4,
            ),
            (
                5769,
                BetGatedPostRoundEffectV1::ReduceOpponentLife {
                    life: 2,
                    minimum: 0,
                },
                2,
            ),
            (
                4797,
                BetGatedPostRoundEffectV1::ReduceOpponentPillz {
                    pillz: 2,
                    minimum: 1,
                },
                6,
            ),
        ] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_bet_gated_post_round(definition, CombatStatEffectSourceV1::Ability),
                Some((expected, Above(threshold))),
                "{id}"
            );
            assert!(has_bet_gated_post_round_shape(definition), "{id}");
            // Post-round work, never a combat-stat modifier.
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                None,
                "{id}"
            );
        }
        // The Zenith bonus is the one clan bonus that prints a gate, over Victory Life only.
        assert!(classify_bet_gated_post_round(
            registry.get(4657).unwrap(),
            CombatStatEffectSourceV1::Bonus
        )
        .is_some());
        assert_eq!(
            classify_bet_gated_post_round(
                registry.get(5387).unwrap(),
                CombatStatEffectSourceV1::Bonus
            ),
            None
        );
        // The combat-stat bodies and `Stop Opp. Bonus`, card abilities only.
        for (id, predicate) in [
            (5189, Above(5)),
            (5190, Above(5)),
            (5401, Above(5)),
            (4807, Below(6)),
            (4808, Below(6)),
            (4860, Above(6)),
            (4949, Above(4)),
        ] {
            let definition = registry.get(id).expect("registry definition");
            assert!(
                matches!(
                    classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                    Some((_, actual)) if actual == predicate
                ),
                "{id}"
            );
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "{id} as a bonus"
            );
        }
        // The gated Copy adopts only when its gate holds.
        assert_eq!(
            classify_copy_opponent_source(registry.get(5304).unwrap()),
            Some((CopiedSourceKindV1::Ability, Above(3)))
        );

        // The threshold and direction are read from the record and must agree with the
        // printed prefix; `Bet <` is printed over no post-round body, Stop or Copy.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (id, field, value) in [
            ("4657", "valueCondition", serde_json::json!(4)),
            ("4657", "betPillzLink", serde_json::json!("less")),
            ("4657", "betPillzLink", serde_json::json!("no")),
            ("5387", "valueCondition", serde_json::json!(0)),
            ("5401", "valueCondition", serde_json::json!(6)),
            ("4808", "betPillzLink", serde_json::json!("more")),
            ("4860", "betPillzLink", serde_json::json!("less")),
            ("4860", "valueCondition", serde_json::json!(5)),
            ("5304", "valueCondition", serde_json::json!(4)),
            ("5304", "betPillzLink", serde_json::json!("less")),
        ] {
            let mut malformed = source.clone();
            malformed[id]["abilityData"][field] = value;
            let Ok(malformed) = EffectRegistryV1::from_reader(malformed.to_string().as_bytes())
            else {
                continue;
            };
            let definition = malformed.get(id.parse().unwrap()).unwrap();
            assert_eq!(
                classify_bet_gated_post_round(definition, CombatStatEffectSourceV1::Ability),
                None,
                "{id} {field}"
            );
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                None,
                "{id} {field}"
            );
            assert_eq!(
                classify_copy_opponent_source(definition),
                None,
                "{id} {field}"
            );
        }
    }

    #[test]
    fn players_pillz_is_admitted_and_players_life_stays_closed() {
        let registry = registry();
        let pillz = registry.get(5511).expect("registry definition");
        assert_eq!(
            classify_victory_or_defeat_both_players_gain(pillz, CombatStatEffectSourceV1::Ability),
            Some((BothPlayersGainV1::Pillz, 3))
        );
        assert_eq!(
            classify_victory_or_defeat_both_players_gain(pillz, CombatStatEffectSourceV1::Bonus),
            None
        );
        assert_eq!(
            classify_combat_stat_effect(pillz, CombatStatEffectSourceV1::Ability),
            None
        );
        // The Life form keeps its shape, so replay can still call it a selected hazard, but
        // is not admitted: no round shows it meeting a knockout.
        for id in [3187, 5321] {
            let life = registry.get(id).expect("registry definition");
            assert!(has_victory_or_defeat_both_players_gain_shape(life), "{id}");
            assert_eq!(
                classify_victory_or_defeat_both_players_gain(
                    life,
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "{id}"
            );
        }
        // The printed number is authority, and the Pillz record carries no `valueMin`.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (field, value) in [
            ("value", serde_json::json!(2)),
            ("valueMin", serde_json::json!(1)),
            ("sideAffected", serde_json::json!("player")),
            ("currentRoundRequirement", serde_json::json!("win")),
        ] {
            let mut malformed = source.clone();
            malformed["5511"]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            assert_eq!(
                classify_victory_or_defeat_both_players_gain(
                    malformed.get(5511).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "{field}"
            );
        }
    }

    #[test]
    fn victory_or_defeat_gains_and_the_opposing_compound_are_admitted_by_text() {
        let registry = registry();
        let ability = CombatStatEffectSourceV1::Ability;
        for (id, amount, minimum) in [
            (1721, 1, 0),
            (5355, 1, 0),
            (2720, 2, 5),
            (2721, 2, 4),
            (5893, 2, 4),
            (2897, 2, 1),
        ] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_victory_opponent_pillz_and_life(definition, ability),
                Some((amount, minimum)),
                "{id}"
            );
            assert_eq!(
                classify_victory_opponent_pillz_and_life(
                    definition,
                    CombatStatEffectSourceV1::Bonus
                ),
                None
            );
        }
        let harston = registry.get(3012).expect("registry definition");
        assert_eq!(
            classify_victory_or_defeat_pillz_amount(harston, ability),
            Some(2)
        );
        for id in [2007, 5071] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_victory_or_defeat_life_per_damage(definition, ability),
                Some(1),
                "{id}"
            );
        }
        // The one-Pillz text stays the reviewed identity set, and the prefixed compounds
        // (`Revenge:`, `Killshot:`) are other grammars.
        for id in [1034, 1652, 5775, 5776] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_victory_or_defeat_pillz_amount(definition, ability),
                None
            );
            assert_eq!(
                classify_victory_opponent_pillz_and_life(definition, ability),
                None,
                "{id}"
            );
        }
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (id, field, value) in [
            ("2721", "value", serde_json::json!(3)),
            ("2721", "valueMin", serde_json::json!(5)),
            ("2721", "currentRoundRequirement", serde_json::json!("any")),
            ("3012", "value", serde_json::json!(3)),
            ("3012", "currentRoundRequirement", serde_json::json!("win")),
            ("2007", "value", serde_json::json!(2)),
            ("2007", "specialAction", serde_json::json!("none")),
        ] {
            let mut malformed = source.clone();
            malformed[id]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            let definition = malformed.get(id.parse().unwrap()).unwrap();
            assert_eq!(
                classify_victory_opponent_pillz_and_life(definition, ability),
                None,
                "{id} {field} = {value}"
            );
            assert_eq!(
                classify_victory_or_defeat_pillz_amount(definition, ability),
                None,
                "{id} {field} = {value}"
            );
            assert_eq!(
                classify_victory_or_defeat_life_per_damage(definition, ability),
                None,
                "{id} {field} = {value}"
            );
        }
    }

    #[test]
    fn unison_defeat_life_and_the_unison_compound_are_their_own_grammars() {
        let registry = registry();
        let ability = CombatStatEffectSourceV1::Ability;
        for (id, life) in [(4015, 2), (5312, 3)] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(classify_unison_defeat_life(definition, ability), Some(life));
            assert_eq!(
                classify_unison_defeat_life(definition, CombatStatEffectSourceV1::Bonus),
                None
            );
            assert_eq!(classify_defeat_life(definition, ability), None);
            assert_eq!(classify_unison_pillz_and_life(definition, ability), None);
        }
        let korakine = registry.get(3973).expect("registry definition");
        assert_eq!(classify_unison_pillz_and_life(korakine, ability), Some(2));
        assert_eq!(
            classify_unison_pillz_and_life(korakine, CombatStatEffectSourceV1::Bonus),
            None
        );
        assert_eq!(classify_unison_defeat_life(korakine, ability), None);
        // The plain forms are other grammars.
        for id in [862, 1714, 1716] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_unison_defeat_life(definition, ability),
                None,
                "{id}"
            );
            assert_eq!(
                classify_unison_pillz_and_life(definition, ability),
                None,
                "{id}"
            );
        }
        // The printed numbers and the gate are authority.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (id, field, value) in [
            ("4015", "value", serde_json::json!(3)),
            ("4015", "isClanmatesCountLinked", serde_json::json!(false)),
            ("4015", "currentRoundRequirement", serde_json::json!("win")),
            ("3973", "value", serde_json::json!(3)),
            ("3973", "valueMin", serde_json::json!(0)),
            ("3973", "isClanmatesCountLinked", serde_json::json!(false)),
            ("3973", "currentRoundRequirement", serde_json::json!("lose")),
        ] {
            let mut malformed = source.clone();
            malformed[id]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            let definition = malformed.get(id.parse().unwrap()).unwrap();
            assert_eq!(
                classify_unison_defeat_life(definition, ability),
                None,
                "{id} {field} = {value}"
            );
            assert_eq!(
                classify_unison_pillz_and_life(definition, ability),
                None,
                "{id} {field} = {value}"
            );
        }
    }

    #[test]
    fn dope_is_admitted_on_either_latch_as_an_ability_and_support_dope_stays_closed() {
        let registry = registry();
        for (id, pillz, maximum, latch) in [
            (1451, 1, 11, DopeLatchV1::Victory),
            (4931, 3, 4, DopeLatchV1::Victory),
            (4932, 3, 4, DopeLatchV1::Victory),
            (5888, 1, 10, DopeLatchV1::Victory),
            (1507, 1, 13, DopeLatchV1::Defeat),
        ] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_dope_pillz(definition, CombatStatEffectSourceV1::Ability),
                Some((pillz, maximum, latch)),
                "{id}"
            );
            assert_eq!(
                classify_dope_pillz(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "{id}"
            );
            assert!(has_dope_pillz_shape(definition), "{id}");
            // A permanent is post-round work, never a combat-stat effect.
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                None,
                "{id}"
            );
        }
        // `Support: Dope` scales by the clan count, and Regen is the Life twin.
        for id in [2000, 1458, 3433] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_dope_pillz(definition, CombatStatEffectSourceV1::Ability),
                None,
                "{id}"
            );
        }
        // The printed numbers, the latch and the immediacy are all authority.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (id, field, value) in [
            ("1451", "value", serde_json::json!(2)),
            ("1451", "valueMax", serde_json::json!(12)),
            ("1451", "valueMin", serde_json::json!(1)),
            ("1451", "isImmediatePermanent", serde_json::json!(false)),
            ("1451", "currentRoundRequirement", serde_json::json!("lose")),
            ("1451", "previousRoundRequirement", serde_json::json!("win")),
            ("1451", "isSupport", serde_json::json!(true)),
            ("1507", "currentRoundRequirement", serde_json::json!("win")),
            ("4931", "valueMax", serde_json::json!(3)),
        ] {
            let mut malformed = source.clone();
            malformed[id]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            let definition = malformed.get(id.parse().unwrap()).unwrap();
            assert_eq!(
                classify_dope_pillz(definition, CombatStatEffectSourceV1::Ability),
                None,
                "{id} {field} = {value}"
            );
        }
    }

    #[test]
    fn consume_and_combust_are_admitted_as_plain_pillz_permanents() {
        let registry = registry();
        for (id, amount, minimum) in [(5871, 1, 2), (5873, 1, 2)] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_consume_opponent_pillz_on_victory(
                    definition,
                    CombatStatEffectSourceV1::Ability
                ),
                Some((amount, minimum, CombatStatPredicateV1::Always)),
                "{id}"
            );
            assert_eq!(
                classify_consume_opponent_pillz_on_victory(
                    definition,
                    CombatStatEffectSourceV1::Bonus
                ),
                None
            );
        }
        for (id, amount, minimum) in [(4799, 1, 1), (5683, 1, 2), (5684, 1, 0)] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_combust_opponent_life_and_pillz_on_victory(
                    definition,
                    CombatStatEffectSourceV1::Ability
                ),
                Some((amount, minimum, CombatStatPredicateV1::Always)),
                "{id}"
            );
            assert!(has_combust_opponent_life_and_pillz_on_victory_shape(
                definition
            ));
        }
        // The Unison and clan-gated Consume records and the other Pillz permanents are other
        // grammars.
        for id in [4695, 5275, 1451, 3796, 2582, 5286] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_consume_opponent_pillz_on_victory(
                    definition,
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "{id}"
            );
            assert_eq!(
                classify_combust_opponent_life_and_pillz_on_victory(
                    definition,
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "{id}"
            );
        }
        // The printed numbers are authority.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (id, field, value) in [
            ("5871", "value", serde_json::json!(2)),
            ("5871", "valueMin", serde_json::json!(3)),
            ("5683", "value", serde_json::json!(2)),
            ("5683", "valueMin", serde_json::json!(1)),
        ] {
            let mut malformed = source.clone();
            malformed[id]["abilityData"][field] = value;
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            let definition = malformed.get(id.parse().unwrap()).unwrap();
            assert_eq!(
                classify_consume_opponent_pillz_on_victory(
                    definition,
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "{id} {field}"
            );
            assert_eq!(
                classify_combust_opponent_life_and_pillz_on_victory(
                    definition,
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "{id} {field}"
            );
        }
    }

    #[test]
    fn round_scaled_post_round_grammars_are_admitted_by_exact_text_and_shape() {
        let registry = registry();
        for (id, scale) in [
            (1730, RoundScaleV1::Growth),
            (1332, RoundScaleV1::Growth),
            (1419, RoundScaleV1::Growth),
            (4551, RoundScaleV1::Growth),
            (5144, RoundScaleV1::Growth),
            (1116, RoundScaleV1::Growth),
            (2590, RoundScaleV1::Growth),
            (1603, RoundScaleV1::Degrowth),
            (2169, RoundScaleV1::Degrowth),
        ] {
            let definition = registry.get(id).expect("registry definition");
            assert!(
                matches!(
                    classify_round_scaled_post_round(definition, CombatStatEffectSourceV1::Ability),
                    Some((actual, _)) if actual == scale
                ),
                "definition {id}",
            );
            assert!(has_round_scaled_post_round_shape(definition), "{id}");
            assert_eq!(
                classify_round_scaled_post_round(definition, CombatStatEffectSourceV1::Bonus),
                None
            );
        }
        // The permanent Growth Poison and the combat-stat Growth forms are other grammars,
        // and the plain Victory grammars still refuse the round-scaled flag.
        for id in [1266, 1282, 1676] {
            assert_eq!(
                classify_round_scaled_post_round(
                    registry.get(id).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "{id}"
            );
        }
        assert!(!has_victory_opponent_life_shape(
            registry.get(1730).unwrap()
        ));
        // Both flags at once, or the flag under the other prefix, is refused.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        let mut both = source.clone();
        both["1332"]["abilityData"]["isDivide"] = serde_json::json!(true);
        let both = EffectRegistryV1::from_reader(both.to_string().as_bytes()).unwrap();
        assert_eq!(
            classify_round_scaled_post_round(
                both.get(1332).unwrap(),
                CombatStatEffectSourceV1::Ability
            ),
            None
        );
        let mut swapped = source.clone();
        swapped["1332"]["description"] = serde_json::json!("Degrowth: +1 Life");
        let swapped = EffectRegistryV1::from_reader(swapped.to_string().as_bytes()).unwrap();
        assert_eq!(
            classify_round_scaled_post_round(
                swapped.get(1332).unwrap(),
                CombatStatEffectSourceV1::Ability
            ),
            None
        );
    }

    #[test]
    fn defeat_pillz_gains_are_admitted_by_exact_text_and_shape() {
        let registry = registry();
        for (id, pillz) in [(2221, 2), (2222, 3), (3313, 2)] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_defeat_pillz(definition, CombatStatEffectSourceV1::Ability),
                Some(pillz),
                "definition {id}"
            );
            assert_eq!(
                classify_defeat_pillz(definition, CombatStatEffectSourceV1::Bonus),
                None
            );
        }
        let kubra = registry.get(1716).unwrap();
        assert_eq!(
            classify_defeat_pillz_and_life(kubra, CombatStatEffectSourceV1::Ability),
            Some(1)
        );
        // Argos' capped form and the Recover records are other grammars.
        for id in [1158, 729, 1418, 770] {
            let definition = registry.get(id).unwrap();
            assert_eq!(
                classify_defeat_pillz(definition, CombatStatEffectSourceV1::Ability),
                None,
                "{id}"
            );
        }
        // The Kubra shape reads its `valueMin` of 1 exactly, so a Victory compound or a
        // zero-Min record cannot pass for it.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (field, value) in [
            ("valueMin", serde_json::json!(0)),
            ("currentRoundRequirement", serde_json::json!("win")),
            ("valueMax", serde_json::json!(12)),
        ] {
            let mut malformed = source.clone();
            malformed["1716"]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            assert_eq!(
                classify_defeat_pillz_and_life(
                    malformed.get(1716).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "malformed {field} = {value}",
            );
        }
    }

    #[test]
    fn per_pillz_and_life_per_opposing_damage_are_admitted_by_exact_text() {
        let registry = registry();
        for (id, multiplier, predicate) in [
            (
                955,
                MagnitudeMultiplierV1::OwnerPillz,
                CombatStatPredicateV1::Always,
            ),
            (
                1015,
                MagnitudeMultiplierV1::OwnerPillz,
                CombatStatPredicateV1::Always,
            ),
            (
                1425,
                MagnitudeMultiplierV1::OwnerPillz,
                CombatStatPredicateV1::Always,
            ),
            (
                4119,
                MagnitudeMultiplierV1::OwnerPillz,
                CombatStatPredicateV1::OwnerHandUnison,
            ),
            (
                5175,
                MagnitudeMultiplierV1::OwnerPillzLost,
                CombatStatPredicateV1::Always,
            ),
            (
                5305,
                MagnitudeMultiplierV1::OwnerPillzLost,
                CombatStatPredicateV1::Always,
            ),
        ] {
            let classified = classify_combat_stat_effect(
                registry.get(id).expect("registry definition"),
                CombatStatEffectSourceV1::Ability,
            );
            assert!(
                matches!(
                    classified,
                    Some((SupportedEffectV1::ModifyCombatStat { multiplier: actual, .. }, p))
                        if actual == multiplier && p == predicate
                ),
                "definition {id}: {classified:?}",
            );
        }
        let definition = registry.get(3779).unwrap();
        assert_eq!(
            classify_victory_life_per_opponent_damage(
                definition,
                CombatStatEffectSourceV1::Ability
            ),
            Some(1)
        );
        assert_eq!(
            classify_victory_life_per_opponent_damage(definition, CombatStatEffectSourceV1::Bonus),
            None
        );
        assert_eq!(
            classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
            None
        );
        // The Pillz link without its text, and the text over the Life link, are refused.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (field, value) in [
            ("isPillzLinked", serde_json::json!(false)),
            ("isLifeLinked", serde_json::json!(true)),
            ("value", serde_json::json!(2)),
        ] {
            let mut malformed = source.clone();
            malformed["955"]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            assert_eq!(
                classify_combat_stat_effect(
                    malformed.get(955).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "malformed {field} = {value}",
            );
        }
    }

    #[test]
    fn unison_numeric_is_admitted_as_a_whole_hand_gate() {
        let registry = registry();
        for id in [
            3743, 3833, 3841, 3843, 3890, 3900, 4052, 4075, 4553, 5311, 5318,
        ] {
            let definition = registry.get(id).expect("registry definition");
            let classified =
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability);
            assert!(
                matches!(
                    classified,
                    Some((
                        SupportedEffectV1::ModifyCombatStat {
                            multiplier: MagnitudeMultiplierV1::Fixed,
                            ..
                        },
                        CombatStatPredicateV1::OwnerHandUnison,
                    ))
                ),
                "definition {id}: {classified:?}",
            );
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "definition {id} as a bonus",
            );
        }
        // The flag without the text, or the text without the flag, is refused, and so is a
        // Unison over any body this grammar does not name.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        let mut unflagged = source.clone();
        unflagged["5318"]["abilityData"]["isClanmatesCountLinked"] = serde_json::json!(false);
        let unflagged = EffectRegistryV1::from_reader(unflagged.to_string().as_bytes()).unwrap();
        assert!(classify_unison_numeric(
            unflagged.get(5318).unwrap(),
            CombatStatEffectSourceV1::Ability
        )
        .is_none());
        let mut retexted = source.clone();
        let body = retexted["5318"]["description"]
            .as_str()
            .unwrap()
            .replace("Unison : ", "Courage: ");
        retexted["5318"]["description"] = serde_json::json!(body);
        let retexted = EffectRegistryV1::from_reader(retexted.to_string().as_bytes()).unwrap();
        assert_eq!(
            classify_combat_stat_effect(
                retexted.get(5318).unwrap(),
                CombatStatEffectSourceV1::Ability
            ),
            None
        );
        // `3953`, the Unison Damage Exchange, is the conditional stat-Copy grammar's since
        // revision 50, `4119`, the Unison Pillz-Left Attack, the Pillz magnitude's since
        // revision 51, and `3839`, the Unison Stop, the conditional Stop's since revision 68.
        for id in [3973, 4015, 4033, 4695] {
            assert_eq!(
                classify_combat_stat_effect(
                    registry.get(id).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "definition {id}",
            );
        }
    }

    #[test]
    fn per_life_left_is_admitted_by_exact_text_over_the_life_linked_shape() {
        let registry = registry();
        for id in [
            923, 1717, 1788, 2710, 3276, 4302, 4517, 4518, 4519, 4829, 5357, 5555, 5848,
        ] {
            let definition = registry.get(id).expect("registry definition");
            let classified =
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability);
            assert!(
                matches!(
                    classified,
                    Some((
                        SupportedEffectV1::ModifyCombatStat {
                            multiplier: MagnitudeMultiplierV1::OwnerLife,
                            ..
                        },
                        CombatStatPredicateV1::Always,
                    ))
                ),
                "definition {id}: {classified:?}",
            );
        }
        // Huracan prints `923` as its clan bonus, so the Bonus slot is admitted too.
        assert!(classify_combat_stat_effect(
            registry.get(923).unwrap(),
            CombatStatEffectSourceV1::Bonus
        )
        .is_some());

        // Text and structure must corroborate, and the Max may ride only on this magnitude:
        // the life link dropped, a Min on an increase, a missing Max, or a condition field
        // all refuse, and the ordinary capped `Power +6, Max. 8` stays refused.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (field, value) in [
            ("isLifeLinked", serde_json::json!(false)),
            ("value", serde_json::json!(2)),
            ("valueMax", serde_json::json!(12)),
            ("valueMax", serde_json::json!(0)),
            ("valueMin", serde_json::json!(1)),
            ("positionRequirement", serde_json::json!("attacker")),
            ("isPillzLinked", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed["1788"]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            assert_eq!(
                classify_combat_stat_effect(
                    malformed.get(1788).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "malformed {field} = {value}",
            );
        }
        assert_eq!(
            classify_combat_stat_effect(
                registry.get(2969).unwrap(),
                CombatStatEffectSourceV1::Ability
            ),
            None,
            "an ordinary capped increase is not a Per Life Left grammar",
        );
    }

    #[test]
    fn cards_numeric_is_admitted_on_both_selected_cards_by_exact_text() {
        use crate::effect_registry::{DescriptionContextV1, UnsupportedReasonV1};
        let registry = registry();
        for (id, stat, operation, value, minimum) in [
            (
                3295,
                CombatStatV1::Damage,
                StatOperationV1::Increase,
                2,
                None,
            ),
            (
                4011,
                CombatStatV1::Damage,
                StatOperationV1::Increase,
                2,
                None,
            ),
            (
                5411,
                CombatStatV1::Damage,
                StatOperationV1::Increase,
                2,
                None,
            ),
            (
                2018,
                CombatStatV1::Damage,
                StatOperationV1::Decrease,
                2,
                Some(1),
            ),
            (
                4957,
                CombatStatV1::Damage,
                StatOperationV1::Decrease,
                2,
                Some(1),
            ),
            (
                3570,
                CombatStatV1::Damage,
                StatOperationV1::Decrease,
                2,
                Some(4),
            ),
            (
                4616,
                CombatStatV1::Attack,
                StatOperationV1::Decrease,
                7,
                Some(0),
            ),
        ] {
            let definition = registry.get(id).expect("registry definition");
            // The registry keeps refusing the text as a description context; admission is
            // this projection's, by exact grammar.
            assert!(definition.compiled().unsupported_reasons().contains(
                &UnsupportedReasonV1::DescriptionContext {
                    context: DescriptionContextV1::Cards,
                }
            ));
            let effect = SupportedEffectV1::ModifyCombatStat {
                side: AffectedSideV1::Both,
                stat,
                operation,
                value,
                minimum,
                maximum: None,
                multiplier: MagnitudeMultiplierV1::Fixed,
            };
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                Some((effect, CombatStatPredicateV1::Always)),
                "definition {id}",
            );
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "definition {id} from a Bonus",
            );
            assert!(matches!(
                compact_effect(effect),
                Some(CombatStatEffectV1::ModifyCombatStat {
                    side: CombatStatAffectedSideV1::Both,
                    ..
                })
            ));
        }
        // The shape must be the neutral unconditional both-sides record and the text the one
        // its own numbers print.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (id, field, value) in [
            ("2018", "sideAffected", serde_json::json!("opponent")),
            ("2018", "valueMin", serde_json::json!(2)),
            ("2018", "valueMax", serde_json::json!(3)),
            ("2018", "attributeAffected", serde_json::json!("pwr")),
            ("2018", "positionRequirement", serde_json::json!("attacker")),
            ("2018", "previousRoundRequirement", serde_json::json!("win")),
            ("2018", "isSupport", serde_json::json!(true)),
            ("3295", "valueMax", serde_json::json!(8)),
            ("3295", "value", serde_json::json!(3)),
            ("3295", "isOverdrive", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed[id]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            let definition = malformed.get(id.parse().unwrap()).unwrap();
            assert_eq!(
                classify_cards_numeric(definition, CombatStatEffectSourceV1::Ability),
                None,
                "malformed {id} {field} = {value}",
            );
        }
        let mut retexted = source.clone();
        retexted["2018"]["description"] = serde_json::json!("-2 Cards Damage, Min 2");
        let retexted = EffectRegistryV1::from_reader(retexted.to_string().as_bytes()).unwrap();
        assert_eq!(
            classify_cards_numeric(
                retexted.get(2018).unwrap(),
                CombatStatEffectSourceV1::Ability
            ),
            None
        );
    }

    #[test]
    fn tune_out_is_admitted_from_the_clan_bonus_slot_only() {
        let registry = registry();
        let tune_out = registry.get(3496).expect("registry definition");
        assert_eq!(
            classify_combat_stat_effect(tune_out, CombatStatEffectSourceV1::Bonus),
            Some((
                SupportedEffectV1::SimplifyAttackToPillz,
                CombatStatPredicateV1::Always
            ))
        );
        // Noon Steevens prints it as an ability, which no round has shown.
        assert_eq!(
            classify_combat_stat_effect(tune_out, CombatStatEffectSourceV1::Ability),
            None
        );
        assert_eq!(
            compact_effect(SupportedEffectV1::SimplifyAttackToPillz),
            Some(CombatStatEffectV1::SimplifyAttackToPillz)
        );
    }

    /// The captured registry source, parsed once for the revision-70 tests.
    fn abilities_source() -> &'static serde_json::Value {
        static SOURCE: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
        SOURCE.get_or_init(|| {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
            serde_json::from_reader(File::open(&path).unwrap()).unwrap()
        })
    }

    /// The registry with one abilityData field of one record replaced, for the malformed-shape
    /// halves of the revision-70 tests.
    fn with_field(id: &str, field: &str, value: serde_json::Value) -> EffectRegistryV1 {
        let mut source = abilities_source().clone();
        source[id]["abilityData"][field] = value;
        EffectRegistryV1::from_reader(source.to_string().as_bytes()).unwrap()
    }

    /// The registry with one record's printed text replaced.
    fn with_text(id: &str, text: &str) -> EffectRegistryV1 {
        let mut source = abilities_source().clone();
        source[id]["description"] = serde_json::json!(text);
        EffectRegistryV1::from_reader(source.to_string().as_bytes()).unwrap()
    }

    fn clan_set(ids: &[u32]) -> ClanSetV1 {
        ClanSetV1::from_ids(ids).unwrap()
    }

    /// Revision 70: the plain fixed numeric body under the owner-clan gate and one more
    /// condition - Dark Nunavik's `Courage:` (`4680`, `5299`) and Hypnos' `Asy. :` (`5072`,
    /// with its `Opp Dam.` spelling) - carried as `OwnerClanInAnd`. Card abilities only, and
    /// the record must be the gated plain one once the one condition field is cleared.
    #[test]
    fn clan_gated_courage_and_asymmetry_numerics_carry_the_compound_predicate() {
        let registry = registry();
        let nunavik = clan_set(&[38, 25, 47, 54, 59]);
        let hypnos = clan_set(&[46, 58, 40, 55, 42, 50]);
        for (id, expected) in [
            (
                4680,
                (
                    SupportedEffectV1::ModifyCombatStat {
                        side: AffectedSideV1::Player,
                        stat: CombatStatV1::Power,
                        operation: StatOperationV1::Increase,
                        value: 4,
                        minimum: None,
                        maximum: None,
                        multiplier: MagnitudeMultiplierV1::Fixed,
                    },
                    CombatStatPredicateV1::OwnerClanInAnd(nunavik, ClanConjunctV1::OwnerMovesFirst),
                ),
            ),
            (
                5299,
                (
                    SupportedEffectV1::ModifyCombatStat {
                        side: AffectedSideV1::Player,
                        stat: CombatStatV1::Power,
                        operation: StatOperationV1::Increase,
                        value: 4,
                        minimum: None,
                        maximum: None,
                        multiplier: MagnitudeMultiplierV1::Fixed,
                    },
                    CombatStatPredicateV1::OwnerClanInAnd(nunavik, ClanConjunctV1::OwnerMovesFirst),
                ),
            ),
            (
                5072,
                (
                    SupportedEffectV1::ModifyCombatStat {
                        side: AffectedSideV1::Opponent,
                        stat: CombatStatV1::Damage,
                        operation: StatOperationV1::Decrease,
                        value: 3,
                        minimum: Some(1),
                        maximum: None,
                        multiplier: MagnitudeMultiplierV1::Fixed,
                    },
                    CombatStatPredicateV1::OwnerClanInAnd(
                        hypnos,
                        ClanConjunctV1::SelectedHandSlotsDiffer,
                    ),
                ),
            ),
        ] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                Some(expected),
                "{id}"
            );
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "{id} as a bonus"
            );
        }
        // A second condition, a magnitude, a different position or no position at all, and
        // numbers the text disagrees with are all refused.
        for (id, field, value) in [
            ("4680", "positionRequirement", serde_json::json!("defender")),
            ("4680", "positionRequirement", serde_json::json!("both")),
            ("4680", "indexRequirement", serde_json::json!("asymmetry")),
            ("4680", "previousRoundRequirement", serde_json::json!("win")),
            ("4680", "currentRoundRequirement", serde_json::json!("win")),
            ("4680", "isOverdrive", serde_json::json!(true)),
            ("4680", "value", serde_json::json!(5)),
            ("4680", "clanRequirement", serde_json::json!("38,25,47,54")),
            ("5072", "indexRequirement", serde_json::json!("any")),
            ("5072", "indexRequirement", serde_json::json!("symmetry")),
            ("5072", "positionRequirement", serde_json::json!("attacker")),
            ("5072", "valueMin", serde_json::json!(2)),
            ("5072", "isOppStarsLinked", serde_json::json!(true)),
        ] {
            let malformed = with_field(id, field, value.clone());
            assert_eq!(
                classify_combat_stat_effect(
                    malformed.get(id.parse().unwrap()).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "{id} {field} = {value}"
            );
        }
        for (id, text) in [
            ("4680", "[clan:38][clan:25][clan:47][clan:54][clan:59] Power +4"),
            (
                "4680",
                "[clan:38][clan:25][clan:47][clan:54][clan:59] Courage : Power +4",
            ),
            ("4680", "[clan:38][clan:25][clan:47][clan:54] Courage: Power +4"),
            ("4680", "Courage: Power +4"),
            (
                "5072",
                "[clan:46][clan:58][clan:40][clan:55][clan:42][clan:50] Asymmetry: -3 Opp Dam., Min 1",
            ),
            (
                "5072",
                "[clan:46][clan:58][clan:40][clan:55][clan:42][clan:50] Asy. : -3 Opp Dam., Min 2",
            ),
        ] {
            let retexted = with_text(id, text);
            assert_eq!(
                classify_combat_stat_effect(
                    retexted.get(id.parse().unwrap()).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "{id} as {text:?}"
            );
        }
    }

    /// Revision 70: Kupanda's `[clan:..] Asymm.: Stop Opp. Ability` (`4999`) is the
    /// `Asymmetry:` Stop under the owner-clan gate, card abilities only, by its printed
    /// `Asymm.:` spelling over the conditional Stop's record once the gate is cleared.
    #[test]
    fn clan_gated_asymmetry_stop_carries_the_compound_predicate() {
        let registry = registry();
        let kupanda = registry.get(4999).expect("registry definition");
        let expected = Some((
            SupportedEffectV1::StopOpponentAbility,
            CombatStatPredicateV1::OwnerClanInAnd(
                clan_set(&[31, 46, 54, 49]),
                ClanConjunctV1::SelectedHandSlotsDiffer,
            ),
        ));
        assert_eq!(
            classify_conditional_stop(kupanda, CombatStatEffectSourceV1::Ability),
            expected
        );
        assert_eq!(
            classify_combat_stat_effect(kupanda, CombatStatEffectSourceV1::Ability),
            expected
        );
        assert_eq!(
            classify_conditional_stop(kupanda, CombatStatEffectSourceV1::Bonus),
            None
        );
        let set = clan_set(&[1]);
        assert!(conditional_stop_predicate_admitted(
            CombatStatPredicateV1::OwnerClanInAnd(set, ClanConjunctV1::SelectedHandSlotsDiffer)
        ));
        for conjunct in [
            ClanConjunctV1::OwnerMovesFirst,
            ClanConjunctV1::OwnerMovesSecond,
        ] {
            assert!(!conditional_stop_predicate_admitted(
                CombatStatPredicateV1::OwnerClanInAnd(set, conjunct)
            ));
        }
        for (field, value) in [
            ("indexRequirement", serde_json::json!("any")),
            ("indexRequirement", serde_json::json!("symmetry")),
            ("positionRequirement", serde_json::json!("attacker")),
            ("previousRoundRequirement", serde_json::json!("lose")),
            ("value", serde_json::json!(1)),
            ("isClanmatesCountLinked", serde_json::json!(true)),
        ] {
            let malformed = with_field("4999", field, value.clone());
            assert_eq!(
                classify_conditional_stop(
                    malformed.get(4999).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "4999 {field} = {value}"
            );
        }
        for text in [
            "[clan:31][clan:46][clan:54][clan:49] Asymmetry: Stop Opp. Ability",
            "[clan:31][clan:46][clan:54][clan:49] Asymm.: Stop Opp. Bonus",
            "[clan:31][clan:46][clan:54] Asymm.: Stop Opp. Ability",
            "Asymm.: Stop Opp. Ability",
        ] {
            let retexted = with_text("4999", text);
            assert_eq!(
                classify_conditional_stop(
                    retexted.get(4999).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "4999 as {text:?}"
            );
        }
    }

    /// Revision 70: Yayoi's `[clan:..] Copy: Opp. Ability` (`4132`) and Hypnos' `[clan:..]
    /// Asy. : Copy: Opp. Ability` (`5073`) adopt only when the copier's effective clan is
    /// listed, the second also only across different slots. The record must be the ungated
    /// Copy's once the gate is cleared.
    #[test]
    fn clan_gated_copies_gate_the_adoption_on_the_copiers_clan() {
        let registry = registry();
        for (id, expected) in [
            (
                4132,
                (
                    CopiedSourceKindV1::Ability,
                    CombatStatPredicateV1::OwnerClanIn(clan_set(&[25, 4, 50, 33])),
                ),
            ),
            (
                5073,
                (
                    CopiedSourceKindV1::Ability,
                    CombatStatPredicateV1::OwnerClanInAnd(
                        clan_set(&[46, 58, 40, 55, 42, 50]),
                        ClanConjunctV1::SelectedHandSlotsDiffer,
                    ),
                ),
            ),
        ] {
            let definition = registry.get(id).expect("registry definition");
            assert!(is_copy_opponent_source_description(
                definition.description()
            ));
            assert_eq!(
                classify_copy_opponent_source(definition),
                Some(expected),
                "{id}"
            );
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                None
            );
        }
        for (id, field, value) in [
            ("4132", "indexRequirement", serde_json::json!("asymmetry")),
            ("4132", "positionRequirement", serde_json::json!("defender")),
            ("4132", "specialAction", serde_json::json!("copy_bonus")),
            ("4132", "clanRequirement", serde_json::json!("25,4,50")),
            ("5073", "indexRequirement", serde_json::json!("any")),
            (
                "5073",
                "previousRoundRequirement",
                serde_json::json!("lose"),
            ),
            ("5073", "isClanmatesCountLinked", serde_json::json!(true)),
        ] {
            let malformed = with_field(id, field, value.clone());
            assert_eq!(
                classify_copy_opponent_source(malformed.get(id.parse().unwrap()).unwrap()),
                None,
                "{id} {field} = {value}"
            );
        }
        for (id, text) in [
            ("4132", "[clan:25][clan:4][clan:50][clan:33] Copy: Opp. Bonus"),
            ("4132", "[clan:25][clan:4][clan:50][clan:33] Copy Opp. Ability"),
            ("4132", "[clan:25][clan:4][clan:50][clan:34] Copy: Opp. Ability"),
            (
                "5073",
                "[clan:46][clan:58][clan:40][clan:55][clan:42][clan:50] Asymmetry: Copy: Opp. Ability",
            ),
        ] {
            let retexted = with_text(id, text);
            assert_eq!(
                classify_copy_opponent_source(retexted.get(id.parse().unwrap()).unwrap()),
                None,
                "{id} as {text:?}"
            );
        }
        // The text-only router needs at least one well-formed tag and exactly one space.
        for text in [
            "[clan:]Copy: Opp. Ability",
            "[clan:25]Copy: Opp. Ability",
            "[clan:x] Copy: Opp. Ability",
            " Copy: Opp. Ability",
        ] {
            assert!(!is_copy_opponent_source_description(text), "{text:?}");
        }
    }

    /// Revision 70: the owner-clan gate over the end-of-round bodies the projection already
    /// executes, each by its one printed text over the ungated grammar's complete shape once
    /// the gate is cleared. Card abilities only; never a combat stat; and the ungated
    /// classifiers still refuse every one of them.
    #[test]
    fn clan_gated_post_round_bodies_are_admitted_by_exact_text_over_the_ungated_shape() {
        let registry = registry();
        let kaizerin = CombatStatPredicateV1::OwnerClanIn(clan_set(&[52, 54, 27, 57, 4]));
        let cases = [
            (
                5392,
                (
                    CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictory {
                        life: 2,
                        minimum: 2,
                    },
                    CombatStatEffectV1::ReduceOpponentLifeOnVictory {
                        life: 2,
                        minimum: 2,
                    },
                    CombatStatPredicateV1::OwnerClanIn(clan_set(&[58, 40, 50, 49, 44])),
                ),
            ),
            (
                4037,
                (
                    CombatStatPostRoundEffectV1::ReduceOpponentPillzOnVictory {
                        pillz: 2,
                        minimum: 2,
                    },
                    CombatStatEffectV1::ReduceOpponentPillzOnVictory {
                        pillz: 2,
                        minimum: 2,
                    },
                    kaizerin,
                ),
            ),
            (
                4038,
                (
                    CombatStatPostRoundEffectV1::ReduceOpponentPillzOnVictory {
                        pillz: 2,
                        minimum: 2,
                    },
                    CombatStatEffectV1::ReduceOpponentPillzOnVictory {
                        pillz: 2,
                        minimum: 2,
                    },
                    kaizerin,
                ),
            ),
            (
                5165,
                (
                    CombatStatPostRoundEffectV1::GainPillzOnVictoryPerOpponentStars { per_star: 1 },
                    CombatStatEffectV1::GainPillzOnVictoryPerOpponentStars { per_star: 1 },
                    CombatStatPredicateV1::OwnerClanIn(clan_set(&[32, 51, 49, 30, 45])),
                ),
            ),
            (
                5616,
                (
                    CombatStatPostRoundEffectV1::GainLifeOnVictoryPerOpponentStars { per_star: 1 },
                    CombatStatEffectV1::GainLifeOnVictoryPerOpponentStars { per_star: 1 },
                    CombatStatPredicateV1::OwnerClanIn(clan_set(&[4, 30, 44, 60, 45])),
                ),
            ),
            (
                5613,
                (
                    CombatStatPostRoundEffectV1::ToxinOpponentLifeOnVictory {
                        life: 1,
                        minimum: 1,
                    },
                    CombatStatEffectV1::ToxinOpponentLifeOnVictory {
                        life: 1,
                        minimum: 1,
                    },
                    CombatStatPredicateV1::OwnerClanIn(clan_set(&[55, 50, 49, 44, 60])),
                ),
            ),
            (
                5275,
                (
                    CombatStatPostRoundEffectV1::ConsumeOpponentPillzOnVictory {
                        pillz: 1,
                        minimum: 4,
                    },
                    CombatStatEffectV1::ConsumeOpponentPillzOnVictory {
                        pillz: 1,
                        minimum: 4,
                    },
                    CombatStatPredicateV1::OwnerClanInAnd(
                        clan_set(&[53, 52, 37, 57, 44]),
                        ClanConjunctV1::OwnerMovesSecond,
                    ),
                ),
            ),
        ];
        for (id, expected) in cases {
            let definition = registry.get(id).expect("registry definition");
            let ability = CombatStatEffectSourceV1::Ability;
            assert_eq!(
                classify_clan_gated_post_round(definition, ability),
                Some(expected),
                "{id}"
            );
            assert!(has_clan_gated_post_round_shape(definition), "{id}");
            assert_eq!(
                classify_clan_gated_post_round(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "{id} as a bonus"
            );
            assert_eq!(
                classify_combat_stat_effect(definition, ability),
                None,
                "{id}"
            );
            // The ungated grammars still see the gate and refuse it.
            assert_eq!(classify_victory_opponent_life(definition, ability), None);
            assert_eq!(classify_victory_opponent_pillz(definition, ability), None);
            assert_eq!(
                classify_equalizer_post_round_gain(definition, ability),
                None
            );
            assert_eq!(
                classify_toxin_opponent_life_on_victory(definition, ability),
                None
            );
            assert_eq!(
                classify_consume_opponent_pillz_on_victory(definition, ability),
                None
            );
        }
        // The shared permanent predicate list is untouched: the gates ride on their own
        // effects only, in the plan validator.
        assert!(!permanent_predicate_admitted(
            CombatStatPredicateV1::OwnerClanIn(clan_set(&[1]))
        ));
        // The closed neighbours stay closed: `2317`'s gated Victory Life, `4673`'s gated
        // Defeat reduction, `5578`'s gated Defeat Heal, and the `Versus` Toxin `5563`.
        for id in [2317, 4673, 5578, 5563] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_clan_gated_post_round(definition, CombatStatEffectSourceV1::Ability),
                None,
                "{id}"
            );
            assert!(!has_clan_gated_post_round_shape(definition), "{id}");
        }
        for (id, field, value) in [
            ("5392", "value", serde_json::json!(3)),
            ("5392", "valueMin", serde_json::json!(1)),
            ("5392", "currentRoundRequirement", serde_json::json!("lose")),
            ("5392", "positionRequirement", serde_json::json!("attacker")),
            ("5392", "isPermanent", serde_json::json!(true)),
            ("5392", "clanRequirement", serde_json::json!("58,40,50,49")),
            ("4037", "valueMin", serde_json::json!(3)),
            ("4037", "currentRoundRequirement", serde_json::json!("lose")),
            ("4037", "indexRequirement", serde_json::json!("asymmetry")),
            ("5165", "isOppStarsLinked", serde_json::json!(false)),
            ("5165", "attributeAffected", serde_json::json!("life")),
            ("5165", "currentRoundRequirement", serde_json::json!("any")),
            ("5616", "isOppStarsLinked", serde_json::json!(false)),
            ("5616", "valueMax", serde_json::json!(5)),
            ("5613", "isImmediatePermanent", serde_json::json!(false)),
            ("5613", "previousRoundRequirement", serde_json::json!("win")),
            ("5275", "positionRequirement", serde_json::json!("both")),
            ("5275", "positionRequirement", serde_json::json!("attacker")),
            ("5275", "isImmediatePermanent", serde_json::json!(false)),
            ("5275", "attributeAffected", serde_json::json!("life")),
            ("5275", "indexRequirement", serde_json::json!("asymmetry")),
        ] {
            let malformed = with_field(id, field, value.clone());
            assert_eq!(
                classify_clan_gated_post_round(
                    malformed.get(id.parse().unwrap()).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "{id} {field} = {value}"
            );
        }
        for (id, text) in [
            (
                "5392",
                "[clan:58][clan:40][clan:50][clan:49][clan:44] -2 Opp. Life Min 2",
            ),
            (
                "5392",
                "[clan:58][clan:40][clan:50][clan:49][clan:44] - 2 Opp. Life Min 3",
            ),
            ("5392", "- 2 Opp. Life Min 2"),
            (
                "4037",
                "[clan:52][clan:54][clan:27][clan:57][clan:4] -2 Opp. Pillz, Min 2",
            ),
            (
                "5165",
                "[clan:32][clan:51][clan:49][clan:30][clan:45] Equalizer: +1 Life",
            ),
            (
                "5616",
                "[clan:4][clan:30][clan:44][clan:60][clan:45] Equalizer : +1 Life",
            ),
            (
                "5613",
                "[clan:55][clan:50][clan:49][clan:44][clan:60] Poison 1, Min 1",
            ),
            (
                "5613",
                "[clan:55][clan:50][clan:49][clan:44][clan:61] Toxin 1, Min 1",
            ),
            (
                "5275",
                "[clan:53][clan:52][clan:37][clan:57][clan:44] Reprisal: Consume 1, Min 4",
            ),
            (
                "5275",
                "[clan:53][clan:52][clan:37][clan:57][clan:44] Consume 1, Min 4",
            ),
        ] {
            let retexted = with_text(id, text);
            assert_eq!(
                classify_clan_gated_post_round(
                    retexted.get(id.parse().unwrap()).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "{id} as {text:?}"
            );
        }
    }

    /// Revision 69: the capped Victory Pillz grammar, plain (Mandrak Cr's `1139`) and under
    /// the `Night:` match constant (Nox Ld's night ability `4747`), by exact text over the
    /// unconditional Victory Pillz record with a positive `valueMax`, card abilities only.
    #[test]
    fn capped_victory_pillz_is_admitted_by_text_plain_and_under_night() {
        let registry = registry();
        let ability = CombatStatEffectSourceV1::Ability;
        for (id, expected) in [
            (1139, (3, 9, CombatStatPredicateV1::Always)),
            (4747, (2, 12, CombatStatPredicateV1::MatchIsNight)),
        ] {
            let definition = registry.get(id).expect("registry definition");
            assert!(has_victory_pillz_max_shape(definition), "{id}");
            assert_eq!(
                classify_victory_pillz_max(definition, ability),
                Some(expected),
                "{id}"
            );
            assert_eq!(
                classify_victory_pillz_max(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "{id} as a bonus"
            );
            // Post-round work, never a combat stat, and never the uncapped grammar.
            assert_eq!(classify_combat_stat_effect(definition, ability), None);
            assert_eq!(classify_victory_pillz(definition, ability), None);
        }
        // The Brawl capped gain, Argos' capped Defeat gain and the plain grammar are other
        // grammars, and the capped shape is not theirs.
        for id in [5822, 5844, 1158, 1150, 1451] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_victory_pillz_max(definition, ability),
                None,
                "{id}"
            );
            assert!(!has_victory_pillz_max_shape(definition), "{id}");
        }
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (id, field, value) in [
            ("1139", "value", serde_json::json!(2)),
            ("1139", "valueMax", serde_json::json!(0)),
            ("1139", "valueMax", serde_json::json!(10)),
            ("1139", "valueMin", serde_json::json!(1)),
            ("4747", "currentRoundRequirement", serde_json::json!("lose")),
            ("4747", "previousRoundRequirement", serde_json::json!("win")),
            ("4747", "positionRequirement", serde_json::json!("attacker")),
            ("4747", "isAntiSupport", serde_json::json!(true)),
            ("4747", "isPermanent", serde_json::json!(true)),
            ("4747", "sideAffected", serde_json::json!("opponent")),
        ] {
            let mut malformed = source.clone();
            malformed[id]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            let definition = malformed.get(id.parse().unwrap()).unwrap();
            assert_eq!(
                classify_victory_pillz_max(definition, ability),
                None,
                "{id} {field} = {value}"
            );
        }
        for (id, text) in [
            ("4747", "Day: +2 Pillz Max. 12"),
            ("4747", "Night : +2 Pillz Max. 12"),
            ("4747", "Night: +2 Pillz, Max. 12"),
            ("1139", "+3 Pillz"),
            ("1139", "Confidence: +3 Pillz Max. 9"),
        ] {
            let mut retexted = source.clone();
            retexted[id]["description"] = serde_json::json!(text);
            let retexted = EffectRegistryV1::from_reader(retexted.to_string().as_bytes()).unwrap();
            let definition = retexted.get(id.parse().unwrap()).unwrap();
            assert_eq!(
                classify_victory_pillz_max(definition, ability),
                None,
                "{id} as {text:?}"
            );
            assert_eq!(classify_victory_pillz(definition, ability), None);
        }
    }

    /// Revision 69: Schwarz's night ability `Night: Confid.: -2 Opp Pow. & Damage, Min 3`
    /// is the plain numeric body under both the match constant and a won previous round,
    /// carried as one conjunctive predicate. Card abilities only.
    #[test]
    fn night_confidence_compound_is_admitted_under_its_conjunctive_predicate() {
        let registry = registry();
        let schwarz = registry.get(1643).expect("registry definition");
        let expected = Some((
            SupportedEffectV1::ModifyCombatStat {
                side: AffectedSideV1::Opponent,
                stat: CombatStatV1::PowerAndDamage,
                operation: StatOperationV1::Decrease,
                value: 2,
                minimum: Some(3),
                maximum: None,
                multiplier: MagnitudeMultiplierV1::Fixed,
            },
            CombatStatPredicateV1::OwnerWonPreviousRoundAtNight,
        ));
        assert_eq!(
            classify_combat_stat_effect(schwarz, CombatStatEffectSourceV1::Ability),
            expected
        );
        assert_eq!(
            classify_combat_stat_effect(schwarz, CombatStatEffectSourceV1::Bonus),
            None
        );
        assert!(!conditional_stop_predicate_admitted(
            CombatStatPredicateV1::OwnerWonPreviousRoundAtNight
        ));
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (field, value) in [
            ("previousRoundRequirement", serde_json::json!("any")),
            ("previousRoundRequirement", serde_json::json!("lose")),
            ("positionRequirement", serde_json::json!("attacker")),
            ("indexRequirement", serde_json::json!("symmetry")),
            ("value", serde_json::json!(3)),
            ("valueMin", serde_json::json!(2)),
            ("valueMax", serde_json::json!(3)),
            ("isSupport", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed["1643"]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            assert_eq!(
                classify_combat_stat_effect(
                    malformed.get(1643).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "1643 {field} = {value}"
            );
        }
        for text in [
            "Night: Confidence: -2 Opp Pow. & Damage, Min 3",
            "Confid.: -2 Opp Pow. & Damage, Min 3",
            "Day: Confid.: -2 Opp Pow. & Damage, Min 3",
            "Night: Confid.: -2 Opp Pow. & Damage, Min 4",
        ] {
            let mut retexted = source.clone();
            retexted["1643"]["description"] = serde_json::json!(text);
            let retexted = EffectRegistryV1::from_reader(retexted.to_string().as_bytes()).unwrap();
            assert_eq!(
                classify_combat_stat_effect(
                    retexted.get(1643).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "1643 as {text:?}"
            );
        }
    }

    #[test]
    fn night_and_day_numeric_is_admitted_under_the_match_constant_predicate() {
        let registry = registry();
        for (id, predicate) in [
            (1442, CombatStatPredicateV1::MatchIsNight),
            (1553, CombatStatPredicateV1::MatchIsNight),
            (1623, CombatStatPredicateV1::MatchIsNight),
            (1622, CombatStatPredicateV1::MatchIsDay),
        ] {
            let definition = registry.get(id).expect("registry definition");
            for source in [
                CombatStatEffectSourceV1::Ability,
                CombatStatEffectSourceV1::Bonus,
            ] {
                let classified = classify_combat_stat_effect(definition, source);
                assert!(
                    matches!(
                        classified,
                        Some((
                            SupportedEffectV1::ModifyCombatStat {
                                multiplier: MagnitudeMultiplierV1::Fixed,
                                ..
                            },
                            actual,
                        )) if actual == predicate
                    ),
                    "definition {id} from {source:?}: {classified:?}",
                );
            }
        }
        // `Day: Cancel` is a description context and is not admitted. The Night post-round
        // effects `4747` and `4750` are their post-round grammars' since revision 69, never a
        // combat stat, and the compound `Night: Confid.:` `1643` has its own conjunctive
        // predicate (tested below). The Night Stop `5564` is the conditional-Stop grammar's,
        // since revision 47. `5391`'s stray `valueMax` is admitted by identity since
        // revision 72 (tested below).
        for id in [4747, 4750, 2369] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                None,
                "definition {id}",
            );
        }
        // The prefix is read from the text alone, so the body must still be the complete
        // neutral shape and an exact printed spelling.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (field, value) in [
            ("value", serde_json::json!(2)),
            ("valueMin", serde_json::json!(2)),
            ("positionRequirement", serde_json::json!("attacker")),
            ("previousRoundRequirement", serde_json::json!("win")),
            ("isAntiSupport", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed["1442"]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            assert_eq!(
                classify_day_night_numeric(
                    malformed.get(1442).unwrap(),
                    CombatStatEffectSourceV1::Bonus
                ),
                None,
                "malformed {field} = {value}",
            );
        }
        let mut retexted = source.clone();
        retexted["1442"]["description"] = serde_json::json!("Dusk: -1 Opp Pow. And Damage, Min 1");
        let retexted = EffectRegistryV1::from_reader(retexted.to_string().as_bytes()).unwrap();
        assert_eq!(
            classify_day_night_numeric(
                retexted.get(1442).unwrap(),
                CombatStatEffectSourceV1::Bonus
            ),
            None
        );
    }

    #[test]
    fn post_round_brawl_is_admitted_by_exact_text_and_complete_shape() {
        let registry = registry();
        for (id, expected) in [
            (
                2893,
                BrawlPostRoundEffectV1::ReduceOpponentLife {
                    per_count: 1,
                    minimum: 0,
                },
            ),
            (
                3551,
                BrawlPostRoundEffectV1::ReduceOpponentLife {
                    per_count: 1,
                    minimum: 0,
                },
            ),
            (
                4380,
                BrawlPostRoundEffectV1::ReduceOpponentLife {
                    per_count: 1,
                    minimum: 0,
                },
            ),
            (
                4381,
                BrawlPostRoundEffectV1::ReduceOpponentLife {
                    per_count: 1,
                    minimum: 0,
                },
            ),
            (
                5457,
                BrawlPostRoundEffectV1::ReduceOpponentLife {
                    per_count: 1,
                    minimum: 0,
                },
            ),
            (
                5650,
                BrawlPostRoundEffectV1::ReduceOpponentLife {
                    per_count: 1,
                    minimum: 3,
                },
            ),
            (
                5172,
                BrawlPostRoundEffectV1::ReduceOpponentPillz {
                    per_count: 1,
                    minimum: 1,
                },
            ),
            (
                4583,
                BrawlPostRoundEffectV1::GainPillz {
                    per_count: 1,
                    maximum: 0,
                },
            ),
            (
                5822,
                BrawlPostRoundEffectV1::GainPillz {
                    per_count: 1,
                    maximum: 9,
                },
            ),
            (
                5844,
                BrawlPostRoundEffectV1::GainPillz {
                    per_count: 1,
                    maximum: 9,
                },
            ),
        ] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_brawl_post_round(definition, CombatStatEffectSourceV1::Ability),
                Some(expected),
                "definition {id}",
            );
            assert!(
                has_brawl_post_round_shape(definition),
                "definition {id} shape"
            );
            // No clan bonus prints one, so the Bonus slot is a hazard, not a source.
            assert_eq!(
                classify_brawl_post_round(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "definition {id} as a bonus",
            );
            // Post-round work never doubles as a combat-stat modifier.
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                None,
                "definition {id} must stay out of the combat-stat grammars",
            );
        }

        // The plain Victory grammars never admit an anti-support record: the per-X flag is
        // part of every post-round shape, not a field they ignore.
        for id in [2893, 5172, 4583, 5822] {
            let definition = registry.get(id).unwrap();
            assert!(!has_victory_opponent_life_shape(definition), "{id}");
            assert!(!has_victory_opponent_pillz_shape(definition), "{id}");
            assert!(!has_victory_pillz_shape(definition), "{id}");
        }
        // The combat-stat and clan-gated Brawls are not post-round records.
        for id in [1488, 1490, 5519, 3666, 3667] {
            let definition = registry.get(id).unwrap();
            assert!(!has_brawl_post_round_shape(definition), "{id}");
        }

        // Text and structure must corroborate. A record whose printed numbers disagree with
        // its own, or which differs in any structured field, is refused.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (id, field, value) in [
            ("2893", "value", serde_json::json!(2)),
            ("5650", "valueMin", serde_json::json!(2)),
            ("5822", "valueMax", serde_json::json!(11)),
            ("4583", "valueMax", serde_json::json!(9)),
            ("5172", "valueMin", serde_json::json!(0)),
            ("2893", "isAntiSupport", serde_json::json!(false)),
            ("2893", "currentRoundRequirement", serde_json::json!("lose")),
            ("2893", "positionRequirement", serde_json::json!("attacker")),
            ("5822", "previousRoundRequirement", serde_json::json!("win")),
            ("5172", "isPermanent", serde_json::json!(true)),
            ("4583", "sideAffected", serde_json::json!("opponent")),
        ] {
            let mut malformed = source.clone();
            malformed[id]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            let definition = malformed.get(id.parse().unwrap()).unwrap();
            assert_eq!(
                classify_brawl_post_round(definition, CombatStatEffectSourceV1::Ability),
                None,
                "malformed {id} {field} = {value}",
            );
        }
        // The complete shape under other text is a hazard for replay, not a source.
        let mut retexted = source.clone();
        retexted["2893"]["description"] = serde_json::json!("Brawl: -1 Opp. Life Min 0");
        let retexted = EffectRegistryV1::from_reader(retexted.to_string().as_bytes()).unwrap();
        let definition = retexted.get(2893).unwrap();
        assert_eq!(
            classify_brawl_post_round(definition, CombatStatEffectSourceV1::Ability),
            None
        );
        assert!(has_brawl_post_round_shape(definition));
    }

    #[test]
    fn post_round_support_equalizer_and_courage_are_admitted_by_exact_text_and_shape() {
        use CombatStatEffectSourceV1::{Ability, Bonus};
        let registry = registry();

        // Support: every printed level of each grammar, each read from its own numbers.
        for (id, expected) in [
            (
                827,
                SupportPostRoundEffectV1::ReduceOpponentLife {
                    per_count: 1,
                    minimum: 1,
                },
            ),
            (
                4844,
                SupportPostRoundEffectV1::ReduceOpponentLife {
                    per_count: 1,
                    minimum: 1,
                },
            ),
            (
                1789,
                SupportPostRoundEffectV1::ReduceOpponentLife {
                    per_count: 1,
                    minimum: 0,
                },
            ),
            (
                4937,
                SupportPostRoundEffectV1::ReduceOpponentLife {
                    per_count: 1,
                    minimum: 0,
                },
            ),
            (1221, SupportPostRoundEffectV1::GainLife { per_count: 1 }),
            (1666, SupportPostRoundEffectV1::GainLife { per_count: 1 }),
            (384, SupportPostRoundEffectV1::GainPillz { per_count: 1 }),
        ] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_support_post_round(definition, Ability),
                Some(expected),
                "definition {id}",
            );
            assert!(has_support_post_round_shape(definition), "{id} shape");
            // No clan bonus prints one, so the Bonus slot is a hazard, not a source.
            assert_eq!(classify_support_post_round(definition, Bonus), None, "{id}");
            assert_eq!(
                classify_combat_stat_effect(definition, Ability),
                None,
                "{id}"
            );
            // The plain grammars never admit a Support record: `isSupport` is part of every
            // post-round shape, not a field they ignore.
            assert!(!has_victory_life_shape(definition), "{id}");
            assert!(!has_victory_pillz_shape(definition), "{id}");
            assert!(!has_victory_opponent_life_shape(definition), "{id}");
            assert!(!has_brawl_post_round_shape(definition), "{id}");
        }
        // Dope is a Pillz permanent with a family of its own; combat-stat Support is not
        // post-round work at all.
        for id in [2000, 266, 2535] {
            let definition = registry.get(id).unwrap();
            assert!(!has_support_post_round_shape(definition), "{id}");
            assert_eq!(
                classify_support_post_round(definition, Ability),
                None,
                "{id}"
            );
        }

        // Equalizer: the opponent-Life grammar beyond its two identities, which keep both
        // source slots, and the two own gains.
        let el_cazador = registry.get(5793).unwrap();
        assert_eq!(
            classify_equalizer_opponent_life_on_victory(el_cazador, Ability),
            Some((1, 0))
        );
        assert_eq!(
            classify_equalizer_opponent_life_on_victory(el_cazador, Bonus),
            None
        );
        assert!(has_equalizer_post_round_shape(el_cazador));
        for id in [1415, 4458] {
            let definition = registry.get(id).unwrap();
            for source_kind in [Ability, Bonus] {
                assert_eq!(
                    classify_equalizer_opponent_life_on_victory(definition, source_kind),
                    Some((1, 2)),
                    "{id} {source_kind:?}",
                );
            }
        }
        for (id, expected) in [
            (5199, EqualizerPostRoundGainV1::Life { per_star: 1 }),
            (5582, EqualizerPostRoundGainV1::Pillz { per_star: 1 }),
        ] {
            let definition = registry.get(id).unwrap();
            assert_eq!(
                classify_equalizer_post_round_gain(definition, Ability),
                Some(expected),
                "{id}",
            );
            assert_eq!(classify_equalizer_post_round_gain(definition, Bonus), None);
            assert!(has_equalizer_post_round_shape(definition), "{id} shape");
            assert_eq!(
                classify_combat_stat_effect(definition, Ability),
                None,
                "{id}"
            );
            assert!(!has_victory_life_shape(definition), "{id}");
            assert!(!has_victory_pillz_shape(definition), "{id}");
        }
        // The clan-gated and Victory-or-Defeat Equalizer gains differ in a structured field.
        for id in [5165, 5616, 1679] {
            let definition = registry.get(id).unwrap();
            assert_eq!(
                classify_equalizer_post_round_gain(definition, Ability),
                None
            );
            assert!(!has_equalizer_post_round_shape(definition), "{id}");
        }

        // Courage: the plain Victory gains under the first-move predicate, abilities only.
        let fhtagn = registry.get(5592).unwrap();
        assert_eq!(
            classify_victory_life(fhtagn, Ability),
            Some((5, CombatStatPredicateV1::OwnerMovesFirst))
        );
        assert_eq!(classify_victory_life(fhtagn, Bonus), None);
        let ysmereth = registry.get(5474).unwrap();
        assert_eq!(
            classify_victory_pillz(ysmereth, Ability),
            Some((1, CombatStatPredicateV1::OwnerMovesFirst))
        );
        assert_eq!(classify_victory_pillz(ysmereth, Bonus), None);
        for definition in [fhtagn, ysmereth] {
            assert_eq!(classify_combat_stat_effect(definition, Ability), None);
        }
        // Anita's conversion and the Courage opponent-Life identities keep their own records.
        for id in [274, 843, 3314, 4531] {
            let definition = registry.get(id).unwrap();
            assert!(!has_victory_life_shape(definition), "{id}");
            assert!(!has_victory_pillz_shape(definition), "{id}");
        }

        // Text and structure must corroborate: a record whose printed numbers disagree with
        // its own, or which differs in any structured field, is admitted by no grammar.
        let admitted = |definition: &EffectDefinitionV1| {
            classify_support_post_round(definition, Ability).is_some()
                || classify_equalizer_opponent_life_on_victory(definition, Ability).is_some()
                || classify_equalizer_post_round_gain(definition, Ability).is_some()
                || classify_victory_life(definition, Ability).is_some()
                || classify_victory_pillz(definition, Ability).is_some()
                || classify_victory_opponent_life(definition, Ability).is_some()
        };
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (id, field, value) in [
            ("1789", "valueMin", serde_json::json!(1)),
            ("1789", "isSupport", serde_json::json!(false)),
            ("827", "currentRoundRequirement", serde_json::json!("lose")),
            ("1221", "value", serde_json::json!(2)),
            ("1221", "positionRequirement", serde_json::json!("attacker")),
            ("384", "valueMax", serde_json::json!(7)),
            ("384", "isPermanent", serde_json::json!(true)),
            ("384", "isOppStarsLinked", serde_json::json!(true)),
            ("5793", "valueMin", serde_json::json!(2)),
            ("5793", "isOppStarsLinked", serde_json::json!(false)),
            ("5199", "valueMin", serde_json::json!(1)),
            ("5199", "previousRoundRequirement", serde_json::json!("win")),
            ("5582", "sideAffected", serde_json::json!("opponent")),
            ("5592", "positionRequirement", serde_json::json!("both")),
            ("5592", "previousRoundRequirement", serde_json::json!("win")),
            ("5474", "positionRequirement", serde_json::json!("defender")),
            ("5474", "indexRequirement", serde_json::json!("symmetry")),
        ] {
            let mut malformed = source.clone();
            malformed[id]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            let definition = malformed.get(id.parse().unwrap()).unwrap();
            assert!(!admitted(definition), "malformed {id} {field} = {value}");
        }
        // The complete shape under other text is a hazard for replay, not a source.
        for (id, text) in [
            ("1789", "Support: -1 Opp. Life Min 0"),
            ("384", "Support: +1 Pillz"),
            ("5582", "Equalizer: + 1 Pillz"),
        ] {
            let mut retexted = source.clone();
            retexted[id]["description"] = serde_json::json!(text);
            let retexted = EffectRegistryV1::from_reader(retexted.to_string().as_bytes()).unwrap();
            let definition = retexted.get(id.parse().unwrap()).unwrap();
            assert!(!admitted(definition), "retexted {id}");
            assert!(
                has_support_post_round_shape(definition)
                    || has_equalizer_post_round_shape(definition),
                "retexted {id} shape",
            );
        }
    }

    #[test]
    fn brawl_is_admitted_by_grammar_across_every_combat_stat_form() {
        let registry = registry();

        // The whole combat-stat Brawl set. Each is admitted by its printed text over the
        // anti-support shape, never by an id list, and each must carry the AntiSupport
        // magnitude rather than riding Fixed.
        for id in [
            1488, 1490, 1556, 1703, 1707, 1759, 1834, 2560, 2859, 2905, 2917, 2973, 3047, 3219,
            3272, 3303, 3855, 3936, 3948, 4463, 4826, 4897, 5255, 5339, 5340, 5376, 5497, 5519,
            5524, 5527, 5759,
        ] {
            let definition = registry.get(id).expect("registry definition");
            let classified =
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability);
            let Some((SupportedEffectV1::ModifyCombatStat { multiplier, .. }, predicate)) =
                classified
            else {
                panic!("{id} was not admitted as a combat-stat modifier: {classified:?}");
            };
            assert_eq!(
                multiplier,
                MagnitudeMultiplierV1::AntiSupport,
                "definition {id} magnitude",
            );
            assert_eq!(
                predicate,
                CombatStatPredicateV1::Always,
                "definition {id} predicate",
            );
        }

        // The clan-gated Brawls carry a condition this grammar does not model, and the
        // Life and Pillz Brawls are the post-round Brawl grammars rather than combat stats.
        for id in [3666, 3667, 2893, 5650, 4583, 5172, 5822, 5844] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_brawl_numeric(definition),
                None,
                "definition {id} must stay out of the combat-stat Brawl grammar",
            );
        }

        // Text and structure must corroborate. A Brawl record whose printed numbers
        // disagree with its own magnitude, or which drops the anti-support flag, is
        // refused rather than trusted either way.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        //
        // `isSupport` is not in this list: the registry loader refuses a record carrying
        // both support and anti-support outright, which is a stronger guarantee than the
        // grammar could give, and is asserted separately below.
        for (field, value) in [
            ("value", serde_json::json!(2)),
            ("isAntiSupport", serde_json::json!(false)),
            ("isOverdrive", serde_json::json!(true)),
            ("currentRoundRequirement", serde_json::json!("win")),
            ("positionRequirement", serde_json::json!("attacker")),
        ] {
            let mut malformed = source.clone();
            malformed["1488"]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            assert_eq!(
                classify_brawl_numeric(malformed.get(1488).unwrap()),
                None,
                "malformed {field} = {value}",
            );
        }

        // Support and anti-support are mutually exclusive at the registry boundary, so a
        // record claiming both never reaches any grammar at all.
        let mut both = source.clone();
        both["1488"]["abilityData"]["isSupport"] = serde_json::json!(true);
        assert!(EffectRegistryV1::from_reader(both.to_string().as_bytes()).is_err());
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
    fn killshot_own_gains_unison_and_toxin_are_admitted_by_grammar_and_stay_ability_only() {
        use KillshotPostRoundEffectV1::{GainLife, GainPillz, ToxinOpponentLife};
        let registry = registry();
        let always = CombatStatPredicateV1::Always;
        for (id, description, effect, predicate) in [
            (2250, "Killshot: +3 Pillz", GainPillz { pillz: 3 }, always),
            (4311, "Killshot: +3 Pillz", GainPillz { pillz: 3 }, always),
            (4645, "Killshot: +2 Pillz", GainPillz { pillz: 2 }, always),
            (
                1231,
                "Killshot: +3 Life",
                GainLife {
                    life: 3,
                    maximum: 0,
                },
                always,
            ),
            (
                3760,
                "Killshot: +3 Life",
                GainLife {
                    life: 3,
                    maximum: 0,
                },
                always,
            ),
            (
                2956,
                "Killshot: +4 Life",
                GainLife {
                    life: 4,
                    maximum: 0,
                },
                always,
            ),
            (
                5065,
                "Killshot: +5 Life Max. 14",
                GainLife {
                    life: 5,
                    maximum: 14,
                },
                always,
            ),
            (
                5066,
                "Killshot: +5 Life Max. 14",
                GainLife {
                    life: 5,
                    maximum: 14,
                },
                always,
            ),
            (
                3894,
                "Unison: Killshot: +4 Life",
                GainLife {
                    life: 4,
                    maximum: 0,
                },
                CombatStatPredicateV1::OwnerHandUnison,
            ),
            (
                2497,
                "Killshot: Toxin 1, Min 0",
                ToxinOpponentLife {
                    life: 1,
                    minimum: 0,
                },
                always,
            ),
        ] {
            let definition = registry.lookup_capture(id, description).unwrap();
            assert_eq!(
                classify_killshot_post_round(definition, CombatStatEffectSourceV1::Ability),
                Some((effect, predicate)),
                "definition {id}",
            );
            // No clan bonus prints a Killshot.
            assert_eq!(
                classify_killshot_post_round(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "definition {id} as a bonus",
            );
            assert!(has_killshot_post_round_shape(definition), "shape {id}");
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                None,
                "definition {id} as a combat stat",
            );
            // The neighbouring grammars never claim it.
            let ability = CombatStatEffectSourceV1::Ability;
            assert_eq!(classify_killshot_opponent_life(definition, ability), None);
            assert_eq!(classify_killshot_pillz_and_life(definition, ability), None);
            assert_eq!(classify_victory_life(definition, ability), None);
            assert_eq!(classify_victory_pillz(definition, ability), None);
            assert_eq!(
                classify_toxin_opponent_life_on_victory(definition, ability),
                None
            );
            assert_eq!(classify_unison_numeric(definition, ability), None);
        }

        // The rest of the channel stays closed: the admitted opposing reduction and the
        // compound have their own grammars, the opposing Pillz-and-Life compound and the
        // whole-hand `Team:` form have none.
        for (id, description) in [
            (1204, "Killshot: -6 Opp. Life Min 0"),
            (1768, "Killshot: +2 Pillz And Life"),
            (5775, "Killshot: -2 Opp. Pillz And Life, Min 2"),
            (5776, "Killshot: -2 Opp. Pillz And Life, Min 0"),
            (3480, "Team: Killshot: -2 Opp. Life Min 2"),
        ] {
            let definition = registry.lookup_capture(id, description).unwrap();
            assert_eq!(
                classify_killshot_post_round(definition, CombatStatEffectSourceV1::Ability),
                None,
                "definition {id}",
            );
            assert!(!has_killshot_post_round_shape(definition), "shape {id}");
        }

        // The printed numbers and the channel are authority. A record whose text disagrees
        // with its own magnitude or cap, that asks a won round - the plain Victory grammar
        // wearing Killshot's text - or that gates a form no card prints gated is refused.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (id, description, field, value) in [
            ("1231", "Killshot: +3 Life", "value", serde_json::json!(4)),
            (
                "1231",
                "Killshot: +3 Life",
                "valueMax",
                serde_json::json!(14),
            ),
            (
                "1231",
                "Killshot: +3 Life",
                "currentRoundRequirement",
                serde_json::json!("win"),
            ),
            (
                "1231",
                "Killshot: +3 Life",
                "isClanmatesCountLinked",
                serde_json::json!(true),
            ),
            (
                "1231",
                "Killshot: +3 Life",
                "sideAffected",
                serde_json::json!("opponent"),
            ),
            (
                "1231",
                "Killshot: +3 Life",
                "isPermanent",
                serde_json::json!(true),
            ),
            (
                "2250",
                "Killshot: +3 Pillz",
                "isClanmatesCountLinked",
                serde_json::json!(true),
            ),
            (
                "2250",
                "Killshot: +3 Pillz",
                "valueMax",
                serde_json::json!(9),
            ),
            (
                "5065",
                "Killshot: +5 Life Max. 14",
                "isClanmatesCountLinked",
                serde_json::json!(true),
            ),
            (
                "3894",
                "Unison: Killshot: +4 Life",
                "isClanmatesCountLinked",
                serde_json::json!(false),
            ),
            (
                "2497",
                "Killshot: Toxin 1, Min 0",
                "isImmediatePermanent",
                serde_json::json!(false),
            ),
            (
                "2497",
                "Killshot: Toxin 1, Min 0",
                "indexRequirement",
                serde_json::json!("symmetry"),
            ),
            (
                "2497",
                "Killshot: Toxin 1, Min 0",
                "currentRoundRequirement",
                serde_json::json!("win"),
            ),
        ] {
            let mut malformed = source.clone();
            malformed[id]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            assert_eq!(
                classify_killshot_post_round(
                    malformed
                        .lookup_capture(id.parse().unwrap(), description)
                        .unwrap(),
                    CombatStatEffectSourceV1::Ability,
                ),
                None,
                "malformed {id} {field} = {value}",
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
        // Since revision 50 the Unison source Copy is a grammar of its own.
        assert_eq!(
            classify_copy_opponent_source(
                registry
                    .lookup_capture(3994, "Unison : Copy: Opp. Ability")
                    .unwrap()
            ),
            Some((
                CopiedSourceKindV1::Ability,
                CombatStatPredicateV1::OwnerHandUnison
            )),
        );
        // Every other conditional keeps its own deferred grammar, and a stat-copying
        // variant is never a source copy: since revision 24 the unconditional ones are
        // admitted as their own effect, and `4126` matters in particular because it is a
        // Reprisal Copy, but of a stat. (`5304`, the `Bet > 3 Pillz:` Copy, is admitted
        // since revision 56 with its gate as the adoption predicate, and the clan-gated
        // `5073` since revision 70.)
        for (id, description) in [
            (1409, "Confidence: Copy: Opp. Power"),
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
            // Courage carries its condition in the position field, and Ligea's first two
            // levels are the only conditional members whose Min is not zero.
            (
                3314,
                "Courage: - 1 Opp. Life Min 0",
                1,
                CombatStatPredicateV1::OwnerMovesFirst,
            ),
            (
                4533,
                "Courage: - 3 Opp. Life Min 0",
                3,
                CombatStatPredicateV1::OwnerMovesFirst,
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
        // Ligea's first two levels print the same text under two ids and are the only
        // conditional members with a non-zero Min, so they are checked apart from the loop
        // above rather than by widening its tuple.
        for id in [4531, 4532] {
            let floored = registry
                .lookup_capture(id, "Courage: - 3 Opp. Life Min 1")
                .unwrap();
            assert_eq!(
                classify_victory_opponent_life(floored, CombatStatEffectSourceV1::Ability),
                Some((3, 1, CombatStatPredicateV1::OwnerMovesFirst)),
                "Ligea {id}",
            );
            assert_eq!(
                classify_victory_opponent_life(floored, CombatStatEffectSourceV1::Bonus),
                None,
                "Ligea {id} as bonus",
            );
        }
        // Growth `1730` is a round-scaled magnitude rather than a predicate, so it stays
        // deferred although it shares this exact structure. Dragomer Cr's levels 4 and 5
        // print `3001` and `2302`, which have no registry definition at all: they are
        // fail-closed by absence and there is nothing here to assert about them.
        for (id, description) in [(1730, "Growth: - 1 Opp. Life Min 4")] {
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

        // `Night:` prints the complete Victory shape under the match constant. Since revision
        // 69 it is the grammar's night form, a card ability only; any other prefix over the
        // same record stays refused and is reported as a structural near-miss instead.
        let night = registry
            .lookup_capture(4750, "Night: -2 Opp. Life Min 0")
            .unwrap();
        assert_eq!(
            classify_victory_opponent_life(night, CombatStatEffectSourceV1::Ability),
            Some((2, 0, CombatStatPredicateV1::MatchIsNight)),
        );
        assert_eq!(
            classify_victory_opponent_life(night, CombatStatEffectSourceV1::Bonus),
            None,
        );
        assert!(has_victory_opponent_life_shape(night));
        {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
            let source: serde_json::Value =
                serde_json::from_reader(File::open(&path).unwrap()).unwrap();
            for (field, value) in [
                ("description", serde_json::json!("Day: -2 Opp. Life Min 0")),
                (
                    "description",
                    serde_json::json!("Night : -2 Opp. Life Min 0"),
                ),
                (
                    "description",
                    serde_json::json!("Night: -3 Opp. Life Min 0"),
                ),
            ] {
                let mut malformed = source.clone();
                malformed["4750"][field] = value.clone();
                let malformed =
                    EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
                assert_eq!(
                    classify_victory_opponent_life(
                        malformed.get(4750).unwrap(),
                        CombatStatEffectSourceV1::Ability
                    ),
                    None,
                    "4750 {field} = {value}",
                );
            }
            for (field, value) in [
                ("value", serde_json::json!(3)),
                ("valueMax", serde_json::json!(4)),
                ("previousRoundRequirement", serde_json::json!("win")),
                ("currentRoundRequirement", serde_json::json!("lose")),
            ] {
                let mut malformed = source.clone();
                malformed["4750"]["abilityData"][field] = value.clone();
                let malformed =
                    EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
                assert_eq!(
                    classify_victory_opponent_life(
                        malformed.get(4750).unwrap(),
                        CombatStatEffectSourceV1::Ability
                    ),
                    None,
                    "4750 {field} = {value}",
                );
            }
        }

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
        // inventory is the only thing that had to learn about it. The same holds for `513`
        // `Confidence: Attack +12`, `644` `Revenge: -2 Opp. Power, Min 5` and `2194`
        // `Confidence: Power And Damage +2`, which arrived with the 2026-09-23 captures.
        let admitted = BTreeSet::from([
            463, 465, 478, 513, 520, 553, 555, 556, 560, 585, 591, 634, 644, 784, 801, 859, 883,
            884, 921, 938, 965, 1053, 1091, 1107, 1278, 1286, 1303, 1395, 1417, 1839, 2194, 2628,
            2657, 3827, 3829, 4316, 4399, 4464, 4623, 4711, 4838, 5406, 5881,
        ]);
        // Since revision 47 the `Confidence:` and `Revenge:` Stops are admitted by the
        // conditional-Stop grammar, card abilities only.
        // And since revision 50 the `Confidence:` stat Copy and Exchange, by the conditional
        // stat-Copy grammar.
        // Since revision 72 Betul's `Revenge: + 2 Attack Per Opp. Power` too, by its own
        // grammar and from card abilities only.
        let ability_only = BTreeSet::from([490, 589, 1409, 1680, 1713, 1719]);
        let deferred = BTreeSet::from([
            814, 1643, 1652, 1661, 1702, 1751, 1810, 2113, 2582, 3016, 3301, 3546, 4301, 4449, 4972,
        ]);
        let observed: BTreeSet<_> = registry
            .iter()
            .filter_map(|(id, definition)| {
                (definition.structured_input().previous_round_requirement
                    != PreviousRoundRequirementV1::Any)
                    .then_some(id)
            })
            .collect();
        assert_eq!(
            observed,
            admitted
                .iter()
                .chain(&ability_only)
                .chain(&deferred)
                .copied()
                .collect()
        );

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
            let expected: BTreeSet<_> = if source == CombatStatEffectSourceV1::Ability {
                admitted.union(&ability_only).copied().collect()
            } else {
                admitted.clone()
            };
            assert_eq!(classified, expected, "{source:?}");
        }
    }

    #[test]
    fn stop_triggered_numeric_is_admitted_over_the_inverted_shape() {
        let registry = registry();
        for id in [505, 654, 908, 1203, 1474, 1984, 2175, 5923] {
            let definition = registry.get(id).expect("registry definition");
            let classified =
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability);
            assert!(
                matches!(
                    classified,
                    Some((
                        SupportedEffectV1::ModifyCombatStat { .. },
                        CombatStatPredicateV1::OwnerAbilityStopped,
                    ))
                ),
                "definition {id}: {classified:?}",
            );
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "definition {id} as a bonus",
            );
        }
        // The Pillz forms are post-round work and stay closed.
        for id in [646, 918] {
            assert_eq!(
                classify_combat_stat_effect(
                    registry.get(id).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "definition {id}",
            );
        }
        // The inverted flag is the only structured trace of the prefix: without it the
        // `Stop:` text is refused, and the flag under plain text is refused too.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        let mut plain = source.clone();
        plain["1474"]["abilityData"]["isInverted"] = serde_json::json!(false);
        let plain = EffectRegistryV1::from_reader(plain.to_string().as_bytes()).unwrap();
        assert_eq!(
            classify_stop_triggered_numeric(
                plain.get(1474).unwrap(),
                CombatStatEffectSourceV1::Ability
            ),
            None
        );
        let mut retexted = source.clone();
        retexted["1474"]["description"] = serde_json::json!("Damage +4");
        let retexted = EffectRegistryV1::from_reader(retexted.to_string().as_bytes()).unwrap();
        assert_eq!(
            classify_combat_stat_effect(
                retexted.get(1474).unwrap(),
                CombatStatEffectSourceV1::Ability
            ),
            None
        );
    }

    #[test]
    fn conditional_stop_is_admitted_by_grammar_over_the_resolved_predicates() {
        let registry = registry();
        for (id, effect, predicate) in [
            (
                287,
                SupportedEffectV1::StopOpponentBonus,
                CombatStatPredicateV1::OwnerMovesFirst,
            ),
            (
                425,
                SupportedEffectV1::StopOpponentAbility,
                CombatStatPredicateV1::OwnerMovesFirst,
            ),
            (
                490,
                SupportedEffectV1::StopOpponentAbility,
                CombatStatPredicateV1::OwnerWonPreviousRound,
            ),
            (
                589,
                SupportedEffectV1::StopOpponentAbility,
                CombatStatPredicateV1::OwnerLostPreviousRound,
            ),
            (
                2320,
                SupportedEffectV1::StopOpponentAbility,
                CombatStatPredicateV1::SelectedHandSlotsDiffer,
            ),
            (
                5540,
                SupportedEffectV1::StopOpponentBonus,
                CombatStatPredicateV1::SelectedHandSlotsDiffer,
            ),
            (
                4525,
                SupportedEffectV1::StopOpponentBonus,
                CombatStatPredicateV1::SelectedHandSlotsMatch,
            ),
            (
                5564,
                SupportedEffectV1::StopOpponentAbility,
                CombatStatPredicateV1::MatchIsNight,
            ),
            (
                3839,
                SupportedEffectV1::StopOpponentAbility,
                CombatStatPredicateV1::OwnerHandUnison,
            ),
            (
                5752,
                SupportedEffectV1::StopOpponentAbility,
                CombatStatPredicateV1::OwnerHandUnison,
            ),
            (
                5753,
                SupportedEffectV1::StopOpponentBonus,
                CombatStatPredicateV1::OwnerHandUnison,
            ),
        ] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_combat_stat_effect(definition, CombatStatEffectSourceV1::Ability),
                Some((effect, predicate)),
                "definition {id}",
            );
            assert_eq!(
                classify_conditional_stop(definition, CombatStatEffectSourceV1::Bonus),
                None,
                "definition {id} as a bonus",
            );
        }
        // Reprisal keeps its identity lock, and the `Bet >` and `After` forms are other
        // grammars. The clan-gated `Asymm.:` form `4999` is admitted since revision 70 under
        // its compound predicate, tested on its own.
        for id in [4860, 4949, 5738] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_conditional_stop(definition, CombatStatEffectSourceV1::Ability),
                None,
                "definition {id}",
            );
        }
        // The text must name the structured condition and nothing else. `5564` shares the
        // plain Stop's structure exactly, so the `Night:` text is its only mark - a classifier
        // that ignored text would admit it as unconditional.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for (id, field, value) in [
            ("287", "positionRequirement", serde_json::json!("defender")),
            ("287", "previousRoundRequirement", serde_json::json!("win")),
            ("490", "positionRequirement", serde_json::json!("attacker")),
            ("589", "value", serde_json::json!(1)),
            ("2320", "indexRequirement", serde_json::json!("symmetry")),
            ("5564", "positionRequirement", serde_json::json!("attacker")),
            // The clan-mates flag is the only structured trace of `Unison :`: without it the
            // text is refused, and under another condition the record is a compound.
            ("5753", "isClanmatesCountLinked", serde_json::json!(false)),
            ("5753", "previousRoundRequirement", serde_json::json!("win")),
            ("3839", "isSupport", serde_json::json!(true)),
        ] {
            let mut malformed = source.clone();
            malformed[id]["abilityData"][field] = value.clone();
            let malformed =
                EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
            assert_eq!(
                classify_conditional_stop(
                    malformed.get(id.parse().unwrap()).unwrap(),
                    CombatStatEffectSourceV1::Ability
                ),
                None,
                "malformed {id} {field} = {value}",
            );
        }
        // And the flag under the plain text is not a plain Stop either.
        let mut retexted = source.clone();
        retexted["3839"]["description"] = serde_json::json!("Stop Opp. Ability");
        let retexted = EffectRegistryV1::from_reader(retexted.to_string().as_bytes()).unwrap();
        assert_eq!(
            classify_combat_stat_effect(
                retexted.get(3839).unwrap(),
                CombatStatEffectSourceV1::Ability
            ),
            None
        );
    }

    #[test]
    fn recover_is_admitted_by_grammar_over_every_printed_prefix_and_ratio() {
        let registry = registry();
        let read = |id, text| {
            classify_recover_pillz(
                registry.lookup_capture(id, text).unwrap(),
                CombatStatEffectSourceV1::Ability,
            )
        };
        let defeat = |numerator, denominator| RecoverPillzV1 {
            on_victory: false,
            numerator,
            denominator,
            predicate: CombatStatPredicateV1::Always,
        };
        let victory = |numerator, denominator, predicate| RecoverPillzV1 {
            on_victory: true,
            numerator,
            denominator,
            predicate,
        };
        // Revision 8's identities, and the same-text Sasl Lovelace record it had locked out.
        for id in [577, 729, 1418, 2475] {
            assert_eq!(
                read(id, "Defeat: Recover 2 Pillz Out Of 3"),
                Some(defeat(2, 3))
            );
        }
        for id in [770, 902, 1035, 2108, 2217] {
            assert_eq!(
                read(id, "Defeat: Recover 1 Pillz Out Of 2"),
                Some(defeat(1, 2))
            );
        }
        for id in [3459, 4610, 5651] {
            assert_eq!(
                read(id, "Recover 1 Pillz Out Of 3"),
                Some(victory(1, 3, CombatStatPredicateV1::Always))
            );
        }
        assert_eq!(
            read(3752, "Unison : Recover 1 Pillz Out Of 3"),
            Some(victory(1, 3, CombatStatPredicateV1::OwnerHandUnison))
        );
        assert_eq!(
            read(4050, "Unison : Recover 1 Pillz Out Of 2"),
            Some(victory(1, 2, CombatStatPredicateV1::OwnerHandUnison))
        );
        // The Vortex bonus prints the Defeat form; no clan prints the Victory forms.
        assert_eq!(
            classify_recover_pillz(
                registry
                    .lookup_capture(577, "Defeat: Recover 2 Pillz Out Of 3")
                    .unwrap(),
                CombatStatEffectSourceV1::Bonus,
            ),
            Some(defeat(2, 3))
        );
        for (id, text) in [
            (5651, "Recover 1 Pillz Out Of 3"),
            (4050, "Unison : Recover 1 Pillz Out Of 2"),
        ] {
            assert_eq!(
                classify_recover_pillz(
                    registry.lookup_capture(id, text).unwrap(),
                    CombatStatEffectSourceV1::Bonus,
                ),
                None,
                "{id}"
            );
        }
        // Every structural near miss: another outcome, a lost clan-mates link, a swapped or
        // degenerate ratio, and each neutral field disturbed.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        for id in ["577", "1035", "3459", "3752"] {
            for (field, value) in [
                ("currentRoundRequirement", serde_json::json!("any")),
                ("isClanmatesCountLinked", serde_json::json!(id != "3752")),
                ("value", serde_json::json!(0)),
                ("valueMin", serde_json::json!(1)),
                ("valueMin", serde_json::json!(0)),
                ("valueMax", serde_json::json!(4)),
                ("previousRoundRequirement", serde_json::json!("win")),
                ("positionRequirement", serde_json::json!("attacker")),
                ("sideAffected", serde_json::json!("opponent")),
                ("attributeAffected", serde_json::json!("life")),
                ("specialAction", serde_json::json!("none")),
                ("isPermanent", serde_json::json!(true)),
                ("clanRequirement", serde_json::json!("45")),
            ] {
                let mut malformed = source.clone();
                malformed[id]["abilityData"][field] = value.clone();
                let malformed =
                    EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap();
                let definition = malformed.get(id.parse().unwrap()).unwrap();
                assert_eq!(
                    classify_recover_pillz(definition, CombatStatEffectSourceV1::Ability),
                    None,
                    "malformed {id} {field} = {value}",
                );
            }
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

    /// Revision 71: Backlash Life on the Victory channel for a Min of at least 1, the capped
    /// Defeat Life and the Defeat opposing Pillz gift, each by exact text over the complete
    /// shape, card abilities only. The `Min 0` Backlash records keep the structural shape, so
    /// replay rejects them when selected, but the classifier refuses them.
    #[test]
    fn revision_71_post_round_grammars_are_admitted_by_exact_text_and_shape() {
        let registry = registry();
        let ability = CombatStatEffectSourceV1::Ability;
        for (id, expected) in [
            (1058, (3, 1)),
            (1351, (3, 2)),
            (5410, (1, 3)),
            (2853, (3, 3)),
        ] {
            let definition = registry.get(id).expect("registry definition");
            assert!(has_backlash_life_shape(definition), "{id}");
            assert_eq!(classify_backlash_life(definition, ability), Some(expected));
            assert_eq!(
                classify_backlash_life(definition, CombatStatEffectSourceV1::Bonus),
                None
            );
            assert_eq!(classify_combat_stat_effect(definition, ability), None);
        }
        for id in [3092, 1667] {
            let definition = registry.get(id).expect("registry definition");
            assert!(has_backlash_life_shape(definition), "{id}");
            assert_eq!(classify_backlash_life(definition, ability), None, "{id}");
        }
        // `Defeat: Backlash:`, the Pillz form, the own-Life Poison and `Corrupt` are other
        // structures.
        for id in [2417, 1401, 4124, 5286] {
            let definition = registry.get(id).expect("registry definition");
            assert!(!has_backlash_life_shape(definition), "{id}");
            assert_eq!(classify_backlash_life(definition, ability), None, "{id}");
        }
        for (id, expected) in [(1217, (2, 12)), (5083, (3, 11)), (5574, (3, 10))] {
            let definition = registry.get(id).expect("registry definition");
            assert!(has_defeat_capped_life_shape(definition), "{id}");
            assert_eq!(
                classify_defeat_capped_life(definition, ability),
                Some(expected)
            );
            assert_eq!(
                classify_defeat_capped_life(definition, CombatStatEffectSourceV1::Bonus),
                None
            );
            // Never the uncapped grammar.
            assert_eq!(classify_defeat_life(definition, ability), None);
            assert!(!has_defeat_life_shape(definition));
            assert_eq!(classify_combat_stat_effect(definition, ability), None);
        }
        let gift = registry.get(3223).expect("registry definition");
        assert!(has_defeat_opponent_pillz_gain_shape(gift));
        assert_eq!(classify_defeat_opponent_pillz_gain(gift, ability), Some(1));
        assert_eq!(
            classify_defeat_opponent_pillz_gain(gift, CombatStatEffectSourceV1::Bonus),
            None
        );
        assert_eq!(classify_defeat_opponent_pillz(gift, ability), None);
        assert_eq!(classify_combat_stat_effect(gift, ability), None);
        // `Defeat: +1 Opp. Life`, the plain Defeat gains and the Defeat Pillz reduction are
        // other grammars.
        for id in [1762, 912, 2221] {
            let definition = registry.get(id).expect("registry definition");
            assert!(!has_defeat_opponent_pillz_gain_shape(definition), "{id}");
            assert!(!has_defeat_capped_life_shape(definition), "{id}");
        }

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        let malformed = |id: &str, field: &str, value: serde_json::Value| {
            let mut malformed = source.clone();
            malformed[id]["abilityData"][field] = value;
            EffectRegistryV1::from_reader(malformed.to_string().as_bytes()).unwrap()
        };
        for (field, value) in [
            ("value", serde_json::json!(2)),
            ("valueMin", serde_json::json!(2)),
            ("valueMax", serde_json::json!(5)),
            ("currentRoundRequirement", serde_json::json!("lose")),
            ("currentRoundRequirement", serde_json::json!("any")),
            ("sideAffected", serde_json::json!("opponent")),
            ("attributeAffected", serde_json::json!("pillz")),
            ("attributeAction", serde_json::json!("increase")),
            ("isPermanent", serde_json::json!(true)),
            ("previousRoundRequirement", serde_json::json!("win")),
        ] {
            let registry = malformed("1058", field, value.clone());
            let definition = registry.get(1058).unwrap();
            assert_eq!(
                classify_backlash_life(definition, ability),
                None,
                "1058 {field} = {value}"
            );
        }
        for (field, value) in [
            ("value", serde_json::json!(3)),
            ("valueMin", serde_json::json!(0)),
            ("valueMax", serde_json::json!(0)),
            ("valueMax", serde_json::json!(11)),
            ("currentRoundRequirement", serde_json::json!("win")),
            ("isPermanent", serde_json::json!(true)),
            ("isClanmatesCountLinked", serde_json::json!(true)),
        ] {
            let registry = malformed("1217", field, value.clone());
            let definition = registry.get(1217).unwrap();
            assert_eq!(
                classify_defeat_capped_life(definition, ability),
                None,
                "1217 {field} = {value}"
            );
        }
        for (field, value) in [
            ("value", serde_json::json!(2)),
            ("valueMin", serde_json::json!(1)),
            ("currentRoundRequirement", serde_json::json!("win")),
            ("sideAffected", serde_json::json!("player")),
            ("attributeAffected", serde_json::json!("life")),
            ("attributeAction", serde_json::json!("decrease")),
        ] {
            let registry = malformed("3223", field, value.clone());
            let definition = registry.get(3223).unwrap();
            assert_eq!(
                classify_defeat_opponent_pillz_gain(definition, ability),
                None,
                "3223 {field} = {value}"
            );
        }
        for (id, text) in [
            ("1058", "Backlash: -3 Life Min 1"),
            ("1058", "Backlash: - 3 Life, Min 1"),
            ("1058", "- 3 Life Min 1"),
            ("1217", "Defeat: +2 Life Max. 12"),
            ("1217", "Defeat: +2 Life"),
            ("3223", "Defeat: +1 Opp Pillz"),
            ("3223", "+1 Opp. Pillz"),
        ] {
            let mut retexted = source.clone();
            retexted[id]["description"] = serde_json::json!(text);
            let retexted = EffectRegistryV1::from_reader(retexted.to_string().as_bytes()).unwrap();
            let definition = retexted.get(id.parse().unwrap()).unwrap();
            assert_eq!(
                classify_backlash_life(definition, ability),
                None,
                "{text:?}"
            );
            assert_eq!(
                classify_defeat_capped_life(definition, ability),
                None,
                "{text:?}"
            );
            assert_eq!(
                classify_defeat_opponent_pillz_gain(definition, ability),
                None,
                "{text:?}"
            );
        }
    }

    /// Revision 72: `+N Attack Per Opp. Power` (registry-compiled) and its `Revenge:` form
    /// (compiler-side), Corrupt by exact text over the complete shape, and Djanghost Ld's
    /// Night numeric by identity. All card abilities only.
    #[test]
    fn revision_72_grammars_are_admitted_by_exact_text_shape_and_identity() {
        let registry = registry();
        let ability = CombatStatEffectSourceV1::Ability;
        let bonus = CombatStatEffectSourceV1::Bonus;
        let per_power = SupportedEffectV1::ModifyCombatStat {
            side: AffectedSideV1::Player,
            stat: CombatStatV1::Attack,
            operation: StatOperationV1::Increase,
            value: 2,
            minimum: None,
            maximum: None,
            multiplier: MagnitudeMultiplierV1::OpponentPower,
        };
        for id in [1785, 4661] {
            let definition = registry.get(id).expect("registry definition");
            assert_eq!(
                classify_combat_stat_effect(definition, ability),
                Some((per_power, CombatStatPredicateV1::Always)),
                "{id}"
            );
            assert_eq!(classify_combat_stat_effect(definition, bonus), None, "{id}");
        }
        let betul = registry.get(1719).expect("registry definition");
        assert_eq!(
            classify_combat_stat_effect(betul, ability),
            Some((per_power, CombatStatPredicateV1::OwnerLostPreviousRound))
        );
        assert_eq!(classify_combat_stat_effect(betul, bonus), None);

        let nega = registry.get(5286).expect("registry definition");
        assert!(has_corrupt_own_life_shape(nega));
        assert_eq!(classify_corrupt_own_life(nega, ability), Some((2, 5)));
        assert_eq!(classify_corrupt_own_life(nega, bonus), None);
        assert_eq!(classify_combat_stat_effect(nega, ability), None);
        // Xantiax, Backlash and the opposing Victory reduction are other structures.
        for id in [1379, 5198, 1058, 3092, 512] {
            let definition = registry.get(id).expect("registry definition");
            assert!(!has_corrupt_own_life_shape(definition), "{id}");
            assert_eq!(classify_corrupt_own_life(definition, ability), None, "{id}");
        }
        assert!(!has_both_players_life_reduction_shape(nega));
        assert!(!has_backlash_life_shape(nega));

        let djanghost = registry.get(5391).expect("registry definition");
        let night_power = SupportedEffectV1::ModifyCombatStat {
            side: AffectedSideV1::Opponent,
            stat: CombatStatV1::Power,
            operation: StatOperationV1::Decrease,
            value: 4,
            minimum: Some(4),
            maximum: None,
            multiplier: MagnitudeMultiplierV1::Fixed,
        };
        assert_eq!(
            classify_combat_stat_effect(djanghost, ability),
            Some((night_power, CombatStatPredicateV1::MatchIsNight))
        );
        assert_eq!(classify_combat_stat_effect(djanghost, bonus), None);

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../captures/abilities.json");
        let source: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        let edited = |id: &str, field: &str, value: serde_json::Value| {
            let mut edited = source.clone();
            if field == "description" {
                edited[id]["description"] = value;
            } else {
                edited[id]["abilityData"][field] = value;
            }
            EffectRegistryV1::from_reader(edited.to_string().as_bytes()).unwrap()
        };
        // The Revenge form: its one condition, its special action and its spelling.
        for (field, value) in [
            ("previousRoundRequirement", serde_json::json!("win")),
            ("previousRoundRequirement", serde_json::json!("any")),
            ("positionRequirement", serde_json::json!("attacker")),
            ("specialAction", serde_json::json!("convert_opp_dmg_to_atk")),
            ("specialAction", serde_json::json!("none")),
            ("valueMax", serde_json::json!(4)),
            ("valueMin", serde_json::json!(1)),
            ("value", serde_json::json!(3)),
            ("sideAffected", serde_json::json!("opponent")),
            ("attributeAffected", serde_json::json!("pwr")),
            ("isSupport", serde_json::json!(true)),
            ("isPermanent", serde_json::json!(true)),
            (
                "description",
                serde_json::json!("Revenge: +2 Attack Per Opp. Power"),
            ),
            (
                "description",
                serde_json::json!("Revenge: + 2 Attack Per Opp Power"),
            ),
            (
                "description",
                serde_json::json!("Revenge: + 2 Attack Per Opp. Damage"),
            ),
            (
                "description",
                serde_json::json!("Confidence: + 2 Attack Per Opp. Power"),
            ),
            (
                "description",
                serde_json::json!("+ 2 Attack Per Opp. Power"),
            ),
        ] {
            let registry = edited("1719", field, value.clone());
            assert_eq!(
                classify_combat_stat_effect(registry.get(1719).unwrap(), ability),
                None,
                "1719 {field} = {value}"
            );
        }
        // Corrupt: the refused `Min 0`, the structure and the spelling.
        let min_zero = edited("5286", "valueMin", serde_json::json!(0));
        let min_zero_definition = min_zero.get(5286).unwrap();
        assert!(has_corrupt_own_life_shape(min_zero_definition));
        assert_eq!(
            classify_corrupt_own_life(min_zero_definition, ability),
            None
        );
        for (field, value) in [
            ("value", serde_json::json!(3)),
            ("valueMin", serde_json::json!(4)),
            ("valueMax", serde_json::json!(5)),
            ("currentRoundRequirement", serde_json::json!("win")),
            ("currentRoundRequirement", serde_json::json!("lose")),
            ("sideAffected", serde_json::json!("both")),
            ("sideAffected", serde_json::json!("opponent")),
            ("attributeAffected", serde_json::json!("pillz")),
            ("attributeAction", serde_json::json!("increase")),
            ("previousRoundRequirement", serde_json::json!("win")),
            ("isPermanent", serde_json::json!(true)),
            ("isOverdrive", serde_json::json!(true)),
            ("description", serde_json::json!("Corrupt 2 Min 5")),
            ("description", serde_json::json!("Corrupt 2, Min. 5")),
            ("description", serde_json::json!("Xantiax: -2 Life, Min. 5")),
            ("description", serde_json::json!("Corrupt 2 Min. 5 ")),
        ] {
            let registry = edited("5286", field, value.clone());
            assert_eq!(
                classify_corrupt_own_life(registry.get(5286).unwrap(), ability),
                None,
                "5286 {field} = {value}"
            );
        }
        // Djanghost Ld: the identity is the id, the text and the three equal numbers. A
        // `valueMax` on any other decrease stays refused.
        for (field, value) in [
            ("valueMax", serde_json::json!(5)),
            ("valueMax", serde_json::json!(3)),
            ("valueMin", serde_json::json!(3)),
            ("value", serde_json::json!(3)),
            (
                "description",
                serde_json::json!("Night: -4 Opp Power, Min 3"),
            ),
            ("description", serde_json::json!("Day: -4 Opp Power, Min 4")),
            ("description", serde_json::json!("-4 Opp Power, Min 4")),
            ("positionRequirement", serde_json::json!("attacker")),
        ] {
            let registry = edited("5391", field, value.clone());
            assert_eq!(
                classify_combat_stat_effect(registry.get(5391).unwrap(), ability),
                None,
                "5391 {field} = {value}"
            );
        }
        // The same stray bound on the plain `-4 Opp Power, Min 4` (`616`) is not admitted.
        let stray = edited("616", "valueMax", serde_json::json!(4));
        assert_eq!(
            classify_combat_stat_effect(stray.get(616).unwrap(), ability),
            None
        );
        assert!(matches!(
            classify_combat_stat_effect(registry.get(616).unwrap(), ability),
            Some((SupportedEffectV1::ModifyCombatStat { .. }, _))
        ));
    }
}
