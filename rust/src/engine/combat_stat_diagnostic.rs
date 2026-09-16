//! Explicit ordinary combat-stat diagnostic projection.
//!
//! This module owns compact effect plans and hot-path resolution. Capture-specific
//! preparation and rich disposition metadata live in the replay diagnostic module.

use super::combat_resolution::{
    prepare_combat_resolution_with_post_round, CombatResolutionArithmeticStage,
    CombatResolutionError, PreparedCombatResolution, ResolutionCardPlan, ResolutionSourcePlan,
};
use super::combat_stat_compiler::victory_or_defeat_pillz_identity_matches;
use super::{
    BaseRulesError, BaseRulesGame, BaseRulesMatchSpec, BaseRulesPosition, BaseRulesRoundInput,
    BaseRulesRoundReport, BaseRulesUndo, ByPlayer, DiagnosticAffectedSideV1,
    DiagnosticCombatEffectV1, DiagnosticCombatStatV1, DiagnosticMagnitudeV1,
    DiagnosticStatOperationV1, HandSlot, PlayerId, PostRoundEffect, ValidatedSelection, HAND_SIZE,
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
    /// Legacy public name retained for source compatibility; abilities and bonuses both
    /// use this magnitude with their own independently validated Support counts.
    SourceBonusSupport,
    Growth,
    Degrowth,
    OpponentStars,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CombatStatPredicateV1 {
    Always,
    OwnerMovesFirst,
    OwnerMovesSecond,
    OwnerWonPreviousRound,
    OwnerLostPreviousRound,
    SelectedHandSlotsMatch,
    SelectedHandSlotsDiffer,
}

/// Public provenance metadata for an admitted post-round effect. The hot path converts this
/// fixed effect into its private typed commit plan; its numeric rule is not configurable.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CombatStatPostRoundEffectV1 {
    RecoverPaidPillzOnDefeat,
    GainOnePillzOnVictoryOrDefeat,
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
    /// Fixed non-stat post-round work. Identity and source are checked at plan
    /// construction; its values are intentionally not caller-configurable.
    RecoverPaidPillzOnDefeat,
    /// Fixed end-of-round resource work. It is neither a combat modifier nor configurable
    /// public data: a direct plan must use one exact audited source/id pair.
    GainOnePillzOnVictoryOrDefeat,
}

/// Compact per-source disposition consumed in the engine hot path. Rich descriptions and
/// preparation policy remain at the outer replay or catalog boundary. `source_id` is an
/// opaque diagnostic identity; replay plans use capture ids and catalog plans use the
/// resolved registry-definition id.
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
    /// Clan identity after immutable whole-draw rules such as Oculus infiltration.
    /// This is deliberately separate from the canonical clan retained by base rules.
    pub effective_clan_id: u32,
    pub ability: CombatStatSourcePlanV1,
    pub bonus: CombatStatSourcePlanV1,
    /// Distinct character ids sharing this card's effective clan across the immutable
    /// whole draw when its bonus is active; otherwise zero.
    pub source_bonus_support_count: u16,
    /// Distinct character ids sharing this card's effective clan across the immutable
    /// whole draw when its executable ability has Support magnitude; otherwise zero.
    pub source_ability_support_count: u16,
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
    DefeatRecoveryIdentity,
    DefeatRecoveryPredicate,
    VictoryOrDefeatIdentity,
    VictoryOrDefeatPredicate,
    /// The ability uses Support outside the unconditional basic-stat subset admitted by
    /// this projection. The legacy variant name is retained for source compatibility.
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
    InvalidAbilitySupportContext {
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
            Self::InvalidAbilitySupportContext {
                player,
                hand_slot,
                source_id,
                expected,
                actual,
            } => write!(
                formatter,
                "invalid combat-stat diagnostic ability Support context for {player:?} slot {} source {source_id:?}: expected {expected} distinct effective-clan character ids, got {actual}",
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
            Self::InvalidSourceBonusContext { .. }
            | Self::InvalidAbilitySupportContext { .. }
            | Self::InvalidExecute { .. } => None,
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
                validate_source_bonus_context(player, slot, &spec.cards[player])?;
                validate_ability_support_context(player, slot, &spec.cards[player])?;
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
        let previous_round_winner = self.base_rules.position().previous_round_winner;
        let prepared = prepare_combat_stat_diagnostic(
            validated,
            &self.spec.cards,
            input.first_mover,
            rounds_played,
            previous_round_winner,
        )?;
        let (report, base_rules) =
            self.base_rules
                .commit(input, prepared.selections, prepared.post_round)?;
        Ok((report, CombatStatDiagnosticUndoV1 { base_rules }))
    }

    pub fn unmake(&mut self, undo: CombatStatDiagnosticUndoV1) {
        self.base_rules.unmake(undo.base_rules);
    }
}

