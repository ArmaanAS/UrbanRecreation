//! The current, replay-grounded engine.
//!
//! This module intentionally lives alongside the frozen historical implementation. The
//! first vertical slice implements only the effect-free base rules; its names make that
//! limitation explicit so it cannot be mistaken for full ability or bonus support.

use crate::catalog::CardKey;
use std::error::Error;
use std::fmt;

pub const HAND_SIZE: usize = 4;
pub const MAX_ROUNDS: u8 = 4;
pub const FURY_COST: u16 = 3;
pub const FURY_DAMAGE: u16 = 2;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PlayerId {
    P1,
    P2,
}

impl PlayerId {
    pub const ALL: [Self; 2] = [Self::P1, Self::P2];

    pub const fn index(self) -> usize {
        match self {
            Self::P1 => 0,
            Self::P2 => 1,
        }
    }

    pub const fn other(self) -> Self {
        match self {
            Self::P1 => Self::P2,
            Self::P2 => Self::P1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HandSlot(u8);

impl HandSlot {
    pub const ALL: [Self; HAND_SIZE] = [Self(0), Self(1), Self(2), Self(3)];

    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for HandSlot {
    type Error = InvalidHandSlot;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value < HAND_SIZE as u8 {
            Ok(Self(value))
        } else {
            Err(InvalidHandSlot { hand_index: value })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidHandSlot {
    pub hand_index: u8,
}

impl fmt::Display for InvalidHandSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "hand index {} is outside 0..{}",
            self.hand_index,
            HAND_SIZE - 1
        )
    }
}

impl Error for InvalidHandSlot {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ByPlayer<T>(pub [T; 2]);

impl<T> ByPlayer<T> {
    pub const fn new(p1: T, p2: T) -> Self {
        Self([p1, p2])
    }

    pub fn get(&self, player: PlayerId) -> &T {
        &self.0[player.index()]
    }

    pub fn get_mut(&mut self, player: PlayerId) -> &mut T {
        &mut self.0[player.index()]
    }

    pub fn map<U>(self, mut map: impl FnMut(T) -> U) -> ByPlayer<U> {
        let [p1, p2] = self.0;
        ByPlayer::new(map(p1), map(p2))
    }
}

impl<T> std::ops::Index<PlayerId> for ByPlayer<T> {
    type Output = T;

    fn index(&self, player: PlayerId) -> &Self::Output {
        self.get(player)
    }
}

impl<T> std::ops::IndexMut<PlayerId> for ByPlayer<T> {
    fn index_mut(&mut self, player: PlayerId) -> &mut Self::Output {
        self.get_mut(player)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BaseRulesCardSpec {
    pub key: CardKey,
    pub clan_id: u32,
    pub power: u16,
    pub damage: u16,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BaseRulesPlayerSpec {
    pub initial_life: u16,
    pub initial_pillz: u16,
    pub hand: [BaseRulesCardSpec; HAND_SIZE],
}

/// Immutable inputs shared by every branch of a base-rules search.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct BaseRulesMatchSpec {
    pub battle_rule_id: u32,
    pub night: bool,
    pub players: ByPlayer<BaseRulesPlayerSpec>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BaseRulesPlayerState {
    pub life: u16,
    pub pillz: u16,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MatchStatus {
    Playing,
    Won(PlayerId),
    Draw,
}

/// All mutable state. Equality and hashing are intentionally structural for undo and solver
/// checks. A future transposition key must pair this with externally known turn context, such
/// as the next explicit first mover; that information is not derivable from round parity.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct BaseRulesPosition {
    pub players: ByPlayer<BaseRulesPlayerState>,
    pub played: ByPlayer<[bool; HAND_SIZE]>,
    pub rounds_played: u8,
    pub status: MatchStatus,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BaseRulesSelection {
    pub hand_index: u8,
    /// Paid pillz only. The free attack pill is implicit.
    pub pillz: u16,
    pub fury: bool,
}

impl BaseRulesSelection {
    pub const fn new(hand_index: u8, pillz: u16, fury: bool) -> Self {
        Self {
            hand_index,
            pillz,
            fury,
        }
    }
}

/// One atomic round input. Selections are keyed by player, independent of submission order.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BaseRulesRoundInput {
    pub first_mover: PlayerId,
    pub selections: ByPlayer<BaseRulesSelection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BaseRulesCardResult {
    pub key: CardKey,
    pub hand_slot: HandSlot,
    pub power: u16,
    pub damage: u16,
    pub attack: u32,
    pub won: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaseRulesRoundReport {
    pub round: u8,
    pub first_mover: PlayerId,
    pub selections: ByPlayer<BaseRulesSelection>,
    pub cards: ByPlayer<BaseRulesCardResult>,
    pub players: ByPlayer<BaseRulesPlayerState>,
    pub status: MatchStatus,
}

/// Snapshot undo is simple and exact; no field is reconstructed from the report.
///
/// An undo token belongs to the game and branch that produced it. Consume tokens on that
/// game in reverse `make` order. Using a token with another game or after a sibling move is
/// outside this API's contract.
#[derive(Debug, Eq, PartialEq)]
pub struct BaseRulesUndo {
    before: BaseRulesPosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BaseRulesError {
    MatchFinished {
        status: MatchStatus,
    },
    RoundLimitReached {
        rounds_played: u8,
    },
    InvalidHandSlot {
        player: PlayerId,
        hand_index: u8,
    },
    CardAlreadyPlayed {
        player: PlayerId,
        hand_slot: HandSlot,
    },
    CostOverflow {
        player: PlayerId,
    },
    InsufficientPillz {
        player: PlayerId,
        available: u16,
        required: u16,
    },
    AttackOverflow {
        player: PlayerId,
    },
    DamageOverflow {
        player: PlayerId,
    },
}

impl fmt::Display for BaseRulesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MatchFinished { status } => write!(formatter, "match is already {status:?}"),
            Self::RoundLimitReached { rounds_played } => {
                write!(
                    formatter,
                    "round limit reached after {rounds_played} rounds"
                )
            }
            Self::InvalidHandSlot { player, hand_index } => {
                write!(
                    formatter,
                    "{player:?} hand index {hand_index} is outside 0..3"
                )
            }
            Self::CardAlreadyPlayed { player, hand_slot } => write!(
                formatter,
                "{player:?} already played hand slot {}",
                hand_slot.get()
            ),
            Self::CostOverflow { player } => {
                write!(formatter, "{player:?} selection cost overflow")
            }
            Self::InsufficientPillz {
                player,
                available,
                required,
            } => write!(
                formatter,
                "{player:?} has {available} pillz but selection costs {required}"
            ),
            Self::AttackOverflow { player } => write!(formatter, "{player:?} attack overflow"),
            Self::DamageOverflow { player } => write!(formatter, "{player:?} damage overflow"),
        }
    }
}

impl Error for BaseRulesError {}

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
    MissingActiveBonusContext,
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
    base_rules: BaseRulesGame,
    cards: ByPlayer<[DiagnosticCardPlanV1; HAND_SIZE]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaseRulesGame {
    spec: BaseRulesMatchSpec,
    position: BaseRulesPosition,
}

#[derive(Clone, Copy)]
struct PreparedSelection {
    slot: HandSlot,
    cost: u16,
    card: BaseRulesCardSpec,
    result: BaseRulesCardResult,
}

#[derive(Clone, Copy)]
struct ValidatedSelection {
    slot: HandSlot,
    cost: u16,
    card: BaseRulesCardSpec,
    selection: BaseRulesSelection,
}

impl BaseRulesGame {
    pub fn new(spec: BaseRulesMatchSpec) -> Self {
        let players = ByPlayer::new(
            BaseRulesPlayerState {
                life: spec.players[PlayerId::P1].initial_life,
                pillz: spec.players[PlayerId::P1].initial_pillz,
            },
            BaseRulesPlayerState {
                life: spec.players[PlayerId::P2].initial_life,
                pillz: spec.players[PlayerId::P2].initial_pillz,
            },
        );
        let status = initial_status(&players);
        Self {
            spec,
            position: BaseRulesPosition {
                players,
                played: ByPlayer::new([false; HAND_SIZE], [false; HAND_SIZE]),
                rounds_played: 0,
                status,
            },
        }
    }

    pub fn spec(&self) -> &BaseRulesMatchSpec {
        &self.spec
    }

    pub fn position(&self) -> &BaseRulesPosition {
        &self.position
    }

    pub fn make(
        &mut self,
        input: BaseRulesRoundInput,
    ) -> Result<(BaseRulesRoundReport, BaseRulesUndo), BaseRulesError> {
        // Preserve the historical base-rules error precedence: each player is fully
        // validated and prepared before looking at the next player.
        self.validate_round_state()?;
        let p1 = self.validate_player(input, PlayerId::P1)?;
        let p1 = prepare_base_rules_selection(PlayerId::P1, p1)?;
        let p2 = self.validate_player(input, PlayerId::P2)?;
        let p2 = prepare_base_rules_selection(PlayerId::P2, p2)?;
        let prepared = ByPlayer::new(p1, p2);
        Ok(self.commit(input, prepared))
    }

    pub fn unmake(&mut self, undo: BaseRulesUndo) {
        self.position = undo.before;
    }

    fn commit(
        &mut self,
        input: BaseRulesRoundInput,
        prepared: ByPlayer<PreparedSelection>,
    ) -> (BaseRulesRoundReport, BaseRulesUndo) {
        let undo = BaseRulesUndo {
            before: self.position.clone(),
        };
        let round = self.position.rounds_played;

        for player in PlayerId::ALL {
            let selected = prepared[player];
            self.position.players[player].pillz -= selected.cost;
            self.position.played[player][selected.slot.index()] = true;
        }

        let winner = round_winner(input.first_mover, &prepared);
        let loser = winner.other();
        self.position.players[loser].life = self.position.players[loser]
            .life
            .saturating_sub(prepared[winner].result.damage);
        self.position.rounds_played += 1;
        self.position.status = status_after_round(&self.position);

        let mut results = prepared.map(|selection| selection.result);
        results[winner].won = true;
        let report = BaseRulesRoundReport {
            round,
            first_mover: input.first_mover,
            selections: input.selections,
            cards: results,
            players: self.position.players,
            status: self.position.status,
        };
        (report, undo)
    }

    fn validate(
        &self,
        input: BaseRulesRoundInput,
    ) -> Result<ByPlayer<ValidatedSelection>, BaseRulesError> {
        self.validate_round_state()?;
        Ok(ByPlayer::new(
            self.validate_player(input, PlayerId::P1)?,
            self.validate_player(input, PlayerId::P2)?,
        ))
    }

    fn validate_round_state(&self) -> Result<(), BaseRulesError> {
        if self.position.rounds_played >= MAX_ROUNDS {
            return Err(BaseRulesError::RoundLimitReached {
                rounds_played: self.position.rounds_played,
            });
        }
        if self.position.status != MatchStatus::Playing {
            return Err(BaseRulesError::MatchFinished {
                status: self.position.status,
            });
        }
        Ok(())
    }

    fn validate_player(
        &self,
        input: BaseRulesRoundInput,
        player: PlayerId,
    ) -> Result<ValidatedSelection, BaseRulesError> {
        let selection = input.selections[player];
        let slot = HandSlot::try_from(selection.hand_index).map_err(|_| {
            BaseRulesError::InvalidHandSlot {
                player,
                hand_index: selection.hand_index,
            }
        })?;
        if self.position.played[player][slot.index()] {
            return Err(BaseRulesError::CardAlreadyPlayed {
                player,
                hand_slot: slot,
            });
        }
        let fury_cost = if selection.fury { FURY_COST } else { 0 };
        let cost = selection
            .pillz
            .checked_add(fury_cost)
            .ok_or(BaseRulesError::CostOverflow { player })?;
        let available = self.position.players[player].pillz;
        if cost > available {
            return Err(BaseRulesError::InsufficientPillz {
                player,
                available,
                required: cost,
            });
        }
        let card = self.spec.players[player].hand[slot.index()];
        Ok(ValidatedSelection {
            slot,
            cost,
            card,
            selection,
        })
    }
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
                    spec.cards[player][slot.index()].source_bonus_support_count,
                )?;
                validate_diagnostic_source_plan(
                    player,
                    slot,
                    DiagnosticEffectSourceV1::Bonus,
                    spec.cards[player][slot.index()].bonus,
                    spec.cards[player][slot.index()].source_bonus_support_count,
                )?;
            }
        }
        Ok(Self {
            base_rules: BaseRulesGame::new(spec.base_rules),
            cards: spec.cards,
        })
    }

    pub fn spec(&self) -> &BaseRulesMatchSpec {
        self.base_rules.spec()
    }

    pub fn position(&self) -> &BaseRulesPosition {
        self.base_rules.position()
    }

    pub fn card_plans(&self) -> &ByPlayer<[DiagnosticCardPlanV1; HAND_SIZE]> {
        &self.cards
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
            let card = self.cards[player][selected.slot.index()];
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
        let prepared = prepare_clan_bonus_diagnostic(validated, &self.cards)?;
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
    source_bonus_support_count: u16,
) -> Result<(), DiagnosticPlanErrorV1> {
    let DiagnosticSourcePlanV1::Execute { source_id, effect } = plan else {
        return Ok(());
    };
    if source == DiagnosticEffectSourceV1::Bonus && source_bonus_support_count == 0 {
        return Err(invalid_diagnostic_execute(
            player,
            hand_slot,
            source,
            source_id,
            InvalidDiagnosticPlanReasonV1::MissingActiveBonusContext,
        ));
    }
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

fn prepare_base_rules_selection(
    player: PlayerId,
    selected: ValidatedSelection,
) -> Result<PreparedSelection, BaseRulesError> {
    let attack = u32::from(selected.card.power)
        .checked_mul(u32::from(selected.selection.pillz) + 1)
        .ok_or(BaseRulesError::AttackOverflow { player })?;
    let fury_damage = if selected.selection.fury {
        FURY_DAMAGE
    } else {
        0
    };
    let damage = selected
        .card
        .damage
        .checked_add(fury_damage)
        .ok_or(BaseRulesError::DamageOverflow { player })?;
    Ok(PreparedSelection {
        slot: selected.slot,
        cost: selected.cost,
        card: selected.card,
        result: BaseRulesCardResult {
            key: selected.card.key,
            hand_slot: selected.slot,
            power: selected.card.power,
            damage,
            attack,
            won: false,
        },
    })
}

fn round_winner(first_mover: PlayerId, prepared: &ByPlayer<PreparedSelection>) -> PlayerId {
    let p1 = prepared[PlayerId::P1];
    let p2 = prepared[PlayerId::P2];
    match p1.result.attack.cmp(&p2.result.attack) {
        std::cmp::Ordering::Greater => PlayerId::P1,
        std::cmp::Ordering::Less => PlayerId::P2,
        std::cmp::Ordering::Equal => match p1.card.key.level.cmp(&p2.card.key.level) {
            std::cmp::Ordering::Less => PlayerId::P1,
            std::cmp::Ordering::Greater => PlayerId::P2,
            std::cmp::Ordering::Equal => first_mover,
        },
    }
}

fn status_after_round(position: &BaseRulesPosition) -> MatchStatus {
    let p1 = position.players[PlayerId::P1].life;
    let p2 = position.players[PlayerId::P2].life;
    if p1 == 0 || p2 == 0 || position.rounds_played == MAX_ROUNDS {
        match p1.cmp(&p2) {
            std::cmp::Ordering::Greater => MatchStatus::Won(PlayerId::P1),
            std::cmp::Ordering::Less => MatchStatus::Won(PlayerId::P2),
            std::cmp::Ordering::Equal => MatchStatus::Draw,
        }
    } else {
        MatchStatus::Playing
    }
}

fn initial_status(players: &ByPlayer<BaseRulesPlayerState>) -> MatchStatus {
    let p1 = players[PlayerId::P1].life;
    let p2 = players[PlayerId::P2].life;
    if p1 != 0 && p2 != 0 {
        MatchStatus::Playing
    } else {
        match p1.cmp(&p2) {
            std::cmp::Ordering::Greater => MatchStatus::Won(PlayerId::P1),
            std::cmp::Ordering::Less => MatchStatus::Won(PlayerId::P2),
            std::cmp::Ordering::Equal => MatchStatus::Draw,
        }
    }
}
