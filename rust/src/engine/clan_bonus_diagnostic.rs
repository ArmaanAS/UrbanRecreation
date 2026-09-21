//! Explicit first-effect diagnostic projection.
//!
//! This module owns compact effect plans and hot-path resolution. Capture-specific
//! preparation and rich disposition metadata live in the replay diagnostic module.

use super::combat_resolution::{
    prepare_combat_resolution, CombatResolutionArithmeticStage, CombatResolutionError,
    ResolutionCardPlan, ResolutionSourcePlan,
};
use super::{
    BaseRulesError, BaseRulesGame, BaseRulesMatchSpec, BaseRulesPosition, BaseRulesRoundInput,
    BaseRulesRoundReport, BaseRulesUndo, ByPlayer, HandSlot, PlayerId, PostRoundPlan,
    PreparedSelection, ValidatedSelection, HAND_SIZE,
};
use crate::catalog::CardKey;
use std::error::Error;
use std::fmt;

/// The source slot of a projected effect. This is part of rule identity: a numeric
/// modifier attached as an ability is deliberately disabled by the diagnostic projection,
/// while the same modifier captured as an active bonus is executable.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticEffectSourceV1 {
    Ability,
    Bonus,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticCombatStatV1 {
    Attack,
    Damage,
    Power,
    PowerAndDamage,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticAffectedSideV1 {
    Opponent,
    Player,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticStatOperationV1 {
    Decrease,
    Increase,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticMagnitudeV1 {
    Fixed,
    /// Legacy public name retained for source compatibility; the shared resolver also
    /// uses this magnitude for ability Support with a separate count.
    SourceBonusSupport,
    Growth,
    Degrowth,
    OpponentStars,
    /// Scaled by the opposing selected card's resolved Damage, before Fury.
    OpponentDamage,
    /// `Brawl:`. Scaled by the number of distinct characters in the opposing hand sharing
    /// the opposing selected card's effective clan - the mirror of Support, which counts
    /// the owner's own hand.
    AntiSupport,
}

/// String-free execution primitives admitted by the first diagnostic projection.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DiagnosticCombatEffectV1 {
    ModifyCombatStat {
        side: DiagnosticAffectedSideV1,
        stat: DiagnosticCombatStatV1,
        operation: DiagnosticStatOperationV1,
        value: u16,
        minimum: Option<u16>,
        maximum: Option<u16>,
        multiplier: DiagnosticMagnitudeV1,
    },
    StopOpponentAbility,
    StopOpponentBonus,
    CancelOpponentCombatStatModifiers {
        stat: DiagnosticCombatStatV1,
    },
    /// The owner's own stat cannot be reduced by the opposing selected card.
    ProtectOwnCombatStat {
        stat: DiagnosticCombatStatV1,
    },
    /// The owner's own Ability survives an opposing Stop.
    ProtectOwnAbility,
    /// The owner's own Bonus survives an opposing Stop.
    ProtectOwnBonus,
    /// The owner's own stat is replaced by the opposing selected card's printed value.
    CopyOpponentPrintedCombatStat {
        stat: DiagnosticCombatStatV1,
    },
}

/// Compact per-source disposition consumed in the engine hot path. Rich descriptions and
/// disabled reasons remain at the outer replay-diagnostic boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DiagnosticSourcePlanV1 {
    Absent,
    Execute {
        source_id: u32,
        effect: DiagnosticCombatEffectV1,
    },
    Disabled {
        source_id: u32,
    },
    RejectIfSelected {
        source_id: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DiagnosticCardPlanV1 {
    pub key: CardKey,
    pub ability: DiagnosticSourcePlanV1,
    pub bonus: DiagnosticSourcePlanV1,
    /// Distinct captured character ids sharing this card's active source-bonus id across
    /// the immutable whole draw. This is capture context, not inferred clan membership.
    pub source_bonus_support_count: u16,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ClanBonusDiagnosticMatchSpecV1 {
    pub base_rules: BaseRulesMatchSpec,
    pub cards: ByPlayer<[DiagnosticCardPlanV1; HAND_SIZE]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticPlanMismatch {
    pub player: PlayerId,
    pub hand_slot: HandSlot,
    pub expected: CardKey,
    pub actual: CardKey,
}

impl fmt::Display for DiagnosticPlanMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "clan-bonus diagnostic plan for {:?} slot {} has {:?}, expected {:?}",
            self.player,
            self.hand_slot.get(),
            self.actual,
            self.expected
        )
    }
}

impl Error for DiagnosticPlanMismatch {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidDiagnosticPlanReasonV1 {
    AbilityCombatModifier,
    IncompatibleBounds,
    InvalidModifierDirection,
    OpponentStarsMagnitude,
    /// This older projection has no opposing-hand context, so a Brawl magnitude cannot be
    /// evaluated here however a caller labels it.
    AntiSupportMagnitude,
    RoundScaledMagnitude,
    ZeroMagnitude,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticPlanErrorV1 {
    CardMismatch(DiagnosticPlanMismatch),
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
        source: DiagnosticEffectSourceV1,
        source_id: u32,
        reason: InvalidDiagnosticPlanReasonV1,
    },
}

impl fmt::Display for DiagnosticPlanErrorV1 {
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
                "invalid clan-bonus diagnostic source-bonus context for {player:?} slot {} source {source_id:?}: expected {expected} distinct character ids, got {actual}",
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
                "invalid clan-bonus diagnostic Execute plan for {player:?} slot {} {source:?} {source_id}: {reason:?}",
                hand_slot.get()
            ),
        }
    }
}