fn validate_ability_support_context(
    player: PlayerId,
    hand_slot: HandSlot,
    cards: &[CombatStatCardPlanV1; HAND_SIZE],
) -> Result<(), CombatStatPlanErrorV1> {
    let plan = cards[hand_slot.index()].ability;
    let source_id = source_plan_id(plan);
    let expected = matches!(
        plan,
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::ModifyCombatStat {
                multiplier: CombatStatMagnitudeV1::SourceBonusSupport,
                ..
            },
            ..
        }
    )
    .then(|| effective_clan_character_count(hand_slot, cards))
    .unwrap_or(0);
    let actual = cards[hand_slot.index()].source_ability_support_count;
    if actual == expected {
        Ok(())
    } else {
        Err(CombatStatPlanErrorV1::InvalidAbilitySupportContext {
            player,
            hand_slot,
            source_id,
            expected,
            actual,
        })
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
    let expected = if source_id.is_some() {
        effective_clan_character_count(hand_slot, cards)
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

fn effective_clan_character_count(
    hand_slot: HandSlot,
    cards: &[CombatStatCardPlanV1; HAND_SIZE],
) -> u16 {
    let effective_clan_id = cards[hand_slot.index()].effective_clan_id;
    let mut ids = [0_u32; HAND_SIZE];
    let mut count = 0_usize;
    for card in cards {
        if card.effective_clan_id == effective_clan_id && !ids[..count].contains(&card.key.id) {
            ids[count] = card.key.id;
            count += 1;
        }
    }
    count as u16
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
    if effect == CombatStatEffectV1::RecoverPaidPillzOnDefeat {
        if !matches!(
            (source, source_id),
            (CombatStatEffectSourceV1::Ability, 729 | 1418)
                | (CombatStatEffectSourceV1::Bonus, 577)
        ) {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::DefeatRecoveryIdentity,
            ));
        }
        if predicate != CombatStatPredicateV1::Always {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::DefeatRecoveryPredicate,
            ));
        }
        return Ok(());
    }
    if effect == CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat {
        if !victory_or_defeat_pillz_identity_matches(source, source_id) {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOrDefeatIdentity,
            ));
        }
        if predicate != CombatStatPredicateV1::Always {
            return Err(invalid_combat_stat_execute(
                player,
                hand_slot,
                source,
                source_id,
                InvalidCombatStatPlanReasonV1::VictoryOrDefeatPredicate,
            ));
        }
        return Ok(());
    }
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
        stat,
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
    if source == CombatStatEffectSourceV1::Ability
        && multiplier == CombatStatMagnitudeV1::SourceBonusSupport
        && (predicate != CombatStatPredicateV1::Always
            || stat == CombatStatAttributeV1::PowerAndDamage)
    {
        return Err(invalid_combat_stat_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidCombatStatPlanReasonV1::SupportAbility,
        ));
    }
    if matches!(
        multiplier,
        CombatStatMagnitudeV1::Growth
            | CombatStatMagnitudeV1::Degrowth
            | CombatStatMagnitudeV1::OpponentStars
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
                | CombatStatPredicateV1::OwnerWonPreviousRound
                | CombatStatPredicateV1::OwnerLostPreviousRound
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
    previous_round_winner: Option<PlayerId>,
) -> Option<CombatStatEffectV1> {
    match plan {
        CombatStatSourcePlanV1::Execute {
            predicate, effect, ..
        } if predicate_matches(
            predicate,
            owner,
            first_mover,
            owner_slot,
            opponent_slot,
            previous_round_winner,
        ) =>
        {
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
    previous_round_winner: Option<PlayerId>,
) -> bool {
    match predicate {
        CombatStatPredicateV1::Always => true,
        CombatStatPredicateV1::OwnerMovesFirst => owner == first_mover,
        CombatStatPredicateV1::OwnerMovesSecond => owner != first_mover,
        CombatStatPredicateV1::OwnerWonPreviousRound => previous_round_winner == Some(owner),
        CombatStatPredicateV1::OwnerLostPreviousRound => {
            previous_round_winner == Some(owner.other())
        }
        CombatStatPredicateV1::SelectedHandSlotsMatch => owner_slot == opponent_slot,
        CombatStatPredicateV1::SelectedHandSlotsDiffer => owner_slot != opponent_slot,
    }
}

fn prepare_combat_stat_diagnostic(
    validated: ByPlayer<ValidatedSelection>,
    cards: &ByPlayer<[CombatStatCardPlanV1; HAND_SIZE]>,
    first_mover: PlayerId,
    rounds_played: u8,
    previous_round_winner: Option<PlayerId>,
) -> Result<PreparedCombatResolution, CombatStatDiagnosticErrorV1> {
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
            previous_round_winner,
        ),
        resolution_card_plan(
            selected[PlayerId::P2],
            PlayerId::P2,
            first_mover,
            validated[PlayerId::P2].slot,
            validated[PlayerId::P1].slot,
            previous_round_winner,
        ),
    );
    prepare_combat_resolution_with_post_round(validated, plans, rounds_played)
        .map_err(map_resolution_error)
}

