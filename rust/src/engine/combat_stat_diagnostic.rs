//! Explicit ordinary combat-stat diagnostic projection.
//!
//! This module owns compact effect plans and hot-path resolution. Capture-specific
//! preparation and rich disposition metadata live in the replay diagnostic module.

use super::combat_resolution::{
    prepare_combat_resolution, CombatResolutionArithmeticStage, CombatResolutionError,
    ResolutionCardPlan, ResolutionSourcePlan,
};
use super::{
    BaseRulesError, BaseRulesGame, BaseRulesMatchSpec, BaseRulesPosition, BaseRulesRoundInput,
    BaseRulesRoundReport, BaseRulesUndo, ByPlayer, DiagnosticAffectedSideV1,
    DiagnosticCombatEffectV1, DiagnosticCombatStatV1, DiagnosticMagnitudeV1,
    DiagnosticStatOperationV1, HandSlot, PlayerId, PreparedSelection, ValidatedSelection,
    HAND_SIZE,
};
use crate::catalog::CardKey;
use std::error::Error;
use std::fmt;

/// The source slot of a projected effect. Source identity is retained for cancellation,
/// stable source ordering, and Support validation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CombatStatEffectSourceV1 {
    Ability,
    Bonus,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CombatStatAttributeV1 {
    Attack,
    Damage,
    Power,
    PowerAndDamage,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CombatStatAffectedSideV1 {
    Opponent,
    Player,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CombatStatOperationV1 {
    Decrease,
    Increase,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CombatStatMagnitudeV1 {
    Fixed,
    SourceBonusSupport,
    Growth,
    Degrowth,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CombatStatPredicateV1 {
    Always,
    OwnerMovesFirst,
    OwnerMovesSecond,
    SelectedHandSlotsMatch,
    SelectedHandSlotsDiffer,
}

/// String-free execution primitives admitted by the first diagnostic projection.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CombatStatEffectV1 {
    ModifyCombatStat {
        side: CombatStatAffectedSideV1,
        stat: CombatStatAttributeV1,
        operation: CombatStatOperationV1,
        value: u16,
        minimum: Option<u16>,
        maximum: Option<u16>,
        multiplier: CombatStatMagnitudeV1,
    },
    StopOpponentBonus,
    CancelOpponentCombatStatModifiers {
        stat: CombatStatAttributeV1,
    },
}

/// Compact per-source disposition consumed in the engine hot path. Rich descriptions and
/// disabled reasons remain at the outer replay-diagnostic boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CombatStatSourcePlanV1 {
    Absent,
    Execute {
        source_id: u32,
        predicate: CombatStatPredicateV1,
        effect: CombatStatEffectV1,
    },
    Disabled {
        source_id: u32,
    },
    RejectIfSelected {
        source_id: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CombatStatCardPlanV1 {
    pub key: CardKey,
    pub ability: CombatStatSourcePlanV1,
    pub bonus: CombatStatSourcePlanV1,
    /// Distinct captured character ids sharing this card's active source-bonus id across
    /// the immutable whole draw. This is capture context, not inferred clan membership.
    pub source_bonus_support_count: u16,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CombatStatDiagnosticMatchSpecV1 {
    pub base_rules: BaseRulesMatchSpec,
    pub cards: ByPlayer<[CombatStatCardPlanV1; HAND_SIZE]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CombatStatPlanMismatchV1 {
    pub player: PlayerId,
    pub hand_slot: HandSlot,
    pub expected: CardKey,
    pub actual: CardKey,
}

impl fmt::Display for CombatStatPlanMismatchV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "combat-stat diagnostic plan for {:?} slot {} has {:?}, expected {:?}",
            self.player,
            self.hand_slot.get(),
            self.actual,
            self.expected
        )
    }
}

impl Error for CombatStatPlanMismatchV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidCombatStatPlanReasonV1 {
    CappedIncrease,
    CompoundPredicateAndMagnitude,
    ConditionalBonus,
    ConditionalControl,
    IncompatibleBounds,
    InvalidModifierDirection,
    SupportAbility,
    ZeroMagnitude,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CombatStatPlanErrorV1 {
    CardMismatch(CombatStatPlanMismatchV1),
    InvalidSourceBonusContext {
        player: PlayerId,
        hand_slot: HandSlot,
        source_id: Option<u32>,
        expected: u16,
        actual: u16,
    },
    InvalidExecute {
        player: PlayerId,
        hand_slot: HandSlot,
        source: CombatStatEffectSourceV1,
        source_id: u32,
        reason: InvalidCombatStatPlanReasonV1,
    },
}

impl fmt::Display for CombatStatPlanErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CardMismatch(source) => source.fmt(formatter),
            Self::InvalidSourceBonusContext {
                player,
                hand_slot,
                source_id,
                expected,
                actual,
            } => write!(
                formatter,
                "invalid combat-stat diagnostic source-bonus context for {player:?} slot {} source {source_id:?}: expected {expected} distinct character ids, got {actual}",
                hand_slot.get()
            ),
            Self::InvalidExecute {
                player,
                hand_slot,
                source,
                source_id,
                reason,
            } => write!(
                formatter,
                "invalid combat-stat diagnostic Execute plan for {player:?} slot {} {source:?} {source_id}: {reason:?}",
                hand_slot.get()
            ),
        }
    }
}

impl Error for CombatStatPlanErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CardMismatch(source) => Some(source),
            Self::InvalidSourceBonusContext { .. } | Self::InvalidExecute { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CombatStatArithmeticStageV1 {
    EffectMagnitude,
    Power,
    Damage,
    Attack,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CombatStatDiagnosticErrorV1 {
    BaseRules(BaseRulesError),
    UnsupportedSelectedHazard {
        player: PlayerId,
        hand_slot: HandSlot,
        source: CombatStatEffectSourceV1,
        source_id: u32,
    },
    ArithmeticOverflow {
        player: PlayerId,
        stage: CombatStatArithmeticStageV1,
    },
}

impl fmt::Display for CombatStatDiagnosticErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BaseRules(source) => source.fmt(formatter),
            Self::UnsupportedSelectedHazard {
                player,
                hand_slot,
                source,
                source_id,
            } => write!(
                formatter,
                "combat-stat diagnostic cannot select {player:?} slot {}: unsupported {source:?} hazard {source_id}",
                hand_slot.get()
            ),
            Self::ArithmeticOverflow { player, stage } => write!(
                formatter,
                "combat-stat diagnostic {stage:?} arithmetic overflow for {player:?}"
            ),
        }
    }
}

