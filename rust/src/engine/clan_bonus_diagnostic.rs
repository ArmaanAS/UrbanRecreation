//! Explicit first-effect diagnostic projection.
//!
//! This module owns compact effect plans and hot-path resolution. Capture-specific
//! preparation and rich disposition metadata live in the replay diagnostic module.

use super::{
    BaseRulesCardResult, BaseRulesError, BaseRulesGame, BaseRulesMatchSpec, BaseRulesPosition,
    BaseRulesRoundInput, BaseRulesRoundReport, BaseRulesUndo, ByPlayer, HandSlot, PlayerId,
    PreparedSelection, ValidatedSelection, FURY_DAMAGE, HAND_SIZE,
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
    SourceBonusSupport,
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
    StopOpponentBonus,
    CancelOpponentCombatStatModifiers {
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
        let prepared = prepare_clan_bonus_diagnostic(validated, &self.spec.cards)?;
        let (report, base_rules) = self.base_rules.commit(input, prepared);
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

#[derive(Clone, Copy, Default)]
struct DiagnosticCancellationMask {
    attack: bool,
    damage: bool,
    power: bool,
}

impl DiagnosticCancellationMask {
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

fn executing_effect(plan: DiagnosticSourcePlanV1) -> Option<DiagnosticCombatEffectV1> {
    match plan {
        DiagnosticSourcePlanV1::Execute { effect, .. } => Some(effect),
        DiagnosticSourcePlanV1::Absent
        | DiagnosticSourcePlanV1::Disabled { .. }
        | DiagnosticSourcePlanV1::RejectIfSelected { .. } => None,
    }
}

fn is_stop_bonus(effect: Option<DiagnosticCombatEffectV1>) -> bool {
    matches!(effect, Some(DiagnosticCombatEffectV1::StopOpponentBonus))
}

fn add_cancellation(
    mask: &mut DiagnosticCancellationMask,
    effect: Option<DiagnosticCombatEffectV1>,
) {
    if let Some(DiagnosticCombatEffectV1::CancelOpponentCombatStatModifiers { stat }) = effect {
        mask.insert(stat);
    }
}

fn prepare_clan_bonus_diagnostic(
    validated: ByPlayer<ValidatedSelection>,
    cards: &ByPlayer<[DiagnosticCardPlanV1; HAND_SIZE]>,
) -> Result<ByPlayer<PreparedSelection>, ClanBonusDiagnosticError> {
    let selected_plans = ByPlayer::new(
        cards[PlayerId::P1][validated[PlayerId::P1].slot.index()],
        cards[PlayerId::P2][validated[PlayerId::P2].slot.index()],
    );
    let mut bonus_live = ByPlayer::new(
        executing_effect(selected_plans[PlayerId::P1].bonus).is_some(),
        executing_effect(selected_plans[PlayerId::P2].bonus).is_some(),
    );

    // Ability-origin Stop Bonus is outside the bonus-vs-bonus dependency and therefore
    // resolves first in this deliberately narrow projection.
    for player in PlayerId::ALL {
        if is_stop_bonus(executing_effect(selected_plans[player].ability)) {
            bonus_live[player.other()] = false;
        }
    }

    // Surviving bonus-origin Stop Bonus is simultaneous. Snapshot before applying either
    // result so player iteration order cannot change a mutual Stop Bonus outcome.
    let bonus_stops = ByPlayer::new(
        bonus_live[PlayerId::P1]
            && is_stop_bonus(executing_effect(selected_plans[PlayerId::P1].bonus)),
        bonus_live[PlayerId::P2]
            && is_stop_bonus(executing_effect(selected_plans[PlayerId::P2].bonus)),
    );
    if bonus_stops[PlayerId::P1] {
        bonus_live[PlayerId::P2] = false;
    }
    if bonus_stops[PlayerId::P2] {
        bonus_live[PlayerId::P1] = false;
    }

    let mut cancellations = ByPlayer::new(
        DiagnosticCancellationMask::default(),
        DiagnosticCancellationMask::default(),
    );
    for player in PlayerId::ALL {
        add_cancellation(
            &mut cancellations[player],
            executing_effect(selected_plans[player].ability),
        );
        if bonus_live[player] {
            add_cancellation(
                &mut cancellations[player],
                executing_effect(selected_plans[player].bonus),
            );
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

    // Own Power/Damage bonuses resolve before opponent reductions and their Min clamps.
    for origin in PlayerId::ALL {
        if bonus_live[origin] {
            apply_power_damage_effect(
                origin,
                DiagnosticAffectedSideV1::Player,
                DiagnosticStatOperationV1::Increase,
                executing_effect(selected_plans[origin].bonus),
                selected_plans[origin].source_bonus_support_count,
                cancellations[origin.other()],
                &mut power,
                &mut damage,
            )?;
        }
    }
    for origin in PlayerId::ALL {
        if bonus_live[origin] {
            apply_power_damage_effect(
                origin,
                DiagnosticAffectedSideV1::Opponent,
                DiagnosticStatOperationV1::Decrease,
                executing_effect(selected_plans[origin].bonus),
                selected_plans[origin].source_bonus_support_count,
                cancellations[origin.other()],
                &mut power,
                &mut damage,
            )?;
        }
    }

    // The server and current TypeScript engine add Fury after damage modifiers.
    for player in PlayerId::ALL {
        if validated[player].selection.fury {
            damage[player] = damage[player].checked_add(FURY_DAMAGE).ok_or(
                ClanBonusDiagnosticError::ArithmeticOverflow {
                    player,
                    stage: DiagnosticArithmeticStageV1::Damage,
                },
            )?;
        }
    }

    let mut attack = ByPlayer::new(0_u32, 0_u32);
    for player in PlayerId::ALL {
        attack[player] = u32::from(power[player])
            .checked_mul(u32::from(validated[player].selection.pillz) + 1)
            .ok_or(ClanBonusDiagnosticError::ArithmeticOverflow {
                player,
                stage: DiagnosticArithmeticStageV1::Attack,
            })?;
    }
    for origin in PlayerId::ALL {
        if bonus_live[origin] {
            apply_attack_effect(
                origin,
                DiagnosticAffectedSideV1::Player,
                DiagnosticStatOperationV1::Increase,
                executing_effect(selected_plans[origin].bonus),
                selected_plans[origin].source_bonus_support_count,
                cancellations[origin.other()],
                &mut attack,
            )?;
        }
    }
    for origin in PlayerId::ALL {
        if bonus_live[origin] {
            apply_attack_effect(
                origin,
                DiagnosticAffectedSideV1::Opponent,
                DiagnosticStatOperationV1::Decrease,
                executing_effect(selected_plans[origin].bonus),
                selected_plans[origin].source_bonus_support_count,
                cancellations[origin.other()],
                &mut attack,
            )?;
        }
    }

    Ok(ByPlayer::new(
        finish_diagnostic_selection(
            validated[PlayerId::P1],
            power[PlayerId::P1],
            damage[PlayerId::P1],
            attack[PlayerId::P1],
        ),
        finish_diagnostic_selection(
            validated[PlayerId::P2],
            power[PlayerId::P2],
            damage[PlayerId::P2],
            attack[PlayerId::P2],
        ),
    ))
}

fn finish_diagnostic_selection(
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

#[allow(clippy::too_many_arguments)]
fn apply_power_damage_effect(
    origin: PlayerId,
    expected_side: DiagnosticAffectedSideV1,
    expected_operation: DiagnosticStatOperationV1,
    effect: Option<DiagnosticCombatEffectV1>,
    support_count: u16,
    opponent_cancellation: DiagnosticCancellationMask,
    power: &mut ByPlayer<u16>,
    damage: &mut ByPlayer<u16>,
) -> Result<(), ClanBonusDiagnosticError> {
    let Some(DiagnosticCombatEffectV1::ModifyCombatStat {
        side,
        stat,
        operation,
        value,
        minimum,
        maximum,
        multiplier,
    }) = effect
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
    let amount = diagnostic_effect_amount(origin, value, multiplier, support_count)?;
    if affects_power && !opponent_cancellation.contains(DiagnosticCombatStatV1::Power) {
        power[target] = apply_u16_modifier(
            origin,
            DiagnosticArithmeticStageV1::Power,
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
            DiagnosticArithmeticStageV1::Damage,
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
    effect: Option<DiagnosticCombatEffectV1>,
    support_count: u16,
    opponent_cancellation: DiagnosticCancellationMask,
    attack: &mut ByPlayer<u32>,
) -> Result<(), ClanBonusDiagnosticError> {
    let Some(DiagnosticCombatEffectV1::ModifyCombatStat {
        side,
        stat: DiagnosticCombatStatV1::Attack,
        operation,
        value,
        minimum,
        maximum,
        multiplier,
    }) = effect
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
    let amount = diagnostic_effect_amount(origin, value, multiplier, support_count)?;
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

fn diagnostic_effect_amount(
    player: PlayerId,
    value: u16,
    multiplier: DiagnosticMagnitudeV1,
    support_count: u16,
) -> Result<u32, ClanBonusDiagnosticError> {
    let multiplier = match multiplier {
        DiagnosticMagnitudeV1::Fixed => 1,
        DiagnosticMagnitudeV1::SourceBonusSupport => u32::from(support_count),
    };
    u32::from(value)
        .checked_mul(multiplier)
        .ok_or(ClanBonusDiagnosticError::ArithmeticOverflow {
            player,
            stage: DiagnosticArithmeticStageV1::EffectMagnitude,
        })
}

fn apply_u16_modifier(
    player: PlayerId,
    stage: DiagnosticArithmeticStageV1,
    current: u16,
    operation: DiagnosticStatOperationV1,
    amount: u32,
    minimum: Option<u16>,
    maximum: Option<u16>,
) -> Result<u16, ClanBonusDiagnosticError> {
    let current = u32::from(current);
    let next = match operation {
        DiagnosticStatOperationV1::Increase => match maximum.map(u32::from) {
            Some(maximum) if current < maximum => current
                .checked_add(amount)
                .ok_or(ClanBonusDiagnosticError::ArithmeticOverflow { player, stage })?
                .min(maximum),
            Some(_) => current,
            None => current
                .checked_add(amount)
                .ok_or(ClanBonusDiagnosticError::ArithmeticOverflow { player, stage })?,
        },
        DiagnosticStatOperationV1::Decrease => match minimum.map(u32::from) {
            Some(minimum) if current > minimum => current.saturating_sub(amount).max(minimum),
            Some(_) => current,
            None => current.saturating_sub(amount),
        },
    };
    u16::try_from(next).map_err(|_| ClanBonusDiagnosticError::ArithmeticOverflow { player, stage })
}

fn apply_u32_modifier(
    player: PlayerId,
    current: u32,
    operation: DiagnosticStatOperationV1,
    amount: u32,
    minimum: Option<u32>,
    maximum: Option<u32>,
) -> Result<u32, ClanBonusDiagnosticError> {
    match operation {
        DiagnosticStatOperationV1::Increase => {
            match maximum {
                Some(maximum) if current < maximum => current
                    .checked_add(amount)
                    .ok_or(ClanBonusDiagnosticError::ArithmeticOverflow {
                        player,
                        stage: DiagnosticArithmeticStageV1::Attack,
                    })
                    .map(|value| value.min(maximum)),
                Some(_) => Ok(current),
                None => current.checked_add(amount).ok_or(
                    ClanBonusDiagnosticError::ArithmeticOverflow {
                        player,
                        stage: DiagnosticArithmeticStageV1::Attack,
                    },
                ),
            }
        }
        DiagnosticStatOperationV1::Decrease => Ok(match minimum {
            Some(minimum) if current > minimum => current.saturating_sub(amount).max(minimum),
            Some(_) => current,
            None => current.saturating_sub(amount),
        }),
    }
}
