//! Replay execution for the current engine's effects-disabled base-rules slice.
//!
//! Expected capture outputs are assertion targets only. Match construction and every round
//! input use source identity, canonical card data, and submitted selections exclusively.

use super::model::{
    EnginePlayer, ExpectedCardResult, ReplayCaseV1, ReplayPlay, ReplayRound, REPLAY_SCHEMA_VERSION,
};
use crate::catalog::{CardCatalog, CardKey};
use crate::effect_registry::{
    AffectedSideV1, AttributeActionV1, AttributeAffectedV1, CombatStatV1, CompiledEffectV1,
    EffectLookupError, EffectRegistryV1, MagnitudeMultiplierV1, SpecialActionV1, StatOperationV1,
    SupportedEffectV1, UnsupportedReasonV1,
};
use crate::engine::{
    BaseRulesCardResult, BaseRulesCardSpec, BaseRulesError, BaseRulesGame, BaseRulesMatchSpec,
    BaseRulesPlayerSpec, BaseRulesPosition, BaseRulesRoundInput, BaseRulesRoundReport,
    BaseRulesSelection, ByPlayer, ClanBonusDiagnostic, ClanBonusDiagnosticError,
    ClanBonusDiagnosticMatchSpecV1, DiagnosticAffectedSideV1, DiagnosticCardPlanV1,
    DiagnosticCombatEffectV1, DiagnosticCombatStatV1, DiagnosticEffectSourceV1,
    DiagnosticMagnitudeV1, DiagnosticPlanErrorV1, DiagnosticSourcePlanV1,
    DiagnosticStatOperationV1, PlayerId, HAND_SIZE, MAX_ROUNDS,
};
use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayValidationError {
    pub battle_id: u64,
    pub field: String,
    pub detail: String,
}

impl fmt::Display for ReplayValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "battle {}: invalid {}: {}",
            self.battle_id, self.field, self.detail
        )
    }
}

impl Error for ReplayValidationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BaseRulesReplayRoundContext {
    pub battle_id: u64,
    pub round: u8,
    pub cards: ByPlayer<CardKey>,
    pub selections: ByPlayer<BaseRulesSelection>,
    pub first_mover: PlayerId,
}

impl fmt::Display for BaseRulesReplayRoundContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let p1 = self.cards[PlayerId::P1];
        let p2 = self.cards[PlayerId::P2];
        write!(
            formatter,
            "battle {}, round {}, first={:?}, P1={}@{} {:?}, P2={}@{} {:?}",
            self.battle_id,
            self.round,
            self.first_mover,
            p1.id,
            p1.level,
            self.selections[PlayerId::P1],
            p2.id,
            p2.level,
            self.selections[PlayerId::P2]
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BaseRulesReplayError {
    PrefixOutOfRange {
        battle_id: u64,
        requested: usize,
        available: usize,
    },
    Engine {
        context: BaseRulesReplayRoundContext,
        source: BaseRulesError,
    },
    Mismatch {
        context: BaseRulesReplayRoundContext,
        field: String,
        expected: String,
        actual: String,
    },
}

impl fmt::Display for BaseRulesReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrefixOutOfRange {
                battle_id,
                requested,
                available,
            } => write!(
                formatter,
                "battle {battle_id}: requested {requested} effects-disabled rounds, only {available} available"
            ),
            Self::Engine { context, source } => {
                write!(formatter, "effects-disabled engine error at {context}: {source}")
            }
            Self::Mismatch {
                context,
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "effects-disabled mismatch at {context}: {field}: expected {expected}, actual {actual}"
            ),
        }
    }
}

impl Error for BaseRulesReplayError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Engine { source, .. } => Some(source),
            Self::PrefixOutOfRange { .. } | Self::Mismatch { .. } => None,
        }
    }
}