impl Error for CombatStatDiagnosticErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::BaseRules(source) => Some(source),
            Self::UnsupportedSelectedHazard { .. } | Self::ArithmeticOverflow { .. } => None,
        }
    }
}

impl From<BaseRulesError> for CombatStatDiagnosticErrorV1 {
    fn from(source: BaseRulesError) -> Self {
        Self::BaseRules(source)
    }
}

/// Mode-specific, single-use undo token.
#[derive(Debug, Eq, PartialEq)]
pub struct CombatStatDiagnosticUndoV1 {
    base_rules: BaseRulesUndo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CombatStatDiagnosticV1 {
    spec: CombatStatDiagnosticMatchSpecV1,
    base_rules: BaseRulesGame,
}

impl CombatStatDiagnosticV1 {
    pub fn new(spec: CombatStatDiagnosticMatchSpecV1) -> Result<Self, CombatStatPlanErrorV1> {
        // Validate identity independently of source context so a malformed plan always
        // reports the fundamental card mismatch first.
        for player in PlayerId::ALL {
            for slot in HandSlot::ALL {
                let expected = spec.base_rules.players[player].hand[slot.index()].key;
                let actual = spec.cards[player][slot.index()].key;
                if actual != expected {
                    return Err(CombatStatPlanErrorV1::CardMismatch(
                        CombatStatPlanMismatchV1 {
                            player,
                            hand_slot: slot,
                            expected,
                            actual,
                        },
                    ));
                }
            }
        }
        for player in PlayerId::ALL {
            for slot in HandSlot::ALL {
                validate_source_bonus_context(player, slot, &spec.cards[player])?;
                validate_combat_stat_source_plan(
                    player,
                    slot,
                    CombatStatEffectSourceV1::Ability,
                    spec.cards[player][slot.index()].ability,
                )?;
                validate_combat_stat_source_plan(
                    player,
                    slot,
                    CombatStatEffectSourceV1::Bonus,
                    spec.cards[player][slot.index()].bonus,
                )?;
            }
        }
        let base_rules = BaseRulesGame::new(spec.base_rules.clone());
        Ok(Self { spec, base_rules })
    }

    pub fn match_spec(&self) -> &CombatStatDiagnosticMatchSpecV1 {
        &self.spec
    }

    pub fn base_rules_spec(&self) -> &BaseRulesMatchSpec {
        &self.spec.base_rules
    }