fn resolution_card_plan(
    plan: CombatStatCardPlanV1,
    owner: PlayerId,
    first_mover: PlayerId,
    owner_slot: HandSlot,
    opponent_slot: HandSlot,
    previous_round_winner: Option<PlayerId>,
) -> ResolutionCardPlan {
    ResolutionCardPlan {
        ability: ResolutionSourcePlan {
            effect: active_effect(
                plan.ability,
                owner,
                first_mover,
                owner_slot,
                opponent_slot,
                previous_round_winner,
            )
            .and_then(shared_effect),
            post_round: active_effect(
                plan.ability,
                owner,
                first_mover,
                owner_slot,
                opponent_slot,
                previous_round_winner,
            )
            .and_then(shared_post_round_effect),
            support_count: plan.source_ability_support_count,
        },
        bonus: ResolutionSourcePlan {
            effect: active_effect(
                plan.bonus,
                owner,
                first_mover,
                owner_slot,
                opponent_slot,
                previous_round_winner,
            )
            .and_then(shared_effect),
            post_round: active_effect(
                plan.bonus,
                owner,
                first_mover,
                owner_slot,
                opponent_slot,
                previous_round_winner,
            )
            .and_then(shared_post_round_effect),
            support_count: plan.source_bonus_support_count,
        },
    }
}

fn shared_effect(effect: CombatStatEffectV1) -> Option<DiagnosticCombatEffectV1> {
    Some(match effect {
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
                CombatStatMagnitudeV1::OpponentStars => DiagnosticMagnitudeV1::OpponentStars,
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
        CombatStatEffectV1::RecoverPaidPillzOnDefeat
        | CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat => return None,
    })
}