/// A validated replay plus immutable canonical inputs for effects-disabled execution.
#[derive(Clone, Debug)]
pub struct BaseRulesReplay {
    replay: ReplayCaseV1,
    match_spec: BaseRulesMatchSpec,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaseRulesReplayReport {
    pub battle_id: u64,
    pub effects_enabled: bool,
    pub rounds: Vec<BaseRulesRoundReport>,
    pub final_position: BaseRulesPosition,
}

/// Explicit authorization for the deliberately incomplete replay projection.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ClanBonusDiagnosticProjectionV1 {
    DisableOrdinaryAbilitiesAndOutOfSliceBonuses,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DiagnosticReplayModelV1 {
    ClanBonusDiagnosticV1,
}

impl fmt::Display for DiagnosticReplayModelV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("clan-bonus-diagnostic-v1")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticModifierIdentityV1 {
    pub id: u32,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticDisabledReasonV1 {
    OrdinaryAbility {
        registry_reasons: Box<[UnsupportedReasonV1]>,
    },
    OutOfSliceBonus {
        registry_reasons: Box<[UnsupportedReasonV1]>,
    },
    UnsupportedPromisedControl {
        registry_reasons: Box<[UnsupportedReasonV1]>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticProjectionDispositionV1 {
    Absent,
    Execute {
        identity: DiagnosticModifierIdentityV1,
        effect: SupportedEffectV1,
    },
    Disabled {
        identity: DiagnosticModifierIdentityV1,
        reason: DiagnosticDisabledReasonV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticCardPreparationV1 {
    pub key: CardKey,
    pub source_bonus_support_count: u16,
    pub ability: DiagnosticProjectionDispositionV1,
    pub bonus: DiagnosticProjectionDispositionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClanBonusDiagnosticSelectedCardReportV1 {
    pub key: CardKey,
    pub hand_slot: u8,
    pub source_bonus_support_count: u16,
    pub ability: DiagnosticProjectionDispositionV1,
    pub bonus: DiagnosticProjectionDispositionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClanBonusDiagnosticRoundReportV1 {
    pub model: DiagnosticReplayModelV1,
    pub round: BaseRulesRoundReport,
    pub selected: ByPlayer<ClanBonusDiagnosticSelectedCardReportV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClanBonusDiagnosticReplayReportV1 {
    pub battle_id: u64,
    pub model: DiagnosticReplayModelV1,
    pub rounds: Vec<ClanBonusDiagnosticRoundReportV1>,
    pub final_position: BaseRulesPosition,
}

#[derive(Debug)]
pub enum ClanBonusDiagnosticPreparationErrorV1 {
    Replay(ReplayValidationError),
    Lookup {
        battle_id: u64,
        player: PlayerId,
        hand_slot: u8,
        source_kind: DiagnosticEffectSourceV1,
        source: EffectLookupError,
    },
    UnsupportedCompiledShape {
        battle_id: u64,
        player: PlayerId,
        hand_slot: u8,
        source_kind: DiagnosticEffectSourceV1,
        effect_id: u32,
    },
    EnginePlan(DiagnosticPlanErrorV1),
}

impl fmt::Display for ClanBonusDiagnosticPreparationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Replay(source) => source.fmt(formatter),
            Self::Lookup {
                battle_id,
                player,
                hand_slot,
                source_kind,
                source,
            } => write!(
                formatter,
                "{model} battle {battle_id} {player:?} slot {hand_slot} {source_kind:?} lookup failed: {source}",
                model = DiagnosticReplayModelV1::ClanBonusDiagnosticV1
            ),
            Self::UnsupportedCompiledShape {
                battle_id,
                player,
                hand_slot,
                source_kind,
                effect_id,
            } => write!(
                formatter,
                "{model} battle {battle_id} {player:?} slot {hand_slot} {source_kind:?} effect {effect_id} cannot map to a compact plan",
                model = DiagnosticReplayModelV1::ClanBonusDiagnosticV1
            ),
            Self::EnginePlan(source) => write!(
                formatter,
                "{} preparation failed: {source}",
                DiagnosticReplayModelV1::ClanBonusDiagnosticV1
            ),
        }
    }
}

impl Error for ClanBonusDiagnosticPreparationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Replay(source) => Some(source),
            Self::Lookup { source, .. } => Some(source),
            Self::EnginePlan(source) => Some(source),
            Self::UnsupportedCompiledShape { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClanBonusDiagnosticReplayErrorV1 {
    PrefixOutOfRange {
        battle_id: u64,
        requested: usize,
        available: usize,
    },
    Engine {
        context: BaseRulesReplayRoundContext,
        source: ClanBonusDiagnosticError,
    },
    Mismatch {
        context: BaseRulesReplayRoundContext,
        field: String,
        expected: String,
        actual: String,
    },
}

impl fmt::Display for ClanBonusDiagnosticReplayErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let model = DiagnosticReplayModelV1::ClanBonusDiagnosticV1;
        match self {
            Self::PrefixOutOfRange {
                battle_id,
                requested,
                available,
            } => write!(
                formatter,
                "{model} battle {battle_id}: requested {requested} rounds, only {available} available"
            ),
            Self::Engine { context, source } => {
                write!(formatter, "{model} engine error at {context}: {source}")
            }
            Self::Mismatch {
                context,
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "{model} mismatch at {context}: {field}: expected {expected}, actual {actual}"
            ),
        }
    }
}

impl Error for ClanBonusDiagnosticReplayErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Engine { source, .. } => Some(source),
            Self::PrefixOutOfRange { .. } | Self::Mismatch { .. } => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ClanBonusDiagnosticReplayV1 {
    base: BaseRulesReplay,
    match_spec: ClanBonusDiagnosticMatchSpecV1,
    cards: ByPlayer<[DiagnosticCardPreparationV1; HAND_SIZE]>,
}

impl BaseRulesReplay {
    /// Revalidates the public/deserializable replay model and binds canonical base stats.
    pub fn new(replay: ReplayCaseV1, catalog: &CardCatalog) -> Result<Self, ReplayValidationError> {
        validate_replay(&replay, catalog)?;
        let players = PlayerId::ALL.map(|player| {
            let source = &replay.players[player.index()];
            let hand = std::array::from_fn(|index| {
                let key = source.hand[index].key;
                let canonical = catalog
                    .get(key)
                    .expect("validated catalog identity must remain present");
                BaseRulesCardSpec {
                    key,
                    clan_id: canonical.clan_id,
                    power: u16::from(canonical.power),
                    damage: u16::from(canonical.damage),
                }
            });
            BaseRulesPlayerSpec {
                initial_life: source.base_life,
                initial_pillz: source.base_pillz,
                hand,
            }
        });
        let match_spec = BaseRulesMatchSpec {
            battle_rule_id: replay.metadata.battle_rule_id,
            night: replay.metadata.night,
            players: ByPlayer(players),
        };
        Ok(Self { replay, match_spec })
    }

    pub fn battle_id(&self) -> u64 {
        self.replay.metadata.battle_id
    }

    pub fn replay(&self) -> &ReplayCaseV1 {
        &self.replay
    }

    pub fn match_spec(&self) -> &BaseRulesMatchSpec {
        &self.match_spec
    }

    pub fn new_game(&self) -> BaseRulesGame {
        BaseRulesGame::new(self.match_spec.clone())
    }

    pub fn execute_effects_disabled(&self) -> Result<BaseRulesReplayReport, BaseRulesReplayError> {
        self.execute_effects_disabled_prefix(self.replay.rounds.len())
    }

    pub fn execute_effects_disabled_prefix(
        &self,
        rounds: usize,
    ) -> Result<BaseRulesReplayReport, BaseRulesReplayError> {
        if rounds > self.replay.rounds.len() {
            return Err(BaseRulesReplayError::PrefixOutOfRange {
                battle_id: self.battle_id(),
                requested: rounds,
                available: self.replay.rounds.len(),
            });
        }

        let mut game = self.new_game();
        let mut reports = Vec::with_capacity(rounds);
        for replay_round in &self.replay.rounds[..rounds] {
            let (input, context) = round_input(self.battle_id(), replay_round);
            let (report, _) = game
                .make(input)
                .map_err(|source| BaseRulesReplayError::Engine { context, source })?;
            assert_round(replay_round, &report, context)?;
            reports.push(report);
        }
        Ok(BaseRulesReplayReport {
            battle_id: self.battle_id(),
            effects_enabled: false,
            rounds: reports,
            final_position: game.position().clone(),
        })
    }
}

impl ClanBonusDiagnosticReplayV1 {
    pub fn new(
        replay: ReplayCaseV1,
        catalog: &CardCatalog,
        registry: &EffectRegistryV1,
        _projection: ClanBonusDiagnosticProjectionV1,
    ) -> Result<Self, ClanBonusDiagnosticPreparationErrorV1> {
        let base = BaseRulesReplay::new(replay, catalog)
            .map_err(ClanBonusDiagnosticPreparationErrorV1::Replay)?;
        let battle_id = base.battle_id();
        let cards = prepare_diagnostic_cards(base.replay(), registry, battle_id)?;
        let compact_cards = ByPlayer::new(
            std::array::from_fn(|slot| compact_card_plan(&cards[PlayerId::P1][slot])),
            std::array::from_fn(|slot| compact_card_plan(&cards[PlayerId::P2][slot])),
        );
        let match_spec = ClanBonusDiagnosticMatchSpecV1 {
            base_rules: base.match_spec().clone(),
            cards: compact_cards,
        };
        ClanBonusDiagnostic::new(match_spec.clone())
            .map_err(ClanBonusDiagnosticPreparationErrorV1::EnginePlan)?;
        Ok(Self {
            base,
            match_spec,
            cards,
        })
    }

    pub fn battle_id(&self) -> u64 {
        self.base.battle_id()
    }

    pub fn replay(&self) -> &ReplayCaseV1 {
        self.base.replay()
    }

    pub fn preparation(&self) -> &ByPlayer<[DiagnosticCardPreparationV1; HAND_SIZE]> {
        &self.cards
    }

    pub fn new_game(&self) -> ClanBonusDiagnostic {
        ClanBonusDiagnostic::new(self.match_spec.clone())
            .expect("the immutable diagnostic match plan was validated at construction")
    }

    pub fn execute_clan_bonus_diagnostic_v1(
        &self,
    ) -> Result<ClanBonusDiagnosticReplayReportV1, ClanBonusDiagnosticReplayErrorV1> {
        self.execute_clan_bonus_diagnostic_v1_prefix(self.replay().rounds.len())
    }

    pub fn execute_clan_bonus_diagnostic_v1_prefix(
        &self,
        rounds: usize,
    ) -> Result<ClanBonusDiagnosticReplayReportV1, ClanBonusDiagnosticReplayErrorV1> {
        if rounds > self.replay().rounds.len() {
            return Err(ClanBonusDiagnosticReplayErrorV1::PrefixOutOfRange {
                battle_id: self.battle_id(),
                requested: rounds,
                available: self.replay().rounds.len(),
            });
        }
        let mut game = self.new_game();
        let mut reports = Vec::with_capacity(rounds);
        for replay_round in &self.replay().rounds[..rounds] {
            let (input, context) = round_input(self.battle_id(), replay_round);
            let selected = self.selected_report(input);
            let (report, _) = game
                .make(input)
                .map_err(|source| ClanBonusDiagnosticReplayErrorV1::Engine { context, source })?;
            assert_diagnostic_round(replay_round, &report, context)?;
            reports.push(ClanBonusDiagnosticRoundReportV1 {
                model: DiagnosticReplayModelV1::ClanBonusDiagnosticV1,
                round: report,
                selected,
            });
        }
        Ok(ClanBonusDiagnosticReplayReportV1 {
            battle_id: self.battle_id(),
            model: DiagnosticReplayModelV1::ClanBonusDiagnosticV1,
            rounds: reports,
            final_position: game.position().clone(),
        })
    }

    fn selected_report(
        &self,
        input: BaseRulesRoundInput,
    ) -> ByPlayer<ClanBonusDiagnosticSelectedCardReportV1> {
        let make = |player: PlayerId| {
            let hand_slot = input.selections[player].hand_index;
            let prepared = &self.cards[player][usize::from(hand_slot)];
            ClanBonusDiagnosticSelectedCardReportV1 {
                key: prepared.key,
                hand_slot,
                source_bonus_support_count: prepared.source_bonus_support_count,
                ability: prepared.ability.clone(),
                bonus: prepared.bonus.clone(),
            }
        };
        ByPlayer::new(make(PlayerId::P1), make(PlayerId::P2))
    }
}

fn prepare_diagnostic_cards(
    replay: &ReplayCaseV1,
    registry: &EffectRegistryV1,
    battle_id: u64,
) -> Result<ByPlayer<[DiagnosticCardPreparationV1; HAND_SIZE]>, ClanBonusDiagnosticPreparationErrorV1>
{
    let mut prepared: ByPlayer<[Option<DiagnosticCardPreparationV1>; HAND_SIZE]> =
        ByPlayer::new([const { None }; HAND_SIZE], [const { None }; HAND_SIZE]);
    for player in PlayerId::ALL {
        for slot in 0..HAND_SIZE {
            let card = &replay.players[player.index()].hand[slot];
            let source_bonus_support_count = source_bonus_support_count(
                &replay.players[player.index()].hand,
                card.source_bonus.as_ref().map(|modifier| modifier.id),
            );
            prepared[player][slot] = Some(DiagnosticCardPreparationV1 {
                key: card.key,
                source_bonus_support_count,
                ability: prepare_diagnostic_source(
                    registry,
                    battle_id,
                    player,
                    slot as u8,
                    DiagnosticEffectSourceV1::Ability,
                    card.source_ability.as_ref(),
                )?
                .0,
                bonus: prepare_diagnostic_source(
                    registry,
                    battle_id,
                    player,
                    slot as u8,
                    DiagnosticEffectSourceV1::Bonus,
                    card.source_bonus.as_ref(),
                )?
                .0,
            });
        }
    }
    Ok(ByPlayer::new(
        prepared[PlayerId::P1]
            .clone()
            .map(|card| card.expect("all four P1 cards were prepared")),
        prepared[PlayerId::P2]
            .clone()
            .map(|card| card.expect("all four P2 cards were prepared")),
    ))
}

fn source_bonus_support_count(
    hand: &[super::model::ReplayCard; HAND_SIZE],
    id: Option<u32>,
) -> u16 {
    let Some(id) = id else {
        return 0;
    };
    let mut distinct = [0_u32; HAND_SIZE];
    let mut count = 0_usize;
    for card in hand {
        if card.source_bonus.as_ref().map(|modifier| modifier.id) != Some(id)
            || distinct[..count].contains(&card.key.id)
        {
            continue;
        }
        distinct[count] = card.key.id;
        count += 1;
    }
    count as u16
}

fn prepare_diagnostic_source(
    registry: &EffectRegistryV1,
    battle_id: u64,
    player: PlayerId,
    hand_slot: u8,
    source_kind: DiagnosticEffectSourceV1,
    source: Option<&super::model::SourceModifier>,
) -> Result<
    (DiagnosticProjectionDispositionV1, DiagnosticSourcePlanV1),
    ClanBonusDiagnosticPreparationErrorV1,
> {
    let Some(source) = source else {
        return Ok((
            DiagnosticProjectionDispositionV1::Absent,
            DiagnosticSourcePlanV1::Absent,
        ));
    };
    let definition = registry
        .lookup_capture(source.id, &source.description)
        .map_err(|error| ClanBonusDiagnosticPreparationErrorV1::Lookup {
            battle_id,
            player,
            hand_slot,
            source_kind,
            source: error,
        })?;
    let identity = DiagnosticModifierIdentityV1 {
        id: source.id,
        description: source.description.clone(),
    };
    match definition.compiled() {
        CompiledEffectV1::Supported(effect) => {
            let execute = source_kind == DiagnosticEffectSourceV1::Bonus
                || matches!(
                    effect,
                    SupportedEffectV1::StopOpponentBonus
                        | SupportedEffectV1::CancelOpponentCombatStatModifiers { .. }
                );
            if execute {
                let compact = compact_effect(*effect).ok_or(
                    ClanBonusDiagnosticPreparationErrorV1::UnsupportedCompiledShape {
                        battle_id,
                        player,
                        hand_slot,
                        source_kind,
                        effect_id: source.id,
                    },
                )?;
                Ok((
                    DiagnosticProjectionDispositionV1::Execute {
                        identity,
                        effect: *effect,
                    },
                    DiagnosticSourcePlanV1::Execute {
                        source_id: source.id,
                        effect: compact,
                    },
                ))
            } else {
                Ok((
                    DiagnosticProjectionDispositionV1::Disabled {
                        identity,
                        reason: DiagnosticDisabledReasonV1::OrdinaryAbility {
                            registry_reasons: Box::new([]),
                        },
                    },
                    DiagnosticSourcePlanV1::Disabled {
                        source_id: source.id,
                    },
                ))
            }
        }
        CompiledEffectV1::Unsupported(reasons) => {
            let attempted_control = attempts_promised_control(definition.structured_input());
            let reason = if attempted_control {
                DiagnosticDisabledReasonV1::UnsupportedPromisedControl {
                    registry_reasons: reasons.clone(),
                }
            } else if source_kind == DiagnosticEffectSourceV1::Ability {
                DiagnosticDisabledReasonV1::OrdinaryAbility {
                    registry_reasons: reasons.clone(),
                }
            } else {
                DiagnosticDisabledReasonV1::OutOfSliceBonus {
                    registry_reasons: reasons.clone(),
                }
            };
            let compact = if attempted_control {
                DiagnosticSourcePlanV1::RejectIfSelected {
                    source_id: source.id,
                }
            } else {
                DiagnosticSourcePlanV1::Disabled {
                    source_id: source.id,
                }
            };
            Ok((
                DiagnosticProjectionDispositionV1::Disabled { identity, reason },
                compact,
            ))
        }
    }
}

fn attempts_promised_control(input: &crate::effect_registry::StructuredEffectV1) -> bool {
    input.special_action == SpecialActionV1::StopBonus
        || (input.attribute_action == AttributeActionV1::StopModifier
            && matches!(
                input.attribute_affected,
                AttributeAffectedV1::Attack
                    | AttributeAffectedV1::Damage
                    | AttributeAffectedV1::Power
                    | AttributeAffectedV1::PowerAndDamage
            ))
}

fn compact_card_plan(prepared: &DiagnosticCardPreparationV1) -> DiagnosticCardPlanV1 {
    DiagnosticCardPlanV1 {
        key: prepared.key,
        ability: compact_source_plan(&prepared.ability),
        bonus: compact_source_plan(&prepared.bonus),
        source_bonus_support_count: prepared.source_bonus_support_count,
    }
}

fn compact_source_plan(disposition: &DiagnosticProjectionDispositionV1) -> DiagnosticSourcePlanV1 {
    match disposition {
        DiagnosticProjectionDispositionV1::Absent => DiagnosticSourcePlanV1::Absent,
        DiagnosticProjectionDispositionV1::Execute { identity, effect } => {
            DiagnosticSourcePlanV1::Execute {
                source_id: identity.id,
                effect: compact_effect(*effect)
                    .expect("prepared executable effects must have a compact representation"),
            }
        }
        DiagnosticProjectionDispositionV1::Disabled { identity, reason } => {
            if matches!(
                reason,
                DiagnosticDisabledReasonV1::UnsupportedPromisedControl { .. }
            ) {
                DiagnosticSourcePlanV1::RejectIfSelected {
                    source_id: identity.id,
                }
            } else {
                DiagnosticSourcePlanV1::Disabled {
                    source_id: identity.id,
                }
            }
        }
    }
}

fn compact_effect(effect: SupportedEffectV1) -> Option<DiagnosticCombatEffectV1> {
    match effect {
        SupportedEffectV1::ModifyCombatStat {
            side,
            stat,
            operation,
            value,
            minimum,
            maximum,
            multiplier,
        } => Some(DiagnosticCombatEffectV1::ModifyCombatStat {
            side: match side {
                AffectedSideV1::Opponent => DiagnosticAffectedSideV1::Opponent,
                AffectedSideV1::Player => DiagnosticAffectedSideV1::Player,
                AffectedSideV1::Both => return None,
            },
            stat: compact_stat(stat),
            operation: match operation {
                StatOperationV1::Decrease => DiagnosticStatOperationV1::Decrease,
                StatOperationV1::Increase => DiagnosticStatOperationV1::Increase,
            },
            value,
            minimum,
            maximum,
            multiplier: match multiplier {
                MagnitudeMultiplierV1::Fixed => DiagnosticMagnitudeV1::Fixed,
                MagnitudeMultiplierV1::Support => DiagnosticMagnitudeV1::SourceBonusSupport,
            },
        }),
        SupportedEffectV1::StopOpponentBonus => Some(DiagnosticCombatEffectV1::StopOpponentBonus),
        SupportedEffectV1::CancelOpponentCombatStatModifiers { stat } => Some(
            DiagnosticCombatEffectV1::CancelOpponentCombatStatModifiers {
                stat: compact_stat(stat),
            },
        ),
    }
}

const fn compact_stat(stat: CombatStatV1) -> DiagnosticCombatStatV1 {
    match stat {
        CombatStatV1::Attack => DiagnosticCombatStatV1::Attack,
        CombatStatV1::Damage => DiagnosticCombatStatV1::Damage,
        CombatStatV1::Power => DiagnosticCombatStatV1::Power,
        CombatStatV1::PowerAndDamage => DiagnosticCombatStatV1::PowerAndDamage,
    }
}

fn validate_replay(
    replay: &ReplayCaseV1,
    catalog: &CardCatalog,
) -> Result<(), ReplayValidationError> {
    let battle_id = replay.metadata.battle_id;
    if replay.schema_version != REPLAY_SCHEMA_VERSION {
        return Err(invalid(
            battle_id,
            "schema_version",
            format!(
                "expected {REPLAY_SCHEMA_VERSION}, found {}",
                replay.schema_version
            ),
        ));
    }
    if replay.rounds.len() > MAX_ROUNDS as usize {
        return Err(invalid(
            battle_id,
            "rounds",
            format!(
                "expected at most {MAX_ROUNDS}, found {}",
                replay.rounds.len()
            ),
        ));
    }

    for player in PlayerId::ALL {
        let engine_player = as_engine_player(player);
        let source = &replay.players[player.index()];
        if source.engine_player != engine_player {
            return Err(invalid(
                battle_id,
                format!("players[{}].engine_player", player.index()),
                format!(
                    "expected {engine_player:?}, found {:?}",
                    source.engine_player
                ),
            ));
        }
        let expected_side = if player == PlayerId::P1 {
            replay.metadata.source_first_side
        } else {
            replay.metadata.source_first_side.other()
        };
        if source.source_side != expected_side {
            return Err(invalid(
                battle_id,
                format!("players[{}].source_side", player.index()),
                format!("expected {expected_side:?}, found {:?}", source.source_side),
            ));
        }
        for (index, card) in source.hand.iter().enumerate() {
            if card.hand_index as usize != index {
                return Err(invalid(
                    battle_id,
                    format!("players[{}].hand[{index}].hand_index", player.index()),
                    format!("expected {index}, found {}", card.hand_index),
                ));
            }
            if catalog.get(card.key).is_none() {
                return Err(invalid(
                    battle_id,
                    format!("players[{}].hand[{index}].key", player.index()),
                    format!(
                        "unknown canonical card {} level {}",
                        card.key.id, card.key.level
                    ),
                ));
            }
        }
    }

    let mut used = ByPlayer::new([false; HAND_SIZE], [false; HAND_SIZE]);
    for (index, round) in replay.rounds.iter().enumerate() {
        if round.round as usize != index {
            return Err(invalid(
                battle_id,
                format!("rounds[{index}].round"),
                format!("expected {index}, found {}", round.round),
            ));
        }
        let first_mover = as_player_id(round.first_mover);
        if index == 0 && first_mover != PlayerId::P1 {
            return Err(invalid(
                battle_id,
                "rounds[0].first_mover",
                format!("expected P1, found {first_mover:?}"),
            ));
        }
        let expected_source_first = replay.players[first_mover.index()].source_side;
        if round.source_first_side != expected_source_first {
            return Err(invalid(
                battle_id,
                format!("rounds[{index}].source_first_side"),
                format!(
                    "expected {expected_source_first:?} for {:?}, found {:?}",
                    round.first_mover, round.source_first_side
                ),
            ));
        }
        validate_play(replay, round, index, 0, first_mover, &mut used)?;
        validate_play(replay, round, index, 1, first_mover.other(), &mut used)?;
    }
    Ok(())
}

fn validate_play(
    replay: &ReplayCaseV1,
    _round: &ReplayRound,
    round_index: usize,
    play_index: usize,
    expected_player: PlayerId,
    used: &mut ByPlayer<[bool; HAND_SIZE]>,
) -> Result<(), ReplayValidationError> {
    let battle_id = replay.metadata.battle_id;
    let play = &_round.plays[play_index];
    let actual_player = as_player_id(play.engine_player);
    if actual_player != expected_player {
        return Err(invalid(
            battle_id,
            format!("rounds[{round_index}].plays[{play_index}].engine_player"),
            format!("expected {expected_player:?}, found {actual_player:?}"),
        ));
    }
    let expected_side = replay.players[actual_player.index()].source_side;
    if play.source_side != expected_side {
        return Err(invalid(
            battle_id,
            format!("rounds[{round_index}].plays[{play_index}].source_side"),
            format!("expected {expected_side:?}, found {:?}", play.source_side),
        ));
    }
    let slot = usize::from(play.hand_index);
    if slot >= HAND_SIZE {
        return Err(invalid(
            battle_id,
            format!("rounds[{round_index}].plays[{play_index}].hand_index"),
            format!("expected 0..{}, found {slot}", HAND_SIZE - 1),
        ));
    }
    let hand_card = replay.players[actual_player.index()].hand[slot].key;
    if play.card != hand_card {
        return Err(invalid(
            battle_id,
            format!("rounds[{round_index}].plays[{play_index}].card"),
            format!("expected {hand_card:?}, found {:?}", play.card),
        ));
    }
    if used[actual_player][slot] {
        return Err(invalid(
            battle_id,
            format!("rounds[{round_index}].plays[{play_index}].hand_index"),
            format!("{actual_player:?} already played slot {slot}"),
        ));
    }
    used[actual_player][slot] = true;
    let source_pillz_used = play.pillz.checked_add(1).ok_or_else(|| {
        invalid(
            battle_id,
            format!("rounds[{round_index}].plays[{play_index}].pillz"),
            "cannot add the source free pill within u16",
        )
    })?;
    if play.source_pillz_used != source_pillz_used {
        return Err(invalid(
            battle_id,
            format!("rounds[{round_index}].plays[{play_index}].source_pillz_used"),
            format!(
                "expected {source_pillz_used}, found {}",
                play.source_pillz_used
            ),
        ));
    }
    Ok(())
}

fn round_input(
    battle_id: u64,
    round: &ReplayRound,
) -> (BaseRulesRoundInput, BaseRulesReplayRoundContext) {
    let mut selections = ByPlayer::new(
        BaseRulesSelection::new(0, 0, false),
        BaseRulesSelection::new(0, 0, false),
    );
    let mut cards = ByPlayer::new(CardKey::new(0, 0), CardKey::new(0, 0));
    for play in &round.plays {
        let player = as_player_id(play.engine_player);
        selections[player] = selection(play);
        cards[player] = play.card;
    }
    let first_mover = as_player_id(round.first_mover);
    let input = BaseRulesRoundInput {
        first_mover,
        selections,
    };
    let context = BaseRulesReplayRoundContext {
        battle_id,
        round: round.round,
        cards,
        selections,
        first_mover,
    };
    (input, context)
}

fn selection(play: &ReplayPlay) -> BaseRulesSelection {
    BaseRulesSelection::new(play.hand_index, play.pillz, play.fury)
}

fn assert_round(
    expected: &ReplayRound,
    actual: &BaseRulesRoundReport,
    context: BaseRulesReplayRoundContext,
) -> Result<(), BaseRulesReplayError> {
    for player in PlayerId::ALL {
        let expected_player = expected.expected_player_states[player.index()];
        compare_field(
            context,
            format!("players.{player:?}.life"),
            expected_player.life,
            actual.players[player].life,
        )?;
        compare_field(
            context,
            format!("players.{player:?}.pillz"),
            expected_player.pillz,
            actual.players[player].pillz,
        )?;
        if let Some(expected_card) = expected.expected_card_results[player.index()] {
            assert_card(context, player, expected_card, actual.cards[player])?;
        }
    }
    Ok(())
}

fn assert_diagnostic_round(
    expected: &ReplayRound,
    actual: &BaseRulesRoundReport,
    context: BaseRulesReplayRoundContext,
) -> Result<(), ClanBonusDiagnosticReplayErrorV1> {
    for player in PlayerId::ALL {
        let expected_player = expected.expected_player_states[player.index()];
        compare_diagnostic_field(
            context,
            format!("players.{player:?}.life"),
            expected_player.life,
            actual.players[player].life,
        )?;
        compare_diagnostic_field(
            context,
            format!("players.{player:?}.pillz"),
            expected_player.pillz,
            actual.players[player].pillz,
        )?;
        if let Some(expected_card) = expected.expected_card_results[player.index()] {
            let actual_card = actual.cards[player];
            compare_diagnostic_field(
                context,
                format!("cards.{player:?}.power"),
                expected_card.power,
                actual_card.power,
            )?;
            compare_diagnostic_field(
                context,
                format!("cards.{player:?}.damage"),
                expected_card.damage,
                actual_card.damage,
            )?;
            compare_diagnostic_field(
                context,
                format!("cards.{player:?}.attack"),
                expected_card.attack,
                actual_card.attack,
            )?;
            compare_diagnostic_field(
                context,
                format!("cards.{player:?}.won"),
                expected_card.won,
                actual_card.won,
            )?;
        }
    }
    Ok(())
}

fn compare_diagnostic_field<T: fmt::Debug + PartialEq>(
    context: BaseRulesReplayRoundContext,
    field: String,
    expected: T,
    actual: T,
) -> Result<(), ClanBonusDiagnosticReplayErrorV1> {
    if expected == actual {
        Ok(())
    } else {
        Err(ClanBonusDiagnosticReplayErrorV1::Mismatch {
            context,
            field,
            expected: format!("{expected:?}"),
            actual: format!("{actual:?}"),
        })
    }
}

fn assert_card(
    context: BaseRulesReplayRoundContext,
    player: PlayerId,
    expected: ExpectedCardResult,
    actual: BaseRulesCardResult,
) -> Result<(), BaseRulesReplayError> {
    compare_field(
        context,
        format!("cards.{player:?}.power"),
        expected.power,
        actual.power,
    )?;
    compare_field(
        context,
        format!("cards.{player:?}.damage"),
        expected.damage,
        actual.damage,
    )?;
    compare_field(
        context,
        format!("cards.{player:?}.attack"),
        expected.attack,
        actual.attack,
    )?;
    compare_field(
        context,
        format!("cards.{player:?}.won"),
        expected.won,
        actual.won,
    )
}

fn compare_field<T: fmt::Debug + PartialEq>(
    context: BaseRulesReplayRoundContext,
    field: String,
    expected: T,
    actual: T,
) -> Result<(), BaseRulesReplayError> {
    if expected == actual {
        Ok(())
    } else {
        Err(BaseRulesReplayError::Mismatch {
            context,
            field,
            expected: format!("{expected:?}"),
            actual: format!("{actual:?}"),
        })
    }
}

const fn as_player_id(player: EnginePlayer) -> PlayerId {
    match player {
        EnginePlayer::P1 => PlayerId::P1,
        EnginePlayer::P2 => PlayerId::P2,
    }
}

const fn as_engine_player(player: PlayerId) -> EnginePlayer {
    match player {
        PlayerId::P1 => EnginePlayer::P1,
        PlayerId::P2 => EnginePlayer::P2,
    }
}

fn invalid(
    battle_id: u64,
    field: impl Into<String>,
    detail: impl Into<String>,
) -> ReplayValidationError {
    ReplayValidationError {
        battle_id,
        field: field.into(),
        detail: detail.into(),
    }
}