    pub fn position(&self) -> &BaseRulesPosition {
        self.base_rules.position()
    }

    pub fn card_plans(&self) -> &ByPlayer<[CombatStatCardPlanV1; HAND_SIZE]> {
        &self.spec.cards
    }

    pub fn make(
        &mut self,
        input: BaseRulesRoundInput,
    ) -> Result<(BaseRulesRoundReport, CombatStatDiagnosticUndoV1), CombatStatDiagnosticErrorV1>
    {
        // Validate both player selections before inspecting either selected effect. A P2
        // selection error therefore cannot be hidden behind P1's reject-if-selected plan.
        let validated = self.base_rules.validate(input)?;
        for player in PlayerId::ALL {
            let selected = validated[player];
            let card = self.spec.cards[player][selected.slot.index()];
            reject_selected_control(
                player,
                selected.slot,
                CombatStatEffectSourceV1::Ability,
                card.ability,
            )?;
            reject_selected_control(
                player,
                selected.slot,
                CombatStatEffectSourceV1::Bonus,
                card.bonus,
            )?;
        }
        let rounds_played = self.base_rules.position().rounds_played;
        let prepared = prepare_combat_stat_diagnostic(
            validated,
            &self.spec.cards,
            input.first_mover,
            rounds_played,
        )?;
        let (report, base_rules) = self.base_rules.commit(input, prepared);
        Ok((report, CombatStatDiagnosticUndoV1 { base_rules }))
    }

    pub fn unmake(&mut self, undo: CombatStatDiagnosticUndoV1) {
        self.base_rules.unmake(undo.base_rules);
    }
}

fn source_plan_id(plan: CombatStatSourcePlanV1) -> Option<u32> {
    match plan {
        CombatStatSourcePlanV1::Absent => None,
        CombatStatSourcePlanV1::Execute { source_id, .. }
        | CombatStatSourcePlanV1::Disabled { source_id }
        | CombatStatSourcePlanV1::RejectIfSelected { source_id } => Some(source_id),
    }
}

fn validate_source_bonus_context(
    player: PlayerId,
    hand_slot: HandSlot,
    cards: &[CombatStatCardPlanV1; HAND_SIZE],
) -> Result<(), CombatStatPlanErrorV1> {
    let source_id = source_plan_id(cards[hand_slot.index()].bonus);
    let expected = if let Some(source_id) = source_id {
        let mut ids = [0_u32; HAND_SIZE];
        let mut count = 0_usize;
        for card in cards {
            if source_plan_id(card.bonus) == Some(source_id) && !ids[..count].contains(&card.key.id)
            {
                ids[count] = card.key.id;
                count += 1;
            }
        }
        count as u16
    } else {
        0
    };
    let actual = cards[hand_slot.index()].source_bonus_support_count;
    if actual == expected {
        Ok(())
    } else {
        Err(CombatStatPlanErrorV1::InvalidSourceBonusContext {
            player,
            hand_slot,
            source_id,
            expected,
            actual,
        })
    }
}

fn validate_combat_stat_source_plan(
    player: PlayerId,
    hand_slot: HandSlot,
    source: CombatStatEffectSourceV1,
    plan: CombatStatSourcePlanV1,
) -> Result<(), CombatStatPlanErrorV1> {
    let CombatStatSourcePlanV1::Execute {
        source_id,
        predicate,
        effect,
    } = plan
    else {
        return Ok(());
    };
    if !matches!(effect, CombatStatEffectV1::ModifyCombatStat { .. })
        && predicate != CombatStatPredicateV1::Always
    {
        return Err(invalid_combat_stat_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidCombatStatPlanReasonV1::ConditionalControl,
        ));
    }
    let CombatStatEffectV1::ModifyCombatStat {
        side,
        operation,
        value,
        minimum,
        maximum,
        multiplier,
        ..
    } = effect
    else {
        return Ok(());
    };
    if matches!(
        multiplier,
        CombatStatMagnitudeV1::Growth | CombatStatMagnitudeV1::Degrowth
    ) && predicate != CombatStatPredicateV1::Always
    {
        return Err(invalid_combat_stat_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidCombatStatPlanReasonV1::CompoundPredicateAndMagnitude,
        ));
    }
    if source == CombatStatEffectSourceV1::Bonus
        && (matches!(
            predicate,
            CombatStatPredicateV1::OwnerMovesFirst | CombatStatPredicateV1::OwnerMovesSecond
        ) || (matches!(
            predicate,
            CombatStatPredicateV1::SelectedHandSlotsMatch
                | CombatStatPredicateV1::SelectedHandSlotsDiffer
        ) && multiplier != CombatStatMagnitudeV1::Fixed))
    {
        return Err(invalid_combat_stat_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidCombatStatPlanReasonV1::ConditionalBonus,
        ));
    }
    if source == CombatStatEffectSourceV1::Ability
        && multiplier == CombatStatMagnitudeV1::SourceBonusSupport
    {
        return Err(invalid_combat_stat_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidCombatStatPlanReasonV1::SupportAbility,
        ));
    }
    if operation == CombatStatOperationV1::Increase && maximum.is_some() {
        return Err(invalid_combat_stat_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidCombatStatPlanReasonV1::CappedIncrease,
        ));
    }
    if value == 0 {
        return Err(invalid_combat_stat_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidCombatStatPlanReasonV1::ZeroMagnitude,
        ));
    }
    if !matches!(
        (side, operation),
        (
            CombatStatAffectedSideV1::Player,
            CombatStatOperationV1::Increase
        ) | (
            CombatStatAffectedSideV1::Opponent,
            CombatStatOperationV1::Decrease
        )
    ) {
        return Err(invalid_combat_stat_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidCombatStatPlanReasonV1::InvalidModifierDirection,
        ));
    }
    if (operation == CombatStatOperationV1::Increase && minimum.is_some())
        || (operation == CombatStatOperationV1::Decrease && maximum.is_some())
    {
        return Err(invalid_combat_stat_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidCombatStatPlanReasonV1::IncompatibleBounds,
        ));
    }
    Ok(())
}