impl Error for DiagnosticPlanErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CardMismatch(source) => Some(source),
            Self::InvalidSourceBonusContext { .. } | Self::InvalidExecute { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticArithmeticStageV1 {
    EffectMagnitude,
    Power,
    Damage,
    Attack,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClanBonusDiagnosticError {
    BaseRules(BaseRulesError),
    UnsupportedSelectedControl {
        player: PlayerId,
        hand_slot: HandSlot,
        source: DiagnosticEffectSourceV1,
        source_id: u32,
    },
    ArithmeticOverflow {
        player: PlayerId,
        stage: DiagnosticArithmeticStageV1,
    },
}

impl fmt::Display for ClanBonusDiagnosticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BaseRules(source) => source.fmt(formatter),
            Self::UnsupportedSelectedControl {
                player,
                hand_slot,
                source,
                source_id,
            } => write!(
                formatter,
                "clan-bonus diagnostic cannot select {player:?} slot {}: unsupported {source:?} control {source_id}",
                hand_slot.get()
            ),
            Self::ArithmeticOverflow { player, stage } => write!(
                formatter,
                "clan-bonus diagnostic {stage:?} arithmetic overflow for {player:?}"
            ),
        }
    }
}

impl Error for ClanBonusDiagnosticError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::BaseRules(source) => Some(source),
            Self::UnsupportedSelectedControl { .. } | Self::ArithmeticOverflow { .. } => None,
        }
    }
}

impl From<BaseRulesError> for ClanBonusDiagnosticError {
    fn from(source: BaseRulesError) -> Self {
        Self::BaseRules(source)
    }
}