fn shared_post_round_effect(effect: CombatStatEffectV1) -> Option<PostRoundEffect> {
    match effect {
        CombatStatEffectV1::RecoverPaidPillzOnDefeat => {
            Some(PostRoundEffect::RecoverPaidPillzOnDefeat)
        }
        CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat => {
            Some(PostRoundEffect::GainOnePillzOnVictoryOrDefeat)
        }
        CombatStatEffectV1::ModifyCombatStat { .. }
        | CombatStatEffectV1::StopOpponentBonus
        | CombatStatEffectV1::CancelOpponentCombatStatModifiers { .. } => None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::CardKey;
    use crate::engine::{BaseRulesCardSpec, BaseRulesPlayerSpec, BaseRulesSelection, MatchStatus};
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn base_spec(p1_pillz: u16) -> BaseRulesMatchSpec {
        let card = |id, power| BaseRulesCardSpec {
            key: CardKey::new(id, 3),
            clan_id: id,
            power,
            damage: 3,
        };
        BaseRulesMatchSpec {
            battle_rule_id: 10,
            night: false,
            players: ByPlayer::new(
                BaseRulesPlayerSpec {
                    initial_life: 20,
                    initial_pillz: p1_pillz,
                    hand: [card(1, 6), card(2, 6), card(3, 6), card(4, 6)],
                },
                BaseRulesPlayerSpec {
                    initial_life: 20,
                    initial_pillz: 20,
                    hand: [card(11, 31), card(12, 31), card(13, 31), card(14, 31)],
                },
            ),
        }
    }

    fn absent(card: BaseRulesCardSpec) -> CombatStatCardPlanV1 {
        CombatStatCardPlanV1 {
            key: card.key,
            effective_clan_id: card.clan_id,
            ability: CombatStatSourcePlanV1::Absent,
            bonus: CombatStatSourcePlanV1::Absent,
            source_bonus_support_count: 0,
            source_ability_support_count: 0,
        }
    }

    fn spec_with_p1(
        effect_source: CombatStatEffectSourceV1,
        id: u32,
        effect: CombatStatEffectV1,
        p1_pillz: u16,
    ) -> CombatStatDiagnosticMatchSpecV1 {
        let base_rules = base_spec(p1_pillz);
        let mut cards = ByPlayer::new(
            base_rules.players[PlayerId::P1].hand.map(absent),
            base_rules.players[PlayerId::P2].hand.map(absent),
        );
        let plan = CombatStatSourcePlanV1::Execute {
            source_id: id,
            predicate: CombatStatPredicateV1::Always,
            effect,
        };
        match effect_source {
            CombatStatEffectSourceV1::Ability => cards[PlayerId::P1][0].ability = plan,
            CombatStatEffectSourceV1::Bonus => {
                cards[PlayerId::P1][0].bonus = plan;
                cards[PlayerId::P1][0].source_bonus_support_count = 1;
            }
        }
        CombatStatDiagnosticMatchSpecV1 { base_rules, cards }
    }

    fn input(p1_pillz: u16, fury: bool) -> BaseRulesRoundInput {
        BaseRulesRoundInput {
            first_mover: PlayerId::P1,
            selections: ByPlayer::new(
                BaseRulesSelection::new(0, p1_pillz, fury),
                BaseRulesSelection::new(0, 0, false),
            ),
        }
    }

    #[test]
    fn defeat_recovery_uses_fury_inclusive_cost_and_undo_is_exact() {
        let spec = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            729,
            CombatStatEffectV1::RecoverPaidPillzOnDefeat,
            7,
        );
        let mut game = CombatStatDiagnosticV1::new(spec).unwrap();
        let before = game.position().clone();
        let mut hasher = DefaultHasher::new();
        before.hash(&mut hasher);
        let before_hash = hasher.finish();

        // Four selected pillz plus Fury costs seven; losing recovers ceil(7 * 2 / 3) = 5.
        let (report, undo) = game.make(input(4, true)).unwrap();
        assert!(!report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P1].pillz, 5);
        assert_eq!(game.position().players[PlayerId::P1].pillz, 5);
        game.unmake(undo);
        assert_eq!(game.position(), &before);
        let mut restored_hasher = DefaultHasher::new();
        game.position().hash(&mut restored_hasher);
        assert_eq!(restored_hasher.finish(), before_hash);
    }

    #[test]
    fn defeat_recovery_is_source_live_but_not_a_combat_stat_modifier() {
        let mut spec = spec_with_p1(
            CombatStatEffectSourceV1::Bonus,
            577,
            CombatStatEffectV1::RecoverPaidPillzOnDefeat,
            3,
        );
        spec.cards[PlayerId::P2][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1,
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::StopOpponentBonus,
        };
        let mut stopped = CombatStatDiagnosticV1::new(spec).unwrap();
        let (report, _) = stopped.make(input(3, false)).unwrap();
        assert_eq!(report.players[PlayerId::P1].pillz, 0);

        let mut spec = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1418,
            CombatStatEffectV1::RecoverPaidPillzOnDefeat,
            3,
        );
        spec.cards[PlayerId::P2][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1,
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::CancelOpponentCombatStatModifiers {
                stat: CombatStatAttributeV1::PowerAndDamage,
            },
        };
        let mut uncancelled = CombatStatDiagnosticV1::new(spec).unwrap();
        let (report, _) = uncancelled.make(input(3, false)).unwrap();
        assert_eq!(report.players[PlayerId::P1].pillz, 2);

        let mut minimum = CombatStatDiagnosticV1::new(spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1418,
            CombatStatEffectV1::RecoverPaidPillzOnDefeat,
            0,
        ))
        .unwrap();
        let (report, _) = minimum.make(input(0, false)).unwrap();
        assert_eq!(report.players[PlayerId::P1].pillz, 1);

        // The two independently active sources represent two END events, so each exact
        // recovery applies after the same paid cost.
        let mut double = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            729,
            CombatStatEffectV1::RecoverPaidPillzOnDefeat,
            3,
        );
        double.cards[PlayerId::P1][0].bonus = CombatStatSourcePlanV1::Execute {
            source_id: 577,
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::RecoverPaidPillzOnDefeat,
        };
        double.cards[PlayerId::P1][0].source_bonus_support_count = 1;
        let mut double = CombatStatDiagnosticV1::new(double).unwrap();
        let (report, _) = double.make(input(3, false)).unwrap();
        assert_eq!(report.players[PlayerId::P1].pillz, 4);
    }

    #[test]
    fn defeat_recovery_requires_a_round_loss_and_still_runs_after_a_tie_break_or_ko() {
        let mut winning_spec = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1418,
            CombatStatEffectV1::RecoverPaidPillzOnDefeat,
            3,
        );
        winning_spec.base_rules.players[PlayerId::P1].hand[0].power = 40;
        let mut winner = CombatStatDiagnosticV1::new(winning_spec).unwrap();
        let (report, _) = winner.make(input(3, false)).unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P1].pillz, 0);

        let mut tied_spec = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1418,
            CombatStatEffectV1::RecoverPaidPillzOnDefeat,
            0,
        );
        tied_spec.base_rules.players[PlayerId::P2].hand[0].power = 6;
        let mut tied = CombatStatDiagnosticV1::new(tied_spec).unwrap();
        let (report, _) = tied
            .make(BaseRulesRoundInput {
                first_mover: PlayerId::P2,
                selections: ByPlayer::new(
                    BaseRulesSelection::new(0, 0, false),
                    BaseRulesSelection::new(0, 0, false),
                ),
            })
            .unwrap();
        assert!(!report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P1].pillz, 1);

        let mut ko_spec = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1418,
            CombatStatEffectV1::RecoverPaidPillzOnDefeat,
            3,
        );
        ko_spec.base_rules.players[PlayerId::P1].initial_life = 2;
        let mut ko = CombatStatDiagnosticV1::new(ko_spec).unwrap();
        let (report, _) = ko.make(input(3, false)).unwrap();
        assert_eq!(report.players[PlayerId::P1].life, 0);
        assert_eq!(report.players[PlayerId::P1].pillz, 2);
        assert_eq!(report.status, MatchStatus::Won(PlayerId::P2));
    }

    #[test]
    fn defeat_recovery_public_plans_are_identity_locked_and_overflow_is_atomic() {
        let wrong_identity = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            2475,
            CombatStatEffectV1::RecoverPaidPillzOnDefeat,
            3,
        );
        assert!(matches!(
            CombatStatDiagnosticV1::new(wrong_identity),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::DefeatRecoveryIdentity,
                ..
            })
        ));

        let mut wrong_predicate = spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1418,
            CombatStatEffectV1::RecoverPaidPillzOnDefeat,
            3,
        );
        wrong_predicate.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1418,
            predicate: CombatStatPredicateV1::OwnerLostPreviousRound,
            effect: CombatStatEffectV1::RecoverPaidPillzOnDefeat,
        };
        assert!(matches!(
            CombatStatDiagnosticV1::new(wrong_predicate),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::DefeatRecoveryPredicate,
                ..
            })
        ));

        let mut overflow = CombatStatDiagnosticV1::new(spec_with_p1(
            CombatStatEffectSourceV1::Ability,
            1418,
            CombatStatEffectV1::RecoverPaidPillzOnDefeat,
            u16::MAX,
        ))
        .unwrap();
        let before = overflow.position().clone();
        assert!(matches!(
            overflow.make(input(0, false)),
            Err(CombatStatDiagnosticErrorV1::BaseRules(
                BaseRulesError::PillzRecoveryOverflow {
                    player: PlayerId::P1
                }
            ))
        ));
        assert_eq!(overflow.position(), &before);
    }

    #[test]
    fn victory_or_defeat_gains_one_after_winning_losing_or_a_ko_and_undo_is_exact() {
        let effect = CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat;

        let mut winner_spec = spec_with_p1(CombatStatEffectSourceV1::Bonus, 1034, effect, 3);
        winner_spec.base_rules.players[PlayerId::P1].hand[0].power = 40;
        let mut winner = CombatStatDiagnosticV1::new(winner_spec).unwrap();
        let before = winner.position().clone();
        let mut before_hasher = DefaultHasher::new();
        before.hash(&mut before_hasher);
        let before_hash = before_hasher.finish();
        let (report, undo) = winner.make(input(3, false)).unwrap();
        assert!(report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P1].pillz, 1);
        winner.unmake(undo);
        assert_eq!(winner.position(), &before);
        let mut restored_hasher = DefaultHasher::new();
        winner.position().hash(&mut restored_hasher);
        assert_eq!(restored_hasher.finish(), before_hash);

        let mut loser = CombatStatDiagnosticV1::new(spec_with_p1(
            CombatStatEffectSourceV1::Bonus,
            1034,
            effect,
            3,
        ))
        .unwrap();
        let (report, _) = loser.make(input(3, false)).unwrap();
        assert!(!report.cards[PlayerId::P1].won);
        assert_eq!(report.players[PlayerId::P1].pillz, 1);

        let mut ko_spec = spec_with_p1(CombatStatEffectSourceV1::Bonus, 1034, effect, 3);
        ko_spec.base_rules.players[PlayerId::P1].initial_life = 2;
        let mut ko = CombatStatDiagnosticV1::new(ko_spec).unwrap();
        let (report, _) = ko.make(input(3, false)).unwrap();
        assert_eq!(report.status, MatchStatus::Won(PlayerId::P2));
        assert_eq!(report.players[PlayerId::P1].life, 0);
        assert_eq!(report.players[PlayerId::P1].pillz, 1);

        // The active Bonus:1034 is applied before the static Ability:1375, after the
        // KO damage has resolved. Both effects are still committed atomically with the
        // round and undo restores the byte-identical starting position.
        let mut both_spec = spec_with_p1(CombatStatEffectSourceV1::Bonus, 1034, effect, 3);
        both_spec.base_rules.players[PlayerId::P1].initial_life = 2;
        both_spec.cards[PlayerId::P1][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1375,
            predicate: CombatStatPredicateV1::Always,
            effect,
        };
        let mut both = CombatStatDiagnosticV1::new(both_spec).unwrap();
        let before = both.position().clone();
        let mut before_hasher = DefaultHasher::new();
        before.hash(&mut before_hasher);
        let before_hash = before_hasher.finish();
        let (report, undo) = both.make(input(3, false)).unwrap();
        assert_eq!(report.status, MatchStatus::Won(PlayerId::P2));
        assert_eq!(report.players[PlayerId::P1].life, 0);
        assert_eq!(report.players[PlayerId::P1].pillz, 2);
        both.unmake(undo);
        assert_eq!(both.position(), &before);
        let mut restored_hasher = DefaultHasher::new();
        both.position().hash(&mut restored_hasher);
        assert_eq!(restored_hasher.finish(), before_hash);
    }

    #[test]
    fn victory_or_defeat_bonus_is_stopped_by_stop_bonus_but_not_reinterpreted_by_cancellation() {
        let effect = CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat;
        let mut stopped_spec = spec_with_p1(CombatStatEffectSourceV1::Bonus, 1034, effect, 3);
        stopped_spec.cards[PlayerId::P2][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1,
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::StopOpponentBonus,
        };
        let mut stopped = CombatStatDiagnosticV1::new(stopped_spec).unwrap();
        let (report, _) = stopped.make(input(3, false)).unwrap();
        assert_eq!(report.players[PlayerId::P1].pillz, 0);

        let mut cancelled_spec = spec_with_p1(CombatStatEffectSourceV1::Bonus, 1034, effect, 3);
        cancelled_spec.cards[PlayerId::P2][0].ability = CombatStatSourcePlanV1::Execute {
            source_id: 1,
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::CancelOpponentCombatStatModifiers {
                stat: CombatStatAttributeV1::PowerAndDamage,
            },
        };
        let mut cancelled = CombatStatDiagnosticV1::new(cancelled_spec).unwrap();
        let (report, _) = cancelled.make(input(3, false)).unwrap();
        assert_eq!(report.players[PlayerId::P1].pillz, 1);
    }

    #[test]
    fn victory_or_defeat_public_plans_are_exact_and_overflow_is_atomic() {
        let effect = CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat;
        for (source, id) in [
            (CombatStatEffectSourceV1::Bonus, 1034),
            (CombatStatEffectSourceV1::Ability, 1034),
            (CombatStatEffectSourceV1::Ability, 1375),
            (CombatStatEffectSourceV1::Ability, 4111),
            (CombatStatEffectSourceV1::Ability, 5085),
            (CombatStatEffectSourceV1::Ability, 5520),
        ] {
            assert!(CombatStatDiagnosticV1::new(spec_with_p1(source, id, effect, 3)).is_ok());
        }
        for (source, id) in [
            (CombatStatEffectSourceV1::Bonus, 1375),
            (CombatStatEffectSourceV1::Bonus, 4111),
        ] {
            assert!(matches!(
                CombatStatDiagnosticV1::new(spec_with_p1(source, id, effect, 3)),
                Err(CombatStatPlanErrorV1::InvalidExecute {
                    reason: InvalidCombatStatPlanReasonV1::VictoryOrDefeatIdentity,
                    ..
                })
            ));
        }
        let mut wrong_predicate = spec_with_p1(CombatStatEffectSourceV1::Bonus, 1034, effect, 3);
        wrong_predicate.cards[PlayerId::P1][0].bonus = CombatStatSourcePlanV1::Execute {
            source_id: 1034,
            predicate: CombatStatPredicateV1::OwnerWonPreviousRound,
            effect,
        };
        assert!(matches!(
            CombatStatDiagnosticV1::new(wrong_predicate),
            Err(CombatStatPlanErrorV1::InvalidExecute {
                reason: InvalidCombatStatPlanReasonV1::VictoryOrDefeatPredicate,
                ..
            })
        ));

        let mut overflow = CombatStatDiagnosticV1::new(spec_with_p1(
            CombatStatEffectSourceV1::Bonus,
            1034,
            effect,
            u16::MAX,
        ))
        .unwrap();
        let before = overflow.position().clone();
        assert!(matches!(
            overflow.make(input(0, false)),
            Err(CombatStatDiagnosticErrorV1::BaseRules(
                BaseRulesError::PillzIncreaseOverflow {
                    player: PlayerId::P1
                }
            ))
        ));
        assert_eq!(overflow.position(), &before);
    }
}