fn invalid_combat_stat_execute(
    player: PlayerId,
    hand_slot: HandSlot,
    source: CombatStatEffectSourceV1,
    source_id: u32,
    reason: InvalidCombatStatPlanReasonV1,
) -> CombatStatPlanErrorV1 {
    CombatStatPlanErrorV1::InvalidExecute {
        player,
        hand_slot,
        source,
        source_id,
        reason,
    }
}

fn reject_selected_control(
    player: PlayerId,
    hand_slot: HandSlot,
    source: CombatStatEffectSourceV1,
    plan: CombatStatSourcePlanV1,
) -> Result<(), CombatStatDiagnosticErrorV1> {
    if let CombatStatSourcePlanV1::RejectIfSelected { source_id } = plan {
        Err(CombatStatDiagnosticErrorV1::UnsupportedSelectedHazard {
            player,
            hand_slot,
            source,
            source_id,
        })
    } else {
        Ok(())
    }
}

fn active_effect(
    plan: CombatStatSourcePlanV1,
    owner: PlayerId,
    first_mover: PlayerId,
    owner_slot: HandSlot,
    opponent_slot: HandSlot,
) -> Option<CombatStatEffectV1> {
    match plan {
        CombatStatSourcePlanV1::Execute {
            predicate, effect, ..
        } if predicate_matches(predicate, owner, first_mover, owner_slot, opponent_slot) => {
            Some(effect)
        }
        CombatStatSourcePlanV1::Absent
        | CombatStatSourcePlanV1::Disabled { .. }
        | CombatStatSourcePlanV1::RejectIfSelected { .. }
        | CombatStatSourcePlanV1::Execute { .. } => None,
    }
}

fn predicate_matches(
    predicate: CombatStatPredicateV1,
    owner: PlayerId,
    first_mover: PlayerId,
    owner_slot: HandSlot,
    opponent_slot: HandSlot,
) -> bool {
    match predicate {
        CombatStatPredicateV1::Always => true,
        CombatStatPredicateV1::OwnerMovesFirst => owner == first_mover,
        CombatStatPredicateV1::OwnerMovesSecond => owner != first_mover,
        CombatStatPredicateV1::SelectedHandSlotsMatch => owner_slot == opponent_slot,
        CombatStatPredicateV1::SelectedHandSlotsDiffer => owner_slot != opponent_slot,
    }
}

fn prepare_combat_stat_diagnostic(
    validated: ByPlayer<ValidatedSelection>,
    cards: &ByPlayer<[CombatStatCardPlanV1; HAND_SIZE]>,
    first_mover: PlayerId,
    rounds_played: u8,
) -> Result<ByPlayer<PreparedSelection>, CombatStatDiagnosticErrorV1> {
    let selected = ByPlayer::new(
        cards[PlayerId::P1][validated[PlayerId::P1].slot.index()],
        cards[PlayerId::P2][validated[PlayerId::P2].slot.index()],
    );
    let plans = ByPlayer::new(
        resolution_card_plan(
            selected[PlayerId::P1],
            PlayerId::P1,
            first_mover,
            validated[PlayerId::P1].slot,
            validated[PlayerId::P2].slot,
        ),
        resolution_card_plan(
            selected[PlayerId::P2],
            PlayerId::P2,
            first_mover,
            validated[PlayerId::P2].slot,
            validated[PlayerId::P1].slot,
        ),
    );
    prepare_combat_resolution(validated, plans, rounds_played).map_err(map_resolution_error)
}