/// Mode-specific, single-use undo token.
#[derive(Debug, Eq, PartialEq)]
pub struct ClanBonusDiagnosticUndoV1 {
    base_rules: BaseRulesUndo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClanBonusDiagnostic {
    spec: ClanBonusDiagnosticMatchSpecV1,
    base_rules: BaseRulesGame,
}

impl ClanBonusDiagnostic {
    pub fn new(spec: ClanBonusDiagnosticMatchSpecV1) -> Result<Self, DiagnosticPlanErrorV1> {
        // Validate identity independently of source context so a malformed plan always
        // reports the fundamental card mismatch first.
        for player in PlayerId::ALL {
            for slot in HandSlot::ALL {
                let expected = spec.base_rules.players[player].hand[slot.index()].key;
                let actual = spec.cards[player][slot.index()].key;
                if actual != expected {
                    return Err(DiagnosticPlanErrorV1::CardMismatch(
                        DiagnosticPlanMismatch {
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
                validate_diagnostic_source_plan(
                    player,
                    slot,
                    DiagnosticEffectSourceV1::Ability,
                    spec.cards[player][slot.index()].ability,
                )?;
                validate_diagnostic_source_plan(
                    player,
                    slot,
                    DiagnosticEffectSourceV1::Bonus,
                    spec.cards[player][slot.index()].bonus,
                )?;
            }
        }
        let base_rules = BaseRulesGame::new(spec.base_rules.clone());
        Ok(Self { spec, base_rules })
    }

    pub fn match_spec(&self) -> &ClanBonusDiagnosticMatchSpecV1 {
        &self.spec
    }

    pub fn base_rules_spec(&self) -> &BaseRulesMatchSpec {
        &self.spec.base_rules
    }

    pub fn position(&self) -> &BaseRulesPosition {
        self.base_rules.position()
    }

    pub fn card_plans(&self) -> &ByPlayer<[DiagnosticCardPlanV1; HAND_SIZE]> {
        &self.spec.cards
    }

    pub fn make(
        &mut self,
        input: BaseRulesRoundInput,
    ) -> Result<(BaseRulesRoundReport, ClanBonusDiagnosticUndoV1), ClanBonusDiagnosticError> {
        // Validate both player selections before inspecting either selected effect. A P2
        // selection error therefore cannot be hidden behind P1's reject-if-selected plan.
        let validated = self.base_rules.validate(input)?;
        for player in PlayerId::ALL {
            let selected = validated[player];
            let card = self.spec.cards[player][selected.slot.index()];
            reject_selected_control(
                player,
                selected.slot,
                DiagnosticEffectSourceV1::Ability,
                card.ability,
            )?;
            reject_selected_control(
                player,
                selected.slot,
                DiagnosticEffectSourceV1::Bonus,
                card.bonus,
            )?;
        }
        let rounds_played = self.base_rules.position().rounds_played;
        let prepared = prepare_clan_bonus_diagnostic(validated, &self.spec.cards, rounds_played)?;
        let (report, base_rules) = self.base_rules.commit(
            input,
            prepared,
            ByPlayer::new(PostRoundPlan::default(), PostRoundPlan::default()),
        )?;
        Ok((report, ClanBonusDiagnosticUndoV1 { base_rules }))
    }

    pub fn unmake(&mut self, undo: ClanBonusDiagnosticUndoV1) {
        self.base_rules.unmake(undo.base_rules);
    }
}

fn source_plan_id(plan: DiagnosticSourcePlanV1) -> Option<u32> {
    match plan {
        DiagnosticSourcePlanV1::Absent => None,
        DiagnosticSourcePlanV1::Execute { source_id, .. }
        | DiagnosticSourcePlanV1::Disabled { source_id }
        | DiagnosticSourcePlanV1::RejectIfSelected { source_id } => Some(source_id),
    }
}

fn validate_source_bonus_context(
    player: PlayerId,
    hand_slot: HandSlot,
    cards: &[DiagnosticCardPlanV1; HAND_SIZE],
) -> Result<(), DiagnosticPlanErrorV1> {
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
        Err(DiagnosticPlanErrorV1::InvalidSourceBonusContext {
            player,
            hand_slot,
            source_id,
            expected,
            actual,
        })
    }
}

fn validate_diagnostic_source_plan(
    player: PlayerId,
    hand_slot: HandSlot,
    source: DiagnosticEffectSourceV1,
    plan: DiagnosticSourcePlanV1,
) -> Result<(), DiagnosticPlanErrorV1> {
    let DiagnosticSourcePlanV1::Execute { source_id, effect } = plan else {
        return Ok(());
    };
    let DiagnosticCombatEffectV1::ModifyCombatStat {
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
    if source == DiagnosticEffectSourceV1::Ability {
        return Err(invalid_diagnostic_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidDiagnosticPlanReasonV1::AbilityCombatModifier,
        ));
    }
    if multiplier == DiagnosticMagnitudeV1::OpponentStars {
        return Err(invalid_diagnostic_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidDiagnosticPlanReasonV1::OpponentStarsMagnitude,
        ));
    }
    if multiplier == DiagnosticMagnitudeV1::AntiSupport {
        return Err(invalid_diagnostic_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidDiagnosticPlanReasonV1::AntiSupportMagnitude,
        ));
    }
    if matches!(
        multiplier,
        DiagnosticMagnitudeV1::Growth | DiagnosticMagnitudeV1::Degrowth
    ) {
        return Err(invalid_diagnostic_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidDiagnosticPlanReasonV1::RoundScaledMagnitude,
        ));
    }
    if value == 0 {
        return Err(invalid_diagnostic_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidDiagnosticPlanReasonV1::ZeroMagnitude,
        ));
    }
    if !matches!(
        (side, operation),
        (
            DiagnosticAffectedSideV1::Player,
            DiagnosticStatOperationV1::Increase
        ) | (
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease
        )
    ) {
        return Err(invalid_diagnostic_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidDiagnosticPlanReasonV1::InvalidModifierDirection,
        ));
    }
    if (operation == DiagnosticStatOperationV1::Increase && minimum.is_some())
        || (operation == DiagnosticStatOperationV1::Decrease && maximum.is_some())
    {
        return Err(invalid_diagnostic_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidDiagnosticPlanReasonV1::IncompatibleBounds,
        ));
    }
    Ok(())
}

fn invalid_diagnostic_execute(
    player: PlayerId,
    hand_slot: HandSlot,
    source: DiagnosticEffectSourceV1,
    source_id: u32,
    reason: InvalidDiagnosticPlanReasonV1,
) -> DiagnosticPlanErrorV1 {
    DiagnosticPlanErrorV1::InvalidExecute {
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
    source: DiagnosticEffectSourceV1,
    plan: DiagnosticSourcePlanV1,
) -> Result<(), ClanBonusDiagnosticError> {
    if let DiagnosticSourcePlanV1::RejectIfSelected { source_id } = plan {
        Err(ClanBonusDiagnosticError::UnsupportedSelectedControl {
            player,
            hand_slot,
            source,
            source_id,
        })
    } else {
        Ok(())
    }
}

fn executing_effect(plan: DiagnosticSourcePlanV1) -> Option<DiagnosticCombatEffectV1> {
    match plan {
        DiagnosticSourcePlanV1::Execute { effect, .. } => Some(effect),
        DiagnosticSourcePlanV1::Absent
        | DiagnosticSourcePlanV1::Disabled { .. }
        | DiagnosticSourcePlanV1::RejectIfSelected { .. } => None,
    }
}

fn prepare_clan_bonus_diagnostic(
    validated: ByPlayer<ValidatedSelection>,
    cards: &ByPlayer<[DiagnosticCardPlanV1; HAND_SIZE]>,
    rounds_played: u8,
) -> Result<ByPlayer<PreparedSelection>, ClanBonusDiagnosticError> {
    let selected_plans = ByPlayer::new(
        resolution_card_plan(cards[PlayerId::P1][validated[PlayerId::P1].slot.index()]),
        resolution_card_plan(cards[PlayerId::P2][validated[PlayerId::P2].slot.index()]),
    );
    prepare_combat_resolution(validated, selected_plans, rounds_played)
        .map_err(map_resolution_error)
}

fn resolution_card_plan(plan: DiagnosticCardPlanV1) -> ResolutionCardPlan {
    ResolutionCardPlan {
        ability: ResolutionSourcePlan {
            // This older projection has no opposing-hand context and admits no Brawl.
            anti_support_count: 0,
            effect: executing_effect(plan.ability),
            post_round: None,
            support_count: 0,
        },
        bonus: ResolutionSourcePlan {
            anti_support_count: 0,
            effect: executing_effect(plan.bonus),
            post_round: None,
            support_count: plan.source_bonus_support_count,
        },
    }
}

fn map_resolution_error(error: CombatResolutionError) -> ClanBonusDiagnosticError {
    let stage = match error.stage {
        CombatResolutionArithmeticStage::EffectMagnitude => {
            DiagnosticArithmeticStageV1::EffectMagnitude
        }
        CombatResolutionArithmeticStage::Power => DiagnosticArithmeticStageV1::Power,
        CombatResolutionArithmeticStage::Damage => DiagnosticArithmeticStageV1::Damage,
        CombatResolutionArithmeticStage::Attack => DiagnosticArithmeticStageV1::Attack,
    };
    ClanBonusDiagnosticError::ArithmeticOverflow {
        player: error.player,
        stage,
    }
}