fn resolution_card_plan(
    plan: CombatStatCardPlanV1,
    owner: PlayerId,
    first_mover: PlayerId,
    owner_slot: HandSlot,
    opponent_slot: HandSlot,
) -> ResolutionCardPlan {
    ResolutionCardPlan {
        ability: ResolutionSourcePlan {
            effect: active_effect(plan.ability, owner, first_mover, owner_slot, opponent_slot)
                .map(shared_effect),
            // Ability Support is rejected by plan validation and can never consume the
            // captured source-bonus Support count.
            support_count: 0,
        },
        bonus: ResolutionSourcePlan {
            effect: active_effect(plan.bonus, owner, first_mover, owner_slot, opponent_slot)
                .map(shared_effect),
            support_count: plan.source_bonus_support_count,
        },
    }
}

fn shared_effect(effect: CombatStatEffectV1) -> DiagnosticCombatEffectV1 {
    match effect {
        CombatStatEffectV1::ModifyCombatStat {
            side,
            stat,
            operation,
            value,
            minimum,
            maximum,
            multiplier,
        } => DiagnosticCombatEffectV1::ModifyCombatStat {
            side: match side {
                CombatStatAffectedSideV1::Opponent => DiagnosticAffectedSideV1::Opponent,
                CombatStatAffectedSideV1::Player => DiagnosticAffectedSideV1::Player,
            },
            stat: match stat {
                CombatStatAttributeV1::Attack => DiagnosticCombatStatV1::Attack,
                CombatStatAttributeV1::Damage => DiagnosticCombatStatV1::Damage,
                CombatStatAttributeV1::Power => DiagnosticCombatStatV1::Power,
                CombatStatAttributeV1::PowerAndDamage => DiagnosticCombatStatV1::PowerAndDamage,
            },
            operation: match operation {
                CombatStatOperationV1::Decrease => DiagnosticStatOperationV1::Decrease,
                CombatStatOperationV1::Increase => DiagnosticStatOperationV1::Increase,
            },
            value,
            minimum,
            maximum,
            multiplier: match multiplier {
                CombatStatMagnitudeV1::Fixed => DiagnosticMagnitudeV1::Fixed,
                CombatStatMagnitudeV1::SourceBonusSupport => {
                    DiagnosticMagnitudeV1::SourceBonusSupport
                }
                CombatStatMagnitudeV1::Growth => DiagnosticMagnitudeV1::Growth,
                CombatStatMagnitudeV1::Degrowth => DiagnosticMagnitudeV1::Degrowth,
            },
        },
        CombatStatEffectV1::StopOpponentBonus => DiagnosticCombatEffectV1::StopOpponentBonus,
        CombatStatEffectV1::CancelOpponentCombatStatModifiers { stat } => {
            DiagnosticCombatEffectV1::CancelOpponentCombatStatModifiers {
                stat: match stat {
                    CombatStatAttributeV1::Attack => DiagnosticCombatStatV1::Attack,
                    CombatStatAttributeV1::Damage => DiagnosticCombatStatV1::Damage,
                    CombatStatAttributeV1::Power => DiagnosticCombatStatV1::Power,
                    CombatStatAttributeV1::PowerAndDamage => DiagnosticCombatStatV1::PowerAndDamage,
                },
            }
        }
    }
}

fn map_resolution_error(error: CombatResolutionError) -> CombatStatDiagnosticErrorV1 {
    let stage = match error.stage {
        CombatResolutionArithmeticStage::EffectMagnitude => {
            CombatStatArithmeticStageV1::EffectMagnitude
        }
        CombatResolutionArithmeticStage::Power => CombatStatArithmeticStageV1::Power,
        CombatResolutionArithmeticStage::Damage => CombatStatArithmeticStageV1::Damage,
        CombatResolutionArithmeticStage::Attack => CombatStatArithmeticStageV1::Attack,
    };
    CombatStatDiagnosticErrorV1::ArithmeticOverflow {
        player: error.player,
        stage,
    }
}
